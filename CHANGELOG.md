# Changelog

All notable changes to this project will be documented in this file.

## [0.1.9] - 2026-09-03

### Security
- Path traversal protection: new `safe_join()` helper rejects `..` components in `list_files`, `read_file`, and `write_doc` tool arguments
- `.env` config file written with `0600` permissions on Unix (was default `0644`)

### Added
- Agent loop iteration cap (default 100 turns) to prevent runaway API costs from models that never finish
- HTTP retry with exponential backoff (3 attempts, 500ms/1s/2s) on 429 and 5xx responses from LLM providers
- Malformed JSON tool arguments now surface a clear error to the LLM instead of silently defaulting to empty
- `provider::create` unit tests covering all dispatch paths
- `CONTRIBUTING.md` with development workflow, commit style, and project structure

### Changed
- `compute_file_hash` documented as non-cryptographic (SipHash, change-detection only)
- `output::write_doc` now returns `Result<PathBuf, String>` for path traversal rejection
- Test suite expanded from 71 to 79 tests

## [0.1.8] - 2026-09-01

### Added
- `--index` flag: builds structured JSON index of all symbols and files (`wakawiki/index.json`)
- `--embed` flag: generates TF-IDF embeddings for semantic search (`wakawiki/embeddings.json`)
- `-q`, `--query` option: text-based search on the index by symbol name, kind, or file path
- `-s`, `--semantic` option: vector-based semantic search using cosine similarity
- `--serve` flag: starts MCP server (JSON-RPC 2.0 over stdio) with 6 tools
  - `query_symbols` — search by pattern
  - `get_symbol` — lookup by name
  - `list_files` — list all indexed files
  - `get_file_info` — file details + symbols
  - `get_project_info` — project metadata
  - `semantic_search` — vector-based search (requires `--embed`)

### New Modules
- `src/index.rs`: JSON index builder and query engine
- `src/mcp.rs`: MCP server implementation
- `src/vector.rs`: TF-IDF embeddings and cosine similarity search

### Changed
- Added `wakawiki/` to `.gitignore` (generated output)
- Updated README and docs with new CLI options
- Test suite expanded from 56 to 66 tests

## [0.1.7] - 2026-07-05

### Fixed
- opencode provider timeout raised from 120s to 600s (10 minutes) — local LLMs on consumer hardware can take several minutes per response

## [0.1.6] - 2026-07-05

### Added
- `--scan` mode: heuristic documentation without an LLM — parses Rust source files for `pub` items, doc comments, module structure, and project metadata. Runs in milliseconds.
- `--version` flag

### Changed
- Animated spinner with progress messages during LLM API calls
- Tool call progress output now shown in all modes (one-shot, update, interactive)
- File filtering in `--scan` respects `.gitignore` for any codebase

### Fixed
- 120-second timeout on all provider HTTP requests (OpenAI, Anthropic, custom)
- 120-second timeout on opencode subprocess calls
- Removed hard tool call iteration cap (previously 100); agent runs until completion

### Documentation
- VitePress docs updated with `--scan` and `--version` usage

## [0.1.5] - 2026-07-05

### Added
- `--version` flag (uses `CARGO_PKG_VERSION`)
- `AGENTS.md` with release checklist for automated tooling

## [0.1.4] - 2026-07-05

### Added
- Animated spinner with status messages during LLM API calls ("Generating documentation...", "Updating documentation...", "Thinking...")
- Tool call progress output in one-shot (`-p`) and update (`--update`) modes
- 120-second timeout on all provider HTTP requests (OpenAI, Anthropic, custom)
- 120-second timeout on opencode subprocess calls

### Changed
- VitePress documentation site added

## [0.1.3] - 2026-06-29

### Changed
- Renamed project from wikigen to wakawiki

### Fixed
- npm binary wrapper renamed to wakawiki

## [0.1.2] - 2026-06-29

### Changed
- Node.js version bumped to 24 in CI

## [0.1.1] - 2026-06-29

### Added
- npm package distribution
- Multi-platform binary release CI workflow

## [0.1.0] - 2026-06-28

### Added
- Initial release
- Interactive documentation generation with LLM agents
- One-shot mode (`-p`) for non-interactive use
- Update mode (`--update`) for refreshing existing docs
- Multi-provider support: OpenAI, Anthropic, DeepSeek, OpenRouter, opencode
- Filesystem scanner with list, read, search, and hash operations
- Metadata tracking for incremental updates
- Comprehensive test suite (40 tests)
- GitHub Actions CI
