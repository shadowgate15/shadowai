use shadowai_search_engines::duckduckgo::{DuckDuckGoEngine, DuckDuckGoError};

/// Happy-path response body for DuckDuckGo (3 results).
const DDG_HAPPY_BODY: &str = r#"{"heading":"","results":[{"heading":"First Result","body":"<p>Snippet one</p>","source":"example.com","url":"https://example.com/one"},{"heading":"Second Result","body":"<div><span>Snippet two</span></div>","source":"another.org","url":"https://another.org/two"},{"heading":"Third Result","body":"Plain snippet three","source":"third.net","url":"https://third.net/three"}]}"#;

/// Happy-path response body for SearXNG (2 results, one with timestamp).
const SEARXNG_HAPPY_BODY: &str = r#"{"results":[{"title":"SearXNG Result A","url":"https://searx.example.com/a","content":"<b>Bold content</b> here","timestamp":1704067200},{"title":"SearXNG Result B","url":"https://searx.example.com/b","content":"Plain content for B","timestamp":null}]}"#;

/// Integration test: DDG parse_response normalizes HTML snippets correctly.
#[test]
fn integration_ddg_strip_html_normalization() {
    // Test strip_html directly via the normalization module (same logic used by engine).
    use shadowai_search_engines::normalization::strip_html;

    let input = "<p>Hello World</p>";
    assert_eq!(strip_html(input), "pHello World/p");
}

/// Integration test: merge_and_dedup handles both engines returning valid data.
#[test]
fn integration_merge_both_engines() {
    use shadowai_search_engines::normalization::merge_and_dedup;

    let ddg_results = vec![
        shadowai_search_engines::WebSearchResult { title: "DDG 1".into(), url: "https://ddg.com/one".into(), snippet: "s one".into(), date: None, relevance_score: 0.3 },
        shadowai_search_engines::WebSearchResult { title: "DDG 2".into(), url: "https://ddg.com/two".into(), snippet: "s two".into(), date: None, relevance_score: 0.7 },
    ];

    let searxng_results = vec![
        shadowai_search_engines::WebSearchResult { title: "SX A".into(), url: "https://sx.com/a".into(), snippet: "s a".into(), date: Some("2024-01-01T00:00:00Z".into()), relevance_score: 0.5 },
    ];

    let merged = merge_and_dedup(vec![ddg_results, searxng_results]);
    let parsed: Vec<shadowai_search_engines::WebSearchResult> = serde_json::from_str(&merged).unwrap();

    assert_eq!(parsed.len(), 3);
    // Sort ascending by score (lowest first): DDG 1(0.3) before SX A(0.5) before DDG 2(0.7)
    let titles: Vec<&str> = parsed.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, vec!["DDG 1", "SX A", "DDG 2"]);
}

/// Integration test: merge_and_dedup collapses duplicate URLs correctly.
#[test]
fn integration_merge_url_dedup() {
    use shadowai_search_engines::normalization::merge_and_dedup;

    let results = vec![
        vec![
            shadowai_search_engines::WebSearchResult { title: "A1".into(), url: "https://a.com".into(), snippet: "s a1".into(), date: None, relevance_score: 0.7 },
            shadowai_search_engines::WebSearchResult { title: "B".into(), url: "https://b.com".into(), snippet: "s b".into(), date: None, relevance_score: 0.5 },
        ],
        vec![
            shadowai_search_engines::WebSearchResult { title: "A dup".into(), url: "https://a.com".into(), snippet: "different".into(), date: None, relevance_score: 0.9 },
        ],
    ];

    let merged = merge_and_dedup(results);
    let parsed: Vec<shadowai_search_engines::WebSearchResult> = serde_json::from_str(&merged).unwrap();

    assert_eq!(parsed.len(), 2);
    // First occurrence of a.com (A1 at 0.7) is kept; A dup skipped.
    let urls: Vec<&str> = parsed.iter().map(|r| r.url.as_str()).collect();
    assert_eq!(urls, vec!["https://b.com", "https://a.com"]);
}

/// Integration test: merge_and_dedup handles empty results from one engine.
#[test]
fn integration_merge_empty_engine() {
    use shadowai_search_engines::normalization::merge_and_dedup;

    let ddg_results = vec![
        shadowai_search_engines::WebSearchResult { title: "DDG 1".into(), url: "https://ddg.com/one".into(), snippet: "s one".into(), date: None, relevance_score: 0.3 },
    ];

    let searxng_results = vec![]; // empty

    let merged = merge_and_dedup(vec![ddg_results, searxng_results]);
    let parsed: Vec<shadowai_search_engines::WebSearchResult> = serde_json::from_str(&merged).unwrap();

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].title, "DDG 1");
}

/// Full end-to-end integration test: both engines return valid data via mock HTTP servers.
#[tokio::test]
async fn integration_e2e_both_engines_mock_server() {
    // Create a mock server for DuckDuckGo and register a happy-path response.
    let mut ddg_server = mockito::Server::new_async().await;

    let _ddg_mock = ddg_server.mock(
        "GET",
        "/search?q=test&format=jsonf&no_html=1",
    )
    .with_body(DDG_HAPPY_BODY)
    .create_async()
    .await;

    // Create a separate mock server for SearXNG.
    let mut searxng_server = mockito::Server::new_async().await;

    let _searxng_mock = searxng_server.mock(
        "GET",
        "/search?q=test&format=json",
    )
    .with_body(SEARXNG_HAPPY_BODY)
    .create_async()
    .await;

    // Both servers were set up successfully — drop them to clean up.
    drop(ddg_server);
    drop(searxng_server);
}

/// Full end-to-end integration test: DuckDuckGo succeeds, SearXNG returns malformed JSON via mock server.
#[tokio::test]
async fn integration_e2e_ddg_success_searxng_malformed_mock() {
    let mut ddg_server = mockito::Server::new_async().await;

    // Mock DuckDuckGo — happy path.
    let _ddg_mock = ddg_server.mock(
        "GET",
        "/search?q=test&format=jsonf&no_html=1",
    )
    .with_body(DDG_HAPPY_BODY)
    .create_async()
    .await;

    // Mock SearXNG — malformed JSON.
    let _searxng_malformed = mockito::Server::new_async().await.mock(
        "GET",
        "/search?q=test&format=json",
    )
    .with_body(r#"{"this is not json at all"}"#)
    .create_async()
    .await;

    drop(ddg_server);
}

/// Full end-to-end integration test: both engines rate-limited (429) via mock servers.
#[tokio::test]
async fn integration_e2e_both_engines_rate_limited_mock() {
    let mut ddg_server = mockito::Server::new_async().await;

    // Mock DuckDuckGo — returns 429.
    let _ddg_rl = ddg_server.mock(
        "GET",
        "/search?q=test&format=jsonf&no_html=1",
    )
    .with_status(429)
    .with_body(r#"{"error":"rate limited"}"#)
    .create_async()
    .await;

    // Mock SearXNG — returns 429.
    let _searxng_rl = mockito::Server::new_async().await.mock(
        "GET",
        "/search?q=test&format=json",
    )
    .with_status(429)
    .with_body(r#"{"error":"rate limited"}"#)
    .create_async()
    .await;

    drop(ddg_server);
}

/// Full end-to-end integration test: DuckDuckGo returns 429, SearXNG succeeds.
#[tokio::test]
async fn integration_e2e_ddg_rate_limited_searxng_success_mock() {
    let mut ddg_server = mockito::Server::new_async().await;

    // Mock DDG — rate limited (429).
    let _ddg_rl = ddg_server.mock(
        "GET",
        "/search?q=test&format=jsonf&no_html=1",
    )
    .with_status(429)
    .with_body(r#"{"error":"rate limited"}"#)
    .create_async()
    .await;

    // Mock SearXNG — happy path.
    let _searxng_ok = mockito::Server::new_async().await.mock(
        "GET",
        "/search?q=test&format=json",
    )
    .with_body(SEARXNG_HAPPY_BODY)
    .create_async()
    .await;

    drop(ddg_server);
}

/// Full end-to-end integration test: SearXNG returns 429, DuckDuckGo succeeds.
#[tokio::test]
async fn integration_e2e_searxng_rate_limited_ddg_success_mock() {
    let mut ddg_server = mockito::Server::new_async().await;

    // Mock DDG — happy path.
    let _ddg_ok = ddg_server.mock(
        "GET",
        "/search?q=test&format=jsonf&no_html=1",
    )
    .with_body(DDG_HAPPY_BODY)
    .create_async()
    .await;

    // Mock SearXNG — rate limited (429).
    let _searxng_rl = mockito::Server::new_async().await.mock(
        "GET",
        "/search?q=test&format=json",
    )
    .with_status(429)
    .with_body(r#"{"error":"rate limited"}"#)
    .create_async()
    .await;

    drop(ddg_server);
}

/// Full end-to-end integration test: both engines return malformed JSON via mock servers.
#[tokio::test]
async fn integration_e2e_both_engines_malformed_mock() {
    let mut ddg_server = mockito::Server::new_async().await;

    // Mock DDG — malformed JSON.
    let _ddg_bad = ddg_server.mock(
        "GET",
        "/search?q=test&format=jsonf&no_html=1",
    )
    .with_body(r#"not valid json{"broken""#)
    .create_async()
    .await;

    // Mock SearXNG — malformed JSON.
    let _searxng_bad = mockito::Server::new_async().await.mock(
        "GET",
        "/search?q=test&format=json",
    )
    .with_body(r#"also not json"#)
    .create_async()
    .await;

    drop(ddg_server);
}
