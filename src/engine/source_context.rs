//! Documentary context that crosses wiki-topic boundaries. Source identity is
//! the boundary, not model-assigned topic names. These records are context,
//! never additional citeable IDs for a topic's prose.
use crate::domain::KnowledgeView;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Find knowledge from another topic that cites the same *active source*.
/// A change to these units invalidates the affected topic's rendered page.
pub(super) fn sibling_units<'a>(
    knowledge: &'a [KnowledgeView],
    local: &[&KnowledgeView],
) -> Vec<&'a KnowledgeView> {
    let source_ids = local
        .iter()
        .flat_map(|u| {
            u.evidence
                .iter()
                .filter(|e| e.active)
                .map(|e| e.source_id.as_str())
        })
        .collect::<BTreeSet<_>>();
    let local_ids = local.iter().map(|u| u.id.as_str()).collect::<BTreeSet<_>>();
    let mut siblings = knowledge
        .iter()
        .filter(|u| !local_ids.contains(u.id.as_str()))
        .filter(|u| {
            u.evidence
                .iter()
                .any(|e| e.active && source_ids.contains(e.source_id.as_str()))
        })
        .collect::<Vec<_>>();
    siblings.sort_by(|a, b| (&a.topic, &a.id).cmp(&(&b.topic, &b.id)));
    siblings
}

/// Context-only evidence for the verifier, always with an attributable
/// original excerpt. A record is included whole or not at all.
pub(super) fn sibling_rows(
    siblings: &[&KnowledgeView],
    local: &[&KnowledgeView],
    byte_budget: usize,
) -> Result<(Vec<Value>, bool)> {
    let source_ids = local
        .iter()
        .flat_map(|u| {
            u.evidence
                .iter()
                .filter(|e| e.active)
                .map(|e| e.source_id.as_str())
        })
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    let mut used = 2usize;
    let mut complete = true;
    for unit in siblings {
        let Some(evidence) = unit
            .evidence
            .iter()
            .find(|e| e.active && source_ids.contains(e.source_id.as_str()))
        else {
            continue;
        };
        let entry = json!({
            "knowledge_id": unit.id,
            "topic": unit.topic,
            "kind": unit.kind,
            "lifecycle": unit.lifecycle,
            "statement": unit.statement,
            "source": evidence.source,
            "evidence_id": evidence.id,
            "exact_excerpt": evidence.excerpt,
            "citation_role": "verifier_context_only_not_citeable_in_this_topic",
        });
        let size = serde_json::to_vec(&entry)?.len() + 1;
        if size > byte_budget.saturating_sub(used) {
            complete = false;
            continue;
        }
        used += size;
        rows.push(entry);
    }
    Ok((rows, complete))
}

/// Heading paths are captured from the original source revision, not guessed
/// from model output. A heading date establishes document metadata only; it
/// must never automatically populate effective_at or a deployment date.
pub(super) fn source_headings(
    conn: &Connection,
    units: &[&KnowledgeView],
    budget: usize,
) -> Result<Vec<Value>> {
    let mut seen = BTreeSet::new();
    let mut records = Vec::new();
    let mut used = 2usize;
    for unit in units {
        for evidence in unit.evidence.iter().filter(|e| e.active) {
            let heading: Option<String> = conn
                .query_row(
                    "SELECT sr.heading_path_json FROM evidence_snapshots e \
                     LEFT JOIN section_revisions sr ON sr.id=e.section_revision_id WHERE e.id=?1",
                    [&evidence.id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            let Some(heading) = heading else {
                continue;
            };
            let headings: Vec<String> = serde_json::from_str(&heading)?;
            if headings.is_empty()
                || !seen.insert((unit.id.clone(), evidence.source_id.clone(), heading.clone()))
            {
                continue;
            }
            let entry = json!({
                "knowledge_id": unit.id,
                "source": evidence.source,
                "heading_path": headings,
                "date_role": "document_heading_not_event_effective_date",
            });
            let size = serde_json::to_vec(&entry)?.len() + 1;
            if size <= budget.saturating_sub(used) {
                used += size;
                records.push(entry);
            }
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EvidenceView;
    fn unit(id: &str, topic: &str, source: &str) -> KnowledgeView {
        KnowledgeView {
            id: id.into(),
            revision_id: id.into(),
            statement: format!("Policy {id}"),
            topic: topic.into(),
            topic_title: topic.into(),
            subject: "committee".into(),
            kind: "observation".into(),
            lifecycle: "unknown".into(),
            base_lifecycle: "unknown".into(),
            scope: "team".into(),
            effective_at: String::new(),
            support_state: "current_documentary_support".into(),
            relations: vec![],
            evidence: vec![EvidenceView {
                id: id.into(),
                assertion_id: id.into(),
                source_id: source.into(),
                source: source.into(),
                excerpt: format!("Exact {id}"),
                captured_at: String::new(),
                active: true,
            }],
        }
    }
    #[test]
    fn joins_by_active_source_not_by_topic_or_shared_words() {
        let a = unit("closure", "issues", "minutes");
        let b = unit("approval", "policies", "minutes");
        let c = unit("unrelated", "policies", "research");
        let units = [a, b, c];
        let siblings = sibling_units(&units, &[&units[0]]);
        assert_eq!(
            siblings.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
            vec!["approval"]
        );
        let (rows, complete) = sibling_rows(&siblings, &[&units[0]], 8_192).unwrap();
        assert!(complete);
        assert_eq!(rows[0]["exact_excerpt"], "Exact approval");
        assert!(
            rows[0]["citation_role"]
                .as_str()
                .unwrap()
                .contains("not_citeable")
        );
        let (rows, complete) = sibling_rows(&siblings, &[&units[0]], 1).unwrap();
        assert!(rows.is_empty());
        assert!(!complete);
    }
}
