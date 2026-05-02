use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_warns_when_skill_contains_nested_skill_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("skills/pm/write-spec")).expect("create nested");
    std::fs::write(
        temp.path().join("skills/pm/SKILL.md"),
        "---\ndescription: Product toolkit.\n---\n# PM\n",
    )
    .expect("write parent");
    std::fs::write(
        temp.path().join("skills/pm/write-spec/SKILL.md"),
        "---\ndescription: Write specs.\n---\n# Write Spec\n",
    )
    .expect("write nested");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: pm\n    source: ./skills/pm\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stderr(predicate::str::contains("contains nested skill files"));
}
