# WebSearch Tool — Testing Strategy

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

---

## 9. Testing Strategy

**Prompt:** Research:

- How to mock external HTTP calls in tests (tokio test, mockito, etc.)?
- Unit vs. integration split for a tool that depends on network I/O?
- What fixtures do we need (fake search responses)?

**Why:** Tests validate behavior without burning API credits or hitting rate limits during dev.

---

### 9a. Mocking HTTP in Rust — the three crates under consideration

For tests that exercise `web_search` (and, by extension, any tool that talks to an external service), we need a way to intercept outgoing HTTP requests and return controlled responses. Three crates are relevant:

| Crate | Downloads | Approach | Async? | Min Rust version |
|---|---|---|---|---|
| **mockito** (1.7.2) | ~50M | Local TCP server that records/mocks HTTP requests on a pool of ports. Tests configure mocks via `Server::new()`, then any HTTP client pointing at the mock's URL receives the pre-configured responses. | Yes — `_async` variants (`mock_async`, `create_async`) for `#[tokio::test]`. Sync API also available. | 1.85 |
| **wiremock** (0.6.5) | ~64M | Similar concept to mockito: a standalone local HTTP server that records and replays requests. Emphasizes black-box testing (doesn't care how the app was built). | Yes — async API via `Server::start()` / `mocks!` macro. Also has sync. | 1.70+ (not yet pinned in workspace) |
| **reqwest mock middleware** (no separate crate, reqwest feature: `mock`) | N/A | Client-side mock interceptor built into the `reqwest` HTTP client itself — no external server needed. Responses are configured per-request via `mock.method(...).with_status(...).with_body(...)`. | Yes — native async. | 1.60+ (not yet pinned in workspace) |

**Key trade-off between mockito/wiremock and reqwest's built-in mock:**

| Aspect | Local server (mockito / wiremock) | reqwest mock middleware |
|---|---|---|
| **Scope** | Intercepts *any* HTTP client that points at the mock URL — works regardless of how `fetchkit` or `reqwest` is configured. | Only works if our code uses `reqwest` directly (not through fetchkit, which wraps reqwest internally). |
| **Setup friction** | ~3 lines: `let mut server = Server::new();` then configure mocks. The test must know the mock URL and point its client there. | No setup — just call `.mock()` on a `ClientBuilder`. But we can't easily inject this into `fetchkit`, which builds its own reqwest client internally. |
| **Failure mode** | Mock server must start successfully; if it doesn't, tests fail loudly. Port conflicts are rare (auto-assigns from pool). | No failure mode — mocks just work when the test runs. But they don't exercise any of our HTTP transport logic (DNS, TCP handshake, TLS). |
| **Best for** | Integration-style tests that want to verify end-to-end behavior including request routing, retries, timeouts. | Unit-level tests where we only care about response parsing and business logic. |

### 9b. Which crate should we use? — recommendation

**Primary: `mockito`.** Reasoning:

1. **Async support is first-class.** mockito has dedicated `_async` APIs (`Server::new_async()`, `.mock(...).create_async()`), so it works naturally with `#[tokio::test]` and async test code. wiremock's async API exists but is less mature; reqwest's mock middleware is sync-first (async support is partial).
2. **Small, focused surface.** mockito's API is ~15 public functions — easy to learn for new contributors. wiremock has a larger API due to its black-box emphasis.
3. **Works with any HTTP client.** If `fetchkit` ever switches from reqwest to something else (or adds a pluggable transport), mockito still works because it just needs the test's client pointed at the mock URL. The reqwest middleware is tied specifically to reqwest's internals.
4. **Mature ecosystem.** 50M downloads, active since 2016, well-documented. wiremock has similar download counts but a newer codebase; its latest major version (0.6) requires Rust 1.70+ and some features are behind feature flags.

**Secondary: reqwest mock middleware for unit tests.** When we want to test *only* the response-parsing / normalization logic of `web_search` — without spinning up a server or mocking network I/O at all — we can use reqwest's built-in `.mock()` API directly on a `ClientBuilder`. This avoids any external dependency and is zero-setup. Good for fast, cheap unit tests that exercise `normalize_results()`, `merge_and_dedup()`, etc.

### 9c. Unit vs. integration split — where to draw the line?

For a tool like `web_search` (which depends on network I/O), we should split tests into two tiers:

| Tier | Name | What it verifies | Mocking approach | Example test cases |
|---|---|---|---|---|
| **Tier 1: Unit** | Pure logic tests | Response parsing, normalization, dedup, merge, error classification, retry/backoff math. No network I/O exercised. | reqwest mock middleware (no server) or hardcoded JSON fixtures deserialized into `WebSearchResult`. | "Google response → WebSearchResult" mapping works; empty results handled correctly; URL dedup collapses duplicates; backoff calculation yields expected values. |
| **Tier 2: Integration** | End-to-end with mocks | Full engine call path — request construction, HTTP transport (via mock), retry logic, timeout handling, partial-failure aggregation. Verifies that the tool behaves correctly when real engines are replaced by controlled responses. | mockito server with pre-configured per-engine mocks. Each engine gets its own `Server::new()`. | "Bing returns 5 results + Google times out → merge returns Bing's 5"; "all engines rate-limited → WebSearchError::RateLimited surfaces correctly"; "SearXNG returns malformed JSON → MalformedResponse variant". |

**Why not just one tier?** Unit tests are ~10× faster (no TCP/server setup) and can run in parallel without port contention. They catch the most common regressions — parsing bugs, wrong field mappings, dedup logic errors — at zero infrastructure cost. Integration tests catch the more subtle issues: retry timing, timeout handling, partial-failure aggregation, mock-to-real engine translation. Running both gives us confidence without excessive test time.

### 9d. Fixtures — fake search responses we need to write

For Tier 1 (unit) and Tier 2 (integration), we need representative response bodies for each supported engine. These fixtures should be written once and reused across tests. They live in a `fixtures/` module alongside the test code, or as constants at the top of the integration test file.

**Recommended fixture set (per engine):**

| Fixture | Description | Why needed |
|---|---|---|
| **Google: success with 3 results** | HTTP 200, JSON matching Google's Custom Search response shape (section 2a). Three items in the `items` array. | Tests normalization of title → `title`, link → `url`, snippet → `snippet`. Verifies the mapping from Google's field names to our canonical struct. |
| **Google: empty results** | HTTP 200, JSON with `"items": []`. | Tests that an empty response is classified as a soft error (`EmptyResults`), not a hard failure, and doesn't break merge logic. |
| **Bing v7: success with 5 results** | HTTP 200, JSON matching Bing's Web Search shape (section 2b). Five items in `webPages.value`. | Tests normalization of `name` → `title`, `url` → `url`, `snippet` → `snippet`, `dateLastCrawled` → `date`. Verifies the mapping from Bing's field names. |
| **Bing: rate-limited (429)** | HTTP 429 with a `Retry-After` header set to `"60"` and an empty or minimal body. | Tests that we detect 429 status, parse the `Retry-After` header, and surface it as `WebSearchError::RateLimited { retry_after_s: 60, engine: "bing" }`. |
| **SearXNG: success with 10 results** | HTTP 200, JSON matching SearXNG's response shape (section 2d). Ten items in the `results` array. | Tests normalization of `title`, `url`, `content` → canonical fields; verifies Unix timestamp (`timestamp`) is converted to ISO-8601 for the `date` field. |
| **SearXNG: malformed JSON** | HTTP 200, body contains invalid JSON (e.g., `{broken`). | Tests that we detect a malformed response and surface it as `WebSearchError::MalformedResponse { status: 200, bytes_read }`. |

**How to write fixtures:** Each fixture is a raw JSON string stored as a constant. A helper function deserializes the fixture into the engine-specific intermediate struct (or directly into the raw fields needed for normalization). Example pattern:

```rust
const GOOGLE_RESPONSE_3_RESULTS: &str = r#"{
  "kind": "customsearch#res",
  "items": [
    { "title": "Example Page One", "link": "https://example.com/one", "snippet": "First result snippet" },
    { "title": "Example Page Two", "link": "https://example.com/two", "snippet": "Second result snippet" },
    { "title": "Example Page Three", "link": "https://example.com/three", "snippet": "Third result snippet" }
  ]
}"#;

fn parse_google_response(body: &str) -> GoogleResponse {
    serde_json::from_str(body).expect("Fixture is valid JSON")
}
```

For Tier 2 (integration), we don't need separate fixtures — the mockito server's `with_body(...)` call provides the response directly. But it's still useful to have a canonical "expected response" string as documentation for what each engine should return, so tests can assert on both the *structure* and the *content* of results.

### 9e. Example: integration test with mockito — full pattern

Here is how a Tier 2 integration test would look for `web_search` using mockito. This pattern applies to every engine + scenario combination we want to verify:

```rust
#[tokio::test]
async fn web_search_bing_success_google_timeout() {
    // Arrange: spin up two mock servers, one per engine.
    let mut google = Server::new_async().await;
    let mut bing = Server::new_async().await;

    // Google's mock: return 200 with a fake response body after 6 seconds (simulates slow/unresponsive engine).
    let _google_mock = google.mock("GET", "/customsearch")
        .match_query(mockito::Matcher::Regex(r"q=.*"))
        .with_body(GOOGLE_RESPONSE_3_RESULTS)
        .create_async()
        .await;

    // Bing's mock: return 5 results.
    let bing_mock = bing.mock("GET", "/search")
        .match_query(mockito::Matcher::Regex(r"q=.*"))
        .with_body(BING_RESPONSE_5_RESULTS)
        .create_async()
        .await;

    // Act: run the web_search tool with both engines mocked.
    let args = WebSearchArgs {
        query: "tokio async runtime".to_string(),
        max_results: 10,
        search_type: "general".to_string(),
        language: None,
        region: None,
    };

    // We'd need a way to inject the mock URLs into the engine config.
    // In practice this means building Engine instances that point at the mock servers.
    let result = web_search::search(args.clone(), engines_with_mocks(&google.url(), &bing.url())).await;

    // Assert: Bing's 5 results are returned, deduplicated (no duplicates since Google didn't return anything within timeout).
    assert_eq!(result.len(), 5);
    assert_eq!(result[0].title, "Bing Result One"); // or whatever the first result is
}
```

**Key points about this pattern:**

1. **Two servers per test.** Each engine gets its own mockito server. This isolates engines — if one mock fails to start, it doesn't affect the other. The pool-based port assignment means we don't need to worry about collisions.
2. **`match_query` with a regex for `q=`.** Every search call includes the query in the URL's query string (`?q=...`). Using a regex matcher on the query parameter lets us accept any test query without hardcoding it — the mock responds regardless of what `q=` is set to.
3. **Timeout simulation via backoff, not mock body.** To simulate Google timing out, we don't need a separate "timeout" fixture. Instead, we can either: (a) configure the mock with a long response body and rely on our per-engine timeout (5 s) to abort it naturally; or (b) use `tokio::time::sleep` before returning the response in an async handler registered with mockito's custom server. Option (a) is simpler — just set up the mock to respond after 6+ seconds, and let the per-engine timeout handle the rest.
4. **Engine injection.** The hardest part of this pattern: how do we tell `web_search`'s engine configuration that Google should talk to `google.url()` instead of `https://www.googleapis.com/customsearch/v1`? This is a test-only concern — in production, engines point at real URLs. We'd need either:
   - A `#[cfg(test)]` module that overrides the engine builder with mock-aware versions, or
   - An environment variable / feature flag that swaps the base URL at construction time.

   Option (a) is cleaner for a small codebase; option (b) is more robust if we ever add engines beyond Google/Bing/SearXNG. The research doc's section 10 ("Open Questions") flags this as a decision — picking one approach early will simplify the test setup.

### 9f. Example: unit test with reqwest mock middleware — no server needed

For Tier 1 (pure logic), we can skip the external dependency entirely by using reqwest's built-in mock feature. This is fast, requires no port management, and works even if our `fetchkit` integration doesn't support pluggable transports:

```rust
#[test]
fn normalize_google_response_parses_correctly() {
    // Build a reqwest Client with mock middleware that returns a fake Google response.
    let client = reqwest::ClientBuilder::new()
        .mock("GET", "/customsearch")
            .with_status(200)
            .with_body(GOOGLE_RESPONSE_3_RESULTS)
            .create();

    // Call our normalization function directly — no network I/O.
    let response = parse_google_response(GOOGLE_RESPONSE_3_RESULTS);
    let results = normalize_results(response, SearchEngine::Google, 10).unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].title, "Example Page One");
    assert_eq!(results[0].url, "https://example.com/one");
}
```

**Why this works without a real HTTP call:** reqwest's mock middleware intercepts the request at the client layer and returns the configured body immediately — no TCP connection is established. This means `normalize_results()` runs in pure CPU time; it never touches the network stack. Perfect for fast unit tests.

**Caveat:** We can only use this approach if our normalization code takes a deserialized response struct (not raw HTTP bytes). If we need to test the *parsing* layer too, we'd still need mockito or wiremock — but that's rare; most parsing bugs are caught by simple JSON round-trip tests.

### 9g. Summary of recommendations

| Concern | Recommendation | Rationale |
|---|---|---|
| **Primary mocking crate** | `mockito` (in dev-dependencies) | Async support, small API, works with any HTTP client, mature ecosystem. |
| **Secondary for unit tests** | reqwest's `.mock()` middleware | Zero external dependency; fast; good enough when we only need to test response parsing/normalization logic. |
| **Test tiers** | Unit (parse + normalize) + Integration (full engine path with mockito). | Tier 1 catches the most common regressions quickly; Tier 2 verifies end-to-end behavior including retries, timeouts, and partial failures. |
| **Fixture strategy** | One raw JSON string per engine × scenario (success, empty, error). Store as `const &str` constants near the test module. | Reusable across unit and integration tests; serves as documentation for expected engine response shapes. |
| **Engine injection in tests** | Resolve via a `#[cfg(test)]` override or env var that swaps base URLs at construction time. (See section 10.) | Production code stays untouched; test-only concerns are isolated behind cfg gates so they don't leak into the public API. |