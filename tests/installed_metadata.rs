use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_writes_installed_metadata_summary() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\n  pi:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude, pi]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .success();

    let metadata = std::fs::read_to_string(temp.path().join(".agentics/installed.yaml")).unwrap();
    assert!(metadata.contains("target: '.claude/skills/review'"));
    assert!(metadata.contains("target: '.agents/skills/review'"));
    assert!(metadata.contains("integrity: sha256:"));
    assert!(metadata.contains("owners:"));
    assert!(metadata.contains("- claude"));
    assert!(metadata.contains("- pi"));
}

#[test]
fn sync_harness_filter_metadata_contains_only_applied_targets() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\n  pi:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude, pi]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--harness", "claude"])
        .assert()
        .success();

    let metadata = std::fs::read_to_string(temp.path().join(".agentics/installed.yaml")).unwrap();
    assert!(metadata.contains("target: '.claude/skills/review'"));
    assert!(!metadata.contains("target: '.agents/skills/review'"));
}

#[test]
fn sync_dry_run_does_not_write_installed_metadata_summary() {
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
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "would install .claude/skills/review from ./skills/review",
        ));

    assert!(!temp.path().join(".agentics/installed.yaml").exists());
}
