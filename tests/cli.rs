use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_core_commands() {
    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command.arg("--help").assert().success().stdout(
        predicate::str::contains("init")
            .and(predicate::str::contains("status"))
            .and(predicate::str::contains("sync"))
            .and(predicate::str::contains("update"))
            .and(predicate::str::contains("doctor")),
    );
}

#[test]
fn doctor_accepts_valid_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = temp.path().join("agentics.yaml");
    std::fs::write(
        &manifest,
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .expect("write manifest");

    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command
        .arg("--manifest")
        .arg(&manifest)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Manifest OK"));
}

#[test]
fn doctor_plain_output_omits_resource_hashes() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("skills/review")).expect("create skill");
    std::fs::write(temp.path().join("skills/review/SKILL.md"), "# Review\n").expect("write skill");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Manifest OK").and(predicate::str::contains("sha256:").not()),
        );
}

#[test]
fn doctor_rejects_invalid_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = temp.path().join("agentics.yaml");
    std::fs::write(
        &manifest,
        "apiVersion: wrong\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .expect("write manifest");

    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command
        .arg("--manifest")
        .arg(&manifest)
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported apiVersion"));
}
