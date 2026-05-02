use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn pi_skill_root_can_use_native_pi_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").expect("write skill");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  pi:\n    enabled: true\n    skillRoot: pi\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [pi]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "would install .pi/skills/review from ./skills/review",
        ));
}
