//! Actual binary contracts for source-grounded human experiences. Ingestion
//! uses the existing deterministic model; CLI inference is denied before a
//! network client can be constructed. These are not human learning outcomes.

mod common;

use common::*;
use lore::{
    config::ResolvedConfig,
    context::count_tokens,
    engine::{self, UpdateOptions},
    experience::{self, ExperienceResult},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const CONDITION: &str = "Production dispatch preserves tenant ordering with a queue capacity of 128 jobs; staging uses 64 jobs. A full queue returns Busy without accepting a job.";
const PRIVATE_CODE: &str = "const CHECKOUT_ONLY_HUMAN_CLI_SENTINEL: usize = 128;\n";

async fn fixture() -> (tempfile::TempDir, ResolvedConfig) {
    let (temp, mut config, model) = project();
    put(
        &config,
        "dispatch.md",
        "DECISION dispatch: Harbor separates dispatch from worker execution so that a slow tenant cannot exhaust every worker.\n",
    );
    put(
        &config,
        "capacity.md",
        &format!("DECISION dispatch: {CONDITION}\n"),
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    fs::create_dir(config.base.join("src")).unwrap();
    fs::write(config.base.join("src/dispatch.rs"), PRIVATE_CODE).unwrap();

    // A read-only experience must remain usable with unavailable/forbidden
    // inference configuration. OpenAI under local_only is denied before HTTP
    // client construction; no loopback or real provider request is needed.
    config.config.models.generative.provider = "openai".into();
    config.config.models.generative.model = "denied-human-cli-fixture".into();
    config.config.models.decision = None;
    config.config.models.embedding = None;
    config.config.privacy.local_only = true;
    config.config.context.cache = true;
    config.config.context.inspection.enabled = true;
    config.config.context.inspection.root = Some(".".into());
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&config.config).unwrap(),
    )
    .unwrap();
    let config = ResolvedConfig::load_for_read(&config.config_path).unwrap();
    (temp, config)
}

fn command(config: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lore"));
    command
        .arg("--config")
        .arg(config)
        .env_remove("OPENAI_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("LORE_INSPECTION_ROOT")
        .env_remove("LORE_ALLOW_HOSTED_EGRESS")
        .env_remove("LORE_ALLOW_CHECKOUT_EGRESS");
    command
}

fn invoke(config: &ResolvedConfig, json: bool, extra: &[&str]) -> Output {
    let mut command = command(&config.config_path);
    if json {
        command.arg("--json");
    }
    command
        .args(["onboard", "--no-inspect", "--no-cache"])
        .args(extra)
        .output()
        .unwrap()
}

fn successful(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn json(config: &ResolvedConfig, extra: &[&str]) -> (Value, String) {
    let text = successful(invoke(config, true, extra));
    (serde_json::from_str(&text).unwrap(), text)
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "test fixture unexpectedly has a symlink"
            );
            if kind.is_dir() {
                visit(root, &entry.path(), result);
            } else {
                result.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

#[tokio::test]
async fn onboard_cli_immediately_returns_useful_exact_evidence_without_inspection_or_inference() {
    let (_temp, config) = fixture().await;
    let before = files(&config.base);
    let (result, text) = json(&config, &[]);
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["mode"], "explanation");
    assert_eq!(result["generation_basis"], "deterministic_fallback");
    assert_eq!(result["presentation_status"], "source_fallback");
    assert_eq!(result["model_calls"], 0);
    assert_eq!(result["presentation_model_calls"], 0);
    assert_eq!(result["capabilities"]["inspection"], "disabled_by_caller");
    assert_eq!(result["capabilities"]["hosted_egress"], false);
    assert_eq!(result["capabilities"]["execution"], false);
    assert_eq!(result["intelligence"]["schema_version"], 4);
    assert!(
        result["intelligence"]["inspection"]["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        result["orientation"]["concepts"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        result["orientation"]["constraints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|condition| condition["text"] == CONDITION)
    );
    assert!(result["tutorial"].is_null());
    assert!(!text.contains("CHECKOUT_ONLY_HUMAN_CLI_SENTINEL"));

    let shared_output = command(&config.config_path)
        .args([
            "--json",
            "context",
            "Understand dispatch",
            "--schema-version",
            "5",
            "--max-tokens",
            "6000",
            "--no-inspect",
            "--no-cache",
        ])
        .output()
        .unwrap();
    let shared: Value = serde_json::from_str(&successful(shared_output)).unwrap();
    assert_eq!(result["snapshot"], shared["snapshot"]);
    assert_eq!(files(&config.base), before);
}

#[tokio::test]
async fn onboard_cli_json_and_markdown_fit_the_complete_budget_and_preserve_sources_wiki_and_state()
{
    let (_temp, config) = fixture().await;
    let before = files(&config.base);
    let (result, raw) = json(
        &config,
        &["--topic", "Explain dispatch", "--max-tokens", "6000"],
    );
    let parsed: ExperienceResult = serde_json::from_value(result.clone()).unwrap();
    let markdown = successful(invoke(
        &config,
        false,
        &["--topic", "Explain dispatch", "--max-tokens", "6000"],
    ));
    assert_eq!(markdown, experience::render_markdown(&parsed));
    let used = result["budget"]["used_tokens"].as_u64().unwrap() as usize;
    assert_eq!(result["budget"]["max_tokens"], 6000);
    assert!(
        count_tokens(&raw) <= used && count_tokens(&markdown) <= used,
        "reported {used}, JSON {}, Markdown {}",
        count_tokens(&raw),
        count_tokens(&markdown)
    );
    assert!(used <= 6000);
    assert!(markdown.contains(CONDITION));
    assert!(markdown.contains("lore evidence ev_"));
    assert_eq!(files(&config.base), before);
}

#[tokio::test]
async fn supplied_task_bypasses_tutorial_and_retains_actual_intent() {
    let (_temp, config) = fixture().await;
    let before = files(&config.base);
    let task = "Refactor dispatch without changing tenant ordering";
    let (result, _) = json(&config, &["--task", task]);
    assert_eq!(result["mode"], "how_to");
    assert_eq!(result["first_task"]["task"], task);
    assert_eq!(result["first_task"]["provenance"], "user_supplied_task");
    assert_eq!(result["intelligence"]["task"], task);
    assert!(result["tutorial"].is_null());
    assert!(
        result["first_task"]["qualification"]
            .as_str()
            .unwrap()
            .contains("not checks Lore executed")
    );
    assert_eq!(files(&config.base), before);
}

#[tokio::test]
async fn direct_reference_cli_preserves_verbatim_passages_and_live_evidence_drilldown() {
    let (_temp, config) = fixture().await;
    let before = files(&config.base);
    let (result, _) = json(
        &config,
        &[
            "--topic",
            "Exact dispatch queue capacity",
            "--mode",
            "reference",
        ],
    );
    assert_eq!(result["mode"], "reference");
    assert_eq!(result["presentation_status"], "exact_reference");
    assert_eq!(result["model_calls"], 0);
    assert!(result["orientation"]["purpose"].is_null());
    let references = result["references"].as_array().unwrap();
    assert!(!references.is_empty());
    assert!(
        references[0]["excerpt"]
            .as_str()
            .unwrap()
            .contains(CONDITION)
    );
    for reference in references {
        let output = command(&config.config_path)
            .args([
                "--json",
                "evidence",
                reference["evidence_id"].as_str().unwrap(),
            ])
            .output()
            .unwrap();
        let evidence: Value = serde_json::from_str(&successful(output)).unwrap();
        assert_eq!(reference["excerpt"], evidence["excerpt"]);
        assert_eq!(reference["content_hash"], evidence["digest"]);
        assert_eq!(reference["revision_id"], evidence["source_revision_id"]);
    }
    assert_eq!(files(&config.base), before);
}

#[tokio::test]
async fn optional_answer_has_ungraded_fallback_without_a_stored_profile_or_answer() {
    let (_temp, config) = fixture().await;
    let before = files(&config.base);
    let (result, raw) = json(
        &config,
        &[
            "--topic",
            "Teach me dispatch queues",
            "--mode",
            "tutorial",
            "--answer",
            "PRIVATE_LEARNER_ANSWER_CLI_9371: production capacity is 128 jobs",
        ],
    );
    assert_eq!(result["feedback"]["basis"], "source_comparison");
    assert_eq!(result["feedback"]["outcome"], "review_needed");
    assert_eq!(result["model_calls"], 0);
    assert!(!raw.contains("PRIVATE_LEARNER_ANSWER_CLI_9371"));
    assert!(result["tutorial"]["primary"]["solution"].is_null());
    assert!(result["tutorial"]["transfer"]["solution"].is_null());
    assert_ne!(
        result["tutorial"]["primary"]["task"],
        result["tutorial"]["transfer"]["task"]
    );
    assert_eq!(files(&config.base), before);
}

#[tokio::test]
async fn initialized_onboard_flushes_usage_on_native_stacks_and_linux_one_mib_stack() {
    let (_temp, config) = fixture().await;
    let before = files(&config.base);
    let ledger_directory = tempfile::tempdir().unwrap();
    let ledger_base = ledger_directory.path().canonicalize().unwrap();
    let cases: &[(&str, bool, &[&str])] = &[
        ("initial-json", true, &[]),
        ("initial-markdown", false, &[]),
        (
            "optional-answer",
            true,
            &[
                "--topic",
                "Teach me dispatch queues",
                "--mode",
                "tutorial",
                "--answer",
                "Production capacity is 128 jobs",
            ],
        ),
        (
            "supplied-task",
            true,
            &[
                "--task",
                "Refactor dispatch without changing tenant ordering",
            ],
        ),
    ];
    for (name, json, extra) in cases {
        let ledger_path = ledger_base.join(format!("{name}.json"));
        let mut child = command(&config.config_path);
        if *json {
            child.arg("--json");
        }
        child
            .args(["onboard", "--no-inspect", "--no-cache"])
            .args(*extra)
            .env("LORE_USAGE_LEDGER", &ledger_path);
        // Every platform exercises the actual executable and native stack.
        // Linux additionally matches the Windows 1 MiB stack reserve; Darwin
        // rejects this stack-limit change in the test worker's fork context.
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;

            // SAFETY: the child hook calls only the async-signal-safe setrlimit;
            // no runtime, locks, allocation, or shared state is used after fork.
            unsafe {
                child.pre_exec(|| {
                    let limit = libc::rlimit {
                        rlim_cur: 1024 * 1024,
                        rlim_max: 1024 * 1024,
                    };
                    if libc::setrlimit(libc::RLIMIT_STACK, &limit) == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                });
            }
        }
        let text = successful(child.output().unwrap());
        assert!(text.contains(CONDITION), "case: {name}");
        if *json {
            let result: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(result["model_calls"], 0, "case: {name}");
        }
        let ledger: lore::inference::usage::UsageLedger =
            serde_json::from_slice(&fs::read(&ledger_path).unwrap()).unwrap();
        assert_eq!(ledger.invocation_status, "completed", "case: {name}");
        assert!(ledger.events.is_empty(), "case: {name}");
        assert_eq!(
            ledger.summary.provider_request_count,
            Some(0),
            "case: {name}"
        );
    }
    assert_eq!(files(&config.base), before);
}

#[test]
fn invalid_onboard_cli_inputs_are_rejected_before_configuration_is_opened() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing.yml");
    let cases: &[&[&str]] = &[
        &["--mode", "quiz"],
        &["--activity", "mastered"],
        &["--hint", "4"],
        &["--max-tokens", "0"],
        &["--max-tokens", "511"],
        &["--max-tokens", "100001"],
        &["--topic", "   "],
        &["--task", ""],
        &["--answer", " "],
        &["--lesson", "blake3:not-a-returned-digest"],
        &["goal", "--topic", "other"],
        &["--inspect", "--no-inspect"],
    ];
    for args in cases {
        let output = command(&missing)
            .arg("onboard")
            .args(*args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "args: {args:?}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            !error.contains("load configuration"),
            "args: {args:?}; {error}"
        );
        assert!(output.stdout.is_empty());
    }
    assert!(!missing.exists());
    assert!(files(temp.path()).is_empty());
}
