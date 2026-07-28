# WebSearch Tool — Tool Description & Prompt Engineering

Research prompts for building a `web_search` tool for the ShadowCode agent. Each section lists what to investigate and why it matters for the design doc.

---

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