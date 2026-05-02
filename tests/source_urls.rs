use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_accepts_github_url_with_percent_encoded_subpath() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: https://github.com/acme/resources/tree/main/skills/review%20skill\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Manifest OK"));
}
