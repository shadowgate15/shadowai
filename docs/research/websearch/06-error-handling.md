# WebSearch Tool — Error Handling & Failure Modes

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

---

## 6. Error Handling & Failure Modes

**Prompt:** Study `FetchError` from fetchkit used in web_fetch, then research:

- Network timeout handling — should it be retryable? With what backoff?
- Rate-limit responses (HTTP 429) — how to surface them to the agent?
- Empty results — is that a hard error or a soft one?
- Partial engine failures when using multiple engines in parallel

Propose an `Error` type for websearch and retry strategy.

**Why:** Robust failure modes keep the tool usable even when individual engines are down.

### 6a. FetchError from fetchkit (used by web_fetch)

`web_fetch` delegates entirely to fetchkit, which defines its own error enum:

```rust
pub enum FetchError {
    MissingUrl,
    InvalidUrlScheme,
    InvalidMethod,
    BlockedUrl,
    ClientBuildError(reqwest::Error),
    FirstByteTimeout,
    ConnectError(reqwest::Error),
    RequestError(String),
    FetcherError(String),
    SaveError(String),
    SaverNotAvailable,
    RenderNotAvailable,
}
```

Key variants relevant to search:

| Variant | Meaning | Retryable? | Notes |
|---|---|---|---|
| `FirstByteTimeout` | Request timed out waiting for first byte | Yes — transient network issue | Requires backoff; retrying immediately hits the same wall. |
| `ConnectError(reqwest::Error)` | Failed to connect to server | Mostly yes — DNS or TCP handshake failure is usually transient | Retry with exponential backoff; if it persists, likely a permanent outage. |
| `ClientBuildError(reqwest::Error)` | HTTP client construction failed | No — infrastructure-level problem (TLS config, etc.) | Not retryable per call; indicates the tool's own setup is broken. |
| `BlockedUrl` | URL blocked by policy/prefix list | No — configuration issue | Retry would just hit the same block. Treat as a hard error to surface to the user. |

For web_fetch (single-engine, single-purpose), these variants make sense: every error is fatal because there's no fallback path. For websearch (multi-engine with redundancy), we need finer-grained control — retryable vs. non-retryable must be explicit per variant, and some failures should degrade gracefully rather than aborting the whole call.

### 6b. Proposing a custom `WebSearchError` for websearch

Rather than wrapping fetchkit's `FetchError`, we define our own error type using `thiserror::Error`, following the same pattern as `shell.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum WebSearchError {
    #[error("Timeout waiting for response: {0}")]
    Timeout(String),

    #[error("Failed to connect to search engine: {0}")]
    ConnectFailure(String),

    #[error("Rate limit exceeded (HTTP 429): retry after {retry_after_s}s. Engine: {engine}")]
    RateLimited { retry_after_s: u64, engine: String },

    #[error("Search engine returned no results for query `{query}`")]
    EmptyResults { query: String },

    #[error("Invalid search parameters: {0}")]
    InvalidArguments(String),

    #[error("Engine configuration error: {0}")]
    ConfigError(String),

    #[error("Unexpected response from engine: status {status}, body truncated at {bytes_read} bytes")]
    MalformedResponse { status: u16, bytes_read: usize },
}
```

**Why a custom enum instead of reusing `FetchError`:**

| Aspect | fetchkit's `FetchError` (used by web_fetch) | Custom `WebSearchError` (proposed) |
|---|---|---|
| **Retryability semantics** | Implicit — caller must decide per variant | Explicit via the enum shape: timeout/connect are retryable, rate-limit is retriable with backoff, empty/config/malformed are not. The agent can see exactly what to do next. |
| **Engine identity** | No engine tag | `RateLimited` and `ConnectFailure` carry an `engine` field — when firing multiple engines in parallel (section 7), we know which one failed and can retry only that one or fall back to another. |
| **Empty results** | Not applicable (web_fetch always returns content) | First-class soft error: a query returned zero hits, but the tool succeeded otherwise. The agent sees "no results" without confusing it with a hard failure. |
| **Malformed responses** | Caught as `RequestError(String)` — opaque message | Structured variant with status code + byte count — gives the agent actionable info ("Bing returned 502, body truncated at 14 bytes"). |
| **Config errors** | `ClientBuildError` is a generic reqwest error wrapped in fetchkit | Explicit config error — surfaces auth/key problems without hiding them behind opaque strings. |

### 6c. Network timeout handling — retryable with exponential backoff

Timeouts are the most common transient failure: DNS resolution stalls, TCP handshake takes too long, or an engine's server is overloaded. All three should be retried.

**Backoff strategy:** exponential with jitter, capped at a maximum delay. This avoids thundering-herd patterns when many agents fire simultaneously.

| Parameter | Value | Rationale |
|---|---|---|
| **Initial delay** | 100 ms | Fast enough that the agent doesn't feel a noticeable pause on the first retry; cheap to wait if the engine is just momentarily slow. |
| **Multiplier** | 2× per attempt | Classic exponential backoff; keeps total retry time bounded even across many attempts. |
| **Jitter** | ±30% (uniform random) | Prevents all agents from hitting the same retry schedule at once. A single engine recovering takes seconds, not minutes. |
| **Max delay** | 8 s | Hard cap to avoid indefinite hangs. After this, we give up and return a timeout error. |
| **Max retries** | 3 per engine | Enough attempts for transient flakiness; beyond that, the engine is likely down or rate-limiting us hard. |

**Implementation sketch:**

```rust
async fn retry_with_backoff(
    mut attempt: u32,
    delay_ms: Duration,
) -> Result<T> {
    loop {
        match inner_call().await {
            Ok(v) => return Ok(v),
            Err(WebSearchError::Timeout(_)) | Err(WebSearchError::ConnectFailure(_) ) => {
                if attempt < MAX_RETRIES {
                    let jitter = (random_u32() % 1_000u32) as f64 / 10.0; // 0–1000 ms
                    let backoff = delay_ms * 2 + Duration::from_millis(jitter as u64);
                    tracing::warn!("Retrying engine after {backoff:?}");
                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                } else {
                    return Err(WebSearchError::Timeout("Max retries exceeded".to_string()));
                }
            },
            other => return Err(other), // non-retryable — propagate immediately
        }
    }
}
```

**When NOT to retry:** rate-limit responses (HTTP 429) are retried but with a longer backoff (see next subsection). Configuration errors, malformed responses, and empty results are not retried — they indicate something fundamentally wrong that retries won't fix.

### 6d. Rate-limit responses (HTTP 429) — surfaced explicitly to the agent

When an engine returns HTTP 429, it includes a `Retry-After` header indicating how long to wait before retrying. We should honor this:

| Field | Source | Value |
|---|---|---|
| `retry_after_s` | Parsed from response's `Retry-After` header (seconds) or default 60 s | How long the agent should wait before trying again with a different engine. |
| `engine` | Tagged on the error variant | Which engine hit the limit — useful for choosing an alternative in parallel execution. |

**Behavior:** On rate-limit, we do NOT retry the same engine. Instead:

1. Surface the error to the agent immediately (with the suggested wait time).
2. If this is a multi-engine call (section 7), skip the rate-limited engine and use the remaining ones. The other engines are unaffected — they returned results normally.
3. If all engines are rate-limited, return the first one's error as the tool-level failure.

**Why not just retry with backoff:** A rate-limit is a policy decision by the API provider. Retrying after the suggested wait wastes quota and risks further throttling. Honoring `Retry-After` respects the provider's guidance and keeps us within their limits.

### 6e. Empty results — soft error, not hard failure

When an engine returns zero results for a query (status 200 but empty result set), we treat this as a **soft** error:

| Scenario | Classification | Reasoning |
|---|---|---|
| Engine returns `[]` with HTTP 200 | Soft — `EmptyResults { query }` | The engine processed the request successfully; it just found nothing. Other engines may have results for the same query. |
| Engine returns `[]` after a timeout | Hard — `Timeout` | The engine didn't respond properly; empty result is a symptom of failure to process. |
| All engines return empty | Hard — propagate as soft error with note | We've exhausted all paths and genuinely found nothing. Surface this clearly so the agent knows no other approach will help. |

**Why not treat empty as hard:** A search query legitimately might have zero results (e.g., a very niche topic, misspelled term). The agent needs to know "nothing was found" vs. "something went wrong." Mixing them up causes the agent to retry on queries that are genuinely answerless, burning quota and wasting time.

**Agent-facing message:** When all engines return empty, we surface it as a soft error with a clear message: *"No search results were returned across any engine for query `{query}`."* This tells the agent it can either try rephrasing or accept that the information isn't available via web search.

### 6f. Partial engine failures in parallel execution

When we fire multiple engines concurrently (section 7), some may fail while others succeed. The tool should:

1. **Collect results from all successful engines.** If Bing returns results and SearXNG fails, we still have Bing's data to return.
2. **Deduplicate across engines** before merging into the final result set (section 5).
3. **Report partial failures only if they affect the outcome.** If at least one engine succeeded with usable results, partial failures are internal details — not worth surfacing to the agent unless they're rate-limited (which suggests a quota issue worth knowing about).

**Error aggregation strategy:**

| Outcome | Surface? | Message |
|---|---|---|
| All engines succeed | No error | Normal response with merged results. |
| Some fail, at least one succeeds | No error to agent | Internal handling only; log for observability. |
| All fail except rate-limited ones | Soft error + note | "Engine X hit rate limit (retry after Y s), using remaining engines." |
| All fail hard (timeout/connect/config) | Hard error | Aggregate all failures into one message: "All search engines are currently unavailable." |

**Why this matters:** Without partial-failure handling, a single flaky engine can take down the whole tool. With it, the agent gets results from whatever engines work at that moment — dramatically improving availability. The trade-off is slightly more complex error aggregation logic, but it's small and worth it for resilience.

### 6g. Retry strategy summary

| Error type | Retry? | Backoff | Max attempts | Notes |
|---|---|---|---|---|
| `Timeout` (network) | Yes | Exponential: 100ms → 2×, capped at 8s + jitter | 3 per engine | Transient network issue; retry with backoff. |
| `ConnectFailure` | Yes | Same as timeout | 3 per engine | DNS/TCP handshake failure; usually transient. |
| `RateLimited` (429) | Yes, but different strategy | Honor `Retry-After` header or default 60s; no retries on same engine | N/A — skip to next engine | Policy-driven wait; don't retry immediately. |
| `EmptyResults` | No (soft error) | None | N/A | Already returned successfully with empty set. |
| `InvalidArguments` | No | None | N/A | Wrong input; retrying won't fix it. |
| `ConfigError` | No | None | N/A | Infrastructure problem; needs config change. |
| `MalformedResponse` | No | None | N/A | Bad payload; engine is broken or returning unexpected data. |

**Implementation outline:** Per-engine calls go through a retry loop for timeout/connect failures (section 6c). Rate limits are detected and converted to a rate-limit error with the suggested wait time. Empty results, config errors, malformed responses, and invalid arguments propagate immediately without retrying. When multiple engines run in parallel, successful ones contribute to the result set; failed ones are either retried internally or reported as partial failures (section 6f).