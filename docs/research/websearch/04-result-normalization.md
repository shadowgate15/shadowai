# WebSearch Tool — Result Normalization Strategy

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

---

## 4. Result Normalization Strategy

**Prompt:** Research:

- What fields should every result contain regardless of source? (title, url, snippet, date)
- How to handle engines that return different field names or types?
- Should we expose raw results alongside normalized ones?

Propose a `WebSearchResult` struct and a normalization pipeline.

**Why:** A single output type means the agent can consume results from any engine without knowing which one was used.

### 4a. Required fields (the contract)

Every result, no matter where it came from, must expose these four fields:

| Field      | Type   | Notes                                                                                                  |
|------------|--------|--------------------------------------------------------------------------------------------------------|
| `title`    | String | Display title of the page. Google uses `title`, Bing uses `name`, DuckDuckGo uses `Headline`.     |
| `url`      | String | Canonical URL. Google uses `link`, Bing uses `url`, DuckDuckGo uses `Url`.                            |
| `snippet`  | String | Plain-text excerpt. HTML variants (`htmlSnippet`) exist but must be stripped before sending to the agent. |
| `date`     | Option<String> | ISO-8601 timestamp when available (Bing: `dateLastCrawled`; DuckDuckGo: `meta.created_date`). Null otherwise. |

The contract is intentionally small — four fields that every engine can fill, with `date` being optional since not all engines expose freshness metadata.

### 4b. Handling divergent field names and types

Each engine's response uses different casing and sometimes different value shapes:

| Engine         | Title   | URL           | Snippet       | Date                            |
|----------------|---------|---------------|---------------|---------------------------------|
| Google Custom  | `title` | `link`        | `snippet`     | —                               |
| Bing v7        | `name`  | `url`         | `snippet`     | `dateLastCrawled` (ISO-8601)   |
| DuckDuckGo     | `Headline` | `Url`      | `Body`        | `meta.created_date` (ISO-8601) |
| SearXNG        | `title` | `url`         | `content`     | `timestamp` (Unix epoch seconds)|

The normalization step maps each engine's response into the canonical struct. The mapping is a small, one-time translation — not something that happens at runtime per result. It lives in an enum match on the source engine identifier.

### 4c. Should we expose raw results alongside normalized ones?

**No.** Exposing both would:

- Double API surface area for every call site
- Force downstream consumers to know which engine was used and how to interpret each field (e.g., DuckDuckGo's `meta` object, SearXNG's `engines` array)
- Undermine the whole point of normalization — a single struct means zero engine-specific knowledge at the call site

The trade-off is lost fidelity: Google's `htmlTitle`, Bing's `displayUrl`, and SearXNG's `thumbnail` are dropped. That's acceptable because those fields are decorative (rendering, attribution) rather than semantic for agent consumption. If they become important later, a separate "rich metadata" extension can be added — but the core contract stays minimal.

### 4d. Proposed `WebSearchResult` struct

```rust
#[derive(Debug, Clone)]
pub struct WebSearchResult {
    /// Display title of the page.
    pub title: String,

    /// Canonical URL (the one the agent should fetch).
    pub url: String,

    /// Plain-text snippet extracted from the result.
    pub snippet: String,

    /// When available — ISO-8601 timestamp or None if the engine doesn't expose freshness metadata.
    pub date: Option<String>,
}
```

The struct derives only `Debug` and `Clone` — we don't need serialization for now because the tool's output is a `String` (serialized via serde before returning to the agent). If we later want structured return, adding `Serialize`/`Deserialize` is trivial.

### 4e. Normalization pipeline

The pipeline runs after each engine returns its native response:

1. **Parse** — deserialize into an engine-specific intermediate struct (or hand-rolled variant) that captures the raw fields needed for normalization.
2. **Map** — translate each intermediate field to the canonical name/value using a match on `SearchEngine::Id` (e.g., `google`, `bing`, `duckduckgo`, `searxng`). For SearXNG's Unix timestamp, convert via `timestamp.to_string()` into ISO-8601 format.
3. **Sanitize** — strip HTML from any snippet or title that came back in markup form (Google's `htmlSnippet`, Bing's `displayUrl` if used). DuckDuckGo's `no_html=1` param already handles this at the request level, but we keep a defensive trim step in case.
4. **Collect** — push each normalized result into a shared `Vec<WebSearchResult>` that is returned regardless of which engine(s) were called.

This pipeline is implemented once and reused for every engine. The mapping logic lives in a private method on the tool, so individual engines only need to provide their raw response type and the field translation — no duplication across files.