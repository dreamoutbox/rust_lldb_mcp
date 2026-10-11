# PLAN: rust-lldb-mcp [STATUS: TODO]

Rust MCP server that exposes LLDB debugging capabilities as structured MCP tools.
Uses `rmcp` for the MCP transport layer and drives `rust-lldb` as a long-lived
subprocess via `tokio::process`, communicating over the LLDB Machine Interface (MI)
protocol (`--source-quietly` + `script` / MI commands).

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
  DebugSession  ─── tokio::process::Child (rust-lldb --interpreter=mi2)
                    stdin  ──► send MI commands
                    stdout ──► parse MI responses
```

- One `rust-lldb` child process per session, started in MI mode (`--interpreter=mi2`).
- Sessions are identified by a UUID; state lives in `Arc<Mutex<SessionStore>>`.
- The MCP server runs on stdio (default) or SSE (opt-in via `--transport sse`).
- All logging goes to `stderr` via `tracing`; `stdout` is reserved for the MCP protocol.

---

## LLDB MI Primer

LLDB supports the GDB/MI protocol when launched as:

```sh
rust-lldb --interpreter=mi2
```

Commands are sent as lines to stdin; responses arrive on stdout in structured
`^result`, `*async`, `=notify` records. The driver layer wraps this into an
async request/response interface.

Key MI commands used per tool group:

| Group | MI commands |
|-------|-------------|
| Program | `-file-exec-and-symbols`, `-exec-run`, `-exec-interrupt`, `quit` |
| Breakpoints | `-break-insert`, `-break-list`, `-break-delete`, `-break-condition` |
| Stepping | `-exec-continue`, `-exec-next`, `-exec-step`, `-exec-finish` |
| Print | `-var-create` / `-var-evaluate-expression`, `-stack-list-frames`, `-stack-list-locals` |
| Eval | `-data-evaluate-expression` |
| Threads | `-thread-info`, `-thread-select` |
| Raw | pass-through any MI command string |

---

## Module Layout (target)

```
src/
  main.rs           -- CLI entry, transport wiring, server startup
  error.rs          -- thiserror error types
  session/
    mod.rs          -- re-exports
    store.rs        -- SessionStore, SessionId, DebugSession struct
    manager.rs      -- create/list/delete session logic
  driver/
    mod.rs          -- re-exports
    process.rs      -- spawn rust-lldb MI child, read/write lines
    mi.rs           -- MI command builder + response parser
    response.rs     -- typed MI response structs
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

### Phase 2: MI Driver [STATUS: TODO]

- **Goal**: Reliable async read/write layer over a `rust-lldb` MI child process.
- **Tasks**:
  - [ ] `src/driver/process.rs`:
    - `LldbProcess`: wraps `tokio::process::Child`; exposes `send_command(&str) -> Result<MiResponse>`
    - Spawn: `Command::new("rust-lldb").arg("--interpreter=mi2").stdin(Piped).stdout(Piped).stderr(Piped)`
    - Write command line to child stdin; read stdout lines until a result record (`^done`, `^error`, `^exit`) is seen; collect async/notify records emitted before it
    - Token-based multiplexing: prefix each command with an incrementing integer token (MI spec); match responses by token
  - [ ] `src/driver/mi.rs`: MI command builder — formats command strings with proper quoting/escaping
  - [ ] `src/driver/response.rs`: parse MI output records into typed structs using `serde_json` (MI record values map to JSON-compatible key=value pairs after normalization)
  - [ ] Unit tests: spawn a real `rust-lldb --interpreter=mi2`, send `-gdb-version`, assert a `^done` record arrives
- **Done when**: driver can send a command and receive a structured response; tests pass.

---

### Phase 3: Session Management [STATUS: TODO]

- **Goal**: LLM can create, list, and delete LLDB debug sessions.
- **Tasks**:
  - [ ] `src/session/store.rs`: define `SessionId` (newtype over `Uuid`), `DebugSession` (holds `LldbProcess`, session name, creation timestamp), `SessionStore` (`HashMap<SessionId, DebugSession>` behind `Arc<Mutex<...>>`)
  - [ ] `src/session/manager.rs`: `create_session` (spawns MI child), `list_sessions`, `delete_session` (sends `quit` to child, awaits exit)
  - [ ] `src/tools/session.rs`: expose three MCP tools:
    - `session_create(name: String) -> SessionId`
    - `session_list() -> Vec<SessionInfo>`
    - `session_delete(session_id: SessionId) -> ()`
  - [ ] Register tools in `src/tools/mod.rs`
  - [ ] Unit tests for `SessionStore` CRUD
- **Done when**: `session_create` returns a valid UUID; `session_delete` sends `quit` and reaps the child; tests pass.

---

### Phase 4: Program Tools [STATUS: TODO]

- **Goal**: Load an executable into a session, launch it, and kill it.
- **Tasks**:
  - [ ] `src/tools/program.rs`:
    - `load_program(session_id, executable_path: String, args: Vec<String>, env: Vec<String>, working_dir: Option<String>) -> ()` — sends `-file-exec-and-symbols <path>` then `-exec-arguments <args>`
    - `run_program(session_id, stop_at_entry: bool) -> ProcessInfo` — sends `-exec-run` (with `--start` flag if `stop_at_entry`); waits for `*stopped` async record; returns pid + stop reason
    - `kill_process(session_id) -> ()` — sends `-exec-interrupt` then `-target-detach`; or `kill` MI command
  - [ ] Error if `run_program` called before `load_program`, or `kill_process` when no process is live (track state on `DebugSession`)
  - [ ] Smoke test: load `/bin/true`, run, assert `^done` + process exits
- **Done when**: three tools work end-to-end; tests pass.

---

### Phase 5: Breakpoint Tools [STATUS: TODO]

- **Goal**: Set, list, delete breakpoints; support conditions.
- **Tasks**:
  - [ ] `src/tools/breakpoints.rs`:
    - `set_breakpoint(session_id, location: BreakpointLocation) -> BreakpointId`
      - `BreakpointLocation` enum: `File { path: String, line: u32 }` | `Function { name: String }` | `Address { addr: u64 }`
      - Maps to: `-break-insert <file>:<line>` | `-break-insert -f <name>` | `-break-insert *0x<addr>`
    - `list_breakpoints(session_id) -> Vec<BreakpointInfo>` — `-break-list`; parse `BreakpointTable`
    - `delete_breakpoint(session_id, breakpoint_id: u32) -> ()` — `-break-delete <id>`
    - `set_breakpoint_condition(session_id, breakpoint_id: u32, condition: String) -> ()` — `-break-condition <id> <expr>`
  - [ ] `BreakpointInfo`: id, location, hit count, enabled, condition
  - [ ] Tests use a small compiled fixture binary in `tests/fixtures/`
- **Done when**: breakpoints survive a launch/stop cycle; condition respected; tests pass.

---

### Phase 6: Stepping Tools [STATUS: TODO]

- **Goal**: Full stepping control after a breakpoint hit.
- **Tasks**:
  - [ ] `src/tools/stepping.rs`:
    - `continue_process(session_id) -> StopReason` — `-exec-continue`; await `*stopped`
    - `step_next(session_id, thread_id: Option<u64>) -> StopReason` — `-exec-next [--thread <id>]`
    - `step_into(session_id, thread_id: Option<u64>) -> StopReason` — `-exec-step [--thread <id>]`
    - `step_out(session_id, thread_id: Option<u64>) -> StopReason` — `-exec-finish [--thread <id>]`
    - `finish_frame(session_id, thread_id: Option<u64>) -> FinishResult` — `-exec-finish`; capture `return-value` from `*stopped` record if present
  - [ ] Shared helper: `wait_for_stopped(driver, timeout_ms: u64) -> StopEvent` — polls async records until `*stopped` or timeout
  - [ ] All stepping tools default to selected thread when `thread_id` is `None`
- **Done when**: step through a loop in a fixture binary; assert `line` advances correctly; tests pass.

---

### Phase 7: Print Tools [STATUS: TODO]

- **Goal**: Inspect variables, frames, and filtered backtraces.
- **Tasks**:
  - [ ] `src/tools/print.rs`:
    - `print_variable(session_id, name: String, thread_id: Option<u64>, frame_index: Option<u32>) -> ValueInfo` — `-var-create - @ <name>` in the target frame; parse value + type
    - `print_frame_info(session_id, thread_id: Option<u64>, frame_index: Option<u32>) -> FrameInfo` — `-stack-info-frame`; returns file, line, function, module
    - `print_backtrace(session_id, thread_id: Option<u64>) -> Vec<FrameInfo>` — `-stack-list-frames`; pass through `src/backtrace.rs` filter
  - [ ] `src/backtrace.rs`: user-code frame filter
    - Input: `Vec<FrameInfo>` from `-stack-list-frames`
    - Drop frames where the `file` path contains any of:
      - `.rustup/toolchains`, `/rustc/` (stdlib)
      - Configurable denylist (default: `tokio`, `tokio_util`, `hyper`, `tower`, `futures`, `async_std`, `smol`, `mio`, `tracing`)
    - Failsafe: if all frames are filtered, return the full list
  - [ ] `ValueInfo`: name, type, value string
- **Done when**: `print_backtrace` on a tokio program returns only user frames; tests pass.

---

### Phase 8: Expression Evaluation [STATUS: TODO]

- **Goal**: Evaluate arbitrary expressions in the current frame.
- **Tasks**:
  - [ ] `src/tools/eval.rs`:
    - `evaluate_expression(session_id, expression: String, thread_id: Option<u64>, frame_index: Option<u32>) -> EvalResult`
    - Sends `-data-evaluate-expression "<expr>"` (with optional `--thread`/`--frame` flags)
    - Parses `value` field from `^done`; surfaces `msg` from `^error`
  - [ ] `EvalResult`: `{ value: String, error: Option<String> }`
  - [ ] Unit test: stop at breakpoint, evaluate an arithmetic expression and a local variable
- **Done when**: errors surfaced as `EvalResult.error`, not panics; tests pass.

---

### Phase 9: Thread Tools + Raw Command [STATUS: TODO]

- **Goal**: Thread enumeration, thread switching, and raw MI command escape hatch.
- **Tasks**:
  - [ ] `src/tools/threads.rs`:
    - `list_threads(session_id) -> Vec<ThreadInfo>` — `-thread-info`; include id, name, state, frame count
    - `switch_thread(session_id, thread_id: u64) -> ()` — `-thread-select <id>`
    - `run_lldb_command(session_id, command: String) -> CommandOutput` — send the raw string as-is to the MI child stdin; collect all output lines until next result record; return as unstructured text
  - [ ] `run_lldb_command` output is intentionally raw/unstructured; document this in tool description
- **Done when**: `list_threads` returns at least main thread; `run_lldb_command("version")` returns LLDB version; tests pass.

---

### Phase 10: Integration & Polish [STATUS: TODO]

- **Goal**: End-to-end smoke test, README, clippy clean.
- **Tasks**:
  - [ ] `tests/integration_test.rs`: spawn the MCP server binary over stdio, send JSON-RPC `tools/list`, assert all tools present; run mini session (create -> load -> run -> breakpoint -> backtrace -> delete)
  - [ ] `README.md`: prerequisites (`rust-lldb` on PATH), build + run instructions, example MCP client config (Claude Desktop / agy), list of all tools
  - [ ] Consistent error propagation: non-existent `session_id` -> structured MCP error, not panic
  - [ ] Audit `unwrap`/`expect` outside tests; replace with `?` or add justification comments
  - [ ] `cargo clippy -- -D warnings` clean
  - [ ] `cargo nextest run --test-threads 1 --fail-fast` full suite green
- **Done when**: integration test passes; clippy clean; README covers all tools.

---

## Unknowns, Risks, Assumptions

| # | Item | Risk | Mitigation |
|---|------|------|-----------|
| 1 | MI protocol completeness | LLDB's MI support is incomplete vs GDB; some commands differ or are missing | Test each MI command against local `rust-lldb` early; use `run_lldb_command` as fallback |
| 2 | MI response parsing | MI output format is line-oriented but not JSON; requires a custom parser | Implement a simple recursive-descent parser in `driver/response.rs`; add fuzz tests |
| 3 | Async record interleaving | `*stopped`, `=library-loaded` etc. arrive between command result records | Buffer all records per token; fan out async records to a separate channel for caller inspection |
| 4 | Backtrace filter completeness | User's codebase may use crates not in the default denylist | Expose `--deny-modules <pattern,...>` CLI flag; document how to extend |
| 5 | `wait_for_stopped` timeout | Long-running programs may not stop within the default timeout | Make timeout configurable per-call; surface a `TimedOut` error rather than blocking forever |
| 6 | rmcp API surface | Macro API may differ from web search examples | Read `rmcp` source/docs before Phase 1; adjust accordingly |
