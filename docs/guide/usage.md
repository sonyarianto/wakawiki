# Commands

## Quick Start

```bash
# Step 1: Configure your LLM provider
wakawiki --init

# Step 2: Generate documentation (interactive)
wakawiki

# Or: One-shot non-interactive
wakawiki -p "Summarize the architecture of this project"

# Or: Update existing documentation
wakawiki --update
```

## CLI Reference

```
wakawiki [OPTIONS] [PROMPT]
```

| Option | Description |
|--------|-------------|
| `--init` | Interactive setup: choose provider, set API key, pick model |
| `-p`, `--print` | Non-interactive one-shot mode (CI-friendly) |
| `--scan` | Heuristic scan (no LLM) — fast, deterministic docs |
| `--index` | Build structured JSON index of all symbols and files |
| `--embed` | Generate embeddings for semantic search |
| `-q`, `--query` | Query the index for symbols or files matching a pattern |
| `-s`, `--semantic` | Semantic search query (requires embeddings) |
| `--serve` | Start MCP server (JSON-RPC 2.0 over stdio) |
| `--update` | Refresh existing `wakawiki/` docs with incremental diff |
| `-h`, `--help` | Show help |
| `--version` | Print version and exit |

## Interactive Mode

Running `wakawiki` without `-p` starts an interactive chat session with the LLM agent. You can guide the documentation process by asking questions or giving instructions.

```bash
wakawiki
# > Please focus on the API layer
# > Add more detail about error handling
# > Generate mermaid diagrams for the architecture
```

## One-Shot Mode

Use `-p` / `--print` for non-interactive, single-prompt runs. This is ideal for scripts and CI pipelines.

```bash
wakawiki -p "Document the public API surface"
```

## Incremental Updates

Once you have an existing `wakawiki/` directory, run `--update` to refresh only the files that changed since the last run. This avoids regenerating everything from scratch.

```bash
wakawiki --update
```

## Heuristic Scan Mode

Run `--scan` for instant, deterministic documentation without an LLM. Ideal for CI or when you just need a structural overview.

```bash
wakawiki --scan
```

The scan mode:
- Parses Rust source files for `pub` items and doc comments
- Reads project metadata from `Cargo.toml`
- Generates `index.md` (dependencies, directory tree) and `architecture.md` (API reference)
- Respects your `.gitignore` for file filtering
- Runs in milliseconds

## Code Indexing

Build a structured JSON index of all symbols and files in your project.

```bash
wakawiki --index
```

Output: `wakawiki/index.json` containing:
- Project metadata (name, version, description)
- All files with paths, sizes, hashes, and language
- All public symbols (fn, struct, enum, trait, etc.) with line numbers and docs

## Query Index

Search the index by symbol name, kind, or file path.

```bash
wakawiki -q "Config"        # search by name
wakawiki -q "fn"            # search by kind
wakawiki -q "provider"      # search by path
```

## Semantic Search

Generate embeddings and perform semantic search.

```bash
wakawiki --embed             # generate embeddings first
wakawiki -s "configuration"  # semantic search
```

Uses TF-IDF vectorization — no external API required.

## MCP Server

Start an MCP (Model Context Protocol) server that exposes your codebase to AI agents.

```bash
wakawiki --index             # build index first
wakawiki --serve             # start MCP server on stdio
```

Available MCP tools:
- `query_symbols` — search by pattern
- `get_symbol` — lookup by name
- `list_files` — list all indexed files
- `get_file_info` — file details + symbols
- `get_project_info` — project metadata
- `semantic_search` — vector-based search (requires `--embed`)
