use rmcp::handler::server::wrapper::Parameters;
use rust_lldb_mcp::{
    LldbMcpServer,
    tools::{
        BreakpointLocation, DeleteBreakpointParams, EvaluateExpressionParams, ListThreadsParams,
        LoadProgramParams, PrintBacktraceParams, PrintFrameInfoParams, PrintVariableParams,
        RunLldbCommandParams, RunProgramParams, SessionCreateParams, SessionDeleteParams,
        SetBreakpointParams, StepParams, SwitchThreadParams,
    },
};

#[tokio::test]
async fn test_full_debug_workflow_end_to_end() {
    let server = LldbMcpServer::new();

    // 1. Create a debug session
    let create_res = server
        .session_create(Parameters(SessionCreateParams {
            name: "integration_workflow".to_string(),
        }))
        .await
        .expect("session_create");
    let session_id = create_res.0.session_id;

    // 2. Load testprog executable
    let load_res = server
        .load_program(Parameters(LoadProgramParams {
            session_id: session_id.clone(),
            executable_path: "testprog/target/debug/testprog".to_string(),
            args: None,
            working_dir: None,
            env: None,
        }))
        .await
        .expect("load_program");
    assert!(load_res.contains("Current executable set to"));

    // 3. Set a breakpoint on `factorial`
    let bp_res = server
        .set_breakpoint(Parameters(SetBreakpointParams {
            session_id: session_id.clone(),
            location: BreakpointLocation::Function {
                name: "factorial".to_string(),
            },
            condition: None,
        }))
        .await
        .expect("set_breakpoint");
    assert_eq!(bp_res.0.id, 1);

    // 4. Run the program
    let run_res = server
        .run_program(Parameters(RunProgramParams {
            session_id: session_id.clone(),
            stop_at_entry: None,
        }))
        .await
        .expect("run_program");
    assert!(run_res.0.pid.is_some());
    assert!(run_res.0.output.contains("stopped") || run_res.0.output.contains("breakpoint"));

    // 5. Inspect current frame
    let frame = server
        .print_frame_info(Parameters(PrintFrameInfoParams {
            session_id: session_id.clone(),
            thread_id: None,
            frame_index: None,
        }))
        .await
        .expect("print_frame_info");
    assert!(frame.0.function.contains("factorial"));

    // 6. Inspect variable `n`
    let var_n = server
        .print_variable(Parameters(PrintVariableParams {
            session_id: session_id.clone(),
            name: "n".to_string(),
            thread_id: None,
            frame_index: None,
        }))
        .await
        .expect("print_variable");
    assert_eq!(var_n.0.name, "n");
    assert_eq!(var_n.0.value, "6");

    // 7. Evaluate expression `n * 2`
    let eval_res = server
        .evaluate_expression(Parameters(EvaluateExpressionParams {
            session_id: session_id.clone(),
            expression: "n * 2".to_string(),
            thread_id: None,
            frame_index: None,
        }))
        .await
        .expect("evaluate_expression");
    assert_eq!(eval_res.0.value, Some("12".to_string()));
    assert_eq!(eval_res.0.error, None);

    // 8. Filtered backtrace should contain user frames and no stdlib frames
    let bt = server
        .print_backtrace(Parameters(PrintBacktraceParams {
            session_id: session_id.clone(),
            thread_id: None,
            filter_external: Some(true),
        }))
        .await
        .expect("print_backtrace");
    assert!(!bt.0.is_empty());
    assert!(
        bt.0.iter()
            .all(|f| !f.function.contains("std::") && !f.function.contains("core::"))
    );

    // 9. Step next inside factorial
    let step_res = server
        .step_next(Parameters(StepParams {
            session_id: session_id.clone(),
            thread_id: None,
        }))
        .await
        .expect("step_next");
    assert!(step_res.0.frame.is_some() || !step_res.0.raw_output.is_empty());

    // 10. List active threads
    let threads = server
        .list_threads(Parameters(ListThreadsParams {
            session_id: session_id.clone(),
        }))
        .await
        .expect("list_threads");
    assert!(!threads.0.is_empty());

    // 11. Switch thread to thread 1
    let switch_res = server
        .switch_thread(Parameters(SwitchThreadParams {
            session_id: session_id.clone(),
            thread_id: 1,
        }))
        .await
        .expect("switch_thread");
    assert!(!switch_res.is_empty());

    // 12. Run raw LLDB command escape hatch
    let raw_out = server
        .run_lldb_command(Parameters(RunLldbCommandParams {
            session_id: session_id.clone(),
            command: "version".to_string(),
        }))
        .await
        .expect("run_lldb_command");
    assert!(raw_out.contains("lldb version"));

    // 13. Delete breakpoint
    let del_bp = server
        .delete_breakpoint(Parameters(DeleteBreakpointParams {
            session_id: session_id.clone(),
            breakpoint_id: 1,
        }))
        .await
        .expect("delete_breakpoint");
    assert!(del_bp.contains("deleted successfully"));

    // 14. Delete debug session
    let del_session = server
        .session_delete(Parameters(SessionDeleteParams {
            session_id: session_id.clone(),
        }))
        .await
        .expect("session_delete");
    assert!(del_session.contains("deleted successfully"));

    // 15. Verify deleting again fails gracefully
    let del_err = server
        .session_delete(Parameters(SessionDeleteParams {
            session_id: session_id.clone(),
        }))
        .await;
    assert!(del_err.is_err());
}

#[tokio::test]
async fn test_logging_enabled_writes_session_log() {
    use std::path::PathBuf;
    use tokio::fs::{read_to_string, remove_dir_all};
    use rust_lldb_mcp::SessionLogger;

    let log_dir = PathBuf::from("target/test_session_logs");
    let _ = remove_dir_all(&log_dir).await;

    let server = LldbMcpServer::new().with_logger(SessionLogger::new(&log_dir));

    let create_res = server
        .session_create(Parameters(SessionCreateParams {
            name: "log_test_session".to_string(),
        }))
        .await
        .expect("session_create");
    let session_id = create_res.0.session_id;

    server
        .load_program(Parameters(LoadProgramParams {
            session_id: session_id.clone(),
            executable_path: "testprog/target/debug/testprog".to_string(),
            args: None,
            working_dir: None,
            env: None,
        }))
        .await
        .expect("load_program");

    server
        .set_breakpoint(Parameters(SetBreakpointParams {
            session_id: session_id.clone(),
            location: BreakpointLocation::Function {
                name: "add".to_string(),
            },
            condition: None,
        }))
        .await
        .expect("set_breakpoint");

    server
        .run_program(Parameters(RunProgramParams {
            session_id: session_id.clone(),
            stop_at_entry: None,
        }))
        .await
        .expect("run_program");

    // Phase 7 print tools
    server
        .print_variable(Parameters(PrintVariableParams {
            session_id: session_id.clone(),
            name: "a".to_string(),
            thread_id: None,
            frame_index: None,
        }))
        .await
        .expect("print_variable");

    server
        .print_frame_info(Parameters(PrintFrameInfoParams {
            session_id: session_id.clone(),
            thread_id: None,
            frame_index: None,
        }))
        .await
        .expect("print_frame_info");

    server
        .print_backtrace(Parameters(PrintBacktraceParams {
            session_id: session_id.clone(),
            thread_id: None,
            filter_external: Some(true),
        }))
        .await
        .expect("print_backtrace");

    // Phase 8 eval tool
    server
        .evaluate_expression(Parameters(EvaluateExpressionParams {
            session_id: session_id.clone(),
            expression: "a + b".to_string(),
            thread_id: None,
            frame_index: None,
        }))
        .await
        .expect("evaluate_expression");

    // Assert log file exists and contains all tool logs
    let log_file = log_dir.join(format!("{session_id}.log"));
    let log_contents = read_to_string(&log_file).await.expect("read session log file");

    assert!(log_contents.contains("[print_variable] name: a | type: int | value: 6"));
    assert!(log_contents.contains("[print_frame_info] frame #0:"));
    assert!(log_contents.contains("[print_backtrace]"));
    assert!(log_contents.contains("[evaluate_expression] expr: \"a + b\" | type: int | value: 13"));

    server
        .session_delete(Parameters(SessionDeleteParams {
            session_id: session_id.clone(),
        }))
        .await
        .expect("session_delete");

    let _ = remove_dir_all(&log_dir).await;
}
