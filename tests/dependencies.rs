use assert_cmd::Command;
use predicates::prelude::*;

fn fixture_skill(root: &std::path::Path, name: &str) {
    let skill_dir = root.join("skills").join(name);
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), format!("# {name}\n")).unwrap();
}

fn write_manifest(root: &std::path::Path, manifest: &str) {
    std::fs::write(root.join("agentics.yaml"), manifest).unwrap();
}

#[test]
fn update_writes_dependencies_in_topological_order() {
    let temp = tempfile::tempdir().unwrap();
    fixture_skill(temp.path(), "shared-shell");
    fixture_skill(temp.path(), "code-review");
    write_manifest(
        temp.path(),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: code-review\n    source: ./skills/code-review\n    harnesses: [claude]\n    requires:\n      - skill:shared-shell\n  - type: skill\n    name: shared-shell\n    source: ./skills/shared-shell\n    harnesses: [claude]\n",
    );

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["update"])
        .assert()
        .success();

    let lockfile = std::fs::read_to_string(temp.path().join("agentics.lock.yaml")).unwrap();
    let shared = lockfile.find("name: shared-shell").unwrap();
    let review = lockfile.find("name: code-review").unwrap();
    assert!(
        shared < review,
        "dependency should be locked before dependent:\n{lockfile}"
    );
    assert!(lockfile.contains("dependencies:\n  - skill:shared-shell"));
}

#[test]
fn sync_dry_run_orders_dependencies_before_dependents() {
    let temp = tempfile::tempdir().unwrap();
    fixture_skill(temp.path(), "a-main");
    fixture_skill(temp.path(), "z-dep");
    write_manifest(
        temp.path(),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: a-main\n    source: ./skills/a-main\n    harnesses: [claude]\n    requires:\n      - skill:z-dep\n  - type: skill\n    name: z-dep\n    source: ./skills/z-dep\n    harnesses: [claude]\n",
    );

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["update"])
        .assert()
        .success();

    let output = Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let dependency = stdout.find(".claude/skills/z-dep").unwrap();
    let dependent = stdout.find(".claude/skills/a-main").unwrap();
    assert!(
        dependency < dependent,
        "dependency should be planned before dependent:\n{stdout}"
    );
}

#[test]
fn doctor_rejects_missing_dependency() {
    let temp = tempfile::tempdir().unwrap();
    fixture_skill(temp.path(), "code-review");
    write_manifest(
        temp.path(),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: code-review\n    source: ./skills/code-review\n    harnesses: [claude]\n    requires:\n      - skill:missing\n",
    );

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["doctor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "missing dependency `skill:missing`",
        ));
}

#[test]
fn doctor_rejects_dependency_cycles() {
    let temp = tempfile::tempdir().unwrap();
    fixture_skill(temp.path(), "a");
    fixture_skill(temp.path(), "b");
    write_manifest(
        temp.path(),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: a\n    source: ./skills/a\n    harnesses: [claude]\n    requires:\n      - skill:b\n  - type: skill\n    name: b\n    source: ./skills/b\n    harnesses: [claude]\n    requires:\n      - skill:a\n",
    );

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["doctor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("dependency cycle"));
}
