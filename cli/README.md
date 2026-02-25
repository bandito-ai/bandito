# Bandito CLI

Command-line tool and TUI grading workbench for [Bandito](https://bandito.dev) — the contextual bandit optimizer for LLM selection. Works with both the Python and JavaScript SDKs.

## Install

```bash
# macOS
brew install bandito-ai/tap/bandito

# From source (any platform)
cargo install --path cli
```

## Quickstart

Go from zero to optimized LLM routing in under 5 minutes:

```bash
# 1. Install
brew install bandito-ai/tap/bandito

# 2. Sign up, create your first bandit, and add arms
bandito signup

# 3. Paste the printed SDK snippet into your app
```

`bandito signup` walks you through account creation, bandit setup, and prints a ready-to-paste code snippet for your chosen SDK. See [Python SDK](../sdks/python/README.md) or [JS SDK](../sdks/javascript/README.md) for full API docs.

Once events are flowing:

```bash
bandito leaderboard my-chatbot    # see which arm is winning
bandito tui                       # grade responses interactively
```

## Commands

### Account & Config

| Command | What it does |
|---------|-------------|
| `bandito signup` | Create account + API key + first bandit + SDK snippet (all-in-one) |
| `bandito config` | Reconfigure an existing account (validates connection) |
| `bandito install python` | Install the Python SDK (uses uv or pip) |
| `bandito install js` | Install the JavaScript SDK (uses pnpm, npm, or yarn) |
| `bandito skill` | Install Claude Code `/bandito` skill into current project |

### Bandits & Arms

| Command | What it does |
|---------|-------------|
| `bandito template bandit <name>` | Write a `<name>.json` skeleton with placeholder arms |
| `bandito create <file.json>` | Create bandit + arms from JSON template |
| `bandito create` | Create bandit interactively (no file needed) |
| `bandito list` | Show all bandits |
| `bandito arm list <bandit>` | Show arms for a bandit |
| `bandito arm add <bandit> <model> <provider> [prompt]` | Add an arm (prompt defaults to "You are a helpful assistant.") |
| `bandito arm add <bandit> <model> <provider> --prompt-file <file>` | Add an arm with system prompt from file |

### Monitoring

| Command | What it does |
|---------|-------------|
| `bandito leaderboard <bandit>` | Arm performance table (pulls, reward, cost, latency) |
| `bandito leaderboard <bandit> --graded` | Same, filtered to graded events only |
| `bandito leaderboard <bandit> --watch` | Auto-refresh every 30s |

### Templates

| Command | What it does |
|---------|-------------|
| `bandito template script --sdk python` | Write `bandito_example.py` starter |
| `bandito template script --sdk js` | Write `bandito_example.ts` starter |
| `bandito template bandit <name>` | Write `<name>.json` bandit skeleton |

## TUI Grading Workbench

```bash
bandito tui
```

Split-pane terminal UI for reviewing and grading LLM responses. Select a bandit, navigate events, and grade with `y`/`n`. Grades feed back into the bandit to improve arm selection.

```
┌──────────────────┬─────────────────────────────────────┐
│ Events (12)      │ Detail                              │
│                  │                                     │
│ > gpt-4o / open  │ gpt-4o / openai                     │
│   5m  r:0.87     │ cost: $0.003  latency: 320ms        │
│                  │                                     │
│   claude / anth   │ USER INPUT                          │
│   3m  r:0.72     │ What is the meaning of life?        │
│                  │                                     │
│                  │ RESPONSE                            │
│                  │ The meaning of life is...            │
│                  │                                     │
│                  │ SYSTEM PROMPT                       │
│                  │ You are a helpful assistant.         │
├──────────────────┴─────────────────────────────────────┤
│ y:good  n:bad  s:skip  r:refresh  1/2/3:copy  ?:help  │
└────────────────────────────────────────────────────────┘
```

**Keyboard shortcuts:**

| Key | Action |
|-----|--------|
| `j`/`k` or arrows | Navigate events |
| `y` | Grade good (1.0) |
| `n` | Grade bad (0.0) |
| `s` | Skip (move to end) |
| `r` | Refresh from cloud |
| `1` | Copy user input to clipboard |
| `2` | Copy response to clipboard |
| `3` | Copy system prompt to clipboard |
| `?` | Help screen |
| `q` / `Esc` | Back to bandit select / quit |

The TUI merges cloud event metadata with local SQLite text. If you're using `data_storage = "local"` (the default), query and response text are read from `~/.bandito/events.db` — they never leave your machine.

## Configuration

All Bandito tools share `~/.bandito/config.toml`:

```toml
api_key = "bnd_..."
data_storage = "local"
```

| Field | Default | Description |
|-------|---------|-------------|
| `api_key` | — | Your API key (created by `bandito signup`) |
| `data_storage` | `"local"` | `"local"` keeps query/response text on your machine; `"cloud"` sends it to the server |

Environment variables `BANDITO_API_KEY` and `BANDITO_DATA_STORAGE` override the config file.

## Development

```bash
cargo build -p bandito-cli
cargo test -p bandito-cli
```
