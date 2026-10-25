use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Finding {
    pub kind: FindingKind,
    pub path: PathBuf,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    AbsolutePathInFile,
    AbsoluteSymlink,
    VenvScript,
    PyVenvCfg,
    NodeModulesBin,
    CargoTargetFile,
    TsBuildInfo,
}

impl FindingKind {
    pub fn description(&self) -> &'static str {
        match self {
            FindingKind::AbsolutePathInFile => "file contains absolute path to golden tree",
            FindingKind::AbsoluteSymlink => "symlink points to absolute path in golden tree",
            FindingKind::VenvScript => "venv activation script with hardcoded path",
            FindingKind::PyVenvCfg => "pyvenv.cfg with hardcoded home path",
            FindingKind::NodeModulesBin => "node_modules/.bin script with absolute path",
            FindingKind::CargoTargetFile => "Cargo target/ file with build fingerprint",
            FindingKind::TsBuildInfo => "TypeScript .tsbuildinfo with absolute paths",
        }
    }

    pub fn suggestion(&self) -> &'static str {
        match self {
            FindingKind::AbsolutePathInFile => "use relative paths or environment variables",
            FindingKind::AbsoluteSymlink => "use relative symlinks instead",
            FindingKind::VenvScript => "recreate venv after cloning, or use virtualenv --copies",
            FindingKind::PyVenvCfg => "recreate venv after cloning",
            FindingKind::NodeModulesBin => {
                "reinstall deps after cloning, or use pnpm which handles this"
            }
            FindingKind::CargoTargetFile => "keep target/ out of golden tree (add to .gitignore)",
            FindingKind::TsBuildInfo => "keep .tsbuildinfo out of golden tree (add to .gitignore)",
        }
    }
}

/// Scan the golden tree at `base` for path-dependent content.
/// Returns a list of findings.
pub fn check_base(base: &Path) -> Result<Vec<Finding>> {
    let base_canon = fs::canonicalize(base)?;
    let base_str = base_canon.to_string_lossy();
    let mut findings = Vec::new();

    // Walk the tree
    walk_and_check(base, &base_str, &mut findings)?;

    Ok(findings)
}

fn walk_and_check(dir: &Path, base_str: &str, findings: &mut Vec<Finding>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let md = fs::symlink_metadata(&path)?;

        if md.file_type().is_symlink() {
            check_symlink(&path, base_str, findings)?;
        } else if md.is_dir() {
            // Check for special directories
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "target" || name == ".tsbuildinfo" {
                // These are likely build artifacts that should not be in the golden tree
                check_dir_for_findings(&path, base_str, findings)?;
            } else if name == "node_modules" {
                check_node_modules(&path, base_str, findings)?;
            } else {
                walk_and_check(&path, base_str, findings)?;
            }
        } else if md.is_file() {
            check_file(&path, base_str, findings)?;
        }
    }
    Ok(())
}

fn check_symlink(path: &Path, base_str: &str, findings: &mut Vec<Finding>) -> Result<()> {
    let target = fs::read_link(path)?;
    if target.is_absolute() {
        // Canonicalize the target to handle symlinks like /var -> /private/var
        let target_canon = fs::canonicalize(&target).unwrap_or(target.clone());
        let target_str = target_canon.to_string_lossy();
        if target_str.starts_with(base_str) {
            findings.push(Finding {
                kind: FindingKind::AbsoluteSymlink,
                path: path.to_path_buf(),
                detail: format!("points to {}", target.display()),
            });
        }
    }
    Ok(())
}

fn check_file(path: &Path, base_str: &str, findings: &mut Vec<Finding>) -> Result<()> {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    // Check for pyvenv.cfg
    if name == "pyvenv.cfg" {
        check_pyvenv_cfg(path, base_str, findings)?;
        return Ok(());
    }

    // Check for venv/bin/activate style scripts
    if name == "activate" || name == "activate.csh" || name == "activate.fish" {
        check_venv_script(path, base_str, findings)?;
        return Ok(());
    }

    // Check for .tsbuildinfo files
    if name.ends_with(".tsbuildinfo") {
        check_tsbuildinfo(path, base_str, findings)?;
        return Ok(());
    }

    // Check for Cargo fingerprint files
    if name.ends_with(".fingerprint") || name == "buildscript-dep" {
        findings.push(Finding {
            kind: FindingKind::CargoTargetFile,
            path: path.to_path_buf(),
            detail: "Cargo build fingerprint file".to_string(),
        });
        return Ok(());
    }

    // General check for absolute path in text files
    if is_text_file(path) {
        check_text_file_for_path(path, base_str, findings)?;
    }

    Ok(())
}

fn check_dir_for_findings(dir: &Path, base_str: &str, findings: &mut Vec<Finding>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let md = fs::symlink_metadata(&path)?;
        if md.is_file() {
            check_file(&path, base_str, findings)?;
        } else if md.is_dir() {
            check_dir_for_findings(&path, base_str, findings)?;
        }
    }
    Ok(())
}

fn check_node_modules(dir: &Path, base_str: &str, findings: &mut Vec<Finding>) -> Result<()> {
    let bin_dir = dir.join(".bin");
    if bin_dir.exists() && bin_dir.is_dir() {
        for entry in fs::read_dir(&bin_dir)? {
            let entry = entry?;
            let path = entry.path();
            let md = fs::symlink_metadata(&path)?;
            if md.file_type().is_symlink() {
                check_symlink(&path, base_str, findings)?;
            } else if md.is_file() && is_text_file(&path) {
                // Check for absolute path in node_modules/.bin scripts
                let content = fs::read_to_string(&path)?;
                if content.contains(base_str) {
                    findings.push(Finding {
                        kind: FindingKind::NodeModulesBin,
                        path: path.to_path_buf(),
                        detail: "node_modules/.bin script contains golden tree absolute path"
                            .to_string(),
                    });
                }
            }
        }
    }
    // Also check for package.json with absolute paths
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name().and_then(|s| s.to_str()) == Some("package.json") {
            check_text_file_for_path(&path, base_str, findings)?;
        }
    }
    Ok(())
}

fn check_pyvenv_cfg(path: &Path, base_str: &str, findings: &mut Vec<Finding>) -> Result<()> {
    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        if line.trim().starts_with("home = ") {
            let home_path = line.trim().strip_prefix("home = ").unwrap().trim();
            if home_path.starts_with(base_str) {
                findings.push(Finding {
                    kind: FindingKind::PyVenvCfg,
                    path: path.to_path_buf(),
                    detail: format!("home points to {}", home_path),
                });
            }
        }
    }
    Ok(())
}

fn check_venv_script(path: &Path, base_str: &str, findings: &mut Vec<Finding>) -> Result<()> {
    let content = fs::read_to_string(path)?;
    if content.contains(base_str) {
        findings.push(Finding {
            kind: FindingKind::VenvScript,
            path: path.to_path_buf(),
            detail: "venv activation script contains golden tree path".to_string(),
        });
    }
    Ok(())
}

fn check_tsbuildinfo(path: &Path, base_str: &str, findings: &mut Vec<Finding>) -> Result<()> {
    let content = fs::read_to_string(path)?;
    if content.contains(base_str) {
        findings.push(Finding {
            kind: FindingKind::TsBuildInfo,
            path: path.to_path_buf(),
            detail: "TypeScript build info contains golden tree path".to_string(),
        });
    }
    Ok(())
}

fn check_text_file_for_path(
    path: &Path,
    base_str: &str,
    findings: &mut Vec<Finding>,
) -> Result<()> {
    let content = fs::read_to_string(path)?;
    if content.contains(base_str) {
        findings.push(Finding {
            kind: FindingKind::AbsolutePathInFile,
            path: path.to_path_buf(),
            detail: "file contains golden tree absolute path".to_string(),
        });
    }
    Ok(())
}

fn is_text_file(path: &Path) -> bool {
    // Quick check: try to read as UTF-8, skip binary files
    if let Ok(content) = fs::read(path) {
        // Heuristic: if it contains null bytes, it's likely binary
        !content.contains(&0)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn detects_absolute_symlink() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("sub/file.txt"), b"hello").unwrap();

        let link = base.join("link_to_sub");
        symlink(base.join("sub"), &link).unwrap();

        let findings = check_base(&base).unwrap();
        assert!(findings
            .iter()
            .any(|f| matches!(f.kind, FindingKind::AbsoluteSymlink)));
    }

    #[test]
    fn detects_pyvenv_cfg() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        fs::create_dir_all(&base).unwrap();
        let base_canon = fs::canonicalize(&base).unwrap();
        let base_str = base_canon.to_string_lossy();
        fs::write(
            base.join("pyvenv.cfg"),
            format!(
                "home = {}\ninclude-system-site-packages = false\n",
                base_str
            ),
        )
        .unwrap();

        let findings = check_base(&base).unwrap();
        assert!(findings
            .iter()
            .any(|f| matches!(f.kind, FindingKind::PyVenvCfg)));
    }

    #[test]
    fn detects_venv_activate() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        fs::create_dir_all(base.join("venv/bin")).unwrap();
        let base_canon = fs::canonicalize(&base).unwrap();
        let base_str = base_canon.to_string_lossy();
        fs::write(
            base.join("venv/bin/activate"),
            format!("VIRTUAL_ENV=\"{}\"\nexport VIRTUAL_ENV\n", base_str),
        )
        .unwrap();

        let findings = check_base(&base).unwrap();
        assert!(findings
            .iter()
            .any(|f| matches!(f.kind, FindingKind::VenvScript)));
    }

    #[test]
    fn detects_tsbuildinfo() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        fs::create_dir_all(&base).unwrap();
        let base_canon = fs::canonicalize(&base).unwrap();
        let base_str = base_canon.to_string_lossy();
        fs::write(
            base.join("project.tsbuildinfo"),
            format!(
                "{{\"version\": 1, \"program\": \"{}/src/main.ts\"}}",
                base_str
            ),
        )
        .unwrap();

        let findings = check_base(&base).unwrap();
        assert!(findings
            .iter()
            .any(|f| matches!(f.kind, FindingKind::TsBuildInfo)));
    }

    #[test]
    fn detects_cargo_fingerprint() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        fs::create_dir_all(base.join("target/debug/.fingerprint")).unwrap();
        fs::write(
            base.join("target/debug/.fingerprint/some-crate.fingerprint"),
            b"fingerprint",
        )
        .unwrap();

        let findings = check_base(&base).unwrap();
        assert!(findings
            .iter()
            .any(|f| matches!(f.kind, FindingKind::CargoTargetFile)));
    }

    #[test]
    fn detects_node_modules_bin() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        fs::create_dir_all(base.join("node_modules/.bin")).unwrap();
        let base_canon = fs::canonicalize(&base).unwrap();
        let base_str = base_canon.to_string_lossy();
        // Create a script with absolute path
        fs::write(
            base.join("node_modules/.bin/some-script"),
            format!("#!/bin/sh\n{}", base_str),
        )
        .unwrap();

        let findings = check_base(&base).unwrap();
        assert!(findings
            .iter()
            .any(|f| matches!(f.kind, FindingKind::NodeModulesBin)));
    }

    #[test]
    fn clean_tree_no_findings() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        fs::create_dir_all(base.join("src")).unwrap();
        fs::write(base.join("src/main.rs"), b"fn main() {}").unwrap();
        fs::write(
            base.join("Cargo.toml"),
            b"[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let findings = check_base(&base).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn relative_symlink_ok() {
        let d = tempdir().unwrap();
        let base = d.path().join("base");
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("sub/file.txt"), b"hello").unwrap();

        let link = base.join("link_to_sub");
        symlink("sub", &link).unwrap(); // relative!

        let findings = check_base(&base).unwrap();
        assert!(!findings
            .iter()
            .any(|f| matches!(f.kind, FindingKind::AbsoluteSymlink)));
    }
}
