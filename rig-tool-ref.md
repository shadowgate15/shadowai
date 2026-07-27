# Rig Tool Trait Implementation Reference

This document summarizes how to implement the [`Tool`](https://docs.rs/rig/0.40.0/rig_core/tool/trait.Tool.html) trait in the `rig` crate (version 0.40.0). It serves as a reference for future sessions where we will be implementing a Tool.

**Source location:** `/Users/taylorschley/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rig-core-0.40.0/src/tool/mod.rs`

---

## 1. The `Tool` Trait Definition

```rust
pub trait Tool: Sized + WasmCompatSend + WasmCompatSync {
    /// The name of the tool. This name should be unique within a single
    /// [`ToolSet`] or other registration scope that dispatches tools by name.
    const NAME: &'static str;

    /// The error type of the tool.
    type Error: std::error::Error + WasmCompatSend + WasmCompatSync + 'static;
    /// The arguments type of the tool.
    type Args: for<'a> Deserialize<'a> + WasmCompatSend + WasmCompatSync;
    /// The output type of the tool.
    type Output: Serialize;

    /// A method returning the name of the tool (default impl returns NAME).
    fn name(&self) -> String { Self::NAME.to_string() }

    /// Model-facing description of what the tool does.
    fn description(&self) -> String;

    /// JSON Schema for the tool arguments.
    fn parameters(&self) -> serde_json::Value;

    /// The tool execution method (default: async function).
    fn call(
        &self, args: Self::Args,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + WasmCompatSend;

    /// Tool execution with per-call runtime extensions. Default delegates to `call`.
    fn call_with_extensions(&self, args: Self::Args, _extensions: &ToolCallExtensions)
        -> impl Future<Output = Result<Self::Output, Self::Error>> + WasmCompatSend {
            self.call(args)
    }

    /// Classify an error into a structured [`ToolFailure`] (default: `Other`).
    fn classify_error(&self, error: &Self::Error) -> ToolFailure {
        ToolFailure::other(error.to_string())
    }

    /// Execute the tool, returning a structured [`ToolReturn`]. Default wraps call.
    fn call_structured(
        &self, args: Self::Args, extensions: &ToolCallExtensions,
    ) -> impl Future<Output = Result<ToolReturn<Self::Output>, Self::Error>> + WasmCompatSend {
            async move { self.call_with_extensions(args, extensions).await.map(ToolReturn::success) }
    }
}
```

---

## 2. Generic Type Parameters

| Parameter | Constraint | Purpose | Example |
|-----------|-----------|---------|---------|
| `Error` | `std::error::Error + WasmCompatSend + WasmCompatSync + 'static` | Tool's error type, surfaced via [`ToolFailure`](crate::tool::result::ToolFailure) | custom typed errors |
| `Args` | `for<'a> Deserialize<'a> + WasmCompatSend + WasmCompatSync` | Deserialized from JSON before execution | struct, `serde_json::Value`, `()` (unit type) |
| `Output` | `Serialize` | Serialized to model-visible string | `String`, `i32`, `serde_json::Value` |

**Key behaviors:**
- **`Args: ()` is the unit type.** When no arguments are needed, use `type Args = ();`. The framework handles JSON null → `{}` normalization automatically.
- **`Output: String`** keeps the output verbatim; any other `Serialize` type gets JSON-encoded.

---

## 3. Minimal Implementation (Simplest Tool)

```rust
use rig_core::tool::{Tool, ToolCallExtensions};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
#[error("my tool error")]
pub struct MyError;

#[derive(Deserialize)]
pub struct MyArgs {
    pub input: String,
}

#[derive(Serialize)]
pub struct MyOutput {
    pub result: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MyTool;

impl Tool for MyTool {
    const NAME: &'static str = "my_tool";
    type Error = MyError;
    type Args = MyArgs;
    type Output = MyOutput;

    fn description(&self) -> String {
        "Description of what the tool does".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "The input to process"
                }
            },
            "required": ["input"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = format!("Processed: {}", args.input);
        Ok(MyOutput { result })
    }
}
```

---

## 4. Overriding `call_structured` for Rich Results

Override [`call_structured`](crate::tool::Tool::call_structured) when you need to attach metadata, report handled failures, or mark calls as denied:

```rust
use rig_core::tool::{ToolCallExtensions, ToolFailure, ToolReturn};

impl Tool for MyHandledFailureTool {
    const NAME: &'static str = "lookup";
    type Error = MyError;
    type Args = MyArgs;
    type Output = String;

    fn description(&self) -> String { "Looks up a record".to_string() }
    fn parameters(&self) -> serde_json::Value { /* JSON schema */ }

    async fn call_structured(
        &self, args: Self::Args, extensions: &ToolCallExtensions,
    ) -> Result<ToolReturn<Self::Output>, Self::Error> {
        match lookup_record(&args).await {
            Ok(record) => Ok(ToolReturn::success(format!("Found record"))),
            Err(e) if e.is_not_found() => Ok(ToolReturn::failed(
                format!("Record not found; try a different id"),
                ToolFailure::not_found("record missing").with_http_status(404).with_code("NOT_FOUND")
            )),
            Err(e) => Err(e), // re-classified by classify_error
        }
    }
}
```

---

## 5. Overriding `call_with_extensions` for Context-Aware Tools

Override [`call_with_extensions`](crate::tool::Tool::call_with_extensions) when you need per-call runtime context (auth tokens, session IDs):

```rust
use rig_core::tool::{Tool, ToolCallExtensions};
use serde_json;

#[derive(Clone)]
pub struct SessionId(pub String);

struct AuthProbeTool;

impl Tool for AuthProbeTool {
    const NAME: &'static str = "auth_probe";
    type Error = MyError;
    type Args = serde_json::Value;
    type Output = String;

    fn description(&self) -> String { "Probes session context".to_string() }
    fn parameters(&self) -> serde_json::Value { /* JSON schema */ }

    async fn call_with_extensions(
        &self, _args: Self::Args, extensions: &ToolCallExtensions,
    ) -> Result<Self::Output, Self::Error> {
        if let Some(session) = extensions.get::<SessionId>() {
            Ok(format!("session:{}", session.0))
        } else {
            Ok("no-session".to_string())
        }
    }
}
```

---

## 6. Overriding `classify_error` for Structured Failure Handling

Override [`classify_error`](crate::tool::Tool::classify_error) to map your error variants onto standard failure kinds:

```rust
use rig_core::tool::{ToolFailure, ToolFailureKind};

struct FlakyTool { kind: ToolFailureKind }

impl Tool for FlakyTool {
    const NAME: &'static str = "flaky_tool";
    type Error = MyError;
    type Args = serde_json::Value;
    type Output = String;

    fn description(&self) -> String { "A tool that always fails".to_string() }
    fn parameters(&self) -> serde_json::Value { /* JSON schema */ }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> { Err(MyError) }

    fn classify_error(&self, error: &Self::Error) -> ToolFailure {
        match self.kind {
            ToolFailureKind::Timeout => ToolFailure::timeout(error.to_string()),
            ToolFailureKind::NotFound => ToolFailure::not_found(error.to_string()).with_http_status(404).with_code("NOT_FOUND"),
            ToolFailureKind::RateLimited => ToolFailure::rate_limited(error.to_string()).with_http_status(429),
            other => ToolFailure::other(error.to_string()),
        }
    }
}
```

---

## 7. The `ToolDyn` Dynamic Dispatch Trait

[`ToolDyn`](crate::tool::ToolDyn) provides object-safe dynamic dispatch for runtime tool invocation:

| Method | Description |
|--------|-------------|
| `fn name(&self) -> String` | Tool name for dispatch and provider advertisement |
| `fn description(&self) -> String` | Model-facing description |
| `fn parameters(&self) -> serde_json::Value` | JSON Schema for tool arguments |
| `fn call<'a>(&'a self, args: String) -> WasmBoxedFuture<'a, Result<String, ToolError>>` | Execution with JSON-encoded string args and model-visible string output |
| `fn call_with_extensions<'a>(...)` | Extension-aware execution (default delegates to `call`) |
| `fn call_structured<'a>(...)` | Returns a [`ToolExecutionResult`](crate::tool::result::ToolExecutionResult) with structured outcome |

---

## 8. Structured Result Types

### `ToolReturn<T>` — The tool's return value carrying an optional structured outcome and metadata:
```rust
pub struct ToolReturn<T> {
    pub output: T,           // Serialized to model-visible string
    pub outcome: ToolReturnOutcome,  // Success / Error(ToolFailure) / Denied
    pub extensions: ToolResultExtensions,  // Metadata never sent to the model
}
```

- **`ToolReturn::success(output)`** — plain success with no metadata
- **`ToolReturn::failed(output, failure)`** — handled failure that still shows output to the model
- **`ToolReturn::denied(output)`** — tool refused the call (tool-side counterpart to hook `Flow::Skip`)

### `ToolExecutionResult` — The full structured result of a single tool execution:
```rust
pub struct ToolExecutionResult {
    pub(crate) model_output: String,   // Text delivered to the LLM
    pub(crate) outcome: ToolOutcome,   // Success / Error(ToolFailure) / Skipped / Denied
    pub(crate) extensions: ToolResultExtensions,  // Provider/application metadata (never sent to model)
}
```

---

## 9. The `ToolSet` and Registration

Tools are registered via [`ToolSet`](crate::tool::ToolSet):

```rust
use rig_core::tool::{ToolDyn, ToolSet};

// Static registration:
let toolset = ToolSet::default()
    .add_tool(MyTool);  // impl ToolDyn + 'static

// Builder pattern for static and dynamic tools:
let toolset = ToolSet::builder()
    .static_tool(MySimpleTool)
    .dynamic_tool(MyRaggableTool)
    .build();

// Calling a tool by name:
let result = toolset.call("my_tool", "{}".to_string()).await?;
```

---

## 10. The `ToolEmbedding` Trait (for RAG-able Tools)

The [`ToolEmbedding`](crate::tool::ToolEmbedding) trait extends Tool to allow tools that can be stored in a vector store and retrieved via RAG:

| Method | Description |
|--------|-------------|
| `fn embedding_docs(&self) -> Vec<String>` | Documents for embeddings (empty if not ragged) |
| `fn context(&self) -> Self::Context` | Tool's static configuration / local context |
| `fn init(state: Self::State, context: Self::Context) -> Result<Self, Self::InitError>` | Reconstruct the tool from stored state and context |

```rust
pub trait ToolEmbedding: Tool {
    type InitError: std::error::Error + WasmCompatSend + WasmCompatSync + 'static;
    type Context: for<'a> Deserialize<'a> + Serialize;
    type State: WasmCompatSend;

    fn embedding_docs(&self) -> Vec<String>;
    fn context(&self) -> Self::Context;
    fn init(state: Self::State, context: Self::Context) -> Result<Self, Self::InitError>;
}
```

---

## 11. Key Design Notes and Conventions

- **`ToolSet` preserves registration order.** Tools are stored in an `IndexMap<String, ToolType>` keyed by name, so iteration follows insertion order. Re-registering a tool with the same name replaces it in place (keeping its position).
- **The registered name is the single source of truth** for provider advertisement and dispatch. If you override `name()` at runtime, definitions will still use the original registration name.
- **`call_structured` is the structured dynamic entry point.** Under dynamic dispatch, the agent loop routes every tool call here via the blanket [`ToolDyn`](crate::tool::ToolDyn) impl. If you override it, the `call` / `call_with_extensions` bodies are unreachable on that path (a direct Tool call still runs them).
- **`ToolReturn` output is serialized verbatim for Strings; other types become JSON.** This means switching from `Output = String` to `Output = ToolReturn<String>` never changes what the model sees for success.
- **`ToolCallExtensions::EMPTY`** is a `'static` shared empty instance used by dispatch layers — no allocation needed when no caller-provided values are required.
- **`ToolResultExtensions` (not sent to the model)** carries provider/application metadata like raw HTTP headers, response IDs, retry hints, etc. Use `with_extension()` / `with_extensions()` to attach them.

---

## 12. Example: Complete Working Tool

```rust
use rig_core::tool::{Tool, ToolCallExtensions, ToolFailure};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
#[error("HTTP request failed")]
pub struct HttpError { pub status: u16 }

#[derive(Deserialize, Debug)]
pub struct SearchArgs {
    pub query: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct WebSearchTool;

impl Tool for WebSearchTool {
    const NAME: &'static str = "web_search";
    type Error = HttpError;
    type Args = SearchArgs;
    type Output = SearchResult;

    fn description(&self) -> String { "Search the web for information".to_string() }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results (optional)"
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let title = format!("Results for '{}'", args.query);
        let url = format!("https://example.com/search?q={}", args.query.replace(' ', "%20"));
        let snippet = format!("Found {} results", if args.limit.unwrap_or(10) > 5 { "many" } else { "a few" });

        Ok(SearchResult { title, url, snippet })
    }
}
```
