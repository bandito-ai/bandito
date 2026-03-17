# Phase 1: Onboarding

## Step 1: Install the CLI

```bash
bandito --version
```

If not installed:
```bash
# macOS
brew install bandito-ai/tap/bandito

# Any platform with Rust
cargo install --git https://github.com/bandito-ai/bandito bandito-cli
```

---

## Step 2: Account setup

> **Interactive — pause here.** Ask the user to run this, then continue once they confirm it completed.

```bash
bandito signup
```

Walks through: account creation, API key, data storage preference, first bandit, arms, and prints an SDK snippet.

If they already have an account:
```bash
bandito config        # reconfigure API key
```

**Data storage decision** — ask if not already set. Three options:

| Mode | When to use |
|------|-------------|
| `"cloud"` | Recommended for most teams. Full event text stored in Bandito cloud. Enables cloud-side analytics, LLM-as-judge, event clustering. |
| `"local"` | Default. Text stays on device — only metadata (tokens, cost, latency, reward) goes to cloud. Good for experimentation and sensitive data. |
| `"s3"` | Production + privacy. Text goes to your own S3 bucket in OTLP JSON format, never to Bandito. Full data ownership. |

Guide based on context:
- **Sensitive data** (medical, legal, financial, PII) → `"local"` or `"s3"`
- **Production app, own the data** → `"s3"` (requires AWS bucket)
- **Getting started / experimenting** → `"local"` (fewest prerequisites)
- **Want full Bandito analytics** → `"cloud"`

If they choose `"s3"`, they'll need to provide a bucket name, prefix, and region — `bandito config` handles this interactively. AWS credentials are resolved via the standard chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` or `~/.aws/credentials`), never stored in Bandito config.

When in doubt, keep `"local"` — it can be upgraded to `"s3"` later and historical events auto-migrate on next `connect()`.

---

## ✅ Checkpoint A

> Account configured. Ready to create your bandit + arms, or stopping here?

Only continue if the user confirms.

---

## Step 3: Create bandit + arms

**Preferred path: template → edit → create.** This keeps config reviewable and version-controllable.

```bash
bandito template bandit my-chatbot     # writes my-chatbot.json
```

Generated skeleton:
```json
{
  "name": "my-chatbot",
  "description": "",
  "type": "online",
  "cost_importance": 2,
  "latency_importance": 2,
  "optimization_mode": "base",
  "arms": [
    { "model": "gpt-4o", "provider": "openai", "prompt": "You are a helpful assistant." },
    { "model": "claude-sonnet-4-20250514", "provider": "anthropic", "prompt": "You are a helpful assistant." }
  ]
}
```

Help the user edit before creating:

**Name:** descriptive kebab-case — `"customer-support"`, `"email-summarizer"`, not `"bandit-1"`.

**`cost_importance` / `latency_importance`:** integers 0–5. Ask explicitly: does cost matter here? Does latency matter? 0 = ignore, 5 = heavily penalize. Setting these intentionally upfront means the bandit learns the right tradeoff from the start.

**`optimization_mode`:** `"base"` is the right default — Thompson Sampling explores naturally when uncertain. Use `"explore"` only if you want to force more arm diversity early on, or if you notice one arm being over-selected before enough data is collected.

**Arms:** each is a (model, provider, system_prompt) tuple. Help them think about what to vary:
- Different models: `gpt-4o` vs `claude-sonnet-4-20250514` vs `gemini-2.0-flash`
- Different providers for the same model: `openai` vs `azure` for gpt-4o
- Different prompts: concise vs detailed, formal vs casual, with/without few-shot examples

**Multi-provider flag:** if arms span multiple providers, raise this before writing any code. Recommend a provider abstraction — **LiteLLM** or **OpenRouter** — rather than `if result.provider == ...` branching. This is an architecture decision, not a code detail.

Then create:
```bash
bandito create my-chatbot.json
```

Or interactively (no JSON file needed):
```bash
bandito create
```

To add arms later:
```bash
bandito arm add my-chatbot gpt-4o-mini openai "You are a helpful assistant."
bandito arm add my-chatbot claude-haiku-4-5-20251001 anthropic --prompt-file prompt.txt
```

---

## ✅ Checkpoint B

> Bandit ready — this is a valid stopping point. Continue to SDK install + integration code, or stopping here?

Only continue if the user confirms.

