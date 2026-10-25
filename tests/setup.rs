//! Unit tests for setup module
use std::fs;
use tempfile::tempdir;

#[test]
fn test_builtin_agents_not_empty() {
    let agents = nook::setup::builtin_agents();
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
    let expanded = nook::setup::expand_tilde("~/.claude");
    assert_eq!(expanded, home.join(".claude"));

    let expanded = nook::setup::expand_tilde("~/foo/bar");
    assert_eq!(expanded, home.join("foo/bar"));

    let expanded = nook::setup::expand_tilde("/absolute/path");
    assert_eq!(expanded, std::path::PathBuf::from("/absolute/path"));
}

#[test]
fn test_generate_instructions() {
    let nook_root = std::path::PathBuf::from("/test/nook");
    let instructions = nook::setup::generate_instructions("claude", &nook_root);
    assert!(instructions.contains("nook new"));
    assert!(instructions.contains("/test/nook"));
    assert!(instructions.contains("claude"));
}

#[test]
fn test_generate_skills_sh() {
    let nook_root = std::path::PathBuf::from("/test/nook");
    let skills_sh = nook::setup::generate_skills_sh(&nook_root);
    assert!(skills_sh.contains("/test/nook"));
    assert!(skills_sh.contains("nook --version"));
}

#[test]
fn test_load_custom_agents_nonexistent() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("nonexistent.toml");
    let agents = nook::setup::load_custom_agents(Some(&config_path)).unwrap();
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

    let agents = nook::setup::load_custom_agents(Some(&config_path)).unwrap();
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

    let result = nook::setup::load_custom_agents(Some(&config_path));
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

    let agents = nook::setup::detect_agents(Some(&config_path)).unwrap();
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
    let agent = nook::setup::AgentConfig {
        name: "test".to_string(),
        detect_paths: vec!["~/.test".to_string()],
        config_files: vec![nook::setup::ConfigFile {
            target: "~/.test/config".to_string(),
            template: "test.tmpl".to_string(),
            append: true,
        }],
        skill_registration: nook::setup::SkillRegistration {
            enabled: true,
            skill_dir: "~/.test/skills".to_string(),
            install_script: "skills.sh".to_string(),
        },
        custom_args: std::collections::HashMap::new(),
    };

    let serialized = toml::to_string(&agent).unwrap();
    let deserialized: nook::setup::AgentConfig = toml::from_str(&serialized).unwrap();

    assert_eq!(deserialized.name, "test");
    assert_eq!(deserialized.detect_paths, vec!["~/.test"]);
    assert_eq!(deserialized.config_files.len(), 1);
    assert!(deserialized.skill_registration.enabled);
}
