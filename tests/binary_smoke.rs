use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn binary_version_smoke_test() {
    Command::cargo_bin("agentics")
        .expect("binary exists")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("agentics 0.1.0"));
}
