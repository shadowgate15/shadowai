# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/shadowgate15/shadowai/releases/tag/shadowai-v0.1.0) - 2026-07-30

### Added

- seperate cli from agent
- intial ouptut with hook
- map all events to ipc
- setup ipc
- bump rig to 0.41.0
- integrate web_search into agent
- *(web_search)* add instance-based search engines + integration tests
- *(web_search)* async execution + merge/dedup for tool facade
- shared normalization helpers for search engines
- SearXNG engine — HTTP request + response parsing
- DuckDuckGo engine — HTTP request + response parsing
- implement web_search tool implementation plan
- *(shadowai-agent)* register WebFetchTool with the agent loop
- *(shadowai-tools)* add web_fetch module
- convert to using schemars
- implement DDD split into main.rs
- tests handled
- use shell sub crate
- use filesystem sub crates
- use proper tool definitions
- fix deps and other compilation issues
- setup workspace
- *(shadowai-agent)* add conversation domain crate
- *(shadowai-tools)* add file read, glob, edit, shell command tools + agent repair hook
- add shadowai-shell domain crate — shell execution logic
- extract filesystem domain crate (shadowai-filesystem)
- increase tool call limit
- add tool call repair hook
- report tool calls and reasoning
- add bash tool
- add edit file tool
- update preamble
- ensure that truncation won't happen when calling ollama
- report token usage
- create glob tool
- create read tool
- make a multi-line chat bot
- better system prompt
- chatbot integration
- initial ai assistant

### Fixed

- cleanup some warnings
- use websearch crate instead
- fix test
- *(web_search)* reject invalid queries before engine dispatch
- *(web_search)* cleanup pieces
- *(web_fetch)* fix web_fetch content_focus
- fix tool call repair

### Other

- release-plz config
- cleanup plan docs
- format
- tier-1 unit tests for search-engines parsing and normalization
- Add in-memory cache integration to web_search tool
- use correct verions for deps
- Add shadowai-search-engines crate and web_search tool
- reorganize websearch research doc into focused sub-files
- section 10
- section 9
- section 8
- section 7
- section 6
- section 5
- section 4
- section 2 & 3
- section 1 of research
- initial research doc
- cleanup the discovery artifacts
- Revert "Add web_fetch module to shadowai-tools"
- Add web_fetch module to shadowai-tools
- plan
- update current pattern summary
- add fetchkit again
- Merge branch 'main' into feat/webfetch-tool
- cleanup deps
- cleanup plan.md
- update plan
- plan.md
- cleanup output
- ignore worktrees
- add futures
- add anyhow
- add rig and tokio deps
- init
