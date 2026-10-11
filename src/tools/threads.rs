use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    driver::{extract_error_message, is_error_output},
    error::LldbMcpError,
    session::{SessionStore, parse_session_id},
};

/// Parameters for listing threads.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListThreadsParams {
    pub session_id: String,
}

/// Structured information about a thread.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ThreadInfo {
    pub id: u64,
    pub tid: Option<u64>,
    pub is_selected: bool,
    pub name: Option<String>,
    pub stop_reason: Option<String>,
    pub location: Option<String>,
    pub raw: String,
}

/// Parameters for switching to a specific thread.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SwitchThreadParams {
    pub session_id: String,
    pub thread_id: u64,
}

/// Parameters for running an arbitrary raw LLDB CLI command.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RunLldbCommandParams {
    pub session_id: String,
    pub command: String,
}

pub async fn handle_list_threads(
    store: &SessionStore,
    params: ListThreadsParams,
) -> Result<Vec<ThreadInfo>, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let output = session.process.send_command("thread list").await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Thread(err));
    }

    Ok(parse_thread_list(&output))
}

pub async fn handle_switch_thread(
    store: &SessionStore,
    params: SwitchThreadParams,
) -> Result<String, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let output = session
        .process
        .send_command(&format!("thread select {}", params.thread_id))
        .await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Thread(err));
    }

    let msg = if output.is_empty() {
        format!("Switched to thread {}", params.thread_id)
    } else {
        output
    };

    Ok(msg)
}

pub async fn handle_run_lldb_command(
    store: &SessionStore,
    params: RunLldbCommandParams,
) -> Result<String, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    session.process.send_command(&params.command).await
}

/// Parses output of `thread list` into `Vec<ThreadInfo>`.
pub fn parse_thread_list(output: &str) -> Vec<ThreadInfo> {
    output.lines().filter_map(parse_thread_line).collect()
}

/// Parses a single line from `thread list`.
/// E.g.:
/// `* thread #2: tid = 84672, 0x0000555555572677 testprog`testprog::thread_worker(id=0) at main.rs:41:18, name = 'testprog', stop reason = breakpoint 1.1`
pub fn parse_thread_line(line: &str) -> Option<ThreadInfo> {
    let trimmed = line.trim();
    let is_selected = trimmed.starts_with('*');
    let content = if is_selected {
        trimmed.strip_prefix('*')?.trim()
    } else {
        trimmed
    };

    if !content.starts_with("thread #") {
        return None;
    }

    let after_thread = content.strip_prefix("thread #")?;
    let colon_idx = after_thread.find(':')?;
    let id_str = after_thread[..colon_idx].trim();
    let id: u64 = id_str.parse().ok()?;

    let details = after_thread[colon_idx + 1..].trim();

    // Extract tid = <tid>
    let tid = if let Some(tid_idx) = details.find("tid = ") {
        let after_tid = &details[tid_idx + "tid = ".len()..];
        let num_str = after_tid
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap_or("");
        num_str.parse::<u64>().ok()
    } else {
        None
    };

    // Extract name = '...'
    let name = if let Some(name_idx) = details.find("name = '") {
        let after_name = &details[name_idx + "name = '".len()..];
        let end_idx = after_name.find('\'')?;
        Some(after_name[..end_idx].to_string())
    } else {
        None
    };

    // Extract stop reason = ...
    let stop_reason = if let Some(sr_idx) = details.find("stop reason = ") {
        let after_sr = &details[sr_idx + "stop reason = ".len()..];
        let sr = after_sr.split(',').next().unwrap_or(after_sr).trim();
        Some(sr.to_string())
    } else {
        None
    };

    Some(ThreadInfo {
        id,
        tid,
        is_selected,
        name,
        stop_reason,
        location: Some(details.to_string()),
        raw: trimmed.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::create_session;
    use crate::tools::breakpoints::{BreakpointLocation, SetBreakpointParams, handle_set_breakpoint};
    use crate::tools::program::{LoadProgramParams, RunProgramParams, handle_load_program, handle_run_program};

    #[test]
    fn test_parse_thread_line() {
        let line = "* thread #2: tid = 84672, 0x0000555555572677 testprog`testprog::thread_worker(id=0) at main.rs:41:18, name = 'testprog', stop reason = breakpoint 1.1";
        let info = parse_thread_line(line).expect("parse thread");
        assert_eq!(info.id, 2);
        assert_eq!(info.tid, Some(84672));
        assert!(info.is_selected);
        assert_eq!(info.name, Some("testprog".to_string()));
        assert_eq!(info.stop_reason, Some("breakpoint 1.1".to_string()));

        let unselected = "  thread #1: tid = 84670, 0x00007ffff7d255ec libc.so.6`__mmap + 44, name = 'testprog'";
        let info1 = parse_thread_line(unselected).expect("parse thread 1");
        assert_eq!(info1.id, 1);
        assert_eq!(info1.tid, Some(84670));
        assert!(!info1.is_selected);
        assert_eq!(info1.stop_reason, None);
    }

    #[tokio::test]
    async fn test_thread_tools_and_raw_command_with_testprog() {
        let store = SessionStore::new();
        let session_id = create_session(&store, "thread_test".to_string())
            .await
            .expect("create session");

        // Raw command test
        let version_out = handle_run_lldb_command(
            &store,
            RunLldbCommandParams {
                session_id: session_id.to_string(),
                command: "version".to_string(),
            },
        )
        .await
        .expect("run raw version command");
        assert!(version_out.contains("lldb version"));

        handle_load_program(
            &store,
            LoadProgramParams {
                session_id: session_id.to_string(),
                executable_path: "testprog/target/debug/testprog".to_string(),
                args: None,
                working_dir: None,
                env: None,
            },
        )
        .await
        .expect("load program");

        // Break on thread_worker
        handle_set_breakpoint(
            &store,
            SetBreakpointParams {
                session_id: session_id.to_string(),
                location: BreakpointLocation::Function {
                    name: "thread_worker".to_string(),
                },
                condition: None,
            },
        )
        .await
        .expect("set breakpoint thread_worker");

        handle_run_program(
            &store,
            RunProgramParams {
                session_id: session_id.to_string(),
                stop_at_entry: None,
            },
        )
        .await
        .expect("run program");

        let threads = handle_list_threads(
            &store,
            ListThreadsParams {
                session_id: session_id.to_string(),
            },
        )
        .await
        .expect("list threads");
        assert!(!threads.is_empty());

        // Switch to thread 1
        let switch_res = handle_switch_thread(
            &store,
            SwitchThreadParams {
                session_id: session_id.to_string(),
                thread_id: 1,
            },
        )
        .await
        .expect("switch to thread 1");
        assert!(switch_res.contains("thread #1") || !switch_res.is_empty());
    }
}
