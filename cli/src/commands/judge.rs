/// LLM-as-Judge commands.
///
/// Seven subcommands:
///   config      — Set judge API key and model interactively
///   gen-rubric  — Generate a quality rubric via LLM, save to bandit
///   edit-rubric — Open rubric in $EDITOR for manual editing
///   calibrate   — Score human-graded events, show confusion matrix (no writes)
///   augment     — Grade ungraded events, write to cloud (triggers Bayesian update)
///   review      — Show events with both human and judge grades, flag disagreements
///   status      — Show judge configuration and metrics for a bandit
use anyhow::{bail, Result};
use dialoguer::{Input, Password, Select};
use serde_json::json;
use std::io::{self, Write};
use std::time::Duration;
use std::thread;

use crate::commands::arm::resolve_bandit_id;
use crate::config::Config;
use crate::http::HttpClient;
use crate::judge_client;
use crate::store::{EventStore, JudgeEvent};

// Models shown in the interactive picker. Update this list with each CLI release
// so users always see current options without needing to know model names.
//
// Key by provider:
//   - Anthropic models (claude-*): need an Anthropic API key
//   - OpenAI models (gpt-*, o*): need an OpenAI API key
//   - OpenRouter models (contains "/"): need an OpenRouter key (openrouter.ai)
const KNOWN_MODELS: &[(&str, &str)] = &[
    // ── Anthropic (anthropic.com API key) ─────────────────────────────────
    ("claude-sonnet-4-6",           "Anthropic · balanced quality/cost (recommended)"),
    ("claude-opus-4-6",             "Anthropic · highest quality"),
    // ── OpenAI (openai.com API key) ───────────────────────────────────────
    ("gpt-4.1",                     "OpenAI · comparable to Claude Sonnet"),
    // ── OpenRouter (openrouter.ai API key) ────────────────────────────────
    ("minimax/minimax-m2.5",        "OpenRouter · MiniMax M2.5, top open-source"),
    ("moonshotai/kimi-k2.5",        "OpenRouter · Kimi K2.5, top open-source"),
];
use crate::util::truncate;


// ── config ────────────────────────────────────────────────────────────────

pub fn config_judge() -> Result<()> {
    let mut config = Config::load()?;

    println!("Configure LLM judge\n");
    println!("API key needed depends on the model you choose:");
    println!("  OpenAI models (gpt-*, o*)       → openai.com API key");
    println!("  Anthropic models (claude-*)      → anthropic.com API key");
    println!("  OpenRouter models (contains '/') → openrouter.ai API key\n");

    // API key — allow empty to keep the existing value
    let key_hint = if config.judge.api_key.is_empty() {
        "(none set)".to_string()
    } else {
        let preview = &config.judge.api_key[..8.min(config.judge.api_key.len())];
        format!("{}... (press Enter to keep)", preview)
    };
    println!("Current key: {}", key_hint);
    let new_key = Password::new()
        .with_prompt("Judge API key")
        .allow_empty_password(true)
        .interact()?;
    if !new_key.is_empty() {
        config.judge.api_key = new_key;
    }

    // Model — select from list or enter custom
    let labels: Vec<String> = KNOWN_MODELS
        .iter()
        .map(|(id, desc)| format!("{:<35} {}", id, desc))
        .chain(std::iter::once("Enter model name manually...".to_string()))
        .collect();

    // Pre-select the currently configured model if it's in the list
    let default_idx = KNOWN_MODELS
        .iter()
        .position(|(id, _)| *id == config.judge.model.as_str())
        .unwrap_or(0);

    let selection = Select::new()
        .with_prompt("Model")
        .items(&labels)
        .default(default_idx)
        .interact()?;

    config.judge.model = if selection == labels.len() - 1 {
        Input::<String>::new()
            .with_prompt("Model name")
            .with_initial_text(&config.judge.model)
            .interact_text()?
    } else {
        KNOWN_MODELS[selection].0.to_string()
    };

    config.save()?;

    println!("\nJudge config saved.");
    println!("  model:   {}", config.judge.model);
    if !config.judge.api_key.is_empty() {
        let preview = &config.judge.api_key[..8.min(config.judge.api_key.len())];
        println!("  api_key: {}...", preview);
    }

    Ok(())
}

// ── Feature unlock guard ──────────────────────────────────────────────────

/// Abort if bandit has < 20 human grades. Warn if grade ratio < 5%.
fn check_judge_eligibility(http: &HttpClient, bandit_id: i64) -> Result<()> {
    let resp = http.get(&format!("/analytics/{}/arms/performance", bandit_id), &[])?;
    let arms = resp["arms"].as_array().cloned().unwrap_or_default();

    let total_events: i64 = arms
        .iter()
        .map(|a| a["event_count"].as_i64().unwrap_or(0))
        .sum();
    let graded_count: i64 = arms
        .iter()
        .map(|a| a["graded_count"].as_i64().unwrap_or(0))
        .sum();

    if graded_count < 20 {
        bail!(
            "Not enough human grades to use LLM-as-judge.\n\
             Requires: 20 human grades\n\
             Currently: {} human grade{}\n\n\
             Grade events in the TUI first: bandito tui",
            graded_count,
            if graded_count == 1 { "" } else { "s" }
        );
    }

    let grade_ratio = if total_events > 0 {
        graded_count as f64 / total_events as f64
    } else {
        0.0
    };

    if grade_ratio < 0.05 {
        eprintln!(
            "Warning: only {:.1}% of events are human-graded ({}/{}).\n\
             Judge accuracy may be unreliable. Consider grading more events.",
            grade_ratio * 100.0,
            graded_count,
            total_events
        );
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn parse_judge_score(raw: &str) -> Option<(f64, String)> {
    // Try to find JSON in the response (model may wrap it in prose)
    let start = raw.find('{')?;
    let end = raw.rfind('}').map(|i| i + 1)?;
    let json_str = &raw[start..end];
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let score = v["score"].as_f64()?;
    let reason = v["reason"].as_str().unwrap_or("").to_string();
    Some((score.clamp(0.0, 1.0), reason))
}

/// Build the per-event evaluation prompt, optionally including reference examples.
fn build_eval_prompt(rubric: &str, query: &str, response: &str, examples: &[&JudgeEvent]) -> String {
    let mut out = format!("RUBRIC:\n{}\n", rubric);

    if !examples.is_empty() {
        out.push_str("\nREFERENCE EXAMPLES (human-graded):\n");
        for ex in examples {
            let score = ex.human_reward.unwrap_or(0.0);
            let label = if score >= 0.5 { "GOOD" } else { "BAD" };
            let q = truncate(ex.query_text.as_deref().unwrap_or(""), 300);
            let r = truncate(ex.response.as_deref().unwrap_or(""), 500);
            out.push_str(&format!(
                "\n[{} \u{2014} score {:.2}]\nQuery: {}\nResponse: {}\n",
                label, score, q, r
            ));
        }
        out.push_str("\nNOW EVALUATE:\n");
    }

    out.push_str(&format!(
        "\nUSER QUERY:\n{}\n\nASSISTANT RESPONSE:\n{}\n\n\
         Score this response 0.0\u{2013}1.0 based on the rubric{}.\n\
         Reply with ONLY valid JSON: {{\"score\": 0.75, \"reason\": \"one sentence\"}}",
        query,
        response,
        if examples.is_empty() { "" } else { " and examples above" }
    ));

    out
}

/// Pick up to 2 good + 1 bad reference examples from a pool, excluding one UUID.
/// Returns refs in order: [good1, good2, bad1].
fn pick_examples<'a>(pool: &'a [JudgeEvent], exclude_uuid: &str) -> Vec<&'a JudgeEvent> {
    let mut eligible: Vec<&JudgeEvent> = pool
        .iter()
        .filter(|e| {
            e.uuid != exclude_uuid
                && e.query_text.is_some()
                && e.response.is_some()
                && e.human_reward.is_some()
        })
        .collect();
    eligible.sort_by(|a, b| {
        b.human_reward
            .partial_cmp(&a.human_reward)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result = Vec::new();
    // Top 2 as good examples
    for e in eligible.iter().take(2) {
        result.push(*e);
    }
    // Bottom 1 as bad (only if distinct from good ones)
    if eligible.len() > 2 {
        if let Some(bad) = eligible.last() {
            result.push(*bad);
        }
    }
    result
}

fn confirm(prompt: &str) -> Result<char> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().chars().next().unwrap_or('n'))
}

fn open_editor(content: &str) -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let mut tmp = std::env::temp_dir();
    tmp.push("bandito_rubric.txt");
    std::fs::write(&tmp, content)?;
    std::process::Command::new(&editor).arg(&tmp).status()?;
    Ok(std::fs::read_to_string(&tmp)?)
}

// ── gen-rubric ────────────────────────────────────────────────────────────

pub fn gen_rubric(bandit_name: &str) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    if config.judge.api_key.is_empty() {
        bail!("No judge API key set. Set JUDGE_API_KEY env var or add [judge] api_key = \"...\" to ~/.bandito/config.toml");
    }

    let http = HttpClient::from_config(&config)?;
    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    let bandit_resp = http.get(&format!("/bandits/{}", bandit_id), &[])?;
    let reported_name = bandit_resp["name"].as_str().unwrap_or(bandit_name).to_string();
    let current_version = bandit_resp["judge_rubric_version"].as_i64().unwrap_or(0);

    // Load graded events from local SQLite — system prompt + examples come from here
    let store = match EventStore::open()? {
        Some(s) => s,
        None => bail!(
            "No local event store found (~/.bandito/events.db).\n\
             Grade some events in the TUI first: bandito tui"
        ),
    };
    let graded = store.get_graded_events(bandit_id, 20)?;
    if graded.is_empty() {
        bail!(
            "No locally-graded events found for \"{}\".\n\
             Grade some events in the TUI first — rubric generation uses real \
             examples to calibrate quality.",
            bandit_name
        );
    }

    // Use system prompt from the highest-reward event as task context
    let best = graded
        .iter()
        .max_by(|a, b| {
            a.human_reward
                .partial_cmp(&b.human_reward)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();
    let system_prompt_ctx = best.system_prompt.as_deref().unwrap_or("(none)");

    // Pick 2 good + 1 bad examples
    let examples = pick_examples(&graded, "");

    // Build rubric-generation prompt
    let mut examples_text = String::new();
    for ex in &examples {
        let score = ex.human_reward.unwrap_or(0.0);
        let label = if score >= 0.5 { "GOOD" } else { "BAD" };
        let q = truncate(ex.query_text.as_deref().unwrap_or(""), 300);
        let r = truncate(ex.response.as_deref().unwrap_or(""), 500);
        examples_text.push_str(&format!(
            "\n[{} \u{2014} human score {:.2}]\nQuery: {}\nResponse: {}\n",
            label, score, q, r
        ));
    }

    let generation_prompt = format!(
        "You are creating a quality rubric for an LLM-as-judge.\n\
         \n\
         Task context \u{2014} {}:\n{}\n\
         \n\
         Below are real human-graded response examples. Use these to calibrate \
         your understanding of what quality looks like for this task.\n\
         {}\n\
         Write a rubric of 5\u{2013}7 specific, measurable criteria that define \
         what a high-quality response looks like. Focus on task outcomes and \
         response quality \u{2014} not adherence to any particular writing style \
         or prompt wording. Each criterion should make clear what distinguishes \
         a good response from a poor one.",
        reported_name, system_prompt_ctx, examples_text
    );

    println!("Generating rubric for \"{}\"...", reported_name);
    let draft = judge_client::call_judge(&config.judge.api_key, &config.judge.model, &generation_prompt)?;

    println!("\n{}\n", "─".repeat(60));
    println!("{}", draft);
    println!("{}\n", "─".repeat(60));

    let choice = confirm("Save this rubric? [y/N/e(dit)] ")?;
    let final_rubric = match choice {
        'y' | 'Y' => draft.clone(),
        'e' | 'E' => {
            let edited = open_editor(&draft)?;
            println!("\nEdited rubric:\n{}\n{}\n", "─".repeat(60), edited);
            let confirm_edited = confirm("Save edited rubric? [y/N] ")?;
            if confirm_edited != 'y' && confirm_edited != 'Y' {
                println!("Aborted.");
                return Ok(());
            }
            edited
        }
        _ => {
            println!("Aborted.");
            return Ok(());
        }
    };

    http.patch_json(
        &format!("/bandits/{}", bandit_id),
        &json!({
            "judge_rubric": final_rubric,
            "judge_rubric_version": current_version + 1,
            "judge_enabled": true,
        }),
    )?;

    println!("Rubric saved (v{}).", current_version + 1);
    Ok(())
}

// ── edit-rubric ───────────────────────────────────────────────────────────

pub fn edit_rubric(bandit_name: &str) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }

    let http = HttpClient::from_config(&config)?;
    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    let bandit_resp = http.get(&format!("/bandits/{}", bandit_id), &[])?;
    let current_rubric = bandit_resp["judge_rubric"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let current_version = bandit_resp["judge_rubric_version"].as_i64().unwrap_or(0);

    if current_rubric.is_empty() {
        bail!("No rubric set for \"{}\". Run `bandito judge gen-rubric {}` first.", bandit_name, bandit_name);
    }

    let edited = open_editor(&current_rubric)?;

    println!("\nEdited rubric:\n{}\n{}\n", "─".repeat(60), edited);
    let choice = confirm("Save edited rubric? [y/N] ")?;
    if choice != 'y' && choice != 'Y' {
        println!("Aborted.");
        return Ok(());
    }

    http.patch_json(
        &format!("/bandits/{}", bandit_id),
        &json!({
            "judge_rubric": edited,
            "judge_rubric_version": current_version + 1,
        }),
    )?;

    println!("Rubric updated (v{}).", current_version + 1);
    Ok(())
}

// ── calibrate ────────────────────────────────────────────────────────────

pub fn calibrate(bandit_name: &str, sample: usize) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    if config.judge.api_key.is_empty() {
        bail!("No judge API key set. Set JUDGE_API_KEY env var or add [judge] api_key = \"...\" to ~/.bandito/config.toml");
    }

    let http = HttpClient::from_config(&config)?;
    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    let bandit_resp = http.get(&format!("/bandits/{}", bandit_id), &[])?;
    let rubric = bandit_resp["judge_rubric"].as_str().unwrap_or("").to_string();
    if rubric.is_empty() {
        bail!("No rubric set for \"{}\". Run `bandito judge gen-rubric {}` first.", bandit_name, bandit_name);
    }
    let rubric_version = bandit_resp["judge_rubric_version"].as_i64().unwrap_or(0);

    check_judge_eligibility(&http, bandit_id)?;

    // Load graded events from local SQLite
    let store = match EventStore::open()? {
        Some(s) => s,
        None => bail!("No local event store found (~/.bandito/events.db).\nCalibration requires locally stored events. Make sure the SDK has been used on this machine."),
    };

    // Compute per-arm limit
    let arm_resp = http.get(&format!("/bandits/{}/arms", bandit_id), &[])?;
    let arm_count = arm_resp["items"].as_array().map(|a| a.len()).unwrap_or(1).max(1);
    let limit_per_arm = (sample / arm_count).max(1);

    let events = store.get_graded_events(bandit_id, limit_per_arm)?;

    if events.is_empty() {
        bail!(
            "No locally-graded events found for \"{}\".\n\
             Calibration uses events graded via `bandito tui`. \
             Grade some events in the TUI first.",
            bandit_name
        );
    }

    println!(
        "Calibrating judge for \"{}\" ({} events, rubric v{})...",
        bandit_name,
        events.len(),
        rubric_version
    );

    // Score each event
    let threshold = 0.5_f64;
    let mut tp = 0_i64;
    let mut tn = 0_i64;
    let mut fp = 0_i64;
    let mut fn_ = 0_i64;
    let mut errors = 0_usize;

    for (i, evt) in events.iter().enumerate() {
        let query = evt.query_text.as_deref().unwrap_or("(no query)");
        let response = evt.response.as_deref().unwrap_or("(no response)");
        let human_reward = evt.human_reward.unwrap_or(0.0);

        let examples = pick_examples(&events, &evt.uuid);
        let prompt = build_eval_prompt(&rubric, query, response, &examples);

        match judge_client::call_judge(&config.judge.api_key, &config.judge.model, &prompt) {
            Ok(raw) => {
                if let Some((score, _reason)) = parse_judge_score(&raw) {
                    let judge_pass = score >= threshold;
                    let human_pass = human_reward >= threshold;
                    match (judge_pass, human_pass) {
                        (true, true) => tp += 1,
                        (false, false) => tn += 1,
                        (true, false) => fp += 1,
                        (false, true) => fn_ += 1,
                    }
                    print!("\r[{}/{}] scored {:.2}", i + 1, events.len(), score);
                    io::stdout().flush().ok();
                } else {
                    errors += 1;
                    eprintln!("\n[{}/{}] Failed to parse score from: {}", i + 1, events.len(), truncate(&raw, 80));
                }
            }
            Err(e) => {
                errors += 1;
                eprintln!("\n[{}/{}] LLM call failed: {}", i + 1, events.len(), e);
            }
        }
    }
    println!();

    // Print confusion matrix
    let n = tp + tn + fp + fn_;
    let tpr = if tp + fn_ > 0 { Some(tp as f64 / (tp + fn_) as f64) } else { None };
    let tnr = if tn + fp > 0 { Some(tn as f64 / (tn + fp) as f64) } else { None };
    let precision = if tp + fp > 0 { Some(tp as f64 / (tp + fp) as f64) } else { None };
    let f1 = match (precision, tpr) {
        (Some(p), Some(r)) if p + r > 0.0 => Some(2.0 * p * r / (p + r)),
        _ => None,
    };

    println!();
    println!(
        "Judge Calibration  \u{2014}  {} (rubric v{}, n={})",
        bandit_name, rubric_version, n
    );
    println!("{}", "\u{2500}".repeat(54));
    println!("{:<18} {:>10}  {:>10}", "", "Judge PASS", "Judge FAIL");
    println!(
        "{:<18} {:>10}  {:>10}    TPR: {}",
        "Human PASS",
        tp,
        fn_,
        tpr.map(|v| format!("{:.1}%", v * 100.0)).unwrap_or_else(|| "n/a".into())
    );
    println!(
        "{:<18} {:>10}  {:>10}    TNR: {}",
        "Human FAIL",
        fp,
        tn,
        tnr.map(|v| format!("{:.1}%", v * 100.0)).unwrap_or_else(|| "n/a".into())
    );
    println!();
    println!(
        "Precision: {}   Recall: {}   F1: {}",
        precision.map(|v| format!("{:.1}%", v * 100.0)).unwrap_or_else(|| "n/a".into()),
        tpr.map(|v| format!("{:.1}%", v * 100.0)).unwrap_or_else(|| "n/a".into()),
        f1.map(|v| format!("{:.1}%", v * 100.0)).unwrap_or_else(|| "n/a".into()),
    );

    if errors > 0 {
        println!("\n{} event(s) could not be scored (see errors above).", errors);
    }

    Ok(())
}

// ── augment ───────────────────────────────────────────────────────────────

pub fn augment(bandit_name: &str, sample: usize) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    if config.judge.api_key.is_empty() {
        bail!("No judge API key set. Set JUDGE_API_KEY env var or add [judge] api_key = \"...\" to ~/.bandito/config.toml");
    }

    let http = HttpClient::from_config(&config)?;
    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    let bandit_resp = http.get(&format!("/bandits/{}", bandit_id), &[])?;
    let rubric = bandit_resp["judge_rubric"].as_str().unwrap_or("").to_string();
    if rubric.is_empty() {
        bail!("No rubric set for \"{}\". Run `bandito judge gen-rubric {}` first.", bandit_name, bandit_name);
    }
    let rubric_version = bandit_resp["judge_rubric_version"].as_i64().unwrap_or(0);

    check_judge_eligibility(&http, bandit_id)?;

    let store = match EventStore::open()? {
        Some(s) => s,
        None => bail!("No local event store found (~/.bandito/events.db).\nAugment requires locally stored events."),
    };

    let arm_resp = http.get(&format!("/bandits/{}/arms", bandit_id), &[])?;
    let arm_count = arm_resp["items"].as_array().map(|a| a.len()).unwrap_or(1).max(1);
    let limit_per_arm = (sample / arm_count).max(1);

    let events = store.get_ungraded_events(bandit_id, limit_per_arm)?;

    if events.is_empty() {
        println!("No ungraded flushed events found for \"{}\". Nothing to augment.", bandit_name);
        return Ok(());
    }

    // Load a small set of graded events to use as reference examples in the eval prompt
    let example_pool = store.get_graded_events(bandit_id, 5).unwrap_or_default();
    let aug_examples = pick_examples(&example_pool, "");

    println!(
        "Augmenting {} events for \"{}\" (rubric v{}, model: {}).",
        events.len(),
        bandit_name,
        rubric_version,
        config.judge.model
    );
    let choice = confirm(&format!("Proceed? [y/N] "))?;
    if choice != 'y' && choice != 'Y' {
        println!("Aborted.");
        return Ok(());
    }

    let mut written = 0_usize;
    let mut errors = 0_usize;
    let mut arm_counts: std::collections::HashMap<i64, (usize, f64)> = std::collections::HashMap::new();

    for (i, evt) in events.iter().enumerate() {
        let query = evt.query_text.as_deref().unwrap_or("(no query)");
        let response = evt.response.as_deref().unwrap_or("(no response)");
        let prompt = build_eval_prompt(&rubric, query, response, &aug_examples);

        match judge_client::call_judge(&config.judge.api_key, &config.judge.model, &prompt) {
            Ok(raw) => {
                if let Some((score, reason)) = parse_judge_score(&raw) {
                    // PATCH grade to cloud — triggers Bayesian update
                    let patch_body = json!({
                        "grade": score,
                        "grade_source": "judge",
                        "is_graded": true,
                    });
                    match http.patch_json(&format!("/events/{}/grade", evt.uuid), &patch_body) {
                        Ok(_) => {
                            written += 1;
                            let entry = arm_counts.entry(evt.arm_id).or_insert((0, 0.0));
                            entry.0 += 1;
                            entry.1 += score;
                            println!(
                                "[{}/{}] arm:{} score={:.2} — {}",
                                i + 1, events.len(), evt.arm_id, score,
                                truncate(&reason, 60)
                            );
                        }
                        Err(e) => {
                            errors += 1;
                            eprintln!("[{}/{}] PATCH failed for {}: {}", i + 1, events.len(), evt.uuid, e);
                        }
                    }
                } else {
                    errors += 1;
                    eprintln!("[{}/{}] Failed to parse score: {}", i + 1, events.len(), truncate(&raw, 80));
                }
            }
            Err(e) => {
                errors += 1;
                eprintln!("[{}/{}] LLM call failed: {}", i + 1, events.len(), e);
            }
        }

        // Rate limit: avoid hammering the API
        thread::sleep(Duration::from_millis(100));
    }

    println!("\nDone. {} grade(s) written.", written);

    if !arm_counts.is_empty() {
        println!("\nArm breakdown:");
        let mut arms: Vec<_> = arm_counts.iter().collect();
        arms.sort_by_key(|(id, _)| *id);
        for (arm_id, (count, total_score)) in arms {
            println!(
                "  arm {:>4}: {:>3} events  avg score {:.2}",
                arm_id,
                count,
                total_score / *count as f64
            );
        }
    }

    if errors > 0 {
        println!("\n{} event(s) failed (see errors above).", errors);
    }

    Ok(())
}

// ── review ────────────────────────────────────────────────────────────────

pub fn review(bandit_name: &str, disagreements_only: bool) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }

    let http = HttpClient::from_config(&config)?;
    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    // Fetch events from cloud, paginated
    let mut all_events = Vec::new();
    let mut offset = 0_usize;
    let limit = 100_usize;

    loop {
        let resp = http.get(
            "/events",
            &[
                ("bandit_id", &bandit_id.to_string()),
                ("has_judge_grade", "true"),
                ("has_grade", "true"),
                ("limit", &limit.to_string()),
                ("offset", &offset.to_string()),
            ],
        )?;
        let items = resp["items"].as_array().cloned().unwrap_or_default();
        let total = resp["total"].as_u64().unwrap_or(0) as usize;
        all_events.extend(items);
        offset += limit;
        if offset >= total {
            break;
        }
    }

    // Server already filtered for has_grade + has_judge_grade.
    // Client-side: confirm grade was set by a human (not a prior judge run).
    let mut filtered: Vec<_> = all_events
        .iter()
        .filter(|e| e["grade_source"].as_str() == Some("human"))
        .collect();

    if disagreements_only {
        filtered.retain(|e| {
            let grade = e["grade"].as_f64().unwrap_or(0.0);
            let judge_grade = e["judge_grade"].as_f64().unwrap_or(0.0);
            (grade - judge_grade).abs() >= 0.5
        });
    }

    if filtered.is_empty() {
        if disagreements_only {
            println!("No disagreements found for \"{}\".", bandit_name);
        } else {
            println!(
                "No events with both human and judge grades found for \"{}\".",
                bandit_name
            );
        }
        return Ok(());
    }

    let mode = if disagreements_only { "disagreements only" } else { "all" };
    println!(
        "Review \u{2014} {} [{}]  ({} events)",
        bandit_name, mode, filtered.len()
    );
    println!("{}", "\u{2500}".repeat(80));
    println!(
        "{:<12} {:<32} {:>6}  {:>6}  {:>6}",
        "UUID", "Query", "Human", "Judge", "Delta"
    );
    println!("{}", "\u{2500}".repeat(80));

    for e in &filtered {
        let uuid = e["local_event_uuid"].as_str().unwrap_or("?");
        let query = e["query_text"].as_str().unwrap_or("(none)");
        let grade = e["grade"].as_f64().unwrap_or(0.0);
        let judge_grade = e["judge_grade"].as_f64().unwrap_or(0.0);
        let delta = grade - judge_grade;

        println!(
            "{:<12} {:<32} {:>6.2}  {:>6.2}  {:>+6.2}",
            truncate(uuid, 12),
            truncate(query, 32),
            grade,
            judge_grade,
            delta
        );
    }

    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────

pub fn status(bandit_name: &str) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }

    let http = HttpClient::from_config(&config)?;
    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    let bandit_resp = http.get(&format!("/bandits/{}", bandit_id), &[])?;
    let judge_enabled = bandit_resp["judge_enabled"].as_bool().unwrap_or(false);
    let judge_model = bandit_resp["judge_model"].as_str().unwrap_or("not set");
    let judge_rubric_version = bandit_resp["judge_rubric_version"].as_i64().unwrap_or(0);
    let judge_rubric = bandit_resp["judge_rubric"].as_str().unwrap_or("");

    // Try to get judge metrics (may 404 if no data)
    let metrics = http.get(&format!("/analytics/{}/judge-metrics", bandit_id), &[]).ok();

    // Local store counts
    let (local_ungraded, local_graded) = match EventStore::open()? {
        Some(store) => {
            let ungraded = store.get_ungraded_events(bandit_id, usize::MAX)?.len();
            let graded = store.get_graded_events(bandit_id, usize::MAX)?.len();
            (ungraded, graded)
        }
        None => (0, 0),
    };

    println!("Judge Status \u{2014} {}", bandit_name);
    println!("{}", "\u{2500}".repeat(40));
    println!("Enabled:          {}", if judge_enabled { "yes" } else { "no" });
    println!("Model:            {}", judge_model);
    println!("Rubric version:   {}", judge_rubric_version);
    if !judge_rubric.is_empty() {
        println!("Rubric preview:   {}", truncate(judge_rubric, 60));
    } else {
        println!("Rubric:           (none — run `bandito judge gen-rubric {}` to create)", bandit_name);
    }
    println!();
    println!("Local events:");
    println!("  Ungraded (augment candidates): {}", local_ungraded);
    println!("  Human-graded (calibrate set):  {}", local_graded);

    if let Some(m) = metrics {
        println!();
        println!("Cloud judge metrics (human-vs-judge comparison):");
        println!("  Sample size: {}", m["sample_size"].as_i64().unwrap_or(0));
        let fmt_opt = |v: &serde_json::Value| {
            v.as_f64()
                .map(|f| format!("{:.1}%", f * 100.0))
                .unwrap_or_else(|| "n/a".into())
        };
        println!("  TPR:         {}", fmt_opt(&m["tpr"]));
        println!("  TNR:         {}", fmt_opt(&m["tnr"]));
        println!("  Precision:   {}", fmt_opt(&m["precision"]));
        println!("  F1:          {}", fmt_opt(&m["f1"]));
    } else {
        println!();
        println!("Cloud judge metrics: not available (no human re-grades of judge-graded events yet)");
    }

    Ok(())
}
