use assert_cmd::Command;
use predicates::prelude::*;

fn setup_drifted() -> tempfile::TempDir {
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
    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .success();
    std::fs::write(
        temp.path().join(".claude/skills/review/SKILL.md"),
        "# Drifted\n",
    )
    .expect("drift target");
    temp
}

#[test]
fn sync_force_replaces_drifted_managed_target() {
    let temp = setup_drifted();
    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["sync", "--force"])
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
fn sync_help_lists_safety_flags() {
    Command::cargo_bin("agentics")
        .expect("binary exists")
        .args(["sync", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--force")
                .and(predicate::str::contains("--yes"))
                .and(predicate::str::contains("--non-interactive")),
        );
}
