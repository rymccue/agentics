use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_json_reports_yaml_parse_errors_as_json() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: [unterminated\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"valid\": false"))
        .stdout(predicate::str::contains("failed to parse manifest YAML"))
        .stderr(predicate::str::is_empty());
}
