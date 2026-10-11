use std::path::PathBuf;

use rmcp::{serve_server, transport::io::stdio};
use rust_lldb_mcp::{LldbMcpServer, SessionLogger};

struct CliArgs {
    log_values: bool,
    log_dir: Option<PathBuf>,
}

fn parse_cli_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut log_values = false;
    let mut log_dir = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--log-values" => {
                log_values = true;
            }
            "--log-dir" => {
                if i + 1 < args.len() {
                    i += 1;
                    log_dir = Some(PathBuf::from(&args[i]));
                    log_values = true;
                }
            }
            "-h" | "--help" => {
                eprintln!(
                    "rust_lldb_mcp - LLDB Model Context Protocol server\n\n\
                    USAGE:\n    \
                    rust_lldb_mcp [OPTIONS]\n\n\
                    OPTIONS:\n    \
                    --log-values         Enable logging inspection results to rust_lldb_mcp_logs/<session_id>.log\n    \
                    --log-dir <PATH>     Specify custom directory for session logs (implies --log-values)\n    \
                    -h, --help           Print help information"
                );
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    CliArgs {
        log_values,
        log_dir,
    }
}

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

    let args = parse_cli_args();
    let mut server = LldbMcpServer::new();
    if args.log_values {
        let dir = args
            .log_dir
            .unwrap_or_else(|| PathBuf::from("rust_lldb_mcp_logs"));
        tracing::info!(log_dir = %dir.display(), "Session inspection logging enabled");
        server = server.with_logger(SessionLogger::new(dir));
    }

    let service = serve_server(server, stdio()).await?;
    service.waiting().await?;

    Ok(())
}
