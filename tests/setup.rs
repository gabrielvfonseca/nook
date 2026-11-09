//! Unit tests for setup module
use std::fs;
use tempfile::tempdir;

#[test]
fn test_builtin_agents_not_empty() {
    let agents = replica::setup::builtin_agents();
    assert!(!agents.is_empty());
    assert!(agents.iter().any(|a| a.name == "claude"));
    assert!(agents.iter().any(|a| a.name == "cursor"));
    assert!(agents.iter().any(|a| a.name == "copilot"));
    assert!(agents.iter().any(|a| a.name == "zed"));
    assert!(agents.iter().any(|a| a.name == "aider"));
}

#[test]
fn test_expand_tilde() {
    let home = dirs::home_dir().unwrap();
    let expanded = replica::setup::expand_tilde("~/.claude");
    assert_eq!(expanded, home.join(".claude"));

    let expanded = replica::setup::expand_tilde("~/foo/bar");
    assert_eq!(expanded, home.join("foo/bar"));

    let expanded = replica::setup::expand_tilde("/absolute/path");
    assert_eq!(expanded, std::path::PathBuf::from("/absolute/path"));
}

#[test]
fn test_generate_instructions() {
    let replica_root = std::path::PathBuf::from("/test/replica");
    let instructions = replica::setup::generate_instructions("claude", &replica_root);
    assert!(instructions.contains("replica new"));
    assert!(instructions.contains("/test/replica"));
    assert!(instructions.contains("claude"));
}

#[test]
fn test_generate_skills_sh() {
    let replica_root = std::path::PathBuf::from("/test/replica");
    let skills_sh = replica::setup::generate_skills_sh(&replica_root);
    assert!(skills_sh.contains("/test/replica"));
    assert!(skills_sh.contains("replica --version"));
}

#[test]
fn test_load_custom_agents_nonexistent() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("nonexistent.toml");
    let agents = replica::setup::load_custom_agents(Some(&config_path)).unwrap();
    assert!(agents.is_empty());
}

#[test]
fn test_load_custom_agents_valid() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("agents.toml");
    let config_content = r#"
[[custom_agents]]
name = "my-agent"
detect_paths = ["~/.my-agent"]
config_files = [
    { target = "~/.my-agent/AGENTS.md", template = "my-agent.md", append = true }
]
skill_registration = { enabled = true, skill_dir = "~/.my-agent/skills", install_script = "skills.sh" }
"#;
    fs::write(&config_path, config_content).unwrap();

    let agents = replica::setup::load_custom_agents(Some(&config_path)).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, "my-agent");
    assert_eq!(agents[0].detect_paths, vec!["~/.my-agent"]);
    assert_eq!(agents[0].config_files.len(), 1);
    assert!(agents[0].skill_registration.enabled);
}

#[test]
fn test_load_custom_agents_invalid_toml() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("agents.toml");
    fs::write(&config_path, "invalid toml [[[").unwrap();

    let result = replica::setup::load_custom_agents(Some(&config_path));
    assert!(result.is_err());
}

#[test]
fn test_detect_agents_mock() {
    // Test with a custom config that points to temp dirs we create
    let temp = tempdir().unwrap();
    let claude_dir = temp.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();

    let config_content = format!(
        r#"
[[custom_agents]]
name = "test-agent"
detect_paths = ["{}"]
config_files = [
    {{ target = "{}/config.md", template = "test.md", append = false }}
]
skill_registration = {{ enabled = false, skill_dir = "", install_script = "" }}
"#,
        claude_dir.display(),
        claude_dir.display()
    );

    let config_path = temp.path().join("agents.toml");
    fs::write(&config_path, config_content).unwrap();

    let agents = replica::setup::detect_agents(Some(&config_path)).unwrap();
    // Should find our custom agent plus any built-in agents that exist on the system
    assert!(agents.iter().any(|a| a.config.name == "test-agent"));
    let test_agent = agents
        .iter()
        .find(|a| a.config.name == "test-agent")
        .unwrap();
    assert_eq!(test_agent.detected_paths.len(), 1);
    assert_eq!(test_agent.detected_paths[0], claude_dir);
}

#[test]
fn test_agent_config_serialization() {
    let agent = replica::setup::AgentConfig {
        name: "test".to_string(),
        detect_paths: vec!["~/.test".to_string()],
        config_files: vec![replica::setup::ConfigFile {
            target: "~/.test/config".to_string(),
            template: "test.tmpl".to_string(),
            append: true,
        }],
        skill_registration: replica::setup::SkillRegistration {
            enabled: true,
            skill_dir: "~/.test/skills".to_string(),
            install_script: "skills.sh".to_string(),
        },
        custom_args: std::collections::HashMap::new(),
    };

    let serialized = toml::to_string(&agent).unwrap();
    let deserialized: replica::setup::AgentConfig = toml::from_str(&serialized).unwrap();

    assert_eq!(deserialized.name, "test");
    assert_eq!(deserialized.detect_paths, vec!["~/.test"]);
    assert_eq!(deserialized.config_files.len(), 1);
    assert!(deserialized.skill_registration.enabled);
}

fn agent_for(
    target: &std::path::Path,
    template: &str,
    append: bool,
) -> replica::setup::DetectedAgent {
    replica::setup::DetectedAgent {
        config: replica::setup::AgentConfig {
            name: "test".to_string(),
            detect_paths: vec![],
            config_files: vec![replica::setup::ConfigFile {
                target: target.display().to_string(),
                template: template.to_string(),
                append,
            }],
            skill_registration: replica::setup::SkillRegistration {
                enabled: false,
                skill_dir: String::new(),
                install_script: String::new(),
            },
            custom_args: std::collections::HashMap::new(),
        },
        detected_paths: vec![],
    }
}

#[test]
fn appended_config_is_added_once() {
    let temp = tempdir().unwrap();
    let target = temp.path().join("CLAUDE.md");
    fs::write(&target, "# My notes\n").unwrap();
    let agent = agent_for(&target, "claude.md", true);

    replica::setup::generate_agent_config(&agent, temp.path(), false, false).unwrap();
    replica::setup::generate_agent_config(&agent, temp.path(), false, false).unwrap();

    let text = fs::read_to_string(&target).unwrap();
    assert!(text.starts_with("# My notes"), "user content must be kept");
    assert_eq!(
        text.matches("# Replica Integration for Claude Code")
            .count(),
        1
    );
}

#[test]
fn replaced_config_is_kept_without_force_and_backed_up_with_force() {
    let temp = tempdir().unwrap();
    let target = temp.path().join(".cursorrules");
    fs::write(&target, "my rules").unwrap();
    let agent = agent_for(&target, "cursorrules", false);

    replica::setup::generate_agent_config(&agent, temp.path(), false, false).unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "my rules");
    assert!(!temp.path().join(".cursorrules.bak").exists());

    replica::setup::generate_agent_config(&agent, temp.path(), true, false).unwrap();
    assert_ne!(fs::read_to_string(&target).unwrap(), "my rules");
    assert_eq!(
        fs::read_to_string(temp.path().join(".cursorrules.bak")).unwrap(),
        "my rules"
    );
}

#[test]
fn dry_run_writes_nothing() {
    let temp = tempdir().unwrap();
    let target = temp.path().join("fresh.md");
    let agent = agent_for(&target, "claude.md", true);
    replica::setup::generate_agent_config(&agent, temp.path(), false, true).unwrap();
    assert!(!target.exists());
}
