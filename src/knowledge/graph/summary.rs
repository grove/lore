//! Share original-record formatting and token counts across overlapping views.
//! A cached summary is checked against the exact deterministic source plan;
//! checking streams its fragments instead of rebuilding a summary string.

use super::*;
use std::cell::RefCell;

const OMITTED: &str =
    "Representative summary; full critical records are required during answer selection.";
const FOOTER: &str = "\n\nThis is an extractive documentary view; related conditions outside this concept are included during answer selection.";

pub(in crate::knowledge) struct Summaries<'a> {
    pub records: BTreeMap<&'a str, &'a KnowledgeView>,
    descriptions: BTreeMap<&'a str, String>,
    leaves: BTreeMap<&'a str, String>,
    tokens: RefCell<BTreeMap<&'a str, usize>>,
    critical_tokens: RefCell<BTreeMap<Vec<String>, usize>>,
}

pub(in crate::knowledge) struct SummaryPlan<'a> {
    parts: Vec<&'a str>,
    suffix: String,
    pub knowledge_ids: Vec<String>,
    eligible: usize,
}

impl<'a> Summaries<'a> {
    pub fn new(graph: &'a KnowledgeGraph) -> Self {
        let records = record_map(graph);
        let mut descriptions = BTreeMap::new();
        let mut leaves = BTreeMap::new();
        for (id, record) in &records {
            descriptions.insert(
                *id,
                format!(
                    "{} [{}; {}; {}; scope {}; evidence {}]: {}",
                    record.subject,
                    record.kind,
                    record.lifecycle,
                    record.support_state,
                    record.scope,
                    record
                        .evidence
                        .iter()
                        .map(|e| e.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    record.statement,
                ),
            );
            leaves.insert(
                *id,
                format!(
                    "{}; {}; {}; scope {}: {}",
                    record.kind,
                    record.lifecycle,
                    record.support_state,
                    record.scope,
                    record.statement,
                ),
            );
        }
        Self {
            records,
            descriptions,
            leaves,
            tokens: RefCell::new(BTreeMap::new()),
            critical_tokens: RefCell::new(BTreeMap::new()),
        }
    }

    fn tokens(&self, id: &str) -> usize {
        let mut tokens = self.tokens.borrow_mut();
        *tokens
            .entry(self.records[id].id.as_str())
            .or_insert_with(|| crate::context::count_tokens(&self.descriptions[id]))
    }

    pub fn plan(
        &self,
        kind: &str,
        ids: &BTreeSet<String>,
        critical_ids: &[String],
    ) -> SummaryPlan<'_> {
        if kind == "evidence" {
            let id = ids.iter().next().expect("validated evidence leaf");
            return SummaryPlan {
                parts: vec![&self.leaves[id.as_str()]],
                suffix: String::new(),
                knowledge_ids: vec![id.clone()],
                eligible: ids.len(),
            };
        }
        let fallback = || SummaryPlan {
            parts: Vec::new(),
            suffix: format!(
                "This concept contains {} source-bound records, including {} complete decisions, constraints, exceptions or historical transitions. Their required qualifications exceed the summary budget. Open the concept to select a complete bounded group or follow an exact source reference.",
                ids.len(),
                critical_ids.len(),
            ),
            knowledge_ids: Vec::new(),
            eligible: ids.len(),
        };
        let mut parts = Vec::new();
        let mut bytes = 0;
        for id in critical_ids {
            let text = self.descriptions[id.as_str()].as_str();
            bytes += text.len();
            if bytes > 16_000 {
                return fallback();
            }
            parts.push(text);
        }
        let mut used = {
            let mut costs = self.critical_tokens.borrow_mut();
            *costs
                .entry(critical_ids.to_vec())
                .or_insert_with(|| crate::context::count_tokens(&parts.join("\n\n")))
        };
        if used > 1_200 {
            return fallback();
        }
        let mut selected = critical_ids.to_vec();
        for id in ids {
            if selected.contains(id) {
                continue;
            }
            let text = self.descriptions[id.as_str()].as_str();
            if text.len() > 16_000 {
                continue;
            }
            let cost = self.tokens(id);
            if used + cost > 1_200 {
                continue;
            }
            selected.push(id.clone());
            parts.push(text);
            used += cost;
            if selected.len() >= critical_ids.len() + 3 {
                break;
            }
        }
        let mut suffix = if selected.len() < ids.len() {
            format!(
                "\n\n{} additional original records remain available below.",
                ids.len() - selected.len()
            )
        } else {
            String::new()
        };
        if parts.is_empty() && suffix.is_empty() {
            suffix = format!(
                "{} source-bound records. Complete passages exceed the compact summary budget; use the detailed records below.",
                ids.len(),
            );
        }
        suffix.push_str(FOOTER);
        SummaryPlan {
            parts,
            suffix,
            knowledge_ids: selected,
            eligible: ids.len(),
        }
    }

    pub fn validate(&self, node: &ViewNode) -> Result<()> {
        let ids = node.knowledge_ids.iter().cloned().collect();
        let plan = self.plan(&node.kind, &ids, &node.critical_knowledge_ids);
        ensure!(
            plan.knowledge_ids == node.summary_knowledge_ids
                && plan.coverage() == node.summary_coverage
                && plan.matches(&node.summary),
            "cached summary differs from its complete original-source plan"
        );
        Ok(())
    }
}

impl SummaryPlan<'_> {
    pub fn coverage(&self) -> Coverage {
        Coverage {
            eligible_units: self.eligible,
            included_units: self.knowledge_ids.len(),
            omitted_units: self.eligible - self.knowledge_ids.len(),
            omission_reasons: if self.knowledge_ids.len() < self.eligible {
                vec![OMITTED.into()]
            } else {
                Vec::new()
            },
        }
    }

    pub fn render(&self) -> String {
        let mut text = self.parts.join("\n\n");
        text.push_str(&self.suffix);
        text
    }

    fn matches(&self, text: &str) -> bool {
        let mut expected = blake3::Hasher::new();
        for (index, part) in self.parts.iter().enumerate() {
            if index > 0 {
                expected.update(b"\n\n");
            }
            expected.update(part.as_bytes());
        }
        expected.update(self.suffix.as_bytes());
        expected.finalize() == blake3::hash(text.as_bytes())
    }
}
