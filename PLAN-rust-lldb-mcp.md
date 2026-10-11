# PLAN: rust-lldb-mcp [STATUS: TODO]

Rust MCP server that exposes LLDB debugging capabilities as structured MCP tools.
Uses `rmcp` for the MCP transport layer and drives `rust-lldb` CLI directly as a long-lived
subprocess via `tokio::process`, communicating via interactive stdin/stdout with a custom
prompt sentinel.

---

## Architecture Overview

```
MCP Client (LLM host)
      │  stdio / SSE
      ▼
  rmcp server
      │
  Tool dispatcher (per-session routing)
      │
  SessionStore  ─── HashMap<SessionId, DebugSession>
      │
  DebugSession  ─── tokio::process::Child (rust-lldb --no-use-colors --source-quietly)
                    stdin  ──► send LLDB CLI commands
                    stdout ──► read output until prompt sentinel
```

- One `rust-lldb` child process per session, started with `--no-use-colors --source-quietly`.
- Initialized with `settings set prompt "__LLDB_PROMPT__ "` and `settings set auto-confirm true`.
- Sessions are identified by a UUID; state lives in `Arc<Mutex<SessionStore>>`.
- The MCP server runs on stdio (default) or SSE (opt-in via `--transport sse`).
- All logging goes to `stderr` via `tracing`; `stdout` is reserved for the MCP protocol.

---

## LLDB CLI Command Mapping

| Group | LLDB CLI command |
|-------|------------------|
| Program | `target create <path>`, `process launch`, `process kill`, `quit` |
| Breakpoints | `breakpoint set -f <file> -l <line>`, `breakpoint set -n <func>`, `breakpoint list`, `breakpoint delete <id>`, `breakpoint modify -c <cond> <id>` |
| Stepping | `process continue`, `thread step-over`, `thread step-in`, `thread step-out`, `thread return` |
| Print | `frame variable <name>`, `thread backtrace`, `frame info` |
| Eval | `expr -- <expression>` |
| Threads | `thread list`, `thread select <id>` |
| Raw | pass-through any LLDB CLI command string |

---

## Module Layout (target)

```
src/
  lib.rs            -- library exports + MCP server setup
  main.rs           -- binary CLI entry point
  error.rs          -- thiserror domain error types
  session/
    mod.rs          -- re-exports
    store.rs        -- SessionStore, SessionId, DebugSession struct
    manager.rs      -- create/list/delete session logic
  driver/
    mod.rs          -- re-exports
    process.rs      -- spawn rust-lldb child, send_command, read until prompt
    parser.rs       -- parse outputs for backtrace, variables, threads, breakpoints
  tools/
    mod.rs          -- re-exports + tool registration
    session.rs      -- session_create / session_list / session_delete
    program.rs      -- load_program / run_program / kill_process
    breakpoints.rs  -- set/list/delete breakpoint, conditional breakpoint
    stepping.rs     -- continue / step_next / step_into / step_out / finish
    print.rs        -- print_variable / print_backtrace / print_frame_info
    eval.rs         -- evaluate_expression
    threads.rs      -- list_threads / switch_thread / run_lldb_command
  backtrace.rs      -- user-code backtrace filtering logic
```

---

### Phase 1: Project Scaffold [STATUS: DONE]

- **Goal**: Compiling binary with rmcp wired up on stdio; no tools yet.
- **Tasks**:
  - [x] `cargo new --bin rust_lldb_mcp` (Rust 2024 edition in `Cargo.toml`)
  - [x] Add dependencies: `rmcp` (features: `server`, `macros`, `transport-io`), `tokio` (features: `full`), `tracing`, `tracing-subscriber`, `serde`, `serde_json`, `schemars`, `thiserror`, `anyhow`, `uuid` (v4)
  - [x] No `lldb` / `lldb-sys` crates; only `rust-lldb` as an external binary on PATH
  - [x] Create `src/error.rs` with `LldbMcpError` (`thiserror`)
  - [x] Stub `main.rs`: initialise `tracing_subscriber`, parse `--transport` flag (stdio/sse), start empty rmcp server
- **Done when**: `cargo build` is green; `cargo nextest run --test-threads 1 --fail-fast` passes.

---

### Phase 2: LLDB CLI Driver [STATUS: DONE]

- **Goal**: Reliable async read/write layer over a `rust-lldb` CLI child process.
- **Tasks**:
  - [x] `src/driver/process.rs`:
    - `LldbProcess`: wraps `tokio::process::Child`, `BufReader<ChildStdout>`, `ChildStdin`
    - Spawn: `Command::new("rust-lldb").args(["--no-use-colors", "--source-quietly"]).stdin(Piped).stdout(Piped).stderr(Piped)`
    - Initialize: send `settings set auto-confirm true`
    - `send_command(&mut self, cmd: &str) -> Result<String, LldbMcpError>`: writes command line, reads output until `DONE_SENTINEL` marker
    - Timeout protection: abort after configurable duration if child stalls
  - [x] `src/driver/parser.rs`: helper functions for common LLDB output parsing
  - [x] Unit tests: spawn `rust-lldb`, send `version`, verify version output captured cleanly
- **Done when**: driver can launch `rust-lldb`, send commands, receive cleaned output text; tests pass.

---

### Phase 3: Session Management [STATUS: DONE]

- **Goal**: LLM can create, list, and delete LLDB debug sessions.
- **Tasks**:
  - [x] `src/session/store.rs`: define `SessionId` (newtype over `Uuid`), `DebugSession` (holds `LldbProcess`, session name, creation timestamp, loaded target), `SessionStore` (`HashMap<SessionId, DebugSession>` behind `Arc<Mutex<...>>`)
  - [x] `src/session/manager.rs`: `create_session` (spawns LLDB child), `list_sessions`, `delete_session` (sends `quit` to child, awaits exit)
  - [x] `src/tools/session.rs`: expose three MCP tools:
    - `session_create(name: String) -> SessionId`
    - `session_list() -> Vec<SessionInfo>`
    - `session_delete(session_id: SessionId) -> ()`
  - [x] Register tools in `src/tools/mod.rs`
  - [x] Unit tests for `SessionStore` CRUD
- **Done when**: `session_create` returns a valid UUID; `session_delete` sends `quit` and reaps the child; tests pass.

---

### Phase 4: Program Tools [STATUS: DONE]

- **Goal**: Load an executable into a session, launch it, and kill it.
- **Tasks**:
  - [x] `src/tools/program.rs`:
    - `load_program(session_id, executable_path: String, args: Vec<String>, env: Vec<String>, working_dir: Option<String>) -> String` — runs `target create <path>`
    - `run_program(session_id, stop_at_entry: bool) -> ProcessInfo` — runs `process launch` (or `process launch --stop-at-entry`)
    - `kill_process(session_id) -> ()` — runs `process kill`
  - [x] Error if `run_program` called before `load_program`, or `kill_process` when no process is live
  - [x] Integration smoke test with `testprog`: load, run, assert output
- **Done when**: three tools work end-to-end; tests pass.

---

### Phase 5: Breakpoint Tools [STATUS: TODO]

- **Goal**: Set, list, delete breakpoints; support conditions.
- **Tasks**:
  - [ ] `src/tools/breakpoints.rs`:
    - `set_breakpoint(session_id, location: BreakpointLocation) -> BreakpointInfo`
      - `BreakpointLocation` enum: `File { path: String, line: u32 }` | `Function { name: String }` | `Address { addr: String }`
      - Maps to: `breakpoint set -f <file> -l <line>` | `breakpoint set -n <name>` | `breakpoint set -a <addr>`
    - `list_breakpoints(session_id) -> Vec<BreakpointInfo>` — runs `breakpoint list` and parses table
    - `delete_breakpoint(session_id, breakpoint_id: u32) -> ()` — runs `breakpoint delete <id>`
    - `set_breakpoint_condition(session_id, breakpoint_id: u32, condition: String) -> ()` — runs `breakpoint modify -c "<condition>" <id>`
  - [ ] `BreakpointInfo`: id, location, hit count, enabled, condition
  - [ ] Tests use `testprog` binary
- **Done when**: breakpoints survive a launch/stop cycle; condition respected; tests pass.

---

### Phase 6: Stepping Tools [STATUS: TODO]

- **Goal**: Full stepping control after a breakpoint hit.
- **Tasks**:
  - [ ] `src/tools/stepping.rs`:
    - `continue_process(session_id) -> StopInfo` — `process continue`
    - `step_next(session_id, thread_id: Option<u64>) -> StopInfo` — `thread step-over`
    - `step_into(session_id, thread_id: Option<u64>) -> StopInfo` — `thread step-in`
    - `step_out(session_id, thread_id: Option<u64>) -> StopInfo` — `thread step-out`
    - `finish_frame(session_id, thread_id: Option<u64>) -> FinishResult` — `thread return` / finish
  - [ ] `StopInfo` captures stop reason, thread id, current source file and line
- **Done when**: step through a loop in `testprog`; assert line advances correctly; tests pass.

---

### Phase 7: Print Tools [STATUS: TODO]

- **Goal**: Inspect variables, frames, and filtered backtraces.
- **Tasks**:
  - [ ] `src/tools/print.rs`:
    - `print_variable(session_id, name: String, thread_id: Option<u64>, frame_index: Option<u32>) -> VariableInfo` — runs `frame variable <name>`
    - `print_frame_info(session_id, thread_id: Option<u64>, frame_index: Option<u32>) -> FrameInfo` — runs `frame info`
    - `print_backtrace(session_id, thread_id: Option<u64>) -> Vec<FrameInfo>` — runs `thread backtrace`
  - [ ] `src/backtrace.rs`: user-code frame filter
    - Drop frames where file path contains:
      - `.rustup/toolchains`, `/rustc/` (stdlib)
      - Configurable denylist (default: `tokio`, `tokio_util`, `hyper`, `tower`, `futures`, `async_std`, `smol`, `mio`, `tracing`)
    - Failsafe: if all frames are filtered, return the full list
  - [ ] `VariableInfo`: name, type, value string
- **Done when**: `print_backtrace` on `testprog` returns only user frames; tests pass.

---

### Phase 8: Expression Evaluation [STATUS: TODO]

- **Goal**: Evaluate arbitrary expressions in the current frame.
- **Tasks**:
  - [ ] `src/tools/eval.rs`:
    - `evaluate_expression(session_id, expression: String, thread_id: Option<u64>, frame_index: Option<u32>) -> EvalResult`
    - Runs `expr -- <expression>`
    - Captures output value and error if evaluation fails
  - [ ] `EvalResult`: `{ value: String, error: Option<String> }`
  - [ ] Unit test: stop at breakpoint, evaluate an arithmetic expression and local variable
- **Done when**: errors surfaced in `EvalResult`, not panics; tests pass.

---

### Phase 9: Thread Tools + Raw Command [STATUS: TODO]

- **Goal**: Thread enumeration, thread switching, and raw LLDB command escape hatch.
- **Tasks**:
  - [ ] `src/tools/threads.rs`:
    - `list_threads(session_id) -> Vec<ThreadInfo>` — runs `thread list`
    - `switch_thread(session_id, thread_id: u64) -> ()` — runs `thread select <id>`
    - `run_lldb_command(session_id, command: String) -> String` — sends raw command string as-is to child stdin, captures raw output
  - [ ] `run_lldb_command` output is unstructured text
- **Done when**: `list_threads` returns threads from `testprog`; `run_lldb_command("version")` returns LLDB version; tests pass.

---

### Phase 10: Integration & Polish [STATUS: TODO]

- **Goal**: End-to-end smoke test, README, clippy clean.
- **Tasks**:
  - [ ] `tests/integration_test.rs`: spawn the MCP server binary over stdio, send JSON-RPC `tools/list`, assert all tools present; run mini session (create -> load -> run -> breakpoint -> backtrace -> delete)
  - [ ] `README.md`: prerequisites (`rust-lldb` on PATH), build + run instructions, example MCP client config, list of all tools
  - [ ] Consistent error propagation: non-existent `session_id` -> structured MCP error, not panic
  - [ ] Audit `unwrap`/`expect` outside tests; replace with `?` or add justification comments
  - [ ] `cargo clippy -- -D warnings` clean
  - [ ] `cargo nextest run --test-threads 1 --fail-fast` full suite green
- **Done when**: integration test passes; clippy clean; README covers all tools.

---

## Unknowns, Risks, Assumptions

| # | Item | Risk | Mitigation |
|---|------|------|-----------|
| 1 | Prompt detection | Long command outputs or prompts embedded in text | Unique custom prompt sentinel `__LLDB_PROMPT__ `; verify delimiter handling |
| 2 | Backtrace filter completeness | User's codebase may use crates not in default denylist | Expose `--deny-modules <pattern,...>` CLI flag |
| 3 | Command timeout | Long-running process launch or evaluation may block | Async timeout with `tokio::time::timeout` |
| 4 | Non-interactive auto-confirm | LLDB asking interactive prompts (y/n) | Initialized with `settings set auto-confirm true` |
