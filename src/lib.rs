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
    #[tool(name = "session_create", description = "Create a new LLDB debug session")]
    pub async fn session_create(
        &self,
        params: Parameters<SessionCreateParams>,
    ) -> Result<Json<SessionCreateResult>, String> {
        handle_session_create(&self.store, params.0)
            .await
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(name = "session_list", description = "List all active LLDB debug sessions")]
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
