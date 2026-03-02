use anyhow::Result;
use dialoguer::Select;
use std::fs;
use std::path::Path;

const SKILL_CONTENT: &str = include_str!("../../../skills/bandito/SKILL.md");

#[derive(Clone, Copy)]
enum AiTool {
    ClaudeCode,
    OpenCode,
    Codex,
}

impl AiTool {
    fn label(&self) -> &'static str {
        match self {
            AiTool::ClaudeCode => "Claude Code",
            AiTool::OpenCode => "OpenCode",
            AiTool::Codex => "Codex CLI",
        }
    }

    fn skill_dir(&self) -> &'static str {
        match self {
            AiTool::ClaudeCode => ".claude/skills/bandito",
            AiTool::OpenCode => ".opencode/skills/bandito",
            AiTool::Codex => ".agents/skills/bandito",
        }
    }

    fn invoke_hint(&self) -> &'static str {
        match self {
            AiTool::ClaudeCode => "type /bandito in Claude Code",
            AiTool::OpenCode => "type /bandito in OpenCode",
            AiTool::Codex => "mention $bandito in Codex CLI",
        }
    }
}

const TOOLS: [AiTool; 3] = [AiTool::ClaudeCode, AiTool::OpenCode, AiTool::Codex];

pub fn run() -> Result<()> {
    let selection = Select::new()
        .with_prompt("Which AI coding tool do you use?")
        .items(&TOOLS.map(|t| t.label()))
        .default(0)
        .interact()?;

    let tool = TOOLS[selection];
    let dir = Path::new(tool.skill_dir());
    let file = dir.join("SKILL.md");

    let updated = file.exists();

    fs::create_dir_all(dir)?;
    fs::write(&file, SKILL_CONTENT)?;

    if updated {
        println!("Updated {} skill at {}/SKILL.md", tool.label(), tool.skill_dir());
    } else {
        println!(
            "Installed {} skill at {}/SKILL.md",
            tool.label(),
            tool.skill_dir()
        );
    }
    println!();
    println!("Usage: {} to get help with:", tool.invoke_hint());
    println!("  - Onboarding and account setup");
    println!("  - SDK integration into your codebase");
    println!("  - Designing reward functions");
    println!("  - Interpreting leaderboard results");

    Ok(())
}
