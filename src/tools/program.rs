use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    driver::{extract_error_message, is_error_output, parse_process_id},
    error::LldbMcpError,
    session::{SessionStore, parse_session_id},
};

/// Parameters for loading an executable binary.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LoadProgramParams {
    pub session_id: String,
    pub executable_path: String,
    pub args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub env: Option<Vec<String>>,
}

/// Parameters for running/launching the program.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RunProgramParams {
    pub session_id: String,
    pub stop_at_entry: Option<bool>,
}

/// Result of launching a program.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProcessInfo {
    pub pid: Option<u32>,
    pub output: String,
}

/// Parameters for killing the running target process.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct KillProcessParams {
    pub session_id: String,
}

pub async fn handle_load_program(
    store: &SessionStore,
    params: LoadProgramParams,
) -> Result<String, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    // Set working directory if specified
    if let Some(ref cwd) = params.working_dir {
        session
            .process
            .send_command(&format!("settings set target.working-dir \"{cwd}\""))
            .await?;
    }

    // Set run arguments if specified
    if let Some(ref args) = params.args {
        let joined_args = args
            .iter()
            .map(|a| format!("\"{a}\""))
            .collect::<Vec<_>>()
            .join(" ");
        session
            .process
            .send_command(&format!("settings set target.run-args {joined_args}"))
            .await?;
    }

    // Set environment variables if specified
    if let Some(ref env_vars) = params.env {
        for env in env_vars {
            session
                .process
                .send_command(&format!("settings set target.env-vars {env}"))
                .await?;
        }
    }

    let output = session
        .process
        .send_command(&format!("target create \"{}\"", params.executable_path))
        .await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Lldb(err));
    }

    session.target_executable = Some(params.executable_path);
    Ok(output)
}

pub async fn handle_run_program(
    store: &SessionStore,
    params: RunProgramParams,
) -> Result<ProcessInfo, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    if session.target_executable.is_none() {
        return Err(LldbMcpError::TargetNotLoaded(params.session_id.clone()));
    }

    let cmd = if params.stop_at_entry == Some(true) {
        "process launch --stop-at-entry"
    } else {
        "process launch"
    };

    let output = session.process.send_command(cmd).await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Lldb(err));
    }

    let pid = parse_process_id(&output);

    Ok(ProcessInfo { pid, output })
}

pub async fn handle_kill_process(
    store: &SessionStore,
    params: KillProcessParams,
) -> Result<String, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let output = session.process.send_command("process kill").await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::ProcessNotRunning(err));
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::create_session;

    #[tokio::test]
    async fn test_program_lifecycle_with_testprog() {
        let store = SessionStore::new();
        let session_id = create_session(&store, "prog_test".to_string())
            .await
            .expect("create session");

        let exe_path = "testprog/target/debug/testprog";

        let load_res = handle_load_program(
            &store,
            LoadProgramParams {
                session_id: session_id.to_string(),
                executable_path: exe_path.to_string(),
                args: None,
                working_dir: None,
                env: None,
            },
        )
        .await
        .expect("load program");
        assert!(load_res.contains("Current executable set to"));

        let run_res = handle_run_program(
            &store,
            RunProgramParams {
                session_id: session_id.to_string(),
                stop_at_entry: Some(true),
            },
        )
        .await
        .expect("run program");
        assert!(run_res.pid.is_some());

        let kill_res = handle_kill_process(
            &store,
            KillProcessParams {
                session_id: session_id.to_string(),
            },
        )
        .await
        .expect("kill process");
        assert!(kill_res.contains("Process") || kill_res.contains("killed") || !kill_res.is_empty());
    }
}
