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
- Long-form guidance on when to use web_search vs. web_fetch
- Examples of good queries vs. bad ones

Also research how other agents describe their search tools and what language works best for prompting LLMs.

**Why:** The description is the first thing the agent sees — it shapes usage patterns.

---

### 8a. Short description (tool list)

The tool list is a compact surface area shown to the agent alongside every other tool. It needs to be scannable in one glance — long or flowery descriptions crowd out the actual tools and encourage misfires. Best practice across agent frameworks (OpenAI function-calling examples, Claude's Computer Use docs) is **one clear sentence of intent + one sentence of output shape**.

**Candidate A:**
> Search the web for current information and answers. Returns up to N results with titles, URLs, and snippets from Google, Bing, or SearXNG.

**Candidate B (preferred):**
> Find current web content by query — returns ranked search results with titles, URLs, and snippets across Google, Bing, and SearXNG.

**Why candidate B wins:** It's more active ("find" rather than "search"), explicitly names the engines so the agent knows what sources it'll tap into (which builds trust when results come back), and mentions ranking — a useful signal that not all returned URLs are equal. The word "current" is intentional: web_search is for *now*, while web_fetch is for specific known pages.

### 8b. Long-form guidance on when to use web_search vs. web_fetch

The agent's most common mistake with search tools is treating them interchangeably. This section tells it how to pick the right one. The rule of thumb:

- **web_search** = "I don't know the URL, but I want current information about X."
- **web_fetch** = "I have a specific URL and just need its contents."

| When to use web_search | When to use web_fetch |
|---|---|
| Breaking news or recent developments | You already know the documentation page URL |
| Current pricing, availability, release dates | A public API endpoint you've seen referenced elsewhere |
| Definitions, explanations of concepts | Blog posts, tutorials, or guides with a known link |
| Comparing products, libraries, frameworks | Internal docs on your own hosted site (if you have access) |
| Opinions, trends, community sentiment | Any URL the agent has already been given in context |

**Don't use web_search when:** The information is static and won't change between calls — e.g., a Rust book's table of contents. Use web_fetch instead; it's faster (no multi-engine overhead) and more reliable for known URLs.

**Don't use web_fetch when:** You're asking "what do people think about X?" or "is there a way to do Y in 2025?" — these need current search, not a specific page.

**A useful mental model:** Think of web_search as the equivalent of going to a library and asking a librarian for recommendations on a topic you're exploring. Think of web_fetch as opening a specific book you already have on your shelf. Confusing them is like trying to ask a librarian to fetch a specific book from a random store — possible, but not what either tool was designed for.

### 8c. Examples of good queries vs. bad ones

The agent should treat the `query` parameter as non-negotiable: every call must include a meaningful search string. Bad queries produce useless results and waste API quota; they're also more likely to hit rate limits on Google's free tier (100/day) or Bing's paid plans. Here are concrete examples of what works and what doesn't:

**Good queries:**

| Query | Why it works |
|---|---|
| "Rust async runtime comparison 2025" | Specific topic + recency signal; search engines can rank by freshness. |
| "How does OAuth 2.1 differ from OAuth 2.0?" | Clear technical question with a definitive answer scope. |
| "Latest changes to EU AI Act implementation timeline" | Current affairs, narrow enough for useful results. |
| "Best practices for caching in tokio applications" | Well-scoped engineering topic; search engines will surface relevant blog posts and docs. |

**Bad queries:**

| Query | Why it's bad |
|---|---|
| "" (empty) | No query to search on — the tool should reject this at validation time. |
| "hi" or "hello" | Too short; returns generic homepage results, not useful content. |
| "everything about Rust" | Overly broad; engines return millions of hits with no clear relevance signal. |
| "weather today in New York" (without specifying a date) | Ambiguous — search engines can't reliably resolve "today" across timezones and languages without more context. |

**Key principle:** A good query is specific enough that the top 10 results will be useful, but broad enough to capture multiple viewpoints if relevant ones exist. If you're unsure whether a query is too narrow or too broad, start with the narrower version — web_search returns up to N results and can always be re-issued with different phrasing.

### 8d. Language that works best for prompting LLMs about search tools

Research from OpenAI's function-calling examples and Claude's tool-use documentation shows a pattern: agent prompts work best when they're **concrete, imperative, and paired with failure-mode awareness**. Flowery prose or abstract guidelines ("think carefully") tend to be ignored; specific "do X in this case" rules stick.

Three observations relevant here:

1. **Failure modes are more instructive than success cases.** Telling the agent "use web_search for current info" is vague; telling it "don't use web_search when you already have a URL — that's what web_fetch exists for" prevents a whole class of mistakes. The table in 8b leverages this.

2. **Concrete examples > abstract principles.** A single "good vs. bad query" example teaches the agent more than a paragraph about "being specific." Two examples (one good, one bad) are enough to calibrate; three or four is diminishing returns for a tool description that already has plenty of other content.

3. **The description should name the tool's boundaries.** Agents generalize aggressively — if you describe what the tool *is*, they'll also try it in contexts where it's wrong. Explicitly stating "don't use this when X" sharpens those boundaries.

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

---

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
| **Per-engine timeout** | 5 s per engine, independent deadlines via `tokio::time::timeout`. | One slow engine can't extend the total call time — each has its own budget. Timeout converts to `WebSearchError::Timeout(engine_id)` so callers skip it during merge (section 7b). |
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

