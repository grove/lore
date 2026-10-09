//! Deterministic candidate retrieval over the compiled knowledge registry.
//!
//! Lexical/path matches establish the task's concepts. Subject and topic
//! expansion is deliberately one-pass, while explicit relationships permit
//! two hops. This finds connected knowledge without turning a broad topic
//! into a second, uncited inference pipeline.

use crate::{domain::KnowledgeView, storage};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet};

const MAX_QUERY_TERMS: usize = 32;
const MAX_SEEDS: usize = 48;
const MAX_CANDIDATES: usize = 128;
const MAX_SUBJECT_NEIGHBORS: usize = 12;
const MAX_TOPIC_NEIGHBORS: usize = 8;
const SMALL_TOPIC: usize = 12;
const MAX_RELATION_HOPS: usize = 2;
const MAX_REASONS: usize = 6;

#[derive(Debug, Clone)]
pub struct RetrievalHit {
    pub knowledge: KnowledgeView,
    pub score: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RetrievalReport {
    pub hits: Vec<RetrievalHit>,
    /// A search bound dropped terms or candidates. This is deliberately
    /// conservative: later graph expansion may recover some omitted records.
    pub truncated: bool,
}

#[derive(Default, Clone)]
struct Candidate {
    lexical: f64,
    path: f64,
    expansion: f64,
    reasons: BTreeSet<String>,
}

impl Candidate {
    fn score(&self) -> f64 {
        (self.lexical + self.path).max(self.expansion)
    }
}

/// Retrieve task-relevant candidates without writing state or invoking a model.
///
/// The returned candidate set is bounded. Context assembly must preserve the
/// status and relationships of retained records when applying its own budget;
/// in particular, an omitted conflict endpoint must never make a claim appear
/// unqualified. Reasons describe retrieval signals, not new factual claims.
pub fn retrieve(conn: &Connection, task: &str, paths: &[String]) -> Result<Vec<RetrievalHit>> {
    Ok(retrieve_with_report(conn, task, paths)?.hits)
}

pub fn retrieve_with_report(
    conn: &Connection,
    task: &str,
    paths: &[String],
) -> Result<RetrievalReport> {
    let (task_terms, mut truncated) = query_terms_with_report(task);
    let path_hints: Vec<_> = paths
        .iter()
        .filter_map(|path| PathHint::new(path))
        .collect();
    if task_terms.is_empty() && path_hints.is_empty() {
        return Ok(RetrievalReport {
            hits: Vec::new(),
            truncated,
        });
    }
    let knowledge: BTreeMap<_, _> = storage::views(conn)?
        .into_iter()
        .map(|view| (view.id.clone(), view))
        .collect();
    if knowledge.is_empty() {
        return Ok(RetrievalReport {
            hits: Vec::new(),
            truncated,
        });
    }

    let mut candidates = BTreeMap::<String, Candidate>::new();
    let (lexical, limited) = lexical_hits(conn, &task_terms, None)?;
    truncated |= limited;
    for (id, rank) in lexical {
        if knowledge.contains_key(&id) {
            let candidate = candidates.entry(id).or_default();
            candidate.lexical = 5.0 + 3.0 * rank;
            candidate
                .reasons
                .insert("task keyword relevance (BM25)".into());
        }
    }
    for hint in &path_hints {
        let (terms, limited) = hint.terms();
        truncated |= limited;
        let (lexical, limited) = lexical_hits(conn, &terms, hint.root.as_deref())?;
        truncated |= limited;
        for (id, rank) in lexical {
            if knowledge.contains_key(&id) {
                let candidate = candidates.entry(id).or_default();
                candidate.path = candidate.path.max(2.0 + 2.0 * rank);
                candidate
                    .reasons
                    .insert(format!("path terminology: {}", hint.display));
            }
        }
        for view in knowledge.values() {
            for evidence in &view.evidence {
                let (root, path) = evidence
                    .source
                    .split_once(':')
                    .unwrap_or(("", &evidence.source));
                if hint
                    .root
                    .as_deref()
                    .is_some_and(|requested| requested != root)
                {
                    continue;
                }
                let source_path = normalize_path(path);
                let exact = source_path == hint.path;
                let directory = source_path.starts_with(&format!("{}/", hint.path));
                let mentioned = path_mentioned(&evidence.excerpt, &hint.path);
                if !exact && !directory && !mentioned {
                    continue;
                }
                let candidate = candidates.entry(view.id.clone()).or_default();
                let score: f64 = if exact {
                    10.0
                } else if mentioned {
                    9.0
                } else {
                    7.0
                };
                candidate.path = candidate
                    .path
                    .max(score - if evidence.active { 0.0 } else { 1.0 });
                candidate.reasons.insert(if exact || directory {
                    format!("source path: {}", evidence.source)
                } else {
                    format!("path cited in source evidence: {}", hint.display)
                });
            }
        }
    }
    truncated |= retain_best(&mut candidates, MAX_SEEDS);
    // Only initial matches can cause subject/topic expansion. A retrieved
    // constraint must not recursively pull in its entire topic and neighbors.
    let seeds = ranked(&candidates);
    expand_concepts(&knowledge, &seeds, &mut candidates, &mut truncated);
    truncated |= retain_best(&mut candidates, MAX_CANDIDATES);
    expand_relations(conn, &knowledge, &mut candidates, &mut truncated)?;

    let hits = ranked(&candidates)
        .into_iter()
        .filter_map(|(id, score)| {
            knowledge.get(&id).map(|view| RetrievalHit {
                knowledge: view.clone(),
                score,
                reasons: concise_reasons(&candidates[&id].reasons),
            })
        })
        .collect();
    Ok(RetrievalReport { hits, truncated })
}

fn concise_reasons(reasons: &BTreeSet<String>) -> Vec<String> {
    let priority = |reason: &str| {
        if reason.starts_with("task keyword")
            || reason.starts_with("source path:")
            || reason.starts_with("path cited")
        {
            0
        } else if reason.contains(" relationship with ") {
            1
        } else if reason.starts_with("path terminology:") {
            2
        } else if reason.starts_with("same subject") {
            3
        } else {
            4
        }
    };
    let mut reasons: Vec<_> = reasons.iter().cloned().collect();
    reasons.sort_by(|a, b| priority(a).cmp(&priority(b)).then_with(|| a.cmp(b)));
    reasons.truncate(MAX_REASONS);
    reasons
}

fn ranked(candidates: &BTreeMap<String, Candidate>) -> Vec<(String, f64)> {
    let mut rows: Vec<_> = candidates
        .iter()
        .map(|(id, candidate)| (id.clone(), candidate.score()))
        .collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

fn retain_best(candidates: &mut BTreeMap<String, Candidate>, limit: usize) -> bool {
    if candidates.len() <= limit {
        return false;
    }
    let retained: BTreeSet<_> = ranked(candidates)
        .into_iter()
        .take(limit)
        .map(|(id, _)| id)
        .collect();
    candidates.retain(|id, _| retained.contains(id));
    true
}

/// Quote every token so punctuation and FTS operators in a task are data.
/// BM25's relative score is normalized only within this query, leaving stable
/// document IDs as a deterministic tie-breaker when scores are identical.
fn lexical_hits(
    conn: &Connection,
    terms: &[String],
    root: Option<&str>,
) -> Result<(Vec<(String, f64)>, bool)> {
    if terms.is_empty() {
        return Ok((Vec::new(), false));
    }
    let expression = terms
        .iter()
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" OR ");
    let mut statement = conn.prepare(
        "SELECT knowledge_id,bm25(knowledge_fts,0.0,1.0,1.6,1.2)
         FROM knowledge_fts WHERE knowledge_fts MATCH ?1
         AND (?2 IS NULL OR knowledge_id IN (
             SELECT aa.knowledge_id FROM assertion_assignments aa
             JOIN assertion_revisions ar ON ar.id=aa.assertion_revision_id
             JOIN sources s ON s.id=ar.source_id WHERE s.root_id=?2
         ))
         ORDER BY bm25(knowledge_fts,0.0,1.0,1.6,1.2),knowledge_id LIMIT ?3",
    )?;
    let mut rows: Vec<(String, f64)> = statement
        .query_map(params![expression, root, (MAX_SEEDS + 1) as i64], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let truncated = rows.len() > MAX_SEEDS;
    rows.truncate(MAX_SEEDS);
    let best = rows
        .first()
        .map_or(1.0, |(_, score)| (-score).max(f64::EPSILON));
    Ok((
        rows.into_iter()
            .map(|(id, score)| (id, ((-score) / best).clamp(0.0, 1.0)))
            .collect(),
        truncated,
    ))
}

fn query_terms(text: &str) -> Vec<String> {
    query_terms_with_report(text).0
}

fn query_terms_with_report(text: &str) -> (Vec<String>, bool) {
    // Function words carry almost no task identity. This is a language-level
    // filter, never a corpus-specific alias or synonym dictionary.
    const STOP: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "how", "i", "in", "into",
        "is", "it", "of", "on", "or", "our", "please", "should", "that", "the", "their", "this",
        "to", "we", "with", "would", "you", "your",
    ];
    let mut terms = BTreeSet::new();
    let mut truncated = false;
    for term in text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .filter(|token| !STOP.contains(&token.as_str()))
    {
        if terms.len() == MAX_QUERY_TERMS && !terms.contains(&term) {
            truncated = true;
            break;
        }
        terms.insert(term);
    }
    (terms.into_iter().collect(), truncated)
}

fn normalized_words(text: &str) -> String {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shares_root(a: &KnowledgeView, b: &KnowledgeView) -> bool {
    a.evidence.iter().any(|left| {
        b.evidence.iter().any(|right| {
            let left_root = left.source.split_once(':').map(|(root, _)| root);
            let right_root = right.source.split_once(':').map(|(root, _)| root);
            left_root.is_some() && left_root == right_root
        })
    })
}

fn shares_source(a: &KnowledgeView, b: &KnowledgeView) -> bool {
    a.evidence.iter().any(|left| {
        b.evidence
            .iter()
            .any(|right| left.source_id == right.source_id)
    })
}

fn subject_overlap(a: &KnowledgeView, b: &KnowledgeView) -> bool {
    let left = query_terms(&a.subject);
    let right = query_terms(&b.subject);
    left.iter().any(|word| right.contains(word))
}

fn importance(view: &KnowledgeView) -> f64 {
    let kind = match view.kind.as_str() {
        "constraint" => 1.8,
        "question" | "risk" => 1.6,
        "decision" => 1.4,
        "reported_outcome" | "observation" => 1.2,
        _ => 1.0,
    };
    kind + if view.support_state == "needs_review" {
        0.3
    } else {
        0.0
    }
}

fn expand_concepts(
    knowledge: &BTreeMap<String, KnowledgeView>,
    seeds: &[(String, f64)],
    candidates: &mut BTreeMap<String, Candidate>,
    truncated: &mut bool,
) {
    let mut visited_subjects = BTreeSet::new();
    let mut topic_additions = BTreeMap::<String, BTreeSet<String>>::new();
    for (seed_id, seed_score) in seeds {
        let seed = &knowledge[seed_id];
        let roots = seed
            .evidence
            .iter()
            .filter_map(|evidence| evidence.source.split_once(':').map(|(root, _)| root))
            .collect::<BTreeSet<_>>();
        let subject = normalized_words(&seed.subject);
        let subject_group = (seed.topic.clone(), subject.clone(), roots.clone());
        let same_topic: Vec<_> = knowledge
            .values()
            .filter(|view| {
                view.id != seed.id && view.topic == seed.topic && shares_root(seed, view)
            })
            .collect();
        if !subject.is_empty() && visited_subjects.insert(subject_group) {
            let mut neighbors: Vec<_> = same_topic
                .iter()
                .copied()
                .filter(|view| normalized_words(&view.subject) == subject)
                .collect();
            neighbors.sort_by(|a, b| {
                importance(b)
                    .total_cmp(&importance(a))
                    .then_with(|| a.id.cmp(&b.id))
            });
            let mut additions = 0;
            for view in neighbors {
                if !candidates.contains_key(&view.id) {
                    if additions == MAX_SUBJECT_NEIGHBORS {
                        *truncated = true;
                        continue;
                    }
                    additions += 1;
                }
                let score = seed_score * 0.66 + importance(view) * 0.2;
                let candidate = candidates.entry(view.id.clone()).or_default();
                candidate.expansion = candidate.expansion.max(score);
                candidate
                    .reasons
                    .insert(format!("same subject as {seed_id}: {}", seed.subject));
            }
        }
        // Topic names may cover an entire project. In larger clusters require
        // another recorded anchor, instead of pulling arbitrary constraints
        // solely because they share a broad topic label.
        let mut neighbors: Vec<_> = same_topic
            .iter()
            .copied()
            .filter(|view| {
                let same_source = shares_source(seed, view);
                let same_scope = normalized_words(&seed.scope) == normalized_words(&view.scope);
                (same_source || same_scope)
                    && (same_source
                        || subject_overlap(seed, view)
                        || same_topic.len() < SMALL_TOPIC)
            })
            .collect();
        neighbors.sort_by(|a, b| {
            let affinity = |view: &KnowledgeView| {
                importance(view)
                    + if shares_source(seed, view) { 1.0 } else { 0.0 }
                    + if subject_overlap(seed, view) {
                        0.5
                    } else {
                        0.0
                    }
            };
            affinity(b)
                .total_cmp(&affinity(a))
                .then_with(|| a.id.cmp(&b.id))
        });
        let topic_key = format!(
            "{}:{}",
            roots.into_iter().collect::<Vec<_>>().join(","),
            seed.topic
        );
        let added = topic_additions.entry(topic_key).or_default();
        for view in neighbors {
            if !candidates.contains_key(&view.id) {
                if added.len() >= MAX_TOPIC_NEIGHBORS {
                    *truncated = true;
                    continue;
                }
                added.insert(view.id.clone());
            }
            let candidate = candidates.entry(view.id.clone()).or_default();
            candidate.expansion = candidate
                .expansion
                .max(seed_score * 0.42 + importance(view) * 0.2);
            candidate
                .reasons
                .insert(format!("same topic as {seed_id}: {}", seed.topic));
        }
    }
}

fn expand_relations(
    conn: &Connection,
    knowledge: &BTreeMap<String, KnowledgeView>,
    candidates: &mut BTreeMap<String, Candidate>,
    truncated: &mut bool,
) -> Result<()> {
    let edges = storage::relations(conn)?;
    let mut adjacency = BTreeMap::<&str, Vec<(&str, &str, bool)>>::new();
    for edge in &edges {
        adjacency
            .entry(&edge.from)
            .or_default()
            .push((&edge.to, &edge.kind, edge.active));
        adjacency
            .entry(&edge.to)
            .or_default()
            .push((&edge.from, &edge.kind, edge.active));
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    let mut frontier = ranked(candidates);
    let mut visited: BTreeSet<_> = frontier.iter().map(|(id, _)| id.clone()).collect();
    for _ in 0..MAX_RELATION_HOPS {
        let mut discovered = BTreeSet::new();
        for (from, score) in &frontier {
            for &(to, kind, active) in adjacency.get(from.as_str()).into_iter().flatten() {
                if !knowledge.contains_key(to) {
                    continue;
                }
                let critical = matches!(kind, "supersedes" | "contradicts" | "suspected_conflict");
                let factor = if critical {
                    0.90
                } else if kind == "related_to" {
                    0.62
                } else {
                    0.76
                };
                let related_score = score * factor + if critical { 1.0 } else { 0.0 };
                let candidate = candidates.entry(to.to_owned()).or_default();
                candidate.expansion = candidate.expansion.max(related_score);
                candidate.reasons.insert(format!(
                    "{kind} relationship with {from} ({})",
                    if active {
                        "current evidence"
                    } else {
                        "historical evidence"
                    }
                ));
                if !visited.contains(to) {
                    discovered.insert(to.to_owned());
                }
            }
        }
        *truncated |= retain_best(candidates, MAX_CANDIDATES);
        frontier = ranked(candidates)
            .into_iter()
            .filter(|(id, _)| discovered.contains(id))
            .collect();
        visited.extend(discovered);
        if frontier.is_empty() {
            break;
        }
    }
    if frontier.iter().any(|(id, _)| {
        adjacency
            .get(id.as_str())
            .into_iter()
            .flatten()
            .any(|(to, _, _)| knowledge.contains_key(*to) && !candidates.contains_key(*to))
    }) {
        *truncated = true;
    }
    Ok(())
}

struct PathHint {
    root: Option<String>,
    path: String,
    display: String,
}

impl PathHint {
    fn new(input: &str) -> Option<Self> {
        let display = input.trim().to_owned();
        if display.is_empty() {
            return None;
        }
        let (root, path) = match display.split_once(':') {
            Some((root, path)) if !root.is_empty() && !root.contains(['/', '\\']) => {
                (Some(root.to_owned()), path)
            }
            _ => (None, display.as_str()),
        };
        let path = normalize_path(path);
        if path.is_empty() {
            return None;
        }
        Some(Self {
            root,
            path,
            display,
        })
    }

    fn terms(&self) -> (Vec<String>, bool) {
        let parts: Vec<_> = self.path.split('/').collect();
        let leaf = parts.last().copied().unwrap_or("");
        let stem = leaf
            .rsplit_once('.')
            .filter(|(stem, _)| !stem.is_empty())
            .map_or(leaf, |(stem, _)| stem);
        // Use the leaf name and nearest directory as weak lexical hints;
        // file extensions alone do not identify the relevant concept.
        let parent = if parts.len() >= 2 {
            parts[parts.len() - 2]
        } else {
            ""
        };
        query_terms_with_report(&format!("{parent} {stem}"))
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn path_mentioned(excerpt: &str, path: &str) -> bool {
    let excerpt = excerpt.replace('\\', "/");
    let is_path_character =
        |character: char| character.is_alphanumeric() || matches!(character, '/' | '.' | '_' | '-');
    excerpt.match_indices(path).any(|(start, _)| {
        let before = excerpt[..start].chars().next_back();
        let after = excerpt[start + path.len()..].chars().next();
        !before.is_some_and(is_path_character) && !after.is_some_and(is_path_character)
    })
}
