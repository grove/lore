use lore::{
    config::{ImportKind, ImportSource},
    imports::adapters::{self, ObservationKind, ObservationVerification},
    util,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const LIMIT: usize = 1_000_000;

fn source(kind: ImportKind) -> ImportSource {
    ImportSource {
        id: "upstream".into(),
        kind,
        path: "snapshot".into(),
        project: None,
        include_memories: false,
    }
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn write_jsonl(path: &Path, values: &[Value]) {
    fs::write(
        path,
        values
            .iter()
            .map(|value| format!("{}\n", serde_json::to_string(value).unwrap()))
            .collect::<String>(),
    )
    .unwrap();
}

fn wiki(root: &Path) {
    fs::create_dir_all(root.join(".claims/payments")).unwrap();
    fs::create_dir_all(root.join("payments")).unwrap();
    fs::write(
        root.join("index.md"),
        "---\nokf_version: \"0.2\"\n---\n# Wiki\n",
    )
    .unwrap();
    fs::write(root.join("payments/retries.md"), "---\ntype: Reference\ntitle: Payment retries\nsubject: payment-retry-policy\ncomponent: payment-processing\nenvironment: production\ntags: [payments, retries]\nsources:\n  - resource: repo://src/payments/retry.ts\nverified:\n  - by: human\n    at: 2026-10-01T12:00:00Z\n---\n# Payment retries\n\nThe retry limit is five. See [Policy](../../docs/adr-017.md).\n").unwrap();
}

fn claim() -> Value {
    json!({
        "id": "claim-retry-limit",
        "statement": "The retry limit is five",
        "evidence": [{"resource":"repo://src/payments/retry.ts#L42-L48", "version":"repo-lines-v1:sha256:abc:opaque-context"}]
    })
}

fn sidecar(root: &Path, verified: bool) -> Value {
    let mut sidecar = json!({
        "schemaVersion": 1,
        "pageVersion": format!("sha256:{:x}", Sha256::digest(fs::read(root.join("payments/retries.md")).unwrap())),
        "claims": [claim()]
    });
    if verified {
        sidecar["verification"] = json!({ "by":"openwiki/0.7.1", "at":"2026-10-01T12:00:00Z" });
    }
    sidecar
}

fn engram_observation(id: i64, sync_id: &str, session: &str, project: Option<&str>) -> Value {
    json!({
        "id": id, "sync_id": sync_id, "session_id": session,
        "title":"Retry discovery", "content":"Changing retries needs [policy review](docs/adr-017.md).",
        "type":"discovery", "project":project, "scope":"project", "topic_key":"payment-retry-policy",
        "created_at":"2026-09-30 12:00:00", "updated_at":"2026-10-01 12:00:00", "revision_count":2
    })
}

#[test]
fn imports_real_openwiki_shape_and_preserves_opaque_versions_without_copying_the_page() {
    let directory = tempfile::tempdir().unwrap();
    wiki(directory.path());
    let path = directory.path().join(".claims/payments/retries.json");
    write_json(&path, &sidecar(directory.path(), true));
    let before = fs::read(&path).unwrap();
    let mut config = source(ImportKind::Openwiki);
    config.project = Some("payments".into());
    let batch = adapters::read(&config, directory.path(), LIMIT).unwrap();
    assert_eq!(batch.format, "openwiki-okf/0.2+claims/1");
    assert_eq!(batch.records.len(), 1);
    let record = &batch.records[0];
    assert_eq!(record.native_id, "claim-retry-limit");
    assert_eq!(record.native_record, claim());
    assert_eq!(record.kind, ObservationKind::Implementation);
    assert_eq!(
        record.verification,
        ObservationVerification::UpstreamVerifiedAtRevision
    );
    assert_eq!(record.scope.repository.as_deref(), Some("payments"));
    assert_eq!(
        record.scope.component.as_deref(),
        Some("payment-processing")
    );
    assert_eq!(record.scope.environment.as_deref(), Some("production"));
    assert_eq!(record.subject, "payment-retry-policy");
    assert_eq!(
        record.evidence[1].revision.as_deref(),
        Some("repo-lines-v1:sha256:abc:opaque-context")
    );
    assert_eq!(record.metadata["current_checkout_verified"], false);
    assert!(record.links.contains(&"../../docs/adr-017.md".to_owned()));
    assert!(
        record
            .links
            .contains(&"repo://src/payments/retry.ts".to_owned())
    );
    for evidence in &record.evidence {
        assert!(
            record
                .native_record
                .pointer(evidence.field.as_deref().unwrap())
                .is_some()
        );
    }
    assert_eq!(
        batch,
        adapters::read(&config, directory.path(), LIMIT).unwrap()
    );
    assert_eq!(before, fs::read(&path).unwrap());
}

#[test]
fn old_claims_without_durable_verification_remain_reported_even_with_verified_markdown() {
    let directory = tempfile::tempdir().unwrap();
    wiki(directory.path());
    write_json(
        &directory.path().join(".claims/payments/retries.json"),
        &sidecar(directory.path(), false),
    );
    let batch = adapters::read(&source(ImportKind::Openwiki), directory.path(), LIMIT).unwrap();
    assert_eq!(
        batch.records[0].verification,
        ObservationVerification::Reported
    );
    assert!(
        batch
            .warnings
            .iter()
            .any(|warning| warning.contains("no durable Claims verification"))
    );
}

#[test]
fn okf_without_sidecars_stays_documentary_with_exact_markdown_evidence() {
    let directory = tempfile::tempdir().unwrap();
    wiki(directory.path());
    let batch = adapters::read(&source(ImportKind::Openwiki), directory.path(), LIMIT).unwrap();
    assert_eq!(batch.records.len(), 1);
    let record = &batch.records[0];
    assert_eq!(record.native_id, "page:payments/retries.md");
    assert_eq!(record.kind, ObservationKind::Documentation);
    assert_eq!(record.verification, ObservationVerification::Documentary);
    assert_eq!(
        record.native_record["markdown"],
        fs::read_to_string(directory.path().join("payments/retries.md")).unwrap()
    );
}

#[test]
fn rejects_unknown_openwiki_versions_invalid_claims_and_incomplete_snapshots() {
    let directory = tempfile::tempdir().unwrap();
    wiki(directory.path());
    let path = directory.path().join(".claims/payments/retries.json");
    let config = source(ImportKind::Openwiki);
    let mut invalid = sidecar(directory.path(), true);
    invalid["schemaVersion"] = json!(2);
    write_json(&path, &invalid);
    assert!(
        format!(
            "{:#}",
            adapters::read(&config, directory.path(), LIMIT).unwrap_err()
        )
        .contains("schemaVersion")
    );
    invalid = sidecar(directory.path(), true);
    invalid["claims"][0]["evidence"] = json!([]);
    write_json(&path, &invalid);
    assert!(adapters::read(&config, directory.path(), LIMIT).is_err());
    invalid = sidecar(directory.path(), true);
    invalid["claims"] = json!([claim(), claim()]);
    write_json(&path, &invalid);
    assert!(
        format!(
            "{:#}",
            adapters::read(&config, directory.path(), LIMIT).unwrap_err()
        )
        .contains("duplicate native record")
    );
    write_json(&path, &sidecar(directory.path(), true));
    fs::remove_file(directory.path().join("payments/retries.md")).unwrap();
    assert!(
        format!(
            "{:#}",
            adapters::read(&config, directory.path(), LIMIT).unwrap_err()
        )
        .contains("no corresponding Markdown page")
    );
    fs::write(
        directory.path().join("index.md"),
        "---\nokf_version: \"0.9\"\n---\n",
    )
    .unwrap();
    assert!(
        format!(
            "{:#}",
            adapters::read(&config, directory.path(), LIMIT).unwrap_err()
        )
        .contains("unsupported OpenWiki OKF version")
    );
}

#[test]
fn empty_claims_set_does_not_turn_page_prose_into_code_evidence() {
    let directory = tempfile::tempdir().unwrap();
    wiki(directory.path());
    let mut empty = sidecar(directory.path(), false);
    empty["claims"] = json!([]);
    write_json(
        &directory.path().join(".claims/payments/retries.json"),
        &empty,
    );
    let batch = adapters::read(&source(ImportKind::Openwiki), directory.path(), LIMIT).unwrap();
    assert!(batch.records.is_empty());
}

#[test]
fn modified_page_scope_cannot_qualify_claims_verified_for_a_previous_page() {
    let directory = tempfile::tempdir().unwrap();
    wiki(directory.path());
    write_json(
        &directory.path().join(".claims/payments/retries.json"),
        &sidecar(directory.path(), true),
    );
    let page = directory.path().join("payments/retries.md");
    let modified = fs::read_to_string(&page)
        .unwrap()
        .replace("environment: production", "environment: staging");
    fs::write(page, modified).unwrap();
    let error = adapters::read(&source(ImportKind::Openwiki), directory.path(), LIMIT).unwrap_err();
    assert!(format!("{error:#}").contains("pageVersion mismatch"));
}

#[test]
fn engram_project_filter_honors_explicit_ownership_before_session_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("engram.json");
    let export = json!({
        "version":"0.2.0", "exported_at":"2026-10-01T12:00:00Z",
        "sessions":[{"id":"s-payments","project":"payments"},{"id":"s-other","project":"other"}],
        "observations":[
            engram_observation(1,"obs-own","s-other",Some("payments")),
            engram_observation(2,"obs-inherited","s-payments",None),
            engram_observation(3,"obs-other","s-payments",Some("other")),
            engram_observation(4,"obs-unscoped","unknown",None)
        ],
        "prompts":[{"content":"Do not import prompts as curated memories"}]
    });
    write_json(&path, &export);
    let mut config = source(ImportKind::Engram);
    config.project = Some("payments".into());
    let batch = adapters::read(&config, &path, LIMIT).unwrap();
    assert_eq!(
        batch
            .records
            .iter()
            .map(|record| record.native_id.as_str())
            .collect::<Vec<_>>(),
        vec!["obs-inherited", "obs-own"]
    );
    for record in &batch.records {
        assert_eq!(record.kind, ObservationKind::Recollection);
        assert_eq!(record.verification, ObservationVerification::Reported);
        assert_eq!(record.subject, "payment-retry-policy");
        assert_eq!(record.scope.repository.as_deref(), Some("payments"));
        assert_eq!(
            record.native_record.pointer("/content").unwrap(),
            &json!(record.statement)
        );
    }
}

#[test]
fn engram_timestamp_only_reexport_is_identical_and_native_relationship_judgments_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("engram.json");
    let relation = json!({
        "sync_id":"rel-new-old", "source_id":"obs-new", "target_id":"obs-old", "relation":"supersedes",
        "judgment_status":"judged", "marked_by_actor":"agent:example", "marked_by_kind":"agent",
        "created_at":"2026-10-01 12:00:00", "updated_at":"2026-10-01 12:00:00"
    });
    let pending = json!({"sync_id":"rel-pending", "source_id":"obs-old", "target_id":"obs-new", "relation":"pending", "judgment_status":"pending"});
    let mut export = json!({"version":"0.2.0", "exported_at":"first", "observations":[engram_observation(1,"obs-new","s",None),engram_observation(2,"obs-old","s",None)], "relations":[relation,pending]});
    write_json(&path, &export);
    let config = source(ImportKind::Engram);
    let first = adapters::read(&config, &path, LIMIT).unwrap();
    export["exported_at"] = json!("second");
    write_json(&path, &export);
    let second = adapters::read(&config, &path, LIMIT).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.records[0].relationships[0].native_record, relation);
    assert!(first.records[0].relationships[0].active);
    assert!(!first.records[1].relationships[0].active);
    assert!(
        first
            .records
            .iter()
            .all(|record| record.verification == ObservationVerification::Reported)
    );
    assert_eq!(
        util::json_digest(&first).unwrap(),
        util::json_digest(&second).unwrap()
    );
}

#[test]
fn engram_legacy_numeric_identity_survives_content_edits_and_tombstones() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("engram.json");
    let mut export = json!({"version":"0.1.0", "sessions":null, "observations":[{"id":17,"title":"Old title","content":"First content"}], "prompts":null});
    write_json(&path, &export);
    let config = source(ImportKind::Engram);
    let first = adapters::read(&config, &path, LIMIT).unwrap();
    export["observations"][0]["title"] = json!("New title");
    export["observations"][0]["content"] = json!("Second content");
    export["observations"][0]["deleted_at"] = json!("2026-10-01 13:00:00");
    write_json(&path, &export);
    let second = adapters::read(&config, &path, LIMIT).unwrap();
    assert_eq!(first.records[0].native_id, "observation:17");
    assert_eq!(first.records[0].native_id, second.records[0].native_id);
    assert_eq!(second.records[0].lifecycle, "deleted");
    assert_ne!(
        first.records[0].native_record,
        second.records[0].native_record
    );
}

#[test]
fn engram_rejects_unknown_versions_duplicate_identity_and_missing_observations() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("engram.json");
    let config = source(ImportKind::Engram);
    for invalid in [
        json!({"version":"0.3.0","observations":[]}),
        json!({"version":"0.2.0"}),
        json!({"version":"0.2.0","observations":[engram_observation(1,"obs-duplicate","s",None),engram_observation(2,"obs-duplicate","s",None)]}),
    ] {
        write_json(&path, &invalid);
        assert!(adapters::read(&config, &path, LIMIT).is_err());
    }
}

#[test]
fn beads_keeps_closed_decisions_as_work_state_and_preserves_dependencies_comments_and_raw_record() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("beads.jsonl");
    let issue = json!({
        "_type":"issue", "id":"pay-42", "title":"Increase retry limit", "status":"closed", "issue_type":"decision", "priority":1,
        "description":"Retry limit should be five.", "notes":"Implementation was reported complete.", "close_reason":"Merged change",
        "updated_at":"2026-10-01T13:00:00Z", "labels":["payments"], "external_ref":"gh-42",
        "dependencies":[{"issue_id":"pay-42","depends_on_id":"pay-41","type":"blocks","created_at":"2026-10-01T12:00:00Z"}],
        "comments":[{"id":7,"issue_id":"pay-42","author":"developer","text":"Review ADR-017 before release.","created_at":"2026-10-01T12:00:00Z"}],
        "metadata":{"component":"payment-processing","environment":"production"}
    });
    write_jsonl(&path, std::slice::from_ref(&issue));
    let before = fs::read(&path).unwrap();
    let batch = adapters::read(&source(ImportKind::Beads), &path, LIMIT).unwrap();
    let record = &batch.records[0];
    assert_eq!(record.kind, ObservationKind::WorkState);
    assert_eq!(record.lifecycle, "closed");
    assert_eq!(record.verification, ObservationVerification::Reported);
    assert_eq!(record.native_record, issue);
    assert_eq!(record.evidence[0].field.as_deref(), Some(""));
    assert_eq!(record.relationships[0].target_native_id, "pay-41");
    assert_eq!(record.relationships[0].kind, "blocks");
    assert!(record.statement.contains("Review ADR-017 before release."));
    assert!(record.links.contains(&"gh-42".into()));
    assert_eq!(before, fs::read(&path).unwrap());
}

#[test]
fn beads_memory_import_requires_explicit_opt_in() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("beads.jsonl");
    write_jsonl(
        &path,
        &[
            json!({"_type":"issue","id":"pay-1","title":"Normal work","status":"open"}),
            json!({"_type":"memory","key":"agent_context","value":"Remember the migration"}),
            json!({"id":"legacy-1","title":"Older issue","status":"closed"}),
        ],
    );
    let mut config = source(ImportKind::Beads);
    let defaults = adapters::read(&config, &path, LIMIT).unwrap();
    assert_eq!(defaults.records.len(), 2);
    assert!(
        defaults
            .warnings
            .iter()
            .any(|warning| warning.contains("Excluded 1 Beads memory"))
    );
    config.include_memories = true;
    let opted_in = adapters::read(&config, &path, LIMIT).unwrap();
    assert_eq!(opted_in.records.len(), 3);
    let memory = opted_in
        .records
        .iter()
        .find(|record| record.native_id == "memory:agent_context")
        .unwrap();
    assert_eq!(memory.kind, ObservationKind::Recollection);
    assert_eq!(memory.verification, ObservationVerification::Reported);
}

#[test]
fn beads_rejects_unknown_record_types_malformed_lines_and_version_envelopes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("beads.jsonl");
    let config = source(ImportKind::Beads);
    for text in [
        "{\"_type\":\"transaction\",\"id\":\"a\",\"title\":\"A\"}\n",
        "{\"schema_version\":2,\"issues\":[]}\n",
        "{\"id\":\"a\",\"title\":\"A\"}\n{invalid}\n",
    ] {
        fs::write(&path, text).unwrap();
        assert!(adapters::read(&config, &path, LIMIT).is_err());
    }
}

#[test]
fn missing_and_oversized_input_fail_instead_of_becoming_an_empty_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("snapshot.json");
    assert!(adapters::read(&source(ImportKind::Engram), &path, LIMIT).is_err());
    write_json(&path, &json!({"version":"0.2.0","observations":[]}));
    assert!(adapters::read(&source(ImportKind::Engram), &path, 10).is_err());
}

#[test]
fn bundled_cross_project_evaluation_snapshots_have_resolvable_native_evidence() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("evaluation/corpora/cross-source");
    for project in ["payments", "ledger", "releases"] {
        let root = corpus.join(project).join("initial");
        for (kind, relative) in [
            (ImportKind::Openwiki, "openwiki"),
            (ImportKind::Engram, "imports/engram.json"),
            (ImportKind::Beads, "imports/beads.jsonl"),
        ] {
            let mut config = source(kind);
            if kind == ImportKind::Engram {
                config.project = Some(project.into());
            }
            let batch = adapters::read(&config, &root.join(relative), LIMIT).unwrap();
            assert!(!batch.records.is_empty(), "{project}: {relative}");
            for record in batch.records {
                lore::imports::validate_record(&record).unwrap();
                assert!(!record.native_id.starts_with("memory:"));
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_inputs_and_descendants_without_reading_outside_the_snapshot() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.json");
    write_json(&target, &json!({"version":"0.2.0","observations":[]}));
    let link = directory.path().join("link.json");
    symlink(&target, &link).unwrap();
    assert!(adapters::read(&source(ImportKind::Engram), &link, LIMIT).is_err());
    let root = directory.path().join("openwiki");
    wiki(&root);
    symlink(&target, root.join(".claims/payments/unsafe.json")).unwrap();
    assert!(
        format!(
            "{:#}",
            adapters::read(&source(ImportKind::Openwiki), &root, LIMIT).unwrap_err()
        )
        .contains("symlink")
    );
}
