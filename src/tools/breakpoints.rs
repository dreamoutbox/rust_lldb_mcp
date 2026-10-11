use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    driver::{extract_error_message, is_error_output, parse_breakpoint_id, parse_breakpoint_list},
    error::LldbMcpError,
    session::{SessionStore, parse_session_id},
};

/// Target location specification for a breakpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BreakpointLocation {
    /// File path and line number (e.g. src/main.rs:18)
    File { path: String, line: u32 },
    /// Function name symbol (e.g. testprog::add or add)
    Function { name: String },
    /// Memory address (e.g. 0x55555557270c)
    Address { addr: String },
}

/// Parameters for setting a breakpoint.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SetBreakpointParams {
    pub session_id: String,
    pub location: BreakpointLocation,
    pub condition: Option<String>,
}

/// Information about a set breakpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BreakpointInfo {
    pub id: u32,
    pub location: String,
    pub condition: Option<String>,
    pub hit_count: u32,
}

/// Parameters for listing breakpoints.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListBreakpointsParams {
    pub session_id: String,
}

/// Parameters for deleting a breakpoint.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteBreakpointParams {
    pub session_id: String,
    pub breakpoint_id: u32,
}

/// Parameters for adding or updating a condition on an existing breakpoint.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SetBreakpointConditionParams {
    pub session_id: String,
    pub breakpoint_id: u32,
    pub condition: String,
}

pub async fn handle_set_breakpoint(
    store: &SessionStore,
    params: SetBreakpointParams,
) -> Result<BreakpointInfo, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let cmd = match &params.location {
        BreakpointLocation::File { path, line } => format!("breakpoint set -f \"{path}\" -l {line}"),
        BreakpointLocation::Function { name } => format!("breakpoint set -n \"{name}\""),
        BreakpointLocation::Address { addr } => format!("breakpoint set -a \"{addr}\""),
    };

    let output = session.process.send_command(&cmd).await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Breakpoint(err));
    }

    let id = parse_breakpoint_id(&output)
        .ok_or_else(|| LldbMcpError::Breakpoint(format!("Failed to determine breakpoint ID: {output}")))?;

    // If condition was specified, apply it
    if let Some(ref cond) = params.condition {
        let cond_cmd = format!("breakpoint modify -c \"{cond}\" {id}");
        let cond_out = session.process.send_command(&cond_cmd).await?;
        if is_error_output(&cond_out) {
            let err = extract_error_message(&cond_out).unwrap_or(cond_out);
            return Err(LldbMcpError::Breakpoint(err));
        }
    }

    let loc_str = match &params.location {
        BreakpointLocation::File { path, line } => format!("{path}:{line}"),
        BreakpointLocation::Function { name } => name.clone(),
        BreakpointLocation::Address { addr } => addr.clone(),
    };

    Ok(BreakpointInfo {
        id,
        location: loc_str,
        condition: params.condition,
        hit_count: 0,
    })
}

pub async fn handle_list_breakpoints(
    store: &SessionStore,
    params: ListBreakpointsParams,
) -> Result<Vec<BreakpointInfo>, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let output = session.process.send_command("breakpoint list").await?;

    let parsed = parse_breakpoint_list(&output);
    let list = parsed
        .into_iter()
        .map(|b| BreakpointInfo {
            id: b.id,
            location: b.description,
            condition: b.condition,
            hit_count: b.hit_count,
        })
        .collect();

    Ok(list)
}

pub async fn handle_delete_breakpoint(
    store: &SessionStore,
    params: DeleteBreakpointParams,
) -> Result<String, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let output = session
        .process
        .send_command(&format!("breakpoint delete {}", params.breakpoint_id))
        .await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or(output);
        return Err(LldbMcpError::Breakpoint(err));
    }

    Ok(format!("Breakpoint {} deleted successfully", params.breakpoint_id))
}

pub async fn handle_set_breakpoint_condition(
    store: &SessionStore,
    params: SetBreakpointConditionParams,
) -> Result<String, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let output = session
        .process
        .send_command(&format!(
            "breakpoint modify -c \"{}\" {}",
            params.condition, params.breakpoint_id
        ))
        .await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or(output);
        return Err(LldbMcpError::Breakpoint(err));
    }

    Ok(format!(
        "Condition set for breakpoint {}",
        params.breakpoint_id
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::create_session;
    use crate::tools::program::{LoadProgramParams, handle_load_program};

    #[tokio::test]
    async fn test_breakpoint_crud_and_condition() {
        let store = SessionStore::new();
        let session_id = create_session(&store, "bp_test".to_string())
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

        let bp = handle_set_breakpoint(
            &store,
            SetBreakpointParams {
                session_id: session_id.to_string(),
                location: BreakpointLocation::File {
                    path: "src/main.rs".to_string(),
                    line: 18,
                },
                condition: Some("a == 6".to_string()),
            },
        )
        .await
        .expect("set breakpoint");
        assert_eq!(bp.id, 1);
        assert_eq!(bp.condition, Some("a == 6".to_string()));

        let list = handle_list_breakpoints(
            &store,
            ListBreakpointsParams {
                session_id: session_id.to_string(),
            },
        )
        .await
        .expect("list breakpoints");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, 1);
        assert_eq!(list[0].condition, Some("a == 6".to_string()));

        handle_set_breakpoint_condition(
            &store,
            SetBreakpointConditionParams {
                session_id: session_id.to_string(),
                breakpoint_id: 1,
                condition: "a == 10".to_string(),
            },
        )
        .await
        .expect("modify condition");

        let del_res = handle_delete_breakpoint(
            &store,
            DeleteBreakpointParams {
                session_id: session_id.to_string(),
                breakpoint_id: 1,
            },
        )
        .await
        .expect("delete breakpoint");
        assert!(del_res.contains("deleted successfully"));
    }
}
