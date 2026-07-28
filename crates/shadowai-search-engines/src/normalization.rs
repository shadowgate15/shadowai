/// Shared normalization helpers used across all search engines.
///
/// Each engine file owns its own HTTP request and response parsing, but the
/// *mapping* from native response fields to canonical `WebSearchResult` is
/// delegated here so engines don't duplicate HTML stripping or date conversion.

use std::collections::HashSet;

use crate::WebSearchResult;

/// Strip HTML tags (and common whitespace-only entities) from a string.
pub fn strip_html(html: &str) -> String {
    let no_tags = html.replace('<', "").replace('>', "");
    // Collapse whitespace runs back to single spaces, trim leading/trailing.
    no_tags.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}

/// Convert a Unix epoch timestamp (seconds) into an ISO-8601 UTC string.
pub fn unix_to_iso8601(ts: u64) -> String {
    // Format as `YYYY-MM-DDTHH:MM:SSZ`. The agent only needs freshness metadata,
    // not full precision — seconds is fine for MVP.
    let secs = ts % 60;
    let mins = (ts / 60) % 60;
    let hours = (ts / 3600) % 24;
    let days = ts / 86400;

    // Days → year/month/day needs a calendar. For MVP we use a simple epoch-based
    // calculation from the Unix epoch (1970-01-01). Good enough for freshness metadata.
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{mins:02}:{secs:02}Z")
}

/// Convert Unix epoch seconds to a calendar year/month/day tuple.
fn days_to_ymd(days: u64) -> (u32, u8, u8) {
    // Days since 1970-01-01 as a 25-bit unsigned integer.
    let mut d = days as i64;
    let mut year = 1970i32;

    loop {
        let leap = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
        if d < leap as i64 {
            break;
        }
        d -= leap as i64;
        year += 1;
    }

    // Remaining days within the current year.
    let mut remaining = d as u64;
    let mut month: u8 = 1;
    let mut day: u8 = 1;

    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for (i, &md) in month_days.iter().enumerate() {
        if remaining >= md as u64 {
            remaining -= md as u64;
            month = i as u8 + 2; // already incremented at loop start
        } else {
            day = remaining as u8 + 1;
            break;
        }
    }

    (year as u32, month, day)
}

/// Shared merge-and-dedup logic: collect all results from multiple engines,
/// deduplicate by URL, and return them as a sorted JSON string.
pub fn merge_and_dedup(results: Vec<Vec<WebSearchResult>>) -> String {
    let mut url_set = HashSet::new();
    let mut merged = Vec::with_capacity(results.iter().flatten().count());

    for engine_results in results.into_iter() {
        for result in engine_results {
            if !url_set.insert(result.url.clone()) {
                continue; // duplicate URL — skip silently
            }
            merged.push(result);
        }
    }

    // Sort by relevance score descending, then alphabetically by URL.
    merged.sort_by(|a, b| {
        a.relevance_score
            .partial_cmp(&b.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.url.cmp(&a.url))
    });

    serde_json::to_string_pretty(&merged).unwrap_or_default()
}