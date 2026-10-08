//! Cross-topic documentary decision relationships. These are explicit source-backed
//! transitions, not guessed chronological dates or verified deployments.
use crate::{domain::KnowledgeView, storage::RelationRow};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub(super) struct DecisionLink {
    pub relation: String,
    pub from_id: String,
    pub to_id: String,
    pub from_statement: String,
    pub to_statement: String,
    pub from_topic: String,
    pub to_topic: String,
    pub from_title: String,
    pub to_title: String,
    pub claimed_effective_at: Option<String>,
    pub supporting_source: Option<String>,
    pub evidence_id: Option<String>,
}
impl DecisionLink {
    pub fn touches(&self, slug: &str) -> bool {
        self.from_topic == slug || self.to_topic == slug
    }
}
pub(super) fn decision_links(
    units: &[KnowledgeView],
    relations: &[RelationRow],
) -> Vec<DecisionLink> {
    let by_id: BTreeMap<&str, &KnowledgeView> =
        units.iter().map(|unit| (unit.id.as_str(), unit)).collect();
    let mut out = Vec::new();
    for relation in relations {
        if !relation.active || !matches!(relation.kind.as_str(), "supersedes" | "reaffirms") {
            continue;
        }
        let (Some(from), Some(to)) = (
            by_id.get(relation.from.as_str()),
            by_id.get(relation.to.as_str()),
        ) else {
            continue;
        };
        if from.kind != "decision" || to.kind != "decision" {
            continue;
        }
        let observed = from.evidence.iter().find(|e| e.active);
        out.push(DecisionLink {
            relation: relation.kind.clone(),
            from_id: from.id.clone(),
            to_id: to.id.clone(),
            from_statement: from.statement.clone(),
            to_statement: to.statement.clone(),
            from_topic: from.topic.clone(),
            to_topic: to.topic.clone(),
            from_title: from.topic_title.clone(),
            to_title: to.topic_title.clone(),
            claimed_effective_at: if from.effective_at.trim().is_empty() {
                None
            } else {
                Some(from.effective_at.clone())
            },
            supporting_source: observed.map(|e| e.source.clone()),
            evidence_id: observed.map(|e| e.id.clone()),
        });
    }
    out.sort_by(|a, b| {
        (&a.from_topic, &a.to_topic, &a.from_id, &a.to_id).cmp(&(
            &b.from_topic,
            &b.to_topic,
            &b.from_id,
            &b.to_id,
        ))
    });
    out
}
