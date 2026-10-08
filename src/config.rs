use crate::util;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub project: Project,
    pub sources: Sources,
    pub output: Output,
    pub models: Models,
    pub providers: BTreeMap<String, ProviderSettings>,
    pub processing: Processing,
    pub privacy: Privacy,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            project: Project::default(),
            sources: Sources::default(),
            output: Output::default(),
            models: Models::default(),
            providers: BTreeMap::new(),
            processing: Processing::default(),
            privacy: Privacy::default(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Project {
    pub name: String,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            name: "my-project".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sources {
    pub roots: Vec<SourceRoot>,
    pub exclude: Vec<String>,
}
impl Default for Sources {
    fn default() -> Self {
        Self {
            roots: vec![SourceRoot {
                id: "docs".into(),
                path: "./docs".into(),
            }],
            exclude: vec!["**/node_modules/**".into(), "**/target/**".into()],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRoot {
    pub id: String,
    pub path: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Output {
    pub wiki_dir: PathBuf,
    pub state_dir: PathBuf,
}
impl Default for Output {
    fn default() -> Self {
        Self {
            wiki_dir: "./lore".into(),
            state_dir: "./.lore".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelRole {
    pub provider: String,
    pub model: String,
    pub enabled: bool,
}
impl Default for ModelRole {
    fn default() -> Self {
        Self {
            provider: "ollama".into(),
            model: "gemma4:12b".into(),
            enabled: true,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Models {
    pub generative: ModelRole,
    pub decision: Option<ModelRole>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderSettings {
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Privacy {
    pub local_only: bool,
}
impl Default for Privacy {
    fn default() -> Self {
        Self { local_only: true }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Processing {
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub max_file_bytes: usize,
    pub max_section_bytes: usize,
    pub max_context_bytes: usize,
    pub candidate_limit: usize,
    pub verify_synthesis: bool,
}
impl Default for Processing {
    fn default() -> Self {
        Self {
            timeout_seconds: 120,
            retry_attempts: 2,
            max_file_bytes: 2_000_000,
            max_section_bytes: 12_000,
            max_context_bytes: 64_000,
            candidate_limit: 40,
            verify_synthesis: true,
        }
    }
}
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub config: Config,
    pub config_path: PathBuf,
    pub base: PathBuf,
    pub wiki: PathBuf,
    pub state: PathBuf,
    pub roots: Vec<(String, PathBuf)>,
    pub project_id: String,
    pub fingerprint: String,
}
impl ResolvedConfig {
    pub fn load(path: &Path) -> Result<Self> {
        ensure!(
            !std::fs::symlink_metadata(path)?.file_type().is_symlink(),
            "configuration must not be a symlink"
        );
        let absolute = path.canonicalize().context("locate lore.yml")?;
        let config: Config = serde_yaml::from_str(&util::read_limited(&absolute, 128_000)?)
            .context("invalid lore.yml")?;
        Self::resolve(config, &absolute)
    }
    pub fn resolve(config: Config, config_path: &Path) -> Result<Self> {
        ensure!(
            config.schema_version == 1,
            "unsupported configuration schema_version"
        );
        ensure!(
            !config.project.name.trim().is_empty()
                && config.project.name.len() <= 200
                && !config.project.name.chars().any(char::is_control),
            "project.name cannot be empty"
        );
        let base = config_path
            .parent()
            .context("configuration has no parent")?
            .canonicalize()?;
        let wiki = util::absolute(&base, &config.output.wiki_dir)?;
        let state = util::absolute(&base, &config.output.state_dir)?;
        ensure!(
            !wiki.starts_with(&state) && !state.starts_with(&wiki),
            "wiki and state directories must not overlap"
        );
        ensure!(
            !base.starts_with(&wiki) && !base.starts_with(&state),
            "outputs cannot contain the project root"
        );
        ensure!(
            !config.sources.roots.is_empty(),
            "configure at least one Markdown source root"
        );
        let mut ids = BTreeSet::new();
        let mut roots = Vec::new();
        for root in &config.sources.roots {
            ensure!(
                !root.id.is_empty()
                    && root.id.len() <= 80
                    && root
                        .id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'),
                "invalid source root ID"
            );
            ensure!(ids.insert(root.id.clone()), "duplicate source root ID");
            let p = util::absolute(&base, &root.path)?;
            ensure!(
                !p.starts_with(&wiki) && !p.starts_with(&state),
                "a source root cannot be generated output"
            );
            ensure!(
                !roots
                    .iter()
                    .any(|(_, old): &(String, PathBuf)| p.starts_with(old) || old.starts_with(&p)),
                "source roots overlap; each file must have one owner"
            );
            roots.push((root.id.clone(), p));
        }
        let p = &config.processing;
        ensure!(
            (1..=600).contains(&p.timeout_seconds) && p.retry_attempts <= 5,
            "invalid timeout/retry budget"
        );
        ensure!(
            p.max_section_bytes >= 512
                && p.max_section_bytes <= 100_000
                && p.max_file_bytes >= p.max_section_bytes
                && p.max_file_bytes <= 50_000_000,
            "invalid file/section limits"
        );
        ensure!(
            p.max_context_bytes >= p.max_section_bytes * 2 + 8192
                && p.max_context_bytes <= 1_000_000,
            "invalid context budget"
        );
        ensure!(
            (1..=100).contains(&p.candidate_limit),
            "candidate_limit must be 1..100"
        );
        ensure!(
            config.models.generative.enabled && config.models.generative.provider != "typesafe",
            "a supported generative model must be enabled"
        );
        for role in std::iter::once(&config.models.generative)
            .chain(config.models.decision.iter().filter(|r| r.enabled))
        {
            ensure!(
                ["ollama", "openai", "typesafe"].contains(&role.provider.as_str()),
                "unsupported provider {}",
                role.provider
            );
            ensure!(!role.model.trim().is_empty(), "model name is required");
            if config.privacy.local_only {
                ensure!(
                    role.provider == "ollama",
                    "local_only forbids hosted providers; explicitly set privacy.local_only: false to opt in"
                );
                ensure!(
                    !role.model.contains(":cloud") && !role.model.ends_with("-cloud"),
                    "cloud model tag is forbidden in local_only mode"
                );
            }
        }
        let fingerprint =
            util::json_digest(&(env!("CARGO_PKG_VERSION"), "pipeline-v5-quality", &config))?;
        let project_id = format!("project_{}", &util::digest(&config.project.name)[7..31]);
        Ok(Self {
            config,
            config_path: config_path.to_owned(),
            base,
            wiki,
            state,
            roots,
            project_id,
            fingerprint,
        })
    }
}
