pub use crate::domain::ImportKind;
use crate::{domain::SourceMaterial, inference::ReasoningEffort, util};
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
    /// Read-only, file-based snapshots from independently maintained systems.
    pub imports: Vec<ImportSource>,
    pub output: Output,
    pub models: Models,
    pub providers: BTreeMap<String, ProviderSettings>,
    pub processing: Processing,
    pub privacy: Privacy,
    /// Task guidance settings; deterministic `context --fast` ignores these.
    pub context: ContextSettings,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            project: Project::default(),
            sources: Sources::default(),
            imports: Vec::new(),
            output: Output::default(),
            models: Models::default(),
            providers: BTreeMap::new(),
            processing: Processing::default(),
            privacy: Privacy::default(),
            context: ContextSettings::default(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportSource {
    pub id: String,
    pub kind: ImportKind,
    pub path: PathBuf,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub include_memories: bool,
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
                material: SourceMaterial::Primary,
                origin: None,
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
    /// Importer-declared provenance, never a claim of verified correctness.
    #[serde(default)]
    pub material: SourceMaterial,
    /// Optional upstream identity (for example a repository URL). Lore records
    /// this label verbatim and never fetches it or treats it as primary evidence.
    #[serde(default)]
    pub origin: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reasoning {
    /// Disable to omit reasoning.effort (provider/model default or legacy models).
    pub enabled: bool,
    /// Fallback for new or synthetic generative tasks.
    pub default: ReasoningEffort,
    pub extraction: ReasoningEffort,
    pub reconciliation: ReasoningEffort,
    pub synthesis: ReasoningEffort,
    pub overview: ReasoningEffort,
    pub verification: ReasoningEffort,
    pub overview_verification: ReasoningEffort,
    pub context_synthesis: ReasoningEffort,
    pub context_verification: ReasoningEffort,
}
impl Default for Reasoning {
    fn default() -> Self {
        Self {
            enabled: true,
            default: ReasoningEffort::Medium,
            extraction: ReasoningEffort::Low,
            reconciliation: ReasoningEffort::High,
            synthesis: ReasoningEffort::Medium,
            overview: ReasoningEffort::Medium,
            verification: ReasoningEffort::High,
            overview_verification: ReasoningEffort::High,
            context_synthesis: ReasoningEffort::Medium,
            context_verification: ReasoningEffort::High,
        }
    }
}
impl Reasoning {
    /// Map Lore's stable inference tasks to the configured Responses effort.
    /// Never silently escalate verification to a different model or API.
    pub fn for_task(&self, task: &str) -> Option<ReasoningEffort> {
        if !self.enabled {
            return None;
        }
        Some(match task {
            "extract" => self.extraction,
            "reconcile" | "cross_source_reconcile" => self.reconciliation,
            "synthesize" => self.synthesis,
            "overview" => self.overview,
            "verify" => self.verification,
            "verify_overview" => self.overview_verification,
            "context_synthesis" => self.context_synthesis,
            "context_verification" => self.context_verification,
            _ => self.default,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Models {
    pub generative: ModelRole,
    pub decision: Option<ModelRole>,
    /// Optional semantic search. No embedding model is contacted unless enabled.
    pub embedding: Option<ModelRole>,
    /// Sent to OpenAI Responses (not OpenAI Decisions or Ollama).
    pub reasoning: Reasoning,
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
    /// Separate permission for sending fresh checkout content to a hosted
    /// model. Permission to process retained knowledge does not imply this.
    pub allow_checkout_egress: bool,
}
impl Default for Privacy {
    fn default() -> Self {
        Self {
            local_only: true,
            allow_checkout_egress: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContextSettings {
    /// Cache guidance and embeddings outside the source knowledge registry.
    /// `context --no-cache` overrides this for one invocation.
    pub cache: bool,
    pub inspection: crate::context::inspection::InspectionSettings,
    pub investigation: crate::context::investigation::InvestigationSettings,
}
impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            cache: true,
            inspection: crate::context::inspection::InspectionSettings::default(),
            investigation: crate::context::investigation::InvestigationSettings::default(),
        }
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
    pub imports: Vec<(String, PathBuf)>,
    pub project_id: String,
    pub fingerprint: String,
}
impl ResolvedConfig {
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_impl(path, true)
    }
    /// Read stored knowledge without requiring a usable inference setup. Source
    /// paths may be offline; the normal schema and filesystem safety checks apply.
    pub fn load_for_read(path: &Path) -> Result<Self> {
        Self::load_impl(path, false)
    }
    fn load_impl(path: &Path, validate_inference: bool) -> Result<Self> {
        ensure!(
            !std::fs::symlink_metadata(path)?.file_type().is_symlink(),
            "configuration must not be a symlink"
        );
        let absolute = path.canonicalize().context("locate lore.yml")?;
        let config: Config = serde_yaml::from_str(&util::read_limited(&absolute, 128_000)?)
            .context("invalid lore.yml")?;
        Self::resolve_impl(config, &absolute, validate_inference)
    }
    pub fn resolve(config: Config, config_path: &Path) -> Result<Self> {
        Self::resolve_impl(config, config_path, true)
    }
    fn resolve_impl(config: Config, config_path: &Path, validate_inference: bool) -> Result<Self> {
        let base = config_path
            .parent()
            .context("configuration has no parent")?
            .canonicalize()?;
        Self::resolve_at(config, config_path, base, validate_inference)
    }
    /// First contact has already selected and read this normalized path through
    /// no-follow descriptors. Keep that parent identity instead of resolving a
    /// new canonical target between the read and source discovery.
    pub(crate) fn resolve_read_snapshot(config: Config, config_path: &Path) -> Result<Self> {
        ensure!(
            config_path.is_absolute(),
            "configuration path must be absolute"
        );
        let base = config_path
            .parent()
            .context("configuration has no parent")?
            .to_owned();
        Self::resolve_at(config, config_path, base, false)
    }
    fn resolve_at(
        config: Config,
        config_path: &Path,
        base: PathBuf,
        validate_inference: bool,
    ) -> Result<Self> {
        ensure!(
            (1..=2).contains(&config.schema_version),
            "unsupported configuration schema_version"
        );
        ensure!(
            config.schema_version >= 2 || config.imports.is_empty(),
            "native imports require configuration schema_version: 2"
        );
        ensure!(
            !config.project.name.trim().is_empty()
                && config.project.name.len() <= 200
                && !config.project.name.chars().any(char::is_control),
            "project.name cannot be empty"
        );
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
            !config.sources.roots.is_empty() || !config.imports.is_empty(),
            "configure at least one Markdown source root or native import"
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
            if let Some(origin) = &root.origin {
                ensure!(
                    !origin.trim().is_empty()
                        && origin.len() <= 2048
                        && !origin.chars().any(char::is_control),
                    "source origin must be a nonempty label of at most 2048 bytes without control characters"
                );
            }
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
        let mut imports = Vec::new();
        for source in &config.imports {
            ensure!(
                !source.id.is_empty()
                    && source.id.len() <= 80
                    && source
                        .id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'),
                "invalid import ID"
            );
            ensure!(ids.insert(source.id.clone()), "duplicate source/import ID");
            ensure!(
                source.project.as_ref().is_none_or(|p| !p.trim().is_empty()
                    && p.len() <= 200
                    && !p.chars().any(char::is_control)),
                "import project filter must contain 1..200 bytes without control characters"
            );
            ensure!(
                source.kind == ImportKind::Engram || source.project.is_none(),
                "project filter is supported only for Engram imports"
            );
            ensure!(
                source.kind == ImportKind::Beads || !source.include_memories,
                "include_memories is supported only for Beads imports"
            );
            let path = util::absolute(&base, &source.path)?;
            ensure!(
                !path.starts_with(&wiki)
                    && !path.starts_with(&state)
                    && !wiki.starts_with(&path)
                    && !state.starts_with(&path),
                "an import must not overlap generated output"
            );
            ensure!(
                !roots
                    .iter()
                    .chain(imports.iter())
                    .any(
                        |(_, previous): &(String, PathBuf)| path.starts_with(previous)
                            || previous.starts_with(&path)
                    ),
                "source roots and native imports overlap; each input must have one owner"
            );
            imports.push((source.id.clone(), path));
        }
        if validate_inference {
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
                .chain(config.models.embedding.iter().filter(|r| r.enabled))
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
            if let Some(role) = config.models.embedding.as_ref().filter(|r| r.enabled) {
                ensure!(
                    ["ollama", "openai"].contains(&role.provider.as_str()),
                    "embedding requires the ollama or openai provider"
                );
            }
        }
        let fingerprint = util::json_digest(&(
            env!("CARGO_PKG_VERSION"),
            "pipeline-v12-native-observations",
            &config,
        ))?;
        let project_id = format!("project_{}", &util::digest(&config.project.name)[7..31]);
        Ok(Self {
            config,
            config_path: config_path.to_owned(),
            base,
            wiki,
            state,
            roots,
            imports,
            project_id,
            fingerprint,
        })
    }
}
