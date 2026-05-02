use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_installs_skill_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").expect("write skill");
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

    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed .claude/skills/review"));

    assert_eq!(
        std::fs::read_to_string(temp.path().join(".claude/skills/review/SKILL.md"))
            .expect("installed skill"),
        "# Review\n"
    );
}

#[test]
fn sync_refuses_unmanaged_existing_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").expect("write skill");
    let target = temp.path().join(".claude/skills/review");
    std::fs::create_dir_all(&target).expect("create target");
    std::fs::write(target.join("SKILL.md"), "# Local\n").expect("write local");
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

    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "refusing to overwrite unmanaged target",
        ));
}
