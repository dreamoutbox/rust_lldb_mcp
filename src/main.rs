use rmcp::{serve_server, transport::io::stdio};
use rust_lldb_mcp::LldbMcpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting rust_lldb_mcp server on stdio");

    let server = LldbMcpServer;
    let service = serve_server(server, stdio()).await?;
    service.waiting().await?;

    Ok(())
}
