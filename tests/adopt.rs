use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn adopt_writes_metadata_for_matching_existing_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("skills/review")).expect("create source skill");
    std::fs::write(temp.path().join("skills/review/SKILL.md"), "# Review\n").expect("write source");
    std::fs::create_dir_all(temp.path().join(".claude/skills/review"))
        .expect("create target skill");
    std::fs::write(
        temp.path().join(".claude/skills/review/SKILL.md"),
        "# Review\n",
    )
    .expect("write target");
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
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .failure()
        .stdout(predicate::str::contains("unmanaged .claude/skills/review"));

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["adopt", "skill:review"])
        .assert()
        .success()
        .stdout(predicate::str::contains("adopted .claude/skills/review"));

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed .claude/skills/review"));
}

#[test]
fn adopt_rejects_non_matching_existing_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("skills/review")).expect("create source skill");
    std::fs::write(temp.path().join("skills/review/SKILL.md"), "# Review\n").expect("write source");
    std::fs::create_dir_all(temp.path().join(".claude/skills/review"))
        .expect("create target skill");
    std::fs::write(
        temp.path().join(".claude/skills/review/SKILL.md"),
        "# Different\n",
    )
    .expect("write target");
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
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["adopt", "skill:review"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("target content does not match"));
}
