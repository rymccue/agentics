use assert_cmd::Command;

#[test]
fn init_can_include_catalog_declarations() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args([
            "init",
            "--catalog",
            "team=git:https://github.com/myorg/catalog.git#v1//catalog.yaml",
        ])
        .assert()
        .success();

    let manifest = std::fs::read_to_string(temp.path().join("agentics.yaml")).unwrap();
    assert!(manifest.contains("catalogs:"));
    assert!(manifest.contains("name: team"));
    assert!(manifest.contains("source: git:https://github.com/myorg/catalog.git#v1//catalog.yaml"));
}

#[test]
fn init_rejects_invalid_catalog_declaration() {
    let temp = tempfile::tempdir().unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["init", "--catalog", "not-a-pair"])
        .assert()
        .failure();
}
