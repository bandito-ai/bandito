use anyhow::{bail, Result};
use std::thread;
use std::time::Duration;

use crate::commands::arm::resolve_bandit_id;
use crate::config::Config;
use crate::http::HttpClient;
use crate::util::truncate;

pub fn run(bandit_name: &str, graded: bool, watch: bool) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    let http = HttpClient::from_config(&config)?;
    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    if watch {
        loop {
            print!("\x1b[2J\x1b[H");
            match render_leaderboard(&http, bandit_name, bandit_id, graded) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Refresh failed: {}", e);
                    eprintln!("Retrying in 30s...");
                }
            }
            println!("\nAuto-refreshing every 30s. Press Ctrl+C to stop.");
            thread::sleep(Duration::from_secs(30));
        }
    } else {
        render_leaderboard(&http, bandit_name, bandit_id, graded)?;
    }

    Ok(())
}

fn convergence_line(arms: &[serde_json::Value], total_events: i64) -> String {
    const MIN_EVENTS: i64 = 20;

    if total_events == 0 {
        return "No events yet — make sure your app is calling bandito.update().".to_string();
    }
    if total_events < MIN_EVENTS {
        return format!(
            "Too few events to assess ({}/{} minimum).",
            total_events, MIN_EVENTS
        );
    }

    // Find the leading arm by pull_share
    let leader = arms
        .iter()
        .max_by(|a, b| {
            let sa = a["pull_share"].as_f64().unwrap_or(0.0);
            let sb = b["pull_share"].as_f64().unwrap_or(0.0);
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        });

    let Some(leader) = leader else {
        return "No arm data.".to_string();
    };

    let share = leader["pull_share"].as_f64().unwrap_or(0.0);
    let model = leader["model_name"].as_str().unwrap_or("?");
    let pct = share * 100.0;

    if share >= 0.80 {
        format!(
            "Converged -> {} ({:.0}% of traffic). The bandit has high confidence in this arm.",
            model, pct
        )
    } else if share >= 0.60 {
        format!(
            "Converging -> {} ({:.0}% of traffic). Keep collecting events to confirm.",
            model, pct
        )
    } else {
        format!(
            "Still exploring — no clear winner yet ({:.0}% leading). Need more events or a stronger reward signal.",
            pct
        )
    }
}

fn render_leaderboard(
    http: &HttpClient,
    bandit_name: &str,
    bandit_id: i64,
    graded: bool,
) -> Result<()> {
    let resp = http.get(
        &format!("/analytics/{}/arms/performance", bandit_id),
        &[],
    )?;

    let total_events = resp["total_events"].as_i64().unwrap_or(0);
    let arms = resp["arms"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Unexpected response format"))?;

    // Header
    println!("{} ({} events)\n", bandit_name, total_events);

    // Convergence signal
    println!("Status: {}\n", convergence_line(arms, total_events));

    if arms.is_empty() {
        println!("No data yet. Make sure your app is calling bandito.update() after each LLM call.");
        return Ok(());
    }

    // Column widths:  Arm(25)  Pulls(6)  Pull%(6)  Reward(7)  Grade(6)  Graded%(8)  Cost(9)  Latency(10)
    println!(
        "{:<25} {:>6} {:>6} {:>7} {:>6} {:>7} {:>9} {:>10}",
        "Arm", "Pulls", "Pull%", "Reward", "Grade", "Graded%", "Avg Cost", "Avg Latency"
    );
    println!("{}", "-".repeat(85));

    let mut rows_printed = 0;
    for arm in arms {
        let model = arm["model_name"].as_str().unwrap_or("?");
        let event_count = arm["event_count"].as_i64().unwrap_or(0);
        let pull_share = arm["pull_share"].as_f64().unwrap_or(0.0);
        let avg_reward = arm["avg_reward"].as_f64();
        let avg_grade = arm["avg_grade"].as_f64();
        let graded_count = arm["graded_count"].as_i64().unwrap_or(0);

        // graded_ratio: prefer field from API, fall back to computing it
        let graded_ratio = arm["graded_ratio"]
            .as_f64()
            .unwrap_or_else(|| {
                if event_count > 0 {
                    graded_count as f64 / event_count as f64
                } else {
                    0.0
                }
            });

        let avg_cost = arm["avg_cost"].as_f64();
        let avg_latency = arm["avg_latency"].as_f64();

        // --graded: skip arms with no human grades
        if graded && graded_count == 0 {
            continue;
        }

        println!(
            "{:<25} {:>6} {:>5.1}% {:>7} {:>6} {:>6.0}% {:>9} {:>10}",
            truncate(model, 25),
            event_count,
            pull_share * 100.0,
            format_opt_f64(avg_reward, 2),
            format_opt_f64(avg_grade, 2),
            graded_ratio * 100.0,
            format_cost(avg_cost),
            format_latency(avg_latency),
        );
        rows_printed += 1;
    }

    if graded && rows_printed == 0 {
        println!("No graded events yet. Grade responses with `bandito tui`.");
    }

    Ok(())
}

fn format_opt_f64(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(val) => format!("{:.prec$}", val, prec = decimals),
        None => "-".to_string(),
    }
}

fn format_cost(v: Option<f64>) -> String {
    match v {
        Some(c) => format!("${:.4}", c),
        None => "-".to_string(),
    }
}

fn format_latency(v: Option<f64>) -> String {
    match v {
        Some(ms) => format!("{:.0}ms", ms),
        None => "-".to_string(),
    }
}
