use assert_cmd::Command;

#[test]
fn installed_metadata_records_lockfile_hash() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .success();

    let metadata = std::fs::read_to_string(temp.path().join(".agentics/installed.yaml")).unwrap();
    assert!(metadata.contains("lockfileHash: sha256:"), "{metadata}");
}
