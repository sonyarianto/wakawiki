# Contributing to WakaWiki

Thanks for your interest in contributing!

## Getting Started

1. Fork and clone the repo
2. Install Rust (stable) via [rustup](https://rustup.rs/)
3. Run `cargo build` to verify the setup

## Development Workflow

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run linter
cargo clippy -- -D warnings

# Format code
cargo fmt
```

All three checks (`cargo fmt`, `cargo clippy`, `cargo test`) must pass before submitting a PR.

## Commit Style

- Use imperative mood ("Add feature" not "Added feature")
- Reference issues where relevant
- Keep commits focused and atomic
- Update `CHANGELOG.md` under the `[Unreleased]` section for user-facing changes

## Pull Requests

1. Create a feature branch from `main`
2. Make your changes with tests
3. Ensure CI passes (fmt, clippy, test)
4. Open a PR with a clear description of the change

## Project Structure

- `src/main.rs` — CLI entry point and argument parsing
- `src/config.rs` — Configuration loading from `~/.wakawiki/.env`
- `src/provider/` — LLM provider integrations (OpenAI, Anthropic, OpenCode)
- `src/agent.rs` — Interactive and oneshot agent loop with tool execution
- `src/scanner.rs` — File scanning, search, and hashing
- `src/index.rs` — Code indexing for the MCP server
- `src/mcp.rs` — MCP (Model Context Protocol) server implementation
- `src/output.rs` — Documentation file writing and metadata
- `src/scan.rs` — Heuristic (non-LLM) documentation generation

## Security

If you discover a security vulnerability, please report it privately rather than opening a public issue.
