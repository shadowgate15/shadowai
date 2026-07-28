pub mod args;
pub mod error;

use shadowai_search_engines::WebSearchResult;

/// Canonical search result — re-exported from the search-engines crate.
pub type SearchResult = WebSearchResult;

pub use args::WebSearchArgs;
pub use error::WebSearchError;