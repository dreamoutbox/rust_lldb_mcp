use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use tokio::fs::{OpenOptions, create_dir_all};
use tokio::io::AsyncWriteExt;

use crate::{
    backtrace::FrameInfo,
    tools::{EvalResult, VariableInfo},
};

pub const DEFAULT_LOG_DIR: &str = "rust_lldb_mcp_logs";

/// Logger for recording inspection results to `rust_lldb_mcp_logs/<session_id>.log`.
#[derive(Debug, Clone)]
pub struct SessionLogger {
    log_dir: PathBuf,
}

impl Default for SessionLogger {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from(DEFAULT_LOG_DIR),
        }
    }
}

impl SessionLogger {
    pub fn new(log_dir: impl Into<PathBuf>) -> Self {
        Self {
            log_dir: log_dir.into(),
        }
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    /// Appends a single inspection log entry with timestamp and tool name.
    async fn append_entry(&self, session_id: &str, tool_name: &str, details: &str) {
        if let Err(e) = create_dir_all(&self.log_dir).await {
            tracing::warn!(?e, "failed to create session log directory");
            return;
        }

        let file_path = self.log_dir.join(format!("{session_id}.log"));
        let timestamp = chrono::Utc::now().to_rfc3339();
        let line = format!("[{timestamp}] [{tool_name}] {details}\n");

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await
        {
            let _ = file.write_all(line.as_bytes()).await;
        }
    }

    /// Logs `print_variable` result.
    pub async fn log_variable(&self, session_id: &str, var: &VariableInfo) {
        let type_str = var.var_type.as_deref().unwrap_or("unknown");
        let details = format!(
            "name: {} | type: {} | value: {}",
            var.name, type_str, var.value
        );
        self.append_entry(session_id, "print_variable", &details)
            .await;
    }

    /// Logs `print_frame_info` result.
    pub async fn log_frame_info(&self, session_id: &str, frame: &FrameInfo) {
        let loc = match (&frame.file, frame.line) {
            (Some(f), Some(l)) => format!(" at {f}:{l}"),
            (Some(f), None) => format!(" at {f}"),
            _ => String::new(),
        };
        let details = format!(
            "frame #{}: 0x{} {}{loc}",
            frame.index, frame.address, frame.function
        );
        self.append_entry(session_id, "print_frame_info", &details)
            .await;
    }

    /// Logs `print_backtrace` result.
    pub async fn log_backtrace(&self, session_id: &str, frames: &[FrameInfo]) {
        let mut details = format!("({} frames)", frames.len());
        for f in frames {
            let loc = match (&f.file, f.line) {
                (Some(file), Some(line)) => format!(" at {file}:{line}"),
                (Some(file), None) => format!(" at {file}"),
                _ => String::new(),
            };
            let _ = write!(
                details,
                "\n  #{} 0x{} {}{loc}",
                f.index, f.address, f.function
            );
        }
        self.append_entry(session_id, "print_backtrace", &details)
            .await;
    }

    /// Logs `evaluate_expression` result.
    pub async fn log_eval(&self, session_id: &str, expr: &str, eval: &EvalResult) {
        let details = if let Some(err) = &eval.error {
            format!("expr: {expr:?} | error: {err}")
        } else {
            let t = eval.val_type.as_deref().unwrap_or("unknown");
            let v = eval.value.as_deref().unwrap_or("<empty>");
            format!("expr: {expr:?} | type: {t} | value: {v}")
        };
        self.append_entry(session_id, "evaluate_expression", &details)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs::{read_to_string, remove_dir_all};

    #[tokio::test]
    async fn test_session_logger_writes_file() {
        let temp_dir = PathBuf::from("target/test_logs_fixture");
        let _ = remove_dir_all(&temp_dir).await;

        let logger = SessionLogger::new(&temp_dir);
        let session_id = "test-session-123";

        logger
            .log_variable(
                session_id,
                &VariableInfo {
                    name: "x".to_string(),
                    var_type: Some("int".to_string()),
                    value: "42".to_string(),
                    raw: "(int) x = 42".to_string(),
                },
            )
            .await;

        logger
            .log_eval(
                session_id,
                "x + 1",
                &EvalResult {
                    value: Some("43".to_string()),
                    val_type: Some("int".to_string()),
                    error: None,
                    raw: "(int) $0 = 43".to_string(),
                },
            )
            .await;

        let log_file = temp_dir.join(format!("{session_id}.log"));
        let content = read_to_string(&log_file).await.expect("read log file");

        assert!(content.contains("[print_variable] name: x | type: int | value: 42"));
        assert!(content.contains("[evaluate_expression] expr: \"x + 1\" | type: int | value: 43"));

        let _ = remove_dir_all(&temp_dir).await;
    }
}
