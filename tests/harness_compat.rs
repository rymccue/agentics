use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_rejects_codex_prompt_in_mvp() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("prompts")).expect("create prompts dir");
    std::fs::write(temp.path().join("prompts/review.md"), "review\n").expect("write prompt");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  codex:\n    enabled: true\ninstall:\n  - type: prompt\n    name: review\n    source: ./prompts/review.md\n    harnesses: [codex]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported for harness"));
}
