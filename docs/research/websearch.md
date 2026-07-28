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

---

## 5. Caching & Deduplication

**Prompt:** Research:

- Should websearch cache queries? If so, what TTL makes sense for search results?
- How to detect and deduplicate results across multiple engines (same domain, similar snippet)?
- What storage primitive fits (in-memory map vs. file-backed cache)?

**Why:** Caching reduces API costs and call volume; deduplication keeps result counts honest.

---

## 6. Error Handling & Failure Modes

**Prompt:** Study `FetchError` from fetchkit used in web_fetch, then research:

- Network timeout handling — should it be retryable? With what backoff?
- Rate-limit responses (HTTP 429) — how to surface them to the agent?
- Empty results — is that a hard error or a soft one?
- Partial engine failures when using multiple engines in parallel

Propose an `Error` type for websearch and retry strategy.

**Why:** Robust failure modes keep the tool usable even when individual engines are down.

---

## 7. Async & Parallel Execution

**Prompt:** Research:

- Should we fire all available engines concurrently and merge results, or pick one engine per call?
- How to handle async timeouts per-engine without blocking others?
- What does `tokio` concurrency look like here (tasks, select, etc.)?

**Why:** Parallel execution gives the agent more diverse results but adds complexity. Must decide trade-off up front.

---

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

