use lore::{
    config::{Config, ImportSource, ResolvedConfig},
    context::{
        self, ContextOptions,
        freshness::{FreshnessReason as Reason, FreshnessStatus as Status},
        imports::ContextImportedEvidence,
        inspection::{InspectionSession, InspectionSettings},
    },
    domain::ImportKind,
    imports::{self, Inventory, SourceBatch, adapters},
    storage, util,
};
use rusqlite::Connection;
use std::{fs, path::Path};

const SOURCE: &[u8] = include_bytes!("fixtures/openwiki-freshness/src/worker.rs");

fn fixture() -> (
    tempfile::TempDir,
    ResolvedConfig,
    Connection,
    Vec<ContextImportedEvidence>,
) {
    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    fs::create_dir(temp.path().join("docs")).unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src/worker.rs"), SOURCE).unwrap();
    let path = temp.path().join("lore.yml");
    fs::write(&path, serde_yaml::to_string(&Config::default()).unwrap()).unwrap();
    let config = ResolvedConfig::load(&path).unwrap();
    let conn = Connection::open_in_memory().unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES(?1,'Freshness fixture','2026-10-14')",
        [&config.project_id],
    )
    .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/openwiki-freshness");
    let batch = adapters::read(
        &ImportSource {
            id: "wiki".into(),
            kind: ImportKind::Openwiki,
            path: root.clone(),
            project: None,
            include_memories: false,
        },
        &root,
        65536,
    )
    .unwrap();
    imports::storage::persist(
        &conn,
        &config.project_id,
        &Inventory {
            sources: vec![SourceBatch {
                id: "wiki".into(),
                kind: ImportKind::Openwiki,
                path: root,
                digest: util::json_digest(&batch).unwrap(),
                batch,
            }],
            ..Inventory::default()
        },
    )
    .unwrap();
    let selected = context::build_context(
        &conn,
        &ContextOptions {
            task: "Adjust worker queue".into(),
            paths: vec![],
            max_tokens: 6000,
        },
    )
    .unwrap();
    assert_eq!(selected.imported_evidence.len(), 1);
    (temp, config, conn, selected.imported_evidence)
}

fn settings() -> InspectionSettings {
    InspectionSettings {
        enabled: true,
        root: Some(".".into()),
        max_elapsed_ms: 10_000,
        ..InspectionSettings::default()
    }
}

fn session(config: &ResolvedConfig, settings: &InspectionSettings) -> InspectionSession {
    InspectionSession::discover(config, "worker queue", &[], &[], settings).unwrap()
}

#[test]
fn pinned_whole_file_checks_are_ephemeral_and_rehash_a_b_a() {
    let (_temp, config, conn, evidence) = fixture();
    let before = imports::evidence(&conn, &evidence[0].id).unwrap();
    let mut s = session(&config, &settings());
    s.check_openwiki(&evidence);
    let first = s.report();
    let check = &first.checkout_freshness[0];
    assert_eq!(check.status, Status::MatchesCheckout);
    assert_eq!(check.reason, None);
    assert_eq!(check.checked_path.as_deref(), Some("src/worker.rs"));
    assert_eq!(
        check.content_hash.as_deref(),
        evidence[0].locators[1]
            .revision
            .as_deref()
            .map(|v| v.strip_prefix("repo-file-v1:").unwrap())
    );
    assert_eq!(first.budget.bytes_read, SOURCE.len());
    assert!(check.qualification.contains("no test execution"));
    let path = config.base.join("src/worker.rs");
    let mtime = fs::metadata(&path).unwrap().modified().unwrap();
    fs::write(
        &path,
        String::from_utf8(SOURCE.to_vec())
            .unwrap()
            .replace("= 1", "= 2"),
    )
    .unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(mtime))
        .unwrap();
    assert!(!s.revalidate(&first.observations).unwrap());
    assert_eq!(
        s.report().checkout_freshness[0].status,
        Status::ChangedSinceSnapshot
    );
    assert_eq!(
        s.report().checkout_freshness[0].reason,
        Some(Reason::ContentChanged)
    );
    fs::write(&path, SOURCE).unwrap();
    assert!(s.revalidate(&first.observations).unwrap());
    assert_eq!(
        s.report().checkout_freshness[0].status,
        Status::MatchesCheckout
    );
    assert_eq!(s.report().budget.bytes_read, SOURCE.len() * 3);
    fs::remove_file(&path).unwrap();
    assert!(!s.revalidate(&first.observations).unwrap());
    assert_eq!(
        s.report().checkout_freshness[0].status,
        Status::UnknownOrUnchecked
    );
    assert!(s.report().observations.is_empty());
    let mut fresh = session(&config, &settings());
    fresh.check_openwiki(&evidence);
    assert_eq!(
        fresh.report().checkout_freshness[0].reason,
        Some(Reason::PathUnavailableOrExcluded)
    );
    let after = imports::evidence(&conn, &evidence[0].id).unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_eq!(evidence[0].metadata["current_checkout_verified"], false);
}

#[test]
fn unsupported_and_unsafe_inputs_do_not_read_source() {
    let (_temp, config, _conn, evidence) = fixture();
    for (resource, version, reason) in [
        (
            "repo://src/worker.rs#L1-L1",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::UnsupportedLocator,
        ),
        (
            "repo://src/worker.rs",
            "repo-lines-v1:sha256:abc:opaque".into(),
            Reason::UnsupportedVersion,
        ),
        (
            "repo://src/worker.rs",
            "123456789abcdef".into(),
            Reason::UnsupportedVersion,
        ),
        (
            "repo://src/worker.rs",
            format!("repo-file-v1:sha256:{}", "A".repeat(64)),
            Reason::UnsupportedVersion,
        ),
        (
            "https://example.com/src/worker.rs",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::UnsupportedLocator,
        ),
        (
            "repo://../src/worker.rs",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::InvalidPath,
        ),
        (
            "repo://%2e%2e/src/worker.rs",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::InvalidPath,
        ),
        (
            "repo:///src/worker.rs",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::InvalidPath,
        ),
        (
            "repo://src%5cworker.rs",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::InvalidPath,
        ),
        (
            "repo://src/%00worker.rs",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::InvalidPath,
        ),
        (
            "repo://src/%zzworker.rs",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::InvalidPath,
        ),
        (
            "repo://.env",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::PathUnavailableOrExcluded,
        ),
        (
            "repo://secrets/worker.rs",
            evidence[0].locators[1].revision.clone().unwrap(),
            Reason::PathUnavailableOrExcluded,
        ),
    ] {
        let mut altered = evidence.clone();
        altered[0].locators[1].locator = resource.into();
        altered[0].locators[1].revision = Some(version);
        let mut s = session(&config, &settings());
        s.check_openwiki(&altered);
        let report = s.report();
        assert_eq!(
            report.checkout_freshness[0].status,
            Status::UnknownOrUnchecked,
            "{resource}"
        );
        assert_eq!(
            report.checkout_freshness[0].reason,
            Some(reason),
            "{resource}"
        );
        assert_eq!(report.budget.bytes_read, 0, "{resource}");
    }
    fs::write(config.base.join("src/worker space.rs"), SOURCE).unwrap();
    let mut encoded = evidence.clone();
    encoded[0].locators[1].locator = "repo://src/worker%20space.rs".into();
    let mut s = session(&config, &settings());
    s.check_openwiki(&encoded);
    assert_eq!(
        s.report().checkout_freshness[0].checked_path.as_deref(),
        Some("src/worker space.rs")
    );
}

#[test]
fn permission_exclusion_and_all_io_budgets_fail_closed() {
    let (_temp, mut config, _conn, evidence) = fixture();
    let mut denied = session(&config, &InspectionSettings::default());
    denied.check_openwiki(&evidence);
    assert_eq!(
        denied.report().checkout_freshness[0].reason,
        Some(Reason::InspectionNotAuthorized)
    );
    assert_eq!(denied.report().budget.index_entries, 0);
    assert_eq!(denied.report().budget.bytes_read, 0);
    let mut unavailable_config = config.clone();
    unavailable_config.base = config.base.ancestors().last().unwrap().to_owned();
    let mut unavailable = session(
        &unavailable_config,
        &InspectionSettings {
            root: None,
            ..settings()
        },
    );
    unavailable.check_openwiki(&evidence);
    assert_eq!(
        unavailable.report().checkout_freshness[0].reason,
        Some(Reason::CheckoutUnavailable)
    );
    config.config.sources.exclude.push("src/**".into());
    let mut excluded = session(&config, &settings());
    excluded.check_openwiki(&evidence);
    assert_eq!(
        excluded.report().checkout_freshness[0].reason,
        Some(Reason::PathUnavailableOrExcluded)
    );
    assert_eq!(excluded.report().budget.bytes_read, 0);
    config.config.sources.exclude.pop();
    let mut small = session(
        &config,
        &InspectionSettings {
            max_file_bytes: 10,
            ..settings()
        },
    );
    small.check_openwiki(&evidence);
    assert_eq!(
        small.report().checkout_freshness[0].status,
        Status::UnknownOrUnchecked
    );
    let mut bytes = session(
        &config,
        &InspectionSettings {
            max_file_bytes: SOURCE.len(),
            max_total_bytes: SOURCE.len(),
            ..settings()
        },
    );
    bytes.inspect("src/worker.rs").unwrap();
    bytes.check_openwiki(&evidence);
    assert_eq!(
        bytes.report().checkout_freshness[0].reason,
        Some(Reason::BudgetExhausted)
    );
    fs::write(config.base.join("src/other.rs"), SOURCE).unwrap();
    let mut files = session(
        &config,
        &InspectionSettings {
            max_files: 1,
            ..settings()
        },
    );
    files.inspect("src/other.rs").unwrap();
    files.check_openwiki(&evidence);
    assert_eq!(
        files.report().checkout_freshness[0].reason,
        Some(Reason::BudgetExhausted)
    );
    let mut time = session(
        &config,
        &InspectionSettings {
            max_elapsed_ms: 1,
            ..settings()
        },
    );
    // Inspection time is cumulative I/O, not time spent between calls.
    for _ in 0..1000 {
        time.check_openwiki(&evidence);
        if time
            .report()
            .budget
            .exhausted
            .contains(&"elapsed_ms".into())
        {
            break;
        }
    }
    assert_eq!(
        time.report().checkout_freshness[0].reason,
        Some(Reason::BudgetExhausted)
    );
    let mut index = session(
        &config,
        &InspectionSettings {
            max_index_entries: 1,
            ..settings()
        },
    );
    index.check_openwiki(&evidence);
    assert_eq!(
        index.report().checkout_freshness[0].reason,
        Some(Reason::BudgetExhausted)
    );
}

#[cfg(unix)]
#[test]
fn symlinks_concurrent_replacement_and_tampered_observations_cannot_certify_a_match() {
    use std::{os::unix::fs::symlink, time::Duration};
    let (_temp, config, _conn, evidence) = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("worker.rs"), "OUTSIDE_SECRET_SENTINEL").unwrap();
    let mut s = session(&config, &settings());
    s.check_openwiki(&evidence);
    let initial = s.report().observations;
    let mut corrupt = initial.clone();
    corrupt[0].content_hash = format!("sha256:{}", "0".repeat(64));
    assert!(!s.revalidate(&corrupt).unwrap());
    fs::rename(config.base.join("src"), config.base.join("original-src")).unwrap();
    symlink(outside.path(), config.base.join("src")).unwrap();
    assert!(!s.revalidate(&initial).unwrap());
    assert_eq!(
        s.report().checkout_freshness[0].status,
        Status::UnknownOrUnchecked
    );
    assert!(s.report().observations.is_empty());
    fs::remove_file(config.base.join("src")).unwrap();
    fs::rename(config.base.join("original-src"), config.base.join("src")).unwrap();
    let mut concurrent = session(&config, &settings());
    let path = config.base.join("src/worker.rs");
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            for _ in 0..100 {
                fs::remove_file(&path).unwrap();
                symlink(outside.path().join("worker.rs"), &path).unwrap();
                std::thread::sleep(Duration::from_micros(50));
                fs::remove_file(&path).unwrap();
                fs::write(&path, SOURCE).unwrap();
            }
        });
        for _ in 0..100 {
            concurrent.check_openwiki(&evidence);
            let report = concurrent.report();
            assert!(
                report
                    .observations
                    .iter()
                    .all(|o| !o.excerpt.contains("OUTSIDE_SECRET_SENTINEL"))
            );
            if report.checkout_freshness[0].status == Status::MatchesCheckout {
                assert_eq!(report.observations[0].excerpt.as_bytes(), SOURCE);
            }
        }
        writer.join().unwrap();
    });
}

#[test]
fn native_report_alone_cannot_assert_current_implementation_even_when_bytes_match() {
    let (_temp, config, conn, evidence) = fixture();
    let selected = context::build_context(
        &conn,
        &ContextOptions {
            task: "Adjust worker queue".into(),
            paths: vec![],
            max_tokens: 6000,
        },
    )
    .unwrap();
    let id = &evidence[0].id;
    let advice =
        |text: &str| serde_json::json!({"text":text,"evidence_ids":[id],"observation_ids":[]});
    let mut draft = serde_json::json!({
        "readiness":"proceed", "change_kind":"behavior_preserving",
        "preferred_approach":advice("Adjust the worker queue while preserving documented behavior."),
        "rationale":advice("A narrow worker change limits the implementation scope."),
        "main_tradeoff":advice("Preserving the queue limits this task's scope."),
        "next_action":advice("Change the worker queue configuration."),
        "completion_criteria":[advice("Check the worker queue behavior during implementation.")],
        "known_record_ids":[selected.imported_observations[0].id],
        "hypotheses":[{"provenance":"hypothesis", "text":"The worker implementation currently uses one queue.",
            "evidence_ids":[id],"observation_ids":[],"confidence":"medium",
            "applicability":"The current worker implementation.","historical_only":false,
            "alternatives":[advice("The checkout may have changed since the report.")]}],
        "heuristics":[],"constraints":[],"checks":[],"implementation_seams":[],"risks":[],"material_blockers":[],"counterevidence":[],"remaining_uncertainty":[]
    });
    let mut s = session(&config, &settings());
    s.check_openwiki(&evidence);
    assert_eq!(
        s.report().checkout_freshness[0].status,
        Status::MatchesCheckout
    );
    let parsed = serde_json::from_value(draft.clone()).unwrap();
    let error = context::decision::validate_decision(&conn, &selected, &[], &parsed, "fixture")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("historical evidence cannot establish current applicability")
    );
    draft["hypotheses"][0]["historical_only"] = true.into();
    draft["hypotheses"][0]["text"] =
        "The worker report described one queue at its recorded revision.".into();
    draft["hypotheses"][0]["applicability"] = "The imported snapshot only.".into();
    let parsed = serde_json::from_value(draft).unwrap();
    let brief =
        context::decision::validate_decision(&conn, &selected, &[], &parsed, "fixture").unwrap();
    assert_eq!(
        brief.facts[0].provenance,
        context::decision::FactProvenance::ImportedReport
    );
    assert_eq!(brief.facts[0].record.basis, "observed");
}
