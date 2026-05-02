use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::*;

pub(crate) struct StagedSource {
    pub(crate) path: PathBuf,
    pub(crate) commit: String,
}

pub(crate) struct GitStageCache {
    pub(crate) root: PathBuf,
    by_repo_ref: BTreeMap<String, (PathBuf, String)>,
}

impl GitStageCache {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            by_repo_ref: BTreeMap::new(),
        }
    }
}

pub(crate) fn stage_git_source(git: &GitSource, cache: &mut GitStageCache) -> Result<StagedSource> {
    let rev = git
        .rev
        .as_deref()
        .filter(|rev| !rev.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("unpinned git source `{}`; specify #<rev>", git.repo))?;
    let key = git_ref_cache_key(&git.repo, rev);
    let (clone_path, commit) = if let Some((clone_path, commit)) = cache.by_repo_ref.get(&key) {
        (clone_path.clone(), commit.clone())
    } else {
        let clone_path = stage_git_checkout(&git.repo, rev, &cache.root)?;
        let commit = git_stdout_in(&clone_path, ["rev-parse", "HEAD"])?;
        cache
            .by_repo_ref
            .insert(key, (clone_path.clone(), commit.clone()));
        (clone_path, commit)
    };
    let path = git
        .subpath
        .as_deref()
        .map(|subpath| safe_join_git_subpath(&clone_path, subpath))
        .transpose()?
        .unwrap_or_else(|| clone_path.clone());
    if !path.exists() {
        bail!("git source subpath does not exist: {}", path.display());
    }
    Ok(StagedSource { path, commit })
}

pub(crate) fn stage_git_checkout(repo: &str, rev: &str, staging_root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(staging_root)
        .with_context(|| format!("failed to create staging root `{}`", staging_root.display()))?;
    let clone_path = staging_root
        .join(git_ref_cache_key(repo, rev))
        .with_extension("git-worktree");
    if clone_path.exists() {
        if git_stdout_in(&clone_path, ["rev-parse", "HEAD"])
            .ok()
            .as_deref()
            == Some(rev)
        {
            return Ok(clone_path);
        }
        remove_path(&clone_path)?;
    }
    run_git([
        "clone",
        "--quiet",
        "--no-checkout",
        repo,
        clone_path.to_string_lossy().as_ref(),
    ])?;
    run_git_in(&clone_path, ["checkout", "--quiet", rev])?;
    Ok(clone_path)
}

pub(crate) fn git_ref_cache_key(repo: &str, rev: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(repo.as_bytes());
    hasher.update(b"\0");
    hasher.update(rev.as_bytes());
    format!("git-{:x}", hasher.finalize())
}

pub(crate) fn safe_join_git_subpath(root: &Path, subpath: &str) -> Result<PathBuf> {
    let subpath = Path::new(subpath);
    if subpath.is_absolute()
        || subpath
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("git source subpath escapes checkout: {}", subpath.display());
    }
    Ok(root.join(subpath))
}

pub(crate) fn run_git<const N: usize>(args: [&str; N]) -> Result<()> {
    let status = ProcessCommand::new("git")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .status()
        .context("failed to execute git")?;
    if !status.success() {
        bail!("git command failed with status {status}");
    }
    Ok(())
}

pub(crate) fn run_git_in<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<()> {
    let status = ProcessCommand::new("git")
        .current_dir(cwd)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .status()
        .context("failed to execute git")?;
    if !status.success() {
        bail!("git command failed with status {status}");
    }
    Ok(())
}

pub(crate) fn git_stdout_in<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<String> {
    let output = ProcessCommand::new("git")
        .current_dir(cwd)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()
        .context("failed to execute git")?;
    if !output.status.success() {
        bail!("git command failed with status {}", output.status);
    }
    Ok(String::from_utf8(output.stdout)
        .context("git output was not utf-8")?
        .trim()
        .to_string())
}
