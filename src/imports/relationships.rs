//! Read-only access to Lore's revision-bound cross-source interpretations.
//!
//! Endpoint evidence retains its native authority. An interpretation is neither
//! an accepted decision nor an independent verification of implementation.

use crate::{reviews, util};
use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossSourceEndpoint {
    /// `observation` or `knowledge`; IDs are never inferred from display text.
    pub kind: String,
    pub id: String,
    /// An immutable native snapshot or knowledge revision, respectively.
    pub revision_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossSourceRelation {
    pub id: String,
    pub input_signature: String,
    pub from: CrossSourceEndpoint,
    pub to: Option<CrossSourceEndpoint>,
    /// `potential_discrepancy`, `uncertain`, `verification_question`,
    /// `related_to`, `consistent_with`, or `upstream_relationship`.
    pub kind: String,
    pub reason: String,
    pub qualifications: Vec<String>,
    /// Immutable IDs resolvable through `lore evidence`.
    pub evidence_ids: Vec<String>,
    /// Source-native vocabulary and direction are preserved unchanged.
    pub upstream_kind: Option<String>,
    #[serde(default)]
    pub upstream_status: Option<String>,
    #[serde(default)]
    pub upstream_active: Option<bool>,
    pub review_id: Option<String>,
    /// Current imported inputs, never proof of the current checkout.
    pub active: bool,
}

impl CrossSourceRelation {
    pub fn is_question(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "potential_discrepancy" | "uncertain" | "verification_question"
        )
    }

    pub fn endpoints(&self) -> impl Iterator<Item = &CrossSourceEndpoint> {
        std::iter::once(&self.from).chain(self.to.iter())
    }
}

fn available(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='cross_source_evaluations')",
        [],
        |row| row.get(0),
    )?)
}

/// Latest interpretations only. Historical records remain available via history.
pub fn relations(conn: &Connection) -> Result<Vec<CrossSourceRelation>> {
    read(conn, false)
}

pub fn history(conn: &Connection) -> Result<Vec<CrossSourceRelation>> {
    read(conn, true)
}

fn read(conn: &Connection, all: bool) -> Result<Vec<CrossSourceRelation>> {
    if !available(conn)? {
        return Ok(Vec::new());
    }
    let mut query = conn.prepare(
        "SELECT r.payload_json,c.input_signature IS NOT NULL
         FROM cross_source_relations r
         LEFT JOIN cross_source_current c ON c.input_signature=r.input_signature
         WHERE ?1 OR c.input_signature IS NOT NULL ORDER BY r.id",
    )?;
    let rows = query
        .query_map([all], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(payload, active)| {
            let mut relation: CrossSourceRelation = serde_json::from_str(&payload)?;
            relation.active = active;
            Ok(relation)
        })
        .collect()
}

/// Bounds and unperformed comparisons are durable qualifications, including on
/// later no-op updates and local model-free context reads.
pub fn warnings(conn: &Connection) -> Result<Vec<String>> {
    if !available(conn)? {
        return Ok(Vec::new());
    }
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM cross_source_state WHERE key='warnings'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    raw.map(|value| serde_json::from_str(&value).map_err(Into::into))
        .unwrap_or_else(|| Ok(Vec::new()))
}

/// Used by review dispositions to reopen a question only when its own inputs
/// change. The absence marker also detects removal and later reappearance.
pub(crate) fn review_fingerprint(conn: &Connection, review_id: &str) -> Result<Option<String>> {
    if !available(conn)? {
        return Ok(None);
    }
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT r.pair_key,c.input_signature FROM cross_source_review_context r
         LEFT JOIN cross_source_current c ON c.pair_key=r.pair_key WHERE r.review_id=?1",
            [review_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    row.map(|row| util::json_digest(&row)).transpose()
}

pub(crate) struct ReviewComparison {
    pub current: bool,
    pub raises_question: bool,
}

/// A review refers to one stable pair, while the interpretation of that pair
/// follows immutable input revisions. Missing comparisons are retired history,
/// not evidence that a project behavior or approval ceased to exist.
pub(crate) fn review_comparison(
    conn: &Connection,
    review_id: &str,
) -> Result<Option<ReviewComparison>> {
    if !available(conn)? {
        return Ok(None);
    }
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT e.disposition FROM cross_source_review_context r
         LEFT JOIN cross_source_current c ON c.pair_key=r.pair_key
         LEFT JOIN cross_source_evaluations e ON e.input_signature=c.input_signature
         WHERE r.review_id=?1",
            [review_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(row.map(|disposition| ReviewComparison {
        current: disposition.is_some(),
        raises_question: disposition.is_some_and(|kind| {
            matches!(
                kind.as_str(),
                "potential_discrepancy" | "uncertain" | "verification_question"
            )
        }),
    }))
}

pub(crate) fn evaluated(conn: &Connection, signature: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cross_source_evaluations WHERE input_signature=?1)",
        [signature],
        |row| row.get(0),
    )?)
}

pub(crate) struct CurrentComparison {
    pub key: String,
    pub from: CrossSourceEndpoint,
    pub to: Option<CrossSourceEndpoint>,
}

pub(crate) fn current_comparisons(conn: &Connection) -> Result<Vec<CurrentComparison>> {
    Ok(conn.prepare(
        "SELECT c.pair_key,e.from_kind,e.from_id,e.from_revision_id,e.to_kind,e.to_id,e.to_revision_id
         FROM cross_source_current c JOIN cross_source_evaluations e ON e.input_signature=c.input_signature
         ORDER BY c.pair_key",
    )?.query_map([],|row|{
        let to=match (row.get::<_,Option<String>>(4)?,row.get::<_,Option<String>>(5)?,row.get::<_,Option<String>>(6)?) {
            (None,None,None)=>None,
            (Some(kind),Some(id),Some(revision_id))=>Some(CrossSourceEndpoint{kind,id,revision_id}),
            _=>return Err(rusqlite::Error::InvalidQuery),
        };
        Ok(CurrentComparison{key:row.get(0)?,from:CrossSourceEndpoint{kind:row.get(1)?,id:row.get(2)?,revision_id:row.get(3)?},to})
    })?.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(crate) fn activate(conn: &Connection, pair_key: &str, signature: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO cross_source_current(pair_key,input_signature) VALUES(?1,?2)
         ON CONFLICT(pair_key) DO UPDATE SET input_signature=excluded.input_signature
         WHERE cross_source_current.input_signature<>excluded.input_signature",
        params![pair_key, signature],
    )?;
    Ok(())
}

/// Resolve evidence through the exact current endpoint revision before storing
/// any interpretation. The caller cannot attach an unrelated, older, or merely
/// similarly named source record to a new cross-source judgment.
fn endpoint_evidence(
    conn: &Connection,
    endpoint: &CrossSourceEndpoint,
) -> Result<BTreeSet<String>> {
    revision_evidence(conn, endpoint, true)
}

fn revision_evidence(
    conn: &Connection,
    endpoint: &CrossSourceEndpoint,
    current: bool,
) -> Result<BTreeSet<String>> {
    match endpoint.kind.as_str() {
        "observation" => {
            let evidence: Option<String> = conn
                .query_row(
                    "SELECT s.evidence_id FROM native_snapshots s
                     WHERE s.observation_id=?1 AND s.id=?2 AND (?3=0 OR EXISTS(
                       SELECT 1 FROM native_current c WHERE c.observation_id=s.observation_id AND c.snapshot_id=s.id))",
                    params![endpoint.id, endpoint.revision_id,current],
                    |row| row.get(0),
                )
                .optional()?;
            let evidence = evidence.ok_or_else(|| {
                anyhow::anyhow!("cross-source observation endpoint is not its current snapshot")
            })?;
            let observation = super::evidence(conn, &evidence)?;
            ensure!(
                !current
                    || !matches!(
                        observation.record.lifecycle.to_ascii_lowercase().as_str(),
                        "deleted"
                            | "withdrawn"
                            | "retracted"
                            | "superseded"
                            | "rejected"
                            | "orphaned"
                            | "deprecated"
                    ),
                "cross-source current endpoint refers to withdrawn native content"
            );
            Ok(BTreeSet::from([evidence]))
        }
        "knowledge" => {
            let exists:bool=conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM knowledge_revisions k WHERE k.knowledge_id=?1 AND k.id=?2
                 AND (?3=0 OR EXISTS(SELECT 1 FROM knowledge_current c WHERE c.knowledge_id=k.knowledge_id AND c.revision_id=k.id)))",
                params![endpoint.id,endpoint.revision_id,current],|row|row.get(0),
            )?;
            ensure!(
                exists,
                "cross-source knowledge endpoint is not its current revision"
            );
            let evidence = conn
                .prepare(
                    "SELECT DISTINCT ae.evidence_id FROM knowledge_support ks
                 JOIN assertion_evidence ae ON ae.assertion_revision_id=ks.assertion_revision_id
                 WHERE ks.knowledge_revision_id=?1 AND (?2=0 OR EXISTS(
                   SELECT 1 FROM active_assertions aa WHERE aa.assertion_revision_id=ae.assertion_revision_id))
                 ORDER BY ae.evidence_id",
                )?
                .query_map(params![endpoint.revision_id,current], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<BTreeSet<_>>>()?;
            ensure!(
                !evidence.is_empty(),
                "cross-source knowledge endpoint lacks active documentary evidence"
            );
            for id in &evidence {
                let snapshot = crate::storage::evidence_snapshot(conn, id)?;
                ensure!(
                    !snapshot.excerpt.is_empty()
                        && util::digest(&snapshot.excerpt) == snapshot.digest,
                    "cross-source documentary evidence failed its integrity check"
                );
            }
            Ok(evidence)
        }
        _ => anyhow::bail!("invalid cross-source endpoint kind"),
    }
}

/// Mechanically validate current endpoint revisions and complete evidence
/// binding without changing the registry or invoking inference. Context and
/// audit can share this check with the update-time writer.
pub fn validate_current_relation(conn: &Connection, relation: &CrossSourceRelation) -> Result<()> {
    ensure!(
        relation.active,
        "cross-source relationship is historical, not a current interpretation"
    );
    let mut expected = BTreeSet::new();
    for endpoint in relation.endpoints() {
        expected.extend(endpoint_evidence(conn, endpoint)?);
    }
    ensure!(
        relation
            .evidence_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            == expected,
        "cross-source evidence must cover exactly the supplied endpoint revisions"
    );
    Ok(())
}

/// Audit retained interpretations as well as current ones. Historical evidence
/// is checked against the immutable knowledge revision's retained support,
/// never against today's active assertions or native-current pointer.
pub fn audit(conn: &Connection) -> Result<Vec<String>> {
    if !available(conn)? {
        return Ok(Vec::new());
    }
    struct Evaluation {
        signature: String,
        key: String,
        from: CrossSourceEndpoint,
        to: Option<CrossSourceEndpoint>,
        current: bool,
    }
    let evaluations = conn
        .prepare(
            "SELECT e.input_signature,e.pair_key,e.from_kind,e.from_id,e.from_revision_id,
         e.to_kind,e.to_id,e.to_revision_id,
         EXISTS(SELECT 1 FROM cross_source_current c WHERE c.input_signature=e.input_signature)
         FROM cross_source_evaluations e ORDER BY e.input_signature",
        )?
        .query_map([], |row| {
            let to = match (
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ) {
                (None, None, None) => None,
                (Some(kind), Some(id), Some(revision_id)) => Some(CrossSourceEndpoint {
                    kind,
                    id,
                    revision_id,
                }),
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(Evaluation {
                signature: row.get(0)?,
                key: row.get(1)?,
                from: CrossSourceEndpoint {
                    kind: row.get(2)?,
                    id: row.get(3)?,
                    revision_id: row.get(4)?,
                },
                to,
                current: row.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut issues = Vec::new();
    for evaluation in evaluations {
        let checked = (|| -> Result<()> {
            let mut endpoints = Vec::new();
            let mut all_evidence = BTreeSet::new();
            for endpoint in std::iter::once(&evaluation.from).chain(evaluation.to.as_ref()) {
                let bound = revision_evidence(conn, endpoint, evaluation.current)?;
                all_evidence.extend(bound.iter().cloned());
                endpoints.push(bound);
            }
            let rows=conn.prepare("SELECT id,payload_json FROM cross_source_relations WHERE input_signature=?1 ORDER BY id")?
                .query_map([&evaluation.signature],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            ensure!(
                rows.len() <= 1,
                "multiple interpretations attached to one input signature"
            );
            for (id, payload) in rows {
                let relation: CrossSourceRelation = serde_json::from_str(&payload)?;
                ensure!(
                    relation.input_signature == evaluation.signature
                        && relation.from == evaluation.from
                        && relation.to == evaluation.to,
                    "relationship payload differs from its immutable evaluated endpoints"
                );
                let expected_id = format!(
                    "xrel_{}",
                    &util::digest(format!("{}:{}", evaluation.signature, relation.kind))[7..]
                );
                ensure!(
                    relation.id == id && id == expected_id,
                    "cross-source relationship identity hash mismatch"
                );
                let supplied: BTreeSet<_> = relation.evidence_ids.iter().cloned().collect();
                ensure!(
                    !supplied.is_empty()
                        && supplied.is_subset(&all_evidence)
                        && endpoints.iter().all(|bound| !bound.is_disjoint(&supplied)),
                    "relationship evidence is not bound to every exact endpoint revision"
                );
                if evaluation.current {
                    ensure!(
                        supplied == all_evidence,
                        "current relationship omits active endpoint evidence"
                    );
                }
                if let Some(upstream_kind) = &relation.upstream_kind {
                    ensure!(
                        evaluation.from.kind == "observation",
                        "upstream relationship has no native source endpoint"
                    );
                    let source = super::evidence(conn, endpoints[0].iter().next().unwrap())?;
                    let matched = source.record.relationships.iter().find(|native| {
                        native.kind == *upstream_kind
                            && native.upstream_status == relation.upstream_status
                            && Some(native.active) == relation.upstream_active
                            && util::json_digest(&(
                                "upstream",
                                &source.id,
                                &native.native_id,
                                &native.target_native_id,
                                &native.kind,
                            ))
                            .is_ok_and(|key| key == evaluation.key)
                    });
                    let native=matched.ok_or_else(||anyhow::anyhow!("upstream relationship vocabulary, direction, or state has no matching retained native record"))?;
                    if let Some(target) = &evaluation.to {
                        ensure!(
                            target.kind == "observation",
                            "upstream native target was silently promoted to documentary knowledge"
                        );
                        let target = super::evidence(conn, endpoints[1].iter().next().unwrap())?;
                        ensure!(
                            target.import_id == source.import_id
                                && target.record.native_id == native.target_native_id,
                            "upstream relationship points to a different native target or source namespace"
                        );
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = checked {
            issues.push(format!(
                "Invalid cross-source interpretation {}: {error}",
                evaluation.signature
            ));
        }
    }
    Ok(issues)
}

pub(crate) struct EvaluationEndpoints<'a> {
    pub from: &'a CrossSourceEndpoint,
    pub to: Option<&'a CrossSourceEndpoint>,
}

pub(crate) fn save(
    conn: &Connection,
    project: &str,
    pair_key: &str,
    signature: &str,
    endpoints: EvaluationEndpoints<'_>,
    disposition: &str,
    mut relation: Option<CrossSourceRelation>,
) -> Result<()> {
    let EvaluationEndpoints { from, to } = endpoints;
    ensure!(
        !evaluated(conn, signature)?,
        "cross-source input signature already evaluated"
    );
    let mut required_evidence = BTreeSet::new();
    for endpoint in std::iter::once(from).chain(to) {
        ensure!(
            matches!(endpoint.kind.as_str(), "observation" | "knowledge"),
            "invalid cross-source endpoint kind"
        );
        ensure!(
            !endpoint.id.is_empty() && !endpoint.revision_id.is_empty(),
            "cross-source endpoint missing identity"
        );
        required_evidence.extend(endpoint_evidence(conn, endpoint)?);
    }
    if let Some(value) = &relation {
        ensure!(
            value.from == *from && value.to.as_ref() == to,
            "cross-source relationship endpoints differ from evaluated inputs"
        );
        ensure!(
            [
                "potential_discrepancy",
                "uncertain",
                "verification_question",
                "related_to",
                "consistent_with",
                "upstream_relationship"
            ]
            .contains(&value.kind.as_str()),
            "invalid cross-source relationship kind"
        );
        ensure!(
            !value.reason.trim().is_empty() && value.reason.len() <= 20_000,
            "missing or oversized cross-source relationship reason"
        );
        ensure!(
            value.evidence_ids.iter().cloned().collect::<BTreeSet<_>>() == required_evidence,
            "cross-source evidence must cover exactly the supplied endpoint revisions"
        );
    }
    conn.execute(
        "INSERT INTO cross_source_evaluations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            signature,
            pair_key,
            from.kind,
            from.id,
            from.revision_id,
            to.map(|value| &value.kind),
            to.map(|value| &value.id),
            to.map(|value| &value.revision_id),
            disposition,
            util::now()
        ],
    )?;
    activate(conn, pair_key, signature)?;
    if let Some(ref mut value) = relation {
        ensure!(
            value.from == *from && value.to.as_ref() == to,
            "cross-source relationship endpoints differ from evaluated inputs"
        );
        ensure!(
            !value.evidence_ids.is_empty(),
            "cross-source relationship lacks evidence references"
        );
        value.evidence_ids.sort();
        value.evidence_ids.dedup();
        value.qualifications.sort();
        value.qualifications.dedup();
        value.input_signature = signature.into();
        value.id = format!(
            "xrel_{}",
            &util::digest(format!("{signature}:{}", value.kind))[7..]
        );
        value.active = true;
        // Routine qualifications on standalone recollections/completed work
        // remain useful task questions without flooding the review queue.
        // An ambiguous relationship (including an unresolved native endpoint)
        // is a concrete review question and retains a disposition history.
        if value.is_question() && (value.to.is_some() || value.kind == "uncertain") {
            let key = format!("cross-source:{pair_key}");
            let id = format!("review_{}", &util::digest(&key)[7..]);
            // Bind before the first review event's fingerprint is captured.
            conn.execute(
                "INSERT OR IGNORE INTO review_items VALUES(?1,?2,?3,'pending')",
                params![id, project, value.reason],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO cross_source_review_context VALUES(?1,?2)",
                params![id, pair_key],
            )?;
            reviews::record(conn, project, &key, &value.reason)?;
            value.review_id = Some(id);
        }
        conn.execute(
            "INSERT INTO cross_source_relations VALUES(?1,?2,?3)",
            params![value.id, signature, serde_json::to_string(value)?],
        )?;
    }
    Ok(())
}

pub(crate) fn finish(
    conn: &Connection,
    retained: &BTreeSet<String>,
    warnings: &[String],
) -> Result<()> {
    let mut query = conn.prepare("SELECT pair_key FROM cross_source_current")?;
    let current = query
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for key in current {
        if !retained.contains(&key) {
            conn.execute("DELETE FROM cross_source_current WHERE pair_key=?1", [key])?;
        }
    }
    conn.execute(
        "INSERT INTO cross_source_state VALUES('warnings',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value WHERE cross_source_state.value<>excluded.value",
        [serde_json::to_string(warnings)?],
    )?;
    Ok(())
}
