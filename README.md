# rust_lldb_mcp

A high-performance Model Context Protocol (MCP) server providing structured LLDB debugging tools for Rust applications.

Built using [`rmcp`](https://crates.io/crates/rmcp), `rust_lldb_mcp` drives the native `rust-lldb` command-line debugger directly over stdio. It exposes structured tools for debugging sessions, breakpoint management, stepping, variable inspection, filtered backtraces, expression evaluation, and thread management.

---

## Features

- **Direct `rust-lldb` CLI Driver**: Uses native `rust-lldb` without fragile C/C++ bindings or outdated `lldb-sys` crates.
- **Sentinel-Delimited Command Protocol**: Robust subprocess synchronization using Python delimiter evaluation and biased stderr/stdout streaming.
- **Multi-Session Management**: Isolate multiple concurrent debugging sessions identified by UUIDs.
- **Smart Backtrace Filtering**: Automatically filters out standard library internals (`core::`, `std::`, `/rustc/`) and external async/runtime crates (`tokio`, `hyper`, `tower`, etc.), displaying only your application's code.
- **Expression Evaluation**: Direct evaluation of arbitrary expressions (`expr -- <expr>`) within active stack frames.
- **Escape Hatch**: Raw command execution via `run_lldb_command` for advanced LLDB operations.

---

## Prerequisites

1. **Rust Toolchain**: Rust 1.85+ (Rust 2024 edition support).
2. **`rust-lldb` on PATH**: Included with Rust toolchains installed via `rustup` (`rust-lldb` wrapper around LLDB with Rust type formatters).

Verify installation:
```bash
rust-lldb --version
```

---

## Installation & Building

Clone the repository and build the binary:

```bash
cargo build --release
```

The server executable will be located at `target/release/rust_lldb_mcp`.

---

## MCP Client Configuration

Add `rust_lldb_mcp` to your MCP client configuration (e.g. Claude Desktop, Antigravity IDE, Cursor).

### Example: Claude Desktop (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "rust_lldb": {
      "command": "/path/to/rust_lldb_mcp/target/release/rust_lldb_mcp",
      "args": [
        "--log-values",
        "--log-dir",
        "/path/to/logs"
      ]
    }
  }
}
```

### CLI Feature Flags

- **`--log-values`**: Enables inspection logging for print tools (`print_variable`, `print_frame_info`, `print_backtrace`) and expression evaluation (`evaluate_expression`). Logs are saved to `rust_lldb_mcp_logs/<session_id>.log` with ISO timestamps, tool names, and inspected values/errors.
- **`--log-dir <PATH>`**: Specifies a custom directory for session log files (defaults to `rust_lldb_mcp_logs`). Passing `--log-dir` automatically implies `--log-values`.
- **`-h, --help`**: Displays CLI help and usage information.

---

## Available MCP Tools

### 1. Session Tools
- **`session_create`**: Spawns a new LLDB debugger instance and returns a session ID.
  - Parameters: `{ "name": "session_name" }`
- **`session_list`**: Lists all active debug sessions and their metadata.
  - Parameters: `{}`
- **`session_delete`**: Gracefully terminates an LLDB debugger instance and reaps the process.
  - Parameters: `{ "session_id": "uuid" }`

### 2. Program Tools
- **`load_program`**: Loads a target executable into the session.
  - Parameters: `{ "session_id": "uuid", "executable_path": "path/to/binary", "args": [...], "working_dir": "path", "env": [...] }`
- **`run_program`**: Launches the loaded program in the debugger.
  - Parameters: `{ "session_id": "uuid", "stop_at_entry": false }`
- **`kill_process`**: Kills the running target process.
  - Parameters: `{ "session_id": "uuid" }`

### 3. Breakpoint Tools
- **`set_breakpoint`**: Sets a breakpoint by function name, source file and line, or memory address.
  - Parameters:
    - Function: `{ "session_id": "uuid", "location": { "type": "Function", "name": "foo" } }`
    - File & Line: `{ "session_id": "uuid", "location": { "type": "File", "path": "src/main.rs", "line": 42 } }`
    - Address: `{ "session_id": "uuid", "location": { "type": "Address", "addr": "0x55555557281a" } }`
- **`list_breakpoints`**: Lists all breakpoints, hit counts, conditions, and resolve states.
  - Parameters: `{ "session_id": "uuid" }`
- **`delete_breakpoint`**: Removes a breakpoint by numerical ID.
  - Parameters: `{ "session_id": "uuid", "breakpoint_id": 1 }`
- **`set_breakpoint_condition`**: Modifies or sets an evaluation condition expression on a breakpoint.
  - Parameters: `{ "session_id": "uuid", "breakpoint_id": 1, "condition": "x > 10" }`

### 4. Stepping Tools
- **`continue_process`**: Resumes execution until the next breakpoint or process termination.
  - Parameters: `{ "session_id": "uuid" }`
- **`step_next`**: Steps over the current line (`thread step-over`).
  - Parameters: `{ "session_id": "uuid", "thread_id": 1 }`
- **`step_into`**: Steps into the function call at the current line (`thread step-in`).
  - Parameters: `{ "session_id": "uuid", "thread_id": 1 }`
- **`step_out`**: Steps out of the current function to its caller (`thread step-out`).
  - Parameters: `{ "session_id": "uuid", "thread_id": 1 }`
- **`finish_frame`**: Executes until the current frame returns (`finish`).
  - Parameters: `{ "session_id": "uuid", "thread_id": 1 }`

### 5. Inspection & Print Tools
- **`print_variable`**: Prints the type and formatted value of a variable in the current or specified stack frame.
  - Parameters: `{ "session_id": "uuid", "name": "my_var", "thread_id": 1, "frame_index": 0 }`
- **`print_frame_info`**: Retrieves structured information about a stack frame (index, address, function, source file, line).
  - Parameters: `{ "session_id": "uuid", "thread_id": 1, "frame_index": 0 }`
- **`print_backtrace`**: Returns the call stack, automatically stripping standard library frames and third-party runtime frames.
  - Parameters: `{ "session_id": "uuid", "thread_id": 1, "filter_external": true }`

### 6. Expression Evaluation Tools
- **`evaluate_expression`**: Evaluates an arbitrary expression in the current frame context and captures return values or diagnostics.
  - Parameters: `{ "session_id": "uuid", "expression": "x + y * 2", "thread_id": 1, "frame_index": 0 }`

### 7. Thread Tools & Raw CLI
- **`list_threads`**: Lists all threads in the target process.
  - Parameters: `{ "session_id": "uuid" }`
- **`switch_thread`**: Switches debugger focus to a specified thread.
  - Parameters: `{ "session_id": "uuid", "thread_id": 1 }`
- **`run_lldb_command`**: Direct CLI escape hatch executing raw LLDB commands.
  - Parameters: `{ "session_id": "uuid", "command": "version" }`

---

## Testing

Run unit and integration tests using `cargo nextest`:

```bash
cargo nextest run --test-threads 1 --fail-fast
```

Run linter:

```bash
cargo clippy -- -D warnings
```
