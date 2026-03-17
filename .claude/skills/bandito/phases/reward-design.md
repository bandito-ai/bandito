# Phase 3: Reward Design

The reward function tells Bandito what "good" means for your specific task. This is the most important design decision.

## What early reward is (and isn't)

Early reward is a float 0.0–1.0 computed immediately after the LLM responds. It is a **machine signal**, not a human judgment. It does not need to be perfect — it needs to be:

- **Deterministic** — same inputs, same output
- **Correlated with quality** for this specific task
- **Quality only** — never bake in cost or latency. Those are controlled separately via `cost_importance` and `latency_importance` on the bandit.

Early reward is often more honest as a **guardrail against known bad** than as a precise measure of good. Catching definite failures is enough to steer the bandit away from poor arms.

---

## Two questions

Ask these in order before writing any code.

### 1. Can you define good deterministically?

What does your task produce — and what observable property of that output signals success?

Start from the task itself:
- What does it produce? (SQL query, JSON blob, prose, citations, code)
- What does a successful output have that a failed one doesn't?
- Can you check that property in code? (execute it, parse it, regex it, check membership)

That check is your reward function. Examples of task-derived signals:
- Q&A against company docs → does the response cite a traceable source?
- Text-to-SQL → does the generated query execute without error?
- Research agent → how many distinct sources are cited?
- Structured output → does the JSON parse and contain required fields?
- Downstream action → did the user click through / convert?

If yes: use this as your positive signal.

### 2. Can you define bad deterministically?

What does your task produce when it fails — and can you detect it?

- Refusal ("I can't", "I'm unable", "as an AI")
- Empty or near-empty response
- Malformed output (unparseable, broken schema)
- Known failure markers specific to your task

If yes: use this as a guardrail floor — score `0.0` for known bad.

---

## Four positions

Work through both questions. Land in one of these:

| Position | Reward strategy |
|----------|----------------|
| Can define good | Deterministic positive signal |
| Can define bad, not good | Guardrail: `0.0` for bad, `1.0` for not-bad |
| Can define both | Combined: `0.0` for bad, scaled signal for quality |
| Can define neither | No early reward — human grading only via `bandito tui` |

**No early reward is valid.** Bandito still optimizes on cost and latency. Collect events, grade manually via the TUI, and revisit reward design once you have a sense of what separates good responses from bad.

---

## Sanity check before shipping

Run your reward function on 5–10 example responses:

- Does it return `0.0` for responses you'd call bad?
- Does it return `1.0` (or near) for responses you'd call good?
- Is the range meaningful, or does everything cluster at one value?

If everything scores `1.0`, the function isn't discriminating — tighten the definition of good or add a bad guardrail.

**Cost/latency tuning is separate.** Adjust `cost_importance` and `latency_importance` (0–5) on the bandit itself, not in the reward function.

---

## Language note

The derivation logic is identical in Python and JS/TS — only syntax differs. Use `string.includes()` / `Array.some()` / `JSON.parse()` try/catch in JS where you'd use `in` / `any()` / `json.loads()` in Python.

---

## ✅ Checkpoint D

**Cold start:** Thompson Sampling explores heavily at first — early leaderboard results are noise. Convergence takes hundreds of events with a meaningful reward signal, not dozens. Don't tune arm strategy on early traffic.

> Ship it and collect events. Come back once you have data for leaderboard monitoring and grading.
> When you have ≥ 20 human grades, we can set up LLM-as-judge to scale grading automatically.
