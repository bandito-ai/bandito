use anyhow::Result;
use dialoguer::Select;
use std::fs;
use std::path::Path;

const SKILL_MD: &str = include_str!("../../../skills/bandito/SKILL.md");
const PHASE_ONBOARDING: &str = include_str!("../../../skills/bandito/phases/onboarding.md");
const PHASE_INTEGRATION: &str = include_str!("../../../skills/bandito/phases/integration.md");
const PHASE_REWARD_DESIGN: &str = include_str!("../../../skills/bandito/phases/reward-design.md");
const PHASE_OPERATIONS: &str = include_str!("../../../skills/bandito/phases/operations.md");
const PHASE_JUDGE: &str = include_str!("../../../skills/bandito/phases/judge.md");
const REF_CLI: &str = include_str!("../../../skills/bandito/references/cli-reference.md");

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
    let updated = dir.join("SKILL.md").exists();

    fs::create_dir_all(dir.join("phases"))?;
    fs::create_dir_all(dir.join("references"))?;

    fs::write(dir.join("SKILL.md"), SKILL_MD)?;
    fs::write(dir.join("phases/onboarding.md"), PHASE_ONBOARDING)?;
    fs::write(dir.join("phases/integration.md"), PHASE_INTEGRATION)?;
    fs::write(dir.join("phases/reward-design.md"), PHASE_REWARD_DESIGN)?;
    fs::write(dir.join("phases/operations.md"), PHASE_OPERATIONS)?;
    fs::write(dir.join("phases/judge.md"), PHASE_JUDGE)?;
    fs::write(dir.join("references/cli-reference.md"), REF_CLI)?;

    if updated {
        println!("Updated {} skill at {}/", tool.label(), tool.skill_dir());
    } else {
        println!(
            "Installed {} skill at {}/",
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
