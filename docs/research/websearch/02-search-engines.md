# WebSearch Tool — Search Engine API Options

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

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