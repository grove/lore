use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use lore::{
    config::{Config, ResolvedConfig, SourceRoot},
    context::{self, ContextOptions},
    engine::{self, UpdateOptions},
    http::HttpModel,
    inference::{
        DecisionModel, DecisionQuestion, DecisionRequest, EgressPolicy, EmbeddingModel,
        EmbeddingRequest, GenerationRequest, GenerativeModel, QuestionKind,
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
    about = "Understand project knowledge and get evidence-linked guidance for a task"
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
    /// Get actionable, source-cited guidance using the configured model.
    Context {
        #[arg(value_name = "TASK", value_parser = nonempty_task)]
        task: String,
        #[arg(
            long = "path",
            value_name = "PATH",
            help = "A relevant path hint; may be repeated"
        )]
        paths: Vec<String>,
        #[arg(long, default_value_t = context::DEFAULT_MAX_TOKENS, value_name = "N", value_parser = context_budget,
            help = "Maximum complete-output token count using the offline cl100k_base tokenizer")]
        max_tokens: usize,
        #[arg(
            long,
            help = "Deterministic, read-only context with zero model calls (schema 2)"
        )]
        fast: bool,
        #[arg(
            long,
            conflicts_with = "fast",
            help = "Bypass guidance and embedding caches for this query"
        )]
        no_cache: bool,
        #[arg(long, conflicts_with_all = ["fast", "no_inspect"],
            help = "Inspect relevant checkout files without executing code (opt-in)")]
        inspect: bool,
        #[arg(long, conflicts_with_all = ["fast", "no_inspect"],
            help = "Investigate consequential uncertainty with bounded read-only inspections")]
        investigate: bool,
        #[arg(
            long,
            conflicts_with = "fast",
            help = "Use retained knowledge only, overriding configured checkout inspection"
        )]
        no_inspect: bool,
        #[arg(long, conflicts_with_all = ["fast", "no_inspect"],
            help = "Explicitly permit checkout content in the configured hosted model for this query")]
        allow_checkout_egress: bool,
        #[arg(long, conflicts_with = "fast", value_parser = clap::value_parser!(u32).range(3..=5),
            help = "JSON contract: 4 (default), 3 for compatibility, or 5 for adaptive shared intelligence")]
        schema_version: Option<u32>,
    },
    /// Read a generated topic by slug, or index.
    Read { topic: String },
    /// Inspect an immutable, verbatim source evidence snapshot.
    Evidence { id: String },
    /// List reusable investigation leads, or clear disposable decision findings.
    Memory {
        #[arg(
            long,
            help = "Delete derived decision findings without changing source knowledge"
        )]
        clear: bool,
    },
    /// Understand a project, explore a workflow, or learn through optional practice.
    Onboard {
        #[arg(value_name = "GOAL", value_parser = human_goal)]
        goal: Option<String>,
        #[arg(long, conflicts_with = "goal", value_parser = human_goal, help = "Begin with a project concept or workflow")]
        topic: Option<String>,
        #[arg(long, value_parser = human_goal, help = "Go directly to a real contribution task")]
        task: Option<String>,
        #[arg(long, default_value = "auto", value_parser = experience_mode)]
        mode: lore::experience::ExperienceMode,
        #[arg(long = "path", value_name = "PATH")]
        paths: Vec<String>,
        #[arg(long, default_value_t = lore::experience::DEFAULT_MAX_TOKENS, value_parser = human_budget)]
        max_tokens: usize,
        #[arg(long = "hint", default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=3))]
        hint_level: u8,
        #[arg(long)]
        show_solution: bool,
        #[arg(long, value_parser = learning_answer, help = "Optional answer for source-grounded learning feedback; never persisted")]
        answer: Option<String>,
        #[arg(long, value_parser = lesson_revision, help = "Pin feedback and hints to a returned lesson revision")]
        lesson: Option<String>,
        #[arg(long, default_value = "primary", value_parser = learning_stage)]
        activity: lore::experience::LearningStage,
        #[arg(long)]
        no_cache: bool,
        #[arg(
            long,
            conflicts_with = "no_inspect",
            help = "Grant read-only inspection within this configuration directory"
        )]
        inspect: bool,
        #[arg(long)]
        no_inspect: bool,
        #[arg(long)]
        allow_checkout_egress: bool,
    },
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
    let args: Vec<_> = std::env::args_os().collect();
    let json_requested = args
        .iter()
        .skip(1)
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "--json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            // clap normally exits before run(), which otherwise makes argument
            // errors the one failure agents cannot parse with --json.
            if json_requested && error.use_stderr() {
                println!(
                    "{}",
                    json!({"error":error.to_string(),"code":"invalid_arguments"})
                );
                std::process::exit(2);
            }
            error.exit();
        }
    };
    let json = cli.json;
    let result = tokio::select! {result=run(cli)=>result,_=tokio::signal::ctrl_c()=>Err(anyhow::anyhow!("cancelled; run update to recover any pending publication"))};
    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            let context_error = e.downcast_ref::<context::ContextError>();
            let code = context_error.map_or("operation_failed", |error| error.code);
            if json {
                println!("{}", json!({"error":format!("{e:#}"),"code":code}));
            } else {
                eprintln!("Lore: {e:#}");
            }
            std::process::exit(if matches!(code, "invalid_query" | "invalid_budget") {
                2
            } else {
                1
            });
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
    let config = if matches!(
        &cli.command,
        Command::Context { .. }
            | Command::Evidence { .. }
            | Command::Memory { .. }
            | Command::Onboard { .. }
    ) {
        ResolvedConfig::load_for_read(&cli.config)
    } else {
        ResolvedConfig::load(&cli.config)
    }
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
                let response=gen_model.generate(&GenerationRequest{instructions:"Return the JSON object {\"ok\":true}. This is a synthetic connectivity check.".into(),input:"Synthetic test; no project material.".into(),schema:Some(json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false})),reasoning_effort:config.config.models.reasoning.for_task("extract")}).await.map_err(|e|anyhow::anyhow!("generative inference: {e:?}"))?;
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
            if let Some(role) = config
                .config
                .models
                .embedding
                .as_ref()
                .filter(|r| r.enabled)
            {
                let model = HttpModel::new(&config, role).map_err(|e| anyhow::anyhow!("{e:?}"))?;
                results.push(
                    model
                        .doctor()
                        .await
                        .map_err(|e| anyhow::anyhow!("embedding preflight: {e:?}"))?,
                );
                if inference {
                    let response = model
                        .embed(&EmbeddingRequest {
                            inputs: vec![
                                "Synthetic connectivity test; no project material.".into(),
                            ],
                        })
                        .await
                        .map_err(|e| anyhow::anyhow!("embedding inference: {e:?}"))?;
                    results.push(json!({
                        "embedding_inference": true,
                        "model": response.model,
                        "dimensions": response.embeddings.first().map(Vec::len),
                    }));
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
        Command::Context {
            task,
            paths,
            max_tokens,
            fast,
            no_cache,
            inspect,
            investigate,
            no_inspect,
            allow_checkout_egress,
            schema_version,
        } => {
            let conn = storage::read_only(&config.state.join("state.db"))
                .context("open knowledge registry (run lore init or lore update first)")?;
            let options = ContextOptions {
                task,
                paths,
                max_tokens,
            };
            // Validate the complete fast output budget before any model call
            // or disposable cache write, including when intelligence is used.
            let fast_result = context::build_context(&conn, &options)?;
            if fast {
                if cli.json {
                    println!("{}", serde_json::to_string(&fast_result)?);
                } else {
                    print!("{}", context::render_context(&fast_result));
                }
            } else if schema_version == Some(5) {
                let result = context::adaptive::run(
                    &config,
                    &conn,
                    &options,
                    &context::decision::runtime::RunOptions {
                        inspect,
                        investigate,
                        no_inspect,
                        no_cache,
                        allow_checkout_egress,
                    },
                )
                .await?;
                if cli.json {
                    println!("{}", serde_json::to_string(&result)?);
                } else {
                    print!("{}", context::adaptive::render(&result));
                }
            } else if schema_version == Some(3) {
                ensure!(
                    !inspect && !investigate && !allow_checkout_egress,
                    "checkout inspection and investigation require schema 4"
                );
                let result = intelligent_context(&config, &conn, &options, no_cache).await?;
                if cli.json {
                    println!("{}", serde_json::to_string(&result)?);
                } else {
                    print!(
                        "{}",
                        context::intelligence::render_intelligent_context(&result)
                    );
                }
            } else {
                let result = context::decision::runtime::run(
                    &config,
                    &conn,
                    &options,
                    &context::decision::runtime::RunOptions {
                        inspect,
                        investigate,
                        no_inspect,
                        no_cache,
                        allow_checkout_egress,
                    },
                )
                .await?;
                if cli.json {
                    println!("{}", serde_json::to_string(&result)?);
                } else {
                    print!("{}", context::decision::runtime::render(&result));
                }
            }
        }
        Command::Onboard {
            goal,
            topic,
            task,
            mode,
            paths,
            max_tokens,
            hint_level,
            show_solution,
            answer,
            lesson,
            activity,
            no_cache,
            inspect,
            no_inspect,
            allow_checkout_egress,
        } => {
            let conn = storage::read_only(&config.state.join("state.db"))
                .context("open project knowledge (run lore init or lore update first)")?;
            let result = lore::experience::run(
                &config,
                &conn,
                &lore::experience::ExperienceOptions {
                    goal: goal.or(topic),
                    task,
                    mode,
                    paths,
                    max_tokens,
                    hint_level,
                    show_solution,
                    answer,
                    lesson,
                    activity,
                    run: context::decision::runtime::RunOptions {
                        inspect,
                        investigate: false,
                        no_inspect,
                        no_cache,
                        allow_checkout_egress,
                    },
                },
            )
            .await?;
            if cli.json {
                println!("{}", serde_json::to_string(&result)?);
            } else {
                print!("{}", lore::experience::render_markdown(&result));
            }
        }
        Command::Memory { clear } => {
            if clear {
                let removed = context::memory::clear(&config)?;
                println!(
                    "{}",
                    json!({"schema_version":1,"removed":removed,"sources_modified":false})
                );
            } else {
                let conn = storage::read_only(&config.state.join("state.db"))?;
                let (effective, _, _) = context::adaptive::authorized_config(
                    &config,
                    &context::decision::runtime::RunOptions::default(),
                )?;
                let result =
                    context::memory::list(&effective, &storage::registry_revision(&conn)?)?;
                if cli.json {
                    println!("{}", serde_json::to_string(&result)?);
                } else {
                    println!("# Reusable investigation leads\n");
                    for item in &result.findings {
                        println!(
                            "- {} — {} inspected files, {} investigation steps. Revalidation required.\n  `{}`",
                            util::markdown_text(&item.question),
                            item.inspected_files,
                            item.investigation_steps,
                            item.id
                        );
                    }
                    if result.findings.is_empty() {
                        println!("No current findings are available under this caller's grants.");
                    }
                }
            }
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
                material: Default::default(),
                origin: None,
            })
            .collect();
    } else if !parent.join("docs").is_dir() {
        config.sources.roots = vec![SourceRoot {
            id: "project".into(),
            path: ".".into(),
            material: Default::default(),
            origin: None,
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
async fn intelligent_context(
    config: &ResolvedConfig,
    conn: &rusqlite::Connection,
    options: &ContextOptions,
    no_cache: bool,
) -> Result<context::intelligence::IntelligentContextResult> {
    let mut config = config.clone();
    config.config.context.cache &= !no_cache;
    // Construction enforces provider and egress configuration without a
    // network request. Read-only commands still work with retired providers.
    let model = HttpModel::new(&config, &config.config.models.generative).ok();
    conn.execute_batch("SAVEPOINT lore_intelligent_context")?;
    let result = async {
        if model.is_some() && config.config.processing.max_context_bytes < 14_048 {
            let selected = context::build_context(conn, options)?;
            return context::intelligence::fast_fallback(
                conn, options, &selected, 0,
                "The configured inference input budget is too small for task guidance; using deterministic context without fresh reasoning.",
            );
        }
        let mut retrieval_warnings = Vec::new();
        let mut retrieval_calls = 0;
        let mut semantic = None;
        if model.is_some() {
            if let Some(role) = config.config.models.embedding.as_ref().filter(|r| r.enabled) {
                match HttpModel::new(&config, role) {
                    Ok(embedding) => {
                        let cache = if config.config.context.cache {
                            context::semantic::open_cache(&config.state.join("semantic.sqlite3"))
                                .or_else(|_| {
                                    retrieval_warnings.push(
                                        "Semantic cache unavailable; this query uses a temporary index.".into(),
                                    );
                                    Ok::<_, anyhow::Error>(rusqlite::Connection::open_in_memory()?)
                                })?
                        } else {
                            rusqlite::Connection::open_in_memory()?
                        };
                        let policy = if config.config.privacy.local_only {
                            EgressPolicy::LocalOnly
                        } else {
                            EgressPolicy::ExplicitHosted
                        };
                        let search = context::semantic::search(
                            conn, &cache, &embedding, policy, &options.task, &options.paths,
                            config.config.processing.max_context_bytes,
                        );
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(config.config.processing.timeout_seconds),
                            search,
                        ).await {
                            Ok(Ok(report)) => {
                                retrieval_calls = report.model_calls;
                                retrieval_warnings.extend(report.warnings.iter().cloned());
                                semantic = Some(report);
                            }
                            outcome => {
                                retrieval_calls = u32::try_from(embedding.embedding_calls.load(
                                    std::sync::atomic::Ordering::Relaxed,
                                )).unwrap_or(u32::MAX);
                                retrieval_warnings.push(if outcome.is_err() {
                                    "Semantic indexing timed out; valid cached progress is reusable. Using lexical and recorded relationship retrieval."
                                } else {
                                    "Semantic retrieval unavailable; using lexical and recorded relationship retrieval."
                                }.into());
                            }
                        }
                    }
                    Err(_) => retrieval_warnings.push(
                        "Embedding configuration unavailable or disallowed; using lexical and recorded relationship retrieval.".into(),
                    ),
                }
            }
        }
        // Input evidence has its own bound. The user's --max-tokens is the
        // complete output budget, and should primarily accommodate guidance.
        let input_bytes = config.config.processing.max_context_bytes.saturating_sub(12_000);
        let mut input_options = options.clone();
        if model.is_some() {
            input_options.max_tokens = (input_bytes / 4)
                .clamp(context::MIN_MAX_TOKENS, context::MAX_MAX_TOKENS.min(16_000));
        }
        let mut selected = loop {
            let candidate = match &semantic {
                Some(report) => context::build_context_with_semantic(conn, &input_options, report),
                None => context::build_context(conn, &input_options),
            };
            let candidate = match candidate {
                Ok(candidate) => candidate,
                Err(error) if error.downcast_ref::<context::ContextError>()
                    .is_some_and(|error| error.code == "invalid_budget") => {
                    let mut selected = context::build_context(conn, options)?;
                    selected.warnings.extend(retrieval_warnings);
                    return context::intelligence::fast_fallback(
                        conn, options, &selected, retrieval_calls,
                        "The task and retained evidence do not fit the inference input budget; using deterministic context without fresh reasoning.",
                    );
                }
                Err(error) => return Err(error),
            };
            if model.is_none() || serde_json::to_vec(&candidate)?.len() <= input_bytes
                || input_options.max_tokens <= 512
            {
                break candidate;
            }
            input_options.max_tokens = (input_options.max_tokens * 3 / 4).max(512);
        };
        selected.model_calls = retrieval_calls;
        selected.warnings.extend(retrieval_warnings);
        context::intelligence::build_intelligent_context(
            conn, &config, options, selected,
            model.as_ref().map(|model| model as &dyn GenerativeModel),
        ).await
    }.await;
    let released = conn.execute_batch("RELEASE lore_intelligent_context");
    match result {
        Ok(value) => {
            released?;
            Ok(value)
        }
        Err(error) => {
            let _ = released;
            Err(error)
        }
    }
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
                "Up to date. No model calls; {} source files, {} imported records, and {} knowledge units.",
                report.source_files, report.imported_records, report.knowledge_units
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
            if report.imported_records > 0 || report.retired_imported_records > 0 {
                println!(
                    "Native records: {} current; {} changed, {} withdrawn. Upstream source authority and evidence are preserved.",
                    report.imported_records,
                    report.changed_imported_records,
                    report.retired_imported_records
                );
            }
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

fn experience_mode(value: &str) -> std::result::Result<lore::experience::ExperienceMode, String> {
    use lore::experience::ExperienceMode;
    match value {
        "auto" => Ok(ExperienceMode::Auto),
        "explanation" => Ok(ExperienceMode::Explanation),
        "how-to" => Ok(ExperienceMode::HowTo),
        "tutorial" => Ok(ExperienceMode::Tutorial),
        "reference" => Ok(ExperienceMode::Reference),
        _ => Err("expected auto, explanation, how-to, tutorial, or reference".into()),
    }
}

fn learning_stage(value: &str) -> std::result::Result<lore::experience::LearningStage, String> {
    match value {
        "primary" => Ok(lore::experience::LearningStage::Primary),
        "transfer" => Ok(lore::experience::LearningStage::Transfer),
        _ => Err("expected primary or transfer".into()),
    }
}

fn human_goal(value: &str) -> std::result::Result<String, String> {
    if value.trim().is_empty() || value.len() > 4_000 || value.chars().any(char::is_control) {
        Err("goal and task need 1..4000 UTF-8 bytes without control characters".into())
    } else {
        Ok(value.into())
    }
}

fn learning_answer(value: &str) -> std::result::Result<String, String> {
    if value.trim().is_empty()
        || value.len() > 8_000
        || value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        Err("a learning answer needs 1..8000 UTF-8 bytes without unsafe control characters".into())
    } else {
        Ok(value.into())
    }
}

fn lesson_revision(value: &str) -> std::result::Result<String, String> {
    if value
        .strip_prefix("blake3:")
        .is_some_and(|id| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        Ok(value.into())
    } else {
        Err("--lesson must be a returned lesson revision key".into())
    }
}

fn human_budget(value: &str) -> std::result::Result<usize, String> {
    let budget = context_budget(value)?;
    if budget < 512 {
        Err("onboard --max-tokens must be 512..100000".into())
    } else {
        Ok(budget)
    }
}

fn nonempty_task(task: &str) -> std::result::Result<String, String> {
    if task.trim().is_empty() {
        Err("TASK must contain a non-whitespace task description".into())
    } else {
        Ok(task.into())
    }
}

fn context_budget(value: &str) -> std::result::Result<usize, String> {
    let budget = value
        .parse::<usize>()
        .map_err(|_| "--max-tokens must be a positive integer".to_owned())?;
    if !(context::MIN_MAX_TOKENS..=context::MAX_MAX_TOKENS).contains(&budget) {
        Err(format!(
            "--max-tokens must be between {} and {}",
            context::MIN_MAX_TOKENS,
            context::MAX_MAX_TOKENS
        ))
    } else {
        Ok(budget)
    }
}
