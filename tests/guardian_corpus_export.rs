//! Explicit source-assertion materializer for the synthetic guardian corpus.
//! This does not impersonate a model or independently reviewed extraction.
//! Export is opt-in; normal test runs only validate the frozen corpus shape.

use lore::{
    config::{Config, ResolvedConfig},
    domain::AssertionProposal,
    imports::relationships::{self, CrossSourceEndpoint, CrossSourceRelation},
    sources::{self, Document},
    storage, util,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

fn sha256(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn capture_state(
    config: &ResolvedConfig,
    conn: &Connection,
    source_root: &Path,
    state: &Value,
    units: &mut BTreeMap<String, String>,
    latest: &mut BTreeMap<String, String>,
) {
    let records = state["capture_records"].as_array().unwrap();
    let paths: BTreeSet<_> = state["source_fingerprint"]["files_sha256"]
        .as_object()
        .unwrap()
        .keys()
        .filter_map(|name| name.strip_prefix("docs/"))
        .collect();
    let heads = storage::source_heads(conn).unwrap();
    for head in &heads {
        if !paths.contains(head.path.as_str()) {
            storage::retire_source(conn, &head.id).unwrap();
        }
    }
    for path in paths {
        assert!(
            !path.is_empty()
                && !path.contains('\\')
                && Path::new(path)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "Fixture source paths must stay inside the selected docs root"
        );
        let physical = source_root.join("docs").join(path);
        util::reject_symlinks(&physical).unwrap();
        assert_eq!(
            sha256(&physical),
            state["source_fingerprint"]["files_sha256"][format!("docs/{path}")],
            "Fixture source bytes changed after preparation"
        );
        let text = fs::read_to_string(&physical).unwrap();
        let (root, configured_path) = config.roots.first().unwrap();
        let document = Document {
            root_id: root.clone(),
            root_path: configured_path.clone(),
            material: Default::default(),
            origin: None,
            relative_path: path.into(),
            physical_path: physical,
            digest: util::digest(&text),
            chunks: sources::split_markdown(&text, root, path, 8000).unwrap(),
            text,
        };
        let previous = heads
            .iter()
            .find(|head| head.root == *root && head.path == path);
        if previous.is_some_and(|head| head.digest == document.digest) {
            continue;
        }
        if let Some(head) = previous {
            for (_, (section, _)) in storage::current_chunks(conn, &head.id).unwrap() {
                storage::retire_section(conn, &section).unwrap();
            }
        }
        let (source, revision) = storage::begin_source(conn, &document, previous).unwrap();
        for record in records
            .iter()
            .filter(|record| record["file"] == format!("docs/{path}"))
        {
            let mut fields = record.as_object().unwrap().clone();
            let key = fields.remove("key").unwrap().as_str().unwrap().to_owned();
            fields.remove("file");
            let proposal: AssertionProposal =
                serde_json::from_value(Value::Object(fields)).unwrap();
            let chunk = document
                .chunks
                .iter()
                .find(|chunk| chunk.text.contains(&proposal.quote))
                .unwrap();
            let (section, section_revision) =
                storage::begin_section(conn, &source, &revision, chunk).unwrap();
            let (assertion, _) = storage::capture_assertion(
                conn,
                storage::AssertionCapture {
                    document: &document,
                    chunk,
                    proposal: &proposal,
                    source: &source,
                    source_revision: &revision,
                    section: &section,
                    section_revision: &section_revision,
                    model: "explicit-synthetic-guardian-capture-no-inference",
                },
            )
            .unwrap();
            let meaning = util::json_digest(&(
                &proposal.topic,
                &proposal.subject,
                &proposal.statement,
                &proposal.kind,
                &proposal.lifecycle,
                &proposal.scope,
                &proposal.effective_at,
            ))
            .unwrap();
            let id = if let Some(id) = units.get(&meaning) {
                storage::assign(conn, &assertion, id).unwrap();
                id.clone()
            } else {
                let id =
                    storage::create_unit(conn, &config.project_id, &assertion, &proposal).unwrap();
                units.insert(meaning, id.clone());
                id
            };
            latest.insert(key, id);
        }
    }
    storage::refresh_knowledge(conn, &config.project_id).unwrap();
    // Do not retain a derived interpretation against replaced source revisions.
    // Its exact source-owned historical record stays available for comparison.
    for relation in relationships::relations(conn).unwrap() {
        if relationships::validate_current_relation(conn, &relation).is_err() {
            conn.execute(
                "DELETE FROM cross_source_current WHERE input_signature=?1",
                [&relation.input_signature],
            )
            .unwrap();
        }
    }
    if let Some(change) = state["event"].get("interpretation") {
        let pair = change["pair"].as_str().unwrap();
        if change["action"] == "withdraw" {
            conn.execute("DELETE FROM cross_source_current WHERE pair_key=?1", [pair])
                .unwrap();
        } else {
            let views = storage::views(conn).unwrap();
            let endpoint = |key: &str| {
                let view = views.iter().find(|view| view.id == latest[key]).unwrap();
                (
                    CrossSourceEndpoint {
                        kind: "knowledge".into(),
                        id: view.id.clone(),
                        revision_id: view.revision_id.clone(),
                    },
                    view.evidence
                        .iter()
                        .filter(|evidence| evidence.active)
                        .map(|evidence| evidence.id.clone())
                        .collect::<Vec<_>>(),
                )
            };
            let (from, from_evidence) = endpoint(change["from"].as_str().unwrap());
            let (to, to_evidence) = endpoint(change["to"].as_str().unwrap());
            let signature = util::json_digest(&(change, &from, &to)).unwrap();
            let kind = change["kind"].as_str().unwrap();
            let relation = CrossSourceRelation {
                id: format!("xrel_{}", &util::digest(format!("{signature}:{kind}"))[7..]),
                input_signature: signature,
                from,
                to: Some(to),
                kind: kind.into(),
                reason: change["reason"].as_str().unwrap().into(),
                qualifications: serde_json::from_value(change["qualifications"].clone()).unwrap(),
                evidence_ids: from_evidence
                    .into_iter()
                    .chain(to_evidence)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                upstream_kind: None,
                upstream_status: None,
                upstream_active: None,
                review_id: None,
                active: true,
            };
            conn.execute("INSERT INTO cross_source_evaluations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'2026-10-10')",
                params![relation.input_signature, pair, relation.from.kind, relation.from.id,
                    relation.from.revision_id, relation.to.as_ref().unwrap().kind,
                    relation.to.as_ref().unwrap().id, relation.to.as_ref().unwrap().revision_id, relation.kind]).unwrap();
            conn.execute(
                "INSERT INTO cross_source_relations VALUES(?1,?2,?3)",
                params![
                    relation.id,
                    relation.input_signature,
                    serde_json::to_string(&relation).unwrap()
                ],
            )
            .unwrap();
            conn.execute("INSERT INTO cross_source_current VALUES(?1,?2) ON CONFLICT(pair_key) DO UPDATE SET input_signature=excluded.input_signature",
                params![pair, relation.input_signature]).unwrap();
            relationships::validate_current_relation(conn, &relation).unwrap();
        }
    }
}

#[test]
fn synthetic_guardian_corpus_has_sixty_successive_labeled_transitions() {
    let manifest: Value =
        serde_json::from_str(include_str!("../evaluation/corpora/guardian/events.json")).unwrap();
    assert_eq!(manifest["fixture_only"], true);
    assert_eq!(manifest["label_provenance"]["independent_review"], false);
    let mut categories = BTreeMap::new();
    for project in manifest["projects"].as_array().unwrap() {
        assert_eq!(project["events"].as_array().unwrap().len(), 20);
        let mut parent = project["initial_revision"].as_str().unwrap();
        for event in project["events"].as_array().unwrap() {
            assert_eq!(event["parent"], parent);
            parent = event["revision"].as_str().unwrap();
            *categories
                .entry(event["expected"]["significance"].as_str().unwrap())
                .or_insert(0) += 1;
            assert!(
                !event["expected"]["safe_next_step"]
                    .as_str()
                    .unwrap()
                    .is_empty()
            );
        }
    }
    assert_eq!(
        categories,
        BTreeMap::from([("ambiguous", 20), ("benign", 20), ("consequential", 20)])
    );
}

#[test]
fn export_synthetic_guardian_retained_snapshots_when_explicitly_requested() {
    let Ok(prepared_path) = std::env::var("LORE_GUARDIAN_PREPARED") else {
        return;
    };
    let output =
        std::env::var("LORE_GUARDIAN_COMPILED").expect("Explicit fixture output is required");
    let prepared_path = Path::new(&prepared_path);
    let output = Path::new(&output);
    assert!(
        !output.exists(),
        "Refusing to overwrite an existing fixture export"
    );
    util::reject_symlinks(prepared_path).unwrap();
    util::reject_symlinks(output).unwrap();
    let prepared: Value =
        serde_json::from_str(&fs::read_to_string(prepared_path.join("prepared.json")).unwrap())
            .unwrap();
    assert_eq!(
        prepared["fixture_only"], true,
        "Explicit assertions are a debug fixture, not real extraction"
    );
    fs::create_dir_all(output).unwrap();
    let mut exported = Vec::new();
    for project in prepared["projects"].as_array().unwrap() {
        let project_id = project["id"].as_str().unwrap();
        assert!(
            !project_id.is_empty()
                && project_id.len() <= 80
                && project_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
            "Fixture project identity must not redirect writes"
        );
        let workspace = output.join("workspaces").join(project_id);
        fs::create_dir_all(workspace.join("docs")).unwrap();
        fs::create_dir_all(workspace.join(".lore")).unwrap();
        let mut settings = Config::default();
        settings.project.name = format!("guardian-debug-{project_id}");
        settings.models.generative.enabled = false;
        settings.output.wiki_dir = "wiki".into();
        let config_path = workspace.join("lore.yml");
        fs::write(&config_path, serde_yaml::to_string(&settings).unwrap()).unwrap();
        let config = ResolvedConfig::load_for_read(&config_path).unwrap();
        let conn = Connection::open(config.state.join("state.db")).unwrap();
        storage::migrate(&conn).unwrap();
        storage::set_meta(&conn, "initialized", "true").unwrap();
        conn.execute(
            "INSERT INTO projects VALUES(?1,?2,'2026-10-10')",
            params![config.project_id, settings.project.name],
        )
        .unwrap();
        for (id, path) in &config.roots {
            conn.execute(
                "INSERT INTO source_roots VALUES(?1,?2,?3)",
                params![id, config.project_id, path.to_string_lossy()],
            )
            .unwrap();
        }
        let mut units = BTreeMap::new();
        let mut latest = BTreeMap::new();
        for state in project["states"].as_array().unwrap() {
            let revision = state["revision"].as_str().unwrap();
            assert!(
                !revision.is_empty()
                    && revision.len() <= 80
                    && revision
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
                "Fixture revision identity must not redirect writes"
            );
            assert_eq!(
                state["source_root"],
                format!("snapshots/{project_id}/{revision}")
            );
            capture_state(
                &config,
                &conn,
                &prepared_path.join(state["source_root"].as_str().unwrap()),
                state,
                &mut units,
                &mut latest,
            );
            let relative = format!(
                "snapshots/{project_id}/{}/state.db",
                state["revision"].as_str().unwrap()
            );
            let path = output.join(&relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            conn.backup(rusqlite::MAIN_DB, &path, None).unwrap();
            exported.push(json!({"project": project_id, "revision": state["revision"],
                "database": relative, "database_sha256": sha256(&path)}));
        }
    }
    fs::write(output.join("compiled.json"), serde_json::to_vec_pretty(&json!({
        "schema_version": 1, "protocol": "guardian-longitudinal-v1", "fixture_only": true,
        "preparation_model_calls": 0, "capture_basis": "explicit synthetic source assertions; no model or independent extraction",
        "prepared_sha256": sha256(&prepared_path.join("prepared.json")), "states": exported,
    })).unwrap()).unwrap();
}
