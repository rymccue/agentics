use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_dry_run_plans_shared_context_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("context")).expect("create context dir");
    std::fs::write(temp.path().join("context/AGENTS.md"), "# Team Context\n")
        .expect("write context");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\n  codex:\n    enabled: true\n  pi:\n    enabled: true\ninstall:\n  - type: context\n    name: agents\n    source: ./context/AGENTS.md\n    harnesses: [claude, codex, pi]\n",
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
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("would install AGENTS.md from ./context/AGENTS.md")
                .and(predicate::str::contains("owners: claude, codex, pi")),
        );
}

#[test]
fn sync_installs_shared_context_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("context")).expect("create context dir");
    std::fs::write(temp.path().join("context/AGENTS.md"), "# Team Context\n")
        .expect("write context");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  pi:\n    enabled: true\ninstall:\n  - type: context\n    name: agents\n    source: ./context/AGENTS.md\n    harnesses: [pi]\n",
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
        .success()
        .stdout(predicate::str::contains("installed AGENTS.md"));

    assert_eq!(
        std::fs::read_to_string(temp.path().join("AGENTS.md")).expect("context copied"),
        "# Team Context\n"
    );
}
