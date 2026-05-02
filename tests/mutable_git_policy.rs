use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn policy_can_reject_mutable_git_refs() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\npolicy:\n  allowMutableGitRefs: false\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: https://github.com/myorg/repo/tree/main/skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("uses mutable git ref `main`"));
}

#[test]
fn mutable_git_refs_are_allowed_by_default() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: https://github.com/myorg/repo/tree/main/skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success();
}
