use super::*;
use crate::domain::documentary_basis;
use anyhow::{Context, ensure};
use std::collections::VecDeque;

/// Controlled evaluation changes only graph-derived ordering and navigation.
/// Both modes receive the same original seed candidates and required closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    GraphGuided,
    DirectOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct BundleDependency {
    pub from: String,
    pub to: String,
    /// A selection dependency, not a new source assertion or runtime finding.
    pub reason: String,
    pub relation_id: Option<String>,
    pub evidence_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceBundle {
    pub seed_id: String,
    pub knowledge_ids: BTreeSet<String>,
    pub dependencies: Vec<BundleDependency>,
}

#[derive(Clone, Copy)]
enum WireFormat {
    Original,
    Compact,
}

/// Union semantic navigation signals with direct original-record retrieval.
/// An exact identifier/path/value never has to survive a parent summary first.
pub fn select(graph: &KnowledgeGraph, options: &ExploreOptions) -> Result<ExploreResult> {
    select_with_candidates(graph, options, &[])
}

pub fn select_compact(
    graph: &KnowledgeGraph,
    options: &ExploreOptions,
) -> Result<CompactExploreResult> {
    select_compact_with_candidates(graph, options, &[])
}

pub(super) fn select_with_candidates(
    graph: &KnowledgeGraph,
    options: &ExploreOptions,
    additional: &[String],
) -> Result<ExploreResult> {
    select_impl(
        graph,
        options,
        additional,
        false,
        SelectionMode::GraphGuided,
        WireFormat::Original,
    )
}

pub(super) fn select_compact_with_candidates(
    graph: &KnowledgeGraph,
    options: &ExploreOptions,
    additional: &[String],
) -> Result<CompactExploreResult> {
    CompactExploreResult::from_expanded(&select_impl(
        graph,
        options,
        additional,
        false,
        SelectionMode::GraphGuided,
        WireFormat::Compact,
    )?)
}

/// Causal ablation hook. Candidate identity is supplied independently of the
/// graph and is never expanded through navigation membership. Documentary
/// relation/qualification closure is identical in both modes.
pub fn select_controlled(
    graph: &KnowledgeGraph,
    options: &ExploreOptions,
    candidates: &[String],
    mode: SelectionMode,
) -> Result<ExploreResult> {
    select_impl(graph, options, candidates, true, mode, WireFormat::Original)
}

pub fn select_compact_controlled(
    graph: &KnowledgeGraph,
    options: &ExploreOptions,
    candidates: &[String],
    mode: SelectionMode,
) -> Result<CompactExploreResult> {
    CompactExploreResult::from_expanded(&select_impl(
        graph,
        options,
        candidates,
        true,
        mode,
        WireFormat::Compact,
    )?)
}

fn select_impl(
    graph: &KnowledgeGraph,
    options: &ExploreOptions,
    additional: &[String],
    fixed_candidates: bool,
    mode: SelectionMode,
    format: WireFormat,
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
    ensure!(
        additional.len() <= graph.options.max_records,
        "too many original candidates"
    );
    let candidate_ids: BTreeSet<_> = additional.iter().map(String::as_str).collect();
    ensure!(
        candidate_ids.iter().all(|id| records.contains_key(id)),
        "unknown controlled original candidate"
    );
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
        if !allowed.contains(record.id.as_str())
            || (fixed_candidates && !candidate_ids.contains(record.id.as_str()))
        {
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
            + usize::from(candidate_ids.contains(record.id.as_str())) * 4;
        if fixed_candidates || query_empty || exact || score > 0 {
            ranked.push((record.id.clone(), score, exact));
        }
    }
    ranked.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(b.1.cmp(&a.1))
            .then(critical(records[b.0.as_str()]).cmp(&critical(records[a.0.as_str()])))
            .then(a.0.cmp(&b.0))
    });
    let has_exact = ranked.iter().any(|(_, _, exact)| *exact);
    let selected_node = if let Some(node) = requested {
        Some(node)
    } else if has_exact {
        ranked
            .iter()
            .filter(|(_, _, exact)| *exact)
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
            .or_else(|| {
                ranked
                    .iter()
                    .find_map(|(id, _, _)| nodes.get(stable_id("record", id).as_str()).copied())
            })
    };
    // Graph membership is only an ordering signal; it cannot introduce a new
    // factual obligation or remove an original candidate in either arm.
    if mode == SelectionMode::GraphGuided {
        let members: BTreeSet<_> = selected_node
            .into_iter()
            .flat_map(|node| node.knowledge_ids.iter().map(String::as_str))
            .collect();
        ranked.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then(b.1.cmp(&a.1))
                .then(critical(records[b.0.as_str()]).cmp(&critical(records[a.0.as_str()])))
                .then(
                    members
                        .contains(b.0.as_str())
                        .cmp(&members.contains(a.0.as_str())),
                )
                .then(a.0.cmp(&b.0))
        });
    }
    let requirements = Requirements::new(graph);
    let mut selection_work = 0;
    let mut selection_truncated = false;
    let mut bundles = Vec::new();
    let mut eligible: BTreeSet<_> = ranked.iter().map(|(id, _, _)| id.clone()).collect();
    for (id, _, _) in &ranked {
        let Some(bundle) = complete_bundle(
            &records,
            &requirements,
            id,
            &options.query,
            &mut selection_work,
            graph.options.max_work,
        ) else {
            selection_truncated = true;
            break;
        };
        eligible.extend(bundle.knowledge_ids.iter().cloned());
        if !bundles
            .iter()
            .any(|existing: &EvidenceBundle| existing.knowledge_ids == bundle.knowledge_ids)
        {
            bundles.push(bundle);
        }
    }
    let (navigation, navigation_truncated) = if mode == SelectionMode::GraphGuided {
        navigation(graph, selected_node, options.max_nodes)
    } else {
        (Vec::new(), false)
    };
    let mut result = ExploreResult {
        schema_version: match format {
            WireFormat::Original => ZOOM_SCHEMA_VERSION,
            WireFormat::Compact => COMPACT_ZOOM_SCHEMA_VERSION,
        },
        snapshot_revision: graph.revision.clone(),
        query: options.query.clone(),
        selected_node_id: (mode == SelectionMode::GraphGuided)
            .then(|| selected_node.map(|n| n.id.clone()))
            .flatten(),
        retrieval: if fixed_candidates {
            "controlled_candidates"
        } else if has_exact {
            "direct_reference"
        } else {
            "cross_level"
        }
        .into(),
        status: "complete".into(),
        model_calls: 0,
        nodes: Vec::new(),
        edges: Vec::new(),
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
        used_tokens: options.max_tokens,
        warnings: graph.report.warnings.clone(),
    };
    result.warnings.push(
        "Original records retain documentary status; source reports do not prove runtime behavior."
            .into(),
    );
    if selection_truncated {
        result.warnings.push("Selection reached its work limit; unknown remaining critical coverage is omitted, and no conclusion is established for it.".into());
    }
    if graph.report.unsupported_units > 0 {
        result.warnings.push(format!(
            "{} unsupported records were ineligible; absence of constraints is not established.",
            graph.report.unsupported_units
        ));
    }
    let mut selected = BTreeSet::new();
    update_coverage(
        &mut result,
        &bundles,
        &records,
        &selected,
        selection_truncated,
    );
    // Reserve the envelope and all source/relationship obligations before
    // spending any tokens on navigation or duplicated summaries.
    for bundle in &bundles {
        let candidate: BTreeSet<_> = selected.union(&bundle.knowledge_ids).cloned().collect();
        if candidate == selected {
            continue;
        }
        let mut trial = result.clone();
        fill_payload(&mut trial, graph, &candidate);
        update_coverage(
            &mut trial,
            &bundles,
            &records,
            &candidate,
            selection_truncated,
        );
        if measure(&trial)? <= options.max_tokens {
            selected = candidate;
            result = trial;
        }
    }
    for node in navigation {
        let mut trial = result.clone();
        trial.nodes.push(node);
        let ids: BTreeSet<_> = trial.nodes.iter().map(|n| n.id.as_str()).collect();
        trial.edges = graph
            .edges
            .iter()
            .filter(|e| ids.contains(e.parent.as_str()) && ids.contains(e.child.as_str()))
            .cloned()
            .collect();
        fill_payload(&mut trial, graph, &selected);
        if measure(&trial)? <= options.max_tokens {
            result = trial;
        } else {
            result.navigation_truncated = true;
        }
    }
    if measure(&result)? > options.max_tokens {
        // This is only the metadata-only lower bound: facts were always
        // measured with their final omission metadata before being accepted.
        ensure!(
            selected.is_empty(),
            "accepted evidence exceeded its measured budget"
        );
        result.nodes.clear();
        result.edges.clear();
        result.warnings.clear();
        result.coverage.omission_reasons = vec!["Required original evidence exceeds this budget. Use lore evidence ID for exact evidence or increase --max-tokens.".into()];
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

fn update_coverage(
    result: &mut ExploreResult,
    bundles: &[EvidenceBundle],
    records: &BTreeMap<&str, &KnowledgeView>,
    selected: &BTreeSet<String>,
    truncated: bool,
) {
    result.coverage.included_units = selected.len();
    result.coverage.omitted_units = result
        .coverage
        .eligible_units
        .saturating_sub(selected.len());
    result.critical_groups_omitted = bundles
        .iter()
        .filter(|bundle| {
            !bundle.knowledge_ids.is_subset(selected)
                && bundle
                    .knowledge_ids
                    .iter()
                    .any(|id| critical(records[id.as_str()]))
        })
        .count();
    if truncated {
        result.critical_groups_omitted = result.critical_groups_omitted.max(1);
    }
    result.coverage.omission_reasons.clear();
    if result.coverage.eligible_units == 0 && !truncated {
        result.status = "no_matches".into();
        result.coverage.omission_reasons.push("No eligible original record matched; this does not establish the absence of constraints.".into());
    } else if result.coverage.omitted_units > 0 || truncated {
        result.status = if selected.is_empty() {
            "budget_limited"
        } else {
            "partial"
        }
        .into();
        result.coverage.omission_reasons.push("Omitted complete evidence bundles may contain material conditions; no conclusion is established for omitted coverage. Exact evidence remains available with lore evidence ID.".into());
    } else {
        result.status = "complete".into();
    }
}

struct Requirements<'a> {
    topics: BTreeMap<&'a str, Vec<&'a str>>,
    subjects: BTreeMap<String, Vec<&'a str>>,
    symbols: BTreeMap<String, Vec<&'a str>>,
    references: BTreeMap<&'a str, Vec<&'a str>>,
    related: BTreeMap<&'a str, Vec<(&'a str, &'a EvidenceRelation)>>,
}

impl<'a> Requirements<'a> {
    fn new(graph: &'a KnowledgeGraph) -> Self {
        let mut requirements = Self {
            topics: BTreeMap::new(),
            subjects: BTreeMap::new(),
            symbols: BTreeMap::new(),
            references: BTreeMap::new(),
            related: BTreeMap::new(),
        };
        let records = record_map(graph);
        for record in &graph.knowledge {
            if explicit_condition(record) || record.kind == "decision" {
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
                for symbol in symbol_terms(&record.statement) {
                    requirements
                        .symbols
                        .entry(symbol)
                        .or_default()
                        .push(&record.id);
                }
            }
            for reference in reference_terms(&record.statement) {
                if let Some(original) = records.get(reference.as_str()) {
                    requirements
                        .references
                        .entry(&original.id)
                        .or_default()
                        .push(&record.id);
                }
            }
        }
        for relation in &graph.relations {
            requirements
                .related
                .entry(&relation.from)
                .or_default()
                .push((&relation.to, relation));
            requirements
                .related
                .entry(&relation.to)
                .or_default()
                .push((&relation.from, relation));
        }
        requirements
    }
}

fn strong_condition(record: &KnowledgeView) -> bool {
    if matches!(record.kind.as_str(), "constraint" | "risk") {
        return true;
    }
    let text = record.statement.to_lowercase();
    [
        "must ",
        "never ",
        "unless ",
        "except",
        "only if",
        "only before",
        "only after",
        "applies only",
        "may only",
        "can only",
        "do not ",
        "contraindicat",
    ]
    .iter()
    .any(|term| text.contains(term))
}

fn explicit_condition(record: &KnowledgeView) -> bool {
    strong_condition(record) || record.statement.to_lowercase().contains("cannot ")
}

fn explanatory_consequence(record: &KnowledgeView) -> bool {
    if strong_condition(record) {
        return false;
    }
    let text = record.statement.to_lowercase();
    text.find("cannot ")
        .is_some_and(|position| text[..position].contains(" so "))
}

fn subject_terms(text: &str) -> BTreeSet<String> {
    let mut words = terms(&text.replace('_', " "));
    words.retain(|word| word.chars().any(char::is_alphabetic));
    for generic in [
        "production",
        "staging",
        "development",
        "test",
        "system",
        "component",
        "operations",
        "must",
        "never",
        "only",
        "before",
        "after",
        "applies",
        "should",
        "would",
        "current",
        "proposed",
        "accepted",
        "selected",
        "uses",
        "use",
    ] {
        words.remove(generic);
    }
    words
}

// Numeric values remain exact retrieval signals but do not make unrelated
// rules (for example a queue limit of 8 and credential 8) depend on one another.
fn symbol_terms(text: &str) -> Vec<String> {
    reference_terms(text)
        .into_iter()
        .filter(|word| {
            word.chars().any(char::is_alphabetic)
                && (word.contains(['/', '_', '.'])
                    || word.chars().filter(|c| c.is_ascii_uppercase()).count() >= 2)
        })
        .collect()
}

fn scope_applies(candidate: &KnowledgeView, query: &str) -> bool {
    const ENVIRONMENTS: &[&str] = &["production", "staging", "development", "sandbox"];
    let query = terms(query);
    let requested: BTreeSet<_> = ENVIRONMENTS
        .iter()
        .filter(|scope| query.contains(**scope))
        .collect();
    if requested.is_empty() {
        return true;
    }
    let statement = terms(&candidate.statement);
    let mut scopes: BTreeSet<_> = ENVIRONMENTS
        .iter()
        .filter(|scope| statement.contains(**scope))
        .collect();
    if scopes.is_empty() {
        let declared = terms(&candidate.scope);
        scopes.extend(
            ENVIRONMENTS
                .iter()
                .filter(|scope| declared.contains(**scope)),
        );
    }
    scopes.is_empty() || !requested.is_disjoint(&scopes)
}

fn applicable_condition(
    seed: &KnowledgeView,
    candidate: &KnowledgeView,
    query: &str,
) -> Option<&'static str> {
    if seed.id == candidate.id || !scope_applies(candidate, query) {
        return None;
    }
    let same_topic = seed.topic == candidate.topic;
    let same_subject = seed.subject.eq_ignore_ascii_case(&candidate.subject);
    let seed_body = subject_terms(&seed.statement);
    let candidate_body = subject_terms(&candidate.statement);
    let subject = subject_terms(&seed.subject);
    let shared_symbol = symbol_terms(&seed.statement)
        .iter()
        .any(|symbol| exact_fragment(&candidate.statement, symbol));
    // The whole named subject is needed across subject boundaries. Shared
    // words such as "file", "search", "input" or "output" only establish
    // relevance; they do not make distinct CLI rules inseparable. Single-word
    // concepts also need an explicit symbol/reference or the same declared
    // subject before a condition can be mandatory.
    let names_requested_subject = subject.len() >= 2 && subject.is_subset(&candidate_body);
    let directly_applicable = shared_symbol
        || (same_subject && !subject.is_empty())
        || (same_topic && names_requested_subject);
    if !directly_applicable {
        return None;
    }
    // A purpose statement's explanatory consequence is relevant context, not
    // a mandatory rule solely because it repeats a broad topic/subject label.
    // A real "cannot" permission/boundary on the requested object still
    // attaches; explicit MUST and typed constraints bypass this refinement.
    if explanatory_consequence(candidate) {
        let topic = subject_terms(&seed.topic);
        let overlap: BTreeSet<_> = seed_body
            .intersection(&candidate_body)
            .filter(|word| !topic.contains(*word))
            .collect();
        if overlap.is_empty() {
            return None;
        }
    }
    if explicit_condition(candidate) {
        // A proposed change is not an authoritative exception to current
        // accepted guidance. Its complete original record remains eligible as
        // related context, with its proposal lifecycle intact.
        if candidate.lifecycle == "proposed" && seed.lifecycle != "proposed" {
            return None;
        }
        return Some("directly_applicable_explicit_condition");
    }
    if seed.lifecycle == "proposed"
        && candidate.kind == "decision"
        && candidate.lifecycle == "accepted"
        && !seed_body.is_disjoint(&candidate_body)
    {
        return Some("current_decision_qualifies_proposal");
    }
    None
}

fn complete_bundle(
    records: &BTreeMap<&str, &KnowledgeView>,
    requirements: &Requirements<'_>,
    seed: &str,
    query: &str,
    work: &mut usize,
    limit: usize,
) -> Option<EvidenceBundle> {
    let mut selected = BTreeSet::from([seed.to_owned()]);
    let mut dependencies = BTreeSet::new();
    let mut queue = VecDeque::from([(seed.to_owned(), true)]);
    let mut processed = BTreeMap::new();
    while let Some((id, expand_conditions)) = queue.pop_front() {
        if processed.get(&id) == Some(&true) || (!expand_conditions && processed.contains_key(&id))
        {
            continue;
        }
        processed.insert(id.clone(), expand_conditions);
        if *work >= limit {
            return None;
        }
        *work += 1;
        let record = records[id.as_str()];
        let mut additions = Vec::new();
        for (candidate, relation) in requirements.related.get(id.as_str()).into_iter().flatten() {
            additions.push(BundleDependency {
                from: id.clone(),
                to: (*candidate).into(),
                reason: "documentary_relationship_endpoint_and_witness".into(),
                relation_id: Some(relation.id.clone()),
                evidence_id: Some(relation.evidence_id.clone()),
            });
        }
        // Applicability is direct to a requested original or a documentary
        // endpoint. Incidental shared words inside an attached condition do
        // not recursively turn the whole topic into an inseparable bundle.
        // Explicit record references and documentary relations still close
        // transitively, including qualifiers attached outside a selected node.
        if expand_conditions {
            let mut candidates = BTreeSet::new();
            candidates.extend(
                requirements
                    .topics
                    .get(record.topic.as_str())
                    .into_iter()
                    .flatten()
                    .copied(),
            );
            candidates.extend(
                requirements
                    .subjects
                    .get(&record.subject.to_lowercase())
                    .into_iter()
                    .flatten()
                    .copied(),
            );
            for symbol in symbol_terms(&record.statement) {
                candidates.extend(
                    requirements
                        .symbols
                        .get(&symbol)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
            for candidate in candidates {
                if *work >= limit {
                    return None;
                }
                *work += 1;
                if let Some(reason) = applicable_condition(record, records[candidate], query) {
                    additions.push(BundleDependency {
                        from: id.clone(),
                        to: candidate.into(),
                        reason: reason.into(),
                        relation_id: None,
                        evidence_id: None,
                    });
                }
            }
        }
        for candidate in requirements
            .references
            .get(id.as_str())
            .into_iter()
            .flatten()
        {
            if explicit_condition(records[candidate]) {
                additions.push(BundleDependency {
                    from: id.clone(),
                    to: (*candidate).into(),
                    reason: "explicit_original_record_qualification".into(),
                    relation_id: None,
                    evidence_id: None,
                });
            }
        }
        for dependency in additions {
            if *work >= limit {
                return None;
            }
            *work += 1;
            selected.insert(dependency.to.clone());
            let expand = dependency.reason != "directly_applicable_explicit_condition";
            if !processed.contains_key(&dependency.to)
                || (expand && processed.get(&dependency.to) != Some(&true))
            {
                queue.push_back((dependency.to.clone(), expand));
            }
            dependencies.insert(dependency);
        }
    }
    Some(EvidenceBundle {
        seed_id: seed.into(),
        knowledge_ids: selected,
        dependencies: dependencies.into_iter().collect(),
    })
}

/// Inspect the original-source obligations independently of output packing.
/// Shared topics alone do not establish a dependency. No source writes,
/// inference or authoritative assertions are introduced by this diagnostic.
pub fn inspect_bundles(graph: &KnowledgeGraph, seeds: &[String]) -> Result<Vec<EvidenceBundle>> {
    validate(graph)?;
    ensure!(
        seeds.len() <= graph.options.max_records,
        "too many bundle seeds"
    );
    let records = record_map(graph);
    let requirements = Requirements::new(graph);
    let mut work = 0;
    seeds
        .iter()
        .map(|seed| {
            ensure!(records.contains_key(seed.as_str()), "unknown bundle seed");
            complete_bundle(
                &records,
                &requirements,
                seed,
                "",
                &mut work,
                graph.options.max_work,
            )
            .context("bundle inspection exceeds the bounded selection work limit")
        })
        .collect()
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

fn measure(result: &ExploreResult) -> Result<usize> {
    let serialized = if result.schema_version == COMPACT_ZOOM_SCHEMA_VERSION {
        serde_json::to_string(&CompactExploreResult::from_expanded(result)?)? + "\n"
    } else {
        // Preserve the existing schema-1 pretty-JSON budget contract.
        serde_json::to_string_pretty(result)? + "\n"
    };
    Ok(crate::context::count_tokens(&serialized)
        .max(crate::context::count_tokens(&render_markdown(result))))
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
