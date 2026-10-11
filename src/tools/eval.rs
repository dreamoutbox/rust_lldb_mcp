use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    driver::{extract_error_message, is_error_output},
    error::LldbMcpError,
    session::{SessionStore, parse_session_id},
};

/// Parameters for evaluating an arbitrary expression.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EvaluateExpressionParams {
    pub session_id: String,
    pub expression: String,
    pub thread_id: Option<u64>,
    pub frame_index: Option<u32>,
}

/// Structured outcome of an expression evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EvalResult {
    pub value: Option<String>,
    pub val_type: Option<String>,
    pub error: Option<String>,
    pub raw: String,
}

pub async fn handle_evaluate_expression(
    store: &SessionStore,
    params: EvaluateExpressionParams,
) -> Result<EvalResult, LldbMcpError> {
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
        .send_command(&format!("expr -- {}", params.expression))
        .await?;

    if is_error_output(&output) {
        let err = extract_error_message(&output).unwrap_or_else(|| output.clone());
        return Ok(EvalResult {
            value: None,
            val_type: None,
            error: Some(err),
            raw: output,
        });
    }

    Ok(parse_eval_output(&output))
}

/// Parses the output of `expr -- <expression>`.
/// E.g. `(int) $0 = 13` or `(testprog::Point) $1 = {x:10, y:20}`.
pub fn parse_eval_output(output: &str) -> EvalResult {
    let trimmed = output.trim();

    let (val_type, remainder) = if trimmed.starts_with('(') {
        if let Some(close_paren) = trimmed.find(')') {
            let t = &trimmed[1..close_paren];
            (Some(t.to_string()), trimmed[close_paren + 1..].trim())
        } else {
            (None, trimmed)
        }
    } else {
        (None, trimmed)
    };

    let value = if let Some(eq_idx) = remainder.find('=') {
        let v = remainder[eq_idx + 1..].trim();
        Some(v.to_string())
    } else if remainder.is_empty() {
        None
    } else {
        Some(remainder.to_string())
    };

    EvalResult {
        value,
        val_type,
        error: None,
        raw: trimmed.to_string(),
    }
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

    #[test]
    fn test_parse_eval_output() {
        let raw = "(int) $0 = 13";
        let res = parse_eval_output(raw);
        assert_eq!(res.val_type, Some("int".to_string()));
        assert_eq!(res.value, Some("13".to_string()));
        assert_eq!(res.error, None);

        let raw_struct = "(testprog::Point) $1 = {x:10, y:20}";
        let res_struct = parse_eval_output(raw_struct);
        assert_eq!(res_struct.val_type, Some("testprog::Point".to_string()));
        assert_eq!(res_struct.value, Some("{x:10, y:20}".to_string()));
    }

    #[tokio::test]
    async fn test_eval_in_testprog() {
        let store = SessionStore::new();
        let session_id = create_session(&store, "eval_test".to_string())
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

        // Stop at line 54 where `x`, `y`, `p` exist
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

        // Evaluate arithmetic
        let eval_sum = handle_evaluate_expression(
            &store,
            EvaluateExpressionParams {
                session_id: session_id.to_string(),
                expression: "x + y".to_string(),
                thread_id: None,
                frame_index: None,
            },
        )
        .await
        .expect("eval x + y");
        assert_eq!(eval_sum.value, Some("13".to_string()));
        assert_eq!(eval_sum.error, None);

        // Evaluate field access
        let eval_field = handle_evaluate_expression(
            &store,
            EvaluateExpressionParams {
                session_id: session_id.to_string(),
                expression: "p.x".to_string(),
                thread_id: None,
                frame_index: None,
            },
        )
        .await
        .expect("eval p.x");
        assert_eq!(eval_field.value, Some("10".to_string()));
        assert_eq!(eval_field.error, None);

        // Evaluate invalid expression
        let eval_invalid = handle_evaluate_expression(
            &store,
            EvaluateExpressionParams {
                session_id: session_id.to_string(),
                expression: "non_existent + 1".to_string(),
                thread_id: None,
                frame_index: None,
            },
        )
        .await
        .expect("eval invalid expression");
        assert!(eval_invalid.error.is_some());
    }
}
