//! Evidence-bound review lifecycle. Disposition never edits knowledge or evidence.
use crate::{storage, util};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const START: &str = "<!-- lore:review-status:start -->";
pub const END: &str = "<!-- lore:review-status:end -->";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: String,
    pub reason: String,
    pub status: String,
    pub category: Option<String>,
    pub assertion_revision_id: Option<String>,
    pub target_unit_id: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReviewEvent {
    pub sequence: i64,
    pub from_status: Option<String>,
    pub to_status: String,
    pub actor_type: String,
    pub actor: String,
    pub reason_code: String,
    pub note: String,
    pub evidence_id: Option<String>,
    pub context_digest: String,
    pub recorded_at: String,
}
fn modern(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='review_context')",
        [],
        |r| r.get(0),
    )?)
}
pub fn list(conn: &Connection, all: bool) -> Result<Vec<ReviewItem>> {
    let sql = if modern(conn)? {
        "SELECT r.id,r.reason,r.status,c.category,c.assertion_revision_id,c.target_unit_id FROM review_items r LEFT JOIN review_context c ON c.review_id=r.id WHERE ?1 OR r.status='pending' ORDER BY r.id"
    } else {
        "SELECT id,reason,status,NULL,NULL,NULL FROM review_items WHERE ?1 OR status='pending' ORDER BY id"
    };
    let mut q = conn.prepare(sql)?;
    Ok(q.query_map([all], |r| {
        Ok(ReviewItem {
            id: r.get(0)?,
            reason: r.get(1)?,
            status: r.get(2)?,
            category: r.get(3)?,
            assertion_revision_id: r.get(4)?,
            target_unit_id: r.get(5)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()?)
}
pub fn events(conn: &Connection, id: &str) -> Result<Vec<ReviewEvent>> {
    if !modern(conn)? {
        return Ok(vec![]);
    }
    let mut q=conn.prepare("SELECT sequence,from_status,to_status,actor_type,actor,reason_code,note,evidence_id,context_digest,recorded_at FROM review_events WHERE review_id=?1 ORDER BY sequence")?;
    Ok(q.query_map([id], |r| {
        Ok(ReviewEvent {
            sequence: r.get(0)?,
            from_status: r.get(1)?,
            to_status: r.get(2)?,
            actor_type: r.get(3)?,
            actor: r.get(4)?,
            reason_code: r.get(5)?,
            note: r.get(6)?,
            evidence_id: r.get(7)?,
            context_digest: r.get(8)?,
            recorded_at: r.get(9)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()?)
}
pub fn show(conn: &Connection, id: &str) -> Result<Value> {
    let review = list(conn, true)?
        .into_iter()
        .find(|r| r.id == id)
        .context("review ID not found")?;
    Ok(
        json!({"review":review,"history":events(conn,id)?,"qualification":"A review disposition is not independent verification and does not modify the knowledge graph."}),
    )
}
fn review_id(key: &str) -> String {
    format!("review_{}", &util::digest(key)[7..])
}

/// Bind exact machine keys, not prose or topic-level resemblance.
fn attach_context(conn: &Connection, id: &str, key: &str) -> Result<()> {
    let fields: Vec<_> = key.split(':').collect();
    let category = match fields[0] {
        "relationship" | "replacement" | "reaffirmation" | "conflict" => fields[0],
        "ambiguous" | "ambiguous-predecessor" | "existing-duplicate" => "ambiguity",
        _ => "other",
    };
    let mut assertion = None;
    if let Some(candidate) = fields.get(1) {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM assertion_revisions WHERE id=?1)",
            [candidate],
            |r| r.get(0),
        )?;
        if exists {
            assertion = Some(*candidate);
        }
    }
    let mut target = None;
    if matches!(
        category,
        "relationship" | "replacement" | "reaffirmation" | "conflict"
    ) {
        if let Some(candidate) = fields.get(2) {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM knowledge_units WHERE id=?1)",
                [candidate],
                |r| r.get(0),
            )?;
            if exists {
                target = Some(*candidate);
            }
        }
    }
    conn.execute(
        "INSERT OR IGNORE INTO review_context VALUES(?1,?2,?3,?4,?5)",
        params![id, key, category, assertion, target],
    )?;
    Ok(())
}
pub fn record(conn: &Connection, project: &str, key: &str, reason: &str) -> Result<()> {
    let id = review_id(key);
    conn.execute(
        "INSERT OR IGNORE INTO review_items VALUES(?1,?2,?3,'pending')",
        params![id, project, reason],
    )?;
    attach_context(conn, &id, key)?;
    if events(conn, &id)?.is_empty() {
        initial_event(conn, &id, "automatic")?;
    }
    Ok(())
}
fn initial_event(conn: &Connection, id: &str, actor: &str) -> Result<()> {
    let status: String =
        conn.query_row("SELECT status FROM review_items WHERE id=?1", [id], |r| {
            r.get(0)
        })?;
    conn.execute("INSERT INTO review_events(id,review_id,from_status,to_status,actor_type,actor,reason_code,note,context_digest,recorded_at) VALUES(?1,?2,NULL,?3,?4,'lore','observed','Review state observed; earlier history is not inferred.',?5,?6)",params![util::id("re"),id,status,actor,fingerprint(conn,id)?,util::now()])?;
    Ok(())
}
/// Verify original hashed keys against retained logs. Unknown legacy questions
/// remain unbound: never auto-close them by guessing at prose.
pub fn backfill(conn: &Connection) -> Result<()> {
    let mut q =
        conn.prepare("SELECT assertion_revision_id,operation_json FROM reconciliation_log")?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (assertion, raw) in rows {
        let Ok(op) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let mut keys = vec![
            format!("ambiguous:{assertion}"),
            format!("existing-duplicate:{assertion}"),
        ];
        if let Some(relations) = op["relations"].as_array() {
            for relation in relations {
                if let Some(target) = relation["target_id"].as_str() {
                    for prefix in ["relationship", "replacement", "reaffirmation", "conflict"] {
                        keys.push(format!("{prefix}:{assertion}:{target}"));
                    }
                }
            }
        }
        for key in keys {
            let id = review_id(&key);
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM review_items WHERE id=?1)",
                [&id],
                |r| r.get(0),
            )?;
            if exists {
                attach_context(conn, &id, &key)?;
            }
        }
    }
    for item in list(conn, true)? {
        if events(conn, &item.id)?.is_empty() {
            initial_event(conn, &item.id, "migration")?;
        }
    }
    Ok(())
}
fn fingerprint(conn: &Connection, id: &str) -> Result<String> {
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT assertion_revision_id,target_unit_id FROM review_context WHERE review_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((assertion, target)) = row else {
        return Ok(String::new());
    };
    let source: Option<(Option<String>, bool)> = if let Some(a) = assertion {
        conn.query_row("SELECT sc.content_digest,EXISTS(SELECT 1 FROM active_assertions x WHERE x.assertion_revision_id=ar.id) FROM assertion_revisions ar LEFT JOIN source_current sc ON sc.source_id=ar.source_id WHERE ar.id=?1",[a],|r|Ok((r.get(0)?,r.get(1)?))).optional()?
    } else {
        None
    };
    let target_digest: Option<String> = if let Some(t) = target {
        conn.query_row(
            "SELECT input_digest FROM knowledge_current WHERE knowledge_id=?1",
            [t],
            |r| r.get(0),
        )
        .optional()?
    } else {
        None
    };
    if source.is_none() && target_digest.is_none() {
        return Ok(String::new());
    }
    util::json_digest(&(source, target_digest))
}
fn transition(
    conn: &Connection,
    id: &str,
    next: &str,
    actor_type: &str,
    actor: &str,
    code: &str,
    note: &str,
    evidence: Option<&str>,
) -> Result<bool> {
    ensure!(
        ["pending", "resolved", "dismissed"].contains(&next),
        "invalid review status"
    );
    ensure!(
        !note.trim().is_empty() && note.len() <= 4000,
        "review reason must contain 1..4000 bytes"
    );
    ensure!(
        !actor.trim().is_empty() && actor.len() <= 120 && !actor.chars().any(char::is_control),
        "invalid review actor"
    );
    let old: String = conn
        .query_row("SELECT status FROM review_items WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .context("review ID not found")?;
    if old == next {
        return Ok(false);
    }
    conn.execute("INSERT INTO review_events(id,review_id,from_status,to_status,actor_type,actor,reason_code,note,evidence_id,context_digest,recorded_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![util::id("re"),id,old,next,actor_type,actor,code,note,evidence,fingerprint(conn,id)?,util::now()])?;
    Ok(true)
}
pub fn manual(conn: &Connection, id: &str, next: &str, reason: &str, actor: &str) -> Result<bool> {
    ensure!(
        modern(conn)?,
        "run lore update to migrate review history first"
    );
    backfill(conn)?;
    transition(
        conn,
        id,
        next,
        "user",
        actor,
        "manual_disposition",
        reason,
        None,
    )
}

/// Only an active explicit edge from the same source revision to the same
/// predecessor resolves a bound documentary relationship question. General
/// ambiguity is not approved. Withdrawal of resolution evidence reopens it.
pub fn refresh(conn: &Connection) -> Result<usize> {
    backfill(conn)?;
    let facts = storage::relation_facts(conn)?;
    let mut changed = 0;
    for item in list(conn, true)? {
        let history = events(conn, &item.id)?;
        let Some(last) = history.last() else { continue };
        if last.actor_type == "user" {
            let now = fingerprint(conn, &item.id)?;
            if now.is_empty() || now == last.context_digest {
                continue;
            }
            changed += transition(
                conn,
                &item.id,
                "pending",
                "automatic",
                "lore",
                "evidence_changed",
                "Evidence relevant to the human disposition changed; review it again.",
                None,
            )? as usize;
            continue;
        }
        if last.actor_type == "automatic"
            && last.reason_code == "documented_relationship"
            && item.status == "resolved"
        {
            let valid = last.evidence_id.as_ref().is_some_and(|ev| {
                facts.iter().any(|f| {
                    f.active && &f.evidence_id == ev && Some(&f.to) == item.target_unit_id.as_ref()
                })
            });
            if !valid {
                changed += transition(
                    conn,
                    &item.id,
                    "pending",
                    "automatic",
                    "lore",
                    "resolution_withdrawn",
                    "The documentary evidence that resolved this question is no longer active.",
                    None,
                )? as usize;
            }
            continue;
        }
        if item.status != "pending" {
            continue;
        }
        let Some(category) = item.category.as_deref() else {
            continue;
        };
        if !matches!(category, "relationship" | "replacement" | "reaffirmation") {
            continue;
        }
        let (Some(assertion), Some(target)) = (&item.assertion_revision_id, &item.target_unit_id)
        else {
            continue;
        };
        let identity: (String,String,String,String)=conn.query_row("SELECT ar.source_id,ar.source_revision_id,ar.modality,json_extract(ad.proposal_json,'$.lifecycle') FROM assertion_revisions ar JOIN assertion_details ad ON ad.assertion_revision_id=ar.id WHERE ar.id=?1",[assertion],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        // A decision relationship cannot settle a proposal or report merely
        // because both appear in the same document revision.
        if identity.2 != "decision" || identity.3 != "accepted" {
            continue;
        }

        let matching: Vec<_> = facts
            .iter()
            .filter(|f| {
                f.active
                    && f.to == *target
                    && f.source_id == identity.0
                    && f.source_revision_id == identity.1
                    && match category {
                        "replacement" => f.kind == "supersedes",
                        "reaffirmation" => f.kind == "reaffirms",
                        _ => matches!(f.kind.as_str(), "supersedes" | "reaffirms"),
                    }
            })
            .collect();
        let outcomes: BTreeSet<_> = matching.iter().map(|f| (&f.from, &f.kind)).collect();
        if outcomes.len() == 1 {
            let f = matching[0];
            changed += transition(
                conn,
                &item.id,
                "resolved",
                "automatic",
                "lore",
                "documented_relationship",
                &format!(
                    "An active, explicit {} relationship from the same source revision to this predecessor resolves this question. No runtime verification is implied.",
                    f.kind
                ),
                Some(&f.evidence_id),
            )? as usize;
        }
    }
    Ok(changed)
}
pub fn status_block(conn: &Connection) -> Result<String> {
    let items = list(conn, true)?;
    let pending = items.iter().filter(|r| r.status == "pending").count();
    Ok(format!(
        "{START}\n**Review queue:** {pending} pending of {} retained questions. [Review history](reviews.md). A closed review is a recorded disposition, not proof of implementation.\n{END}",
        items.len()
    ))
}
pub fn page(conn: &Connection) -> Result<storage::StoredPage> {
    let items = list(conn, true)?;
    let mut text = String::from(
        "# Review queue and history\n\n[Project overview](index.md)\n\nReviews track uncertainty, not project truth. Resolve or dismiss a queue item only after inspecting its evidence. Human dispositions do not change assertions, documented conflicts, or source snapshots.\n\n",
    );
    for item in &items {
        text.push_str(&format!(
            "## {} — {}\n\n{}\n\n",
            item.id,
            item.status,
            util::markdown_text(&item.reason)
        ));
        for event in events(conn, &item.id)? {
            text.push_str(&format!(
                "{} · {} ({}) → **{}**: {}",
                event.recorded_at,
                util::markdown_text(&event.actor),
                event.actor_type,
                event.to_status,
                util::markdown_text(&event.note)
            ));
            if let Some(ev) = event.evidence_id {
                text.push_str(&format!(" Evidence: `{ev}`."));
            }
            text.push_str("\n\n");
        }
    }
    if items.is_empty() {
        text.push_str("No review items recorded.\n");
    }
    ensure!(
        text.len() <= 4_000_000,
        "review history exceeds page size limit"
    );
    Ok(storage::StoredPage {
        path: "reviews.md".into(),
        input_digest: util::digest(&text),
        output_digest: util::digest(&text),
        content: text,
    })
}
