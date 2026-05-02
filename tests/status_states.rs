use assert_cmd::Command;
use predicates::prelude::*;

fn write_skill_manifest(root: &std::path::Path) {
    let skill_dir = root.join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").expect("write skill");
    std::fs::write(
        root.join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");
}

#[test]
fn status_reports_unmanaged_existing_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_skill_manifest(temp.path());
    let target = temp.path().join(".claude/skills/review");
    std::fs::create_dir_all(&target).expect("create unmanaged target");
    std::fs::write(target.join("SKILL.md"), "# Local\n").expect("write unmanaged target");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .failure()
        .stdout(predicate::str::contains("unmanaged .claude/skills/review"));
}

#[test]
fn status_reports_outdated_when_source_changes_after_sync() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_skill_manifest(temp.path());

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

    std::fs::write(temp.path().join("skills/review/SKILL.md"), "# New Review\n")
        .expect("update source");

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
        .stdout(predicate::str::contains("outdated .claude/skills/review"));
}
