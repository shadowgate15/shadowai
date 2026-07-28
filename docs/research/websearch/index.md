# WebSearch Tool — Research Index

Entry point for all design research on building the `web_search` tool for ShadowCode. Each file covers a focused topic; consult it directly for details.

## Contents

| File | Topic |
|---|---|
| [01-tool-signature](./01-tool-signature.md) | Existing tool patterns in shadowai-tools (trait signature, error declarations, parameter exposure, shared utilities). |
| [02-search-engines](./02-search-engines.md) | Search engine API options: Google Custom Search, Bing v7, DuckDuckGo Instant Answer, SearXNG — request/response shapes, rate limits, auth. |
| [03-parameter-schema](./03-parameter-schema.md) | Parameter schema design for `WebSearchArgs`: field set, JSON schema proposal, trade-offs vs. web_fetch's single-field approach. |
| [04-result-normalization](./04-result-normalization.md) | Result normalization strategy: canonical `WebSearchResult` struct, required fields, handling divergent engine schemas, the normalization pipeline. |
| [05-caching-dedup](./05-caching-dedup.md) | Caching & deduplication: TTL recommendation, storage primitives comparison, URL + domain+snippet dedup strategies. |
| [06-error-handling](./06-error-handling.md) | Error handling & failure modes: custom `WebSearchError` type, retry/backoff strategy, rate-limit surfacing, empty results as soft errors, partial engine failures. |
| [07-async-execution](./07-async-execution.md) | Async & parallel execution: fire all engines concurrently with per-engine timeout, merge + dedup pattern, tokio primitives in use. |
| [08-tool-description-prompting](./08-tool-description-prompting.md) | Tool description & prompt engineering: short/long descriptions for the agent, web_search vs. web_fetch guidance, good/bad query examples. |
| [09-testing-strategy](./09-testing-strategy.md) | Testing strategy: mocking crate comparison (mockito / wiremock / reqwest mock), unit vs. integration split, fixtures, full test patterns. |
| [10-decisions-resolved](./10-decisions-resolved.md) | Resolved design decisions + remaining open questions for future sessions. |