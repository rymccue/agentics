use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_force_still_refuses_unmanaged_existing_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").expect("write skill");
    let target = temp.path().join(".claude/skills/review");
    std::fs::create_dir_all(&target).expect("create target");
    std::fs::write(target.join("SKILL.md"), "# Local\n").expect("write local");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
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
        .args(["sync", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "refusing to overwrite unmanaged target",
        ));
}
