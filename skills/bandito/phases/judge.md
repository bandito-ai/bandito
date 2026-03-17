# Phase 5: LLM-as-Judge

Scale grading beyond what humans can do manually. The judge reads your rubric and scores events the same way a human would — feeding grades back into Bayesian updates.

## Prerequisites

- **≥ 20 human grades** — the CLI will block `judge` commands below this threshold
- **Warning at < 5% grade ratio** — judge accuracy degrades when human signal is sparse relative to total events; grade more events in the TUI first

---

## Workflow

```
judge config → gen-rubric → calibrate → augment → review → status
```

Never skip `calibrate` before `augment`. Running the judge without validating rubric quality wastes API calls and may push bad grades into Bayesian state.

---

## Step 1: Configure the judge

> **Interactive — pause here.** Ask the user to run this.

```bash
bandito judge config
```

Interactive picker for:
- **Model** — Claude Sonnet (recommended, balanced), Claude Opus (highest quality), GPT-4.1, or OpenRouter models. Anthropic models need an Anthropic API key; OpenAI models need an OpenAI key; OpenRouter models (containing `/`) need an OpenRouter key.
- **API key** — stored under `[judge]` in `~/.bandito/config.toml`. Also settable via `JUDGE_API_KEY` env var.

---

## Step 2: Generate a rubric

```bash
bandito judge gen-rubric my-chatbot
```

The CLI:
1. Loads graded events from local SQLite
2. Picks 2 good + 1 bad example to calibrate quality
3. Calls the judge LLM to generate 5–7 specific, measurable quality criteria
4. Presents the draft and asks: save / discard / open in `$EDITOR`

**Requires locally-stored events** — `data_storage = "local"` (default) or events graded via `bandito tui`.

After saving, the rubric is stored on the bandit in the cloud. Edit it at any time:
```bash
bandito judge edit-rubric my-chatbot    # opens in $EDITOR
```

**What makes a good rubric:**
- 5–7 criteria, each specific and measurable
- Focuses on task outcomes, not writing style
- Each criterion distinguishes a good response from a poor one
- Avoids vague terms like "helpful" or "clear" without defining them

---

## Step 3: Calibrate (dry run — no writes)

```bash
bandito judge calibrate my-chatbot              # default: 50 events
bandito judge calibrate my-chatbot --sample 100
```

Scores a sample of your human-graded events with the LLM judge and prints a confusion matrix:

```
Judge Calibration — my-chatbot (rubric v1, n=47)
──────────────────────────────────────────────────────
                   Judge PASS    Judge FAIL
Human PASS               38             3    TPR: 92.7%
Human FAIL                2             4    TNR: 66.7%

Precision: 95.0%   Recall: 92.7%   F1: 93.8%
```

**Calibration thresholds:**
- **F1 ≥ 0.75** — proceed to augment
- **F1 0.60–0.75** — edit the rubric, re-calibrate before augmenting
- **F1 < 0.60** — rubric needs significant revision; don't augment yet

Low TNR (judge passes things humans failed) = rubric is too lenient. Low TPR (judge fails things humans passed) = rubric is too strict.

---

## Step 4: Augment (writes to cloud)

```bash
bandito judge augment my-chatbot               # default: 100 events
bandito judge augment my-chatbot --sample 200
```

**Cost note:** Each `augment` call uses your judge LLM API. With `--sample 200` and a high-capability model, this can cost several dollars. Start with `--sample 50` using Sonnet before scaling up.

Grades ungraded events with the LLM judge. For each event:
1. Builds eval prompt from rubric + 2–3 human-graded reference examples
2. Calls the judge LLM
3. PATCHes grade to cloud → triggers Bayesian update

Prints per-event scores and a per-arm breakdown on completion.

Rate-limited internally (100ms between calls) to avoid hammering the judge API.

---

## Step 5: Review disagreements

```bash
bandito judge review my-chatbot
bandito judge review my-chatbot --disagreements    # only |human - judge| >= 0.5
```

Shows a table of events with both human and judge grades. Use `--disagreements` to find calibration gaps — these are the highest-leverage events to re-grade manually and use to improve the rubric.

---

## Step 6: Check status

```bash
bandito judge status my-chatbot
```

Shows: judge enabled/model, rubric version + preview, local event counts (augment candidates, calibration set), and cloud judge metrics (TPR/TNR/Precision/F1 from human re-grades of judge-graded events).

---

## Judge iteration loop

```
grade events in TUI (≥ 20)
  → judge config
  → gen-rubric
  → calibrate
      F1 < 0.75? → edit-rubric → calibrate again
  → augment
  → review --disagreements
      patterns found? → edit-rubric → calibrate → augment again
  → status (track improvement over rubric versions)
```

The rubric improves through iteration. Each `augment` run scales grading to events the TUI never would have reached.
