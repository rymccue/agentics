use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use percent_encoding::percent_decode_str;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceRef {
    LocalPath(PathBuf),
    Git(GitSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitSource {
    pub(crate) repo: String,
    pub(crate) rev: Option<String>,
    pub(crate) subpath: Option<String>,
}

impl SourceRef {
    pub(crate) fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        if input.is_empty() {
            bail!("source reference cannot be empty");
        }

        if let Some(rest) = input.strip_prefix("git:") {
            return parse_git_source(rest);
        }

        if looks_like_scp_git_source(input) {
            return parse_git_source(input);
        }

        if input.starts_with("https://github.com/")
            || input.starts_with("https://raw.githubusercontent.com/")
        {
            return parse_github_url(input);
        }

        if let Some(rest) = input.strip_prefix("file:") {
            return Ok(Self::LocalPath(PathBuf::from(rest)));
        }

        Ok(Self::LocalPath(PathBuf::from(input)))
    }
}

fn looks_like_scp_git_source(input: &str) -> bool {
    let Some((user_and_host, path)) = input.split_once(':') else {
        return false;
    };
    user_and_host.contains('@')
        && !path.is_empty()
        && !input[..input.find(':').unwrap_or(0)].contains('/')
}

fn parse_git_source(input: &str) -> Result<SourceRef> {
    let (repo, rev_and_subpath) = split_once(input, "#");
    let (rev, subpath) = rev_and_subpath
        .map(|value| split_once(value, "//"))
        .unwrap_or(("", None));
    if repo.is_empty() {
        bail!("git source must include a repository");
    }
    Ok(SourceRef::Git(GitSource {
        repo: repo.to_string(),
        rev: (!rev.is_empty()).then(|| rev.to_string()),
        subpath: subpath
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    }))
}

fn parse_github_url(input: &str) -> Result<SourceRef> {
    let url = Url::parse(input).context("invalid GitHub URL")?;
    if url.fragment().is_some() {
        bail!("URL fragments are not supported in source references");
    }
    let host = url.host_str().unwrap_or_default();
    let segments: Vec<String> = url
        .path_segments()
        .map(|segments| segments.map(percent_decode_segment).collect())
        .unwrap_or_default();

    match host {
        "github.com" if segments.len() >= 5 && (segments[2] == "tree" || segments[2] == "blob") => {
            let repo = format!("https://github.com/{}/{}.git", segments[0], segments[1]);
            Ok(SourceRef::Git(GitSource {
                repo,
                rev: Some(segments[3].to_string()),
                subpath: Some(segments[4..].join("/")),
            }))
        }
        "github.com" if segments.len() == 2 => Ok(SourceRef::Git(GitSource {
            repo: format!("https://github.com/{}/{}.git", segments[0], segments[1]),
            rev: None,
            subpath: None,
        })),
        "raw.githubusercontent.com" if segments.len() >= 4 => Ok(SourceRef::Git(GitSource {
            repo: format!("https://github.com/{}/{}.git", segments[0], segments[1]),
            rev: Some(segments[2].to_string()),
            subpath: Some(segments[3..].join("/")),
        })),
        _ => bail!("unsupported GitHub URL shape"),
    }
}

fn percent_decode_segment(segment: &str) -> String {
    percent_decode_str(segment).decode_utf8_lossy().into_owned()
}

fn split_once<'a>(input: &'a str, delimiter: &str) -> (&'a str, Option<&'a str>) {
    match input.split_once(delimiter) {
        Some((left, right)) => (left, Some(right)),
        None => (input, None),
    }
}
