use assert_cmd::Command;

#[test]
fn basic_example_doctor_and_dry_run_work() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example = repo_root.join("examples/basic");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(&example)
        .arg("doctor")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(&example)
        .args(["update", "--check"])
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(&example)
        .args(["sync", "--dry-run"])
        .assert()
        .success();
}
