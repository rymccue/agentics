use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn init_force_overwrites_existing_manifest() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("agentics.yaml"), "old\n").unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["init", "--force", "--harnesses", "claude,pi"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created agentics.yaml"));

    let manifest = std::fs::read_to_string(temp.path().join("agentics.yaml")).unwrap();
    assert!(manifest.contains("kind: AgenticsManifest"));
    assert!(manifest.contains("pi:\n    enabled: true"));
}

#[test]
fn init_without_force_still_refuses_existing_manifest() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("agentics.yaml"), "old\n").unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("manifest already exists"));

    assert_eq!(
        std::fs::read_to_string(temp.path().join("agentics.yaml")).unwrap(),
        "old\n"
    );
}
