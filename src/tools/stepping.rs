use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    driver::{extract_error_message, is_error_output, parse_stop_info},
    error::LldbMcpError,
    session::{SessionStore, parse_session_id},
};

/// Information describing a process stop event after stepping or continuing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopInfo {
    pub stop_reason: Option<String>,
    pub thread_id: Option<u64>,
    pub frame: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub raw_output: String,
}

/// Parameters for continuing execution.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ContinueParams {
    pub session_id: String,
}

/// Parameters for stepping (over, into, out, or finish).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct StepParams {
    pub session_id: String,
    pub thread_id: Option<u64>,
}

pub async fn handle_continue(
    store: &SessionStore,
    params: ContinueParams,
) -> Result<StopInfo, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let output = session.process.send_command("process continue").await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::ProcessNotRunning(err));
    }

    let parsed = parse_stop_info(&output);

    Ok(StopInfo {
        stop_reason: parsed.stop_reason,
        thread_id: parsed.thread_id,
        frame: parsed.frame,
        file: parsed.file,
        line: parsed.line,
        raw_output: output,
    })
}

pub async fn handle_step_next(
    store: &SessionStore,
    params: StepParams,
) -> Result<StopInfo, LldbMcpError> {
    let cmd = match params.thread_id {
        Some(tid) => format!("thread step-over -t {tid}"),
        None => "thread step-over".to_string(),
    };
    execute_step(store, &params.session_id, &cmd).await
}

pub async fn handle_step_into(
    store: &SessionStore,
    params: StepParams,
) -> Result<StopInfo, LldbMcpError> {
    let cmd = match params.thread_id {
        Some(tid) => format!("thread step-in -t {tid}"),
        None => "thread step-in".to_string(),
    };
    execute_step(store, &params.session_id, &cmd).await
}

pub async fn handle_step_out(
    store: &SessionStore,
    params: StepParams,
) -> Result<StopInfo, LldbMcpError> {
    let cmd = match params.thread_id {
        Some(tid) => format!("thread step-out -t {tid}"),
        None => "thread step-out".to_string(),
    };
    execute_step(store, &params.session_id, &cmd).await
}

pub async fn handle_finish_frame(
    store: &SessionStore,
    params: StepParams,
) -> Result<StopInfo, LldbMcpError> {
    let cmd = match params.thread_id {
        Some(tid) => format!("thread step-out -t {tid}"),
        None => "finish".to_string(),
    };
    execute_step(store, &params.session_id, &cmd).await
}

async fn execute_step(
    store: &SessionStore,
    session_id_str: &str,
    cmd: &str,
) -> Result<StopInfo, LldbMcpError> {
    let session_id = parse_session_id(session_id_str)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(session_id_str.to_string()))?;

    let output = session.process.send_command(cmd).await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Thread(err));
    }

    let parsed = parse_stop_info(&output);

    Ok(StopInfo {
        stop_reason: parsed.stop_reason,
        thread_id: parsed.thread_id,
        frame: parsed.frame,
        file: parsed.file,
        line: parsed.line,
        raw_output: output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::create_session;
    use crate::tools::breakpoints::{
        BreakpointLocation, SetBreakpointParams, handle_set_breakpoint,
    };
    use crate::tools::program::{
        LoadProgramParams, RunProgramParams, handle_load_program, handle_run_program,
    };

    #[tokio::test]
    async fn test_stepping_in_testprog() {
        let store = SessionStore::new();
        let session_id = create_session(&store, "step_test".to_string())
            .await
            .expect("create session");

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

        handle_set_breakpoint(
            &store,
            SetBreakpointParams {
                session_id: session_id.to_string(),
                location: BreakpointLocation::Function {
                    name: "add".to_string(),
                },
                condition: None,
            },
        )
        .await
        .expect("set breakpoint on add");

        let run_res = handle_run_program(
            &store,
            RunProgramParams {
                session_id: session_id.to_string(),
                stop_at_entry: None,
            },
        )
        .await
        .expect("run program");
        assert!(run_res.pid.is_some());

        // Step out of add back to main
        let step_out_res = handle_step_out(
            &store,
            StepParams {
                session_id: session_id.to_string(),
                thread_id: None,
            },
        )
        .await
        .expect("step out");
        assert!(step_out_res.raw_output.contains("main") || step_out_res.frame.is_some());

        // Step next in main
        let step_next_res = handle_step_next(
            &store,
            StepParams {
                session_id: session_id.to_string(),
                thread_id: None,
            },
        )
        .await
        .expect("step next");
        assert!(step_next_res.raw_output.contains("main") || step_next_res.line.is_some());
    }
}
