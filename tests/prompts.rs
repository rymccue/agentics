use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_dry_run_plans_prompt_targets_for_claude_and_pi() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("prompts")).expect("create prompts dir");
    std::fs::write(temp.path().join("prompts/review.md"), "Review this.\n").expect("write prompt");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\n  pi:\n    enabled: true\ninstall:\n  - type: prompt\n    name: review\n    source: ./prompts/review.md\n    harnesses: [claude, pi]\n",
    )
    .expect("write manifest");

    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains(
                "would install .claude/commands/review.md from ./prompts/review.md",
            )
            .and(predicate::str::contains(
                "would install .pi/prompts/review.md from ./prompts/review.md",
            )),
        );
}

#[test]
fn sync_installs_prompt_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("prompts")).expect("create prompts dir");
    std::fs::write(temp.path().join("prompts/review.md"), "Review this.\n").expect("write prompt");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  pi:\n    enabled: true\ninstall:\n  - type: prompt\n    name: review\n    source: ./prompts/review.md\n    harnesses: [pi]\n",
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

    assert_eq!(
        std::fs::read_to_string(temp.path().join(".pi/prompts/review.md")).expect("prompt copied"),
        "Review this.\n"
    );
}
