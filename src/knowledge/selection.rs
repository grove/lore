use super::*;
use crate::domain::documentary_basis;
use anyhow::{Context, ensure};
use std::collections::VecDeque;

/// Union semantic navigation signals with direct original-record retrieval.
/// An exact identifier/path/value never has to survive a parent summary first.
pub fn select(graph: &KnowledgeGraph, options: &ExploreOptions) -> Result<ExploreResult> {
    select_with_candidates(graph, options, &[])
}

pub(super) fn select_with_candidates(
    graph: &KnowledgeGraph,
    options: &ExploreOptions,
    additional: &[String],
) -> Result<ExploreResult> {
    ensure!(options.query.len() <= 4_000, "explore query is too long");
    ensure!(
        (512..=100_000).contains(&options.max_tokens),
        "explore token budget must be 512..=100000"
    );
    ensure!(
        (1..=256).contains(&options.max_nodes),
        "explore traversal budget must be 1..=256 nodes"
    );
    validate(graph)?;
    let records = record_map(graph);
    let nodes: BTreeMap<_, _> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let requested = options
        .node
        .as_deref()
        .map(|id| {
            nodes
                .get(id)
                .copied()
                .context("unknown view ID in the current snapshot")
        })
        .transpose()?;
    let allowed: BTreeSet<&str> = requested
        .map(|n| n.knowledge_ids.iter().map(String::as_str).collect())
        .unwrap_or_else(|| records.keys().copied().collect());
    let query_terms = terms(&options.query);
    let references = reference_terms(&options.query);
    let query_empty = options.query.trim().is_empty();
    let mut ranked = Vec::<(String, usize, bool)>::new();
    for record in &graph.knowledge {
        if !allowed.contains(record.id.as_str()) {
            continue;
        }
        let exact = exact_match(record, &options.query, &references);
        let metadata = terms(&format!("{} {}", record.subject, record.topic_title));
        let body = format!(
            "{} {}",
            record.statement,
            record
                .evidence
                .iter()
                .map(|e| e.excerpt.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        )
        .to_lowercase();
        let score = query_terms.intersection(&metadata).count() * 8
            + query_terms
                .iter()
                .filter(|term| body.contains(term.as_str()))
                .count()
            + usize::from(additional.contains(&record.id)) * 4;
        if query_empty || exact || score > 0 {
            ranked.push((record.id.clone(), score, exact));
        }
    }
    let has_exact = ranked.iter().any(|(_, _, exact)| *exact);
    // Precision requests first select exact original facts. Their required
    // conditions and related endpoints are added below, independent of ranks.
    if has_exact {
        ranked.retain(|(_, _, exact)| *exact);
    }
    ranked.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(b.1.cmp(&a.1))
            .then(critical(records[b.0.as_str()]).cmp(&critical(records[a.0.as_str()])))
            .then(a.0.cmp(&b.0))
    });
    let selected_node = if let Some(node) = requested {
        Some(node)
    } else if has_exact {
        ranked
            .iter()
            .find_map(|(id, _, _)| nodes.get(stable_id("record", id).as_str()).copied())
    } else if query_empty {
        nodes.get(graph.root_id.as_str()).copied()
    } else {
        graph
            .nodes
            .iter()
            .filter(|n| n.kind != "evidence")
            .filter_map(|node| {
                let words = terms(&format!("{} {}", node.title, node.grouping_basis.join(" ")));
                let matches = words.intersection(&query_terms).count();
                (matches > 0).then_some((node, matches))
            })
            .max_by(|(a, am), (b, bm)| {
                am.cmp(bm)
                    .then(b.knowledge_ids.len().cmp(&a.knowledge_ids.len()))
                    .then(b.id.cmp(&a.id))
            })
            .map(|(node, _)| node)
            // A sparse project can have a relevant original record without a
            // multi-record concept. Keep that result navigable without adding
            // unrelated conditions from the global root.
            .or_else(|| {
                ranked
                    .iter()
                    .find_map(|(id, _, _)| nodes.get(stable_id("record", id).as_str()).copied())
            })
    };
    if !has_exact
        && !query_empty
        && let Some(node) = selected_node
    {
        for id in &node.critical_knowledge_ids {
            if !ranked.iter().any(|(candidate, _, _)| candidate == id) {
                ranked.push((id.clone(), 0, false));
            }
        }
    }
    let requirements = Requirements::new(graph);
    let mut selection_work = 0;
    let mut selection_truncated = false;
    let mut groups = Vec::new();
    let mut eligible: BTreeSet<_> = ranked.iter().map(|(id, _, _)| id.clone()).collect();
    for (id, _, _) in &ranked {
        let Some(group) = complete_group(
            &records,
            &requirements,
            id,
            &mut selection_work,
            graph.options.max_work,
        ) else {
            selection_truncated = true;
            break;
        };
        eligible.extend(group.iter().cloned());
        if !groups.contains(&group) {
            groups.push(group);
        }
    }
    let (navigation, navigation_truncated) = navigation(graph, selected_node, options.max_nodes);
    let navigation_ids: BTreeSet<_> = navigation.iter().map(|n| n.id.as_str()).collect();
    let mut result = ExploreResult {
        schema_version: ZOOM_SCHEMA_VERSION,
        snapshot_revision: graph.revision.clone(),
        query: options.query.clone(),
        selected_node_id: selected_node.map(|n| n.id.clone()),
        retrieval: if has_exact {
            "direct_reference"
        } else {
            "cross_level"
        }
        .into(),
        status: "complete".into(),
        model_calls: 0,
        edges: graph
            .edges
            .iter()
            .filter(|e| {
                navigation_ids.contains(e.parent.as_str())
                    && navigation_ids.contains(e.child.as_str())
            })
            .cloned()
            .collect(),
        nodes: navigation,
        knowledge: Vec::new(),
        evidence: Vec::new(),
        relations: Vec::new(),
        source_revisions: Vec::new(),
        coverage: Coverage {
            eligible_units: eligible.len(),
            included_units: 0,
            omitted_units: eligible.len(),
            omission_reasons: Vec::new(),
        },
        critical_groups_omitted: 0,
        navigation_truncated: navigation_truncated || graph.report.truncated || selection_truncated,
        max_tokens: options.max_tokens,
        // Reserve the maximum numeric field width during incremental packing.
        used_tokens: options.max_tokens,
        warnings: graph.report.warnings.clone(),
    };
    result.warnings.push("Source records retain their documentary status; neither navigation grouping nor static evidence independently proves runtime behavior.".into());
    if selection_truncated {
        result.warnings.push("Selection reached its work budget. Only groups with complete critical-condition and relationship closure can be included.".into());
    }
    if graph.report.unsupported_units > 0 {
        result.warnings.push(format!("{} unsupported records were ineligible; an empty result does not establish that no relevant constraints exist.", graph.report.unsupported_units));
    }
    // Navigation is expendable when it competes with complete factual support.
    while measure(&result)? > options.max_tokens / 2 && !result.nodes.is_empty() {
        result.nodes.pop();
        result.navigation_truncated = true;
        trim_edges(&mut result);
    }
    let mut selected = BTreeSet::new();
    let mut omitted_critical = 0;
    for group in groups {
        let candidate: BTreeSet<_> = selected.union(&group).cloned().collect();
        let mut trial = result.clone();
        fill_payload(&mut trial, graph, &candidate);
        if measure(&trial)? <= options.max_tokens {
            selected = candidate;
            result = trial;
        } else if group.iter().any(|id| critical(records[id.as_str()])) {
            omitted_critical += 1;
        }
    }
    result.critical_groups_omitted = omitted_critical;
    result.coverage.omitted_units = eligible.len().saturating_sub(selected.len());
    if eligible.is_empty() {
        result.status = "no_matches".into();
        result.warnings.push("No eligible source-bound records matched this request. This does not establish the absence of relevant knowledge or constraints.".into());
    } else if result.coverage.omitted_units > 0 {
        result.status = if selected.is_empty() {
            "budget_limited"
        } else {
            "partial"
        }
        .into();
        result.coverage.omission_reasons.push("Complete evidence and attached critical conditions did not fit the output budget; omitted groups make no recommendation.".into());
    }
    // Final metadata can change tokenization. Remove complete groups, never an
    // exception or one endpoint from a retained relationship, if necessary.
    while measure(&result)? > options.max_tokens && !result.nodes.is_empty() {
        result.nodes.pop();
        result.navigation_truncated = true;
        trim_edges(&mut result);
    }
    if measure(&result)? > options.max_tokens {
        // No selective factual truncation: return an explicit scope limitation.
        fill_payload(&mut result, graph, &BTreeSet::new());
        result.status = "budget_limited".into();
        result.coverage.omitted_units = eligible.len();
        result.critical_groups_omitted = omitted_critical.max(usize::from(!eligible.is_empty()));
        result.coverage.omission_reasons = vec!["The complete source-bound group exceeds the output budget. Narrow the subject or increase --max-tokens.".into()];
        result.warnings.clear();
    }
    result.used_tokens = 0;
    for _ in 0..16 {
        let used = measure(&result)?;
        if used <= result.used_tokens {
            break;
        }
        result.used_tokens = used;
    }
    ensure!(
        measure(&result)? <= result.used_tokens && result.used_tokens <= options.max_tokens,
        "explore budget cannot contain its request and minimum result metadata"
    );
    Ok(result)
}

struct Requirements<'a> {
    topics: BTreeMap<&'a str, Vec<&'a str>>,
    subjects: BTreeMap<String, Vec<&'a str>>,
    related: BTreeMap<&'a str, Vec<&'a str>>,
}

impl<'a> Requirements<'a> {
    fn new(graph: &'a KnowledgeGraph) -> Self {
        let mut requirements = Self {
            topics: BTreeMap::new(),
            subjects: BTreeMap::new(),
            related: BTreeMap::new(),
        };
        for record in &graph.knowledge {
            if critical(record) {
                requirements
                    .topics
                    .entry(&record.topic)
                    .or_default()
                    .push(&record.id);
                requirements
                    .subjects
                    .entry(record.subject.to_lowercase())
                    .or_default()
                    .push(&record.id);
            }
        }
        for relation in &graph.relations {
            requirements
                .related
                .entry(&relation.from)
                .or_default()
                .push(&relation.to);
            requirements
                .related
                .entry(&relation.to)
                .or_default()
                .push(&relation.from);
        }
        requirements
    }
}

fn complete_group(
    records: &BTreeMap<&str, &KnowledgeView>,
    requirements: &Requirements<'_>,
    seed: &str,
    work: &mut usize,
    limit: usize,
) -> Option<BTreeSet<String>> {
    let mut selected = BTreeSet::from([seed.to_owned()]);
    let mut queue = VecDeque::from([seed.to_owned()]);
    let mut seen_topics = BTreeSet::new();
    let mut seen_subjects = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        let record = records[id.as_str()];
        let mut additions = Vec::new();
        if seen_topics.insert(record.topic.as_str()) {
            additions.extend(
                requirements
                    .topics
                    .get(record.topic.as_str())
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        let subject = record.subject.to_lowercase();
        if seen_subjects.insert(subject.clone()) {
            additions.extend(
                requirements
                    .subjects
                    .get(&subject)
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        additions.extend(
            requirements
                .related
                .get(id.as_str())
                .into_iter()
                .flatten()
                .copied(),
        );
        for candidate in additions {
            if *work >= limit {
                return None;
            }
            *work += 1;
            if selected.insert(candidate.to_owned()) {
                queue.push_back(candidate.to_owned());
            }
        }
    }
    Some(selected)
}

fn reference_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|c: char| {
                matches!(
                    c,
                    '`' | '"' | '\'' | '?' | ',' | ';' | '(' | ')' | '[' | ']'
                )
            })
        })
        .filter(|word| !word.is_empty() && word.len() <= 300)
        .filter(|word| {
            word.chars().any(|c| c.is_ascii_digit())
                || word.contains(['/', '_', '.'])
                || word.chars().filter(|c| c.is_ascii_uppercase()).count() >= 2
        })
        .take(32)
        .map(str::to_owned)
        .collect()
}

fn exact_match(record: &KnowledgeView, query: &str, references: &[String]) -> bool {
    let query = query.trim();
    if query == record.id || query == record.revision_id {
        return true;
    }
    let texts: Vec<&str> = std::iter::once(record.statement.as_str())
        .chain(record.evidence.iter().flat_map(|e| {
            [
                e.id.as_str(),
                e.source_revision_id.as_str(),
                e.source.as_str(),
                e.path.as_str(),
                e.excerpt.as_str(),
            ]
        }))
        .collect();
    if texts.contains(&query) && !query.is_empty() {
        return true;
    }
    references
        .iter()
        .any(|needle| texts.iter().any(|text| exact_fragment(text, needle)))
}

fn exact_fragment(text: &str, needle: &str) -> bool {
    text.match_indices(needle).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + needle.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

fn navigation(
    graph: &KnowledgeGraph,
    selected: Option<&ViewNode>,
    limit: usize,
) -> (Vec<NavigationNode>, bool) {
    let Some(selected) = selected else {
        return (Vec::new(), false);
    };
    let map: BTreeMap<_, _> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut queue = VecDeque::from([selected.id.as_str()]);
    queue.extend(selected.parent_view_ids.iter().map(String::as_str));
    let mut visited = BTreeSet::new();
    let mut result = Vec::new();
    let mut truncated = false;
    while let Some(id) = queue.pop_front() {
        if visited.contains(id) {
            continue;
        }
        if result.len() == limit {
            truncated = true;
            break;
        }
        visited.insert(id);
        let node = map[id];
        result.push(NavigationNode {
            id: node.id.clone(),
            revision: node.revision.clone(),
            title: node.title.clone(),
            kind: node.kind.clone(),
            summary: structural_summary(node),
            summary_knowledge_ids: Vec::new(),
            parent_view_ids: node.parent_view_ids.iter().take(limit).cloned().collect(),
            child_view_ids: node.child_view_ids.iter().take(limit).cloned().collect(),
            omitted_parent_nodes: node.parent_view_ids.len().saturating_sub(limit),
            omitted_child_nodes: node.child_view_ids.len().saturating_sub(limit),
            eligible_units: node.knowledge_ids.len(),
            critical_units: node.critical_knowledge_ids.len(),
        });
        // Visiting ancestors enables breadcrumbs but should not fan out into
        // all their unrelated siblings. Only expand the requested descendant DAG.
        if id == selected.id || !selected.parent_view_ids.iter().any(|parent| parent == id) {
            queue.extend(node.child_view_ids.iter().map(String::as_str));
        }
    }
    (result, truncated)
}

fn fill_payload(result: &mut ExploreResult, graph: &KnowledgeGraph, ids: &BTreeSet<String>) {
    for navigation in &mut result.nodes {
        let original = graph
            .nodes
            .iter()
            .find(|node| node.id == navigation.id)
            .unwrap();
        if original
            .summary_knowledge_ids
            .iter()
            .all(|id| ids.contains(id))
            && original
                .critical_knowledge_ids
                .iter()
                .all(|id| ids.contains(id))
        {
            navigation.summary = original.summary.clone();
            navigation.summary_knowledge_ids = original.summary_knowledge_ids.clone();
        } else {
            navigation.summary = structural_summary(original);
            navigation.summary_knowledge_ids.clear();
        }
    }
    result.knowledge = graph
        .knowledge
        .iter()
        .filter(|v| ids.contains(&v.id))
        .cloned()
        .collect();
    result.relations = graph
        .relations
        .iter()
        .filter(|r| ids.contains(&r.from) && ids.contains(&r.to))
        .cloned()
        .collect();
    let mut evidence_ids: BTreeSet<_> = result
        .knowledge
        .iter()
        .flat_map(|v| v.evidence.iter().map(|e| e.id.as_str()))
        .collect();
    evidence_ids.extend(result.relations.iter().map(|r| r.evidence_id.as_str()));
    result.evidence = graph
        .evidence
        .iter()
        .filter(|e| evidence_ids.contains(e.id.as_str()))
        .cloned()
        .collect();
    let source_ids: BTreeSet<_> = result
        .evidence
        .iter()
        .map(|e| e.source_revision_id.as_str())
        .collect();
    result.source_revisions = graph
        .source_revisions
        .iter()
        .filter(|s| source_ids.contains(s.id.as_str()))
        .cloned()
        .collect();
    result.coverage.included_units = ids.len();
    result.coverage.omitted_units = result.coverage.eligible_units.saturating_sub(ids.len());
}

fn structural_summary(node: &ViewNode) -> String {
    format!(
        "{} source-bound records; {} retain critical decisions, constraints, exceptions or history. Open this concept for a bounded answer with complete original evidence.",
        node.knowledge_ids.len(),
        node.critical_knowledge_ids.len()
    )
}

fn trim_edges(result: &mut ExploreResult) {
    let ids: BTreeSet<_> = result.nodes.iter().map(|n| n.id.as_str()).collect();
    result
        .edges
        .retain(|edge| ids.contains(edge.parent.as_str()) && ids.contains(edge.child.as_str()));
}

fn measure(result: &ExploreResult) -> Result<usize> {
    Ok(
        crate::context::count_tokens(&(serde_json::to_string_pretty(result)? + "\n"))
            .max(crate::context::count_tokens(&render_markdown(result))),
    )
}

fn md(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn anchor(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .collect()
}

/// Portable, complete Markdown. It renders the same selected source records,
/// conditions and omissions as JSON, including exact evidence and revision IDs.
pub fn render_markdown(result: &ExploreResult) -> String {
    let mut out = format!(
        "# Explore project knowledge\n\nStatus: **{}** · Retrieval: **{}**\n\n",
        result.status, result.retrieval
    );
    if !result.query.is_empty() {
        out.push_str(&format!("Request: {}\n\n", md(&result.query)));
    }
    if !result.knowledge.is_empty() {
        out.push_str("## Source-bound knowledge\n\n");
        for view in &result.knowledge {
            out.push_str(&format!(
                "### {}\n\n{}\n\n**{} · {} · {}**. Scope: {}. {}\n\n",
                md(&view.subject),
                md(&view.statement),
                view.kind,
                view.lifecycle,
                view.support_state,
                md(&view.scope),
                documentary_basis(&view.kind)
            ));
            if !view.effective_at.is_empty() {
                out.push_str(&format!(
                    "Effective/occurrence time recorded by the source: {}.\n\n",
                    md(&view.effective_at)
                ));
            }
            for e in &view.evidence {
                out.push_str(&format!(
                    "- [{}](#{}) — {}; {}; {} evidence.\n",
                    e.id,
                    anchor(&e.id),
                    md(&e.source),
                    e.material.as_str(),
                    if e.active {
                        "current documentary"
                    } else {
                        "historical"
                    }
                ));
            }
            out.push_str(&format!(
                "\nKnowledge `{}` · revision `{}`.\n\n",
                view.id, view.revision_id
            ));
        }
    }
    if !result.relations.is_empty() {
        out.push_str("## Decisions and relationships\n\n");
        for relation in &result.relations {
            out.push_str(&format!(
                "- `{}` {} `{}` — {} support, [{}](#{}).\n",
                relation.from,
                relation.kind,
                relation.to,
                if relation.active {
                    "current documentary"
                } else {
                    "historical"
                },
                relation.evidence_id,
                anchor(&relation.evidence_id)
            ));
        }
        out.push('\n');
    }
    if !result.nodes.is_empty() {
        out.push_str("## Explore related concepts\n\n");
        for node in &result.nodes {
            out.push_str(&format!(
                "### {}\n\n{}\n\nOpen with `lore explore --node {}`.\n\n",
                md(&node.title),
                md(&node.summary),
                node.id
            ));
            if node.omitted_child_nodes > 0 {
                out.push_str(&format!(
                    "{} additional child nodes are outside this navigation budget.\n\n",
                    node.omitted_child_nodes
                ));
            }
        }
    }
    if !result.evidence.is_empty() {
        out.push_str("## Original evidence\n\n");
        for evidence in &result.evidence {
            out.push_str(&format!(
                "### {}\n\n{}:{} · source revision `{}` · excerpt digest `{}`\n\n{}\n\n",
                anchor(&evidence.id),
                md(&evidence.root_id),
                md(&evidence.path),
                evidence.source_revision_id,
                evidence.digest,
                evidence.material.qualification()
            ));
            if let (Some(start), Some(end)) = (evidence.line_start, evidence.line_end) {
                out.push_str(&format!("Captured lines {start}–{end}.\n\n"));
            }
            if !evidence.context_before.is_empty() {
                out.push_str(&format!(
                    "Context before:\n\n> {}\n\n",
                    md(&evidence.context_before).replace('\n', "\n> ")
                ));
            }
            out.push_str(&format!(
                "> {}\n\n",
                md(&evidence.excerpt).replace('\n', "\n> ")
            ));
            if !evidence.context_after.is_empty() {
                out.push_str(&format!(
                    "Context after:\n\n> {}\n\n",
                    md(&evidence.context_after).replace('\n', "\n> ")
                ));
            }
        }
    }
    out.push_str(&format!(
        "## Coverage\n\n{} of {} eligible records included; {} omitted. Snapshot `{}`.\n\n",
        result.coverage.included_units,
        result.coverage.eligible_units,
        result.coverage.omitted_units,
        result.snapshot_revision
    ));
    for warning in result
        .coverage
        .omission_reasons
        .iter()
        .chain(&result.warnings)
    {
        out.push_str(&format!("- {}\n", md(warning)));
    }
    if result.navigation_truncated {
        out.push_str("- Navigation was bounded. Exact original-record retrieval is independent of the view hierarchy.\n");
    }
    out
}
