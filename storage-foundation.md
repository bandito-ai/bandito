# Storage Foundation

## Vision

Bandito event data lives where you need it: in Bandito cloud for managed
intelligence, on your machine for privacy-first experimentation, or in your own
S3 bucket (in OTEL format) for production deployments where you want to own the
data. The learning signal to Bandito cloud always flows via `/api/v1/events`
regardless of where the full event text is stored — storage mode and learning
signal are independent concerns.

## Three Storage Modes

### 1. Bandito Cloud (`data_storage = "cloud"`)

**Recommended for most teams.** Full event data — query text, response text,
cost, latency, tokens — is stored in Bandito cloud PostgreSQL. Enables
cloud-side analytics, LLM-as-judge evaluation, event clustering, and
changepoint detection without any infrastructure to manage.

```toml
# ~/.bandito/config.toml
data_storage = "cloud"
```

TUI connects to Bandito cloud API for event retrieval.

### 2. Local SQLite (`data_storage = "local"`)

**Default for new installs. Good for experimentation and privacy.** Events
written to `~/.bandito/events.db` via SQLite WAL immediately after `update()`.
Only metadata (model, arm, reward, cost, latency, tokens) is sent to Bandito
cloud — query text and response text stay on your machine.

```toml
# ~/.bandito/config.toml
data_storage = "local"   # default
```

Background thread flushes metadata to Bandito cloud for learning. TUI reads
event text from local SQLite. Crash-safe via WAL.

### 3. SQLite + S3 (`data_storage = "s3"`)

**For production + privacy.** Same crash-safe SQLite WAL as local mode, but the
SDK also exports events to your S3 bucket in OTLP JSON format. Full event text
stays off Bandito's servers. You own the data in your own infrastructure,
queryable by any OTEL-compatible tool.

```toml
# ~/.bandito/config.toml
data_storage = "s3"

[s3]
bucket = "my-events-bucket"
prefix = "bandito"          # default
region = "us-east-1"        # default
```

AWS credentials are resolved via the standard credential chain
(`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` env vars or `~/.aws/credentials`).
They are never stored in `~/.bandito/config.toml`.

**Auto-migration from local → s3:** Changing `data_storage` from `"local"` to
`"s3"` and restarting your app causes the SDK to automatically export all
historical SQLite events (those not yet marked `s3_exported`) to S3 on next
`connect()`. No explicit migration command needed.

**Durability:** SQLite WAL survives process crashes; S3 export happens on
background flush. Hard crashes can delay S3 export but never lose events —
they'll be exported on next restart.

---

## S3 OTLP Format

Events are serialized as OTLP JSON spans and uploaded to S3:

**Key pattern:** `{prefix}/{bandit_name}/{YYYY/MM/DD}/{timestamp}_{event_uuid}.json`

**Span structure:**

```
bandito.llm_call  (span name)

  bandito.bandit.name           string
  bandito.bandit.id             int
  bandito.arm.id                int
  bandito.arm.model             string    # mirrors gen_ai.request.model
  bandito.arm.provider          string
  bandito.event_id              string    # UUID — primary key, ties to grade()
  bandito.reward                float
  bandito.optimization_mode     string
  gen_ai.usage.input_tokens     int
  gen_ai.usage.output_tokens    int
  bandito.cost                  float
  bandito.latency_ms            float
  bandito.query_text            string
  bandito.response_text         string    # or JSON for structured responses
  span duration                           # wall-clock from pull() to update()
```

`bandito.event_id` (a UUID) is used as the OTEL trace ID (hyphens stripped,
padded to 32 hex chars). This is the same UUID that flows through:
- `PullResult.event_id` (returned from `pull()`)
- `local_event_uuid` stored in SQLite
- `local_event_uuid` flushed to Bandito cloud
- `PATCH /api/v1/events/{uuid}/grade` for grade submission

---

## Full Trace Ingestion

### What users can already do

`response` in `update()` accepts `string | dict`. A user running a multi-step
pipeline can pass the full structured output:

```python
bandito.update(result,
    query_text=user_query,
    response={
        "retrieved_chunks": chunks,
        "rerank_scores": scores,
        "final_answer": answer,
        "tool_calls": [...],
    }
)
```

This works today. AI features (LLM-as-judge, clustering) can operate on
whatever structure the user includes. In S3 mode, this full JSON blob lands in
the `bandito.response_text` span attribute.

### S3 format

Events exported to S3 use OTLP JSON format, meaning the files are readable
by any OTEL-compatible tool (Grafana Tempo, Jaeger, Honeycomb, etc.). This
is a serialization choice — there is no live OTEL endpoint. The files sit in
S3 until something queries them.

---

## Current Architecture

```
SDK
  update()  →  SQLite WAL  (crash recovery, local mode TUI source of truth)
  background   flush to /api/v1/events  (learning signal, always)
  background   flush to S3 in OTLP JSON  (s3 mode only)

TUI
  local/s3 mode:  reads SDK's local SQLite for query/response text
  cloud mode:     reads Bandito cloud API for query/response text
```

---

## Implementation Status

**Three storage modes:** Complete. `data_storage = "cloud" | "local" | "s3"`.
S3Config (bucket, prefix, region) in all three config systems (CLI, Python SDK,
JS SDK). SQLite schema updated with `s3_exported` column. Background S3 export
in both SDKs. `bandito config` three-way prompt.

---

## Open Questions

**Grade as span**
`grade()` calls Bandito cloud directly. Should grades also propagate as span
events to the user's OTEL provider / S3? A grade is an annotation on the
original `bandito.llm_call` span — natural in OTEL (span events / linked
spans). Deferred.

**TUI integration with S3**
TUI currently reads from local SQLite or Bandito cloud. Future: TUI connects
to S3 (download + query spans) for display in the grading workbench.
