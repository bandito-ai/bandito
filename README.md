# Bandito

Provider-agnostic contextual bandit optimizer for LLM model and prompt selection. Automatically routes each request to the best (model, provider, prompt) combination — learning continuously from real outcomes.

**Zero-latency architecture:** `pull()` is pure local math (<1ms). No network call on the hot path.

## Install

### CLI

```bash
# macOS
brew install bandito-ai/tap/bandito

# From source (any platform)
cargo install --path cli
```

### Python SDK

```bash
pip install bandito   # or: uv add bandito
```

Requires Python 3.12+.

### JavaScript/TypeScript SDK

```bash
pnpm add @bandito-ai/sdk   # or: npm install @bandito-ai/sdk
```

Requires Node.js 18+.

## Quickstart

```bash
# 1. Install
brew install bandito-ai/tap/bandito

# 2. Sign up, create your first bandit, and add arms — all in one command
bandito signup

# 3. Paste the printed snippet into your app
```

`bandito signup` walks you through account creation, bandit setup, and prints a ready-to-paste SDK snippet. That's it.

### Grade & Monitor

```bash
bandito tui                       # grade responses interactively
bandito leaderboard my-chatbot    # see which arm is winning
```

## Claude Code Skill

If you use [Claude Code](https://claude.ai/code), install the Bandito skill into your project:

```bash
cd your-project
bandito skill
```

This gives you a `/bandito` slash command in Claude Code that helps with onboarding, SDK integration, reward function design, and interpreting leaderboard results.

## Documentation

- [Project Guide](user-guide.md) — repo layout, architecture, how everything fits together
- [Deployment Guide](DEPLOYMENT.md) — releasing CLI, publishing SDKs, Homebrew tap setup
- [CLI Reference](cli/README.md) — all commands, TUI grading workbench, configuration
- [Python SDK](sdks/python/README.md) — full API reference, usage patterns
- [JavaScript SDK](sdks/javascript/README.md) — full API reference, usage patterns

## Development

This is a Cargo workspace with two Rust crates (`engine` and `cli`) plus two SDK directories under `sdks/`.

```bash
# Rust engine tests (31 tests)
cd engine && cargo test

# CLI build
cargo build -p bandito-cli

# Python SDK tests (95 tests)
cd sdks/python && uv sync && uv run pytest -q

# JavaScript SDK tests (41 tests)
cd sdks/javascript && pnpm install && pnpm test
```

See [user-guide.md](user-guide.md) for build prerequisites and full development setup.

## License

[MIT](LICENSE)
