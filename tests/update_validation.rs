use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn update_rejects_missing_local_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "local source for `review` is missing",
        ));
}

#[test]
fn update_rejects_local_skill_without_skill_md() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("skills/review")).expect("create skill dir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .failure()
        .stderr(predicate::str::contains("SKILL.md"));
}
