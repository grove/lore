//! Manual interpretations of exact candidates, separate from documentary truth.
use super::{ReviewActor, events, fingerprint, list, transition};
use crate::{storage, util};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub struct Request<'a> {
    pub decision: &'a str,
    pub relation: Option<&'a str>,
    pub evidence: Option<&'a str>,
    pub reason: &'a str,
    pub actor: &'a str,
    pub actor_kind: &'a str,
    pub expect_digest: Option<&'a str>,
}

fn available(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='review_adjudication_events')",
        [],
        |r| r.get(0),
    )?)
}

pub fn history(conn: &Connection, id: &str) -> Result<Vec<Value>> {
    if !available(conn)? {
        return Ok(vec![]);
    }
    let mut q = conn.prepare(
        "SELECT payload_json FROM review_adjudication_events WHERE review_id=?1 ORDER BY sequence",
    )?;
    let rows = q
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.iter()
        .map(|row| Ok(serde_json::from_str(row)?))
        .collect()
}

fn binding(conn: &Connection, id: &str) -> Result<Value> {
    let item = list(conn, true)?
        .into_iter()
        .find(|r| r.id == id)
        .context("review ID not found")?;
    ensure!(
        matches!(
            item.category.as_deref(),
            Some("relationship" | "replacement" | "reaffirmation" | "conflict")
        ),
        "review does not bind one typed relation candidate"
    );
    let assertion = item
        .assertion_revision_id
        .context("ambiguous assertion binding")?;
    let target = item.target_unit_id.context("ambiguous target binding")?;
    let active: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM active_assertions WHERE assertion_revision_id=?1)",
        [&assertion],
        |r| r.get(0),
    )?;
    ensure!(active, "candidate assertion is no longer current");
    let target_revision: String = conn
        .query_row(
            "SELECT revision_id FROM knowledge_current WHERE knowledge_id=?1",
            [&target],
            |r| r.get(0),
        )
        .optional()?
        .context("target is obsolete")?;
    let target_current: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_support s JOIN active_assertions a ON a.assertion_revision_id=s.assertion_revision_id WHERE s.knowledge_revision_id=?1)", [&target_revision], |r| r.get(0))?;
    ensure!(target_current, "target has no current support");
    let raw: String = conn.query_row("SELECT operation_json FROM reconciliation_log WHERE assertion_revision_id=?1 ORDER BY rowid DESC LIMIT 1", [&assertion], |r| r.get(0)).optional()?.context("candidate proposal is unavailable")?;
    let proposal: Value = serde_json::from_str(&raw)?;
    let candidates: Vec<_> = proposal["relations"]
        .as_array()
        .context("candidate relations are unavailable")?
        .iter()
        .filter(|r| r["target_id"].as_str() == Some(&target))
        .collect();
    ensure!(
        candidates.len() == 1,
        "review requires exactly one unambiguous candidate relation"
    );
    let candidate = candidates[0];
    let kind = candidate["kind"]
        .as_str()
        .context("candidate relation has no kind")?;
    ensure!(
        matches!(
            kind,
            "uncertain" | "contradicts" | "supersedes" | "reaffirms"
        ),
        "unsupported candidate relation"
    );
    ensure!(
        matches!(
            (item.category.as_deref(), kind),
            (Some("relationship"), "uncertain")
                | (Some("replacement"), "supersedes")
                | (Some("reaffirmation"), "reaffirms")
                | (Some("conflict"), "contradicts")
        ),
        "candidate kind does not match its review category"
    );
    let (source, source_revision): (String, String) = conn.query_row(
        "SELECT source_id,source_revision_id FROM assertion_revisions WHERE id=?1",
        [&assertion],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut heads = storage::source_heads(conn)?;
    heads.sort_by(|a, b| a.id.cmp(&b.id));
    let mut q = conn.prepare("SELECT DISTINCT k.knowledge_id FROM knowledge_support s JOIN knowledge_current k ON k.revision_id=s.knowledge_revision_id WHERE s.assertion_revision_id=?1 ORDER BY k.knowledge_id")?;
    let units = q
        .query_map([&assertion], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut q = conn.prepare("SELECT evidence_id FROM assertion_evidence WHERE assertion_revision_id=?1 UNION SELECT ae.evidence_id FROM assertion_evidence ae JOIN knowledge_support ks ON ks.assertion_revision_id=ae.assertion_revision_id WHERE ks.knowledge_revision_id=?2 ORDER BY 1")?;
    let evidence = q
        .query_map(params![assertion, target_revision], |r| {
            r.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(
        json!({"review_id":id,"category":item.category,"assertion_revision_id":assertion,
        "target_unit_id":target,"target_revision_id":target_revision,"source_id":source,
        "source_revision_id":source_revision,"candidate":candidate,"source_units":units,"evidence_ids":evidence,
        "registry_fingerprint":storage::documentary_revision(conn)?,"source_fingerprint":util::json_digest(&heads)?,"review_fingerprint":fingerprint(conn,id)?}),
    )
}

fn current(conn: &Connection) -> Result<Vec<Value>> {
    if !available(conn)? {
        return Ok(vec![]);
    }
    let mut q = conn.prepare("SELECT payload_json FROM review_adjudications ORDER BY review_id")?;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.iter()
        .map(|row| Ok(serde_json::from_str(row)?))
        .collect()
}

pub fn projection(conn: &Connection) -> Result<Vec<Value>> {
    Ok(current(conn)?
        .into_iter()
        .filter(|event| {
            event["review_id"]
                .as_str()
                .and_then(|id| binding(conn, id).ok())
                .is_some_and(|now| now == event["binding"])
        })
        .collect())
}

pub fn annotations(conn: &Connection) -> Result<BTreeMap<String, Vec<String>>> {
    let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for event in projection(conn)? {
        let text = format!(
            "manual_adjudication {}: {} by {} ({}; identity not authenticated). Rationale: {}. Authority: {}. Original documentary assertions and relationships are unchanged.",
            event["review_id"].as_str().unwrap_or_default(),
            event["decision"].as_str().unwrap_or_default(),
            event["actor"].as_str().unwrap_or_default(),
            event["actor_kind"].as_str().unwrap_or_default(),
            event["reason"].as_str().unwrap_or_default(),
            event["authority"].as_str().unwrap_or_default()
        );
        let mut units = event["binding"]["source_units"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        units.push(event["binding"]["target_unit_id"].clone());
        for id in units.iter().filter_map(Value::as_str) {
            let entries = result.entry(id.into()).or_default();
            if !entries.contains(&text) {
                entries.push(text.clone());
            }
        }
    }
    Ok(result)
}

pub fn decide(conn: &Connection, id: &str, request: &Request<'_>, apply: bool) -> Result<Value> {
    ensure!(
        matches!(
            request.decision,
            "keep_separate" | "reject_relation" | "accept_relation"
        ),
        "invalid adjudication decision"
    );
    ensure!(
        !request.reason.trim().is_empty()
            && request.reason.len() <= 4000
            && !request.reason.contains('\0'),
        "reason must contain 1..4000 bytes"
    );
    ensure!(
        !request.actor.trim().is_empty()
            && request.actor.len() <= 120
            && !request.actor.chars().any(char::is_control),
        "invalid actor attribution"
    );
    ensure!(
        matches!(request.actor_kind, "user" | "agent"),
        "actor kind must be user or agent; neither authenticates identity"
    );
    let bound = binding(conn, id)?;
    let previous = history(conn, id)?;
    let review_events = events(conn, id)?;
    let context_digest = util::json_digest(&(
        &bound,
        review_events.last().map(|e| e.sequence),
        previous.last(),
    ))?;
    let mut authority = "human_advisory_interpretation";
    if request.decision == "accept_relation" {
        let relation = request
            .relation
            .context("accept_relation requires --relation")?;
        ensure!(
            matches!(relation, "supersedes" | "reaffirms" | "contradicts"),
            "unsupported relation kind"
        );
        ensure!(
            bound["category"] == "relationship" || bound["candidate"]["kind"] == relation,
            "relation does not match the review category"
        );
        let evidence = request
            .evidence
            .context("accept_relation requires --evidence")?;
        let original = storage::evidence_snapshot(conn, evidence)?;
        ensure!(
            original.source_id == bound["source_id"]
                && original.source_revision_id == bound["source_revision_id"],
            "evidence does not belong to this exact source revision"
        );
        ensure!(
            util::digest(&original.excerpt) == original.digest,
            "evidence integrity failure"
        );
        let active: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM assertion_evidence ae JOIN active_assertions a ON a.assertion_revision_id=ae.assertion_revision_id WHERE ae.evidence_id=?1)", [evidence], |r|r.get(0))?;
        ensure!(active, "evidence is withdrawn or obsolete");
        if storage::relation_facts(conn)?.iter().any(|f| {
            f.active
                && f.kind == relation
                && f.evidence_id == evidence
                && f.to == bound["target_unit_id"]
                && f.source_id == original.source_id
                && f.source_revision_id == original.source_revision_id
                && f.assertion_revision_id == bound["assertion_revision_id"]
        }) {
            authority = "existing_documentary_relation_confirmed";
        }
    } else {
        ensure!(
            request.relation.is_none() && request.evidence.is_none(),
            "relation/evidence are only accepted with accept_relation"
        );
    }
    let identity = util::json_digest(&(
        &bound,
        request.decision,
        request.relation,
        request.evidence,
        request.reason,
        request.actor,
        request.actor_kind,
    ))?;
    let state = if request.decision == "accept_relation" {
        "resolved"
    } else {
        "dismissed"
    };
    let same = previous.last().filter(|event| {
        event["event_kind"] == "applied"
            && event["idempotency_identity"] == identity
            && review_events.last().is_some_and(|review| {
                review.reason_code == "manual_adjudication" && review.to_status == state
            })
    });
    let mut evidence = bound["evidence_ids"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(id) = request.evidence
        && !evidence.contains(&json!(id))
    {
        evidence.push(json!(id));
    }
    let mut result = json!({"contract":"lore.review_adjudication","schema_version":1,"review_id":id,
        "context_digest":context_digest,"binding":bound,"decision":request.decision,
        "prior_relation":bound["candidate"],"proposed_relation":request.relation,"evidence_ids":evidence,
        "reason":request.reason,"actor":request.actor,"actor_kind":request.actor_kind,"actor_authenticated":false,
        "authority":authority,"projected_review_status":state,"idempotency_identity":identity,
        "changed":false,"source_write":false,"model_calls":0,
        "qualification":"Manual adjudication affects this exact candidate's advice only. It never creates documentary authority, merges knowledge or edits historical evidence."});
    if apply {
        ensure!(
            !conn.is_autocommit(),
            "adjudication requires the publication transaction"
        );
        ensure!(
            available(conn)?,
            "run lore update to migrate adjudication storage first"
        );
        let expected = request
            .expect_digest
            .context("--apply requires --expect-context-digest from a preview")?;
        ensure!(
            expected == context_digest
                || same.is_some_and(|event| event["context_digest"] == expected),
            "review context changed; preview again"
        );
        if same.is_none() {
            result["changed"] = json!(true);
            result["event_kind"] = json!("applied");
            result["recorded_at"] = json!(util::now());
            conn.execute("INSERT INTO review_adjudication_events(id,review_id,event_kind,payload_json) VALUES(?1,?2,'applied',?3)", params![util::id("adj"),id,serde_json::to_string(&result)?])?;
            transition(
                conn,
                id,
                state,
                ReviewActor {
                    kind: "user",
                    name: request.actor,
                },
                "manual_adjudication",
                request.reason,
                request.evidence,
            )?;
        }
    }
    Ok(result)
}

pub fn refresh(conn: &Connection) -> Result<usize> {
    let mut changed = 0;
    for event in current(conn)? {
        let id = event["review_id"]
            .as_str()
            .context("invalid adjudication history")?;
        if binding(conn, id).ok().as_ref() == Some(&event["binding"]) {
            continue;
        }
        let invalid = json!({"schema_version":1,"review_id":id,"event_kind":"invalidated","previous":event["idempotency_identity"],"recorded_at":util::now(),"reason":"Candidate or documentary source state changed; the prior interpretation is retained only as history."});
        conn.execute("INSERT INTO review_adjudication_events(id,review_id,event_kind,payload_json) VALUES(?1,?2,'invalidated',?3)",params![util::id("adj"),id,serde_json::to_string(&invalid)?])?;
        transition(
            conn,
            id,
            "pending",
            ReviewActor {
                kind: "automatic",
                name: "lore",
            },
            "manual_adjudication_invalidated",
            invalid["reason"].as_str().unwrap(),
            None,
        )?;
        changed += 1;
    }
    Ok(changed)
}
