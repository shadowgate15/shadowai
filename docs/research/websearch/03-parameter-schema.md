# WebSearch Tool — Parameter Schema Design

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

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