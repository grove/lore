//! Conservative, source-bound checks shared by wiki pages and the overview.
//! These are safeguards against unsupported prose, not a substitute for
//! semantic verification by a model or a human reviewer.
use crate::domain::{KnowledgeView, PageDraft};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

/// A narrative can use temporal ordering directly supported by cited text,
/// or compare two separately evidenced effective dates. Document publication
/// order alone is never an event chronology.
pub(super) fn validate_prose(draft: &PageDraft, units: &[&KnowledgeView]) -> Result<()> {
    let by_id: BTreeMap<_, _> = units.iter().map(|u| (u.id.as_str(), *u)).collect();
    for section in &draft.sections {
        for paragraph in &section.paragraphs {
            let prose = format!(" {} ", paragraph.text.to_lowercase());
            let cited = paragraph
                .knowledge_ids
                .iter()
                .filter_map(|id| by_id.get(id.as_str()).copied())
                .collect::<Vec<_>>();
            // Claims that the *corpus* lacks evidence require a complete
            // search, which a bounded synthesis context cannot provide.
            // This does not prohibit a source-specific denial, nor the
            // truthful qualification "not independently verified".
            const ABSENCE: &[&str] = &[
                "available records do not",
                "supplied record does not",
                "records do not show",
                "records do not resolve",
                "sources do not say",
                "documentation does not state",
                "does not state the rationale",
                "no record of",
                "no evidence exists",
                "nothing in the sources",
            ];
            for phrase in ABSENCE {
                if prose.contains(phrase) {
                    ensure!(
                        cited.iter().any(|unit| unit
                            .evidence
                            .iter()
                            .any(|e| { e.excerpt.to_lowercase().contains(phrase) })),
                        "unsupported corpus-wide absence claim: {phrase}; describe only what the cited sources establish"
                    );
                }
            }

            const ORDER: &[&str] = &[
                " before ",
                " after ",
                " earlier than ",
                " later than ",
                " subsequently ",
                " preceded ",
                " followed by ",
            ];
            for phrase in ORDER {
                if !prose.contains(phrase) {
                    continue;
                }
                let stated = cited.iter().any(|unit| {
                    let statement = format!(" {} ", unit.statement.to_lowercase());
                    statement.contains(phrase)
                        || unit
                            .evidence
                            .iter()
                            .any(|e| format!(" {} ", e.excerpt.to_lowercase()).contains(phrase))
                });
                let dated = cited
                    .iter()
                    .map(|u| u.effective_at.as_str())
                    .filter(|date| !date.is_empty())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    >= 2;
                ensure!(
                    stated || dated,
                    "unsupported event ordering ({phrase:?}); cite an explicit source chronology or two distinct effective dates"
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EvidenceView, PageSection, Paragraph};

    fn unit(statement: &str, excerpt: &str) -> KnowledgeView {
        KnowledgeView {
            id: "k".into(),
            revision_id: "r".into(),
            statement: statement.into(),
            topic: "organizational-policy".into(),
            topic_title: "Organizational policy".into(),
            subject: "access committee".into(),
            kind: "observation".into(),
            lifecycle: "unknown".into(),
            base_lifecycle: "unknown".into(),
            scope: "committee".into(),
            effective_at: String::new(),
            support_state: "current_documentary_support".into(),
            evidence: vec![EvidenceView {
                id: "e".into(),
                assertion_id: "a".into(),
                source_id: "s".into(),
                source: "minutes".into(),
                excerpt: excerpt.into(),
                captured_at: String::new(),
                active: true,
            }],
            relations: vec![],
        }
    }
    fn draft(text: &str) -> PageDraft {
        PageDraft {
            sections: vec![PageSection {
                heading: "Summary".into(),
                paragraphs: vec![Paragraph {
                    text: text.into(),
                    knowledge_ids: vec!["k".into()],
                }],
            }],
        }
    }

    #[test]
    fn does_not_infer_before_from_when() {
        let u = unit(
            "The vote was recorded when the meeting ended.",
            "The vote was recorded when the meeting ended.",
        );
        assert!(
            validate_prose(
                &draft("The vote was recorded before the meeting ended."),
                &[&u]
            )
            .is_err()
        );
    }

    #[test]
    fn accepts_explicit_order_and_qualified_reports() {
        let u = unit(
            "The committee approved the policy after consultation.",
            "The committee approved the policy after consultation.",
        );
        assert!(validate_prose(&draft("The committee approved the policy after consultation; deployment is not independently verified."), &[&u]).is_ok());
    }

    #[test]
    fn rejects_unsourced_absence_but_allows_documented_denials() {
        let u = unit(
            "The board selected the policy for cost reasons.",
            "The board selected the policy for cost reasons.",
        );
        assert!(
            validate_prose(
                &draft("The supplied record does not state the rationale for the policy."),
                &[&u]
            )
            .is_err()
        );
        assert!(validate_prose(&draft("The proposal was not approved."), &[&u]).is_ok());
    }
}
