use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn update_help_lists_dry_run_check_and_resource_argument() {
    Command::cargo_bin("agentics")
        .unwrap()
        .args(["update", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[RESOURCE]").or(predicate::str::contains("[resource]")))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--check"));
}
