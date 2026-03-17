# Phase 4: Operations

## Monitoring

```bash
bandito leaderboard my-chatbot             # arm performance table
bandito leaderboard my-chatbot --graded    # filtered to human-graded events only
bandito leaderboard my-chatbot --watch     # auto-refresh every 30s
```

**Interpreting results:**
- **Pull%** — converges toward the best arm(s) over time. Still spread evenly = still exploring, or the reward signal is too weak to distinguish arms.
- **Reward** — average composite reward per arm. Higher is better.
- **Avg Cost / Avg Latency** — compare across arms to understand the tradeoff the bandit is making.
- One arm dominating Pull% with high reward = converging. Good.
- Arms still evenly spread after significant traffic = needs more data, or stronger reward signal.

---

## Grading events

```bash
bandito tui
```

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate event list |
| `y` / `n` | Grade good / bad |
| `s` | Skip (moves to end, stays skipped for the session) |
| `r` | Refresh from cloud |
| `1` / `2` / `3` | Copy user input / response / system prompt to clipboard |
| `q` / `Esc` | Back / quit |

Grades feed directly into Bayesian updates. Human grades carry more weight than the machine reward and refine arm selection beyond what the automated signal captures.

**Grading requires cloud connectivity.** Local SQLite provides the event content (query + response) for display; the grade itself is written to cloud where the Bayesian state lives.

---

## Managing arms

```bash
bandito arm list my-chatbot

bandito arm add my-chatbot gpt-4o-mini openai "You are a helpful assistant."
bandito arm add my-chatbot claude-haiku-4-5-20251001 anthropic --prompt-file prompt.txt

bandito arm deactivate my-chatbot gpt-4o    # soft-delete: keeps history, stops selection
```

**When to add new arms:**
- A new model is released (gpt-4o-mini, claude-haiku, gemini-flash)
- The user wants to test a meaningfully different prompt strategy
- Current arms have similar performance — try something different

**When to deactivate:**
- A provider has an outage or the model is being retired
- An arm is consistently at the bottom of the leaderboard with enough data to be confident

Deactivation is always soft — history is preserved and the arm can be re-activated.

---

## Configuration reference

**Local or cloud storage:**
```toml
# ~/.bandito/config.toml
api_key = "bnd_..."
data_storage = "local"   # "local" | "cloud" | "s3"

[judge]
api_key = "sk-..."          # or set JUDGE_API_KEY env var
model = "claude-sonnet-4-6"
```

**S3 storage** (add `[s3]` section when `data_storage = "s3"`):
```toml
data_storage = "s3"

[s3]
bucket = "my-events-bucket"
prefix = "bandito"          # default
region = "us-east-1"        # default
# endpoint = "http://localhost:9000"  # optional: MinIO / LocalStack
```

AWS credentials are resolved via the standard chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` or `~/.aws/credentials`) — never stored in `~/.bandito/config.toml`.

**S3 env vars** (useful for container deployments):

| Env var | Purpose |
|---------|---------|
| `BANDITO_S3_BUCKET` | Activates S3 mode implicitly if set |
| `BANDITO_S3_PREFIX` | Override prefix |
| `BANDITO_S3_REGION` | Override region |
| `BANDITO_S3_ENDPOINT` | Override endpoint (MinIO etc.) |

Run `bandito config` to change storage mode interactively — it handles the S3 prompts.

---

## Phase 5 gate

When the user hits ≥ 20 human grades, surface this proactively:

> You have ≥ 20 human grades — ready to try LLM-as-judge to scale grading without manual effort?
> Read `phases/judge.md` to continue.
