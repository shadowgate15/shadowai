# FetchKit API Reference (v0.5.0)

FetchKit is an AI-friendly web content fetching library for Rust, designed to fetch web content with optional HTML-to-markdown/text conversion optimized for LLM consumption. It includes a pluggable fetcher system for specialized URL patterns and strong SSRF/security policies.

---

## Table of Contents

1. [Core Types](#core-types)
2. [Error Types](#error-types)
3. [HTML Conversion Utilities](#html-conversion-utilities)
4. [Transport Layer](#transport-layer)
5. [DNS Policy (SSRF Prevention)](#dns-policy-ssrf-prevention)
6. [Client — Fetch Entry Points](#client--fetch-entry-points)
7. [Tool Builder & Execution](#tool-builder--execution)
8. [File Saving](#file-saving)
9. [Fetcher System](#fetcher-system)
10. [Crawl Discovery](#crawl-discovery)

---

## 1. Core Types

### `FetchRequest`

The request object passed to every fetch operation. Supports builder-pattern chaining:

```rust
let req = FetchRequest::new("https://example.com")
    .method(HttpMethod::Head)
    .as_markdown()
    .content_focus("agent");
```

| Field | Type | Default | Description |
|---|---|---|---|
| `url` | `String` | — (required) | Target URL. Bare domain URLs (`example.com/docs`) are normalized to `https://...`. |
| `method` | `Option<HttpMethod>` | Some(GET) | HTTP method: GET or HEAD only. |
| `as_markdown` | `Option<bool>` | Some(false) | Convert HTML → Markdown. |
| `as_text` | `Option<bool>` | Some(false) | Convert HTML → plain text. |
| `save_to_file` | `Option<String>` | None | Save response bytes to a file path instead of returning content inline. Requires `FileSaver`. |
| `content_focus` | `Option<String>` | Some("full") | Extraction focus: `"full"`, `"main"`, `"readable"`, or `"agent"` (AI-agent optimized). |
| `if_none_match` | `Option<String>` | None | ETag value for conditional requests (`If-None-Match`). |
| `if_modified_since` | `Option<String>` | None | Last-Modified value for conditional requests. |
| `crawl` | `Option<bool>` | Some(false) | Enable bounded same-origin crawl discovery on the seed URL. |
| `max_pages` | `Option<usize>` | — | Max pages to fetch during crawl (default: 5, max: 20). |
| `render` | `Option<RenderMode>` | None | Optional rakers-rendered HTML backend. |

### `FetchResponse`

The response returned by every fetch operation:

```rust
let resp = FetchResponse {
    url: "https://example.com".into(),
    status_code: 200,
    content_type: Some("text/html; charset=utf-8".into()),
    size: Some(1024),
    format: Some("markdown".into()),
    content: Some("# Hello World".into()),
    ..Default::default()
};
```

| Field | Type | Description |
|---|---|---|
| `url` | `String` | The fetched URL. |
| `status_code` | `u16` | HTTP status code. |
| `content_type` | `Option<String>` | Content-Type header value. |
| `size` | `Option<u64>` | Response body size in bytes. |
| `last_modified` | `Option<String>` | Last-Modified header. |
| `etag` | `Option<String>` | ETag header (for conditional requests). |
| `filename` | `Option<String>` | Extracted filename from the URL/path. |
| `format` | `Option<String>` | `"markdown"`, `"text"`, or `"raw"`. |
| `content` | `Option<String>` | Fetched/converted content (Markdown, text, or raw). |
| `truncated` | `Option<bool>` | True if content was truncated due to timeout. |
| `method` | `Option<String>` | `"HEAD"` when a HEAD request was made. |
| `error` | `Option<String>` | Error message for binary/non-HTML responses. |
| `saved_path` | `Option<String>` | Path where file was saved (when `save_to_file` was used). |
| `bytes_written` | `Option<u64>` | Bytes written to file. |
| `metadata` | `Option<PageMetadata>` | Structured metadata extracted from HTML. |
| `quality` | `Option<PageQuality>` | Agent-facing quality signals (score, warnings, suggested action). |
| `crawl` | `Option<CrawlResult>` | Crawl discovery result (when crawl was enabled). |
| `word_count` | `Option<u64>` | Word count of the final content. |
| `redirect_chain` | `Vec<String>` | Chain of URLs followed during redirects. |
| `is_paywall` | `Option<bool>` | Heuristic paywall detection signal. |
| `rendered_by` | `Option<String>` | Rendering backend used before conversion (e.g., `"rakers"`). |

### `HttpMethod`

```rust
enum HttpMethod { Get, Head } // default: GET
```

Parseable from string (`"GET"`, `"HEAD"`), display as uppercase.

### `PageMetadata`

Structured metadata extracted from an HTML page in a single pass: title, description (from `<meta name="description">` or Open Graph), language, canonical URL, author, published/modified dates, links, headings outline, extraction method. Includes `is_empty()` helper.

### `PageLink`

A link extracted from the page with its visible text and href.

### `PageQuality`

Agent-facing quality signals: normalized score (0–1), warnings list, link density, suggested next action for agents when quality is poor.

### `CrawlResult`

Summary of bounded same-origin crawl discovery: seed URL, max pages budget, visited pages (`Vec<CrawlPage>`), and whether the crawl was truncated because there were more candidates than the page budget allowed.

### `CrawlPage`

Per-page info during a crawl: final URL, status code, title, description, content type, word count, quality score, or error message if fetching failed.

---

## 2. Error Types

### `FetchError` (thiserror::Error)

| Variant | Message |
|---|---|
| `MissingUrl` | "Missing required parameter: url" |
| `InvalidUrlScheme` | "Invalid URL: must be http://, https://, or a bare domain URL" |
| `InvalidMethod` | "Invalid method: must be GET or HEAD" |
| `BlockedUrl` | "Blocked URL: not allowed by policy" |
| `ClientBuildError(#[source] reqwest::Error)` | Failed to build HTTP client (sanitized). |
| `FirstByteTimeout` | "Request timed out: server did not respond within 1 second" |
| `ConnectError(#[source] reqwest::Error)` | Failed to connect. |
| `RequestError(String)` | Generic request error. |
| `FetcherError(String)` | Fetcher-specific error. |
| `SaveError(String)` | File save failed. |
| `SaverNotAvailable` | No FileSaver provided but save_to_file was requested. |
| `RenderNotAvailable` | Rendered fetch backend not available. |

### `ToolError` (thiserror::Error)

Two variants: `UserFacing(String)` (safe for LLM display) and `Internal(String)` (operator-facing). Includes `is_user_facing()` helper.

---

## 3. HTML Conversion Utilities

All functions are pure (no async), work on raw HTML strings, strip dangerous elements (`script`, `style`, `noscript`, `iframe`, `svg`), and decode HTML entities.

| Function | Returns | Description |
|---|---|---|
| [`html_to_markdown(html)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.html_to_markdown.html) | `String` | Convert HTML → Markdown (headings, lists, emphasis, code blocks, links, blockquotes, tables, definition lists). |
| [`html_to_markdown_with_base_url(html, base)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.html_to_markdown_with_base_url.html) | `String` | Same as above but resolves relative links/images against the given base URL (useful for fetched pages). |
| [`html_to_text(html)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.html_to_text.html) | `String` | Strip all HTML tags, return plain text with newlines preserved. |
| [`extract_metadata(html)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.extract_metadata.html) | `PageMetadata` | Extract title, description, language, canonical URL, author, dates, links, headings from HTML in a single pass. |
| [`extract_headings(html)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.extract_headings.html) | `Vec<String>` | Second-pass heading extraction (cheap — headings are sparse). Returns outline like `["# Title", "## Section 1"]`. |
| [`strip_boilerplate(html)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.strip_boilerplate.html) | `String` | Remove `<nav>`, `<footer>`, `<aside>`, and role-based boilerplate elements. If `<main>` or `<article>` exists, extracts only its content. |
| [`extract_readable_content(html)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.extract_readable_content.html) | `Option<String>` | Extract the densest article-like block for AI agents. Scores candidates by word count, paragraph count, heading count, semantic bonus (article/main), and penalizes link-heavy/boilerplate-looking blocks. Returns `None` when confidence is too low. |
| [`is_markdown_content_type(ct)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.is_markdown_content_type.html) | `bool` | Check if content-type indicates Markdown (`text/markdown`). |
| [`is_plain_text_content_type(ct)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.is_plain_text_content_type.html) | `bool` | Check if content-type indicates plain text (`text/plain`). |
| [`is_html(content_type, body)`](https://docs.rs/fetchkit/0.5.0/fetchkit/fn.is_html.html) | `bool` | Detect HTML by content type or body start (`<!DOCTYPE`, `<html`). |

---

## 4. Transport Layer

### `HttpTransport` (async_trait::async_trait)

A pluggable trait for socket-level HTTP transport:

```rust
#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn execute(&self, req: TransportRequest) -> Result<TransportResponse, TransportError>;
}
```

### `TransportRequest`

A single outbound hop. Contains method (GET/HEAD), fully-resolved URL, headers (pre-populated by fetchkit with User-Agent, Accept, conditional headers, bot-auth signatures), timeout, pinned addresses (from DNS policy), and proxy env flag.

### `TransportResponse`

Streaming response: status code, final URL, response headers, and a `BodyStream` (`Pin<Box<dyn Stream<Item = Result<Bytes, TransportError>> + Send>>`).

### `TransportError` (thiserror::Error)

| Variant | Description |
|---|---|
| `Connect` | Connection failure. |
| `Timeout` | Timed out waiting for the server. |
| `Request(String)` | Request-level error. |
| `Other(String)` | Generic transport failure. |
| `Reqwest(#[source] reqwest::Error)` | Raw reqwest error from default transport (preserves classification). |

Maps to [`FetchError`] via `From<TransportError>`.

### `ReqwestTransport` (Default)

The built-in implementation: new `reqwest::Client` per request, redirects disabled (`Policy::none()`), ambient proxy env ignored by default, DNS pinned to fetchkit-validated addresses when available.

---

## 5. DNS Policy (SSRF Prevention)

### `DnsPolicy`

Resolves hostnames and validates against blocked IP ranges before allowing connections.

| Method | Description |
|---|---|
| [`block_private_ips()`](https://docs.rs/fetchkit/0.5.0/fetchkit/enum.DnsPolicy.html#method.block_private_ips) | `DnsPolicy` with private/reserved IPs blocked (default). |
| [`allow_all()`](https://docs.rs/fetchkit/0.5.0/fetchkit/enum.DnsPolicy.html#method.allow_all) | Permissive policy — no IP blocking. |
| [`is_blocked_ip(ip)`](https://docs.rs/fetchkit/0.5.0/fetchkit/struct.DnsPolicy.html#method.is_blocked_ip) | Check if an `IpAddr` is in a blocked range (handles IPv6-mapped-IPv4). |
| [`pinned_addrs(host, port)`](https://docs.rs/fetchkit/0.5.0/fetchkit/struct.DnsPolicy.html#method.pinned_addrs) | Resolve hostname and return validated socket addresses for transport pinning. |

Blocked IPv4 ranges: loopback (127), private 10/8, 172.16–31, 192.168, link-local (cloud metadata 169.254.x), carrier-grade NAT (100.64–100.127), documentation/test ranges (TEST-NET-1/2/3), benchmarking, multicast, broadcast.

Blocked IPv6: loopback (::1), unspecified (::), link-local (fe80::/10), unique local (fc00::/7), multicast (ff00::/8), IPv4-compatible and 6to4 encodings of blocked IPs.

---

## 6. Client — Fetch Entry Points

### `fetch(req: FetchRequest) -> Result<FetchResponse, FetchError>`

Convenience wrapper with Markdown + text conversion enabled by default. Uses the default fetcher registry.

### `fetch_with_options(req, options) -> Result<FetchResponse, FetchError>`

Fetch with custom [`FetchOptions`](#fetchoptions). Default registry is used. Crawl requests are routed to crawl handler.

### `batch_fetch(requests: Vec<FetchRequest>, concurrency: Option<usize>) -> Vec<Result<...>>`

Concurrent fetch of multiple URLs (default concurrency: 5, max cap: 20). Results returned in original order. Each failure is independent — one error doesn't affect others.

### `batch_fetch_with_options(requests, options, concurrency)`

Batch with custom options and configurable concurrency.

### `FetchOptions`

Configuration for fetch operations:

| Field | Type | Default | Description |
|---|---|---|---|
| `user_agent` | `Option<String>` | None | Custom UA string. |
| `allow_prefixes` | `Vec<String>` | Empty | Whitelist URL prefixes (URL-aware matching). |
| `block_prefixes` | `Vec<String>` | Empty | Blocklist URL prefixes. |
| `enable_markdown` | `bool` | false | Enable markdown conversion in tool builder. |
| `enable_text` | `bool` | false | Enable text conversion in tool builder. |
| `dns_policy` | `DnsPolicy` | block_private_ips | SSRF prevention policy. |
| `max_body_size` | `Option<usize>` | None | Max response body size (default: 10 MB). |
| `enable_save_to_file` | `bool` | false | Opt-in file saving support. |
| `respect_proxy_env` | `bool` | false | Honor HTTP_PROXY/HTTPS_PROXY env vars. |
| `allowed_ports` | `Vec<u16>` | Empty | Restrict outbound to these ports. |
| `blocked_hosts` | `Vec<String>` | Empty | Block exact hosts and suffix rules (leading '.' = suffix). |
| `same_host_redirects_only` | `bool` | false | Restrict redirects to original host only. |
| `redirect_origin` | `Option<Url>` | None | Internal crawl boundary for redirects. |
| `enable_render_rakers` | `bool` | false | Enable rakers rendered-fetch backend (feature-gated). |
| `transport` | `Option<Arc<dyn HttpTransport>>` | ReqwestTransport | Custom HTTP transport. |

---

## 7. Tool Builder & Execution

### `ToolBuilder`

Fluent builder for configuring a [`Tool`](#tool):

```rust
let tool = Tool::builder()
    .enable_markdown(true)
    .user_agent("MyBot/1.0")
    .allow_prefix("https://docs.example.com")
    .block_prefix("https://internal.example.com")
    .max_body_size(2 * 1024 * 1024) // 2 MB cap
    .respect_proxy_env(false)
    .hardened() // production preset: private IP blocked, proxy env ignored, ports 80+443 only, same-host redirects only
    .build();
```

| Method | Description |
|---|---|
| `locale(locale)` | Set locale (default `"en-US"`). Ukrainian translations supported. |
| `enable_markdown(bool)` | Opt into Markdown conversion in tool schema. |
| `enable_text(bool)` | Opt into text conversion in tool schema. |
| `user_agent(ua)` | Custom User-Agent string. |
| `allow_prefix(prefix)` / `block_prefix(prefix)` | URL prefix allow/block lists. |
| `max_body_size(size)` | Max response body size (default: 10 MB). |
| `enable_save_to_file(bool)` | Opt-in file saving support. |
| `respect_proxy_env(bool)` / `use_env_proxy(bool)` | Honor proxy env vars. Disabled by default. |
| `allow_port(port)` / `block_host(host)` / `block_host_suffix(suffix)` | Network-level restrictions. |
| `same_host_redirects_only(bool)` / `_if_set(Option<bool>)` | Restrict cross-host redirects. |
| `block_private_ips(bool)` | Enable/disable private IP blocking (SSRF). |
| `hardened()` | Production hardening preset (private IPs blocked, proxy env ignored, ports 80+443 only, common internal suffixes blocked, same-host redirects only). |
| `transport(Arc<dyn HttpTransport>)` | Custom HTTP transport. |

### `Tool`

The configured fetcher tool for LLM/agent invocation:

| Method | Description |
|---|---|
| `name()` → `"web_fetch"` | Tool name for LLM calls. |
| `display_name()` / `description()` / `version()` / `locale()` | Metadata accessors. |
| `system_prompt()` | System prompt contribution (locale-aware). |
| `help()` / `llmtxt()` | Comprehensive Markdown help document (parameters, examples, adapters, errors). |
| `input_schema()` → `Value` | JSON Schema for tool input parameters. |
| `output_schema()` → `Value` | JSON Schema for tool output. |
| `build_tool_definition()` → `Value` | OpenAI-compatible function tool definition (`{"type":"function", "function": {"name","description","parameters"}}`). |
| `execution(args: Value)` → `Result<ToolExecution, ToolError>` | Create a single-use execution from JSON args. |
| `execute(req: FetchRequest) -> Result<FetchResponse, FetchError>` | Execute with typed request. |
| `execute_with_status(mut req, mut callback)` | Execute with status updates (`validate` → `connect` → `fetch` → `complete`). |
| `execute_with_saver(req, saver)` | Execute with optional file saving support. |

### `ToolExecution`

Single-use runtime execution:

| Method | Description |
|---|---|
| `execute()` → `Result<ToolOutput, ToolError>` | Run to completion (no file saver). |
| `execute_with(saver)` → `Result<ToolOutput, ToolError>` | Run with injected file saver adapter. |

### `ToolService` (Tower)

Generic JSON args → JSON result service implementing `tower::Service<Value>`. Use for executor-oriented consumers:

```rust
let mut svc = tool.build_service();
let response_value = svc.call(json!({ "url": "https://example.com" })).await.unwrap();
```

---

## 8. File Saving

### `FileSaver` (async_trait::async_trait)

Trait for destination control:

```rust
#[async_trait]
pub trait FileSaver: Send + Sync {
    async fn save(&self, path: &str, bytes: &[u8]) -> Result<SaveResult, FileSaveError>;
    async fn validate_path(&self, path: &str) -> Result<(), FileSaveError> { default: Ok }
}
```

### `LocalFileSaver`

Built-in filesystem saver.

| Constructor | Description |
|---|---|
| [`LocalFileSaver::new(base_dir)`](https://docs.rs/fetchkit/0.5.0/fetchkit/struct.LocalFileSaver.html#method.new) | Optional base directory for relative path resolution. Without it, only absolute paths accepted. |

Security: symlink traversal blocked (O_NOFOLLOW on Unix), path traversal outside base_dir rejected, parent directories auto-created, NUL-byte rejection in paths.

### `SaveResult`

| Field | Description |
|---|---|
| `path` | Canonical/normalized save path. |
| `bytes_written` | Bytes written to file. |

### `FileSaveError`

| Variant | Message |
|---|---|
| `PathNotAllowed(String)` | Path traversal, outside base dir, not a file, symlink detected, etc. |
| `Io(#[from] std::io::Error)` | IO error during save. |
| `Other(String)` | Other save failure. |

---

## 9. Fetcher System

### `Fetcher` (async_trait::async_trait)

Trait for specialized content fetchers:

```rust
#[async_trait]
pub trait Fetcher: Send + Sync {
    fn name(&self) -> &'static str;
    fn matches(&self, url: &Url) -> bool;
    async fn fetch(&self, request: &FetchRequest, options: &FetchOptions) -> Result<FetchResponse, FetchError>;
    // Default impl delegates to fetch() then saves through saver
    async fn fetch_to_file(&self, request, options, saver) -> Result<FetchResponse, FetchError> { ... }
}
```

### `FetcherRegistry`

Ordered dispatch registry. Iterates fetchers and uses the first match:

| Method | Description |
|---|---|
| [`new()`](https://docs.rs/fetchkit/0.5.0/fetchkit/struct.FetcherRegistry.html#method.new) | Empty registry. |
| [`with_defaults()`](https://docs.rs/fetchkit/0.5.0/fetchkit/struct.FetcherRegistry.html#method.with_defaults) | Pre-registered with all built-in fetchers (see below). |
| `register(fetcher)` | Add a custom fetcher. Order matters — more specific first. |

### Built-in Fetchers (21 total, in priority order):

| # | Name | Handles |
|---|---|---|
| 1 | JupyterNotebookFetcher | GitHub/GitLab notebook blob URLs |
| 2–5 | GitHubCodeFetcher, GitHubCommitFetcher, GitHubActionsRunFetcher, GitHubIssueFetcher | GitHub code files, commits, actions runs, issues/PRs + comments |
| 6 | GitHubReleaseFetcher | GitHub release URLs |
| 7 | GitHubRepoFetcher | GitHub repository metadata and README |
| 8 | GitLabFetcher | GitLab project and resource URLs |
| 9 | TwitterFetcher | Twitter/X tweet content with article metadata |
| 10 | StackOverflowFetcher | Stack Exchange Q&A content |
| 11 | PackageRegistryFetcher | PyPI, crates.io, npm package metadata |
| 12 | WikipediaFetcher | Wikipedia articles via MediaWiki API |
| 13 | YouTubeFetcher | YouTube video metadata via oEmbed |
| 14 | ArXivFetcher | arXiv paper metadata and abstract |
| 15 | CrossrefFetcher | Crossref DOI metadata |
| 16 | IetfRfcFetcher | IETF RFC content |
| 17 | PubMedFetcher | PubMed article metadata |
| 18 | HackerNewsFetcher | Hacker News threads via Firebase API |
| 19 | RSSFeedFetcher | RSS/Atom feed parsing |
| 20 | DocsSiteFetcher | docs sites + llms.txt probes with DefaultFetcher fallback |
| 21 | DefaultFetcher | All remaining HTTP/HTTPS URLs (catch-all) |

### URL-Aware Prefix Matching

Policy prefixes support scheme matching, host normalization (case-insensitive, trailing-dot stripped), port-aware matching (explicit port must match exactly; implicit default matches any port on that host), and path prefix matching. This prevents subdomain tricks like `internal.example.com.evil.com`.

---

## 10. Crawl Discovery

Bounded same-origin crawl for agent workflows:

| Method | Description |
|---|---|
| `FetchRequest::crawl(true)` / `.max_pages(n)` | Enable crawl + set budget (default: 5 pages incl. seed, max: 20). |
| `fetch(req.crawl(true))` | Fetch with crawl enabled — returns `FetchResponse { crawl: Some(CrawlResult) }`. |

Behavior: fetches the seed URL first, extracts same-origin links from `<a href>` tags in metadata, filters out non-fetchable assets (`.css`, `.js`, images, PDFs), and fetches up to `max_pages - 1` additional pages. Redirects outside the seed origin are blocked. Returns `truncated: true` if more candidates existed than budget allowed.
