use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::Manifest;

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PiSkillRoot {
    #[default]
    Agents,
    Pi,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ResourceType {
    Skill,
    Context,
    Prompt,
    Agent,
    Extension,
    Package,
    Hook,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum HarnessName {
    Claude,
    Codex,
    Pi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionKind {
    Directory,
    File,
}

impl ResourceType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Context => "context",
            Self::Prompt => "prompt",
            Self::Agent => "agent",
            Self::Extension => "extension",
            Self::Package => "package",
            Self::Hook => "hook",
        }
    }
}

impl HarnessName {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Pi => "pi",
        }
    }
}

pub(crate) fn is_supported_resource_for_harness(
    resource_type: ResourceType,
    harness: HarnessName,
) -> bool {
    match resource_type {
        ResourceType::Skill | ResourceType::Context => true,
        ResourceType::Prompt => matches!(harness, HarnessName::Claude | HarnessName::Pi),
        ResourceType::Agent => matches!(harness, HarnessName::Claude),
        ResourceType::Extension | ResourceType::Package | ResourceType::Hook => false,
    }
}

pub(crate) fn action_kind_for(resource_type: ResourceType) -> Option<ActionKind> {
    match resource_type {
        ResourceType::Skill => Some(ActionKind::Directory),
        ResourceType::Context | ResourceType::Prompt | ResourceType::Agent => {
            Some(ActionKind::File)
        }
        ResourceType::Extension | ResourceType::Package | ResourceType::Hook => None,
    }
}

pub(crate) fn target_for(
    manifest: &Manifest,
    resource_type: ResourceType,
    harness: HarnessName,
    name: &str,
) -> Option<PathBuf> {
    match resource_type {
        ResourceType::Skill => Some(skill_target(manifest, harness, name)),
        ResourceType::Context => context_target(harness, name),
        ResourceType::Prompt => prompt_target(harness, name),
        ResourceType::Agent => agent_target(harness, name),
        ResourceType::Extension | ResourceType::Package | ResourceType::Hook => None,
    }
}

fn context_target(_harness: HarnessName, name: &str) -> Option<PathBuf> {
    (name == "agents").then(|| PathBuf::from("AGENTS.md"))
}

pub(crate) fn skill_target(manifest: &Manifest, harness: HarnessName, name: &str) -> PathBuf {
    match harness {
        HarnessName::Claude => PathBuf::from(".claude").join("skills").join(name),
        HarnessName::Codex => PathBuf::from(".agents").join("skills").join(name),
        HarnessName::Pi => match manifest.harnesses.pi.skill_root {
            PiSkillRoot::Agents => PathBuf::from(".agents").join("skills").join(name),
            PiSkillRoot::Pi => PathBuf::from(".pi").join("skills").join(name),
        },
    }
}

fn prompt_target(harness: HarnessName, name: &str) -> Option<PathBuf> {
    match harness {
        HarnessName::Claude => Some(
            PathBuf::from(".claude")
                .join("commands")
                .join(format!("{name}.md")),
        ),
        HarnessName::Pi => Some(
            PathBuf::from(".pi")
                .join("prompts")
                .join(format!("{name}.md")),
        ),
        HarnessName::Codex => None,
    }
}

fn agent_target(harness: HarnessName, name: &str) -> Option<PathBuf> {
    match harness {
        HarnessName::Claude => Some(
            PathBuf::from(".claude")
                .join("agents")
                .join(format!("{name}.md")),
        ),
        HarnessName::Codex | HarnessName::Pi => None,
    }
}
