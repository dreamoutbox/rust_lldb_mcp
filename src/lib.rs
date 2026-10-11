pub mod backtrace;
pub mod driver;
pub mod error;
pub mod session;
pub mod tools;

use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router,
};

use crate::{
    session::{SessionInfo, SessionStore},
    tools::{
        SessionCreateParams, SessionCreateResult, SessionDeleteParams, handle_session_create,
        handle_session_delete, handle_session_list,
    },
};

/// LLDB MCP Server implementation exposing debug tools.
#[derive(Clone, Default)]
pub struct LldbMcpServer {
    pub store: SessionStore,
}

impl LldbMcpServer {
    pub fn new() -> Self {
        Self {
            store: SessionStore::new(),
        }
    }
}

#[tool_router(server_handler)]
impl LldbMcpServer {
    #[tool(
        name = "session_create",
        description = "Create a new LLDB debug session"
    )]
    pub async fn session_create(
        &self,
        params: Parameters<SessionCreateParams>,
    ) -> Result<Json<SessionCreateResult>, String> {
        handle_session_create(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "session_list",
        description = "List all active LLDB debug sessions"
    )]
    pub async fn session_list(&self) -> Result<Json<Vec<SessionInfo>>, String> {
        let list = handle_session_list(&self.store).await;
        Ok(Json(list))
    }

    #[tool(
        name = "session_delete",
        description = "Delete an LLDB debug session and terminate its process"
    )]
    pub async fn session_delete(
        &self,
        params: Parameters<SessionDeleteParams>,
    ) -> Result<String, String> {
        handle_session_delete(&self.store, params.0)
            .await
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "load_program",
        description = "Load a target executable into an LLDB debug session"
    )]
    pub async fn load_program(
        &self,
        params: Parameters<tools::LoadProgramParams>,
    ) -> Result<String, String> {
        tools::handle_load_program(&self.store, params.0)
            .await
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "run_program",
        description = "Launch the target program in the LLDB session"
    )]
    pub async fn run_program(
        &self,
        params: Parameters<tools::RunProgramParams>,
    ) -> Result<Json<tools::ProcessInfo>, String> {
        tools::handle_run_program(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "kill_process",
        description = "Terminate the running target process in the LLDB session"
    )]
    pub async fn kill_process(
        &self,
        params: Parameters<tools::KillProcessParams>,
    ) -> Result<String, String> {
        tools::handle_kill_process(&self.store, params.0)
            .await
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "set_breakpoint",
        description = "Set a breakpoint by file/line, function name, or address"
    )]
    pub async fn set_breakpoint(
        &self,
        params: Parameters<tools::SetBreakpointParams>,
    ) -> Result<Json<tools::BreakpointInfo>, String> {
        tools::handle_set_breakpoint(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "list_breakpoints",
        description = "List all breakpoints in the session"
    )]
    pub async fn list_breakpoints(
        &self,
        params: Parameters<tools::ListBreakpointsParams>,
    ) -> Result<Json<Vec<tools::BreakpointInfo>>, String> {
        tools::handle_list_breakpoints(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "delete_breakpoint",
        description = "Delete a breakpoint by its numerical ID"
    )]
    pub async fn delete_breakpoint(
        &self,
        params: Parameters<tools::DeleteBreakpointParams>,
    ) -> Result<String, String> {
        tools::handle_delete_breakpoint(&self.store, params.0)
            .await
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "set_breakpoint_condition",
        description = "Set or update an evaluation condition on a breakpoint"
    )]
    pub async fn set_breakpoint_condition(
        &self,
        params: Parameters<tools::SetBreakpointConditionParams>,
    ) -> Result<String, String> {
        tools::handle_set_breakpoint_condition(&self.store, params.0)
            .await
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "continue_process",
        description = "Resume execution of the target process in the LLDB session"
    )]
    pub async fn continue_process(
        &self,
        params: Parameters<tools::ContinueParams>,
    ) -> Result<Json<tools::StopInfo>, String> {
        tools::handle_continue(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(name = "step_next", description = "Step over the current source line")]
    pub async fn step_next(
        &self,
        params: Parameters<tools::StepParams>,
    ) -> Result<Json<tools::StopInfo>, String> {
        tools::handle_step_next(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "step_into",
        description = "Step into the function call on the current source line"
    )]
    pub async fn step_into(
        &self,
        params: Parameters<tools::StepParams>,
    ) -> Result<Json<tools::StopInfo>, String> {
        tools::handle_step_into(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "step_out",
        description = "Step out of the current function to its caller"
    )]
    pub async fn step_out(
        &self,
        params: Parameters<tools::StepParams>,
    ) -> Result<Json<tools::StopInfo>, String> {
        tools::handle_step_out(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "finish_frame",
        description = "Execute until the current stack frame returns"
    )]
    pub async fn finish_frame(
        &self,
        params: Parameters<tools::StepParams>,
    ) -> Result<Json<tools::StopInfo>, String> {
        tools::handle_finish_frame(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "print_variable",
        description = "Print the value of a variable in scope for the current or specified stack frame"
    )]
    pub async fn print_variable(
        &self,
        params: Parameters<tools::PrintVariableParams>,
    ) -> Result<Json<tools::VariableInfo>, String> {
        tools::handle_print_variable(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "print_frame_info",
        description = "Print information about the current or specified stack frame"
    )]
    pub async fn print_frame_info(
        &self,
        params: Parameters<tools::PrintFrameInfoParams>,
    ) -> Result<Json<backtrace::FrameInfo>, String> {
        tools::handle_print_frame_info(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "print_backtrace",
        description = "Print the call backtrace, optionally filtering out stdlib and external runtime frames"
    )]
    pub async fn print_backtrace(
        &self,
        params: Parameters<tools::PrintBacktraceParams>,
    ) -> Result<Json<Vec<backtrace::FrameInfo>>, String> {
        tools::handle_print_backtrace(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "evaluate_expression",
        description = "Evaluate an arbitrary expression in the current or specified stack frame"
    )]
    pub async fn evaluate_expression(
        &self,
        params: Parameters<tools::EvaluateExpressionParams>,
    ) -> Result<Json<tools::EvalResult>, String> {
        tools::handle_evaluate_expression(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "list_threads",
        description = "List all threads currently active in the debugged process"
    )]
    pub async fn list_threads(
        &self,
        params: Parameters<tools::ListThreadsParams>,
    ) -> Result<Json<Vec<tools::ThreadInfo>>, String> {
        tools::handle_list_threads(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "switch_thread",
        description = "Switch current focus to a specified thread by its thread ID"
    )]
    pub async fn switch_thread(
        &self,
        params: Parameters<tools::SwitchThreadParams>,
    ) -> Result<String, String> {
        tools::handle_switch_thread(&self.store, params.0)
            .await
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "run_lldb_command",
        description = "Execute an arbitrary raw LLDB CLI command directly and return its textual output"
    )]
    pub async fn run_lldb_command(
        &self,
        params: Parameters<tools::RunLldbCommandParams>,
    ) -> Result<String, String> {
        tools::handle_run_lldb_command(&self.store, params.0)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let server = LldbMcpServer::new();
        assert!(server.store.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_tool_session_flow() {
        let server = LldbMcpServer::new();

        let create_res = server
            .session_create(Parameters(SessionCreateParams {
                name: "unit_test_session".to_string(),
            }))
            .await
            .expect("session_create failed");
        let session_id = create_res.0.session_id;

        let list_res = server.session_list().await.expect("session_list failed");
        assert_eq!(list_res.0.len(), 1);
        assert_eq!(list_res.0[0].session_id, session_id);

        let delete_res = server
            .session_delete(Parameters(SessionDeleteParams {
                session_id: session_id.clone(),
            }))
            .await
            .expect("session_delete failed");
        assert!(delete_res.contains("deleted successfully"));

        let list_after = server.session_list().await.expect("session_list failed");
        assert!(list_after.0.is_empty());
    }
}
