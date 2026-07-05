# codewiki

A CLI that writes and maintains agent documentation for your codebase. Built in Rust.

## Features

- **Agent-driven documentation** — LLM explores your codebase, reads files, searches for patterns, and writes structured docs
- **Multiple providers** — OpenAI, Anthropic, DeepSeek, OpenRouter, or local `opencode` (no API key needed)
- **Tool-calling agent loop** — `list_files`, `read_file`, `search`, `write_doc`
- **Incremental updates** — `--update` refreshes only what changed
- **AGENTS.md integration** — appends a reference block so coding agents know where the docs live
- **GitHub Actions ready** — one-shot `-p` mode works in CI

## Install

```bash
cargo install --git https://github.com/sonyarianto/codewiki.git
```

Or build from source:

```bash
git clone https://github.com/sonyarianto/codewiki.git
cd codewiki
cargo build --release
```

## Quick Start

```bash
# Configure your LLM provider
codewiki --init

# Generate documentation (interactive)
codewiki

# One-shot non-interactive
codewiki -p "Summarize the architecture of this project"

# Update existing documentation
codewiki --update
```

## Usage

```
codewiki [OPTIONS] [PROMPT]

Options:
      --init     Configure provider, API key, and model
  -p, --print    Non-interactive one-shot mode
      --update   Refresh existing codewiki/ docs
  -h, --help     Show help
```

### Provider Options

| Provider | API Key Required | Notes |
|----------|-----------------|-------|
| OpenAI | Yes | GPT-4, GPT-4o |
| Anthropic | Yes | Claude Sonnet |
| DeepSeek | Yes | OpenAI-compatible API |
| OpenRouter | Yes | Unified API gateway |
| opencode | **No** | Uses local `opencode` CLI |
| Custom | Yes | Any OpenAI-compatible endpoint |

### opencode provider

If you already use [opencode](https://github.com/anomalyco/opencode), you can run codewiki with zero API keys:

```bash
codewiki --init    # select "opencode"
codewiki           # uses your existing opencode setup
```

## Output

```
codewiki/
├── index.md            # Project overview
├── architecture.md     # High-level architecture
├── ...                 # Additional module docs
└── .codewiki.json      # Metadata for incremental updates
```

An `AGENTS.md` file is created (or appended) with a reference block pointing agents to `codewiki/`.

## GitHub Action

Add this workflow to auto-update docs daily via PR:

```yaml
name: codewiki update
on:
  schedule:
    - cron: '0 0 * * *'
  workflow_dispatch:

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cargo install --git https://github.com/sonyarianto/codewiki.git
      - run: codewiki --update
        env:
          CODWIKI_PROVIDER: ${{ secrets.CODWIKI_PROVIDER }}
          CODWIKI_API_KEY: ${{ secrets.CODWIKI_API_KEY }}
          CODWIKI_MODEL: ${{ secrets.CODWIKI_MODEL }}
      - uses: peter-evans/create-pull-request@v6
        with:
          title: 'docs: update codewiki documentation'
          branch: codewiki-update
```

## Configuration

Config is stored in `~/.codewiki/.env`:

```
CODWIKI_PROVIDER=openai
CODWIKI_API_KEY=sk-...
CODWIKI_MODEL=gpt-4o
CODWIKI_BASE_URL=https://api.openai.com/v1
```

You can also set these as environment variables directly.
