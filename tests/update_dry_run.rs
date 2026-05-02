use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn update_dry_run_prints_lockfile_without_writing() {
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
        .args(["update", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry-run lockfile"))
        .stdout(predicate::str::contains("kind: AgenticsLock"))
        .stdout(predicate::str::contains("name: review"));

    assert!(!temp.path().join("agentics.lock.yaml").exists());
}

#[test]
fn update_dry_run_conflicts_with_check() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["update", "--dry-run", "--check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--dry-run cannot be combined with --check",
        ));
}
