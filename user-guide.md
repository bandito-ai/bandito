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
data_storage = "local"   # "cloud" | "local" | "s3"
```

Created by `bandito signup` or `bandito config`. Environment variable overrides: `BANDITO_API_KEY`, `BANDITO_DATA_STORAGE`, `BANDITO_S3_BUCKET`, `BANDITO_S3_PREFIX`, `BANDITO_S3_REGION`.

### data_storage

Controls where query text and response text are stored. The learning signal (metadata: model, arm, reward, cost, latency, tokens) always flows to Bandito cloud regardless of this setting.

**`"local"` (default)** — Events written to `~/.bandito/events.db` via SQLite WAL. Query and response text stay on your machine. Only metadata is sent to Bandito cloud. Best for experimentation and privacy. The TUI grading workbench reads from local SQLite.

**`"cloud"`** — Full event data (including query and response text) stored in Bandito cloud. Enables cloud-side analytics, LLM-as-judge evaluation, and event clustering without any infrastructure to manage.

**`"s3"`** — Same crash-safe SQLite WAL as local mode, but the SDK also exports events to your S3 bucket in OTLP JSON format. Query and response text stay off Bandito's servers. You own the data in your infrastructure.

```toml
data_storage = "s3"

[s3]
bucket = "my-events-bucket"
prefix = "bandito"          # optional, default "bandito"
region = "us-east-1"        # optional, default "us-east-1"
```

AWS credentials are resolved via the standard credential chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` env vars or `~/.aws/credentials`) — they are not stored in config.

**Migrating from local → s3:** Change `data_storage = "s3"`, add the `[s3]` section, and restart. On next `connect()` the SDK automatically exports all existing SQLite events to S3. No explicit migration command needed.

### S3 env vars

For container/k8s deployments without a config file:

```
BANDITO_DATA_STORAGE=s3
BANDITO_S3_BUCKET=my-events-bucket
BANDITO_S3_PREFIX=bandito        # optional, default "bandito"
BANDITO_S3_REGION=us-east-1      # optional, default "us-east-1"
```

These override the `[s3]` TOML section when set.

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
