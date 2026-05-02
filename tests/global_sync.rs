use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_global_is_policy_blocked_by_default() {
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
        .args(["sync", "--global"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("policy blocked global installs"));
}

#[test]
fn sync_global_is_not_implemented_even_when_policy_allows_it() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\npolicy:\n  allowGlobalInstall: true\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--global"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "global installs are not supported in MVP",
        ));
}

#[test]
fn sync_help_lists_global_flag() {
    Command::cargo_bin("agentics")
        .unwrap()
        .args(["sync", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--global"));
}
