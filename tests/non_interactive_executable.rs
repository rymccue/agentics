use assert_cmd::Command;
use predicates::prelude::*;

fn setup_executable_skill(root: &std::path::Path) {
    let skill_dir = root.join("skills/review");
    std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        skill_dir.join("scripts/check.sh"),
        "#!/usr/bin/env bash\necho ok\n",
    )
    .unwrap();
    std::fs::write(
        root.join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();
}

#[test]
fn non_interactive_sync_blocks_executable_content_without_yes() {
    let temp = tempfile::tempdir().unwrap();
    setup_executable_skill(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--non-interactive"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "policy blocked executable content",
        ));

    assert!(!temp.path().join(".claude/skills/review").exists());
}

#[test]
fn non_interactive_sync_allows_executable_content_with_yes() {
    let temp = tempfile::tempdir().unwrap();
    setup_executable_skill(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--non-interactive", "--yes"])
        .assert()
        .success();

    assert!(temp.path().join(".claude/skills/review/SKILL.md").is_file());
}
