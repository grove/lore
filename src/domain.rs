use crate::util;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    pub assertions: Vec<AssertionProposal>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionProposal {
    pub topic: String,
    pub topic_title: String,
    pub subject: String,
    pub statement: String,
    pub kind: String,
    pub lifecycle: String,
    pub scope: String,
    pub effective_at: String,
    pub quote: String,
}
impl AssertionProposal {
    pub fn validate(&self) -> Result<()> {
        util::safe_slug(&self.topic)?;
        for s in [
            &self.topic_title,
            &self.subject,
            &self.statement,
            &self.scope,
        ] {
            ensure!(
                !s.trim().is_empty() && s.len() <= 4_000,
                "missing or oversized assertion field"
            );
        }
        ensure!(
            [
                "decision",
                "plan",
                "proposal",
                "observation",
                "reported_outcome",
                "constraint",
                "question",
                "issue_state",
                "risk",
                "procedure"
            ]
            .contains(&self.kind.as_str()),
            "unsupported assertion kind"
        );
        ensure!(
            [
                "proposed",
                "accepted",
                "active",
                "completed",
                "rejected",
                "superseded",
                "unknown"
            ]
            .contains(&self.lifecycle.as_str()),
            "unsupported lifecycle"
        );
        ensure!(self.effective_at.len() <= 100, "oversized effective time");
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reconciliation {
    pub equivalent_to: String,
    pub relations: Vec<RelationProposal>,
    pub uncertain: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationProposal {
    pub target_id: String,
    pub kind: String,
    pub quote: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceView {
    pub id: String,
    pub assertion_id: String,
    pub source_id: String,
    pub source: String,
    pub excerpt: String,
    pub captured_at: String,
    pub active: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeView {
    pub id: String,
    pub revision_id: String,
    pub statement: String,
    pub topic: String,
    pub topic_title: String,
    pub subject: String,
    pub kind: String,
    pub lifecycle: String,
    pub base_lifecycle: String,
    pub scope: String,
    pub effective_at: String,
    pub support_state: String,
    pub evidence: Vec<EvidenceView>,
    pub relations: Vec<String>,
}
impl KnowledgeView {
    pub fn same_semantics(&self, a: &AssertionProposal) -> bool {
        self.subject.eq_ignore_ascii_case(&a.subject)
            && self.kind == a.kind
            && self.scope.eq_ignore_ascii_case(&a.scope)
            && self.effective_at == a.effective_at
            && self.base_lifecycle == a.lifecycle
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageDraft {
    pub sections: Vec<PageSection>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageSection {
    pub heading: String,
    pub paragraphs: Vec<Paragraph>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paragraph {
    pub text: String,
    pub knowledge_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub supported: bool,
    pub issues: Vec<String>,
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object", "properties":properties,"required":required,"additionalProperties":false})
}
pub fn extraction_schema() -> Value {
    let mut properties = serde_json::Map::new();
    let fields = [
        "topic",
        "topic_title",
        "subject",
        "statement",
        "kind",
        "lifecycle",
        "scope",
        "effective_at",
        "quote",
    ];
    for field in fields {
        properties.insert(field.into(), json!({"type":"string"}));
    }
    object(
        json!({"assertions":{"type":"array", "items":object(Value::Object(properties), &fields)}}),
        &["assertions"],
    )
}
pub fn reconciliation_schema() -> Value {
    object(
        json!({"equivalent_to":{"type":"string"}, "uncertain":{"type":"boolean"}, "relations":{"type":"array", "items":object(json!({"target_id":{"type":"string"},"kind":{"type":"string"},"quote":{"type":"string"},"reason":{"type":"string"}}), &["target_id","kind","quote","reason"])}}),
        &["equivalent_to", "uncertain", "relations"],
    )
}
pub fn page_schema() -> Value {
    let paragraph = object(
        json!({"text":{"type":"string"},"knowledge_ids":{"type":"array","items":{"type":"string"}}}),
        &["text", "knowledge_ids"],
    );
    let section = object(
        json!({"heading":{"type":"string"},"paragraphs":{"type":"array","items":paragraph}}),
        &["heading", "paragraphs"],
    );
    object(
        json!({"sections":{"type":"array","items":section}}),
        &["sections"],
    )
}
pub fn verification_schema() -> Value {
    object(
        json!({"supported":{"type":"boolean"},"issues":{"type":"array","items":{"type":"string"}}}),
        &["supported", "issues"],
    )
}
