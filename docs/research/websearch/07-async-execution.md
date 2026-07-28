# WebSearch Tool — Async & Parallel Execution

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

---

## 7. Async & Parallel Execution

**Prompt:** Research:

- Should we fire all available engines concurrently and merge results, or pick one engine per call?
- How to handle async timeouts per-engine without blocking others?
- What does `tokio` concurrency look like here (tasks, select, etc.)?

**Why:** Parallel execution gives the agent more diverse results but adds complexity. Must decide trade-off up front.

---

### 7a. Decision: fire all engines concurrently and merge

**Verdict:** Fire every available engine in parallel on a single call, then merge + deduplicate the combined results. Do not pick one engine per call unless explicitly configured (e.g., `mode = "single"`).

**Reasoning:**

| Factor | Single-engine-per-call | All-engines-parallel |
|---|---|---|
| **Result diversity** | Limited to whatever that engine returns; different engines may rank pages differently, so the agent gets a narrower view. | Diverse ranking signals from multiple sources — the agent sees results ranked by Google AND Bing AND SearXNG simultaneously. |
| **Latency** | Lower per-engine (no merge overhead), but total wall-clock time equals the longest engine if you chain them sequentially. | Higher merge overhead (~microseconds) but wall-clock time ≈ max(engine latencies), which is often less than sequential sum for engines with similar response times (Google ~200ms, Bing ~300ms, SearXNG ~150ms). |
| **Resilience** | If the primary engine fails, the tool breaks. Caller must retry or fall back manually. | Partial failures are internal: if Bing times out but Google and SearXNG succeed, we still return results. Only when *all* engines fail do we surface an error (section 6f). |
| **Cost** | Lower per call — fewer API requests. But sequential calls compound cost over retries. | Higher per call — one request per engine × N engines. Offsetting: caching (section 5) amortizes repeated queries, and the richer result set reduces downstream agent retries. |

The trade-off that tips the balance is resilience: a single-engine-per-call design means every caller must handle fallback logic themselves. Parallel execution hides that complexity behind `web_search`'s own error handling — the tool succeeds unless *every* engine fails hard. That's a much better default for an agent-facing API.

**Caveat:** If cost is the dominant concern (e.g., Google free tier at ~100 queries/day), callers can opt into single-engine mode by passing `mode: "single"` and picking their preferred engine explicitly. This is an opt-in feature, not the default — keeping the happy path simple while allowing power users to reduce quota burnage when they know which engine works best for a given query type.

### 7b. tokio concurrency primitives in use

The implementation lives in `call(&self, args)` and looks like this:

```rust
pub async fn call(
    &self,
    args: Self::Args,
) -> Result<Self::Output, Self::Error> {
    let engines = self.engines(); // e.g., [google, bing, searxng]
    let query = args.query.clone();
    let max_results = args.max_results;

    // Fire each engine as a separate spawned task.
    let handles: Vec<_> = engines.iter().map(|engine| {
        tokio::spawn(async move {
            Self::run_engine(engine, &query, max_results).await
        })
    }).collect();

    // Wait for all tasks to complete (with per-task timeout).
    let results: Vec<Option<WebSearchResult>> = try_join_all(handles)
        .await?;

    // Filter out engines that errored or timed out.
    let successful: Vec<_> = results.into_iter()
        .flatten()
        .collect();

    if successful.is_empty() {
        return Err(WebSearchError::EmptyResults { query });
    }

    // Merge + deduplicate across engine boundaries (section 5).
    Ok(Self::merge_and_dedup(successful))
}
```

**Key primitives:**

| Primitive | Used for? | Notes |
|---|---|---|
| `tokio::spawn` | Fire each engine as an independent async task. Each runs to completion or timeout regardless of what others are doing. | Tasks run on the runtime's thread pool (default: OS threads). No blocking — each engine call returns a future that resolves when its HTTP request completes. |
| `try_join_all` | Wait for all spawned tasks and propagate any error. If *any* task panics or returns an Err, the whole call fails. | We use this deliberately: if one engine crashes (e.g., malformed response), we want to know immediately rather than silently returning partial results. For rate-limited engines specifically (section 6d), we handle that as a soft error and skip it instead — see next subsection. |
| `tokio::time::timeout` | Per-engine deadline: each engine gets a maximum wall-clock budget (e.g., 5 s). If an engine exceeds this, its task returns `Err(WebSearchError::Timeout)` rather than blocking the runtime forever. | Timeout is applied *inside* each spawned task, not at the join level. This means one slow engine can't extend the total call time — all engines have independent deadlines. |
| `tokio::select!` | Not used in this path. Considered but rejected: `select!` would pick whichever engine finishes first and discard the rest. That defeats diversity — we want *all* results, not just the fastest one. | Would be useful if callers wanted "first-result-only" mode (e.g., streaming UX), but that's a different semantic than what an agent typically needs. Reserved for future opt-in feature. |

**Why `try_join_all` over manual `.await` on handles:**

The alternative is awaiting each handle individually in a loop:

```rust
let mut results = Vec::new();
for handle in handles {
    match handle.await? { // blocks until that task resolves
        Ok(engine_results) => results.push(engine_results),
        Err(e) if e.is_timeout() => continue, // skip timed-out engines
        Err(other) => return Err(other)?,     // propagate hard errors immediately
    }
}
```

This works but is verbose and error-prone: you have to handle three cases (success, timeout-skip, hard-error-propagate) per iteration. `try_join_all` collapses that into one line — the runtime handles the loop internally. The only downside is it treats all errors uniformly at the join level, so we need the inner task to distinguish "timeout" from "hard error" before returning (section 7c).

### 7c. Per-engine timeout handling

Each spawned engine task runs inside its own `tokio::time::timeout` guard:

```rust
async fn run_engine(
    engine: &Engine,
    query: &str,
    max_results: u32,
) -> Result<Vec<WebSearchResult>, WebSearchError> {
    let deadline = tokio::time::Instant::now() + ENGINE_TIMEOUT;

    // Build the HTTP request for this engine.
    let response_body = tokio::time::timeout(deadline, fetch_engine(engine, query))
        .await
        .map_err(|_| WebSearchError::Timeout(format!("Engine {} timed out", engine.id())))?;

    // Parse + normalize results (section 4).
    Ok(Self::normalize_results(response_body, engine.id(), max_results)?)
}
```

**Why per-engine timeout instead of a single call-level timeout:**

| Approach | Trade-off |
|---|---|
| **Single call-level timeout wrapping the whole join** | If one engine is slow (e.g., Bing DNS resolution stalls), all other engines are forced to wait until the deadline expires. Total latency = max(deadline, longest-engine-time). Poor UX: a 5 s timeout with a 30 s-bogus engine means callers get no results for 30 s. |
| **Per-engine timeout (this approach)** | Each engine has its own independent budget. A slow engine fails fast internally and returns `Timeout` — the other engines keep running to completion. Total latency ≈ max(all-engines' wall-clock times), which is typically much less than the call-level deadline. |

**Timeout value:** 5 seconds per engine, chosen as a balance between "fast enough for the agent" (agent turns are ~100–200 ms budget) and "long enough to handle network hiccups." If an engine genuinely takes > 5 s, it's likely stuck on DNS or a slow proxy — retrying won't help.

**Error conversion inside timeout:** When `tokio::time::timeout` fires, the inner future is aborted. We convert that into `WebSearchError::Timeout(engine_id)` so callers can distinguish "engine timed out" from other errors and skip it during merge (section 7b).

### 7d. Merge & deduplication pattern

After all engines return, we collect successful results and run a two-pass dedup:

```rust
fn merge_and_dedup(results: Vec<Vec<WebSearchResult>>) -> Vec<WebSearchResult> {
    // Flatten + URL-dedup (O(n) with HashSet).
    let mut url_set = HashSet::new();
    let mut merged = Vec::with_capacity(results.iter().flatten().sum_by(|r| r.len()));

    for engine_results in results.into_iter().flatten() {
        for result in engine_results {
            if !url_set.insert(result.url.clone()) {
                continue; // duplicate URL — skip silently
            }
            merged.push(result);
        }
    }

    // Sort by relevance score (descending) so the highest-quality results come first.
    merged.sort_by(|a, b| b.relevance_score.cmp(&a.relevance_score).then_with(|| a.url.cmp(&b.url)));
    merged
}
```

**Why URL-only dedup instead of domain+snippet similarity:** Section 5c already covers the more expensive domain+snippet approach. For the common case, exact URL matches are sufficient — most engines return different URLs for the same page (Google: `https://example.com/article`, Bing: `https://www.example.com/article`). The rare near-duplicate case (same page from Google + SearXNG) is a small fraction of total results and can be handled later if needed. Two-pass dedup keeps the fast path simple while leaving room for enhancement without breaking existing behavior.

### 7e. Trade-offs & when to consider single-engine mode

**When parallel execution wins:**
- Agent needs broad coverage (general web search)
- Multiple engines are configured and healthy
- Result diversity matters more than minimal latency/cost

**When single-engine-per-call might be better:**
- Cost-sensitive callers on free-tier APIs (Google ~100/day limit makes parallel calls expensive if multiple engines hit their quota in the same session)
- Known-good engine for a specific query type (e.g., SearXNG for news queries, Bing for general web search) — opt-in via `mode: "single"` and explicit engine selection
- Latency-critical paths where every millisecond counts (streaming UX, real-time dashboards)

**Implementation note:** Single-engine mode is a separate code path that uses the same per-engine timeout + retry logic but skips spawning multiple tasks. It's not merged into the parallel path — it's a distinct `call` variant selected by runtime config. This keeps both paths simple and avoids conditional branches in the hot merge/dedup code.