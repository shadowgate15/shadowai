# WebSearch Tool — Decisions Resolved & Open Questions

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

---

## 10. Decisions Resolved — What Blocks Implementation

**Prompt:** List every design decision that blocks implementation and record the chosen direction so future sessions can resolve remaining questions individually without re-deriving them:

- Which engine is primary vs. fallback?
- Single-engine-per-call vs. multi-engine parallel?
- Caching: yes/no, and if yes, what TTL?
- Should we support `search_type` (news/images) or keep it general-only?
- Any auth model (API key env var vs. built-in)?

**Why:** Future sessions can resolve these individually; the doc stays actionable.

---

### 10a. Primary engine: DuckDuckGo Instant Answer API, fallback SearXNG

| Decision | Value | Rationale |
|---|---|---|
| **Primary** | DuckDuckGo (`api.duckduckgo.com/?q=query&format=jsonf&no_html=1`) | Zero auth required; no subscription or API key. Free tier empirically returns ~500 results/hour (exact threshold undocumented, varies by query). Schema is inconsistent across queries but we normalize to the canonical `WebSearchResult` struct anyway — so schema variance doesn't block use. |
| **Fallback** | SearXNG public instance (`searx.be`, `search.sapti.net`, etc.) | Also free, no auth, aggregates Google/Bing/DuckDuckGo upstream. Multi-engine in one call means richer results at zero marginal cost per query. Instance health is the only risk — pick from [searx.space](https://searx.space) a stable instance with good uptime history; check `robots.txt` for rate limits. |
| **Premium (opt-in)** | Google Custom Search / Bing v7 | Require API key/subscription (`GOOGLE_API_KEY`, `BING_SUBSCRIPTION_KEY` env vars). Add later when budget allows — they're not blockers for an MVP tool. Google's free tier (~100/day) is too low for a primary; Bing has no free tier at all (v6 deprecated, v7 paid-only). |

**Why DuckDuckGo first over SearXNG:** Both are free and unauthenticated, but DuckDuckGo is a single well-known endpoint with no instance-management overhead. SearXNG's advantage (multi-engine aggregation) matters more once we have premium engines to aggregate — for an MVP with only one primary engine, the simpler setup wins. We can swap in SearXNG as fallback immediately; swapping the primary to SearXNG later is a config change, not a code rewrite.

**Why no auth env vars at this stage:** Adding `GOOGLE_API_KEY` or `BING_SUBSCRIPTION_KEY` env var support today means we need an engine-config layer that reads from environment variables and validates them at tool init time. That's non-trivial for the first implementation. DuckDuckGo + SearXNG work with zero config — ship a working tool, then add env-var auth as a feature in a follow-up iteration.

---

### 10b. Multi-engine parallel execution (not single-engine-per-call)

| Decision | Value | Rationale |
|---|---|---|
| **Execution model** | Fire all available engines concurrently; merge + dedup results at the call boundary. | Section 7a covers this in depth — parallel wins on resilience, result diversity, and wall-clock latency (≈ max(engine times) vs sum for sequential). A single-engine-per-call design forces callers to handle fallback logic themselves; we hide that behind `web_search`'s own error handling. |
| **Per-engine timeout** | 5 s per engine, independent deadlines via `tokio::time::timeout`. | Section 7c covers this — one slow engine can't extend the total call time — each has its own budget. Timeout converts to `WebSearchError::Timeout(engine_id)` so callers skip it during merge (section 7b). |
| **Merge strategy** | Two-pass: URL-dedup via `HashSet` (O(n)), then sort by relevance score. | Section 7d covers this — exact URL matches catch duplicates cheaply; near-duplicates (same page, different URL) are rare enough for the MVP. Domain+snippet similarity can be added later if needed. |
| **Opt-in single-engine mode** | `mode: "single"` + explicit engine selection via `engine` parameter. | Reserved for cost-sensitive callers on free-tier APIs or latency-critical paths. Separate code path, not merged into parallel logic. (See section 7e.) |

---

### 10c. Caching: yes — in-memory `HashMap` with TTL, no external dependency today

| Decision | Value | Rationale |
|---|---|---|
| **Cache** | Yes — store cached responses in `std::collections::HashMap<String, CachedEntry>` keyed on normalized query string. | Section 5b covers this — zero dependencies, simple, fast enough for single-process agent sessions (~10k entries < 1 MB). Eviction: sort by age and remove oldest when map exceeds threshold (e.g., 10_000 entries). No persistence across restarts — the session ends when the process exits anyway. |
| **TTL** | Default 120 s (± configurable range 60–300 s). | Section 5b covers this — sweet spot: results fresh enough that snippets still reflect current pages, but we amortize API costs over repeated calls within a session. |
| **Upgrade path** | `cached` crate (`LruTtlCache`, `ShardedLruTtlCache`) if we need sharded concurrency or persistence across restarts. | 39M+ downloads on crates.io; async support via macros; TTL + LRU eviction built in. Opt-in upgrade, not a blocker for MVP. |
| **Storage primitive** | In-memory map only — no Redis/SQLite/file-backed cache. | Section 5d covers this — file-backed adds latency and complexity overkill for an agent tool that lives in one process; Redis requires infrastructure we don't need yet. |

---

### 10d. Scope: general-only for MVP, `search_type` opt-in later

| Decision | Value | Rationale |
|---|---|---|
| **Scope** | General web search only (`search_type: "general"` by default). No news or images support today. | Section 3b/3c covers this — the Args schema already has `search_type` as a field with enum values, but engines handle scope differently (Bing supports general/news via params; DuckDuckGo/SearXNG treat it implicitly). Supporting all scopes upfront means per-engine logic branches that complicate normalization. |
| **Future** | Add `news`, `images`, and potentially `videos` as separate `search_type` enum values once we have engines that natively support them (e.g., Bing's answer-type filtering for news/images). | No engine currently offers a clean, consistent API for image search — DuckDuckGo's image endpoint is undocumented, SearXNG's categories param exists but schema varies. Adding it today would be speculative; better to add it when we have real engine support verified by tests. |
| **Why not general now:** | The agent's most common mistake (section 8b) is treating search and fetch interchangeably. A single-scope tool keeps the mental model simple: "web_search = current info about X." Adding scopes doesn't change that — it just adds complexity for a use case the agent rarely needs in its first iteration. |

---

### 10e. Auth model: zero-config today, env-var auth as follow-up feature

| Decision | Value | Rationale |
|---|---|---|
| **Today** | No auth required — DuckDuckGo and SearXNG are both unauthenticated. Tool initializes with no config; works out of the box. | Section 10a covers this — adding API key env vars today means engine-config layer + validation at init time, which is non-trivial for an MVP. Zero-config wins for first implementation. |
| **Follow-up** | `GOOGLE_API_KEY` env var for Google Custom Search; `BING_SUBSCRIPTION_KEY` env var for Bing v7. Stored in tool config (not hardcoded). | Premium engines require auth, but they're opt-in additions after the MVP is proven. Env vars let users configure them without code changes — standard pattern across Rust agent tooling (see how `shell.rs` and other tools handle optional env-based config). |
| **Why not built-in auth today:** | Built-in auth means we need to validate keys at init time, handle key rotation, expiry detection, and surface auth errors distinctly from engine errors. That's real engineering for no benefit — DuckDuckGo + SearXNG work perfectly without any of that. | The `FetchError` enum (section 6a) already has a `ConfigError(String)` variant for cases where infrastructure is misconfigured; we can reuse it if/when env-var auth is added later. |

---

## Summary of Resolved Decisions

| Question | Decision | Trade-off acknowledged |
|---|---|---|
| **Primary engine** | DuckDuckGo Instant Answer API (free, no auth) | Schema varies by query; we normalize anyway so it doesn't matter for the canonical output. |
| **Fallback engine** | SearXNG public instance from searx.space (free, multi-engine aggregation) | Instance health risk — pick a stable one with good uptime history; check `robots.txt` for rate limits. |
| **Execution model** | Multi-engine parallel execution (fire all, merge + dedup) | Higher per-call cost; amortized by caching and richer results reducing downstream retries. |
| **Opt-in single mode** | `mode: "single"` for cost-sensitive or latency-critical callers | Separate code path; not merged into parallel logic. |
| **Caching** | In-memory `HashMap` + TTL (default 120 s); upgrade to `cached` crate later if needed | No persistence across restarts — acceptable for single-process agent sessions. |
| **Scope** | General web search only (`search_type: "general"` default) | Adding news/images later requires verified engine support; don't add speculative features today. |
| **Auth model** | Zero-config today; env-var auth (`GOOGLE_API_KEY`, `BING_SUBSCRIPTION_KEY`) as follow-up feature | Premium engines are opt-in additions after MVP is proven. No benefit to building auth infrastructure for zero-cost APIs. |

---

## Remaining Open Questions (for future sessions)

These are explicitly left unresolved — they can be addressed individually without re-deriving the above:

1. **Single-engine-per-call mode** (`mode: "single"`) — detailed implementation in section 7e; needs a follow-up session to wire up engine selection and config plumbing.
2. **Premium engine integration** (Google Custom Search / Bing v7) — requires auth env vars, engine-config layer, and response normalization for their specific schemas. Blocked by sections 10a and 10e until we decide which premium engine(s) to add first.
3. **Search scope expansion** (`news`, `images`, `videos`) — needs per-engine support verification before adding enum values; see section 10d for the rationale against adding today.
4. **Domain+snippet deduplication** (section 5c, two-pass approach) — URL-dedup is sufficient for MVP; near-duplicate detection can be added later if result counts prove too high.
5. **Cache persistence across restarts** — file-backed or Redis cache could survive process exits; see section 5d for why this isn't built today.
6. **SearXNG instance selection logic** — picking a stable public instance from searx.space needs automated health-checking (uptime tracking, failover); currently manual curation.

These can be resolved individually in future sessions without re-deriving the five decisions above; the doc stays actionable because each section is self-contained and references only what it needs.