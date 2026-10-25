use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub detect_paths: Vec<String>,
    pub config_files: Vec<ConfigFile>,
    pub skill_registration: SkillRegistration,
    #[serde(default)]
    pub custom_args: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub target: String,
    pub template: String,
    pub append: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRegistration {
    pub enabled: bool,
    pub skill_dir: String,
    pub install_script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAgentConfig {
    pub name: String,
    pub detect_paths: Vec<String>,
    pub config_files: Vec<ConfigFile>,
    pub skill_registration: SkillRegistration,
    #[serde(default)]
    pub custom_args: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    #[serde(default)]
    pub custom_agents: Vec<CustomAgentConfig>,
}

#[derive(Debug, Clone)]
pub struct DetectedAgent {
    pub config: AgentConfig,
    pub detected_paths: Vec<PathBuf>,
}

pub fn builtin_agents() -> Vec<AgentConfig> {
    vec![
        AgentConfig {
            name: "claude".to_string(),
            detect_paths: vec!["~/.claude".to_string(), "~/.config/claude".to_string()],
            config_files: vec![ConfigFile {
                target: "~/.claude/CLAUDE.md".to_string(),
                template: "claude.md".to_string(),
                append: true,
            }],
            skill_registration: SkillRegistration {
                enabled: true,
                skill_dir: "~/.claude/skills".to_string(),
                install_script: "skills.sh".to_string(),
            },
            custom_args: HashMap::new(),
        },
        AgentConfig {
            name: "cursor".to_string(),
            detect_paths: vec!["~/.cursor".to_string(), ".cursor".to_string()],
            config_files: vec![ConfigFile {
                target: ".cursorrules".to_string(),
                template: "cursorrules".to_string(),
                append: false,
            }],
            skill_registration: SkillRegistration {
                enabled: true,
                skill_dir: ".cursor/skills".to_string(),
                install_script: "skills.sh".to_string(),
            },
            custom_args: HashMap::new(),
        },
        AgentConfig {
            name: "copilot".to_string(),
            detect_paths: vec![
                "~/Library/Application Support/Code/User".to_string(),
                "~/.vscode".to_string(),
            ],
            config_files: vec![ConfigFile {
                target: "~/Library/Application Support/Code/User/copilot-instructions.md"
                    .to_string(),
                template: "copilot-instructions.md".to_string(),
                append: true,
            }],
            skill_registration: SkillRegistration {
                enabled: false,
                skill_dir: "".to_string(),
                install_script: "".to_string(),
            },
            custom_args: HashMap::new(),
        },
        AgentConfig {
            name: "zed".to_string(),
            detect_paths: vec!["~/.config/zed".to_string()],
            config_files: vec![ConfigFile {
                target: "~/.config/zed/settings.json".to_string(),
                template: "zed-settings.json".to_string(),
                append: false,
            }],
            skill_registration: SkillRegistration {
                enabled: false,
                skill_dir: "".to_string(),
                install_script: "".to_string(),
            },
            custom_args: HashMap::new(),
        },
        AgentConfig {
            name: "aider".to_string(),
            detect_paths: vec!["~/.aider".to_string()],
            config_files: vec![ConfigFile {
                target: "~/.aider.conf.yml".to_string(),
                template: "aider.conf.yml".to_string(),
                append: true,
            }],
            skill_registration: SkillRegistration {
                enabled: false,
                skill_dir: "".to_string(),
                install_script: "".to_string(),
            },
            custom_args: HashMap::new(),
        },
    ]
}

pub fn load_custom_agents(config_path: Option<&Path>) -> Result<Vec<AgentConfig>> {
    let config_path = config_path.map(PathBuf::from).unwrap_or_else(|| {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("nook/agents.toml")
    });

    if !config_path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("reading custom agents config: {}", config_path.display()))?;

    let config: AgentsConfig = toml::from_str(&content)
        .with_context(|| format!("parsing custom agents config: {}", config_path.display()))?;

    Ok(config
        .custom_agents
        .into_iter()
        .map(|c| AgentConfig {
            name: c.name,
            detect_paths: c.detect_paths,
            config_files: c.config_files,
            skill_registration: c.skill_registration,
            custom_args: c.custom_args,
        })
        .collect())
}

pub fn detect_agents(custom_config_path: Option<&Path>) -> Result<Vec<DetectedAgent>> {
    let mut all_agents = builtin_agents();
    all_agents.extend(load_custom_agents(custom_config_path)?);

    let mut detected = Vec::new();

    for agent in all_agents {
        let mut detected_paths = Vec::new();
        for detect_path in &agent.detect_paths {
            let expanded = expand_tilde(detect_path);
            if expanded.exists() {
                detected_paths.push(expanded);
            }
        }

        if !detected_paths.is_empty() {
            detected.push(DetectedAgent {
                config: agent,
                detected_paths,
            });
        }
    }

    Ok(detected)
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

pub fn generate_agent_config(
    agent: &DetectedAgent,
    nook_root: &Path,
    force: bool,
    dry_run: bool,
) -> Result<()> {
    for config_file in &agent.config.config_files {
        let target_path = expand_tilde(&config_file.target);

        if let Some(parent) = target_path.parent() {
            if !dry_run {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating directory: {}", parent.display()))?;
            }
        }

        let template_content = get_template(&config_file.template, nook_root)?;

        if target_path.exists() && !force && !config_file.append && !dry_run {
            let backup = target_path.with_extension(format!(
                "{}.bak",
                target_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("bak")
            ));
            fs::copy(&target_path, &backup).with_context(|| {
                format!("backing up existing config: {}", target_path.display())
            })?;
            eprintln!(
                "  backed up existing {} to {}",
                target_path.display(),
                backup.display()
            );
        }

        let final_content = if config_file.append && target_path.exists() {
            let existing = fs::read_to_string(&target_path)
                .with_context(|| format!("reading existing config: {}", target_path.display()))?;
            format!("{}\n\n{}", existing, template_content)
        } else {
            template_content
        };

        if dry_run {
            println!(
                "  would write: {} ({} bytes)",
                target_path.display(),
                final_content.len()
            );
        } else {
            fs::write(&target_path, final_content)
                .with_context(|| format!("writing config: {}", target_path.display()))?;
            println!("  wrote: {}", target_path.display());
        }
    }

    Ok(())
}

pub fn install_skills(
    agent: &DetectedAgent,
    nook_root: &Path,
    _force: bool,
    dry_run: bool,
) -> Result<()> {
    let skill_registration = &agent.config.skill_registration;
    if !skill_registration.enabled {
        return Ok(());
    }

    let skill_dir = expand_tilde(&skill_registration.skill_dir);
    let nook_skill_dir = skill_dir.join("nook");

    if !dry_run {
        fs::create_dir_all(&nook_skill_dir)
            .with_context(|| format!("creating skill directory: {}", nook_skill_dir.display()))?;
    }

    let skill_toml_content = r#"name = "nook"
version = "0.1.0"
description = "APFS clonefile workspace manager for agent workloads"
author = "nook contributors"
agent_compatibility = ["claude", "cursor", "copilot", "zed", "aider"]
"#;

    let skill_toml_path = nook_skill_dir.join("skill.toml");
    if dry_run {
        println!("  would write skill.toml: {}", skill_toml_path.display());
    } else {
        fs::write(&skill_toml_path, skill_toml_content)
            .with_context(|| format!("writing skill.toml: {}", skill_toml_path.display()))?;
        println!("  wrote skill.toml: {}", skill_toml_path.display());
    }

    let instructions_content = generate_instructions(&agent.config.name, nook_root);
    let instructions_path = nook_skill_dir.join("instructions.md");
    if dry_run {
        println!(
            "  would write instructions.md: {}",
            instructions_path.display()
        );
    } else {
        fs::write(&instructions_path, instructions_content)
            .with_context(|| format!("writing instructions.md: {}", instructions_path.display()))?;
        println!("  wrote instructions.md: {}", instructions_path.display());
    }

    let skills_sh_content = generate_skills_sh(nook_root);
    let skills_sh_path = skill_dir.join(&skill_registration.install_script);
    if dry_run {
        println!("  would write skills.sh: {}", skills_sh_path.display());
    } else {
        fs::write(&skills_sh_path, skills_sh_content)
            .with_context(|| format!("writing skills.sh: {}", skills_sh_path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&skills_sh_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&skills_sh_path, perms)?;
        }
        println!("  wrote skills.sh: {}", skills_sh_path.display());
    }

    Ok(())
}

fn get_template(template_name: &str, nook_root: &Path) -> Result<String> {
    let template = match template_name {
        "claude.md" => include_str!("../templates/claude.md"),
        "cursorrules" => include_str!("../templates/cursorrules"),
        "copilot-instructions.md" => include_str!("../templates/copilot-instructions.md"),
        "zed-settings.json" => include_str!("../templates/zed-settings.json"),
        "aider.conf.yml" => include_str!("../templates/aider.conf.yml"),
        _ => {
            let template_path = nook_root.join("templates").join(template_name);
            if template_path.exists() {
                return fs::read_to_string(&template_path)
                    .with_context(|| format!("reading template: {}", template_path.display()));
            }
            bail!("unknown template: {}", template_name);
        }
    };
    Ok(template.to_string())
}

pub fn generate_instructions(agent_name: &str, nook_root: &Path) -> String {
    format!(
        r#"# Nook Skill for {}

## When to use nook
- Starting a new coding task that needs an isolated workspace
- Running multiple agent sessions in parallel
- When you need fast workspace creation (< 100ms)

## Commands
- `nook new [--ttl 2h]` - Create workspace, prints path
- `nook env <id>` - Get export lines for workspace
- `nook list` - List active workspaces
- `nook gc` - Clean up expired workspaces
- `nook stats` - Show disk usage

## Integration
Add to your workflow: `cd $(nook new) && <your-agent> .`

## Configuration
NOOK_ROOT: {}
"#,
        agent_name,
        nook_root.display()
    )
}

pub fn generate_skills_sh(nook_root: &Path) -> String {
    format!(
        r#"#!/usr/bin/env bash
# Nook skills installer for {{AGENT}}
# This script is generated by `nook setup`

set -euo pipefail

NOOK_ROOT="${{NOOK_ROOT:-{}}}"
SKILLS_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"

echo "Installing nook skills for {{AGENT}}..."
echo "  NOOK_ROOT: $NOOK_ROOT"
echo "  Skills dir: $SKILLS_DIR"

# Verify nook is available
if ! command -v nook &> /dev/null; then
    echo "Warning: nook not found in PATH. Make sure it's installed."
    echo "  Run: cargo install --path $NOOK_ROOT"
    exit 1
fi

echo "Nook version: $(nook --version)"
echo "Skills installed successfully!"
"#,
        nook_root.display()
    )
}

pub fn generate_homebrew_formula(nook_root: &Path, version: &str, dry_run: bool) -> Result<()> {
    let formula_template = include_str!("../templates/nook.rb.tmpl");

    let formula = formula_template
        .replace("{{VERSION}}", version)
        .replace("{{GITHUB_REPO}}", "gabrielfonseca/nook")
        .replace("{{SHA256_ARM}}", "REPLACE_WITH_ACTUAL_SHA256_ARM")
        .replace("{{SHA256_INTEL}}", "REPLACE_WITH_ACTUAL_SHA256_INTEL")
        .replace("{{HOMEBREW_PREFIX}}", "/usr/local")
        .replace("{{NOOK_ROOT}}", &nook_root.display().to_string());

    let formula_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join("homebrew");

    if !dry_run {
        fs::create_dir_all(&formula_dir)
            .with_context(|| format!("creating homebrew directory: {}", formula_dir.display()))?;
    }

    let formula_path = formula_dir.join("nook.rb");
    if dry_run {
        println!("  would write formula: {}", formula_path.display());
    } else {
        fs::write(&formula_path, formula)
            .with_context(|| format!("writing formula: {}", formula_path.display()))?;
        println!("  wrote formula: {}", formula_path.display());
        println!("  Install with: brew install {}", formula_path.display());
    }

    Ok(())
}

pub fn run_setup(
    nook_root: &Path,
    homebrew: bool,
    force: bool,
    dry_run: bool,
    custom_config: Option<&Path>,
) -> Result<()> {
    println!("Nook Setup");
    println!("==========");
    println!("NOOK_ROOT: {}", nook_root.display());
    println!();

    let agents = detect_agents(custom_config)?;

    if agents.is_empty() {
        println!("No coding agents detected.");
        println!("Supported agents: claude, cursor, copilot, zed, aider");
        println!("Add custom agents in ~/.config/nook/agents.toml");
        return Ok(());
    }

    println!("Detected {} agent(s):", agents.len());
    for agent in &agents {
        println!(
            "  - {} (found at: {})",
            agent.config.name,
            agent
                .detected_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!();

    for agent in &agents {
        println!("Configuring {}...", agent.config.name);
        generate_agent_config(agent, nook_root, force, dry_run)?;
        install_skills(agent, nook_root, force, dry_run)?;
        println!();
    }

    if homebrew {
        println!("Generating Homebrew formula...");
        let version = env!("CARGO_PKG_VERSION");
        generate_homebrew_formula(nook_root, version, dry_run)?;
        println!();
    }

    if dry_run {
        println!("Dry run complete. No files were written.");
    } else {
        println!("Setup complete!");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_ttl() {
        assert_eq!(crate::util::parse_ttl("30m").unwrap(), 1800);
        assert_eq!(crate::util::parse_ttl("2h").unwrap(), 7200);
        assert_eq!(crate::util::parse_ttl("1d").unwrap(), 86400);
    }

    #[test]
    fn test_expand_tilde() {
        let home = dirs::home_dir().unwrap();
        let expanded = expand_tilde("~/.claude");
        assert_eq!(expanded, home.join(".claude"));
    }

    #[test]
    fn test_builtin_agents_not_empty() {
        let agents = builtin_agents();
        assert!(!agents.is_empty());
        assert!(agents.iter().any(|a| a.name == "claude"));
        assert!(agents.iter().any(|a| a.name == "cursor"));
    }

    #[test]
    fn test_generate_instructions() {
        let nook_root = PathBuf::from("/test/nook");
        let instructions = generate_instructions("claude", &nook_root);
        assert!(instructions.contains("nook new"));
        assert!(instructions.contains("/test/nook"));
    }
}
