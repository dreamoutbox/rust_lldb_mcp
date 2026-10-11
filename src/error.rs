use thiserror::Error;

/// Domain error types for LLDB MCP server operations.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum LldbMcpError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Process not running in session: {0}")]
    ProcessNotRunning(String),

    #[error("Target executable not loaded in session: {0}")]
    TargetNotLoaded(String),

    #[error("Debugger process I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Debugger command timed out after {0}ms")]
    Timeout(u64),

    #[error("LLDB error: {0}")]
    Lldb(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Breakpoint error: {0}")]
    Breakpoint(String),

    #[error("Thread error: {0}")]
    Thread(String),
}
