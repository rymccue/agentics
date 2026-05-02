use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn init_can_select_enabled_harnesses() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["init", "--harnesses", "claude,pi"])
        .assert()
        .success();

    let manifest = std::fs::read_to_string(temp.path().join("agentics.yaml")).unwrap();
    assert!(manifest.contains("claude:\n    enabled: true"));
    assert!(manifest.contains("pi:\n    enabled: true"));
    assert!(!manifest.contains("codex:\n    enabled: true"));
}

#[test]
fn init_rejects_unknown_harness() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["init", "--harnesses", "claude,cursor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported harness `cursor`"));
}
