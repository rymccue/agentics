use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn init_creates_starter_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created agentics.yaml"));

    let manifest =
        std::fs::read_to_string(temp.path().join("agentics.yaml")).expect("manifest created");
    assert!(manifest.contains("apiVersion: agentics.dev/v1alpha1"));
    assert!(manifest.contains("kind: AgenticsManifest"));
}

#[test]
fn init_refuses_to_overwrite_existing_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(temp.path().join("agentics.yaml"), "existing\n").expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("manifest already exists"));
}
