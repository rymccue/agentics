use assert_cmd::Command;

fn skill(root: &std::path::Path, name: &str, body: &str) {
    let skill_dir = root.join("skills").join(name);
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), body).unwrap();
}

#[test]
fn update_resource_refreshes_only_selected_lockfile_entry() {
    let temp = tempfile::tempdir().unwrap();
    skill(temp.path(), "review", "# Review v1\n");
    skill(temp.path(), "deploy", "# Deploy v1\n");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n  - type: skill\n    name: deploy\n    source: ./skills/deploy\n    harnesses: [claude]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();
    let original = std::fs::read_to_string(temp.path().join("agentics.lock.yaml")).unwrap();
    let original_review_integrity = lock_integrity_for(&original, "review");
    let original_deploy_integrity = lock_integrity_for(&original, "deploy");

    skill(temp.path(), "review", "# Review v2\n");
    skill(temp.path(), "deploy", "# Deploy v2\n");

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["update", "skill:review"])
        .assert()
        .success();

    let updated = std::fs::read_to_string(temp.path().join("agentics.lock.yaml")).unwrap();
    assert_ne!(
        lock_integrity_for(&updated, "review"),
        original_review_integrity
    );
    assert_eq!(
        lock_integrity_for(&updated, "deploy"),
        original_deploy_integrity
    );
}

#[test]
fn update_unknown_resource_fails() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["update", "skill:missing"])
        .assert()
        .failure();
}

fn lock_integrity_for(lockfile: &str, name: &str) -> String {
    let marker = format!("name: {name}\n");
    let start = lockfile.find(&marker).unwrap();
    lockfile[start..]
        .lines()
        .find_map(|line| line.trim().strip_prefix("integrity: "))
        .unwrap()
        .to_string()
}
