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

## 3. Parameter Schema Design

**Prompt:** Define candidate parameter schemas for `web_search`:
- `query` — required string
- `num_results` / `max_results` — optional integer, default?
- `search_type` — general vs. news vs. images? (pick scope)
- `language` / `region` — optional filters

Produce a concrete JSON schema proposal and compare against how web_fetch handles its single `url` parameter.

**Why:** The Args type drives the user-facing prompt and must match what the agent actually needs.

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