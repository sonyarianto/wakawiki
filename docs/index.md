---
layout: home

hero:
  name: WakaWiki
  text: Agent-driven docs for your codebase
  tagline: Let LLMs explore, read, and write structured documentation automatically.
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/sonyarianto/wakawiki
features:
  - icon: 🤖
    title: Agent-Driven
    details: LLM explores your codebase, reads files, searches for patterns, and writes structured docs into a wakawiki/ directory.
  - icon: 🔌
    title: Multiple Providers
    details: OpenAI, Anthropic, DeepSeek, OpenRouter, or local opencode (no API key needed). Bring your own LLM.
  - icon: 🔧
    title: Tool-Calling Agent Loop
    details: Built-in tools — list_files, read_file, search, write_doc — so the LLM can navigate and document any codebase.
  - icon: 🔄
    title: Incremental Updates
    details: Run wakawiki --update to refresh only what changed since the last run. Keeps docs in sync without redundant work.
  - icon: 📄
    title: AGENTS.md Integration
    details: Automatically creates or appends a reference block so coding agents know exactly where the docs live.
  - icon: ⚡
    title: CI Ready
    details: One-shot -p mode works in GitHub Actions. Schedule auto-updates via PR with zero manual effort.
  - icon: ⚙️
    title: Heuristic Scan Mode
    details: Run wakawiki --scan for instant, zero-LLM documentation. Parses your source code directly in milliseconds.
---

<div style="text-align: center; margin-top: 2rem;">
  <a href="https://github.com/sonyarianto/wakawiki/releases" style="display: inline-block; background: var(--vp-c-brand-1); color: #fff; padding: 0.25rem 0.75rem; border-radius: 1rem; font-size: 0.875rem; font-weight: 600; text-decoration: none;">v0.1.10</a>
</div>

<div style="text-align: center; margin-top: 1rem; opacity: 0.6; font-size: 0.875rem;">
  Inspired by <a href="https://github.com/langchain-ai/openwiki" target="_blank" rel="noopener">OpenWiki</a>
</div>
