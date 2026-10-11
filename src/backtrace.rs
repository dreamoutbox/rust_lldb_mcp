use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents structured information about a single stack frame.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FrameInfo {
    pub index: u32,
    pub address: String,
    pub module: Option<String>,
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub raw: String,
}

/// Parses a single LLDB frame line into `FrameInfo`.
/// Handles standard LLDB formats, e.g.:
/// `frame #0: 0x000055555557281a testprog`testprog::main at main.rs:54:5`
/// `frame #15: 0x0000555555572cee testprog`main + 30`
pub fn parse_frame_line(line: &str) -> Option<FrameInfo> {
    let trimmed = line.trim();

    // LLDB frame lines contain "frame #<idx>:"
    let frame_marker_idx = trimmed.find("frame #")?;
    let after_marker = &trimmed[frame_marker_idx + "frame #".len()..];

    let colon_idx = after_marker.find(':')?;
    let idx_str = after_marker[..colon_idx].trim();
    let index: u32 = idx_str.parse().ok()?;

    let remainder = after_marker[colon_idx + 1..].trim();

    // Find hex address: 0x...
    let (address, after_addr) = if remainder.starts_with("0x") {
        let space_idx = remainder.find(' ').unwrap_or(remainder.len());
        (
            remainder[..space_idx].to_string(),
            remainder[space_idx..].trim(),
        )
    } else {
        (String::new(), remainder)
    };

    // Check for " at <file>:<line>[:<col>]"
    let (func_and_module, file, line_num, col_num) = if let Some(at_idx) = after_addr.rfind(" at ")
    {
        let func_part = after_addr[..at_idx].trim();
        let loc_part = after_addr[at_idx + " at ".len()..].trim();

        let parts: Vec<&str> = loc_part.split(':').collect();
        let f = parts.first().map(|s| s.to_string());
        let l = parts.get(1).and_then(|s| s.parse::<u32>().ok());
        let c = parts.get(2).and_then(|s| s.parse::<u32>().ok());

        (func_part, f, l, c)
    } else {
        (after_addr, None, None, None)
    };

    // Separate module and function by LLDB backtick delimiter (e.g. `module`function`)
    let (module, function) = if let Some(bt_idx) = func_and_module.find('`') {
        (
            Some(func_and_module[..bt_idx].trim().to_string()),
            func_and_module[bt_idx + 1..].trim().to_string(),
        )
    } else {
        (None, func_and_module.to_string())
    };

    Some(FrameInfo {
        index,
        address,
        module,
        function,
        file,
        line: line_num,
        column: col_num,
        raw: trimmed.to_string(),
    })
}

/// Parses all frame lines from LLDB `thread backtrace` output.
pub fn parse_backtrace(output: &str) -> Vec<FrameInfo> {
    output.lines().filter_map(parse_frame_line).collect()
}

/// Filter for stripping standard library and external crate frames from backtraces.
#[derive(Debug, Clone)]
pub struct BacktraceFilter {
    pub denylist_crates: Vec<String>,
}

impl Default for BacktraceFilter {
    fn default() -> Self {
        Self {
            denylist_crates: vec![
                "tokio".to_string(),
                "tokio_util".to_string(),
                "hyper".to_string(),
                "tower".to_string(),
                "futures".to_string(),
                "async_std".to_string(),
                "smol".to_string(),
                "mio".to_string(),
                "tracing".to_string(),
                "serde".to_string(),
                "axum".to_string(),
                "actix".to_string(),
                "actix_rt".to_string(),
            ],
        }
    }
}

impl BacktraceFilter {
    /// Determines whether a frame originates from user application code.
    pub fn is_user_frame(&self, frame: &FrameInfo) -> bool {
        // Drop frames with source files in stdlib or external registry crates
        if let Some(file) = &frame.file
            && (file.contains("/rustc/")
                || file.contains(".rustup/toolchains/")
                || file.contains("/.cargo/registry/")
                || file.contains("/.cargo/git/"))
        {
            return false;
        }

        // Drop shared libraries / system runtime modules
        if let Some(module) = &frame.module
            && (module.contains("libc")
                || module.contains("ld-linux")
                || module.contains("libpthread"))
        {
            return false;
        }

        let func = &frame.function;

        // Drop standard library runtime and language initialization frames
        if func.contains("std::")
            || func.contains("core::")
            || func.contains("alloc::")
            || func.starts_with("__rust_")
            || func.starts_with("__libc_")
            || func.starts_with("_start")
            || func.contains("___lldb_unnamed_symbol")
        {
            return false;
        }

        // Drop CRT runtime entry points that lack source file mapping
        if frame.file.is_none()
            && (func.starts_with("main +")
                || func == "main"
                || func.starts_with("_start")
                || func.starts_with("__libc_start"))
        {
            return false;
        }

        // Drop frames from known external runtime / async / network crates
        for cr in &self.denylist_crates {
            let crate_ns = format!("{cr}::");
            if func.contains(&crate_ns)
                || frame.raw.contains(&crate_ns)
                || frame.module.as_deref() == Some(cr.as_str())
            {
                return false;
            }
            if let Some(file) = &frame.file
                && (file.contains(&format!("/{cr}/")) || file.contains(&format!("/{cr}-")))
            {
                return false;
            }
        }

        true
    }

    /// Filters a collection of frames, keeping only user frames.
    pub fn filter(&self, frames: &[FrameInfo]) -> Vec<FrameInfo> {
        frames
            .iter()
            .filter(|f| self.is_user_frame(f))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frame_line() {
        let line = "  * frame #0: 0x0000555555572bbe testprog`testprog::factorial(n=6) at main.rs:24:8";
        let frame = parse_frame_line(line).expect("parse frame");
        assert_eq!(frame.index, 0);
        assert_eq!(frame.address, "0x0000555555572bbe");
        assert_eq!(frame.module, Some("testprog".to_string()));
        assert_eq!(frame.file, Some("main.rs".to_string()));
        assert_eq!(frame.line, Some(24));
        assert_eq!(frame.column, Some(8));
        assert_eq!(frame.function, "testprog::factorial(n=6)");
    }

    #[test]
    fn test_parse_frame_line_no_loc() {
        let line = "    frame #15: 0x0000555555572cee testprog`main + 30";
        let frame = parse_frame_line(line).expect("parse frame");
        assert_eq!(frame.index, 15);
        assert_eq!(frame.address, "0x0000555555572cee");
        assert_eq!(frame.module, Some("testprog".to_string()));
        assert_eq!(frame.file, None);
        assert_eq!(frame.line, None);
        assert_eq!(frame.function, "main + 30");
    }

    #[test]
    fn test_filter_backtrace() {
        let raw = r#"
* thread #1, name = 'testprog', stop reason = breakpoint 1.1
  * frame #0: 0x0000555555572bbe testprog`testprog::factorial(n=6) at main.rs:24:8
    frame #1: 0x00005555555728ff testprog`testprog::main at main.rs:61:13
    frame #2: 0x000055555556f9ab testprog`<fn() as core::ops::function::FnOnce<()>>::call_once((null)={}, (null)=<unavailable>) at function.rs:250:5
    frame #3: 0x000055555556dfde testprog`std::sys::backtrace::__rust_begin_short_backtrace::<fn(), ()>(f={}) at backtrace.rs:166:18
    frame #4: 0x0000555555570801 testprog`std::rt::lang_start::<()>::{closure#0} at rt.rs:206:18
    frame #5: 0x00005555555995c9 testprog`tokio::runtime::task::core::Header::run at /home/user/.cargo/registry/src/tokio-1.0/src/task.rs:10:5
    frame #15: 0x0000555555572cee testprog`main + 30
    frame #16: 0x00007ffff7c2a1ca libc.so.6`___lldb_unnamed_symbol3278 + 122
    frame #17: 0x00007ffff7c2a28b libc.so.6`__libc_start_main + 139
    frame #18: 0x000055555556df05 testprog`_start + 37
"#;

        let frames = parse_backtrace(raw);
        assert_eq!(frames.len(), 10);

        let filter = BacktraceFilter::default();
        let user_frames = filter.filter(&frames);

        assert_eq!(user_frames.len(), 2);
        assert_eq!(user_frames[0].index, 0);
        assert_eq!(user_frames[1].index, 1);
        assert!(user_frames[0].function.contains("factorial"));
        assert!(user_frames[1].function.contains("main"));
    }
}
