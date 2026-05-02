use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn status_reports_no_supported_actions_for_empty_supported_plan() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("No managed resources."));
}
