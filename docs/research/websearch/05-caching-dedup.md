# WebSearch Tool — Caching & Deduplication

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

---

## 5. Caching & Deduplication

**Prompt:** Research:

- Should websearch cache queries? If so, what TTL makes sense for search results?
- How to detect and deduplicate results across multiple engines (same domain, similar snippet)?
- What storage primitive fits (in-memory map vs. file-backed cache)?

**Why:** Caching reduces API costs and call volume; deduplication keeps result counts honest.

### 5a. Should we cache?

**Yes.** Web search APIs are the most expensive operation in this tool: each call consumes quota, may hit rate limits (Google's free tier is ~100 queries/day), and adds latency to every agent turn. Within a single session, the same query will almost always be reissued — either because the agent retries after an error or because it asks for the "same" question in slightly different wording. A cache turns those repeats into near-free lookups.

### 5b. TTL recommendation

Search results are inherently transient: a page's ranking and snippet can shift between calls. The right TTL balances freshness against cost savings:

| TTL range | Trade-off |
|---|---|
| **60–300 seconds** (default: 120 s) | Sweet spot for most queries — results are fresh enough that snippets still reflect the current page, but we amortize API costs over many repeated calls. |
| **< 60 s** | Over-aggressive; cache hits save money but stale snippets may mislead the agent (e.g., a news result from yesterday). |
| **> 5 min** | Under-aggressive; defeats the purpose of caching — we're burning API quota on queries that are likely to change meaning. |

The `cached` crate (`LruTtlCache`, `ShardedLruTtlCache`) provides exactly this TTL + LRU eviction combo, which is a known pattern in Rust agent tooling. If we adopt the crate, we get thread-safe sharded storage with both capacity limits and expiry — useful if the same query appears thousands of times in a session.

For a simpler approach (no new dependency), an `std::collections::HashMap<String, CachedEntry>` where each entry stores the normalized result + insertion timestamp works fine for single-process use: check the cache on call, serve if fresh, otherwise fetch and insert with TTL. Eviction on capacity overflow can be handled by sorting entries by age and removing the oldest when the map exceeds a threshold (e.g., 10_000 entries).

### 5c. Deduplication across engines

When we fire multiple engines in parallel (see section 7), each returns its own set of results. Some may overlap — SearXNG already tags upstream sources (`engines: ["google", "bing"]`), so a result from Google and the same result from Bing are duplicates. We should collapse them before returning to the agent.

**Strategy:** two-pass dedup on the combined result list, ordered by relevance score or engine priority:

1. **URL-based dedup.** Normalize each URL (resolve `www.` prefixes, strip trailing slashes, canonicalize query parameters where possible) and use a `HashSet<String>` keyed on the normalized URL. This catches exact duplicates cheaply — O(n) insertion + lookup.
2. **Domain + snippet similarity for near-duplicates.** If two results point to different URLs but the same domain (e.g., `example.com/article` vs. `example.com/article?utm_source=bing`), treat them as duplicates if their normalized snippets share a high character-level overlap. A simple Levenshtein distance or Jaccard similarity on lowercased, whitespace-stripped tokens works — no external dependency needed. Threshold: > 70% token overlap → same result.

For SearXNG specifically, the `engines` field already tells us which upstream engines returned each result. We can use that to skip redundant calls when we know an engine combination is available (e.g., if SearXNG is healthy and returns results from both Google and Bing, there's no need to call those two APIs separately).

### 5d. Storage primitive comparison

| Primitive | Pros | Cons | Best for |
|---|---|---|---|
| **`std::collections::HashMap` + TTL check** | Zero dependencies; simple; fast enough for single-process agent sessions (~10k entries fits in < 1 MB) | No persistence across restarts; manual eviction logic needed at capacity overflow | Default / simplest path. Pairs naturally with the tool's existing `String` output — we just store the cached response body alongside a timestamp. |
| **`cached` crate (`LruTtlCache`, `ShardedLruTtlCache`)** | Battle-tested; thread-safe sharding; built-in TTL + LRU eviction; async support via macros | Adds an external dependency (~20 lines in Cargo.toml); slightly more complex API | If we want multi-threaded safety or plan to persist cache state across processes. The crate's `time_stores` feature gives us a ready-made TTL mechanism without hand-rolling it. |
| **File-backed (SQLite / sled)** | Survives restarts; survives OOM; scales beyond memory | Adds latency on every lookup; complicates error handling (disk full, I/O errors); overkill for an agent tool that lives in one process | Not recommended unless the tool is expected to be long-lived or cluster-aware. The overhead isn't justified here — the agent session ends when the process exits anyway. |
| **Redis / network cache** | Distributed; survives restarts; shared across processes | Requires a running Redis instance; adds latency and operational complexity | Only if we're building this tool for production deployment with multiple agents sharing state. Out of scope for now. |

**Recommendation:** start with `std::collections::HashMap` + manual TTL check (section 5b). Add the `cached` crate later if we need sharded concurrency or want to persist cache state across restarts — both are opt-in upgrades, not blockers.