use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn file_resource_status_uses_content_integrity_not_source_filename() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("prompts")).expect("create prompts dir");
    std::fs::write(temp.path().join("prompts/source-name.md"), "Summarize.\n")
        .expect("write prompt");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: prompt\n    name: summarize\n    source: ./prompts/source-name.md\n    harnesses: [claude]\n",
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

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "installed .claude/commands/summarize.md",
        ));
}
