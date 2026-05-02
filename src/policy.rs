pub(crate) fn is_trusted_git_source(repo: &str, trusted_sources: &[String]) -> bool {
    let normalized_repo = normalize_git_repo_for_trust(repo);
    trusted_sources.iter().any(|trusted| {
        if let Some(prefix) = trusted.strip_suffix('*') {
            normalized_repo.starts_with(prefix.trim_end_matches('*'))
        } else {
            normalized_repo == *trusted
        }
    })
}

fn normalize_git_repo_for_trust(repo: &str) -> String {
    repo.trim_end_matches(".git")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("git@")
        .replace(':', "/")
}
