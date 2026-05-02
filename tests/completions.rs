use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn completions_generates_bash_script() {
    Command::cargo_bin("agentics")
        .expect("binary exists")
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("agentics").and(predicate::str::contains("sync")));
}

#[test]
fn help_lists_completions_command() {
    Command::cargo_bin("agentics")
        .expect("binary exists")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("completions"));
}
