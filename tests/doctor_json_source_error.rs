use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_json_reports_source_validation_errors() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./missing/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("\"valid\": false").and(predicate::str::contains(
                "local source for `review` is missing",
            )),
        );
}
