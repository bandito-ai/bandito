# Bandito JavaScript SDK

Zero-latency LLM routing via contextual bandits. `pull()` runs Thompson Sampling locally in <1ms via WASM — no network call on the hot path.

## Install

```bash
pnpm add bandito
```

Requires Node.js 18+. The [Bandito CLI](../../cli/README.md) is a separate binary for account setup, bandit management, and grading:

```bash
brew install bandito-ai/tap/bandito   # or: cargo install --path cli
bandito signup
```

## Quickstart

```typescript
import { connect, pull, update, close } from "bandito";

await connect();

// Pick the best model+prompt for this query (<1ms, WASM math)
const result = pull("my-chatbot", { query: userMessage });

// Call the winning LLM
const response = await openai.chat.completions.create({
  model: result.model,  // e.g. "gpt-4o"
  messages: [
    { role: "system", content: result.prompt },
    { role: "user", content: userMessage },
  ],
});

// Report what happened
update(result, {
  queryText: userMessage,
  response: response.choices[0].message.content,
  inputTokens: response.usage.prompt_tokens,
  outputTokens: response.usage.completion_tokens,
});

await close();
```

Latency is auto-measured between `pull()` and `update()`. Cost is auto-calculated from token counts. Override either by passing `latency` or `cost` explicitly.

## Usage Patterns

**Module-level singleton** (simplest):

```typescript
import { connect, pull, update, close } from "bandito";
await connect({ apiKey: "bnd_..." });
const result = pull("my-chatbot");
```

**Explicit client** (recommended for servers):

```typescript
import { BanditoClient } from "bandito";

const client = new BanditoClient({ apiKey: "bnd_..." });
await client.connect();
const result = client.pull("my-chatbot");
await client.close();
```

## API Reference

### `connect(options?)`

Bootstrap: authenticate, fetch bandit state, load WASM engine, flush pending events. **Async.**

| Option | Default | Description |
|--------|---------|-------------|
| `apiKey` | `BANDITO_API_KEY` env / config file | API key |
| `baseUrl` | `https://bandito-api.onrender.com` | Cloud API endpoint |
| `storePath` | `~/.bandito/events.db` | SQLite file for crash-safe event durability |
| `dataStorage` | `"local"` | `"local"` keeps text on your machine; `"cloud"` sends it to the server |

### `pull(banditName, options?) -> PullResult`

Pick the best arm. Pure WASM math, <1ms, no network. **Synchronous.**

**Returns** a `PullResult`:
- `.model` — model name (e.g. `"gpt-4o"`)
- `.prompt` — system prompt text
- `.provider` — model provider (e.g. `"openai"`)
- `.eventId` — UUID linking pull → update → grade
- `.arm` — full `Arm` object
- `.scores` — `Map<number, number>` arm scores (for debugging)

Pass `exclude: [armId]` to skip a failing arm (circuit breaker pattern).

### `update(pullResult, options?)`

Report event data. Writes to local SQLite immediately, flushes to cloud in background. **Synchronous.**

| Option | Description |
|--------|-------------|
| `queryText` | The user's query |
| `response` | LLM response (string or object) |
| `reward` | Immediate reward (0.0–1.0) |
| `cost` | Cost in dollars (auto-calculated from tokens if omitted) |
| `latency` | Latency in ms (auto-calculated from pull timing if omitted) |
| `inputTokens` | Input token count |
| `outputTokens` | Output token count |
| `segment` | `Record<string, string>` segment tags |
| `failed` | Mark as failed LLM call (defaults reward to 0.0) |

### `grade(eventId, grade)`

Submit a human grade (0.0–1.0) for a previous event. **Async.** Use the [TUI](../../cli/README.md#tui-grading-workbench) for bulk grading.

### `sync()`

Refresh bandit state from cloud. **Async.** Called automatically via background heartbeat.

### `close()`

Flush remaining events and shut down background interval. **Async.**

## How It Works

The SDK loads the Rust WASM engine during `connect()`. On `pull()`:

1. Sample from the posterior via Thompson Sampling
2. Build feature vectors per arm (model + prompt one-hot, query length, latency)
3. Score each arm, return the winner

All Bayesian updates happen server-side. The SDK refreshes via periodic heartbeat.

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
# Build WASM engine
cd engine && wasm-pack build --target nodejs --out-dir pkg --features wasm

# Install and test
cd sdks/javascript && pnpm install
pnpm test    # 41 tests
pnpm build   # CJS + ESM via tsup
```
