//! End-to-end checks of the `replica` binary: help, exit codes, and JSON shape.
//! Each test gets its own REPLICA_ROOT under the temp dir (APFS on macOS).
use std::process::{Command, Output};

fn replica(root: Option<&std::path::Path>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_replica"));
    cmd.args(args).env("NO_COLOR", "1");
    match root {
        Some(r) => cmd.env("REPLICA_ROOT", r),
        None => cmd.env_remove("REPLICA_ROOT"),
    };
    cmd.output().expect("run replica")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn help_works_without_a_root_volume() {
    let o = replica(None, &["help"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let text = stdout(&o);
    assert!(text.contains("USAGE"));
    assert!(text.contains("quickstart"));
    assert!(!text.contains('\x1b'));
}

#[test]
fn help_topic_prints_guide() {
    let o = replica(None, &["help", "exit-codes"]);
    assert!(o.status.success());
    assert!(stdout(&o).contains("no base built"));
}

#[test]
fn unknown_help_topic_fails_with_hint() {
    let o = replica(None, &["help", "nope"]);
    assert_eq!(o.status.code(), Some(1));
    let err = stderr(&o);
    assert!(err.contains("unknown help topic"));
    assert!(err.contains("hint: topics:"));
}

#[test]
fn new_without_base_exits_2_with_hint() {
    let d = tempfile::tempdir().unwrap();
    let o = replica(Some(d.path()), &["new"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(stderr(&o).contains("hint: run `replica base build"));
    assert!(stdout(&o).is_empty());
}

#[test]
fn missing_root_is_a_clear_error() {
    let d = tempfile::tempdir().unwrap();
    let missing = d.path().join("nope");
    let o = replica(Some(&missing), &["list"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("REPLICA_ROOT does not exist"));
}

#[test]
fn list_json_is_empty_array_when_no_workspaces() {
    let d = tempfile::tempdir().unwrap();
    let o = replica(Some(d.path()), &["list", "--json"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "[]");
}

#[test]
fn stats_json_has_expected_keys() {
    let d = tempfile::tempdir().unwrap();
    let o = replica(Some(d.path()), &["stats", "--json"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let text = stdout(&o);
    for key in [
        "\"total_bytes\"",
        "\"used_pct\"",
        "\"workspaces\":0",
        "\"base_built_at\":null",
    ] {
        assert!(text.contains(key), "missing {key} in {text}");
    }
}

#[test]
fn base_status_reports_not_built() {
    let d = tempfile::tempdir().unwrap();
    let o = replica(Some(d.path()), &["base", "status", "--json"]);
    assert!(o.status.success());
    assert_eq!(stdout(&o).trim(), "{\"built\":false}");
}

#[test]
fn rm_unknown_id_is_idempotent() {
    let d = tempfile::tempdir().unwrap();
    let o = replica(Some(d.path()), &["rm", "deadbeef"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stderr(&o).contains("already removed"));
}

#[test]
fn env_unknown_id_fails() {
    let d = tempfile::tempdir().unwrap();
    let o = replica(Some(d.path()), &["env", "deadbeef"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stdout(&o).is_empty());
}

#[test]
fn base_refresh_without_base_exits_2() {
    let d = tempfile::tempdir().unwrap();
    let o = replica(Some(d.path()), &["base", "refresh"]);
    assert_eq!(o.status.code(), Some(2));
}
