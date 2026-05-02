use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_rejects_unsupported_context_name() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("context")).expect("create context dir");
    std::fs::write(temp.path().join("context/TEAM.md"), "# Team\n").expect("write context");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: context\n    name: team\n    source: ./context/TEAM.md\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported context name"));
}
