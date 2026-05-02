use assert_cmd::Command;
use predicates::prelude::*;

fn setup(root: &std::path::Path) {
    let skill_dir = root.join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        root.join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();
}

#[test]
fn sync_write_lock_requires_yes_for_mutating_sync() {
    let temp = tempfile::tempdir().unwrap();
    setup(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--write-lock"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--write-lock requires --yes"));

    assert!(!temp.path().join("agentics.lock.yaml").exists());
    assert!(!temp.path().join(".claude/skills/review").exists());
}

#[test]
fn sync_write_lock_yes_writes_lockfile_and_installs() {
    let temp = tempfile::tempdir().unwrap();
    setup(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--write-lock", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated agentics.lock.yaml"))
        .stdout(predicate::str::contains("installed .claude/skills/review"));

    assert!(temp.path().join("agentics.lock.yaml").is_file());
    assert!(temp.path().join(".claude/skills/review/SKILL.md").is_file());
}

#[test]
fn sync_dry_run_write_lock_is_side_effect_free() {
    let temp = tempfile::tempdir().unwrap();
    setup(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--dry-run", "--write-lock"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "would install .claude/skills/review from ./skills/review",
        ));

    assert!(!temp.path().join("agentics.lock.yaml").exists());
    assert!(!temp.path().join(".claude/skills/review").exists());
}
