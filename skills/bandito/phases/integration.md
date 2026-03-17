# Phase 2: Integration

## Step 1: Install the SDK

Use the CLI when possible:
```bash
bandito install python    # installs via uv or pip
bandito install js        # installs via pnpm, npm, or yarn
```

Or manually with their package manager:

**Python:**
```bash
uv add bandito     # or: pip install bandito
```

**JS/TS:**
```bash
pnpm add @bandito-ai/sdk   # or: npm install @bandito-ai/sdk / yarn add @bandito-ai/sdk
```

Use whichever package manager was captured in the preference interview. Never mix.

---

## Step 2: Understand their codebase

Before writing any code:

1. **How many distinct LLM use cases?** Each becomes a separate bandit. A chatbot and a summarizer are separate bandits — never mix them.
2. **Offer to read their existing LLM call file.** Ask: "Can you share the file where your LLM call lives? I'll adapt the integration to your actual code rather than generating from a template." If they share a path, read it and match their variable names, error handling, and response parsing.
3. **Multi-step pipeline?** If one LLM call feeds into another (e.g., text→SQL then SQL→visualization), each step is its own bandit.

---

## Step 3: Design checkpoint

Confirm before writing code:

1. **Bandits defined?** One per distinct LLM use case.
2. **Multi-provider?** LiteLLM or OpenRouter decided — not custom branching.
3. **`cost_importance` / `latency_importance`** set on the bandit?

Reward signal is addressed in Phase 3 — don't block integration on it.

---

## Step 4: Write the integration code

Use the language and framework from the preference interview. The structural patterns below show where `pull()` and `update()` fit. Fill in the LLM call from the client adapters section.

**Python pattern:**
```python
import atexit
import bandito

bandito.connect()                          # once at startup
atexit.register(bandito.close)             # once at shutdown — or use framework lifecycle (see table below)

def handle_request(user_message: str) -> str:
    result = bandito.pull("BANDIT_NAME", query=user_message)

    # your LLM call — see client adapters below
    text, input_tokens, output_tokens = call_llm(
        model=result.model,
        prompt=result.prompt,
        message=user_message,
    )

    reward = compute_reward(user_message, text)   # see Phase 3

    bandito.update(
        result,
        query_text=user_message,
        response=text,
        reward=reward,
        input_tokens=input_tokens,
        output_tokens=output_tokens,
    )

    return text
```

**JavaScript/TypeScript pattern:**
```typescript
import { connect, pull, update, close } from "@bandito-ai/sdk";

await connect();                           // once at startup
process.on("SIGTERM", close);              // once at shutdown — or use framework lifecycle (see table below)

async function handleRequest(userMessage: string): Promise<string> {
  const result = pull("BANDIT_NAME", { query: userMessage });  // sync, no await

  // your LLM call — see client adapters below
  const { text, inputTokens, outputTokens } = await callLlm({
    model: result.model,
    prompt: result.prompt,
    message: userMessage,
  });

  const reward = computeReward(userMessage, text);  // see Phase 3

  update(result, {  // sync, no await
    queryText: userMessage,
    response: text,
    reward,
    inputTokens,
    outputTokens,
  });

  return text;
}
```

**Key rules:**
- `pull()` and `update()` are always synchronous — no `await` in either SDK
- `connect()` once at startup, `close()` once at shutdown
- Token counts are optional but enable automatic cost tracking
- `reward` is optional — Bandito still learns on cost/latency without it

---

## Verify the integration

Make a request through the app, then:

```bash
bandito leaderboard BANDIT_NAME
```

Pull count should be non-zero. If nothing shows:
- Confirm `connect()` is being called at startup
- `update()` writes to local SQLite immediately but cloud sync is async (up to 30s) — call `bandito.sync()` to flush immediately
- Confirm API key is valid: `bandito config`

---

## Framework placement for `connect()` / `close()`

| Framework | `connect()` | `close()` |
|-----------|------------|----------|
| FastAPI | `@asynccontextmanager` lifespan | same lifespan teardown |
| Flask | app factory or `before_first_request` | `atexit.register` |
| Express | top-level `await connect()` before `app.listen` | `process.on('SIGTERM')` |
| Next.js | `instrumentation.ts` `register()` | not needed (serverless per-request) |
| Plain script | top of file + `atexit.register(bandito.close)` | end of file |

---

## LLM client adapters

Adapt to whatever client the user has (from preference interview).

**OpenAI SDK:**
```python
response = client.chat.completions.create(
    model=result.model,
    messages=[
        {"role": "system", "content": result.prompt},
        {"role": "user", "content": user_message},
    ],
)
text = response.choices[0].message.content
input_tokens = response.usage.prompt_tokens
output_tokens = response.usage.completion_tokens
```

**Anthropic SDK:**
```python
response = client.messages.create(
    model=result.model,
    system=result.prompt,
    messages=[{"role": "user", "content": user_message}],
)
text = response.content[0].text
input_tokens = response.usage.input_tokens
output_tokens = response.usage.output_tokens
```

**LiteLLM (multi-provider):**
```python
response = litellm.completion(
    model=f"{result.provider}/{result.model}",
    messages=[
        {"role": "system", "content": result.prompt},
        {"role": "user", "content": user_message},
    ],
)
text = response.choices[0].message.content
input_tokens = response.usage.prompt_tokens
output_tokens = response.usage.completion_tokens
```

**Vercel AI SDK (TypeScript):**
```typescript
import { generateText } from "ai";
import { openai } from "@ai-sdk/openai";

const { text, usage } = await generateText({
  model: openai(result.model),
  system: result.prompt,
  prompt: userMessage,
});

update(result, {
  queryText: userMessage,
  response: text,
  inputTokens: usage.promptTokens,
  outputTokens: usage.completionTokens,
  reward,
});
```

---

## Multi-step pipeline pattern

Each step is its own bandit. Chain by passing the previous step's output as context to the next `pull()`.

```python
# Step 1: text → SQL
sql_result = bandito.pull("text-to-sql", query=user_question)
sql_response = run_llm(model=sql_result.model, prompt=sql_result.prompt, query=user_question)
sql_reward = 1.0 if execute_sql(sql_response.text) else 0.0
bandito.update(sql_result, response=sql_response.text, reward=sql_reward, ...)

# Step 2: SQL results → visualization
context = f"columns: {get_columns(sql_response.text)}"
viz_result = bandito.pull("viz-selector", query=context)
viz_response = run_llm(model=viz_result.model, prompt=viz_result.prompt, query=context)
bandito.update(viz_result, response=viz_response.text, ...)
```

Each bandit learns independently. Don't combine rewards across steps into one signal.

---

## Starter template (optional)

If the user wants a standalone starter file to edit:
```bash
bandito template script --sdk python    # writes bandito_example.py
bandito template script --sdk js        # writes bandito_example.ts
```

---

## ✅ Checkpoint C

> Integration wired up. Continue to reward design, or stopping here?

Only continue if the user confirms.
