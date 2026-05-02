use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn status_reports_missing_and_installed_skill() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").expect("write skill");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    let mut missing = Command::cargo_bin("agentics").expect("binary exists");
    missing
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing .claude/skills/review"));

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .success();

    let mut installed = Command::cargo_bin("agentics").expect("binary exists");
    installed
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed .claude/skills/review"));
}
