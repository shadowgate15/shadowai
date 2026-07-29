# WebSearch Tool — Implementation Plan

**Goal:** Implement the `web_search` tool for ShadowCode, following the established patterns in `shadowai-tools`. Engine-specific concerns live under a new workspace crate at `crates/shadowai-search-engines`; the tool facade in `shadowai-tools` is responsible only for orchestration.

---

## Architecture Note: DDD Boundaries

The **tool facade** lives in `crates/shadowai-tools/src/web_search/` and handles orchestration — declaring the `Tool` interface, caching, parallel execution, and error aggregation. Engine-specific concerns live under a **separate workspace crate** at `crates/shadowai-search-engines/`:

- Adding a new engine later is an isolated change to its own file within the search-engines crate — no impact on the tool facade or existing engines.
- The tool facade never touches raw HTTP responses; it only sees normalized `WebSearchResult` structs and error variants from the search-engines crate (imported as a dependency).
- Tests for individual engines are independent of tests for the orchestration layer.

---

## Phase 1: Project Setup & Infrastructure

### 1a. Create the `shadowai-search-engines` workspace crate

- Add `crates/shadowai-search-engines/` to the workspace (`Cargo.toml`).
- Create initial file: `src/lib.rs` with module-level re-exports + shared types.
- Add dependencies in its `Cargo.toml`:
  - `tokio` (already a workspace dependency) for async primitives (`spawn`, `time::timeout`).
  - `thiserror` for error types.
  - `reqwest` for HTTP transport.
  - `mockito` as dev-dependency for integration tests.

### 1b. Define the tool trait interface in shadowai-tools (unchanged)

- Implement `rig::tool::Tool` with:

  ```rust
  const NAME: &'static str = "web_search";
  type Error = WebSearchError;
  type Args = WebSearchArgs;
  type Output = String; // serialized Vec<WebSearchResult> as JSON string
  ```

- Define `WebSearchError` enum (see section 6).

### 1c. Define the Args struct in shadowai-tools (unchanged)

- Create `WebSearchArgs` with query, max_results, search_type, language, region fields.
- Implement `parameters()` → `Value` using `schemars::schema_for!(WebSearchArgs)`.

---

## Phase 2: Engine Response Types & Normalization (in shadowai-search-engines/)

### 2a. Define engine-specific response structs + native parsing

Each engine file owns its HTTP request and deserialization into a native response type. No normalization logic — that happens in the shared `normalization.rs` module below.

- **GoogleCustomSearchResponse** — fields: `title`, `link`, `snippet`.
- **BingWebSearchResponse** — fields: `name`, `url`, `snippet`, `dateLastCrawled`.
- **DuckDuckGoInstantAnswerResponse** — fields: `Headline`, `Body`, `Source`, `Url` (from Results array).
- **SearXNGResponse** — fields: `title`, `url`, `content`, `timestamp` (Unix epoch seconds).

### 2b. Implement the normalization pipeline (shared across engines, in shadowai-search-engines/)

A private method/helper that converts native response types → canonical `WebSearchResult`:

1. **Parse** — each engine file deserializes its own HTTP response into its native struct.
2. **Map** — translate each native field to canonical names using a match on engine ID (e.g., Google's `title` → `title`, Bing's `name` → `title`). DuckDuckGo maps Results array items (`Headline` → `title`, `Body` → `snippet`).
3. **Sanitize** — strip HTML from any snippet/title that came back in markup form (defensive trim). SearXNG's Unix timestamp → ISO-8601 via string formatting or a simple epoch converter.
4. **Collect** — push each normalized result into `Vec<WebSearchResult>`.

### 2c. Define the canonical output struct in shadowai-search-engines/

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

Derive only `Debug` and `Clone` for now (no serialization needed; output is serialized as JSON string before returning to agent).

---

## Phase 3: HTTP Transport & Error Handling (in shadowai-search-engines/)

### 3a. Implement per-engine HTTP requests

Each engine file owns its own HTTP client setup and request construction:

- **DuckDuckGo:** `GET https://api.duckduckgo.com/?q={query}&format=jsonf&no_html=1` — no auth headers.
- **SearXNG (using stable public instance):** `GET {instance_url}/search?q={query}&format=json` — no auth for public instances. Use `searx.be` or similar; verify via `robots.txt`.

### 3b. Implement the retry loop with exponential backoff (per engine)

- For timeout and connect failures: initial delay 100ms, double each attempt, ±30% jitter, capped at 8s max delay, 3 retries max per engine.
- Convert inner call errors to `WebSearchError::Timeout` or `WebSearchError::ConnectFailure`.
- Rate-limit (HTTP 429): detect status code, parse `Retry-After` header; do NOT retry same engine — skip it and use remaining engines in parallel execution.

### 3c. Handle partial engine failures gracefully (in tool facade)

- If at least one engine succeeds, return its results without surfacing the failure to the agent (log internally).
- If all engines fail hard (timeout/connect/config), surface aggregated error: "All search engines are currently unavailable."
- If all engines fail except rate-limited ones, surface soft error with note about which engine hit limit.

---

## Phase 4: Async Execution & Merge/Dedup (in shadowai-tools)

### 4a. Fire all engines concurrently

```rust
pub async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
    let engines = self.engines(); // [duckduckgo, searxng] for MVP
    let query = args.query.clone();
    let max_results = args.max_results;

    let handles: Vec<_> = engines.iter().map(|engine| {
        tokio::spawn(async move {
            Self::run_engine(engine, &query, max_results).await
        })
    }).collect();

    let results: Vec<Option<Vec<WebSearchResult>>> = try_join_all(handles).await?;
    let successful: Vec<_> = results.into_iter().flatten().collect();

    if successful.is_empty() {
        return Err(WebSearchError::EmptyResults { query });
    }

    Ok(Self::merge_and_dedup(successful))
}
```

### 4b. Per-engine timeout handling

- Each spawned engine task runs inside `tokio::time::timeout` with a 5-second deadline.
- Timeout converts to `WebSearchError::Timeout(engine_id)` so callers can skip it during merge.
- One slow engine doesn't extend total call time — other engines continue independently.

### 4c. Merge & URL-dedup logic

```rust
fn merge_and_dedup(results: Vec<Vec<WebSearchResult>>) -> String {
    let mut url_set = HashSet::new();
    let mut merged = Vec::new();

    for engine_results in results.into_iter().flatten() {
        for result in engine_results {
            if !url_set.insert(result.url.clone()) {
                continue; // duplicate URL — skip silently
            }
            merged.push(result);
        }
    }

    merged.sort_by(|a, b| b.relevance_score.cmp(&a.relevance_score).then_with(|| a.url.cmp(&b.url)));

    serde_json::to_string_pretty(&merged).unwrap_or_default()
}
```

---

## Phase 5: Caching Layer (in shadowai-tools)

### 5a. In-memory cache with TTL

- Use `std::collections::HashMap<String, CachedEntry>` keyed on normalized query string.
- Each entry stores the cached response body (JSON string) + insertion timestamp.
- Default TTL: 120 seconds; range 60–300s acceptable per research doc.

### 5b. Cache lookup and eviction logic

```rust
async fn check_cache(&self, query: &str) -> Option<String> {
    let now = Instant::now();
    if let Some(entry) = self.cache.get(query).and_then(|e| e.cached_response.as_ref()) {
        if now.duration_since(entry.inserted_at) < Duration::from_secs(120) {
            return Some(entry.cached_response.clone());
        }
    }
    None
}

async fn store_in_cache(&self, query: &str, response: String) {
    let entry = CachedEntry {
        cached_response: response,
        inserted_at: Instant::now(),
    };
    self.cache.insert(query.to_string(), entry);
    // Evict oldest entries if cache exceeds 10_000 items.
    self.evict_old_entries_if_needed();
}
```

### 5c. Upgrade path for `cached` crate — same as before (not a blocker)

---

## Phase 6: Tool Description & Prompt Engineering (unchanged)

### 6a–6c — Short description, long-form guidance, query examples (same as original plan)

---

## Phase 7: Testing Strategy (updated scope)

### 7a. Unit tests (Tier 1)

- Use reqwest's `.mock()` middleware for response parsing/normalization without network I/O **within the search-engines crate**.
- Tests cover: Google/Bing/SearXNG response → `WebSearchResult` mapping; empty results handling; URL dedup collapses duplicates; backoff calculation math.

### 7b. Integration tests (Tier 2) — full engine path with mockito

- Two mock servers per test (one per engine). Each gets its own `Server::new_async()`.
- Mock each engine's endpoint with pre-configured responses matching real API shapes.
- Tests cover: Bing returns results + Google times out → merge returns Bing's results; all engines rate-limited → error surfaces correctly; SearXNG malformed JSON → `MalformedResponse` variant.

### 7c. Fixtures as constants — same approach, stored in the search-engines crate

---

## Phase 8: Implementation Order & Milestones

| Step  | What to build                                                                                    | Files touched                                                                                  | Acceptance criteria                                                                                                              |
| ----- | ------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| **1** | Crate setup for `shadowai-search-engines`, error type, canonical output struct in shadowai-tools | `web_search/mod.rs`, `error.rs`, `args.rs` (tool facade); `shadowai-search-engines/src/lib.rs` | Tool trait compiles; schema generates correctly.                                                                                 |
| **2** | DuckDuckGo engine — HTTP + response parsing                                                      | `shadowai-search-engines/src/duckduckgo.rs`                                                    | Happy path returns 3+ results from mock server; empty results handled gracefully.                                                |
| **3** | SearXNG engine — HTTP + response parsing                                                         | `shadowai-search-engines/src/searxng.rs`                                                       | Happy path parses SearXNG JSON into canonical struct; malformed input surfaces error.                                            |
| **4** | Normalization pipeline helper (shared across engines)                                            | `shadowai-search-engines/src/normalization.rs`                                                 | All four engines map to identical `WebSearchResult` format regardless of source.                                                 |
| **5** | Async execution + merge/dedup logic (tool facade)                                                | `web_search/mod.rs` or tool impl                                                               | Concurrent calls fire both engines; results merged and URL-deduped correctly; timeout skips slow engine without blocking others. |
| **6** | Caching layer integration                                                                        | `cache.rs` in shadowai-tools                                                                   | Repeated queries within TTL return cached response; expired entries refetched from network.                                      |
| **7** | Tool description + prompt engineering                                                            | Doc comments / description method                                                              | Agent uses web_search correctly in examples; rejects invalid queries at validation time.                                         |
| **8** | Unit tests (Tier 1)                                                                              | `shadowai-search-engines/tests/unit.rs`                                                        | Tier 1 passes fast (<10ms per test); parsing/normalization/dedup logic verified without network I/O.                             |
| **9** | Integration tests with mockito                                                                   | `shadowai-search-engines/tests/integration.rs`                                                 | Tier 2 passes; full end-to-end paths including timeout, rate-limit, malformed response scenarios.                                |

---

## Phase 9: Follow-Up Items (Future Sessions) — same as before

1. **Premium engine integration** — Google Custom Search / Bing v7 require API key/subscription auth env vars; add as opt-in feature after MVP is proven.
2. **Single-engine-per-call mode** (`mode: "single"`) — separate code path for cost-sensitive callers on free-tier APIs (Google ~100/day limit).
3. **Search scope expansion** — `news`, `images`, `videos` as separate enum values once engines natively support them; don't add speculative features today.
4. **Domain+snippet deduplication** — URL-dedup is sufficient for MVP; near-duplicate detection (same domain, similar snippet) can be added later if result counts prove too high.
5. **Cache persistence across restarts** — file-backed or Redis cache could survive process exits; not built today since the agent session ends when the process exits anyway.
6. **SearXNG instance selection logic** — automated health-checking (uptime tracking, failover) for picking a stable public instance from searx.space; currently manual curation of `searx.be` or similar.

---

## Summary of Resolved Design Decisions

| Question                  | Decision                                                                             | Rationale                                                                            |
| ------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| **Engine crate location** | Separate workspace crate at `crates/shadowai-search-engines/`                        | Clean DDD boundary; engines are independent concerns that grow over time.            |
| **Tool facade**           | Stays in `shadowai-tools/src/web_search/`, depends on search-engines crate           | Orchestration stays isolated from engine implementation details.                     |
| **Canonical output type** | Defined once in search-engines crate, used by tool facade                            | Single source of truth; avoids duplication between crates.                           |
| **Primary engine**        | DuckDuckGo Instant Answer API (free, no auth)                                        | Zero config; schema variance normalized away by pipeline.                            |
| **Fallback engine**       | SearXNG public instance (`searx.be` etc., free, multi-engine aggregation)            | Free alternative with upstream engines in one call.                                  |
| **Execution model**       | Multi-engine parallel execution (fire all, merge + dedup)                            | Resilience: partial failures are internal; richer results reduce downstream retries. |
| **Caching**               | In-memory `HashMap` + TTL (default 120 s); upgrade to `cached` crate later if needed | Zero dependencies for MVP; simple eviction at capacity overflow.                     |
