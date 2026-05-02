use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_json_reports_invalid_manifest_errors() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: wrong\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("\"valid\": false")
                .and(predicate::str::contains("unsupported apiVersion")),
        );
}
