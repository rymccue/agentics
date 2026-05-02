use assert_cmd::Command;

fn init_repo(repo: &std::path::Path) -> String {
    std::fs::create_dir_all(repo.join("skills/review")).expect("create skill dir");
    std::fs::create_dir_all(repo.join("prompts")).expect("create prompts dir");
    std::fs::write(repo.join("skills/review/SKILL.md"), "# Review Skill\n").expect("write skill");
    std::fs::write(repo.join("prompts/review.md"), "Review prompt\n").expect("write prompt");
    Command::new("git")
        .current_dir(repo)
        .arg("init")
        .assert()
        .success();
    Command::new("git")
        .current_dir(repo)
        .args(["config", "user.email", "test@example.com"])
        .assert()
        .success();
    Command::new("git")
        .current_dir(repo)
        .args(["config", "user.name", "Test User"])
        .assert()
        .success();
    Command::new("git")
        .current_dir(repo)
        .args(["add", "."])
        .assert()
        .success();
    Command::new("git")
        .current_dir(repo)
        .args(["commit", "-m", "initial"])
        .assert()
        .success();
    String::from_utf8(
        Command::new("git")
            .current_dir(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse")
            .stdout,
    )
    .expect("utf8")
    .trim()
    .to_string()
}

#[test]
fn git_cache_keys_include_resource_type_and_name() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("resources");
    let commit = init_repo(&repo);
    std::fs::write(
        temp.path().join("agentics.yaml"),
        format!(
            "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:{}#{}//skills/review\n    harnesses: [claude]\n  - type: prompt\n    name: review\n    source: git:{}#{}//prompts/review.md\n    harnesses: [claude]\n",
            repo.display(), commit, repo.display(), commit
        ),
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();
    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(temp.path().join(".claude/skills/review/SKILL.md"))
            .expect("skill installed"),
        "# Review Skill\n"
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join(".claude/commands/review.md"))
            .expect("prompt installed"),
        "Review prompt\n"
    );
}
