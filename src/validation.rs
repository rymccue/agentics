use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::*;

pub(crate) fn validate_source_shape(
    entry: &InstallEntry,
    source: &Path,
    kind: ActionKind,
) -> Result<()> {
    match kind {
        ActionKind::Directory => {
            let skill_file = source.join("SKILL.md");
            if !skill_file.is_file() {
                bail!("skill `{}` is missing {}", entry.name, skill_file.display());
            }
        }
        ActionKind::File => {
            if !source.is_file() {
                bail!(
                    "{} `{}` is missing file {}",
                    entry.resource_type.as_str(),
                    entry.name,
                    source.display()
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn source_policy_warnings(manifest: &Manifest, source_ref: &SourceRef) -> Vec<String> {
    let SourceRef::Git(git) = source_ref else {
        return Vec::new();
    };
    if manifest.policy.trusted_sources.is_empty()
        || is_trusted_git_source(&git.repo, &manifest.policy.trusted_sources)
    {
        return Vec::new();
    }
    vec![format!("untrusted git source `{}`", git.repo)]
}

pub(crate) fn executable_content_warnings(path: &Path) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    if path.is_file() {
        if is_executable_like(path)? {
            warnings.push("contains executable content".to_string());
        }
        return Ok(warnings);
    }

    let mut files = Vec::new();
    collect_files(path, path, &mut files)?;
    files.sort();
    for relative in files {
        let file = path.join(&relative);
        if is_executable_like(&file)? {
            warnings.push(format!(
                "contains executable content {}",
                relative.display()
            ));
        }
    }
    Ok(warnings)
}

pub(crate) fn filter_allowed_executable_warnings(
    manifest: &Manifest,
    entry: &InstallEntry,
    warnings: Vec<String>,
) -> Vec<String> {
    if !is_allowed_executable_resource(manifest, entry) {
        return warnings;
    }
    warnings
        .into_iter()
        .filter(|warning| !warning.starts_with("contains executable content"))
        .collect()
}

fn is_allowed_executable_resource(manifest: &Manifest, entry: &InstallEntry) -> bool {
    let id = format!("{}:{}", entry.resource_type.as_str(), entry.name);
    manifest
        .policy
        .allowed_executable_resources
        .iter()
        .any(|allowed| allowed == &id)
}

pub(crate) fn collect_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read directory `{}`", current.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read `{}`", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect `{}`", path.display()))?;
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(
                path.strip_prefix(root)
                    .with_context(|| format!("failed to relativize `{}`", path.display()))?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn is_executable_like(path: &Path) -> Result<bool> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if matches!(
        extension,
        "sh" | "bash" | "zsh" | "fish" | "py" | "rb" | "pl" | "js" | "ts" | "ps1" | "bat" | "cmd"
    ) {
        return Ok(true);
    }
    if path.file_name().and_then(|name| name.to_str()) == Some("package.json") {
        return Ok(true);
    }

    let mut file = fs::File::open(path)
        .with_context(|| format!("failed to inspect executable content `{}`", path.display()))?;
    let mut prefix = [0_u8; 2];
    let bytes_read = file
        .read(&mut prefix)
        .with_context(|| format!("failed to read `{}`", path.display()))?;
    if bytes_read == 2 && prefix == *b"#!" {
        return Ok(true);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .with_context(|| format!("failed to inspect `{}`", path.display()))?
            .permissions()
            .mode();
        if mode & 0o111 != 0 {
            return Ok(true);
        }
    }

    Ok(false)
}
