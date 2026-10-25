//! Integration test: clonefile on the temp dir's volume (APFS on macOS runners/dev machines).
use std::fs;

#[test]
fn clone_is_isolated_from_base() {
    let d = tempfile::tempdir().unwrap();
    let base = d.path().join("base");
    fs::create_dir_all(base.join("sub")).unwrap();
    fs::write(base.join("sub/f.txt"), b"original").unwrap();

    let ws = d.path().join("ws1");
    match nook::clone::clone_tree(&base, &ws) {
        Ok(()) => {}
        Err(e) => {
            // Not APFS (e.g. some CI tmpdirs): skip rather than fail.
            eprintln!("skipping, clonefile unsupported here: {e}");
            return;
        }
    }
    fs::write(ws.join("sub/f.txt"), b"changed").unwrap();
    assert_eq!(fs::read(base.join("sub/f.txt")).unwrap(), b"original");
}

#[test]
fn clone_refuses_existing_destination() {
    let d = tempfile::tempdir().unwrap();
    let base = d.path().join("base");
    fs::create_dir_all(&base).unwrap();
    let ws = d.path().join("ws");
    fs::create_dir_all(&ws).unwrap();
    assert!(nook::clone::clone_tree(&base, &ws).is_err());
}
