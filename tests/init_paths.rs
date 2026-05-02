use assert_cmd::Command;

#[test]
fn init_creates_parent_directories_for_custom_manifest_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = temp.path().join("config/agentics/agentics.yaml");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["--manifest", manifest.to_str().expect("utf8 path"), "init"])
        .assert()
        .success();

    assert!(manifest.is_file());
}
