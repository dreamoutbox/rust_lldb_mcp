mod parser;
mod process;

pub use parser::{extract_error_message, is_error_output, parse_breakpoint_id, parse_process_id};
pub use process::LldbProcess;
