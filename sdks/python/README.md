# Bandito Python SDK

Zero-latency LLM routing via contextual bandits. `pull()` runs Thompson Sampling locally in <1ms — no network call on the hot path.

## Install

```bash
pip install bandito
```

Requires Python 3.12+. The [Bandito CLI](../../cli/README.md) is a separate binary for account setup, bandit management, and grading:

```bash
brew install bandito-ai/tap/bandito   # or: cargo install --path cli
bandito signup
```

## Quickstart

```python
import bandito

bandito.connect()

# Pick the best model+prompt for this query (<1ms, pure local math)
result = bandito.pull("my-chatbot", query=user_message)

# Call the winning LLM
response = openai.chat.completions.create(
    model=result.model,  # e.g. "gpt-4o"
    messages=[
        {"role": "system", "content": result.prompt},
        {"role": "user", "content": user_message},
    ],
)

# Report what happened
bandito.update(
    result,
    query_text=user_message,
    response=response.choices[0].message.content,
    input_tokens=response.usage.prompt_tokens,
    output_tokens=response.usage.completion_tokens,
)

bandito.close()
```

Latency is auto-measured between `pull()` and `update()`. Cost is auto-calculated from token counts via model pricing tables. Override either by passing `latency=` or `cost=` explicitly.

## Usage Patterns

**Module-level singleton** (simplest):

```python
import bandito
bandito.connect()
result = bandito.pull("my-chatbot")
```

**Context manager** (recommended — auto-closes):

```python
from bandito import BanditoClient

with BanditoClient(api_key="bnd_...") as client:
    result = client.pull("my-chatbot")
```

**Explicit lifecycle** (servers, testing):

```python
client = BanditoClient(api_key="bnd_...")
client.connect()
# ... use client ...
client.close()
```

## API Reference

### `connect(api_key=None, **kwargs)`

Bootstrap: authenticate, fetch bandit state, flush any pending events from a previous crash.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `api_key` | `BANDITO_API_KEY` env / config file | API key |
| `base_url` | `https://bandito-api.onrender.com` | Cloud API endpoint |
| `store_path` | `~/.bandito/events.db` | SQLite file for crash-safe event durability |
| `data_storage` | `"local"` | `"local"` keeps text on your machine; `"cloud"` sends it to the server |

### `pull(bandit_name, *, query=None, exclude=None) -> PullResult`

Pick the best arm. Pure local math, <1ms, no network.

**Returns** a `PullResult`:
- `.model` — model name (e.g. `"gpt-4o"`)
- `.prompt` — system prompt text
- `.provider` — model provider (e.g. `"openai"`)
- `.event_id` — UUID linking pull → update → grade
- `.arm` — full `Arm` object
- `.scores` — `dict[int, float]` arm scores (for debugging)

Pass `exclude=[arm_id]` to skip a failing arm (circuit breaker pattern).

### `update(pull_result, **kwargs)`

Report event data. Writes to local SQLite immediately (crash-safe), then flushes to cloud in the background.

| Parameter | Description |
|-----------|-------------|
| `query_text` | The user's query |
| `response` | LLM response (string or dict) |
| `reward` | Immediate reward (0.0–1.0) |
| `cost` | Cost in dollars (auto-calculated from tokens if omitted) |
| `latency` | Latency in ms (auto-calculated from pull timing if omitted) |
| `input_tokens` | Input token count |
| `output_tokens` | Output token count |
| `segment` | `dict[str, str]` segment tags |
| `failed` | `bool` — mark as failed LLM call (defaults reward to 0.0) |

### `grade(event_id, grade)`

Submit a human grade (0.0–1.0) for a previous event. Synchronous HTTP — blocks until confirmed. Use the [TUI](../../cli/README.md#tui-grading-workbench) for bulk grading.

### `sync()`

Refresh bandit state from cloud. Called automatically via background heartbeat; call manually for immediate refresh.

### `close()`

Flush remaining events and shut down background threads.

## How It Works

The SDK caches the Bayesian posterior locally. On `pull()`:

1. Sample from the posterior via Thompson Sampling
2. Build feature vectors per arm (model + prompt one-hot, query length, latency)
3. Score each arm, return the winner

All Bayesian updates happen server-side. The SDK refreshes its cache via periodic heartbeat.

**Crash safety** — events go to local SQLite (WAL mode) before any network call. Pending events retry on next `connect()`.

**Fail-safe** — if the cloud is unreachable, the SDK keeps routing with last-known-good weights. Your app never breaks.

## Configuration

Shared `~/.bandito/config.toml` (created by `bandito signup` or `bandito config`):

```toml
api_key = "bnd_..."
base_url = "https://bandito-api.onrender.com"
data_storage = "local"
```

Environment variables `BANDITO_API_KEY`, `BANDITO_BASE_URL`, `BANDITO_DATA_STORAGE` override the file.

## Development

```bash
cd sdks/python
uv sync            # builds Rust engine + installs SDK
uv run pytest -q   # 95 tests
```
