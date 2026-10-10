use super::*;
use anyhow::{Context, ensure};
use std::collections::VecDeque;

mod dependencies;
mod reuse;
mod summary;
pub(super) use dependencies::Dependencies;
pub(super) use reuse::{grouping_signature, validate_reusable_topology};
pub(super) use summary::Summaries;

#[derive(Clone)]
struct Group {
    ids: BTreeSet<String>,
    labels: BTreeSet<String>,
}

struct Work {
    used: usize,
    limit: usize,
    exhausted: bool,
}
impl Work {
    fn take(&mut self, amount: usize) -> bool {
        if amount > self.limit.saturating_sub(self.used) {
            self.exhausted = true;
            return false;
        }
        self.used += amount;
        true
    }
}

pub(super) fn assemble(graph: &mut KnowledgeGraph, dependencies: &Dependencies) -> Result<()> {
    let layout = derive_layout(graph);
    assemble_from_layout(graph, layout, dependencies)
}

/// The canonical navigation plan contains no summary text or view revisions.
/// Cache validation derives this same bounded plan before reusing view objects.
pub(super) struct Layout {
    nodes: Vec<LayoutNode>,
    edges: Vec<ViewEdge>,
    pub(super) report: BuildReport,
}

struct LayoutNode {
    id: String,
    kind: &'static str,
    title: String,
    grouping_basis: Vec<String>,
    ids: BTreeSet<String>,
}

pub(super) fn derive_layout(graph: &KnowledgeGraph) -> Layout {
    let all_ids: BTreeSet<_> = graph.knowledge.iter().map(|v| v.id.clone()).collect();
    let mut work = Work {
        used: 0,
        limit: graph.options.max_work,
        exhausted: false,
    };
    // Preserve room for semantic navigation when original records outnumber
    // the node budget. Original facts remain directly retrievable whether or
    // not they have a navigation leaf. Unused group capacity returns to leaves.
    let available_nodes = graph.options.max_nodes - 1;
    let reserved_groups = available_nodes / 4;
    let reserved_leaves = graph.knowledge.len().min(available_nodes - reserved_groups);
    let group_limit = available_nodes - reserved_leaves;
    let mut groups = BTreeMap::<Vec<String>, Group>::new();
    let candidates = candidate_memberships(graph, &mut work);
    groups.extend(seed_groups(&candidates, all_ids.len(), group_limit));
    // Close useful overlaps under intersection, within a work budget. Strict
    // set containment creates arbitrary meaningful depth; no level counter or
    // fixed number of presentation categories controls the stored structure.
    let mut queue: Vec<Group> = groups.values().cloned().collect();
    let mut cursor = 0;
    'intersections: while cursor < queue.len() && groups.len() < group_limit {
        let left = queue[cursor].clone();
        for right_index in 0..cursor {
            let right = &queue[right_index];
            if !work.take(left.ids.len().min(right.ids.len()).max(1)) {
                break 'intersections;
            }
            let ids: BTreeSet<_> = left.ids.intersection(&right.ids).cloned().collect();
            if ids.len() < 2 || ids.len() == left.ids.len() || ids.len() == right.ids.len() {
                continue;
            }
            let key = ids.iter().cloned().collect::<Vec<_>>();
            if groups.contains_key(&key) {
                continue;
            }
            let group = Group {
                ids,
                labels: left.labels.union(&right.labels).cloned().collect(),
            };
            groups.insert(key, group.clone());
            queue.push(group);
            if groups.len() >= group_limit {
                break 'intersections;
            }
        }
        cursor += 1;
    }
    let leaf_count = graph.knowledge.len().min(available_nodes - groups.len());
    let mut nodes = vec![LayoutNode {
        id: graph.root_id.clone(),
        kind: "project",
        title: "Project knowledge".into(),
        grouping_basis: vec!["all eligible documentary knowledge".into()],
        ids: all_ids,
    }];
    for group in groups.values() {
        let member_key = group.ids.iter().cloned().collect::<Vec<_>>().join("\n");
        let title = group_title(&group.labels);
        nodes.push(LayoutNode {
            id: stable_id("members", &member_key),
            kind: "concept",
            title,
            grouping_basis: group.labels.iter().cloned().collect(),
            ids: group.ids.clone(),
        });
    }
    let mut leaves: Vec<_> = graph.knowledge.iter().collect();
    leaves.sort_by(|a, b| critical(b).cmp(&critical(a)).then(a.id.cmp(&b.id)));
    for view in leaves.into_iter().take(leaf_count) {
        nodes.push(LayoutNode {
            id: stable_id("record", &view.id),
            kind: "evidence",
            title: view.subject.clone(),
            grouping_basis: vec!["original knowledge record".into()],
            ids: BTreeSet::from([view.id.clone()]),
        });
    }
    // Nearest strict supersets are the immediate semantic parents. Overlapping
    // incomparable groups both remain parents of a shared lower-level concept.
    let members: Vec<_> = nodes.iter().map(|node| &node.ids).collect();
    let mut group_indices: Vec<_> = (1..=groups.len()).collect();
    group_indices.sort_by_key(|index| (members[*index].len(), nodes[*index].id.clone()));
    let mut edges = BTreeSet::new();
    for child in 1..nodes.len() {
        let mut parents = Vec::<usize>::new();
        for &parent in &group_indices {
            if parent == child || members[parent].len() <= members[child].len() {
                continue;
            }
            if !work.take(members[child].len().max(1)) {
                break;
            }
            if !members[child].is_subset(members[parent]) {
                continue;
            }
            if parents
                .iter()
                .any(|previous| members[*previous].is_subset(members[parent]))
            {
                continue;
            }
            // Reserve at least one edge for each remaining node so resource
            // pressure degrades to a connected flat index, never orphaned data.
            let reserved = nodes.len() - child - 1;
            if edges.len() + parents.len() + 1 + reserved > graph.options.max_edges {
                work.exhausted = true;
                break;
            }
            parents.push(parent);
        }
        if parents.is_empty() {
            parents.push(0);
        }
        for parent in parents {
            edges.insert(ViewEdge {
                parent: nodes[parent].id.clone(),
                child: nodes[child].id.clone(),
                kind: "contains".into(),
            });
        }
    }
    let omitted_navigation_units = graph.knowledge.len() - leaf_count;
    let mut report = BuildReport {
        work_used: work.used,
        work_limit: graph.options.max_work,
        truncated: work.exhausted || omitted_navigation_units > 0,
        omitted_navigation_units,
        unsupported_units: graph.report.unsupported_units,
        warnings: Vec::new(),
    };
    if work.exhausted {
        report.warnings.push(
            "Navigation grouping reached its work/edge budget; original records remain directly searchable.".into(),
        );
    }
    if omitted_navigation_units > 0 {
        report.warnings.push(
            "Some original records have no dedicated navigation node because of the node budget; direct retrieval still includes them.".into(),
        );
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    Layout {
        nodes,
        edges: edges.into_iter().collect(),
        report,
    }
}

pub(super) fn assemble_from_layout(
    graph: &mut KnowledgeGraph,
    layout: Layout,
    dependencies: &Dependencies,
) -> Result<()> {
    let summaries = Summaries::new(graph);
    let mut nodes = layout
        .nodes
        .into_iter()
        .map(|node| {
            make_node(
                node.id,
                node.kind,
                node.title,
                node.grouping_basis,
                &node.ids,
                &summaries,
            )
        })
        .collect::<Vec<_>>();
    let edges = layout.edges;
    for node in &mut nodes {
        node.child_view_ids = edges
            .iter()
            .filter(|edge| edge.parent == node.id)
            .map(|edge| edge.child.clone())
            .collect();
        node.parent_view_ids = edges
            .iter()
            .filter(|edge| edge.child == node.id)
            .map(|edge| edge.parent.clone())
            .collect();
        node.revision = dependencies.revision(node)?;
    }
    graph.nodes = nodes;
    graph.edges = edges;
    graph.report = layout.report;
    Ok(())
}

fn candidate_memberships(
    graph: &KnowledgeGraph,
    work: &mut Work,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut candidates = BTreeMap::<String, BTreeSet<String>>::new();
    for view in &graph.knowledge {
        if !work.take(2) {
            break;
        }
        candidates
            .entry(format!("topic: {}", view.topic_title))
            .or_default()
            .insert(view.id.clone());
        candidates
            .entry(format!("subject: {}", view.subject))
            .or_default()
            .insert(view.id.clone());
        for term in terms(&format!("{} {}", view.subject, view.topic_title)) {
            if !work.take(1) {
                break;
            }
            candidates
                .entry(format!("concept: {term}"))
                .or_default()
                .insert(view.id.clone());
        }
    }
    // Documentary relationships supply additional semantic memberships across
    // topic boundaries. They do not become containment edges themselves.
    for relation in &graph.relations {
        if !work.take(1) {
            break;
        }
        candidates
            .entry(format!("documented {}: {}", relation.kind, relation.id))
            .or_default()
            .extend([relation.from.clone(), relation.to.clone()]);
    }
    candidates
}

fn seed_groups(
    candidates: &BTreeMap<String, BTreeSet<String>>,
    all_count: usize,
    group_limit: usize,
) -> BTreeMap<Vec<String>, Group> {
    let mut groups = BTreeMap::<Vec<String>, Group>::new();
    for (label, ids) in candidates {
        if ids.len() < 2 || ids.len() == all_count {
            continue;
        }
        let key = ids.iter().cloned().collect::<Vec<_>>();
        if let Some(group) = groups.get_mut(&key) {
            group.labels.insert(label.clone());
        } else if groups.len() < group_limit {
            groups.insert(
                key,
                Group {
                    ids: ids.clone(),
                    labels: BTreeSet::from([label.clone()]),
                },
            );
        }
    }
    groups
}

fn group_title(labels: &BTreeSet<String>) -> String {
    let title = labels
        .iter()
        .find(|label| label.starts_with("topic: "))
        .or_else(|| labels.iter().find(|label| label.starts_with("subject: ")))
        .map(|label| label.split_once(": ").unwrap().1.to_owned())
        .unwrap_or_else(|| {
            labels
                .iter()
                .filter_map(|label| label.strip_prefix("concept: "))
                .take(4)
                .collect::<Vec<_>>()
                .join(" · ")
        });
    if title.is_empty() {
        "Documented relationship".into()
    } else {
        title
    }
}

fn make_node(
    id: String,
    kind: &str,
    title: String,
    basis: Vec<String>,
    ids: &BTreeSet<String>,
    summaries: &Summaries<'_>,
) -> ViewNode {
    let critical_ids: Vec<_> = ids
        .iter()
        .filter(|id| critical(summaries.records[id.as_str()]))
        .cloned()
        .collect();
    let plan = summaries.plan(kind, ids, &critical_ids);
    ViewNode {
        id,
        revision: String::new(),
        kind: kind.into(),
        title,
        summary: plan.render(),
        summary_coverage: plan.coverage(),
        summary_knowledge_ids: plan.knowledge_ids,
        grouping_basis: basis,
        knowledge_ids: ids.iter().cloned().collect(),
        critical_knowledge_ids: critical_ids,
        child_view_ids: Vec::new(),
        parent_view_ids: Vec::new(),
        coverage: Coverage {
            eligible_units: ids.len(),
            included_units: ids.len(),
            omitted_units: 0,
            omission_reasons: Vec::new(),
        },
    }
}

pub(super) fn refresh_node(
    node: &mut ViewNode,
    summaries: &Summaries<'_>,
    dependencies: &Dependencies,
) -> Result<()> {
    let ids = node.knowledge_ids.iter().cloned().collect();
    let mut updated = make_node(
        node.id.clone(),
        &node.kind,
        node.title.clone(),
        node.grouping_basis.clone(),
        &ids,
        summaries,
    );
    updated.child_view_ids = node.child_view_ids.clone();
    updated.parent_view_ids = node.parent_view_ids.clone();
    updated.revision = dependencies.revision(&updated)?;
    *node = updated;
    Ok(())
}

/// Validate containment separately from documentary relationships. The latter
/// may legitimately contain cycles; view dependencies may not.
pub fn validate(graph: &KnowledgeGraph) -> Result<()> {
    validate_with_dependencies(graph, &Dependencies::new(graph)?)
}

pub(super) fn validate_with_dependencies(
    graph: &KnowledgeGraph,
    dependencies: &Dependencies,
) -> Result<()> {
    validate_inner(graph, Some(dependencies))
}

pub(super) fn validate_structure(graph: &KnowledgeGraph) -> Result<()> {
    validate_inner(graph, None)
}

fn validate_inner(graph: &KnowledgeGraph, dependencies: Option<&Dependencies>) -> Result<()> {
    ensure!(
        graph.schema_version == ZOOM_SCHEMA_VERSION,
        "unknown zoom schema"
    );
    ensure!(
        graph.nodes.len() <= graph.options.max_nodes,
        "zoom node budget exceeded"
    );
    ensure!(
        graph.edges.len() <= graph.options.max_edges,
        "zoom edge budget exceeded"
    );
    let nodes: BTreeMap<_, _> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    ensure!(nodes.len() == graph.nodes.len(), "duplicate view identity");
    ensure!(
        nodes.contains_key(graph.root_id.as_str()),
        "missing project root"
    );
    let records = record_map(graph);
    ensure!(
        records.len() == graph.knowledge.len(),
        "duplicate original record"
    );
    let evidence: BTreeMap<_, _> = graph.evidence.iter().map(|e| (e.id.as_str(), e)).collect();
    ensure!(
        evidence.len() == graph.evidence.len(),
        "duplicate evidence identity"
    );
    let source_manifests: BTreeMap<_, _> = graph
        .source_revisions
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();
    for e in &graph.evidence {
        let source = source_manifests
            .get(e.source_revision_id.as_str())
            .context("missing source revision")?;
        ensure!(
            source.source_id == e.source_id
                && source.root_id == e.root_id
                && source.observed_path == e.path,
            "source revision manifest differs from captured evidence provenance"
        );
        ensure!(
            util::digest(&e.excerpt) == e.digest,
            "invalid evidence digest"
        );
    }
    for record in &graph.knowledge {
        ensure!(!record.evidence.is_empty(), "ungrounded knowledge record");
        for citation in &record.evidence {
            let original = evidence
                .get(citation.id.as_str())
                .context("unknown evidence reference")?;
            ensure!(
                original.excerpt == citation.excerpt
                    && original.source_revision_id == citation.source_revision_id
                    && original.source_id == citation.source_id
                    && original.material == citation.material
                    && original.origin == citation.origin,
                "evidence reference differs from its original source manifest"
            );
        }
    }
    for relation in &graph.relations {
        ensure!(
            records.contains_key(relation.from.as_str())
                && records.contains_key(relation.to.as_str()),
            "missing relationship endpoint"
        );
        let citation = evidence
            .get(relation.evidence_id.as_str())
            .context("unknown relation evidence")?;
        ensure!(
            citation.source_revision_id == relation.source_revision_id
                && citation.excerpt == relation.exact_excerpt,
            "relation evidence mismatch"
        );
    }
    let mut indegree: BTreeMap<_, usize> = nodes.keys().map(|id| (*id, 0)).collect();
    let mut children = BTreeMap::<&str, Vec<&str>>::new();
    let mut edge_set = BTreeSet::new();
    for edge in &graph.edges {
        let parent = nodes
            .get(edge.parent.as_str())
            .context("unknown parent view")?;
        let child = nodes
            .get(edge.child.as_str())
            .context("unknown child view")?;
        ensure!(
            edge.parent != edge.child && edge.kind == "contains",
            "invalid containment edge"
        );
        ensure!(
            edge_set.insert((edge.parent.as_str(), edge.child.as_str())),
            "duplicate containment edge"
        );
        let parent_ids: BTreeSet<_> = parent.knowledge_ids.iter().collect();
        ensure!(
            child.knowledge_ids.iter().all(|id| parent_ids.contains(id)),
            "child escapes parent scope"
        );
        ensure!(
            child.kind == "evidence" || child.knowledge_ids.len() < parent.knowledge_ids.len(),
            "containment must become more specific"
        );
        *indegree.get_mut(edge.child.as_str()).unwrap() += 1;
        children
            .entry(edge.parent.as_str())
            .or_default()
            .push(edge.child.as_str());
    }
    for node in &graph.nodes {
        ensure!(
            node.knowledge_ids
                .iter()
                .all(|id| records.contains_key(id.as_str())),
            "unknown original knowledge reference"
        );
        let expected: Vec<_> = node
            .knowledge_ids
            .iter()
            .filter(|id| critical(records[id.as_str()]))
            .cloned()
            .collect();
        ensure!(
            expected == node.critical_knowledge_ids,
            "critical condition was removed from a view"
        );
        ensure!(
            node.summary_knowledge_ids
                .iter()
                .all(|id| node.knowledge_ids.contains(id)),
            "summary cites a record outside its scope"
        );
        ensure!(
            node.summary_coverage.eligible_units == node.knowledge_ids.len()
                && node.summary_coverage.included_units == node.summary_knowledge_ids.len()
                && node
                    .summary_coverage
                    .omitted_units
                    .checked_add(node.summary_coverage.included_units)
                    == Some(node.summary_coverage.eligible_units),
            "summary coverage manifest mismatch"
        );
        if let Some(dependencies) = dependencies {
            ensure!(
                node.revision == dependencies.revision(node)?,
                "view dependency fingerprint does not match its inputs"
            );
        }
        let expected_children: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.parent == node.id)
            .map(|e| e.child.clone())
            .collect();
        let expected_parents: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.child == node.id)
            .map(|e| e.parent.clone())
            .collect();
        ensure!(
            expected_children == node.child_view_ids && expected_parents == node.parent_view_ids,
            "view adjacency manifest mismatch"
        );
    }
    let mut queue: VecDeque<_> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| *id)
        .collect();
    ensure!(
        queue.len() == 1 && queue[0] == graph.root_id,
        "view graph is disconnected from its root"
    );
    let mut visited = 0;
    while let Some(id) = queue.pop_front() {
        visited += 1;
        for child in children.get(id).into_iter().flatten() {
            let degree = indegree.get_mut(child).unwrap();
            *degree -= 1;
            if *degree == 0 {
                queue.push_back(child);
            }
        }
    }
    ensure!(visited == nodes.len(), "cyclic view containment");
    Ok(())
}
