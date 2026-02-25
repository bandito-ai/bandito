# Bandito Project Guide

This document explains how the repo is organized, how the pieces fit together, and how to work with the codebase.

## What Bandito Does

Bandito is a contextual bandit optimizer for LLM model and prompt selection. You define a set of (model, provider, system prompt) combinations — called **arms** — and Bandito learns which arm works best for each type of query. It does this via Thompson Sampling with linear contextual features, updating a shared Bayesian posterior as it observes outcomes.

The key architectural decision: **`pull()` is pure local math.** The SDK caches the Bayesian posterior and runs Thompson Sampling in <1ms with no network call. This removes latency as an objection — Bandito adds zero overhead to the hot path.

## Repo Layout

```
bandito/
├── engine/          Rust math engine (Thompson Sampling, Bayesian updates, feature vectors)
├── cli/             Rust CLI + TUI grading workbench
├── sdks/
│   ├── python/      Python SDK (sync-first, PyO3 bindings to engine)
│   └── javascript/  JavaScript/TypeScript SDK (WASM bindings to engine)
├── skills/
│   └── bandito/     Claude Code skill (onboarding, integration, reward design)
├── .github/
│   └── workflows/   CI for engine, CLI, both SDKs, and release builds
├── homebrew-formula/ Homebrew tap formula (for CLI distribution)
├── Cargo.toml       Workspace root (engine + cli)
└── Cargo.lock
```

## How the Pieces Fit Together

```
                    ┌─────────────────────────┐
                    │      Cloud Backend       │
                    │  (separate repo/deploy)  │
                    │                          │
                    │  Stores events, computes │
                    │  Bayesian updates,       │
                    │  distributes weights     │
                    └────────┬────────────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
        ┌─────▼─────┐ ┌─────▼─────┐ ┌──────▼─────┐
        │ Python SDK │ │   JS SDK  │ │  CLI/TUI   │
        │  (PyO3)    │ │  (WASM)   │ │  (native)  │
        └─────┬──────┘ └─────┬─────┘ └──────┬─────┘
              │              │              │
              └──────────────┼──────────────┘
                             │
                    ┌────────▼────────┐
                    │   Rust Engine   │
                    │  (single source │
                    │   of truth for  │
                    │   all math)     │
                    └─────────────────┘
```

**Engine** is a Rust library crate that compiles three ways:
- Native Rust — used directly by the CLI binary
- WASM (via `wasm-pack`, feature-gated `--features wasm`) — consumed by the JS SDK
- Python extension (via PyO3/maturin, feature-gated `--features python`) — consumed by the Python SDK

**SDKs** provide the developer-facing API: `connect()`, `pull()`, `update()`, `grade()`, `sync()`, `close()`. Both SDKs are thin wrappers — all math lives in the Rust engine. They handle HTTP, SQLite event durability, config loading, and background sync.

**CLI** is a standalone Rust binary. It handles account signup, bandit/arm CRUD, leaderboard monitoring, and the TUI grading workbench. It links the engine natively (no WASM or PyO3).

**Cloud backend** (separate repo) stores all event data, runs Bayesian aggregation, and distributes updated weight vectors to SDKs. The SDKs talk to it via REST API.

## Data Flow

1. **Connect** — SDK authenticates with the cloud, receives full Bayesian state (theta, Cholesky factor, arm list)
2. **Pull** — SDK runs Thompson Sampling locally (<1ms). No network call. Returns the winning arm
3. **Execute** — Your app calls the chosen LLM with the selected model and prompt
4. **Update** — SDK writes the event to local SQLite (crash-safe), background thread flushes to cloud
5. **Grade** — Human evaluates the response (via TUI or `bandito.grade()`). Cloud updates Bayesian state

## Building Everything

### Prerequisites

- **Rust toolchain** — `rustup` with stable channel
- **Python 3.12+** with `uv` package manager
- **Node.js 18+** with `pnpm`
- **wasm-pack** — `cargo install wasm-pack` (only needed for JS SDK development)

### Engine

```bash
# Run tests (no feature flags needed)
cd engine && cargo test

# Build WASM for JS SDK
cd engine && wasm-pack build --target nodejs --out-dir pkg --features wasm
```

### CLI

```bash
cargo build -p bandito-cli
./target/debug/bandito --help
```

### Python SDK

```bash
cd sdks/python
uv sync          # installs deps + builds Rust engine via PyO3
uv run pytest -q # 95 tests
```

### JavaScript SDK

```bash
# First, build WASM (one-time, or after engine changes)
cd engine && wasm-pack build --target nodejs --out-dir pkg --features wasm

# Then install and test
cd sdks/javascript
pnpm install
pnpm test   # 41 tests
pnpm build  # CJS + ESM output
```

## Configuration

All Bandito tools share `~/.bandito/config.toml`:

```toml
api_key = "bnd_..."
data_storage = "local"
```

Created by `bandito signup` (which also creates your first bandit and prints an SDK snippet) or `bandito config`. Environment variables `BANDITO_API_KEY` and `BANDITO_DATA_STORAGE` override the file.

The `data_storage` setting controls privacy: `"local"` (default) keeps query and response text on your machine in SQLite. Only metadata (model, reward, cost, latency, tokens) goes to the cloud. Set `"cloud"` to send full text for cloud-side analytics.

## Key Concepts

- **Bandit** — a named optimization problem (e.g., "my-chatbot"). Has one shared Bayesian posterior
- **Arm** — a (model_name, model_provider, system_prompt) tuple. One of the options the bandit chooses between
- **Pull** — ask the bandit to pick the best arm for a query. Returns a `PullResult` with event_id
- **Update** — report what happened (response text, tokens, cost, latency, optional reward)
- **Grade** — human evaluation (0.0 to 1.0) of a response. Feeds back into learning
- **Reward** — composite signal combining raw reward, cost overhead, and latency overhead

## CI Workflows

| Workflow | Trigger | What it does |
|----------|---------|-------------|
| `engine.yml` | Push to `engine/` | `cargo test` (31 tests) |
| `cli.yml` | Push to `cli/` | `cargo build` + `cargo test` |
| `sdk-python.yml` | Push to `sdks/python/` or `engine/` | `uv sync` + `pytest` (95 tests) |
| `sdk-javascript.yml` | Push to `sdks/javascript/` or `engine/` | WASM build + `pnpm test` (41 tests) |
| `release.yml` | Tag `v*` | Build CLI binaries for 4 targets, create GitHub Release |

## Release Process

1. Tag a version: `git tag v0.1.0 && git push --tags`
2. `release.yml` builds CLI binaries for macOS (x86 + ARM), Linux (x86), and Windows (x86)
3. Binaries are uploaded to a GitHub Release with auto-generated release notes
4. Update SHA256 hashes in the Homebrew formula (`homebrew-formula/bandito.rb`)
