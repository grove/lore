//! Cross-topic documentary decision relationships. These are explicit source-backed
//! transitions, not guessed chronological dates or verified deployments.
use crate::{domain::KnowledgeView, storage::RelationFact};
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
    pub from_label: String,
    pub to_label: String,
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
    relations: &[RelationFact],
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

        out.push(DecisionLink {
            relation: relation.kind.clone(),
            from_id: from.id.clone(),
            to_id: to.id.clone(),
            from_statement: from.statement.clone(),
            to_statement: to.statement.clone(),
            from_topic: from.topic.clone(),
            to_topic: to.topic.clone(),
            from_title: from.topic_title.clone(),
            from_label: source_label(&relation.source_locator),
            to_label: predecessor_label(to),
            to_title: to.topic_title.clone(),
            claimed_effective_at: if from.effective_at.trim().is_empty() {
                None
            } else {
                Some(from.effective_at.clone())
            },
            supporting_source: Some(relation.source_locator.clone()),
            evidence_id: Some(relation.evidence_id.clone()),
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

fn source_label(locator: &str) -> String {
    let path = locator.split_once(':').map_or(locator, |(_, p)| p);
    let stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    if stem.contains('-') && stem.chars().any(|c| c.is_ascii_digit()) {
        stem.to_owned()
    } else {
        path.to_owned()
    }
}
fn predecessor_label(unit: &KnowledgeView) -> String {
    let active = unit
        .evidence
        .iter()
        .filter(|e| e.active)
        .collect::<Vec<_>>();
    let evidence = if active.is_empty() {
        unit.evidence.iter().collect::<Vec<_>>()
    } else {
        active
    };
    let labels = evidence
        .into_iter()
        .map(|e| source_label(&e.source))
        .collect::<std::collections::BTreeSet<_>>();
    if labels.is_empty() {
        format!("Decision {}", unit.id)
    } else {
        labels.into_iter().collect::<Vec<_>>().join(" / ")
    }
}
/// A reaffirmation page must also see its predecessor's explicit successor.
pub(super) fn context_for(slug: &str, links: &[DecisionLink]) -> Vec<DecisionLink> {
    let mut ids = std::collections::BTreeSet::new();
    for link in links.iter().filter(|l| l.touches(slug)) {
        ids.insert(link.from_id.clone());
        ids.insert(link.to_id.clone());
    }
    loop {
        let before = ids.len();
        for link in links {
            if ids.contains(&link.from_id) || ids.contains(&link.to_id) {
                ids.insert(link.from_id.clone());
                ids.insert(link.to_id.clone());
            }
        }
        if before == ids.len() {
            break;
        }
    }
    links
        .iter()
        .filter(|l| ids.contains(&l.from_id) || ids.contains(&l.to_id))
        .cloned()
        .collect()
}
