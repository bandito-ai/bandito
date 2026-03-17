# CLI Reference

## Account & Config

| Command | Interactive | What it does |
|---------|-------------|-------------|
| `bandito signup` | **yes** | Create account + API key + first bandit + SDK snippet |
| `bandito config` | **yes** | Reconfigure API key |
| `bandito install python` | no | Install Python SDK via uv or pip |
| `bandito install js` | no | Install JS SDK via pnpm, npm, or yarn |
| `bandito skill` | no | Install `/bandito` skill into current project |

## Bandits & Arms

| Command | What it does |
|---------|-------------|
| `bandito template bandit <name>` | Write `<name>.json` skeleton with 2 placeholder arms |
| `bandito template script --sdk python` | Write `bandito_example.py` starter |
| `bandito template script --sdk js` | Write `bandito_example.ts` starter |
| `bandito create <file.json>` | Create bandit + arms from JSON |
| `bandito create` | Create bandit interactively (**interactive**) |
| `bandito list` | List all bandits |
| `bandito arm list <bandit>` | List arms for a bandit |
| `bandito arm add <bandit> <model> <provider> [prompt]` | Add arm (prompt defaults to "You are a helpful assistant.") |
| `bandito arm add <bandit> <model> <provider> --prompt-file <file>` | Add arm with prompt from file |
| `bandito arm deactivate <bandit> <model>` | Soft-delete arm — keeps history, stops selection |

## Monitoring & Grading

| Command | What it does |
|---------|-------------|
| `bandito leaderboard <bandit>` | Arm performance table (pull%, reward, cost, latency) |
| `bandito leaderboard <bandit> --graded` | Filtered to graded events only |
| `bandito leaderboard <bandit> --watch` | Auto-refresh every 30s |
| `bandito tui` | Grading workbench (keyboard-driven) |

## LLM-as-Judge

| Command | Interactive | What it does |
|---------|-------------|-------------|
| `bandito judge config` | **yes** | Set judge API key + model |
| `bandito judge gen-rubric <bandit>` | no | Generate quality rubric from graded events |
| `bandito judge edit-rubric <bandit>` | no | Open rubric in `$EDITOR` |
| `bandito judge calibrate <bandit> [--sample N]` | no | Score human-graded events, show confusion matrix (no writes) |
| `bandito judge augment <bandit> [--sample N]` | confirms | Grade ungraded events with judge (writes to cloud) |
| `bandito judge review <bandit> [--disagreements]` | no | Show events with human + judge grades |
| `bandito judge status <bandit>` | no | Judge config, metrics, and local event counts |

## TUI Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate events |
| `y` / `n` | Grade good / bad |
| `s` | Skip |
| `r` | Refresh from cloud |
| `1` / `2` / `3` | Copy user input / response / system prompt to clipboard |
| `q` / `Esc` | Back / quit |
| `?` | Help overlay |

## Config File

```toml
# ~/.bandito/config.toml
api_key = "bnd_..."
base_url = "https://bandito-api.onrender.com"
data_storage = "local"    # "local" | "cloud" | "s3"

# Required when data_storage = "s3"
[s3]
bucket = "my-events-bucket"
prefix = "bandito"          # default
region = "us-east-1"        # default
# endpoint = "http://localhost:9000"  # optional: MinIO / LocalStack / custom S3

[judge]
api_key = "sk-..."          # or set JUDGE_API_KEY env var
model = "claude-sonnet-4-6"
```

AWS credentials for S3 mode are resolved via standard chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` or `~/.aws/credentials`) — not stored here.

**S3 env vars:** `BANDITO_S3_BUCKET` (activates S3 mode implicitly), `BANDITO_S3_PREFIX`, `BANDITO_S3_REGION`, `BANDITO_S3_ENDPOINT`
