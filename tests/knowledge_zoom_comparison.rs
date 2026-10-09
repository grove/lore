//! Measured, synthetic diagnostics for two real read-only retrieval paths.
//! Gold expectations are authored here, independently of graph construction.
//! Passing this test establishes no superiority or human comprehension benefit.

mod common;

use anyhow::{Result, ensure};
use lore::{
    config::ResolvedConfig,
    context::{self, ContextOptions, ContextResult},
    engine::{self, UpdateOptions},
    knowledge::{self, ExploreOptions, ExploreResult},
    storage, util,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Instant,
};

const REPEATS: usize = 3;
const OUTPUT_ENV: &str = "LORE_ZOOM_COMPARISON_OUTPUT";
const PURPOSE: &str = "Harbor isolates dispatch admission from worker execution so a slow tenant cannot consume every worker.";
const CAPACITY: &str = "QUEUE_CAPACITY is 128 jobs in production and 64 jobs in staging.";
const FULL: &str = "A full production dispatch queue must return Busy without accepting a job.";
const EXCEPTION: &str = "Except during an emergency drain, production dispatch must preserve tenant order; a drain still refuses new jobs.";
const BACKOFF: &str = "MAX_BACKOFF_MS is 750 in src/retry/config.rs for idempotent requests.";
const PROPOSAL: &str =
    "Proposed staging queues would hold 256 jobs after the next capacity review.";
const MYSQL: &str = "MySQL is the selected database.";
const POSTGRESQL: &str = "PostgreSQL is the selected database.";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Condition {
    knowledge_id: String,
    statement: String,
    kind: String,
    lifecycle: String,
    scope: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryCase {
    id: String,
    query: String,
    budgets: Vec<usize>,
    expected_knowledge_ids: Vec<String>,
    expected_evidence_ids: Vec<String>,
    critical_conditions: Vec<Condition>,
}

#[derive(Clone, Copy)]
enum Arm {
    Flat,
    Zoom,
}

enum Output {
    Flat(Box<ContextResult>),
    Zoom(Box<ExploreResult>),
}

#[derive(Debug, Serialize)]
struct Recall {
    expected: usize,
    retained: usize,
    fraction: Option<f64>,
    missing_ids: Vec<String>,
}

fn recall(expected: &[String], retained: &BTreeSet<String>) -> Recall {
    let missing_ids: Vec<_> = expected
        .iter()
        .filter(|id| !retained.contains(*id))
        .cloned()
        .collect();
    let matched = expected.len() - missing_ids.len();
    Recall {
        expected: expected.len(),
        retained: matched,
        fraction: (!expected.is_empty()).then(|| matched as f64 / expected.len() as f64),
        missing_ids,
    }
}

fn condition_matches(condition: &Condition, record: &Value) -> bool {
    record["id"] == condition.knowledge_id
        && record["statement"] == condition.statement
        && record["kind"] == condition.kind
        && record["lifecycle"] == condition.lifecycle
        && record["scope"] == condition.scope
}

fn original_records(conn: &Connection) -> BTreeMap<String, Value> {
    // Bound optional existing-project evaluation before materializing records.
    let count: usize = conn
        .query_row("SELECT count(*) FROM knowledge_current", [], |r| r.get(0))
        .unwrap();
    assert!(
        count <= 5_000,
        "comparison accepts at most 5000 current records"
    );
    let bytes: usize = conn.query_row(
        "SELECT COALESCE(sum(length(CAST(exact_excerpt AS BLOB)) + length(CAST(context_before AS BLOB)) + length(CAST(context_after AS BLOB))),0) FROM evidence_snapshots",
        [], |r| r.get(0),
    ).unwrap();
    assert!(
        bytes <= 32 * 1024 * 1024,
        "comparison evidence exceeds 32 MiB"
    );
    storage::views(conn)
        .unwrap()
        .into_iter()
        .map(|record| (record.id.clone(), serde_json::to_value(record).unwrap()))
        .collect()
}

fn validate_cases(conn: &Connection, cases: &[QueryCase], originals: &BTreeMap<String, Value>) {
    assert!(!cases.is_empty() && cases.len() <= 16);
    let mut names = BTreeSet::new();
    for case in cases {
        assert!(!case.id.is_empty() && names.insert(&case.id));
        assert!(!case.query.trim().is_empty() && case.query.len() <= 8_000);
        assert!(!case.budgets.is_empty() && case.budgets.len() <= 4);
        assert!(
            case.budgets
                .iter()
                .all(|budget| (512..=100_000).contains(budget))
        );
        for ids in [&case.expected_knowledge_ids, &case.expected_evidence_ids] {
            assert!(ids.len() <= 128);
            assert_eq!(ids.len(), ids.iter().collect::<BTreeSet<_>>().len());
        }
        for id in &case.expected_knowledge_ids {
            assert!(
                originals.contains_key(id),
                "unknown gold knowledge ID: {id}"
            );
        }
        for id in &case.expected_evidence_ids {
            let source = storage::evidence_snapshot(conn, id).unwrap();
            assert_eq!(util::digest(&source.excerpt), source.digest);
        }
        let mut critical_ids = BTreeSet::new();
        for condition in &case.critical_conditions {
            assert!(critical_ids.insert(&condition.knowledge_id));
            assert!(
                case.expected_knowledge_ids
                    .contains(&condition.knowledge_id)
            );
            assert!(
                condition_matches(condition, &originals[&condition.knowledge_id]),
                "gold critical statement or qualification does not match the registry"
            );
        }
    }
}

fn records(arm: Arm, value: &Value) -> Vec<&Value> {
    match arm {
        Arm::Flat => value["sections"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|section| section.as_array().unwrap())
            .collect(),
        Arm::Zoom => value["knowledge"].as_array().unwrap().iter().collect(),
    }
}

fn evidence_matches(conn: &Connection, arm: Arm, evidence: &Value, current: bool) -> bool {
    let Some(id) = evidence["id"].as_str() else {
        return false;
    };
    let Ok(original) = storage::evidence_snapshot(conn, id) else {
        return false;
    };
    if util::digest(&original.excerpt) != original.digest {
        return false;
    }
    match arm {
        Arm::Zoom => *evidence == serde_json::to_value(original).unwrap(),
        Arm::Flat => {
            evidence["source"] == format!("{}:{}", original.root_id, original.path)
                && evidence["source_revision_id"] == original.source_revision_id
                && evidence["excerpt"] == original.excerpt
                && evidence["material"] == serde_json::to_value(original.material).unwrap()
                && evidence["origin"] == json!(original.origin)
                && evidence["line_start"] == json!(original.line_start)
                && evidence["line_end"] == json!(original.line_end)
                && evidence["current"] == current
                && evidence["provenance_recorded"] == original.root_path.is_some()
        }
    }
}

/// Check additional documentary references directly against the registry. This
/// deliberately does not call the graph validator or reconstruct its grouping.
fn validate_manifest(
    conn: &Connection,
    arm: Arm,
    value: &Value,
    originals: &BTreeMap<String, Value>,
) -> Result<Value> {
    let returned = records(arm, value);
    let ids: BTreeSet<_> = returned
        .iter()
        .map(|record| record["id"].as_str().unwrap())
        .collect();
    let evidence_ids: BTreeSet<_> = value["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    let facts = storage::relation_facts(conn)?;
    let relations = value["relations"].as_array().unwrap();
    for relation in relations {
        let from = relation["from"].as_str().unwrap_or("");
        let to = relation["to"].as_str().unwrap_or("");
        let evidence_id = relation["evidence_id"].as_str().unwrap_or("");
        ensure!(
            ids.contains(from) && ids.contains(to),
            "relation endpoint missing from original records"
        );
        ensure!(
            evidence_ids.contains(evidence_id),
            "relation witness missing from source manifest"
        );
        let witness = storage::evidence_snapshot(conn, evidence_id)?;
        let matches = facts.iter().any(|fact| {
            if relation["id"].as_str() != Some(fact.id.as_str())
                || fact.source_id != witness.source_id
                || fact.source_revision_id != witness.source_revision_id
                || fact.exact_excerpt != witness.excerpt
            {
                return false;
            }
            let expected = match arm {
                Arm::Zoom => serde_json::to_value(fact).unwrap(),
                Arm::Flat => json!({
                    "id": fact.id, "from": fact.from, "to": fact.to,
                    "kind": fact.kind, "current": fact.active,
                    "evidence_id": fact.evidence_id, "material": witness.material,
                }),
            };
            expected == *relation
        });
        ensure!(
            matches,
            "relation fields or source witness differ from original documentary relationship"
        );
    }
    let mut embedded_evidence_count = 0;
    let mut source_revision_count = 0;
    let mut summary_reference_count = 0;
    if let Arm::Zoom = arm {
        for record in &returned {
            let id = record["id"].as_str().unwrap();
            ensure!(
                originals.get(id) == Some(*record),
                "embedded knowledge revision, source metadata or relationship text differs from original"
            );
            embedded_evidence_count += record["evidence"].as_array().unwrap().len();
        }
        let source_heads = storage::source_heads(conn)?;
        let revisions = value["source_revisions"].as_array().unwrap();
        let mut revision_ids = BTreeSet::new();
        for revision in revisions {
            let id = revision["id"].as_str().unwrap_or("");
            ensure!(revision_ids.insert(id), "duplicate source revision");
            let (source_id, path, digest): (String, String, String) = conn.query_row(
                "SELECT source_id,observed_path,content_digest FROM source_revisions WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let provenance = storage::source_provenance(conn, id)?;
            let expected = json!({
                "id": id, "source_id": source_id, "root_id": provenance.root_id,
                "observed_path": path, "content_digest": digest,
                "current": source_heads.iter().any(|source| source.revision == id),
            });
            ensure!(
                *revision == expected,
                "source revision digest, captured root, path or status differs from original"
            );
        }
        let cited_revisions: BTreeSet<_> = value["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["source_revision_id"].as_str().unwrap())
            .collect();
        ensure!(
            revision_ids == cited_revisions,
            "source revision manifest has missing or extra entries"
        );
        source_revision_count = revisions.len();
        let nodes = value["nodes"].as_array().unwrap();
        let node_ids: BTreeSet<_> = nodes
            .iter()
            .map(|node| node["id"].as_str().unwrap())
            .collect();
        ensure!(node_ids.len() == nodes.len(), "duplicate navigation node");
        for node in nodes {
            let summary = node["summary"].as_str().unwrap_or("");
            let mut seen = BTreeSet::new();
            for reference in node["summary_knowledge_ids"].as_array().unwrap() {
                let id = reference.as_str().unwrap_or("");
                ensure!(
                    seen.insert(id) && ids.contains(id),
                    "summary cites a duplicate or unavailable original record"
                );
                let original = &originals[id];
                for field in ["statement", "kind", "lifecycle", "scope"] {
                    ensure!(
                        summary.contains(original[field].as_str().unwrap()),
                        "extractive summary omits cited original text or qualifications"
                    );
                }
                summary_reference_count += 1;
            }
        }
        for edge in value["edges"].as_array().unwrap() {
            ensure!(
                node_ids.contains(edge["parent"].as_str().unwrap_or(""))
                    && node_ids.contains(edge["child"].as_str().unwrap_or("")),
                "displayed navigation edge has a missing endpoint"
            );
        }
    }
    Ok(json!({
        "documentary_relation_witnesses_checked": relations.len(),
        "embedded_evidence_records_checked": embedded_evidence_count,
        "source_revisions_checked": source_revision_count,
        "summary_original_references_checked": summary_reference_count,
    }))
}

fn score(
    conn: &Connection,
    arm: Arm,
    value: &Value,
    case: &QueryCase,
    originals: &BTreeMap<String, Value>,
) -> Value {
    let returned = records(arm, value);
    let mut activity: BTreeMap<String, bool> = BTreeMap::new();
    for record in &returned {
        let original = originals
            .get(record["id"].as_str().unwrap())
            .expect("returned invented knowledge ID");
        for reference in original["evidence"].as_array().unwrap() {
            *activity
                .entry(reference["id"].as_str().unwrap().to_owned())
                .or_default() |= reference["active"].as_bool().unwrap();
        }
    }
    for relation in storage::relation_facts(conn).unwrap() {
        if value["relations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|returned| returned["id"] == relation.id)
        {
            *activity.entry(relation.evidence_id).or_default() |= relation.active;
        }
    }
    let evidence = value["evidence"].as_array().unwrap();
    let valid_evidence: BTreeSet<String> = evidence
        .iter()
        .filter(|entry| {
            evidence_matches(
                conn,
                arm,
                entry,
                activity
                    .get(entry["id"].as_str().unwrap_or(""))
                    .copied()
                    .unwrap_or(false),
            )
        })
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        evidence.len(),
        valid_evidence.len(),
        "invalid or duplicate source evidence"
    );
    let ids: BTreeSet<String> = returned
        .iter()
        .map(|record| record["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(returned.len(), ids.len(), "duplicate original record");
    for record in &returned {
        let id = record["id"].as_str().unwrap();
        let original = originals.get(id).expect("returned invented knowledge ID");
        for field in [
            "statement",
            "kind",
            "lifecycle",
            "scope",
            "effective_at",
            "topic",
            "subject",
            "support_state",
        ] {
            assert_eq!(
                record[field].as_str().unwrap_or(""),
                original[field].as_str().unwrap_or(""),
                "altered original {id} field {field}"
            );
        }
        let refs: Vec<&str> = match arm {
            Arm::Flat => record["evidence_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap())
                .collect(),
            Arm::Zoom => record["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_str().unwrap())
                .collect(),
        };
        assert!(!refs.is_empty(), "source-less original record");
        for id in refs {
            assert!(
                valid_evidence.contains(id),
                "dangling or invalid source link"
            );
            assert!(
                original["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["id"] == id),
                "source belongs to a different original record"
            );
        }
    }
    let critical: BTreeSet<String> = case
        .critical_conditions
        .iter()
        .filter(|condition| {
            returned
                .iter()
                .any(|record| condition_matches(condition, record))
        })
        .map(|condition| condition.knowledge_id.clone())
        .collect();
    let expected_critical: Vec<_> = case
        .critical_conditions
        .iter()
        .map(|c| c.knowledge_id.clone())
        .collect();
    let manifest_checks = validate_manifest(conn, arm, value, originals).unwrap();
    json!({
        "exact_knowledge_id_recall": recall(&case.expected_knowledge_ids, &ids),
        "original_source_evidence_recall": recall(&case.expected_evidence_ids, &valid_evidence),
        "complete_critical_condition_recall": recall(&expected_critical, &critical),
        "returned_knowledge_ids": ids,
        "returned_evidence_ids": valid_evidence,
        "additional_record_count": ids.iter().filter(|id| !case.expected_knowledge_ids.contains(id)).count(),
        "invalid_source_references": 0,
        "altered_original_records": 0,
        "manifest_checks": manifest_checks,
        "validation_scope": "documentary originals, source evidence, relationship witnesses, source revisions and summary references; native imports and grouping quality are not scored",
    })
}

fn measure(
    conn: &Connection,
    arm: Arm,
    case: &QueryCase,
    max_tokens: usize,
    originals: &BTreeMap<String, Value>,
) -> (Value, u64) {
    let start = Instant::now();
    let result = match arm {
        Arm::Flat => context::build_context(
            conn,
            &ContextOptions {
                task: case.query.clone(),
                paths: Vec::new(),
                max_tokens,
            },
        )
        .map(|result| Output::Flat(Box::new(result))),
        Arm::Zoom => knowledge::explore(
            conn,
            &ExploreOptions {
                query: case.query.clone(),
                max_tokens,
                max_nodes: 24,
                node: None,
            },
        )
        .map(|result| Output::Zoom(Box::new(result))),
    };
    let elapsed_us = u64::try_from(start.elapsed().as_micros()).unwrap();
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            return (
                json!({"status": "error", "error": error.to_string()}),
                elapsed_us,
            );
        }
    };
    // Serialize the actual typed result, preserving the CLI's field order.
    // Reserializing a generic Value could change tokenizer boundary counts.
    let (cli_json, pretty_json) = match &output {
        Output::Flat(result) => (
            serde_json::to_string(result).unwrap() + "\n",
            serde_json::to_string_pretty(result).unwrap() + "\n",
        ),
        Output::Zoom(result) => (
            serde_json::to_string(result).unwrap() + "\n",
            serde_json::to_string_pretty(result).unwrap() + "\n",
        ),
    };
    let (value, markdown, reported, status) = match output {
        Output::Flat(result) => {
            let status = if result.empty {
                "empty"
            } else if result.omissions.knowledge_units > 0 || result.retrieval_truncated {
                "partial"
            } else {
                "complete"
            };
            (
                serde_json::to_value(&result).unwrap(),
                context::render_context(&result),
                result.budget.used_tokens,
                status.to_owned(),
            )
        }
        Output::Zoom(result) => (
            serde_json::to_value(&result).unwrap(),
            knowledge::render_markdown(&result),
            result.used_tokens,
            result.status.clone(),
        ),
    };
    // Both actual CLI paths emit compact JSON plus a final newline. Zoom also
    // reserves pretty-JSON space internally; report that overhead separately.
    let json_tokens = context::count_tokens(&cli_json);
    let markdown_tokens = context::count_tokens(&markdown);
    let complete_tokens = json_tokens.max(markdown_tokens);
    assert!(complete_tokens <= reported && reported <= max_tokens);
    assert_eq!(value["model_calls"], 0);
    (
        json!({
            "status": status,
            "schema_version": value["schema_version"],
            "retrieval": value.get("retrieval").cloned().unwrap_or(json!("flat_lexical_and_recorded_relationships")),
            "model_calls": value["model_calls"],
            "output": {
                "cli_json_tokens": json_tokens,
                "cli_json_bytes": cli_json.len(),
                "pretty_json_tokens_diagnostic": context::count_tokens(&pretty_json),
                "markdown_tokens": markdown_tokens,
                "markdown_bytes": markdown.len(),
                "complete_cli_tokens": complete_tokens,
                "reported_used_tokens": reported,
                "max_tokens": max_tokens,
                "cli_json_digest": util::digest(&cli_json),
                "markdown_digest": util::digest(&markdown),
            },
            "scores": score(conn, arm, &value, case, originals),
            "omissions": value.get("omissions").cloned().unwrap_or_else(|| json!({"coverage": value["coverage"], "critical_groups_omitted": value["critical_groups_omitted"]})),
            "warnings": value["warnings"],
        }),
        elapsed_us,
    )
}

fn comparison(conn: &Connection, cases: &[QueryCase], classification: &str) -> Value {
    conn.execute_batch("SAVEPOINT independent_zoom_comparison")
        .unwrap();
    let originals = original_records(conn);
    validate_cases(conn, cases, &originals);
    let snapshot = storage::registry_revision(conn).unwrap();
    let logged_before: u64 = conn
        .query_row("SELECT count(*) FROM model_calls", [], |r| r.get(0))
        .unwrap();
    let changes_before = conn.total_changes();
    // Initialize the embedded tokenizer outside timing. SQLite and OS page
    // caches remain uncontrolled and are explicitly not called cold-cache runs.
    context::count_tokens("initialize comparison tokenizer");
    let mut measurements = Vec::new();
    for (case_index, case) in cases.iter().enumerate() {
        for &budget in &case.budgets {
            let mut outputs: [Vec<Value>; 2] = [Vec::new(), Vec::new()];
            let mut times: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
            for repeat in 0..REPEATS {
                let order = if (case_index + repeat) % 2 == 0 {
                    [Arm::Flat, Arm::Zoom]
                } else {
                    [Arm::Zoom, Arm::Flat]
                };
                for arm in order {
                    let index = match arm {
                        Arm::Flat => 0,
                        Arm::Zoom => 1,
                    };
                    let (output, elapsed) = measure(conn, arm, case, budget, &originals);
                    outputs[index].push(output);
                    times[index].push(elapsed);
                }
            }
            let arms: Vec<_> = outputs.into_iter().zip(times).map(|(outputs, times)| {
                assert!(outputs.iter().all(|output| *output == outputs[0]), "unchanged input produced different output");
                let mut sorted = times.clone();
                sorted.sort_unstable();
                json!({"result": outputs[0], "elapsed_retrieval_us": times, "median_elapsed_retrieval_us": sorted[REPEATS / 2], "identical_outputs_across_repeats": true})
            }).collect();
            measurements.push(json!({"case_id": case.id, "query": case.query, "max_tokens": budget, "flat_direct": arms[0], "knowledge_zoom": arms[1]}));
        }
    }
    let logged_after: u64 = conn
        .query_row("SELECT count(*) FROM model_calls", [], |r| r.get(0))
        .unwrap();
    assert_eq!(logged_before, logged_after);
    assert_eq!(changes_before, conn.total_changes());
    assert_eq!(snapshot, storage::registry_revision(conn).unwrap());
    let sources = storage::source_heads(conn).unwrap();
    conn.execute_batch("RELEASE independent_zoom_comparison")
        .unwrap();
    json!({
        "schema_version": 1,
        "measurement_classification": classification,
        "package_version": env!("CARGO_PKG_VERSION"),
        "build": {
            "debug_assertions": cfg!(debug_assertions),
            "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "source_digests": {
                "tests/knowledge_zoom_comparison.rs": util::digest(include_str!("knowledge_zoom_comparison.rs")),
                "tests/common/mod.rs": util::digest(include_str!("common/mod.rs")),
                "src/context.rs": util::digest(include_str!("../src/context.rs")),
                "src/context/retrieval.rs": util::digest(include_str!("../src/context/retrieval.rs")),
                "src/knowledge.rs": util::digest(include_str!("../src/knowledge.rs")),
                "src/knowledge/graph.rs": util::digest(include_str!("../src/knowledge/graph.rs")),
                "src/knowledge/selection.rs": util::digest(include_str!("../src/knowledge/selection.rs")),
                "src/storage.rs": util::digest(include_str!("../src/storage.rs")),
                "Cargo.lock": util::digest(include_str!("../Cargo.lock")),
            },
        },
        "registry_revision": snapshot,
        "source_inventory": sources,
        "original_record_count": originals.len(),
        "gold_cases": cases,
        "repeats_per_arm": REPEATS,
        "timing_scope": "entrypoint elapsed time; flat build_context versus uncached explore including full graph build and validation; excludes fixture compilation, scoring and report serialization",
        "timing_order": "alternate arms by case index and repeat; tokenizer initialized; SQLite and OS page caches uncontrolled",
        "tokenizer": "cl100k_base",
        "derived_cache": "disabled for both arms",
        "read_only_connection": true,
        "new_logged_model_calls": logged_after - logged_before,
        "sqlite_changes": conn.total_changes() - changes_before,
        "measurements": measurements,
        "interpretation": "synthetic or annotated-registry retrieval diagnostics; no latency threshold, superiority assertion, semantic-accuracy score or measured human learning outcome",
    })
}

fn save_if_requested(report: &Value) {
    let Some(path) = std::env::var_os(OUTPUT_ENV).map(PathBuf::from) else {
        return;
    };
    assert!(
        path.is_absolute(),
        "{OUTPUT_ENV} must be an absolute new file path"
    );
    let text = serde_json::to_string_pretty(report).unwrap() + "\n";
    assert!(
        text.len() <= 8 * 1024 * 1024,
        "comparison report exceeds 8 MiB"
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.sync_all().unwrap();
    println!("Knowledge Zoom comparison report: {}", path.display());
}

fn fixture_cases(conn: &Connection) -> Vec<QueryCase> {
    let originals = original_records(conn);
    let find = |statement: &str| {
        let matched: Vec<_> = originals
            .values()
            .filter(|record| record["statement"] == statement)
            .collect();
        assert_eq!(
            matched.len(),
            1,
            "fixture must retain one original for {statement}"
        );
        matched[0]["id"].as_str().unwrap().to_owned()
    };
    let case = |id: &str, query: String, expected: &[&str], critical: &[(&str, &str, &str)]| {
        let expected_ids: Vec<_> = expected.iter().map(|statement| find(statement)).collect();
        let evidence: BTreeSet<_> = expected_ids
            .iter()
            .flat_map(|id| {
                originals[id]["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| e["id"].as_str().unwrap().to_owned())
            })
            .collect();
        QueryCase {
            id: id.into(),
            query,
            budgets: vec![1_500, 8_000],
            expected_knowledge_ids: expected_ids,
            expected_evidence_ids: evidence.into_iter().collect(),
            critical_conditions: critical
                .iter()
                .map(|(statement, kind, lifecycle)| Condition {
                    knowledge_id: find(statement),
                    statement: (*statement).into(),
                    kind: (*kind).into(),
                    lifecycle: (*lifecycle).into(),
                    scope: "production".into(),
                })
                .collect(),
        }
    };
    let dispatch = [PURPOSE, CAPACITY, FULL, EXCEPTION];
    let dispatch_conditions = [
        (CAPACITY, "decision", "accepted"),
        (FULL, "decision", "accepted"),
        (EXCEPTION, "decision", "accepted"),
    ];
    vec![
        case(
            "exact_original_id",
            find(BACKOFF),
            &[BACKOFF],
            &[(BACKOFF, "decision", "accepted")],
        ),
        case(
            "exact_symbol",
            "MAX_BACKOFF_MS".into(),
            &[BACKOFF],
            &[(BACKOFF, "decision", "accepted")],
        ),
        case(
            "exact_source_path",
            "backoff.md".into(),
            &[BACKOFF],
            &[(BACKOFF, "decision", "accepted")],
        ),
        case(
            "capacity_with_rare_exception",
            "increase QUEUE_CAPACITY for production dispatch".into(),
            &dispatch,
            &dispatch_conditions,
        ),
        case(
            "conceptual_dispatch",
            "tenant worker dispatch admission".into(),
            &dispatch,
            &dispatch_conditions,
        ),
        case(
            "future_scope",
            "proposed staging queue capacity".into(),
            &[CAPACITY, PROPOSAL],
            &[
                (CAPACITY, "decision", "accepted"),
                (PROPOSAL, "proposal", "proposed"),
            ],
        ),
        case(
            "documented_disagreement",
            "selected ledger database".into(),
            &[MYSQL, POSTGRESQL],
            &[
                (MYSQL, "decision", "accepted"),
                (POSTGRESQL, "decision", "accepted"),
            ],
        ),
        case(
            "unrecorded_identifier",
            "ORBITAL_SPECTROMETER_UNRECORDED".into(),
            &[],
            &[],
        ),
    ]
}

fn check_manifest_scorer_rejections(conn: &Connection) {
    let output = knowledge::explore(
        conn,
        &ExploreOptions {
            query: "selected database".into(),
            max_tokens: 8_000,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    let original = serde_json::to_value(output).unwrap();
    let originals = original_records(conn);
    validate_manifest(conn, Arm::Zoom, &original, &originals).unwrap();
    assert!(!original["source_revisions"].as_array().unwrap().is_empty());
    assert!(!original["relations"].as_array().unwrap().is_empty());
    assert!(!original["nodes"].as_array().unwrap().is_empty());
    for (pointer, corrupted) in [
        (
            "/source_revisions/0/content_digest",
            json!("blake3:changed"),
        ),
        (
            "/knowledge/0/evidence/0/active",
            json!(
                !original["knowledge"][0]["evidence"][0]["active"]
                    .as_bool()
                    .unwrap()
            ),
        ),
        ("/relations/0/evidence_id", json!("e_not_in_this_registry")),
        ("/relations/0/from", json!("k_not_in_this_registry")),
        (
            "/nodes/0/summary_knowledge_ids",
            json!(["k_not_in_this_registry"]),
        ),
    ] {
        let mut altered = original.clone();
        *altered.pointer_mut(pointer).unwrap() = corrupted;
        assert!(
            validate_manifest(conn, Arm::Zoom, &altered, &originals).is_err(),
            "independent scorer accepted mutation at {pointer}"
        );
    }
}

#[tokio::test]
async fn measured_flat_direct_and_zoom_on_the_same_compiled_fixture() {
    let (_temp, config, model) = common::project();
    for (path, prefix, topic, statement) in [
        ("purpose.md", "DECISION", "dispatch", PURPOSE),
        ("capacity.md", "DECISION", "dispatch", CAPACITY),
        ("full-queue.md", "DECISION", "dispatch", FULL),
        ("emergency.md", "DECISION", "dispatch", EXCEPTION),
        ("backoff.md", "DECISION", "timing", BACKOFF),
        ("staging-proposal.md", "PLAN", "dispatch", PROPOSAL),
        ("database-01.md", "DECISION", "database", MYSQL),
        ("database-02.md", "DECISION", "database", POSTGRESQL),
        (
            "refunds.md",
            "DECISION",
            "refunds",
            "Refund capture must never be retried unless the request has an idempotency key.",
        ),
        (
            "security.md",
            "DECISION",
            "security",
            "Dispatch traces must never contain identity tokens.",
        ),
        (
            "ledger.md",
            "DECISION",
            "storage",
            "Ledger snapshots use a monotonically increasing sequence.",
        ),
        (
            "release-note.md",
            "REPORT",
            "reporting",
            "A release note reports that staging processed 40 jobs during a rehearsal.",
        ),
    ] {
        common::put(&config, path, &format!("{prefix} {topic}: {statement}\n"));
    }
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let calls_before = model.calls.load(Ordering::SeqCst);
    let database = config.state.join("state.db");
    let bytes_before = fs::read(&database).unwrap();
    let conn = storage::read_only(&database).unwrap();
    let cases = fixture_cases(&conn);
    let mut report = comparison(&conn, &cases, "synthetic_diagnostic_compiled_fixture");
    check_manifest_scorer_rejections(&conn);
    for row in report["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["max_tokens"] == 8_000)
    {
        for arm in ["flat_direct", "knowledge_zoom"] {
            assert_ne!(
                row[arm]["result"]["status"], "error",
                "large-budget fixture must actually execute both retrieval paths"
            );
        }
    }
    assert_eq!(calls_before, model.calls.load(Ordering::SeqCst));
    assert_eq!(bytes_before, fs::read(&database).unwrap());
    assert!(!config.state.join("knowledge-zoom").exists());
    report["fixture_ingestion"] = json!({"model": "common::FakeModel", "calls_excluded_from_retrieval": calls_before, "provider_requests": 0, "source_documents": 12});
    report["scorer_negative_probes"] = json!({"source_revision_digest": "rejected", "embedded_evidence_activity": "rejected", "relation_witness_id": "rejected", "relation_endpoint_id": "rejected", "summary_original_reference": "rejected", "unmeasured_validation_retrieval_calls": 1});
    report["registry_bytes_unchanged"] = json!(true);
    save_if_requested(&report);
}

#[test]
fn critical_recall_requires_the_whole_statement_and_its_qualifications() {
    let condition = Condition {
        knowledge_id: "gold".into(),
        statement: EXCEPTION.into(),
        kind: "decision".into(),
        lifecycle: "accepted".into(),
        scope: "production".into(),
    };
    let original = json!({"id": "gold", "statement": EXCEPTION, "kind": "decision", "lifecycle": "accepted", "scope": "production"});
    assert!(condition_matches(&condition, &original));
    for (field, value) in [
        ("statement", "Production dispatch preserves tenant order."),
        ("scope", "staging"),
        ("lifecycle", "proposed"),
        ("id", "another"),
    ] {
        let mut altered = original.clone();
        altered[field] = json!(value);
        assert!(!condition_matches(&condition, &altered));
    }
    assert_eq!(
        recall(&["gold".into()], &BTreeSet::new()).fraction,
        Some(0.0)
    );
    assert_eq!(recall(&[], &BTreeSet::new()).fraction, None);
}

#[test]
#[ignore = "requires an explicitly selected compiled project, independent gold cases and a new report path"]
fn measured_existing_compiled_project() {
    let config_path =
        std::env::var_os("LORE_ZOOM_COMPARISON_CONFIG").expect("set LORE_ZOOM_COMPARISON_CONFIG");
    let cases_path =
        std::env::var_os("LORE_ZOOM_COMPARISON_CASES").expect("set LORE_ZOOM_COMPARISON_CASES");
    assert!(std::env::var_os(OUTPUT_ENV).is_some(), "set {OUTPUT_ENV}");
    let config = ResolvedConfig::load_for_read(Path::new(&config_path)).unwrap();
    let text = util::read_limited(Path::new(&cases_path), 256 * 1024).unwrap();
    let cases: Vec<QueryCase> = serde_json::from_str(&text).unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let report = comparison(&conn, &cases, "annotated_existing_registry_diagnostic");
    save_if_requested(&report);
}
