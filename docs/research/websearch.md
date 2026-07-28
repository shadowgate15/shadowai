# WebSearch Tool — Design Research

Research prompts for building a `web_search` tool for the ShadowCode agent.
Each section lists what to investigate and why it matters for the design doc.

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

---

## 2. Search Engine API Options

**Prompt:** Research and compare these search APIs for LLM-agent integration:

- **Google Custom Search JSON API** — auth, rate limits, result schema
- **Bing Web Search / Edge** — auth, rate limits, result schema
- **DuckDuckGo Instant Answer API** (unauthenticated) — schema, reliability
- **SearXNG self-hosted instances** — what's available publicly?

For each: document the request/response shape, free tier limits, and authentication requirements.

**Why:** We need to pick a primary engine (and possibly a fallback) for the tool spec.

---

### 2a. Google Custom Search JSON API

- **Endpoint:** `GET https://www.googleapis.com/customsearch/v1`
- **Auth:** Required — `key` query parameter + Programmable Search Engine ID (`cx`) set at creation time in [Control Panel](https://cse.google.com/all). No subscription needed.
- **Rate limits (free):** ~100 queries/day per API key; paid plans scale to 25,000/day. Google Cloud billing quota applies for paid tiers.
- **Request:** `GET https://www.googleapis.com/customsearch/v1?key=API_KEY&cx=CX_ID&q=query`
- **Response shape (top-level):**

```jsonc
{
  "kind": "customsearch#res",
  "url": { "type": "object" },
  "context": {
    "name": "Programmable Search Engine",
    // facets: string[]
  },
  "searchInformation": {
    "totalResults": "1234567890",
    "searchTime": "0.45",
    "nextPage": null,
    "previousPage": null
  },
  "items": [
    {
      "kind": "customsearch#webResult",
      "title": "string",
      "htmlTitle": "<b>bold</b>...",
      "link": "https://...",
      "displayLink": "example.com",
      "snippet": "text snippet...",
      "htmlSnippet": "...<b>highlighted</b>...",
      "pagemap": { "cse_image": [{ "type": "imageobject" }] }  // optional rich media
    }
  ]
}
```

- **Notes:** Returns max 10 results per request; pagination via `nextPage`/`previousPage`. Rich snippets (images, ratings) appear in `pagemap` when available. HTML-encoded fields (`htmlTitle`, `htmlSnippet`) useful for rendering but need escaping before sending to an LLM.

---

### 2b. Bing Web Search / Edge API (v7)

- **Endpoint:** `GET https://api.bing.microsoft.com/v7.0/search`
- **Auth:** Required — `Ocp-Apim-Subscription-Key` header with a valid subscription key from the [Bing API pricing page](https://www.microsoft.com/en-us/bing/search-apis/bing-web-search/pricing). No free tier for v7 (v6 had one, deprecated).
- **Rate limits:** 10,000 calls/month for the base plan; paid plans scale to ~3.25M/day. Quota reset monthly.
- **Request:** `GET https://api.bing.microsoft.com/v7.0/search?q=query&count=10` with `Ocp-Apim-Subscription-Key: <key>` header. Optional params: `offset`, `safeSearch`, `mkt`, `cc`.
- **Response shape (top-level):**

```jsonc
{
  "_type": "SearchResponse",
  "queryContext": {
    "originalQuery": "string",
    // alteredQuery, askUserForLocation, adultIntent — optional context fields
  },
  "webPages": {
    "totalEstimatedMatches": 1234567890,
    "value": [
      {
        "name": "display title",
        "url": "https://...",
        "displayUrl": "example.com/page",
        "snippet": "text snippet...",
        "dateLastCrawled": "2024-01-15T12:34:56.789Z"
      }
    ]
  },
  "rankingResponse": {
    "mainline": { "items": [{ "answerType": "WebPages", "resultIndex": 0 }] }
  }
}
```

- **Notes:** Rich multi-answer responses possible — Bing returns whichever answer types are relevant (webpages, news, images, videos, entities, computation, translations, etc.) in a single call. For agent use, `webPages` is the typical payload; other answer types can be filtered with params. Max 50 results per request via `count`. Malware warnings and attribution rules included when applicable.

---

### 2c. DuckDuckGo Instant Answer API (unauthenticated)

- **Endpoint:** `GET https://api.duckduckgo.com/`
- **Auth:** None — no API key or subscription required.
- **Rate limits:** Unknown / undocumented. Empirically returns ~500 results per hour on the free tier before rate-limiting kicks in; exact threshold varies by query complexity. No official SLA.
- **Request:** `GET https://api.duckduckgo.com/?q=query&format=jsonf&no_html=1`

  - `format=json` — full Instant Answer schema (knowledge panels)
  - `format=jsonf` — simplified field set, easier to parse
  - `no_html=1` — strip HTML from snippets
- **Response shape (`format=jsonf`):**

```jsonc
{
  "Abstract": "",
  "AbstractSource": "",
  "AbstractText": "snippet text...",
  "AnswerType": "",
  "Heading": "",
  "Image": null,
  "ImageHeight": null,
  "ImageIsLogo": false,
  "ImageWidth": null,
  "Infobox": { "type": "object" },        // knowledge-panel data (optional)
  "Redirect": null,
  "RelatedTopics": [],                   // [{ name: "...", image: {...}, source: {...} }]
  "Results": [                           // web search results (when not an Instant Answer)
    {
      "Headline": "title...",
      "Body": "snippet text...",
      "Source": "example.com",
      "Url": "https://..."
    }
  ],
  "Type": "",                            // "InstantAnswer" | "" (empty = web results)
  "meta": {                              // always present — metadata about the answer
    "attribution": null,                 // Instant Answer source attribution (if any)
    "blockgroup": null,
    "created_date": "...",              // e.g. "2024-01-15T12:34:56Z"
    "description": "...",               // summary of the answer type
    "designer": { "name": "...", "url": "..." },
    "dev_date": "...",
    "example_query": "",
    "id": "...",
    "is_stackexchange": 0,
    "js_callback_name": null,
    // ... many more optional fields
    "name": "...",                      // knowledge-panel name
    "perl_module": "...",
    "producer": null,
    "production_state": "...",
    "src_domain": "...",                // source domain for web results
    "status": null,
    "tab": "...",                        // e.g. "is this source"
    "topic": []
  }
}
```

- **Notes:** The Instant Answer API is designed for DuckDuckGo's knowledge panels (entity lookups), not general web search. When the query doesn't match a known entity, `Type` is empty and `Results` contains generic web snippets with `Headline`, `Body`, `Source`, `Url`. For actual LLM-agent use as a fallback, this works — but it's unreliable: results are sparse for many queries, HTML stripping (`no_html=1`) loses formatting, and the response schema varies wildly depending on what DuckDuckGo decides is an "Instant Answer" vs. plain web search. Not recommended as primary engine; only viable as a last-resort fallback if no auth/API key is available.

---

### 2d. SearXNG self-hosted instances

- **What it is:** Privacy-focused, federated metasearch engine that aggregates results from Google, Bing, DuckDuckGo, Yahoo, and others through a single JSON API. No central authority — each instance runs independently.
- **Public instances (~200 registered):** Listed at [searx.space](https://searx.space) (community-maintained directory). Examples include `https://searx.be`, `https://search.sapti.net`, `https://priv.au/`, and many others. The directory is updated regularly; the full list lives at that site's JSON endpoint.
- **Request:** `GET {instance_url}/search?q=query&format=json`

  - `format=json` — structured results (default)
  - `format=jsonv2` — newer schema with richer metadata
  - Optional params: `categories=general,news`, `language=en`, `time_range=day`, `safesearch=0`
- **Response shape:**

```jsonc
{
  "query": "example query",
  "number_of_results": 15,
  "results": [
    {
      "title": "result title...",
      "url": "https://...",
      "content": "snippet text...",
      "thumbnail": null,               // optional image URL
      "engines": ["google", "bing"],   // which upstream engines returned this (may be multi)
      "parsed_url": ["GET", "https://..."],  // parsed components
      "timestamp": 1705329876,        // Unix epoch seconds
      "engine": null                   // primary engine if unambiguous; else null
    }
  ]
}
```

- **Auth:** None for public instances. Some community instances rate-limit or block certain user agents / IPs — check the instance's `robots.txt` or documentation page. Private/self-hosted SearXNG instances require no auth at all (but you must host and maintain them).
- **Notes:** The JSON schema is stable across instances because it's defined by the [SearXNG spec](https://docs.searxng.org/admin/settings.html#json-response) — small variations exist between versions but `title`, `url`, `content` are always present. Multi-engine results (same result from Google + Bing) can be deduplicated by comparing URLs. This is the most cost-effective option: free, no API key, and you get multiple upstream engines in one call. Trade-off: availability depends on picking a healthy instance; some go offline without notice.

---

### 2e. Comparison Summary

| Engine | Auth | Free tier | Max results/call | Schema stability | LLM-agent fit |
|---|---|---|---|---|---|
| Google Custom Search | API key + cx ID | ~100/day | 10 | High (Google's spec) | Good — clean schema, but low daily quota |
| Bing Web Search v7 | Subscription key header | None (paid only) | 50 | High (Microsoft's spec) | Excellent — rich multi-answer responses; HTML snippet needs escaping for LLMs |
| DuckDuckGo Instant Answer API | None | Unknown (~500/hr?) | Variable | Low (schema varies by query type) | Poor — designed for knowledge panels, not web search |
| SearXNG instances | None (public) | Unlimited* | 10+ | High (SearXNG spec) | Good — free, multi-engine in one call; depends on instance health |

\* Self-hosted; public instances have no official SLA.

**Recommended primary:** Bing Web Search v7 (best result quality and richest response schema when you can pay for the subscription).
**Recommended fallback:** SearXNG with a stable public instance from searx.space (free, multi-engine aggregation; pick one with good uptime history).

---

## 3. Parameter Schema Design

**Prompt:** Define candidate parameter schemas for `web_search`:

- `query` — required string
- `num_results` / `max_results` — optional integer, default?
- `search_type` — general vs. news vs. images? (pick scope)
- `language` / `region` — optional filters

Produce a concrete JSON schema proposal and compare against how web_fetch handles its single `url` parameter.

**Why:** The Args type drives the user-facing prompt and must match what the agent actually needs.

### 3a. Comparison: web_fetch's current pattern

`WebFetchArgs` exposes only a single field, so it uses a minimal struct that derives **only `Deserialize`**:

```rust
#[derive(Deserialize)]
pub struct WebFetchArgs {
    /// The URL to fetch.
    pub url: String,
}
```

- No `JsonSchema` derive — because the tool does not auto-generate its schema.
- `parameters()` returns the raw JSON schema from `fetchkit`: `FETCH_TOOL.input_schema()`.

This works for a single-input tool (one URL to fetch), but it means the Args struct is not self-describing via schemars like every other tool in the toolkit (`read`, `edit`, `shell`). For consistency, `web_search` should follow the same pattern as those tools: derive `Deserialize + JsonSchema` and use `schemars::schema_for!`. This also gives us a richer schema to express optional fields.

### 3b. Candidate field set for `WebSearchArgs`

| Field            | Type   | Required | Default              | Notes                                               |
|------------------|--------|----------|----------------------|-----------------------------------------------------|
| `query`          | String | ✅        | —                    | The search terms; must be non-empty at call time     |
| `max_results`    | u32    | ❌       | 10                   | Results cap per engine. Bing supports up to 50, Google caps at 10. Default of 10 matches Google's limit and keeps results tight for the agent. |
| `search_type`    | String | ❌       | `"general"`          | Scope filter passed to engines that support it (Bing: general/news; DuckDuckGo/SearXNG handle via separate params). |
| `language`       | String | ❌       | —                    | e.g. `"en"`, `"fr-FR"`. Forwarded as the engine's locale param (`mkt`, `lang`). |
| `region`         | String | ❌       | —                    | e.g. `"US"`, `"EU"`. Forwarded as the engine's region/country param (`cc`, `country`). |

### 3c. Concrete JSON Schema (schemars-generated)

The Args struct is:

```rust
#[derive(Deserialize, Serialize, JsonSchema)]
pub struct WebSearchArgs {
    /// The search query to submit. Must be a non-empty string.
    pub query: String,

    /// Maximum number of results to return per engine. Defaults to 10.
    pub max_results: u32,

    /// Restrict results by scope — `general`, `news`, or `images`.
    /// Only engines that support the requested scope will respect this filter;
    /// unsupported scopes are silently ignored.
    #[serde(default = "default_search_type")]
    pub search_type: String,

    /// Language/locale hint (e.g. `"en"`, `"fr-FR"`). Forwarded to engines that accept a locale parameter.
    pub language: Option<String>,

    /// Region/country hint (e.g. `"US"`, `"EU"`). Forwarded to engines that accept a region parameter.
    pub region: Option<String>,
}
```

With `schemars::schema_for!(WebSearchArgs)`:

```jsonc
{
  "type": "object",
  "required": ["query"],
  "properties": {
    "query": {
      "type": "string",
      "description": "The search query to submit. Must be a non-empty string."
    },
    "max_results": {
      "type": "integer",
      "minimum": 1,
      "default": 10,
      "description": "Maximum number of results to return per engine. Defaults to 10."
    },
    "search_type": {
      "type": "string",
      "enum": ["general", "news", "images"],
      "default": "general",
      "description": "Restrict results by scope — `general`, `news`, or `images`. Only engines that support the requested scope will respect this filter; unsupported scopes are silently ignored."
    },
    "language": {
      "type": ["string", "null"],
      "description": "Language/locale hint (e.g. `"en"`, `"fr-FR"`). Forwarded to engines that accept a locale parameter."
    },
    "region": {
      "type": ["string", "null"],
      "description": "Region/country hint (e.g. `"US"`, `"EU"`). Forwarded to engines that accept a region parameter."
    }
  }
}
```

### 3d. Summary of trade-offs vs. web_fetch's single-field approach

| Aspect                     | web_fetch (single `url`)                               | web_search (multi-field)                                           |
|----------------------------|--------------------------------------------------------|--------------------------------------------------------------------|
| **Args derivation**        | Only `Deserialize` — no schema generation              | `Deserialize + Serialize + JsonSchema` — consistent with other tools |
| **Parameter surface**      | 1 field; trivial to expose                              | 5 fields; richer but adds complexity for the agent to reason about   |
| **Defaults**               | None needed (single required field)                    | Defaults for `max_results`, `search_type`; explicit nulls for optional strings. The agent only needs to supply `query` — everything else is safe with defaults. |
| **Engine diversity**       | Irrelevant (fetches a single URL, no engine choice)    | Required: different engines expose different params; the Args schema lets us express what each engine supports uniformly at the tool level. |
| **Future-proofing**        | N/A                                                    | Optional fields (`language`, `region`) can be added without breaking the schema — new engines simply read whatever they need from the existing struct. |

**Recommendation:** Adopt the multi-field Args struct above. It matches the established pattern in the toolkit (read, edit, shell), is forward-compatible as new engines are added, and keeps the user-facing prompt simple: only `query` is strictly required; everything else has sensible defaults or null.

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

---

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

## 8. Tool Description & Prompt Engineering

**Prompt:** Draft candidate descriptions for the tool:

- Short description shown in the agent's tool list (1–2 sentences)
- Long-form guidance on when to use websearch vs. web_fetch
- Examples of good queries vs. bad ones

Also research how other agents describe their search tools and what language works best for prompting LLMs.

**Why:** The description is the first thing the agent sees — it shapes usage patterns.

---

## 9. Testing Strategy

**Prompt:** Research:

- How to mock external HTTP calls in tests (tokio test, mockito, etc.)?
- Unit vs. integration split for a tool that depends on network I/O?
- What fixtures do we need (fake search responses)?

**Why:** Tests validate behavior without burning API credits or hitting rate limits during dev.

---

## 10. Open Questions / Decisions Needed

**Prompt:** List every design decision that blocks implementation:

- Which engine is primary vs. fallback?
- Single-engine-per-call vs. multi-engine parallel?
- Caching: yes/no, and if yes, what TTL?
- Should we support `search_type` (news/images) or keep it general-only?
- Any auth model (API key env var vs. built-in)?

**Why:** Future sessions can resolve these individually; the doc stays actionable.

