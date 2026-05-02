use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_dry_run_rejects_duplicate_target_identity_before_planning() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("skills/one")).expect("create skill one");
    std::fs::create_dir_all(temp.path().join("skills/two")).expect("create skill two");
    std::fs::write(temp.path().join("skills/one/SKILL.md"), "# One\n").expect("write one");
    std::fs::write(temp.path().join("skills/two/SKILL.md"), "# Two\n").expect("write two");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/one\n    harnesses: [claude]\n  - type: skill\n    name: review\n    source: ./skills/two\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate install entry"));
}
