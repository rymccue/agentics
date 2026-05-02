use assert_cmd::Command;
use predicates::prelude::*;

fn write_manifest(root: &std::path::Path) {
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
fn status_reports_drifted_managed_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_manifest(temp.path());

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

    std::fs::write(
        temp.path().join(".claude/skills/review/SKILL.md"),
        "# Edited\n",
    )
    .expect("drift installed skill");

    let mut status = Command::cargo_bin("agentics").expect("binary exists");
    status
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .failure()
        .stdout(predicate::str::contains("drifted .claude/skills/review"));
}

#[test]
fn sync_refuses_to_replace_drifted_managed_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_manifest(temp.path());

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

    std::fs::write(
        temp.path().join(".claude/skills/review/SKILL.md"),
        "# Edited\n",
    )
    .expect("drift installed skill");

    let mut sync = Command::cargo_bin("agentics").expect("binary exists");
    sync.current_dir(temp.path())
        .arg("sync")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "refusing to overwrite drifted managed target",
        ));
}
