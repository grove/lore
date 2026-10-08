use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use lore::{
    config::{Config, ResolvedConfig, SourceRoot},
    engine::{self, UpdateOptions},
    http::HttpModel,
    inference::{
        DecisionModel, DecisionQuestion, DecisionRequest, GenerationRequest, GenerativeModel,
        QuestionKind,
    },
    publish, storage, util,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    name = "lore",
    version,
    about = "Compile project Markdown into an evidence-backed, incrementally maintained wiki"
)]
struct Cli {
    #[arg(long, global = true, default_value = "lore.yml")]
    config: PathBuf,
    #[arg(long, global = true, help = "Emit machine-readable JSON")]
    json: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Create configuration and build the initial wiki (existing config is preserved).
    Init {
        #[arg(long)]
        source: Vec<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        configure_only: bool,
        #[arg(
            long,
            help = "Explicitly replace Lore-owned generated output, discarding manual edits"
        )]
        rebuild: bool,
    },
    /// Reconcile changed source sections and refresh affected wiki topics.
    Update {
        #[arg(long)]
        dry_run: bool,
        #[arg(long, help = "Bypass semantic caches and re-extract all sources")]
        refresh: bool,
        #[arg(
            long,
            help = "Replace Lore-owned output, discarding generated-page edits"
        )]
        rebuild: bool,
    },
    /// Inspect source changes and output drift without model calls or writes.
    Status,
    /// Check evidence and integrity; --deep also re-runs semantic reconciliation.
    Audit {
        #[arg(long)]
        deep: bool,
    },
    /// Check provider configuration/model availability, using no project content.
    Doctor {
        #[arg(
            long,
            help = "Also send tiny synthetic generative and decision requests"
        )]
        inference: bool,
    },
    /// Search the maintained knowledge registry without inference.
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Read a generated topic by slug, or index.
    Read { topic: String },
    /// Inspect an immutable, verbatim source evidence snapshot.
    Evidence { id: String },
    /// Inspect and disposition review questions; never edits source knowledge.
    Review {
        #[command(subcommand)]
        action: Option<ReviewCommand>,
    },
    /// Erase all Lore-managed evidence, caches and output, never original sources.
    Purge {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        yes: bool,
    },
}
#[derive(Subcommand)]
enum ReviewCommand {
    /// List pending questions, or all retained records with --all.
    List {
        #[arg(long)]
        all: bool,
    },
    /// Show one review, its evidence binding and immutable history.
    Show { id: String },
    /// Record a human resolution without changing the knowledge graph.
    Resolve {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "user")]
        actor: String,
    },
    /// Dismiss a false-positive question with a recorded reason.
    Dismiss {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "user")]
        actor: String,
    },
    /// Reopen a retained review for investigation.
    Reopen {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "user")]
        actor: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    let result = tokio::select! {result=run(cli)=>result,_=tokio::signal::ctrl_c()=>Err(anyhow::anyhow!("cancelled; run update to recover any pending publication"))};
    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            if json {
                println!(
                    "{}",
                    json!({"error":format!("{e:#}"),"code":"operation_failed"})
                );
            } else {
                eprintln!("Lore: {e:#}");
            }
            std::process::exit(1);
        }
    }
}
async fn run(cli: Cli) -> Result<i32> {
    if let Command::Init {
        source,
        name,
        configure_only,
        ..
    } = &cli.command
    {
        bootstrap(&cli.config, source, name.as_deref())?;
        if *configure_only {
            emit(
                json!({"configured":true,"path":cli.config,"next":"Edit lore.yml, ensure the configured model is available, then run lore init."}),
                cli.json,
            )?;
            return Ok(0);
        }
    }
    let config = ResolvedConfig::load(&cli.config)
        .context("load configuration (start with lore init --configure-only)")?;
    if !matches!(
        &cli.command,
        Command::Init { .. }
            | Command::Update { dry_run: false, .. }
            | Command::Audit { deep: true }
            | Command::Status
            | Command::Doctor { .. }
            | Command::Purge { .. }
    ) {
        ensure!(
            !publish::has_pending(&config),
            "publication recovery is pending; run lore update before reading the knowledge registry"
        );
    }
    match cli.command {
        Command::Init { rebuild, .. } => {
            compile(
                &config,
                UpdateOptions {
                    rebuild,
                    ..UpdateOptions::default()
                },
                cli.json,
            )
            .await?
        }
        Command::Update {
            dry_run,
            refresh,
            rebuild,
        } => {
            if dry_run {
                emit(serde_json::to_value(engine::status(&config)?)?, cli.json)?;
            } else {
                compile(
                    &config,
                    UpdateOptions {
                        rebuild,
                        refresh,
                        deep: false,
                    },
                    cli.json,
                )
                .await?;
            }
        }
        Command::Status => emit(serde_json::to_value(engine::status(&config)?)?, cli.json)?,
        Command::Audit { deep } => {
            if deep {
                compile(
                    &config,
                    UpdateOptions {
                        refresh: true,
                        deep: true,
                        rebuild: false,
                    },
                    cli.json,
                )
                .await?;
            }
            let report = engine::audit(&config)?;
            let ok = report["ok"].as_bool() == Some(true);
            emit(report, cli.json)?;
            if !ok {
                return Ok(3);
            }
        }
        Command::Doctor { inference } => {
            let gen_model = HttpModel::new(&config, &config.config.models.generative)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let mut results = vec![
                gen_model
                    .doctor()
                    .await
                    .map_err(|e| anyhow::anyhow!("generative preflight: {e:?}"))?,
            ];
            if inference {
                let response=gen_model.generate(&GenerationRequest{instructions:"Return the JSON object {\"ok\":true}. This is a synthetic connectivity check.".into(),input:"Synthetic test; no project material.".into(),schema:Some(json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}))}).await.map_err(|e|anyhow::anyhow!("generative inference: {e:?}"))?;
                ensure!(
                    serde_json::from_str::<Value>(&response.text)?["ok"].as_bool() == Some(true),
                    "synthetic structured-output test failed"
                );
                results.push(json!({"generative_inference":true,"model":response.model}));
            }
            if let Some(role) = config.config.models.decision.as_ref().filter(|r| r.enabled) {
                let model = HttpModel::new(&config, role).map_err(|e| anyhow::anyhow!("{e:?}"))?;
                results.push(
                    model
                        .doctor()
                        .await
                        .map_err(|e| anyhow::anyhow!("decision preflight: {e:?}"))?,
                );
                if inference {
                    let request = DecisionRequest {
                        input: "This is a synthetic test.".into(),
                        questions: vec![DecisionQuestion {
                            name: "synthetic".into(),
                            instructions: "Does the text describe a synthetic test?".into(),
                            kind: QuestionKind::Predicate,
                        }],
                    };
                    let response = model
                        .decide(&request)
                        .await
                        .map_err(|e| anyhow::anyhow!("decision inference: {e:?}"))?;
                    response
                        .validate(&request)
                        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
                    results.push(json!({"decision_inference":true,"model":response.model,"answers":format!("{:?}",response.answers)}));
                }
            }
            emit(
                json!({"providers":results,"project_content_sent":false}),
                cli.json,
            )?;
        }
        Command::Search { query, limit } => {
            let conn = storage::read_only(&config.state.join("state.db"))?;
            let views = storage::views(&conn)?;
            let results=storage::search(&conn,&query,limit)?.into_iter().map(|(id,statement,topic)|{
                let view=views.iter().find(|v|v.id==id);
                json!({"id":id,"statement":statement,"topic":topic,"kind":view.map(|v|&v.kind),"lifecycle":view.map(|v|&v.lifecycle),"support_state":view.map(|v|&v.support_state)})
            }).collect::<Vec<_>>();
            emit(json!({"results":results}), cli.json)?;
        }
        Command::Read { topic } => {
            let path = if topic == "index" || topic == "reviews" {
                format!("{topic}.md")
            } else {
                util::safe_slug(&topic)?;
                format!("topics/{topic}.md")
            };
            let text = util::read_limited(&publish::page_path(&config.wiki, &path)?, 4_000_000)?;
            if cli.json {
                emit(json!({"page":path,"content":text}), true)?;
            } else {
                print!("{text}");
            }
        }
        Command::Evidence { id } => emit(engine::evidence(&config, &id)?, cli.json)?,
        Command::Review { action } => match action {
            None | Some(ReviewCommand::List { all: false }) => {
                let conn = storage::read_only(&config.state.join("state.db"))?;
                emit(
                    json!({"reviews":lore::reviews::list(&conn,false)?}),
                    cli.json,
                )?;
            }
            Some(ReviewCommand::List { all: true }) => {
                let conn = storage::read_only(&config.state.join("state.db"))?;
                emit(
                    json!({"reviews":lore::reviews::list(&conn,true)?}),
                    cli.json,
                )?;
            }
            Some(ReviewCommand::Show { id }) => {
                let conn = storage::read_only(&config.state.join("state.db"))?;
                emit(lore::reviews::show(&conn, &id)?, cli.json)?;
            }
            Some(ReviewCommand::Resolve { id, reason, actor }) => emit(
                engine::change_review(&config, &id, "resolved", &reason, &actor)?,
                cli.json,
            )?,
            Some(ReviewCommand::Dismiss { id, reason, actor }) => emit(
                engine::change_review(&config, &id, "dismissed", &reason, &actor)?,
                cli.json,
            )?,
            Some(ReviewCommand::Reopen { id, reason, actor }) => emit(
                engine::change_review(&config, &id, "pending", &reason, &actor)?,
                cli.json,
            )?,
        },
        Command::Purge { all, yes } => {
            ensure!(
                all && yes,
                "purge requires --all --yes; this erases all retained evidence, caches and generated wiki files"
            );
            publish::purge_all(&config)?;
            emit(
                json!({"purged":true,"sources_modified":false,"warning":"External backups and physical secure erasure are outside Lore's control."}),
                cli.json,
            )?;
        }
    }
    Ok(0)
}
fn bootstrap(path: &Path, sources: &[PathBuf], name: Option<&str>) -> Result<()> {
    if path.exists() {
        ensure!(
            sources.is_empty() && name.is_none(),
            "configuration already exists; edit it rather than supplying new init sources/name"
        );
        return Ok(());
    }
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = absolute.parent().context("configuration has no parent")?;
    util::reject_symlinks(parent)?;
    fs::create_dir_all(parent)?;
    let mut config = Config::default();
    config.project.name = name.map(str::to_owned).unwrap_or_else(|| {
        parent
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("my-project")
            .to_owned()
    });
    if !sources.is_empty() {
        config.sources.roots = sources
            .iter()
            .enumerate()
            .map(|(i, p)| SourceRoot {
                id: format!("source-{}", i + 1),
                path: p.clone(),
            })
            .collect();
    } else if !parent.join("docs").is_dir() {
        config.sources.roots = vec![SourceRoot {
            id: "project".into(),
            path: ".".into(),
        }];
    }
    let bytes = serde_yaml::to_string(&config)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&absolute)?;
    file.write_all(bytes.as_bytes())?;
    file.sync_all()?;
    Ok(())
}
async fn compile(config: &ResolvedConfig, options: UpdateOptions, json_output: bool) -> Result<()> {
    // Constructing adapters performs no network request and reads no API key.
    // No-op updates therefore work without running inference servers or secrets.
    let model = HttpModel::new(config, &config.config.models.generative)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let decision = config
        .config
        .models
        .decision
        .as_ref()
        .filter(|r| r.enabled)
        .map(|role| HttpModel::new(config, role))
        .transpose()
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let report = engine::update(
        config,
        &model,
        decision.as_ref().map(|m| m as &dyn DecisionModel),
        options,
    )
    .await?;
    if json_output {
        emit(serde_json::to_value(report)?, true)?;
    } else {
        if report.no_op {
            println!(
                "Up to date. No model calls; {} source files and {} knowledge units.",
                report.source_files, report.knowledge_units
            );
        } else {
            println!(
                "Published {} changed pages from {} processed sections. {} knowledge units; {} generative calls, {} cache hits, {} decision calls.",
                report.changed_pages,
                report.processed_sections,
                report.knowledge_units,
                report.model_calls,
                report.cache_hits,
                report.decision_calls
            );
        }
        if report.pending_reviews > 0 {
            println!(
                "{} review items remain; inspect them with lore review.",
                report.pending_reviews
            );
        }
        for warning in report.warnings {
            eprintln!("Warning: {warning}");
        }
    }
    Ok(())
}
fn emit(value: Value, _json: bool) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
