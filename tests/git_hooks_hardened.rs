use assert_cmd::Command;

fn git(args: &[&str], cwd: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

#[test]
#[cfg(unix)]
fn update_disables_user_configured_git_hooks() {
    use std::os::unix::fs::PermissionsExt;

    let repo = tempfile::tempdir().unwrap();
    git(&["init"], repo.path());
    git(&["config", "user.email", "test@example.com"], repo.path());
    git(&["config", "user.name", "Test User"], repo.path());
    let skill = repo.path().join("skills/review");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "# Review\n").unwrap();
    git(&["add", "."], repo.path());
    git(&["commit", "-m", "add skill"], repo.path());
    let commit = git(&["rev-parse", "HEAD"], repo.path());

    let hooks = tempfile::tempdir().unwrap();
    let hook = hooks.path().join("post-checkout");
    std::fs::write(&hook, "#!/bin/sh\necho hook should not run >&2\nexit 42\n").unwrap();
    let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook, permissions).unwrap();

    let git_config = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        git_config.path(),
        format!("[core]\n\thooksPath = {}\n", hooks.path().display()),
    )
    .unwrap();

    let work = tempfile::tempdir().unwrap();
    std::fs::write(
        work.path().join("agentics.yaml"),
        format!(
            "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:{}#{}//skills/review\n    harnesses: [claude]\n",
            repo.path().display(),
            commit
        ),
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(work.path())
        .env("GIT_CONFIG_GLOBAL", git_config.path())
        .arg("update")
        .assert()
        .success();
}
