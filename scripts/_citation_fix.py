from pathlib import Path
p=Path('.')
def edit(path, old, new):
    f=p/path
    s=f.read_text()
    assert s.count(old)==1, (path, old[:80], s.count(old))
    f.write_text(s.replace(old,new))
edit('src/domain.rs','pub fn verification_schema() -> Value {','''/// A citation is a foreign key into this request, not arbitrary generated text.
/// Keep runtime validation as well: not every provider enforces its schema.
pub fn page_schema_for(allowed: &std::collections::BTreeSet<String>) -> Result<Value> {
    ensure!(!allowed.is_empty(), "cannot synthesize without citable knowledge");
    let mut schema = page_schema();
    schema["properties"]["sections"]["items"]["properties"]["paragraphs"]["items"]
        ["properties"]["knowledge_ids"] = json!({
            "type": "array",
            "minItems": 1,
            "description": "Cite only IDs from the supplied knowledge rows. Evidence IDs, assertion IDs, document names and context-only relationship endpoints are not citations.",
            "items": {"type": "string", "enum": allowed}
        });
    Ok(schema)
}
pub fn verification_schema() -> Value {''')
edit('src/engine.rs','mod overview;','mod citations;\nmod overview;')
edit('src/engine.rs','    pub configuration_changed: bool,','    pub configuration_changed: bool,\n    pub presentation_changed: bool,')
edit('src/engine.rs','    let mut warnings = inventory.warnings.clone();','''    let presentation_changed = match conn {
        Some(c) if initialized => storage::meta(c, "presentation_contract")?.as_deref()
            != Some(citations::CONTRACT_VERSION),
        _ => false,
    };
    let mut warnings = inventory.warnings.clone();''')
edit('src/engine.rs','            || configuration_changed\n            || pending','            || configuration_changed\n            || presentation_changed\n            || pending')
edit('src/engine.rs','        configuration_changed,\n        output_modified,','        configuration_changed,\n        presentation_changed,\n        output_modified,')
edit('src/engine.rs','        ("initialized", "1"),','        ("initialized", "1"),\n        ("presentation_contract", citations::CONTRACT_VERSION),')
edit('src/engine.rs','        options.rebuild || options.refresh || options.deep || plan.status.configuration_changed,','''        options.rebuild || options.refresh || options.deep
            || plan.status.configuration_changed || plan.status.presentation_changed,''')
edit('src/engine/render.rs','use super::{overview, runner::Runner, timeline};','use super::{citations, overview, runner::Runner, timeline};')
edit('src/engine/render.rs','            "page-v2",','            citations::CONTRACT_VERSION,')
edit('src/engine/render.rs','            let mut accepted = None;','            let config = runner.config;\n            let run = runner.run;\n            let mut accepted = None;')
edit('src/engine/render.rs','                        domain::page_schema(),\n                        |p: &mut PageDraft| validate_draft(p, &allowed),','''                        domain::page_schema_for(&allowed)?,
                        |p: &mut PageDraft| {
                            citations::validate(p, &allowed, "synthesize", config, run)?;
                            validate_draft(p, &allowed)
                        },''')
edit('src/engine/render.rs','Every paragraph must name supporting knowledge_ids from this batch; cover every supplied ID at least once.','Every paragraph must name supporting knowledge_ids from this batch; cover every supplied ID at least once. Copy knowledge[].id exactly and choose only values in the knowledge_ids schema enum. Nested evidence/assertion IDs and relationship endpoints not in this batch are context, not citable knowledge.')
f=p/'src/engine/overview.rs'
s=f.read_text();start=s.index('fn select(');end=s.index('fn validate(',start)
s=s[:start]+'''fn select<'a>(
    knowledge: &'a [KnowledgeView],
    decisions: &[DecisionLink],
    budget: usize,
) -> Result<Vec<&'a KnowledgeView>> {
    let mut topics: BTreeMap<&str, Vec<&KnowledgeView>> = BTreeMap::new();
    let mut by_id = BTreeMap::new();
    for u in knowledge {
        ensure!(by_id.insert(u.id.as_str(), u).is_none(), "duplicate knowledge identity");
        topics.entry(&u.topic).or_default().push(u);
    }
    for values in topics.values_mut() {
        values.sort_by(|a, b| rank(a).cmp(&rank(b)));
    }
    // Context and citable evidence must be closed over the same set. A historic
    // predecessor can rank below the eight representative rows from its topic;
    // still include it if a decision link names it. Never merely allow its ID
    // in validation without supplying its record and evidence to the model.
    let mut mandatory = BTreeSet::new();
    for values in topics.values() {
        mandatory.insert(values[0].id.as_str());
    }
    for link in decisions {
        for id in [&link.from_id, &link.to_id] {
            ensure!(by_id.contains_key(id.as_str()), "overview relationship endpoint is absent from knowledge");
            mandatory.insert(id.as_str());
        }
    }
    let mut selected = Vec::new();
    let mut included = BTreeSet::new();
    let mut bytes = 2;
    for id in mandatory {
        let u = by_id[id];
        let cost = serde_json::to_vec(&row(u))?.len() + 1;
        ensure!(bytes + cost <= budget,
            "overview cannot fit every topic and its decision endpoint evidence within max_context_bytes; increase the explicit budget");
        selected.push(u);
        included.insert(id);
        bytes += cost;
    }
    for depth in 0..8 {
        for values in topics.values() {
            if let Some(u) = values.get(depth) {
                if included.contains(u.id.as_str()) { continue; }
                let cost = serde_json::to_vec(&row(u))?.len() + 1;
                if bytes + cost > budget { continue; }
                selected.push(*u);
                included.insert(u.id.as_str());
                bytes += cost;
            }
        }
    }
    Ok(selected)
}
''' + s[end:]
f.write_text(s)
edit('src/engine/overview.rs','use super::{runner::Runner, timeline::DecisionLink};','use super::{citations, runner::Runner, timeline::DecisionLink};')
edit('src/engine/overview.rs','        "overview-v1",','        citations::CONTRACT_VERSION,')
edit('src/engine/overview.rs','        let selected = select(knowledge, budget / 2 - overhead)?;','''        let selected = select(knowledge, decisions, budget / 2 - overhead)?;
        let allowed: BTreeSet<String> = selected.iter().map(|u| u.id.clone()).collect();
        let config = runner.config;
        let run = runner.run;''')
edit('src/engine/overview.rs','                    domain::page_schema(),\n                    |d: &mut PageDraft| validate(d, &selected),','''                    domain::page_schema_for(&allowed)?,
                    |d: &mut PageDraft| {
                        citations::validate(d, &allowed, "overview", config, run)?;
                        validate(d, &selected)
                    },''')
edit('src/engine/overview.rs','Every paragraph must cite supporting knowledge_ids from the supplied records.','Every paragraph must cite supporting knowledge_ids from the supplied records. Copy knowledge[].id exactly and choose only values allowed by the knowledge_ids schema enum. Evidence IDs, assertion IDs, revision IDs and source-document names are not knowledge_ids. All decision relationship endpoints have corresponding supplied knowledge records; use those records for citations.')
with (p/'src/engine/overview.rs').open('a') as f:
    f.write('''
#[cfg(test)]
mod selection_contracts {
    use super::*;
    use crate::domain::EvidenceView;
    fn unit(id: &str, topic: &str, historical: bool) -> KnowledgeView {
        KnowledgeView {
            id:id.into(), revision_id:format!("r_{id}"), statement:format!("Documented decision {id}"),
            topic:topic.into(), topic_title:topic.into(), subject:topic.into(),
            kind:"decision".into(), lifecycle:if historical { "superseded" } else { "accepted" }.into(),
            base_lifecycle:"accepted".into(), scope:"production".into(), effective_at:String::new(),
            support_state:"current_documentary_support".into(), relations:vec![],
            evidence:vec![EvidenceView {
                id:format!("ev_{id}"), assertion_id:format!("as_{id}"), source_id:"source".into(),
                source:"docs:ADR-001.md".into(), excerpt:format!("Exact original text for {id}"),
                captured_at:"2026-10-09".into(), active:true,
            }],
        }
    }
    fn link(from: &KnowledgeView, to: &KnowledgeView) -> DecisionLink {
        DecisionLink {
            relation:"supersedes".into(), from_id:from.id.clone(), to_id:to.id.clone(),
            from_statement:from.statement.clone(), to_statement:to.statement.clone(),
            from_topic:from.topic.clone(), to_topic:to.topic.clone(),
            from_title:from.topic_title.clone(), to_title:to.topic_title.clone(),
            from_label:"ADR-027".into(), to_label:"ADR-001".into(),
            claimed_effective_at:None, supporting_source:Some("docs:ADR-027.md".into()),
            evidence_id:Some("ev_replacement".into()),
        }
    }
    #[test]
    fn historical_endpoint_survives_representative_depth_limit() {
        let mut units = (0..12).map(|i| unit(&format!("ku_{i:02}"), "ledger", false))
            .collect::<Vec<_>>();
        let old = unit("ku_historical", "ledger", true);
        let next = unit("ku_successor", "migration", false);
        let edge = link(&next, &old);
        units.push(old); units.push(next);
        let selected = select(&units, &[edge.clone()], 100_000).unwrap();
        let ids = selected.iter().map(|u| u.id.as_str()).collect::<BTreeSet<_>>();
        assert!(ids.contains(edge.from_id.as_str()));
        assert!(ids.contains(edge.to_id.as_str()));
        assert!(selected.iter().all(|u| !u.evidence.is_empty()));
        assert_eq!(ids.len(), selected.len());
        assert!(units.iter().filter(|u| u.topic=="ledger" && u.lifecycle!="superseded").count()>8);
    }
    #[test]
    fn insufficient_budget_or_dangling_relationship_is_an_explicit_error() {
        let old=unit("ku_old", "ledger", true);
        let next=unit("ku_next", "migration", false);
        let edge=link(&next, &old);
        assert!(select(&[old.clone(),next.clone()], &[edge.clone()], 1).unwrap_err()
            .to_string().contains("cannot fit every topic"));
        assert!(select(&[next], &[edge], 100_000).unwrap_err()
            .to_string().contains("endpoint is absent"));
    }
}
''')
