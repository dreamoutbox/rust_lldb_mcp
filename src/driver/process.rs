use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

use crate::error::LldbMcpError;

pub const DONE_SENTINEL: &str = "___LLDB_DONE___";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// Controls an interactive rust-lldb subprocess over stdin and stdout.
pub struct LldbProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl LldbProcess {
    /// Spawns a new rust-lldb subprocess configured for non-interactive control.
    pub async fn spawn() -> Result<Self, LldbMcpError> {
        let mut child = Command::new("rust-lldb")
            .args([
                "--no-use-colors",
                "--source-quietly",
                "-O",
                "settings set auto-confirm true",
                "-O",
                "settings set target.disable-stdio true",
                "-O",
                "script lldb.debugger.SetAsync(False)",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(LldbMcpError::Io)?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LldbMcpError::Lldb("Failed to open child stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LldbMcpError::Lldb("Failed to open child stdout".to_string()))?;

        let mut proc = Self {
            child,
            stdin,
            stdout,
        };

        // LLDB interactive loop defaults to async mode (True). Force synchronous mode
        // so that process launch, continue, and step commands block until stop events occur.
        proc.send_command("script lldb.debugger.SetAsync(False)").await?;

        Ok(proc)
    }

    /// Sends a command to LLDB and reads the response until the DONE sentinel.
    pub async fn send_command(&mut self, cmd: &str) -> Result<String, LldbMcpError> {
        self.send_command_with_timeout(cmd, DEFAULT_TIMEOUT).await
    }

    /// Sends a command to LLDB with a custom timeout.
    pub async fn send_command_with_timeout(
        &mut self,
        cmd: &str,
        duration: Duration,
    ) -> Result<String, LldbMcpError> {
        // We write the command, followed by a script that outputs our unique delimiter.
        // We construct the script via string concatenation in python so the literal
        // DONE_SENTINEL is not present in the command echo.
        let payload = format!("{cmd}\nscript print(\"___LLDB_\" + \"DONE___\")\n");
        self.stdin
            .write_all(payload.as_bytes())
            .await
            .map_err(LldbMcpError::Io)?;
        self.stdin.flush().await.map_err(LldbMcpError::Io)?;

        let raw_output = self.read_until_sentinel(duration).await?;
        Ok(clean_output(&raw_output, cmd))
    }

    /// Reads stdout chunks until a standalone DONE_SENTINEL line is observed.
    async fn read_until_sentinel(&mut self, duration: Duration) -> Result<String, LldbMcpError> {
        let read_future = async {
            let mut accumulator: Vec<u8> = Vec::with_capacity(4096);
            let mut buffer = [0u8; 1024];

            loop {
                let bytes_read = self.stdout.read(&mut buffer).await?;
                if bytes_read == 0 {
                    // Child process closed stdout (e.g. exited)
                    break;
                }
                accumulator.extend_from_slice(&buffer[..bytes_read]);

                // Check if any completed line matches the sentinel
                let text = String::from_utf8_lossy(&accumulator);
                if text.lines().any(|l| l.trim() == DONE_SENTINEL) {
                    break;
                }
            }

            Ok::<String, LldbMcpError>(String::from_utf8_lossy(&accumulator).into_owned())
        };

        timeout(duration, read_future)
            .await
            .map_err(|_| LldbMcpError::Timeout(duration.as_millis() as u64))?
    }

    /// Gracefully quits the debugger subprocess or kills it if unresponsive.
    pub async fn terminate(&mut self) -> Result<(), LldbMcpError> {
        let _ = self.stdin.write_all(b"quit\n").await;
        let _ = self.stdin.flush().await;

        match timeout(Duration::from_secs(2), self.child.wait()).await {
            Ok(_) => Ok(()),
            Err(_) => {
                self.child.kill().await.map_err(LldbMcpError::Io)?;
                Ok(())
            }
        }
    }
}

/// Strips echoed command headers, prompts, and the sentinel line.
fn clean_output(raw: &str, executed_cmd: &str) -> String {
    let mut cleaned_lines = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();

        // Skip prompt echoes, command echo, and the delimiter marker
        if trimmed == DONE_SENTINEL
            || trimmed == "script print(\"___LLDB_\" + \"DONE___\")"
            || trimmed == "(lldb) script print(\"___LLDB_\" + \"DONE___\")"
            || trimmed == executed_cmd
            || trimmed == format!("(lldb) {executed_cmd}")
        {
            continue;
        }

        // If the line starts with (lldb) prompt and the command, strip it
        if let Some(rest) = trimmed.strip_prefix("(lldb) ")
            && rest == executed_cmd
        {
            continue;
        }

        cleaned_lines.push(line);
    }

    cleaned_lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_output() {
        let raw = format!(
            "(lldb) version\nlldb version 19.1.1\n(lldb) script print(\"___LLDB_\" + \"DONE___\")\n{DONE_SENTINEL}\n"
        );
        let cleaned = clean_output(&raw, "version");
        assert_eq!(cleaned, "lldb version 19.1.1");
    }

    #[tokio::test]
    async fn test_spawn_and_version() {
        let mut proc = LldbProcess::spawn()
            .await
            .expect("Failed to spawn LLDB process");
        let output = proc
            .send_command("version")
            .await
            .expect("Failed to send version command");
        assert!(output.contains("lldb version"), "Output was: {output}");
        proc.terminate().await.expect("Failed to terminate LLDB");
    }
}
