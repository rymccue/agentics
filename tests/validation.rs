use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_rejects_resource_names_with_path_separators() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: ../escape\n    source: ./skills/escape\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid resource name"));
}
