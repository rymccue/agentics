use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_rejects_invalid_lockfile_schema() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("agentics.lock.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: WrongKind\nresources: []\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported lockfile"));
}

#[test]
fn doctor_json_reports_invalid_lockfile_schema() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("agentics.lock.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: WrongKind\nresources: []\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"valid\": false"))
        .stdout(predicate::str::contains("unsupported lockfile"));
}
