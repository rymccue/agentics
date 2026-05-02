use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::commands::init::gitignore_warnings;
use crate::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorReport {
    valid: bool,
    manifest: String,
    git_available: bool,
    resources: Vec<DoctorResource>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DoctorResource {
    name: String,
    resource_type: String,
    source: String,
    integrity: Option<String>,
    warnings: Vec<String>,
}

pub(crate) fn doctor(manifest_path: PathBuf, json: bool, strict: bool) -> Result<()> {
    let manifest_text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read manifest `{}`", manifest_path.display()))?;
    let git_available = git_is_available();
    let manifest = match Manifest::parse_yaml(&manifest_text) {
        Ok(manifest) => manifest,
        Err(error) if json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&DoctorReport {
                    valid: false,
                    manifest: manifest_path.display().to_string(),
                    git_available,
                    resources: Vec::new(),
                    warnings: Vec::new(),
                    errors: vec![error.to_string()],
                })?
            );
            std::process::exit(1);
        }
        Err(error) => return Err(error),
    };

    match manifest.validate() {
        Ok(()) => match validate_doctor_lockfile(&manifest_path)
            .and_then(|_| build_doctor_resources(&manifest, &manifest_path, json))
        {
            Ok(resources) => {
                let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
                let mut warnings = resources
                    .iter()
                    .flat_map(|resource| {
                        resource.warnings.iter().map(|warning| {
                            format!("{}:{}: {warning}", resource.resource_type, resource.name)
                        })
                    })
                    .collect::<Vec<_>>();
                warnings.extend(gitignore_warnings(manifest_dir));
                if strict && !warnings.is_empty() {
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&DoctorReport {
                                valid: false,
                                manifest: manifest_path.display().to_string(),
                                git_available,
                                resources,
                                warnings: warnings.clone(),
                                errors: warnings,
                            })?
                        );
                        std::process::exit(1);
                    }
                    for warning in &warnings {
                        eprintln!("strict: {warning}");
                    }
                    bail!("strict doctor failed with {} warning(s)", warnings.len());
                }
                for warning in gitignore_warnings(manifest_dir) {
                    eprintln!("warning: {warning}");
                }
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&DoctorReport {
                            valid: true,
                            manifest: manifest_path.display().to_string(),
                            git_available,
                            resources,
                            warnings,
                            errors: Vec::new(),
                        })?
                    );
                } else {
                    if git_available {
                        println!("Git OK");
                    } else {
                        eprintln!(
                            "warning: git executable was not found; git sources cannot be updated"
                        );
                    }
                    println!("Manifest OK: {}", manifest_path.display());
                }
                Ok(())
            }
            Err(error) if json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&DoctorReport {
                        valid: false,
                        manifest: manifest_path.display().to_string(),
                        git_available,
                        resources: Vec::new(),
                        warnings: Vec::new(),
                        errors: vec![error.to_string()],
                    })?
                );
                std::process::exit(1);
            }
            Err(error) => Err(error),
        },
        Err(errors) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&DoctorReport {
                        valid: false,
                        manifest: manifest_path.display().to_string(),
                        git_available,
                        resources: Vec::new(),
                        warnings: Vec::new(),
                        errors: errors.into_iter().map(|error| error.to_string()).collect(),
                    })?
                );
            } else {
                eprintln!("Manifest invalid: {}", manifest_path.display());
                for error in errors {
                    eprintln!("- {error}");
                }
            }
            std::process::exit(1);
        }
    }
}

fn resource_validation_warnings(entry: &InstallEntry, source: &Path) -> Result<Vec<String>> {
    match entry.resource_type {
        ResourceType::Skill => {
            let mut warnings = skill_metadata_warnings(&entry.name, &source.join("SKILL.md"))?;
            warnings.extend(nested_skill_warnings(&entry.name, source)?);
            Ok(warnings)
        }
        ResourceType::Prompt => markdown_frontmatter_warnings("prompt", &entry.name, source),
        ResourceType::Agent => markdown_frontmatter_warnings("agent", &entry.name, source),
        ResourceType::Context => Ok(Vec::new()),
        ResourceType::Extension | ResourceType::Package | ResourceType::Hook => Ok(Vec::new()),
    }
}

fn markdown_frontmatter_warnings(kind: &str, name: &str, path: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read {kind} metadata `{}`", path.display()))?;
    let mut warnings = Vec::new();
    if let Some(frontmatter) = text.strip_prefix("---\n") {
        let Some((frontmatter, _body)) = frontmatter.split_once("\n---") else {
            warnings.push(format!("{kind} `{name}` has unterminated YAML frontmatter"));
            return Ok(warnings);
        };
        if let Err(error) = serde_yml::from_str::<serde_yml::Value>(frontmatter) {
            warnings.push(format!(
                "{kind} `{name}` has invalid YAML frontmatter: {error}"
            ));
        }
    }
    Ok(warnings)
}

fn nested_skill_warnings(name: &str, source: &Path) -> Result<Vec<String>> {
    let mut skill_files = Vec::new();
    collect_files(source, source, &mut skill_files)?;
    skill_files.sort();
    let nested = skill_files
        .into_iter()
        .filter(|relative| relative != Path::new("SKILL.md"))
        .filter(|relative| relative.file_name().and_then(|name| name.to_str()) == Some("SKILL.md"))
        .map(|relative| relative.display().to_string())
        .collect::<Vec<_>>();
    if nested.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(vec![format!(
            "skill `{name}` contains nested skill files: {}; install the parent bundle when child skills depend on shared relative references",
            nested.join(", ")
        )])
    }
}

fn skill_metadata_warnings(name: &str, skill_file: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(skill_file)
        .with_context(|| format!("failed to read skill metadata `{}`", skill_file.display()))?;
    let mut warnings = Vec::new();
    let Some(frontmatter) = text.strip_prefix("---\n") else {
        warnings.push(format!(
            "skill `{name}` SKILL.md is missing YAML frontmatter"
        ));
        return Ok(warnings);
    };
    let Some((frontmatter, _body)) = frontmatter.split_once("\n---") else {
        warnings.push(format!(
            "skill `{name}` SKILL.md has unterminated YAML frontmatter"
        ));
        return Ok(warnings);
    };
    let metadata: serde_yml::Value = match serde_yml::from_str(frontmatter) {
        Ok(metadata) => metadata,
        Err(error) => {
            warnings.push(format!(
                "skill `{name}` SKILL.md has invalid YAML frontmatter: {error}"
            ));
            return Ok(warnings);
        }
    };
    let description = metadata
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yml::Value::String("description".to_string())))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if description.is_empty() {
        warnings.push(format!(
            "skill `{name}` SKILL.md frontmatter is missing non-empty description"
        ));
    }
    Ok(warnings)
}

fn validate_doctor_lockfile(manifest_path: &Path) -> Result<()> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    load_lockfile(manifest_dir)?;
    Ok(())
}

fn git_is_available() -> bool {
    ProcessCommand::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn build_doctor_resources(
    manifest: &Manifest,
    manifest_path: &Path,
    _json: bool,
) -> Result<Vec<DoctorResource>> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut resources = Vec::with_capacity(manifest.install.len());
    for entry in &manifest.install {
        let source_ref = SourceRef::parse(&entry.source)?;
        match source_ref {
            SourceRef::LocalPath(path) => {
                let absolute_or_relative = if path.is_absolute() {
                    path
                } else {
                    manifest_dir.join(path)
                };
                if !absolute_or_relative.exists() {
                    bail!(
                        "local source for `{}` is missing: {}",
                        entry.name,
                        absolute_or_relative.display()
                    );
                }
                if let Some(kind) = action_kind_for(entry.resource_type) {
                    validate_source_shape(entry, &absolute_or_relative, kind)?;
                }
                let mut warnings = resource_validation_warnings(entry, &absolute_or_relative)?;
                warnings.extend(executable_content_warnings(&absolute_or_relative)?);
                warnings = filter_allowed_executable_warnings(manifest, entry, warnings);
                for warning in &warnings {
                    eprintln!("warning: {warning}");
                }
                let integrity = hash_path(&absolute_or_relative)?;
                resources.push(DoctorResource {
                    name: entry.name.clone(),
                    resource_type: entry.resource_type.as_str().to_string(),
                    source: entry.source.clone(),
                    integrity: Some(integrity),
                    warnings,
                });
            }
            SourceRef::Git(git) => {
                let mut warnings = Vec::new();
                if git.rev.as_deref().unwrap_or_default().is_empty() {
                    warnings.push(format!(
                        "unpinned git source `{}`; `agentics update` will require #<rev>",
                        git.repo
                    ));
                }
                if !manifest.policy.trusted_sources.is_empty()
                    && !is_trusted_git_source(&git.repo, &manifest.policy.trusted_sources)
                {
                    warnings.push(format!("untrusted git source `{}`", git.repo));
                }
                for warning in &warnings {
                    eprintln!("warning: {warning}");
                }
                resources.push(DoctorResource {
                    name: entry.name.clone(),
                    resource_type: entry.resource_type.as_str().to_string(),
                    source: entry.source.clone(),
                    integrity: None,
                    warnings,
                });
            }
        }
    }
    Ok(resources)
}
