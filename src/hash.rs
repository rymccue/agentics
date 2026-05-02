use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

pub(crate) fn hash_path(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect `{}`", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("refusing to hash symlink `{}`", path.display());
    }

    let mut hasher = Sha256::new();
    if metadata.is_file() {
        // Standalone file resources may be installed under a harness-specific filename
        // (for example a source prompt file can become `.claude/commands/<name>.md`).
        // Hash only content for files so status compares the managed payload rather
        // than the incidental source/target basename.
        hash_file(path, "", &mut hasher)?;
    } else if metadata.is_dir() {
        let mut files = Vec::new();
        collect_regular_files(path, path, &mut files)?;
        files.sort();
        for relative in files {
            hash_file(
                &path.join(&relative),
                &relative.to_string_lossy(),
                &mut hasher,
            )?;
        }
    } else {
        bail!("unsupported file type `{}`", path.display());
    }

    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn collect_regular_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(current)
        .with_context(|| format!("failed to read directory `{}`", current.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect `{}`", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("refusing to hash symlink `{}`", path.display());
        }
        if metadata.is_dir() {
            collect_regular_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path.strip_prefix(root)?.to_path_buf();
            if relative != Path::new(".agentics-owner") {
                files.push(relative);
            }
        } else {
            bail!("unsupported file type `{}`", path.display());
        }
    }

    Ok(())
}

fn hash_file(path: &Path, logical_name: &str, hasher: &mut Sha256) -> Result<()> {
    hasher.update(b"file\0");
    hasher.update(logical_name.as_bytes());
    hasher.update(b"\0");

    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open `{}`", path.display()))?;
    let mut buffer = [0_u8; 8192];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read `{}`", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    hasher.update(b"\0");
    Ok(())
}
