use anyhow::Result;
use std::fs;
use std::path::Path;

const SKILL_CONTENT: &str = include_str!("../../../skills/bandito/SKILL.md");

pub fn run() -> Result<()> {
    let dir = Path::new(".claude/skills/bandito");
    let file = dir.join("SKILL.md");

    let updated = file.exists();

    fs::create_dir_all(dir)?;
    fs::write(&file, SKILL_CONTENT)?;

    if updated {
        println!("Updated Claude Code skill at .claude/skills/bandito/SKILL.md");
    } else {
        println!("Installed Claude Code skill at .claude/skills/bandito/SKILL.md");
    }
    println!();
    println!("Usage: type /bandito in Claude Code to get help with:");
    println!("  - Onboarding and account setup");
    println!("  - SDK integration into your codebase");
    println!("  - Designing reward functions");
    println!("  - Interpreting leaderboard results");

    Ok(())
}
