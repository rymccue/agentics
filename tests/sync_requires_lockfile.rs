use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_without_lockfile_fails_with_clear_guidance() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .failure()
        .stderr(predicate::str::contains("sync requires agentics.lock.yaml"))
        .stderr(predicate::str::contains("run `agentics update` first"));

    assert!(!temp.path().join(".claude/skills/review").exists());
}
