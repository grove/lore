//! Necessary source/citation checks, not a substitute for semantic verification.
//! Ordering paragraphs is presentation; only assertions in the text are claims.
use super::timeline::DecisionLink;
use crate::domain::{KnowledgeView, PageDraft};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Finding {
    pub section: usize,
    pub paragraph: Option<usize>,
    pub excerpt: String,
    pub reason: String,
}

impl Finding {
    pub fn validate(&self, draft: &PageDraft) -> Result<()> {
        let section = draft
            .sections
            .get(self.section)
            .ok_or_else(|| anyhow::anyhow!("unknown finding section"))?;
        let text = match self.paragraph {
            Some(index) => {
                &section
                    .paragraphs
                    .get(index)
                    .ok_or_else(|| anyhow::anyhow!("unknown finding paragraph"))?
                    .text
            }
            None => &section.heading,
        };
        ensure!(
            !self.excerpt.trim().is_empty() && text.contains(&self.excerpt),
            "finding must quote the exact offending text"
        );
        ensure!(
            !self.reason.trim().is_empty() && self.reason.len() <= 4000,
            "invalid finding reason"
        );
        Ok(())
    }
}

fn normalize(text: &str) -> String {
    format!(
        " {} ",
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    )
}

/// Locate every rejected paragraph. Repairs preserve non-rejected paragraphs.
pub(crate) fn findings(draft: &PageDraft, units: &[&KnowledgeView]) -> Vec<Finding> {
    let by_id: BTreeMap<_, _> = units.iter().map(|u| (u.id.as_str(), *u)).collect();
    let mut out = Vec::new();
    for (section, s) in draft.sections.iter().enumerate() {
        for (paragraph, p) in s.paragraphs.iter().enumerate() {
            let prose = normalize(&p.text);
            let cited = p
                .knowledge_ids
                .iter()
                .filter_map(|id| by_id.get(id.as_str()).copied())
                .collect::<Vec<_>>();
            let evidence = cited
                .iter()
                .flat_map(|u| &u.evidence)
                .map(|e| normalize(&e.excerpt))
                .collect::<Vec<_>>();
            let mut reasons = Vec::new();
            // A proposal can accurately report that a timetable or approval
            // was unsettled *when it was written*. It cannot establish a
            // universal present-tense absence: a later source may report a
            // completed change. Require attribution rather than promoting
            // future-intent documents into timeless current-state facts.
            let solely_future_intent = !cited.is_empty()
                && cited
                    .iter()
                    .all(|u| matches!(u.kind.as_str(), "proposal" | "plan"));
            let current_absence = [
                "no date has been",
                "no date is agreed",
                "no date is set",
                "no schedule has been",
                "no timetable has been",
                "has not yet been scheduled",
                "has not been agreed",
                "has not been approved",
                "hasn't been agreed",
                "hasn't been scheduled",
                "date remains unspecified",
            ]
            .iter()
            .any(|phrase| prose.contains(phrase));
            let attributed_to_source = [
                "the proposal",
                "the plan",
                "the document",
                "the source",
                "at the time",
                "as of ",
                "according to",
                "the earlier",
                "the original",
            ]
            .iter()
            .any(|phrase| prose.contains(phrase));
            if solely_future_intent && current_absence && !attributed_to_source {
                reasons.push("An earlier proposal/plan cannot support a current, unqualified absence of a date or approval. Attribute the claim to that source and its stage instead.".into());
            }
            for phrase in [
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
                "no further detail",
                "no additional detail",
                "no additional information",
                "no further information",
                "provides no detail",
                "provides no further",
                "gives no further",
                "gives no additional",
                "does not provide any details",
            ] {
                if prose.contains(phrase) && !evidence.iter().any(|e| e.contains(phrase)) {
                    reasons.push(format!("Unsupported source-wide absence claim ({phrase}); a sample cannot establish that information is missing elsewhere."));
                    break;
                }
            }
            for phrase in [
                " before ",
                " after ",
                " earlier than ",
                " later than ",
                " subsequently ",
                " preceded ",
                " followed by ",
            ] {
                if !prose.contains(phrase) || evidence.iter().any(|e| e.contains(phrase)) {
                    continue;
                }
                // Unrelated date fields are not a blanket license to invent
                // order. The semantic verifier still checks event identity
                // and direction when dates are explicitly part of the claim.
                let dates = cited
                    .iter()
                    .filter(|u| {
                        !u.effective_at.is_empty()
                            && p.text.contains(&u.effective_at)
                            && u.evidence
                                .iter()
                                .any(|e| e.excerpt.contains(&u.effective_at))
                    })
                    .map(|u| u.effective_at.as_str())
                    .collect::<BTreeSet<_>>();
                if dates.len() < 2 {
                    reasons.push(format!("unsupported event ordering ({phrase:?}); preserve the source's temporal wording. In particular, 'when' does not establish 'before' or 'after'."));
                    break;
                }
            }
            if !reasons.is_empty() {
                out.push(Finding {
                    section,
                    paragraph: Some(paragraph),
                    excerpt: p.text.clone(),
                    reason: reasons.join(" "),
                });
            }
        }
    }
    out
}

#[cfg(test)]
fn validate_prose(draft: &PageDraft, units: &[&KnowledgeView]) -> Result<()> {
    let issues = findings(draft, units);
    ensure!(
        issues.is_empty(),
        "{}",
        issues.first().map(|i| i.reason.as_str()).unwrap_or("")
    );
    Ok(())
}

/// Never append citations automatically to an unsupported relationship claim.
pub(crate) fn decision_findings(draft: &PageDraft, links: &[&DecisionLink]) -> Vec<Finding> {
    let mut out = Vec::new();
    for (section, s) in draft.sections.iter().enumerate() {
        for (paragraph, p) in s.paragraphs.iter().enumerate() {
            let text = p.text.to_lowercase();
            for link in links {
                let action = if link.relation == "reaffirms" {
                    text.contains("reaffirm") || text.contains("reconfirm")
                } else {
                    text.contains("supersed") || text.contains("replac")
                };
                let names_successor = text.contains(&link.from_label.to_lowercase());
                let names_predecessor = text.contains(&link.to_label.to_lowercase())
                    || p.knowledge_ids.contains(&link.to_id);
                if action
                    && names_successor
                    && names_predecessor
                    && !(p.knowledge_ids.contains(&link.from_id)
                        && p.knowledge_ids.contains(&link.to_id))
                {
                    out.push(Finding { section, paragraph: Some(paragraph), excerpt: p.text.clone(),
                        reason: format!("The named {} relationship must cite both {} and {}. Add the supplied endpoint citations or remove the relationship claim.", link.relation, link.from_id, link.to_id) });
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EvidenceView, PageSection, Paragraph};
    fn unit(excerpt: &str) -> KnowledgeView {
        KnowledgeView {
            id: "k".into(),
            revision_id: "r".into(),
            statement: excerpt.into(),
            topic: "policy".into(),
            topic_title: "Policy".into(),
            subject: "committee".into(),
            kind: "observation".into(),
            lifecycle: "unknown".into(),
            base_lifecycle: "unknown".into(),
            scope: "team".into(),
            effective_at: String::new(),
            support_state: "current_documentary_support".into(),
            relations: vec![],
            evidence: vec![EvidenceView {
                id: "e".into(),
                assertion_id: "a".into(),
                source_id: "s".into(),
                source: "minutes".into(),
                excerpt: excerpt.into(),
                captured_at: String::new(),
                active: true,
                ..EvidenceView::default()
            }],
        }
    }
    fn draft(text: &str) -> PageDraft {
        PageDraft {
            sections: vec![PageSection {
                heading: "Understanding".into(),
                paragraphs: vec![Paragraph {
                    text: text.into(),
                    knowledge_ids: vec!["k".into()],
                }],
            }],
        }
    }
    #[test]
    fn when_does_not_become_before_or_after() {
        let u = unit("The inquiry closed when the report was delivered.");
        for word in ["before", "after"] {
            let d = draft(&format!(
                "The inquiry closed {word} the report was delivered."
            ));
            let f = findings(&d, &[&u]);
            assert_eq!(f.len(), 1);
            f[0].validate(&d).unwrap();
            assert!(validate_prose(&d, &[&u]).is_err());
        }
        assert!(validate_prose(&draft(&u.statement), &[&u]).is_ok());
    }
    #[test]
    fn presentation_order_is_not_an_event_claim() {
        let u = unit("The committee approved a policy. The study was reported complete.");
        let mut d = draft("The committee approved a policy.");
        d.sections[0].paragraphs.push(Paragraph {
            text: "The study was reported complete.".into(),
            knowledge_ids: vec!["k".into()],
        });
        assert!(findings(&d, &[&u]).is_empty());
        d.sections[0].paragraphs.reverse();
        assert!(findings(&d, &[&u]).is_empty());
    }
    #[test]
    fn explicit_source_order_is_retained_but_sample_absence_is_not() {
        let u = unit("The committee approved the policy after consultation.");
        assert!(validate_prose(&draft("The committee approved the policy after consultation; implementation is not independently verified."), &[&u]).is_ok());
        assert!(validate_prose(&draft("The report gives no further detail."), &[&u]).is_err());
    }
    #[test]
    fn guessed_order_in_model_statement_is_not_original_evidence() {
        let mut u = unit("The inquiry closed when the report arrived.");
        u.statement = "The inquiry closed after the report arrived.".into();
        assert!(validate_prose(&draft(&u.statement), &[&u]).is_err());
    }

    #[test]
    fn a_plan_cannot_turn_its_unsettled_timetable_into_timeless_current_fact() {
        let mut u = unit("The rollout proposal says no implementation date has been agreed.");
        u.kind = "proposal".into();
        let bad = draft("No date has been agreed for the rollout.");
        assert_eq!(findings(&bad, &[&u]).len(), 1);
        let qualified =
            draft("The proposal said no implementation date had been agreed at the time.");
        assert!(findings(&qualified, &[&u]).is_empty());
        let mut current = u.clone();
        current.kind = "reported_outcome".into();
        assert!(findings(&bad, &[&current]).is_empty());
    }
}
