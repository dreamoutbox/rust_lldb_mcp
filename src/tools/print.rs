use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    backtrace::{BacktraceFilter, FrameInfo, parse_backtrace, parse_frame_line},
    driver::{extract_error_message, is_error_output},
    error::LldbMcpError,
    session::{SessionStore, parse_session_id},
};

/// Parameters for inspecting a variable in scope.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PrintVariableParams {
    pub session_id: String,
    pub name: String,
    pub thread_id: Option<u64>,
    pub frame_index: Option<u32>,
}

/// Structured variable information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct VariableInfo {
    pub name: String,
    pub var_type: Option<String>,
    pub value: String,
    pub raw: String,
}

/// Parameters for inspecting current stack frame information.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PrintFrameInfoParams {
    pub session_id: String,
    pub thread_id: Option<u64>,
    pub frame_index: Option<u32>,
}

/// Parameters for obtaining a backtrace.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PrintBacktraceParams {
    pub session_id: String,
    pub thread_id: Option<u64>,
    /// Whether to strip stdlib and external runtime crates. Defaults to true.
    pub filter_external: Option<bool>,
}

pub async fn handle_print_variable(
    store: &SessionStore,
    params: PrintVariableParams,
) -> Result<VariableInfo, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    if let Some(tid) = params.thread_id {
        let sel_thread = session
            .process
            .send_command(&format!("thread select {tid}"))
            .await?;
        if is_error_output(&sel_thread) {
            let err = extract_error_message(&sel_thread).unwrap_or(sel_thread);
            return Err(LldbMcpError::Thread(err));
        }
    }

    if let Some(fidx) = params.frame_index {
        let sel_frame = session
            .process
            .send_command(&format!("frame select {fidx}"))
            .await?;
        if is_error_output(&sel_frame) {
            let err = extract_error_message(&sel_frame).unwrap_or(sel_frame);
            return Err(LldbMcpError::Frame(err));
        }
    }

    let output = session
        .process
        .send_command(&format!("frame variable {}", params.name))
        .await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Variable(err));
    }

    Ok(parse_variable_output(&output, &params.name))
}

pub async fn handle_print_frame_info(
    store: &SessionStore,
    params: PrintFrameInfoParams,
) -> Result<FrameInfo, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    if let Some(tid) = params.thread_id {
        let sel_thread = session
            .process
            .send_command(&format!("thread select {tid}"))
            .await?;
        if is_error_output(&sel_thread) {
            let err = extract_error_message(&sel_thread).unwrap_or(sel_thread);
            return Err(LldbMcpError::Thread(err));
        }
    }

    if let Some(fidx) = params.frame_index {
        let sel_frame = session
            .process
            .send_command(&format!("frame select {fidx}"))
            .await?;
        if is_error_output(&sel_frame) {
            let err = extract_error_message(&sel_frame).unwrap_or(sel_frame);
            return Err(LldbMcpError::Frame(err));
        }
    }

    let output = session.process.send_command("frame info").await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Frame(err));
    }

    let parsed = parse_frame_line(&output).unwrap_or(FrameInfo {
        index: params.frame_index.unwrap_or(0),
        address: String::new(),
        module: None,
        function: output.clone(),
        file: None,
        line: None,
        column: None,
        raw: output,
    });

    Ok(parsed)
}

pub async fn handle_print_backtrace(
    store: &SessionStore,
    params: PrintBacktraceParams,
) -> Result<Vec<FrameInfo>, LldbMcpError> {
    let session_id = parse_session_id(&params.session_id)?;
    let mut map = store.lock().await;
    let session = map
        .get_mut(&session_id)
        .ok_or_else(|| LldbMcpError::SessionNotFound(params.session_id.clone()))?;

    let cmd = match params.thread_id {
        Some(tid) => format!("thread backtrace {tid}"),
        None => "thread backtrace".to_string(),
    };

    let output = session.process.send_command(&cmd).await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Err(LldbMcpError::Thread(err));
    }

    let frames = parse_backtrace(&output);

    let filter_active = params.filter_external.unwrap_or(true);
    if filter_active {
        let filter = BacktraceFilter::default();
        let filtered = filter.filter(&frames);
        if filtered.is_empty() {
            Ok(frames)
        } else {
            Ok(filtered)
        }
    } else {
        Ok(frames)
    }
}

/// Parses the output of `frame variable <name>`.
/// E.g. `(testprog::Point) p = {x:10, y:20}` or `(int) x = 6`.
pub fn parse_variable_output(output: &str, requested_name: &str) -> VariableInfo {
    let trimmed = output.trim();

    let (var_type, remainder) = if trimmed.starts_with('(') {
        if let Some(close_paren) = trimmed.find(')') {
            let t = &trimmed[1..close_paren];
            (Some(t.to_string()), trimmed[close_paren + 1..].trim())
        } else {
            (None, trimmed)
        }
    } else {
        (None, trimmed)
    };

    let (name, value) = if let Some(eq_idx) = remainder.find('=') {
        let n = remainder[..eq_idx].trim();
        let v = remainder[eq_idx + 1..].trim();
        (n.to_string(), v.to_string())
    } else {
        (requested_name.to_string(), remainder.to_string())
    };

    VariableInfo {
        name,
        var_type,
        value,
        raw: trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::create_session;
    use crate::tools::breakpoints::{BreakpointLocation, SetBreakpointParams, handle_set_breakpoint};
    use crate::tools::program::{LoadProgramParams, RunProgramParams, handle_load_program, handle_run_program};

    #[test]
    fn test_parse_variable_output() {
        let raw = "(testprog::Point) p = {x:10, y:20}";
        let info = parse_variable_output(raw, "p");
        assert_eq!(info.name, "p");
        assert_eq!(info.var_type, Some("testprog::Point".to_string()));
        assert_eq!(info.value, "{x:10, y:20}");

        let raw_simple = "(int) x = 6";
        let info_simple = parse_variable_output(raw_simple, "x");
        assert_eq!(info_simple.name, "x");
        assert_eq!(info_simple.var_type, Some("int".to_string()));
        assert_eq!(info_simple.value, "6");
    }

    #[tokio::test]
    async fn test_print_tools_with_testprog() {
        let store = SessionStore::new();
        let session_id = create_session(&store, "print_test".to_string())
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

        // Break at line 54 (right after `let p = Point { x: 10, y: 20 };`)
        handle_set_breakpoint(
            &store,
            SetBreakpointParams {
                session_id: session_id.to_string(),
                location: BreakpointLocation::File {
                    path: "src/main.rs".to_string(),
                    line: 54,
                },
                condition: None,
            },
        )
        .await
        .expect("set breakpoint line 54");

        handle_run_program(
            &store,
            RunProgramParams {
                session_id: session_id.to_string(),
                stop_at_entry: None,
            },
        )
        .await
        .expect("run program");

        // Test frame info
        let frame_info = handle_print_frame_info(
            &store,
            PrintFrameInfoParams {
                session_id: session_id.to_string(),
                thread_id: None,
                frame_index: None,
            },
        )
        .await
        .expect("frame info");
        assert!(frame_info.raw.contains("main"));

        // Test print variable `p`
        let var_p = handle_print_variable(
            &store,
            PrintVariableParams {
                session_id: session_id.to_string(),
                name: "p".to_string(),
                thread_id: None,
                frame_index: None,
            },
        )
        .await
        .expect("print variable p");
        assert_eq!(var_p.name, "p");
        assert!(var_p.value.contains("10") && var_p.value.contains("20"));

        // Test filtered backtrace: only main should be returned, no stdlib
        let bt = handle_print_backtrace(
            &store,
            PrintBacktraceParams {
                session_id: session_id.to_string(),
                thread_id: None,
                filter_external: Some(true),
            },
        )
        .await
        .expect("filtered backtrace");
        assert!(!bt.is_empty());
        assert!(bt.iter().all(|f| !f.function.contains("std::") && !f.function.contains("core::")));
    }
}
