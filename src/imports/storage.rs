//! Storage and exact evidence resolution for native observations.
use super::{ImportedObservation, Inventory, adapters::AdapterRecord, validate_record};
use crate::{domain::ImportKind, util};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default)]
pub struct ImportChanges {
    pub changed: usize,
    pub retired: usize,
}

pub fn available(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='native_snapshots')",
        [],
        |r| r.get(0),
    )?)
}

/// The caller owns the staged publication transaction, so native records,
/// reconciled relationships and documentary knowledge publish atomically.
pub fn persist(conn: &Connection, project: &str, inventory: &Inventory) -> Result<ImportChanges> {
    let mut changes = ImportChanges::default();
    let mut retained = BTreeSet::new();
    let mut retained_imports = BTreeSet::new();
    for source in &inventory.sources {
        retained_imports.insert(source.id.clone());
        let existing: Option<(String, String)> = conn
            .query_row(
                "SELECT project_id,kind FROM native_imports WHERE id=?1",
                [&source.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((old_project, old_kind)) = existing {
            ensure!(
                old_project == project && old_kind == source.kind.as_str(),
                "native import {} already identifies another project/source kind; use a new import ID",
                source.id
            );
        } else {
            conn.execute(
                "INSERT INTO native_imports VALUES(?1,?2,?3)",
                params![source.id, project, source.kind.as_str()],
            )?;
        }
        conn.execute("INSERT INTO native_import_state VALUES(?1,?2,?3,?4) ON CONFLICT(import_id) DO UPDATE SET configured_path=excluded.configured_path,input_digest=excluded.input_digest,warnings_json=excluded.warnings_json",
            params![source.id,source.path.to_string_lossy(),source.digest,serde_json::to_string(&source.batch.warnings)?])?;
        for record in &source.batch.records {
            validate_record(record)?;
            let id = format!(
                "no_{}",
                &util::json_digest(&(project, &source.id, source.kind, &record.native_id))?[7..]
            );
            retained.insert(id.clone());
            let content_hash = util::json_digest(&record.native_record)?;
            let normalized_hash = util::json_digest(record)?;
            let snapshot_id = format!(
                "ns_{}",
                &util::json_digest(&(
                    "native-snapshot-v1",
                    &id,
                    &content_hash,
                    &normalized_hash,
                    &source.path,
                    &source.batch.format
                ))?[7..]
            );
            let evidence_id = format!("ne_{}", &snapshot_id[3..]);
            let old: Option<String> = conn
                .query_row(
                    "SELECT snapshot_id FROM native_current WHERE observation_id=?1",
                    [&id],
                    |r| r.get(0),
                )
                .optional()?;
            if old.as_ref() == Some(&snapshot_id) {
                continue;
            }
            changes.changed += 1;
            conn.execute(
                "INSERT OR IGNORE INTO native_records VALUES(?1,?2,?3)",
                params![id, source.id, record.native_id],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO native_snapshots VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    snapshot_id,
                    id,
                    evidence_id,
                    content_hash,
                    normalized_hash,
                    source.path.to_string_lossy(),
                    source.batch.format,
                    serde_json::to_string(record)?,
                    util::now()
                ],
            )?;
            for table in ["native_latest", "native_current"] {
                conn.execute(&format!("INSERT INTO {table} VALUES(?1,?2) ON CONFLICT(observation_id) DO UPDATE SET snapshot_id=excluded.snapshot_id"),params![id,snapshot_id])?;
            }
            conn.execute("DELETE FROM native_fts WHERE observation_id=?1", [&id])?;
            conn.execute(
                "INSERT INTO native_fts VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    id,
                    record.native_id,
                    record.title,
                    record.subject,
                    record.statement,
                    record.tags.join(" "),
                    record.links.join(" "),
                    record.scope.component.as_deref().unwrap_or("")
                ],
            )?;
        }
    }
    let previous = conn
        .prepare("SELECT observation_id FROM native_current ORDER BY observation_id")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in previous {
        if !retained.contains(&id) {
            conn.execute("DELETE FROM native_current WHERE observation_id=?1", [&id])?;
            // Retained historical text remains searchable and explicitly retired.
            changes.retired += 1;
        }
    }
    let configured = conn
        .prepare("SELECT import_id FROM native_import_state ORDER BY import_id")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in configured {
        if !retained_imports.contains(&id) {
            conn.execute("DELETE FROM native_import_state WHERE import_id=?1", [id])?;
        }
    }
    Ok(changes)
}

const SELECT_VIEW: &str = "SELECT r.id,s.evidence_id,s.id,r.import_id,i.kind,s.source_path,s.format,s.content_hash,s.captured_at,EXISTS(SELECT 1 FROM native_current c WHERE c.observation_id=r.id AND c.snapshot_id=s.id),s.record_json FROM native_snapshots s JOIN native_records r ON r.id=s.observation_id JOIN native_imports i ON i.id=r.import_id";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportedObservation> {
    let raw: String = row.get(10)?;
    let kind: String = row.get(4)?;
    let origin = match kind.as_str() {
        "openwiki" => ImportKind::Openwiki,
        "engram" => ImportKind::Engram,
        "beads" => ImportKind::Beads,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let record = serde_json::from_str::<AdapterRecord>(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(ImportedObservation {
        id: row.get(0)?,
        evidence_id: row.get(1)?,
        snapshot_id: row.get(2)?,
        import_id: row.get(3)?,
        origin,
        source_path: row.get(5)?,
        format: row.get(6)?,
        content_hash: row.get(7)?,
        captured_at: row.get(8)?,
        current: row.get(9)?,
        record,
    })
}

/// Readable on pre-0.4 registries without migration or writes.
pub fn views(conn: &Connection) -> Result<Vec<ImportedObservation>> {
    if !available(conn)? {
        return Ok(Vec::new());
    }
    Ok(conn.prepare(&format!("{SELECT_VIEW} JOIN native_latest l ON l.observation_id=r.id AND l.snapshot_id=s.id ORDER BY r.id"))?
        .query_map([],from_row)?.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn evidence(conn: &Connection, id: &str) -> Result<ImportedObservation> {
    ensure!(
        available(conn)?,
        "native evidence is unavailable in this registry"
    );
    let view = conn
        .query_row(
            &format!("{SELECT_VIEW} WHERE s.evidence_id=?1"),
            [id],
            from_row,
        )
        .optional()?
        .context("native evidence ID not found")?;
    ensure!(
        util::json_digest(&view.record.native_record)? == view.content_hash,
        "native evidence content hash mismatch"
    );
    let normalized_hash: String = conn.query_row(
        "SELECT normalized_hash FROM native_snapshots WHERE evidence_id=?1",
        [id],
        |r| r.get(0),
    )?;
    ensure!(
        util::json_digest(&view.record)? == normalized_hash,
        "native normalized snapshot hash mismatch"
    );
    validate_record(&view.record)?;
    Ok(view)
}

pub fn warnings(conn: &Connection) -> Result<Vec<String>> {
    if !available(conn)? {
        return Ok(Vec::new());
    }
    let rows = conn
        .prepare("SELECT import_id,warnings_json FROM native_import_state ORDER BY import_id")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut result = Vec::new();
    for (id, raw) in rows {
        result.extend(
            serde_json::from_str::<Vec<String>>(&raw)?
                .into_iter()
                .map(|w| format!("Import {id}: {w}")),
        );
    }
    Ok(result)
}

pub fn audit(conn: &Connection) -> Result<Vec<String>> {
    if !available(conn)? {
        return Ok(Vec::new());
    }
    let rows = conn
        .prepare("SELECT evidence_id,normalized_hash FROM native_snapshots ORDER BY id")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut issues = Vec::new();
    for (id, normalized_hash) in rows {
        match evidence(conn, &id) {
            Ok(view) if util::json_digest(&view.record)? == normalized_hash => {}
            Ok(_) => issues.push(format!("Native normalized snapshot hash mismatch: {id}")),
            Err(e) => issues.push(format!("Invalid native evidence {id}: {e}")),
        }
    }
    Ok(issues)
}
