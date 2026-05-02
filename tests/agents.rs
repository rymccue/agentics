use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_installs_claude_agent_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("agents")).expect("create agents dir");
    std::fs::write(
        temp.path().join("agents/reviewer.md"),
        "---\nname: reviewer\n---\n",
    )
    .expect("write agent");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: agent\n    name: reviewer\n    source: ./agents/reviewer.md\n    harnesses: [claude]\n",
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
        .stdout(predicate::str::contains(
            "installed .claude/agents/reviewer.md",
        ));

    assert_eq!(
        std::fs::read_to_string(temp.path().join(".claude/agents/reviewer.md"))
            .expect("agent copied"),
        "---\nname: reviewer\n---\n"
    );
}

#[test]
fn codex_agent_is_rejected_in_mvp() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("agents")).expect("create agents dir");
    std::fs::write(temp.path().join("agents/reviewer.md"), "agent\n").expect("write agent");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  codex:\n    enabled: true\ninstall:\n  - type: agent\n    name: reviewer\n    source: ./agents/reviewer.md\n    harnesses: [codex]\n",
    )
    .expect("write manifest");

    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported for harness"));
}
