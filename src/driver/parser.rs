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

/// Parsed breakpoint entry from LLDB output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBreakpoint {
    pub id: u32,
    pub description: String,
    pub condition: Option<String>,
    pub hit_count: u32,
}

/// Parses the output of `breakpoint list` into a list of breakpoints.
pub fn parse_breakpoint_list(output: &str) -> Vec<ParsedBreakpoint> {
    let mut breakpoints = Vec::new();
    let mut current: Option<ParsedBreakpoint> = None;

    for line in output.lines() {
        // Top-level breakpoint lines start without indentation: "<id>: ..."
        if !line.starts_with(' ')
            && !line.starts_with('\t')
            && line.contains(':')
            && let Some(first_token) = line.split(':').next()
            && let Ok(id) = first_token.trim().parse::<u32>()
        {
            if let Some(b) = current.take() {
                breakpoints.push(b);
            }

            let desc = line.split(':').skip(1).collect::<Vec<_>>().join(":").trim().to_string();
            current = Some(ParsedBreakpoint {
                id,
                description: desc,
                condition: None,
                hit_count: 0,
            });
            continue;
        }

        // Check for condition under the current breakpoint
        if let Some(ref mut b) = current {
            let trimmed = line.trim();
            if let Some(cond) = trimmed.strip_prefix("Condition: ") {
                b.condition = Some(cond.trim().to_string());
            } else if let Some(idx) = trimmed.find("hit count = ") {
                let rest = &trimmed[idx + "hit count = ".len()..];
                if let Some(count_str) = rest.split_whitespace().next()
                    && let Ok(count) = count_str.trim().parse::<u32>()
                {
                    b.hit_count = count;
                }
            }
        }
    }

    if let Some(b) = current {
        breakpoints.push(b);
    }

    breakpoints
}

/// Information parsed from a process stop event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedStopInfo {
    pub stop_reason: Option<String>,
    pub thread_id: Option<u64>,
    pub frame: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// Parses stop reason, thread ID, frame, file, and line from LLDB stop output.
pub fn parse_stop_info(output: &str) -> ParsedStopInfo {
    let mut info = ParsedStopInfo::default();

    for line in output.lines() {
        let trimmed = line.trim();

        // Extract stop reason
        if let Some(idx) = trimmed.find("stop reason = ") {
            let reason = trimmed[idx + "stop reason = ".len()..].trim();
            info.stop_reason = Some(reason.to_string());
        }

        // Extract thread ID (e.g. "* thread #1" or "thread #2")
        if let Some(idx) = trimmed.find("thread #") {
            let rest = &trimmed[idx + "thread #".len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(tid) = digits.parse::<u64>() {
                info.thread_id = Some(tid);
            }
        }

        // Extract frame #0 information
        if trimmed.starts_with("frame #0:") {
            info.frame = Some(trimmed.to_string());
            if let Some(at_idx) = trimmed.rfind(" at ") {
                let loc = &trimmed[at_idx + " at ".len()..];
                let mut parts = loc.split(':');
                if let Some(f) = parts.next() {
                    info.file = Some(f.to_string());
                }
                if let Some(l_str) = parts.next()
                    && let Ok(l) = l_str.parse::<u32>()
                {
                    info.line = Some(l);
                }
            }
        }
    }

    info
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

    #[test]
    fn test_parse_breakpoint_list() {
        let output = "Current breakpoints:\n1: file = 'src/main.rs', line = 18, exact_match = 0, locations = 1\nCondition: a == 6\n  1.1: where = testprog`testprog::add + 12 at main.rs:19:5, address = testprog[0x000000000001e70c], unresolved, hit count = 2\n";
        let parsed = parse_breakpoint_list(output);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, 1);
        assert_eq!(parsed[0].condition, Some("a == 6".to_string()));
        assert_eq!(parsed[0].hit_count, 2);
    }

    #[test]
    fn test_parse_stop_info() {
        let output = "Process 53944 stopped\n* thread #1, name = 'testprog', stop reason = breakpoint 1.1\n    frame #0: 0x000055555557270c testprog`testprog::add(a=6, b=7) at main.rs:19:5\n-> 19      a + b\n";
        let parsed = parse_stop_info(output);
        assert_eq!(parsed.stop_reason, Some("breakpoint 1.1".to_string()));
        assert_eq!(parsed.thread_id, Some(1));
        assert_eq!(parsed.file, Some("main.rs".to_string()));
        assert_eq!(parsed.line, Some(19));
    }
}
