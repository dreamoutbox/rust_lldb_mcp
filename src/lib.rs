pub mod driver;
pub mod error;

use rmcp::{
    ServerHandler,
    model::{ServerCapabilities, ServerConfig},
};

#[derive(Debug, Clone, Default)]
pub struct LldbMcpServer;

impl ServerHandler for LldbMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info() {
        let server = LldbMcpServer;
        let info = server.get_info();
        assert!(info.capabilities.tools.is_some());
    }
}
