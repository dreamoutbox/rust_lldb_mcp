//! Parsing utilities for common LLDB CLI text responses.

/// Parses a breakpoint ID from `breakpoint set` command output.
/// Example input: "Breakpoint 1: where = testprog::main + 4, address = 0x1234"
pub fn parse_breakpoint_id(output: &str) -> Option<u32> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Breakpoint ")
            && let Some(num_str) = rest.split(':').next()
            && let Ok(id) = num_str.trim().parse::<u32>()
        {
            return Some(id);
        }
    }
    None
}

/// Checks if the LLDB output indicates a command error.
pub fn is_error_output(output: &str) -> bool {
    let lower = output.to_lowercase();
    output.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("error:")
            || trimmed.starts_with("error :")
            || trimmed.contains("error: invalid")
    }) || lower.starts_with("error:")
}

/// Extracts error message from an LLDB error output, if present.
pub fn extract_error_message(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(msg) = trimmed.strip_prefix("error:") {
            return Some(msg.trim().to_string());
        }
    }
    if is_error_output(output) {
        Some(output.trim().to_string())
    } else {
        None
    }
}

/// Extracts PID from `Process <PID> launched` or `Process <PID> stopped`.
pub fn parse_process_id(output: &str) -> Option<u32> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Process ") {
            let mut parts = rest.split_whitespace();
            if let Some(pid_str) = parts.next()
                && let Ok(pid) = pid_str.parse::<u32>()
            {
                return Some(pid);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_breakpoint_id() {
        let output = "Breakpoint 3: where = testprog::add + 4 at main.rs:18, address = 0x5555";
        assert_eq!(parse_breakpoint_id(output), Some(3));

        let multiple_locations = "Breakpoint 1: 18 locations.";
        assert_eq!(parse_breakpoint_id(multiple_locations), Some(1));
    }

    #[test]
    fn test_error_detection() {
        assert!(is_error_output("error: 'nonexistent' does not exist"));
        assert!(!is_error_output("Breakpoint 1 set"));
        assert_eq!(
            extract_error_message("error: target not found"),
            Some("target not found".to_string())
        );
    }

    #[test]
    fn test_parse_process_id() {
        let output = "Process 4123 launched: '/path/to/binary' (x86_64)";
        assert_eq!(parse_process_id(output), Some(4123));
    }
}
