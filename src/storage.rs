use crate::{
    domain::{AssertionProposal, EvidenceView, KnowledgeView},
    sources::{Chunk, Document, locate_quote},
    util,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, MAIN_DB, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const SCHEMA_V1: &str = include_str!("../migrations/0001_knowledge.sql");
pub const SCHEMA_V2: &str = include_str!("../migrations/0002_runtime.sql");
pub const SCHEMA_V3: &str = include_str!("../migrations/0003_reaffirmations.sql");
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > 3 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let result = (|| {
        if version == 0 {
            conn.execute_batch(SCHEMA_V1)?;
        }
        if version < 2 {
            conn.execute_batch(SCHEMA_V2)?;
        }
        if version < 3 {
            conn.execute_batch(SCHEMA_V3)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    result
}
pub fn read_only(path: &Path) -> Result<Connection> {
    util::reject_symlinks(path)?;
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?)
}
pub fn stage_database(old: &Path, stage: &Path) -> Result<Connection> {
    if old.exists() {
        read_only(old)?.backup(MAIN_DB, stage, None)?;
    }
    let conn = Connection::open(stage)?;
    migrate(&conn).context("database migration failed")?;
    conn.pragma_update(None, "journal_mode", "DELETE")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    Ok(conn)
}
pub fn meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE name='lore_meta'",
        [],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Ok(None);
    }
    Ok(conn
        .query_row("SELECT value FROM lore_meta WHERE key=?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}
pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO lore_meta VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceHead {
    pub id: String,
    pub root: String,
    pub path: String,
    pub revision: String,
    pub digest: String,
}
pub fn source_heads(conn: &Connection) -> Result<Vec<SourceHead>> {
    if meta(conn, "initialized")?.is_none() {
        return Ok(vec![]);
    }
    let mut s = conn.prepare("SELECT s.id,s.root_id,s.relative_path,c.source_revision_id,c.content_digest FROM sources s JOIN source_current c ON s.id=c.source_id WHERE s.removed_at IS NULL ORDER BY s.root_id,s.relative_path")?;
    Ok(s.query_map([], |r| {
        Ok(SourceHead {
            id: r.get(0)?,
            root: r.get(1)?,
            path: r.get(2)?,
            revision: r.get(3)?,
            digest: r.get(4)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()?)
}
pub fn begin_source(
    conn: &Connection,
    doc: &Document,
    existing: Option<&SourceHead>,
) -> Result<(String, String)> {
    let source_id = existing
        .map(|h| h.id.clone())
        .unwrap_or_else(|| util::id("src"));
    if existing.is_some() {
        conn.execute(
            "UPDATE sources SET relative_path=?2,removed_at=NULL WHERE id=?1",
            params![source_id, doc.relative_path],
        )?;
    } else {
        conn.execute(
            "INSERT INTO sources(id,root_id,relative_path) VALUES(?1,?2,?3)",
            params![source_id, doc.root_id, doc.relative_path],
        )?;
    }
    let unchanged = existing.is_some_and(|h| h.digest == doc.digest && h.path == doc.relative_path);
    let revision = if unchanged {
        existing.unwrap().revision.clone()
    } else {
        let revision = util::id("sr");
        conn.execute(
            "INSERT INTO source_revisions VALUES(?1,?2,?3,?4,?5)",
            params![
                revision,
                source_id,
                doc.relative_path,
                doc.digest,
                util::now()
            ],
        )?;
        revision
    };
    conn.execute("INSERT INTO source_current VALUES(?1,?2,?3) ON CONFLICT(source_id) DO UPDATE SET source_revision_id=excluded.source_revision_id,content_digest=excluded.content_digest", params![source_id,revision,doc.digest])?;
    Ok((source_id, revision))
}
pub fn current_chunks(
    conn: &Connection,
    source: &str,
) -> Result<BTreeMap<String, (String, String)>> {
    let mut s = conn.prepare(
        "SELECT section_key,section_id,input_digest FROM current_sections WHERE source_id=?1",
    )?;
    Ok(
        s.query_map([source], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
            .collect::<rusqlite::Result<_>>()?,
    )
}
pub fn retire_section(conn: &Connection, section: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM active_assertions WHERE section_id=?1",
        [section],
    )?;
    conn.execute(
        "DELETE FROM current_sections WHERE section_id=?1",
        [section],
    )?;
    Ok(())
}
pub fn retire_source(conn: &Connection, source: &str) -> Result<()> {
    for (_, (section, _)) in current_chunks(conn, source)? {
        retire_section(conn, &section)?;
    }
    conn.execute("DELETE FROM source_current WHERE source_id=?1", [source])?;
    conn.execute(
        "UPDATE sources SET removed_at=?2 WHERE id=?1",
        params![source, util::now()],
    )?;
    Ok(())
}
pub fn begin_section(
    conn: &Connection,
    source: &str,
    revision: &str,
    chunk: &Chunk,
) -> Result<(String, String)> {
    let section = format!("sec_{}", &util::json_digest(&(source, &chunk.key))?[7..]);
    conn.execute(
        "INSERT OR IGNORE INTO source_sections VALUES(?1,?2)",
        params![section, source],
    )?;
    retire_section(conn, &section)?;
    let section_revision = format!(
        "sc_{}",
        &util::json_digest(&(&section, revision, &chunk.input_digest))?[7..]
    );
    conn.execute("INSERT OR IGNORE INTO section_revisions(id,section_id,source_revision_id,heading_path_json,content_digest) VALUES(?1,?2,?3,?4,?5)", params![section_revision,section,revision,serde_json::to_string(&chunk.heading_path)?,util::digest(&chunk.text)])?;
    conn.execute(
        "INSERT INTO current_sections VALUES(?1,?2,?3,?4,?5)",
        params![
            section,
            source,
            chunk.key,
            section_revision,
            chunk.input_digest
        ],
    )?;
    Ok((section, section_revision))
}
pub struct AssertionCapture<'a> {
    pub document: &'a Document,
    pub chunk: &'a Chunk,
    pub proposal: &'a AssertionProposal,
    pub source: &'a str,
    pub source_revision: &'a str,
    pub section: &'a str,
    pub section_revision: &'a str,
    pub model: &'a str,
}
pub fn capture_assertion(conn: &Connection, c: AssertionCapture<'_>) -> Result<(String, String)> {
    c.proposal.validate()?;
    if !c.proposal.effective_at.is_empty() {
        ensure!(crate::domain::effective_time_grounded(
            &c.proposal.quote, &c.chunk.context, &c.proposal.effective_at
        ), "claimed effective date lacks event evidence");
        ensure!(
            c.chunk.text.contains(&c.proposal.effective_at)
                || c.chunk.context.contains(&c.proposal.effective_at),
            "effective time is not an explicit source phrase"
        );
    }
    let (first, last, before, after) = locate_quote(c.document, c.chunk, &c.proposal.quote)?;
    let evidence = format!(
        "ev_{}",
        &util::json_digest(&(
            c.source_revision,
            c.section_revision,
            &c.proposal.quote,
            first,
            last
        ))?[7..]
    );
    conn.execute("INSERT OR IGNORE INTO evidence_snapshots(id,source_id,source_revision_id,section_revision_id,exact_excerpt,context_before,context_after,excerpt_digest,line_start,line_end,captured_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![evidence,c.source,c.source_revision,c.section_revision,c.proposal.quote,before,after,util::digest(&c.proposal.quote),first as i64,last as i64,util::now()])?;
    let lineage = util::json_digest(&(
        c.source,
        c.section,
        &c.proposal.subject,
        &c.proposal.statement,
        &c.proposal.kind,
        &c.proposal.scope,
        &c.proposal.effective_at,
    ))?;
    let assertion = format!("as_{}", &lineage[7..]);
    conn.execute(
        "INSERT OR IGNORE INTO source_assertions VALUES(?1,?2,?3)",
        params![assertion, c.source, util::now()],
    )?;
    let revision = format!(
        "ar_{}",
        &util::json_digest(&(&assertion, &evidence, c.proposal, c.model, "extract-v1"))?[7..]
    );
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM assertion_revisions WHERE id=?1)",
        [&revision],
        |r| r.get(0),
    )?;
    if !exists {
        let number: i64 = conn.query_row("SELECT coalesce(max(revision_number),0)+1 FROM assertion_revisions WHERE assertion_id=?1", [&assertion], |r|r.get(0))?;
        conn.execute(
            "INSERT INTO assertion_revisions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                revision,
                assertion,
                c.source,
                c.source_revision,
                number,
                c.proposal.statement,
                c.proposal.kind,
                if c.proposal.effective_at.is_empty() {
                    None
                } else {
                    Some(&c.proposal.effective_at)
                },
                util::now(),
                c.model,
                "extract-v1"
            ],
        )?;
        conn.execute(
            "INSERT INTO assertion_evidence VALUES(?1,?2,?3,?4)",
            params![revision, evidence, c.source, c.source_revision],
        )?;
        conn.execute(
            "INSERT INTO assertion_details VALUES(?1,?2,?3)",
            params![revision, serde_json::to_string(c.proposal)?, lineage],
        )?;
        conn.execute("INSERT INTO sealed_assertions VALUES(?1)", [&revision])?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO active_assertions VALUES(?1,?2)",
        params![revision, c.section],
    )?;
    Ok((revision, evidence))
}
pub fn assigned(conn: &Connection, assertion: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT knowledge_id FROM assertion_assignments WHERE assertion_revision_id=?1",
            [assertion],
            |r| r.get(0),
        )
        .optional()?)
}
pub fn assign(conn: &Connection, assertion: &str, unit: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO assertion_assignments VALUES(?1,?2)",
        params![assertion, unit],
    )?;
    Ok(())
}
pub fn create_unit(
    conn: &Connection,
    project: &str,
    assertion: &str,
    a: &AssertionProposal,
) -> Result<String> {
    let topic: Option<String> = conn
        .query_row(
            "SELECT id FROM topics WHERE project_id=?1 AND slug=?2",
            params![project, a.topic],
            |r| r.get(0),
        )
        .optional()?;
    let topic = match topic {
        Some(t) => t,
        None => {
            let t = util::id("topic");
            conn.execute(
                "INSERT INTO topics VALUES(?1,?2,?3,?4)",
                params![t, project, a.topic, a.topic_title],
            )?;
            t
        }
    };
    let unit = util::id("ku");
    conn.execute(
        "INSERT INTO knowledge_units VALUES(?1,?2,?3)",
        params![unit, project, util::now()],
    )?;
    conn.execute(
        "INSERT INTO knowledge_details VALUES(?1,?2,?3,?4,?5,?6)",
        params![unit, topic, a.subject, a.scope, a.effective_at, a.lifecycle],
    )?;
    conn.execute(
        "INSERT INTO topic_units VALUES(?1,?2)",
        params![topic, unit],
    )?;
    assign(conn, assertion, &unit)?;
    write_revision(
        conn,
        &unit,
        &a.statement,
        &a.kind,
        &a.lifecycle,
        "current_documentary_support",
        "",
        "initial",
    )?;
    Ok(unit)
}
fn write_revision(
    conn: &Connection,
    unit: &str,
    statement: &str,
    kind: &str,
    lifecycle: &str,
    support: &str,
    temporal: &str,
    fingerprint: &str,
) -> Result<String> {
    let number: i64 = conn.query_row(
        "SELECT coalesce(max(revision_number),0)+1 FROM knowledge_revisions WHERE knowledge_id=?1",
        [unit],
        |r| r.get(0),
    )?;
    let revision = util::id("kr");
    conn.execute(
        "INSERT INTO knowledge_revisions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            revision,
            unit,
            number,
            statement,
            kind,
            lifecycle,
            support,
            temporal,
            util::now()
        ],
    )?;
    conn.execute("INSERT INTO knowledge_support SELECT ?1,assertion_revision_id,'supports' FROM assertion_assignments WHERE knowledge_id=?2",params![revision,unit])?;
    conn.execute("INSERT INTO sealed_knowledge VALUES(?1)", [&revision])?;
    conn.execute("INSERT INTO knowledge_current VALUES(?1,?2,?3) ON CONFLICT(knowledge_id) DO UPDATE SET revision_id=excluded.revision_id,input_digest=excluded.input_digest",params![unit,revision,fingerprint])?;
    Ok(revision)
}
#[derive(Debug, Clone, Serialize)]
pub struct RelationRow {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub active: bool,
}
pub fn relations(conn: &Connection) -> Result<Vec<RelationRow>> {
    let mut s=conn.prepare("SELECT f.knowledge_id,t.knowledge_id,r.relation,EXISTS(SELECT 1 FROM active_assertions a WHERE a.assertion_revision_id=ra.assertion_revision_id) FROM knowledge_relations r JOIN knowledge_revisions f ON f.id=r.from_revision_id JOIN knowledge_revisions t ON t.id=r.to_revision_id JOIN relation_assertions ra ON ra.relation_id=r.id ORDER BY r.id")?;
    let mut edges = s
        .query_map([], |r| {
            Ok(RelationRow {
                from: r.get(0)?,
                to: r.get(1)?,
                kind: r.get(2)?,
                active: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut s=conn.prepare("SELECT from_unit_id,to_unit_id,EXISTS(SELECT 1 FROM active_assertions a WHERE a.assertion_revision_id=reaffirmation_links.assertion_revision_id) FROM reaffirmation_links ORDER BY id")?;
    let reaffirmations = s
        .query_map([], |r| {
            Ok(RelationRow {
                from: r.get(0)?,
                to: r.get(1)?,
                kind: "reaffirms".into(),
                active: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    edges.extend(reaffirmations);
    Ok(edges)
}
/// A reaffirmation is a distinct historical project event, not equivalent
/// support for the original decision. Link only exact source-backed evidence.
pub fn add_reaffirmation(
    conn: &Connection,
    from: &str,
    to: &str,
    assertion: &str,
    evidence: &str,
) -> Result<()> {
    ensure!(from != to, "a decision cannot reaffirm itself");
    let id = format!("reaf_{}", &util::json_digest(&(from, to, assertion))?[7..]);
    conn.execute(
        "INSERT OR IGNORE INTO reaffirmation_links VALUES(?1,?2,?3,?4,?5,?6)",
        params![id, from, to, assertion, evidence, util::now()],
    )?;
    Ok(())
}
pub fn add_relation(
    conn: &Connection,
    from: &str,
    to: &str,
    kind: &str,
    assertion: &str,
    evidence: &str,
) -> Result<()> {
    ensure!(from != to, "self-relationship is not allowed");
    let same_source_revision:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM assertion_revisions a JOIN evidence_snapshots e ON e.source_revision_id=a.source_revision_id AND e.source_id=a.source_id WHERE a.id=?1 AND e.id=?2)",params![assertion,evidence],|r|r.get(0))?;
    ensure!(
        same_source_revision,
        "relationship evidence belongs to a different source revision"
    );
    if kind == "supersedes" {
        let edges = relations(conn)?;
        let mut stack = vec![to.to_owned()];
        let mut seen = BTreeSet::new();
        while let Some(node) = stack.pop() {
            ensure!(node != from, "supersession cycle rejected");
            if !seen.insert(node.clone()) {
                continue;
            }
            for e in &edges {
                if e.active && e.kind == "supersedes" && e.from == node {
                    stack.push(e.to.clone());
                }
            }
        }
    }
    let from_rev: String = conn.query_row(
        "SELECT revision_id FROM knowledge_current WHERE knowledge_id=?1",
        [from],
        |r| r.get(0),
    )?;
    let to_rev: String = conn.query_row(
        "SELECT revision_id FROM knowledge_current WHERE knowledge_id=?1",
        [to],
        |r| r.get(0),
    )?;
    let id = format!(
        "rel_{}",
        &util::json_digest(&(from, to, kind, assertion))?[7..]
    );
    conn.execute(
        "INSERT OR IGNORE INTO knowledge_relations VALUES(?1,?2,?3,?4,?5)",
        params![id, from_rev, to_rev, kind, evidence],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO relation_assertions VALUES(?1,?2)",
        params![id, assertion],
    )?;
    Ok(())
}
pub fn review(conn: &Connection, project: &str, key: &str, reason: &str) -> Result<()> {
    let id = format!("review_{}", &util::digest(key)[7..]);
    conn.execute(
        "INSERT OR IGNORE INTO review_items VALUES(?1,?2,?3,'pending')",
        params![id, project, reason],
    )?;
    Ok(())
}
fn evidence_for(conn: &Connection, unit: &str) -> Result<Vec<EvidenceView>> {
    let mut s=conn.prepare("SELECT e.id,a.assertion_revision_id,s.id,s.root_id||':'||s.relative_path,e.exact_excerpt,e.captured_at,EXISTS(SELECT 1 FROM active_assertions x WHERE x.assertion_revision_id=a.assertion_revision_id) FROM assertion_assignments a JOIN assertion_evidence ae ON ae.assertion_revision_id=a.assertion_revision_id JOIN evidence_snapshots e ON e.id=ae.evidence_id JOIN sources s ON s.id=e.source_id WHERE a.knowledge_id=?1 ORDER BY e.id")?;
    Ok(s.query_map([unit], |r| {
        Ok(EvidenceView {
            id: r.get(0)?,
            assertion_id: r.get(1)?,
            source_id: r.get(2)?,
            source: r.get(3)?,
            excerpt: r.get(4)?,
            captured_at: r.get(5)?,
            active: r.get(6)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()?)
}
pub fn views(conn: &Connection) -> Result<Vec<KnowledgeView>> {
    let edges = relations(conn)?;
    let mut s=conn.prepare("SELECT u.knowledge_id,u.revision_id,k.statement,t.slug,t.title,d.subject,k.kind,k.lifecycle,d.base_lifecycle,d.scope,d.effective_at,k.support_state FROM knowledge_current u JOIN knowledge_revisions k ON k.id=u.revision_id JOIN knowledge_details d ON d.knowledge_id=u.knowledge_id JOIN topics t ON t.id=d.topic_id ORDER BY t.slug,u.knowledge_id")?;
    let rows = s
        .query_map([], |r| {
            Ok(KnowledgeView {
                id: r.get(0)?,
                revision_id: r.get(1)?,
                statement: r.get(2)?,
                topic: r.get(3)?,
                topic_title: r.get(4)?,
                subject: r.get(5)?,
                kind: r.get(6)?,
                lifecycle: r.get(7)?,
                base_lifecycle: r.get(8)?,
                scope: r.get(9)?,
                effective_at: r.get(10)?,
                support_state: r.get(11)?,
                evidence: vec![],
                relations: vec![],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|mut v| {
            v.evidence = evidence_for(conn, &v.id)?;
            v.relations = edges
                .iter()
                .filter(|e| e.from == v.id || e.to == v.id)
                .map(|e| {
                    format!(
                        "{} {} {} ({})",
                        e.from,
                        e.kind,
                        e.to,
                        if e.active {
                            "current documentary evidence"
                        } else {
                            "historical evidence only"
                        }
                    )
                })
                .collect();
            v.relations.sort();
            v.relations.dedup();
            Ok(v)
        })
        .collect()
}
pub fn refresh_knowledge(conn: &Connection, project: &str) -> Result<()> {
    let edges = relations(conn)?;
    for unit in views(conn)? {
        let active = unit.evidence.iter().any(|e| e.active);
        let superseded = edges
            .iter()
            .any(|e| e.to == unit.id && e.kind == "supersedes" && e.active);
        let withdrawn = active
            && !superseded
            && edges
                .iter()
                .any(|e| e.to == unit.id && e.kind == "supersedes" && !e.active);
        let support = if withdrawn {
            "needs_review"
        } else if active {
            "current_documentary_support"
        } else {
            "historical_only"
        };
        let lifecycle = if superseded {
            "superseded"
        } else {
            &unit.base_lifecycle
        };
        if withdrawn {
            review(
                conn,
                project,
                &format!("withdrawn:{}", unit.id),
                &format!(
                    "{} previously had replacement evidence which is no longer current; do not assume the old decision is current again.",
                    unit.id
                ),
            )?;
        }
        let fingerprint = util::json_digest(&(
            &unit.statement,
            &unit.kind,
            lifecycle,
            support,
            &unit.scope,
            &unit.effective_at,
            &unit.evidence,
            &unit.relations,
        ))?;
        let old: String = conn.query_row(
            "SELECT input_digest FROM knowledge_current WHERE knowledge_id=?1",
            [&unit.id],
            |r| r.get(0),
        )?;
        if old != fingerprint {
            write_revision(
                conn,
                &unit.id,
                &unit.statement,
                &unit.kind,
                lifecycle,
                support,
                if active { "documented" } else { "historical" },
                &fingerprint,
            )?;
        }
        conn.execute(
            "DELETE FROM knowledge_fts WHERE knowledge_id=?1",
            [&unit.id],
        )?;
        conn.execute(
            "INSERT INTO knowledge_fts VALUES(?1,?2,?3,?4)",
            params![unit.id, unit.statement, unit.subject, unit.topic],
        )?;
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPage {
    pub path: String,
    pub input_digest: String,
    pub output_digest: String,
    pub content: String,
}
pub fn pages(conn: &Connection) -> Result<BTreeMap<String, StoredPage>> {
    let mut s = conn
        .prepare("SELECT path,input_digest,output_digest,content FROM wiki_pages ORDER BY path")?;
    Ok(s.query_map([], |r| {
        let p = StoredPage {
            path: r.get(0)?,
            input_digest: r.get(1)?,
            output_digest: r.get(2)?,
            content: r.get(3)?,
        };
        Ok((p.path.clone(), p))
    })?
    .collect::<rusqlite::Result<_>>()?)
}
pub fn save_pages(conn: &Connection, pages: &BTreeMap<String, StoredPage>) -> Result<()> {
    conn.execute("DELETE FROM wiki_pages", [])?;
    for p in pages.values() {
        conn.execute(
            "INSERT INTO wiki_pages VALUES(?1,?2,?3,?4)",
            params![p.path, p.input_digest, p.output_digest, p.content],
        )?;
    }
    Ok(())
}
pub fn search(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<(String, String, String)>> {
    let terms = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .take(32)
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(" OR ");
    if terms.is_empty() {
        return Ok(vec![]);
    }
    let mut s=conn.prepare("SELECT knowledge_id,statement,topic FROM knowledge_fts WHERE knowledge_fts MATCH ?1 ORDER BY bm25(knowledge_fts) LIMIT ?2")?;
    Ok(s.query_map(params![terms, limit.min(100) as i64], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })?
    .collect::<rusqlite::Result<_>>()?)
}
