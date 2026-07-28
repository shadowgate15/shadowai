# WebSearch Tool — Existing Tool Patterns in shadowai-tools

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

---

## 1. Existing Tool Patterns in shadowai-tools

**Prompt:** Study `crates/shadowai-tools/src/web_fetch.rs`, `read.rs`, `edit.rs`, `shell.rs`, and `glob.rs` to document:

- The `Tool` trait signature (NAME, Args, Output types)
- How errors are declared (`type Error = _`)
- How parameters are exposed via `parameters()` → `Value`
- Any shared utilities or common error types

**Why:** The new websearch tool should follow the same conventions so it integrates cleanly with the existing toolkit.

### 1. Tool trait signature

All tools implement `rig::tool::Tool` with this structure:

```rust
impl Tool for <ToolName> {
    const NAME: &'static str = "tool_name";
    type Error = <CustomError>;     // per-tool error enum
    type Args = <ArgsStruct>;       // struct with doc-commented fields
    type Output = String;           // or Vec<String> (glob)

    fn description(&self) -> String { ... }
    fn parameters(&self) -> Value { schemars::schema_for!(Args).to_value() }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> { ... }
}
```

### 2. Error declarations

Each tool defines its own `#[derive(Error, Debug)]` enum using `thiserror`:

- **shell.rs**: `ShellError` with variants (`CommandFailed`, `IOError`, etc.)
- **edit.rs**: wraps externally-defined `EditFileError` from `shadowai_filesystem`
- **web_fetch.rs**: wraps `FetchError` from `fetchkit`
- **read.rs**: raw `std::io::Error` (only exception to the pattern)

### 3. Parameters via `parameters()` → `Value`

All tools use `schemars::schema_for!(Args).to_value()`. The `Args` struct derives `Deserialize, JsonSchema`, and each field has a doc comment explaining it — this is what populates the tool's JSON schema for the agent.

### 4. Shared utilities

- Tools derive `Deserialize, Serialize` (except read which uses only `JsonSchema`)
- Description is static: either a `pub const DESCRIPTION: &'static str = "..."` or returned via `description()` method
- All tools follow the same async call pattern with error propagation