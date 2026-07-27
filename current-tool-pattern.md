# Current Tool Pattern for ShadowAI Tools

This documents the pattern used in `crates/shadowai-tools/src/` for creating rig tools.

## Project Layout

```text
crates/shadowai-tools/src/
├── lib.rs          # Module declarations, tool re-exports, RepairToolCall hook
├── edit.rs         # EditTool (file editing)
├── read.rs         # ReadTool (file reading)
├── shell.rs        # ShellTool (command execution)
└── glob.rs         # GlobTool (pattern matching)
```

## Per-Tool Pattern

Each tool lives in its own `.rs` file and follows this structure:

### 1. Struct Definition

```rust
#[derive(Deserialize, Serialize)]
pub struct ToolName;
```

The struct is unit-style (`ToolName;`). It derives both `Deserialize` (for parsing JSON args) and `Serialize` (so the tool schema can be serialized).

### 2. DESCRIPTION Constant

Each tool defines a `DESCRIPTION` constant:

```rust
impl ToolName {
    pub const DESCRIPTION: &'static str = "Short description of what this tool does.";
}
```

### 3. Implementing the `Tool` Trait

Every tool implements `rig::tool::Tool`:

| Field | Notes |
|-------|-------|
| `const NAME` | Short identifier, e.g. `"read"`, `"shell"`. Used by the agent to route calls. |
| `type Error` | The error type returned on failure (e.g., `std::io::Error`, `ShellError`, `GlobError`). |
| `type Args` | Either a custom struct or a simple type like `String` / `PathBuf`. |
| `type Output` | The return type of the tool (`String`, `Vec<String>`, etc.). |

### 4. Trait Methods

#### `description()` — returns the DESCRIPTION string:
```rust
fn description(&self) -> String {
    ToolName::DESCRIPTION.to_string()
}
```

#### `parameters()` — returns a JSON Schema describing the tool's arguments:
- Tools that take complex args (file path + old/new text) return an **object** schema with typed properties.
- Tools that take a single string return a **string** schema.

Example for object-based parameters (`edit.rs`):
```rust
fn parameters(&self) -> Value {
    json!({
        "type": "object",
        "properties": {
            "file": { /* ... */ },
            "old_text": { /* ... */ },
            "new_text": { /* ... */ }
        },
        "required": ["file", "new_text"]
    })
}
```

Example for simple string parameters (`shell.rs`):
```rust
fn parameters(&self) -> serde_json::Value {
    serde_json::json!({
        "type": "string",
        "description": "The shell command to execute.",
    })
}
```

#### `call()` — async execution:
Receives deserialized args and calls the underlying filesystem/shell operation. The error type is propagated from the underlying library (`shadowai_filesystem`, `shadowai_shell`, etc.).

### 6. Error Handling with thiserror

When a tool needs to define its own error types (not just re-exporting errors from a dependency), use **`thiserror`** to derive clean, descriptive error enums:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolNameError {
    #[error(transparent)]
    UnderlyingError(#[from] underlying_lib::SomeError),
}
```

Key patterns when using `thiserror`:

- **`#[derive(Error, Debug)]`** — Always derive both.
- **`#[error(...)]`** — Provide a human-readable message for each variant.
- **`#[transparent]` + `#[from]`** — Forward the original error type so callers can match on it, while still providing a descriptive message via the outer enum. This is useful when wrapping errors from external libraries (e.g., `glob::GlobError`, `glob::PatternError`).

## Error Handling Strategy Across Tools

| Tool | Error Type | Origin |
|------|-----------|--------|
| `EditTool` | `EditFileError` | Re-exported from `shadowai_filesystem` (not thiserror-derived) |
| `ReadTool` | `std::io::Error` | Standard library, no wrapper needed |
| `ShellTool` | `ShellError` | Defined in `shadowai_shell` crate |
| `GlobTool` | `GlobError` enum | **thiserror**-derived, wraps two glob errors with `#[transparent] #[from]` |

When creating a new tool that needs its own error type, follow the `GlobTool` pattern: define an enum with `#[derive(Error, Debug)]`, use `#[error(...)]` for each variant, and use `#[transparent] #[from]` to forward any underlying errors you want to preserve.

### 5. Public Re-Export in `lib.rs`

Each tool struct is re-exported at the crate root:

```rust
pub use edit::EditTool;
pub use read::ReadTool;
pub use shell::ShellTool;
pub use glob::GlobTool;
```

## Module Declarations in `lib.rs`

```rust
mod edit;
mod glob;
mod read;
mod shell;
```

## Domain Crate Architecture

The actual work (file I/O, shell execution) is split into dedicated **domain crates** that `shadowai-tools` depends on. This keeps concerns separated: domain crates own the low-level operations and their error types; `shadowai-tools` owns the rig tool wrappers.

### Crate Layout

| Crate | Purpose | Key APIs |
|-------|---------|----------|
| `crates/shadowai-filesystem/` | Filesystem operations (read, write, edit) | `read_file()`, `write_file()`, `edit_file()` |
| `crates/shadowai-shell/` | Shell command execution | `execute()` |

### Dependency Flow

```text
shadowai-tools (rig tool wrappers)
  ├── depends on shadowai-filesystem (file I/O + EditFileError)
  └── depends on shadowai-shell   (shell execution + ShellError)

Each domain crate:
  ├── Owns the async implementation (tokio-based)
  ├── Defines its own error type (using thiserror where appropriate)
  └── Exposes a small, focused public API in src/lib.rs
```

### Example: `shadowai-filesystem`

The filesystem domain exposes three functions plus an error enum:

```rust
// Public API surface of shadowai-filesystem/src/lib.rs
pub async fn read_file(path: PathBuf) -> Result<String, std::io::Error>;
pub async fn write_file(path: PathBuf, content: &str) -> Result<(), std::io::Error>;
pub async fn edit_file(
    path: PathBuf,
    old_text: Option<&str>,
    new_text: &str,
) -> Result<String, EditFileError>;

#[derive(Debug, thiserror::Error)]
pub enum EditFileError {
    #[error("old_text is required when editing an existing file")]
    OldTextNotFound,
    #[error("No occurrences of '{0}' found in the file.")]
    NoOccurrencesFound(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}
```

The `EditFileError` enum uses thiserror with `#[derive(Error, Debug)]`, `#[error(...)]` for descriptive messages, and `#[transparent] #[from]` to forward the underlying I/O error. This same pattern is reused when `shadowai-tools` defines its own errors (see section "Error Handling with thiserror").

### Example: `shadowai-shell`

The shell domain wraps bash command execution:

```rust
// Public API surface of shadowai-shell/src/lib.rs
pub async fn execute(command: &str) -> Result<String, ShellError>;

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("Command failed (exit code {0}): {1}\n{2}")]
    CommandFailed(i32, String, String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),
}
```

### How Tools Wrap Domain Crates

Each tool in `shadowai-tools` is essentially a thin adapter: it imports the domain crate's API, implements `rig::tool::Tool`, and forwards calls to the domain function. The domain error types are used directly as the tool's `Error` type (or wrapped via thiserror when needed).

This pattern can be extended by adding more modules following the same per-tool structure and updating `lib.rs` accordingly.