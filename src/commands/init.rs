use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{manifest::is_valid_resource_name, *};

pub(crate) fn init(
    manifest_path: PathBuf,
    harnesses: Option<String>,
    catalogs: Vec<String>,
    gitignore: bool,
    force: bool,
) -> Result<()> {
    if manifest_path.exists() && !force {
        if gitignore {
            let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
            ensure_gitignore_patterns(manifest_dir)?;
            println!("Updated {}", manifest_dir.join(".gitignore").display());
            return Ok(());
        }
        bail!("manifest already exists: {}", manifest_path.display());
    }
    if let Some(parent) = manifest_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory `{}`", parent.display()))?;
    }
    fs::write(
        &manifest_path,
        starter_manifest(harnesses, catalogs)?.as_bytes(),
    )
    .with_context(|| format!("failed to write manifest `{}`", manifest_path.display()))?;
    println!("Created {}", manifest_path.display());
    if gitignore {
        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        ensure_gitignore_patterns(manifest_dir)?;
        println!("Updated {}", manifest_dir.join(".gitignore").display());
    }
    Ok(())
}

fn ensure_gitignore_patterns(manifest_dir: &Path) -> Result<()> {
    let path = manifest_dir.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut lines_to_add = Vec::new();
    for pattern in ["/.agentics", "/.agentics-owner", "*.agentics-owner"] {
        if !gitignore_contains_pattern(&existing, pattern) {
            lines_to_add.push(pattern);
        }
    }
    if lines_to_add.is_empty() {
        return Ok(());
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    for pattern in lines_to_add {
        updated.push_str(pattern);
        updated.push('\n');
    }
    write_file_atomically(&path, updated.as_bytes())
}

fn gitignore_contains_pattern(contents: &str, pattern: &str) -> bool {
    contents.lines().map(str::trim).any(|line| line == pattern)
}

pub(crate) fn gitignore_warnings(manifest_dir: &Path) -> Vec<String> {
    let contents = fs::read_to_string(manifest_dir.join(".gitignore")).unwrap_or_default();
    ["/.agentics", "/.agentics-owner", "*.agentics-owner"]
        .into_iter()
        .filter(|pattern| !gitignore_contains_pattern(&contents, pattern))
        .map(|pattern| {
            format!(
                ".gitignore is missing `{pattern}`; run `agentics init --gitignore` to add recommended metadata ignores"
            )
        })
        .collect()
}

fn starter_manifest(harnesses: Option<String>, catalogs: Vec<String>) -> Result<String> {
    let enabled = if let Some(harnesses) = harnesses {
        let mut enabled = BTreeSet::new();
        for harness in harnesses
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            match harness {
                "claude" => {
                    enabled.insert(HarnessName::Claude);
                }
                "codex" => {
                    enabled.insert(HarnessName::Codex);
                }
                "pi" => {
                    enabled.insert(HarnessName::Pi);
                }
                other => bail!("unsupported harness `{other}`"),
            }
        }
        if enabled.is_empty() {
            bail!("--harnesses must include at least one harness");
        }
        enabled
    } else {
        BTreeSet::from([HarnessName::Claude])
    };

    let mut manifest =
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n".to_string();
    for harness in [HarnessName::Claude, HarnessName::Codex, HarnessName::Pi] {
        if enabled.contains(&harness) {
            manifest.push_str(&format!("  {}:\n    enabled: true\n", harness.as_str()));
        }
    }
    if !catalogs.is_empty() {
        manifest.push_str("catalogs:\n");
        for catalog in catalogs {
            let Some((name, source)) = catalog.split_once('=') else {
                bail!("invalid catalog declaration `{catalog}`; expected name=source");
            };
            if !is_valid_resource_name(name) {
                bail!("invalid catalog name `{name}`");
            }
            if source.trim().is_empty() {
                bail!("catalog `{name}` has empty source");
            }
            manifest.push_str(&format!("  - name: {name}\n    source: {source}\n"));
        }
    }
    manifest.push_str("install: []\n");
    Ok(manifest)
}
