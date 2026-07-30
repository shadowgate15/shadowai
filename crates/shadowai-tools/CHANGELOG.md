# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/shadowgate15/shadowai/releases/tag/shadowai-tools-v0.1.0) - 2026-07-30

### Added

- bump rig to 0.41.0
- *(web_search)* add instance-based search engines + integration tests
- *(web_search)* async execution + merge/dedup for tool facade
- *(shadowai-agent)* register WebFetchTool with the agent loop
- *(shadowai-tools)* add web_fetch module
- convert to using schemars
- use shell sub crate
- use filesystem sub crates
- use proper tool definitions
- fix deps and other compilation issues
- *(shadowai-tools)* add file read, glob, edit, shell command tools + agent repair hook

### Fixed

- cleanup some warnings
- use websearch crate instead
- *(web_search)* reject invalid queries before engine dispatch
- *(web_search)* cleanup pieces
- *(web_fetch)* fix web_fetch content_focus
- fix tool call repair

### Other

- format
- Add in-memory cache integration to web_search tool
- Add shadowai-search-engines crate and web_search tool
- Revert "Add web_fetch module to shadowai-tools"
- Add web_fetch module to shadowai-tools
