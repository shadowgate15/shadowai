# Plan: Web Fetch Tool (shadowai-tools)

## Goal

Add a new rig `Tool` to `crates/shadowai-tools/src/web_fetch.rs` that wraps fetchkit's `fetch()` for web requests. The tool exposes name, description, and parameters schema via the rig interface, while execution delegates to fetchkit.

## Key decisions (already confirmed)

- **Rig Tool** — implements `rig::tool::Tool`, same pattern as `read.rs` / `shell.rs` / `edit.rs`.
- **Domain crate** — no new domain crate; add `fetchkit = "0.5.0"` directly to `shadowai-tools/Cargo.toml`. fetchkit is already in the workspace root deps and available for direct use.
- **Error type** — use `fetchkit::ToolError` directly as the rig tool's Error (no wrapping). This means the rig tool re-exports or imports it; calls surface its variants to the agent loop.
- **ToolBuilder reuse** — build fetchkit's Tool once via `Tool::builder()`, then extract name / description / parameters from that instance for the rig impl. The rig struct itself is just a unit struct with no fields.

## Files to change

### 1. `crates/shadowai-tools/Cargo.toml`
Add `fetchkit = "0.5.0"` under `[dependencies]`. No other dependency changes needed.

### 2. `crates/shadowai-tools/src/web_fetch.rs` (new)

```rust
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use fetchkit::fetch;
use fetchkit::FetchRequest;
use fetchkit::FetchResponse;
use fetchkit::FetchError;

#[derive(Deserialize, JsonSchema)]
pub struct WebFetchTool;

impl WebFetchTool {
    pub const DESCRIPTION: &'static str = "Fetches web content for an LLM agent.";
}

impl Tool for WebFetchTool {
    const NAME: &'static str = "web_fetch";
    type Error = FetchError; // fetchkit::ToolError, confirmed by user
    type Args = WebFetchArgs;
    type Output = String; // or the full response metadata — TBC with user

    fn description(&self) -> String {
        Self::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> Value { /* JSON schema from fetchkit ToolBuilder */ }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Build FetchRequest from args, execute via fetch(), return response content.
        // TBC: whether to return markdown content as String or richer metadata.
    }
}
```

Concrete design for `call()`:
- Arg struct receives the URL and optional flags (markdown/text/conversion focus).
- Inside `call()`, build a `FetchRequest` with `.url(url)` + conversion options, call `fetch(req)`, return response content as String.
- If we want richer output (status code, word count, quality), we'd need to decide the Output type — but the simplest start is just the content string.

### 3. `crates/shadowai-tools/src/lib.rs`
- Add `mod web_fetch;` and `pub use web_fetch::WebFetchTool;`.

## Pending items (to confirm with user before implementing)

1. **Args shape** — What parameters does the LLM pass? Minimum viable: just a URL string (`url: String`). Optional: markdown/text conversion flags, content focus. Recommend starting with just `url` and adding more later if needed.
2. **Output shape** — Return only the response content as `String`, or include status/word count/quality metadata in a richer struct? Starting simple (just content) is recommended; can extend later.
3. **ToolBuilder reuse approach** — The plan says "build fetchkit's Tool once via ToolBuilder." Two options:
   - Build the fetchkit Tool at module init (`static FETCH_TOOL = Tool::builder().url(...).as_markdown(true)...`) and extract `name()` / `description()` / `input_schema()`. This couples our tool to a specific builder config.
   - Just use fetchkit's public types directly in `call()`: build a `FetchRequest` with `.url(url)` + conversion options, call `fetch(req)`, return the response content. No ToolBuilder needed at all — simpler and more flexible.

Given the user explicitly asked for ToolBuilder usage, we should go with option 1 (build once, reuse). But that means the rig tool's name/description/parameters are baked into a static builder config — which is fine if we want a single fixed configuration. The alternative is to skip ToolBuilder entirely and just use fetchkit's request/response types directly, which is simpler but ignores the user's explicit ask.

**Recommendation:** Use ToolBuilder for the initial setup (as requested), then extract name/description/parameters at module init. Keep it simple — one builder config, one static instance. If we later want different configurations (e.g., markdown-only vs text-only), we can add a second tool variant or a parameterized builder.

4. **SSRF / security policy** — fetchkit's ToolBuilder has `.hardened()` preset and various prefix/host/port restrictions. Should we call `.hardened()` by default? The existing tools don't have network-level concerns, but since this is web fetching with SSRF risk, hardened defaults make sense. Confirm with user.

## Implementation order

1. Update `Cargo.toml` — add fetchkit dependency.
2. Create `web_fetch.rs` — struct + rig Tool impl using fetchkit's ToolBuilder for name/description/parameters, direct fetchkit types for execution.
3. Update `lib.rs` — module declaration and re-export.
4. Run cargo check / test to verify it compiles with the rest of shadowai-tools.
