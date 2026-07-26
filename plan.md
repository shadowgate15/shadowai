# Domain-Driven Design Refactor Plan — `shadowai`

## Current Architecture (Monolithic)

The existing code is a single-file agent (`src/main.rs`) that bundles everything together:
- The main conversation loop and history management
- File read/write/edit operations with their own tool definitions
- Shell command execution
- A repair hook for invalid tool calls

All of these are tightly coupled in one file.


## Target Architecture (DDD — Bounded Contexts)

We will split the project into **4 domain crates** under `crates/`:

```
shadowai/
├── Cargo.toml                  # Workspace root
├── src/main.rs                 # Thin entry-point / orchestrator
└── crates/
    ├── shadowai-agent/         # Agent & conversation domain
    │   └── src/lib.rs
    ├── shadowai-filesystem/    # Filesystem operations domain
    │   └── src/lib.rs
    ├── shadowai-shell/         # Shell command execution domain
    │   └── src/lib.rs
    └── shadowai-tools/         # Tool registry & integration layer
        └── src/lib.rs
```

---

## 1. `shadowai-agent` — Agent & Conversation Domain

**Purpose**: Own the conversational loop, history, user input parsing, and agent configuration (model selection, system prompt, temperature). This is the core "application" bounded context.

**Responsibilities**:
- Parse and manage conversation history (`Vec<rig::message::Message>`)
- Handle multi-line user input (the `"""` toggle)
- Drive the streaming chat loop with `Agent` from `rig`
- Own agent configuration: model name, system prompt preamble, temperature, max turns

**Public API**:
```rust
pub struct AgentConfig {
    pub model: String,
    pub system_prompt: String,
    pub temperature: f32,
    pub default_max_turns: u32,
}

pub async fn run_agent_loop(config: AgentConfig) -> Result<(), anyhow::Error>
```

**Internal types**:
- `UserInput` enum (`Input(String)`, `Exit`) — internal to this crate
- `ConversationHistory` — manages the message history slice

**Dependencies**: `rig`, `shadowai-tools`. The agent depends on tools because it needs to receive, dispatch, and handle tool calls from the conversation loop.


## 2. `shadowai-filesystem` — Filesystem Operations Domain

**Purpose**: Encapsulate all filesystem read/write/edit operations as pure domain logic, free of tool definitions and rig-specific concerns.

**Responsibilities**:
- Open and read file contents from a path
- Write/create files with given content at a path (creating parent dirs)
- Edit/replace occurrences within an existing file (returning the new contents)
- Report errors cleanly without exposing `rig::tool::ToolError` or internal crate details

**Public API**:
```rust
pub async fn read_file(path: PathBuf) -> Result<String, anyhow::Error>
pub async fn write_file(path: PathBuf, content: &str) -> Result<(), anyhow::Error>
pub async fn edit_file(path: PathBuf, old_text: &str, new_text: &str) -> Result<String, anyhow::Error>
```

**Internal types**:
- `FileContents` — owned string of file data (domain value object)
- Error types mapped to `anyhow::Error` for clean boundary semantics

**Dependencies**: Only `tokio`, `anyhow`. No `rig` dependency.


## 3. `shadowai-shell` — Shell Command Execution Domain

**Purpose**: Own all shell command execution logic as a bounded domain, returning structured results with exit code and output.

**Responsibilities**:
- Execute arbitrary bash commands via `std::process::Command`
- Capture stdout/stderr and the exit status
- Translate failures into descriptive errors (exit code + stderr/stdout)

**Public API**:
```rust
pub struct ShellResult {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

pub fn execute(command: &str) -> Result<ShellResult, anyhow::Error>
```

**Internal types**:
- `ShellResult` — value object capturing the full outcome of a shell command (success/failure + all streams)

**Dependencies**: Only `anyhow`. No async runtime needed.


## 4. `shadowai-tools` — Tool Registry & Integration Layer

**Purpose**: Bridge the domain crates with the agent's tool system. This is an **integration context**, not a pure domain — it owns the mapping between domain operations and `rig::tool` definitions.

**Responsibilities**:
- Define tool function signatures (`read_file`, `write_file`, `edit_file`, `shell_command`) as async functions annotated with `#[rig::tool_macro]`
- Register each tool so the agent can invoke them
- Handle repair logic: redirect invalid tool calls (e.g. `write_file` → `edit_file`) and retry on unknown tools

**Public API**:
```rust
pub struct ToolRegistry {
    // internal — owns all registered tool functions
}

impl ToolRegistry {
    pub fn new() -> Self;
    pub async fn register(&mut self);  // wires tools into the rig Agent via rig's mechanism
}
```

**Internal types**:
- `RepairHook` — implements `rig::agent::AgentHook` to handle tool-call repair logic (mirrors existing `RepairToolCall`)

**Dependencies**: `shadowai-filesystem`, `shadowai-shell`, `rig`. The tools crate bridges the pure domain crates with rig's tool system.


## 5. Root Workspace (`Cargo.toml`)

The root workspace will become a simple aggregator that:
- Declares the four crates as members
- Defines the binary crate that depends on all four domain crates and `rig`

**Binary entry-point (`src/main.rs`)** becomes thin — roughly:
```rust
use shadowai_agent::run_agent_loop;
use shadowai_tools::ToolRegistry;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = AgentConfig { /* ... */ };
    ToolRegistry::new().register().await?;
    run_agent_loop(config).await
}
```


## 6. Dependency Graph

```
shadowai-filesystem   ← pure domain (no deps)
shadowai-shell        ← pure domain (no deps)
shadowai-agent        ← depends on shadowai-tools (integration layer — needs tools to handle tool calls)
shadowai-tools        ← depends on filesystem, shell (integration)
main.rs binary        ← depends on all four crates + rig
```

This follows the **Dependency Inversion Principle**: domain entities own their logic; integration/context layers depend on them. The `shadowai-agent` is not a pure domain — it depends on `shadowai-tools` because it needs to receive, dispatch, and handle tool calls from the conversation loop.


## 7. Migration Steps

1. Create `crates/shadowai-filesystem/src/lib.rs` — extract file operations (pure domain)
2. Create `crates/shadowai-shell/src/lib.rs` — extract shell execution (pure domain)
3. Create `crates/shadowai-tools/src/lib.rs` — define tools & repair hook (depends on filesystem + shell domains)
4. Create `crates/shadowai-agent/src/lib.rs` — extract conversation logic (depends on shadowai-tools)
5. Update root `Cargo.toml` as a workspace with 4 members + binary
6. Slim down `src/main.rs` to orchestrate the four crates

---

*Plan written first, no other files edited yet.*
