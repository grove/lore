//! Explicit, stateless learning activities. Educational prerequisites are
//! proposals with cycle handling; they never change the project's evidence DAG.

use super::*;
use anyhow::ensure;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningStage {
    #[default]
    Primary,
    Transfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Predict,
    Explain,
    Trace,
    Change,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prerequisite {
    pub before: String,
    pub after: String,
    pub rationale: Claim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningGraph {
    pub concept_ids: Vec<String>,
    pub prerequisites: Vec<Prerequisite>,
    /// Each group can be learned together. Cyclic proposals are not silently
    /// converted into impossible or arbitrary mandatory prerequisites.
    pub co_learned: Vec<Vec<String>>,
    pub suggested_order: Vec<Vec<String>>,
    pub mandatory: bool,
}

impl LearningGraph {
    pub fn build(concepts: &[Concept], prerequisites: Vec<Prerequisite>) -> Result<Self> {
        ensure!(
            concepts.len() <= 16 && prerequisites.len() <= 32,
            "learning graph exceeds its node/edge bounds"
        );
        let ids = concepts
            .iter()
            .map(|c| c.id.clone())
            .collect::<BTreeSet<_>>();
        ensure!(ids.len() == concepts.len(), "duplicate learning concept ID");
        let mut edges = BTreeSet::new();
        let mut adjacency: BTreeMap<String, BTreeSet<String>> =
            ids.iter().map(|id| (id.clone(), BTreeSet::new())).collect();
        for edge in &prerequisites {
            ensure!(
                ids.contains(&edge.before) && ids.contains(&edge.after),
                "prerequisite endpoint is not a selected concept"
            );
            ensure!(
                edges.insert((edge.before.clone(), edge.after.clone())),
                "duplicate educational dependency"
            );
            adjacency
                .get_mut(&edge.before)
                .expect("validated endpoint")
                .insert(edge.after.clone());
        }
        let reachable = |start: &str| {
            let mut found = BTreeSet::new();
            let mut pending = vec![start.to_string()];
            while let Some(id) = pending.pop() {
                if found.insert(id.clone()) {
                    pending.extend(adjacency[&id].iter().cloned());
                }
            }
            found
        };
        // With at most sixteen concepts, bounded reachability is simpler and
        // easier to audit than a general graph dependency. Mutual reachability
        // identifies exact SCCs, including a self-loop.
        let closure: BTreeMap<_, _> = ids.iter().map(|id| (id.clone(), reachable(id))).collect();
        let mut groups = Vec::new();
        let mut visited = BTreeSet::new();
        for id in &ids {
            if visited.contains(id) {
                continue;
            }
            let group = ids
                .iter()
                .filter(|other| closure[id].contains(*other) && closure[*other].contains(id))
                .cloned()
                .collect::<Vec<_>>();
            visited.extend(group.iter().cloned());
            groups.push(group);
        }
        let co_learned = groups
            .iter()
            .filter(|group| group.len() > 1 || adjacency[&group[0]].contains(&group[0]))
            .cloned()
            .collect();
        let membership: BTreeMap<_, _> = groups
            .iter()
            .enumerate()
            .flat_map(|(i, group)| group.iter().map(move |id| (id.clone(), i)))
            .collect();
        let mut incoming = vec![BTreeSet::new(); groups.len()];
        for (before, after) in edges {
            let a = membership[&before];
            let b = membership[&after];
            if a != b {
                incoming[b].insert(a);
            }
        }
        let mut order = Vec::new();
        let mut emitted = BTreeSet::new();
        while emitted.len() < groups.len() {
            let next = (0..groups.len())
                .find(|i| !emitted.contains(i) && incoming[*i].is_subset(&emitted))
                .ok_or_else(|| {
                    anyhow::anyhow!("collapsed educational graph unexpectedly contains a cycle")
                })?;
            emitted.insert(next);
            order.push(groups[next].clone());
        }
        Ok(Self {
            concept_ids: ids.into_iter().collect(),
            prerequisites,
            co_learned,
            suggested_order: order,
            mandatory: false,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DraftActivity {
    pub title: String,
    pub kind: ActivityKind,
    pub objective: String,
    pub task: String,
    pub starting_point: Claim,
    pub concept_ids: Vec<String>,
    pub hints: Vec<Claim>,
    pub expected: Vec<Claim>,
    pub solution: Claim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DraftLearningPath {
    pub prerequisites: Vec<Prerequisite>,
    pub primary: DraftActivity,
    pub transfer: DraftActivity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningActivity {
    pub id: String,
    pub title: String,
    pub kind: ActivityKind,
    pub objective: String,
    pub task: String,
    pub starting_point: Claim,
    pub concept_ids: Vec<String>,
    pub provenance: String,
    pub hints: Vec<Claim>,
    pub available_hints: usize,
    pub expected: Vec<Claim>,
    pub solution: Option<Claim>,
    pub completion_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPath {
    pub revision_key: String,
    pub prerequisites: LearningGraph,
    pub selected_activity: LearningStage,
    pub primary: LearningActivity,
    pub transfer: LearningActivity,
    pub retention: String,
}

impl LearningPath {
    pub fn activity(&self, stage: LearningStage) -> &LearningActivity {
        match stage {
            LearningStage::Primary => &self.primary,
            LearningStage::Transfer => &self.transfer,
        }
    }

    pub(super) fn reveal(&mut self, selected: LearningStage, hint_level: u8, solution: bool) {
        self.selected_activity = selected;
        for (stage, activity) in [
            (LearningStage::Primary, &mut self.primary),
            (LearningStage::Transfer, &mut self.transfer),
        ] {
            activity.hints.truncate(if stage == selected {
                usize::from(hint_level)
            } else {
                0
            });
            if stage != selected || !solution {
                activity.expected.clear();
                activity.solution = None;
            }
        }
    }
}

pub(super) fn build_path(
    draft: DraftLearningPath,
    concepts: &[Concept],
    snapshot: &str,
) -> Result<LearningPath> {
    let graph = LearningGraph::build(concepts, draft.prerequisites)?;
    ensure!(
        draft.primary.kind != draft.transfer.kind
            && draft.primary.task.trim().to_lowercase()
                != draft.transfer.task.trim().to_lowercase()
            && draft.primary.objective.trim().to_lowercase()
                != draft.transfer.objective.trim().to_lowercase(),
        "transfer must be a distinct activity with a different objective and action"
    );
    let revision_key = util::json_digest(&(
        EXPERIENCE_PROMPT_VERSION,
        snapshot,
        &draft.primary,
        &draft.transfer,
        &graph,
    ))?;
    let activity = |draft: DraftActivity, stage: &str| -> Result<LearningActivity> {
        let digest = util::json_digest(&(snapshot, stage, &draft))?;
        Ok(LearningActivity {
            id: format!("activity_{}", &digest[7..23]),
            title: draft.title,
            kind: draft.kind,
            objective: draft.objective,
            task: draft.task,
            starting_point: draft.starting_point,
            concept_ids: draft.concept_ids,
            provenance: "generated_practice".into(),
            available_hints: draft.hints.len(),
            hints: draft.hints,
            expected: draft.expected,
            solution: Some(draft.solution),
            completion_status: "not_assessed".into(),
        })
    };
    Ok(LearningPath {
        revision_key,
        prerequisites: graph,
        selected_activity: LearningStage::Primary,
        primary: activity(draft.primary, "primary")?,
        transfer: activity(draft.transfer, "transfer")?,
        retention: "stateless; no learner answers or completion history are saved".into(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackBasis {
    ModelAssessed,
    SourceComparison,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackOutcome {
    ConsistentWithSelectedEvidence,
    NeedsRevision,
    ReviewNeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub basis: FeedbackBasis,
    pub outcome: FeedbackOutcome,
    pub message: String,
    pub comparison_points: Vec<Claim>,
    pub hints_requested: u8,
    pub limitations: String,
}

pub(super) fn fallback_feedback(activity: &LearningActivity, hints_requested: u8) -> Feedback {
    Feedback { basis: FeedbackBasis::SourceComparison, outcome: FeedbackOutcome::ReviewNeeded,
        message: "Compare your explanation with these source-grounded conditions. This fallback does not grade your answer.".into(),
        comparison_points: activity.expected.clone(), hints_requested,
        limitations: "A source comparison is not an independent test, a verified contribution, or evidence of mastery.".into() }
}

pub(super) fn fallback_path(
    catalog: &source::Catalog,
    concepts: &[Concept],
    snapshot: &str,
) -> Option<LearningPath> {
    let record = catalog
        .records
        .iter()
        .find(|r| {
            r.describes_present_project() && matches!(r.kind.as_str(), "constraint" | "decision")
        })
        .or_else(|| {
            catalog
                .records
                .iter()
                .find(|r| r.describes_present_project())
        })?;
    let support = record.claim.clone();
    let concept_ids = concepts
        .iter()
        .filter(|c| {
            c.description
                .evidence_ids
                .iter()
                .any(|id| support.evidence_ids.contains(id))
        })
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    let mut scenario = support.clone();
    scenario.basis = ClaimBasis::Hypothetical;
    scenario.text = format!(
        "Practice with the documented condition about {}. Treat a proposed exception as hypothetical, not as an existing project issue.",
        record.title
    );
    let hint = |text: String| Claim {
        text,
        basis: ClaimBasis::Inference,
        evidence_ids: support.evidence_ids.clone(),
        observation_ids: Vec::new(),
        qualifications: support.qualifications.clone(),
    };
    let primary = DraftActivity {
        title: format!("Predict a boundary for {}", record.title), kind: ActivityKind::Predict,
        objective: "Apply a project condition to a concrete proposed exception.".into(),
        task: format!("Propose one concrete situation in which the condition about {} would be missed. Predict what the documented condition requires in that situation and explain your evidence.", record.title),
        starting_point: scenario.clone(), concept_ids: concept_ids.clone(),
        hints: vec![hint("Start by naming the condition's scope: when, where and to which operation it applies.".into()),
            hint("Contrast an in-scope example with a case outside the documented scope; do not assume the rule applies everywhere.".into()), support.clone()],
        expected: vec![support.clone()], solution: support.clone(),
    };
    let transfer = DraftActivity {
        title: format!("Explain a different change to {}", record.title), kind: ActivityKind::Explain,
        objective: "Transfer the same condition to a different change without reusing the first scenario.".into(),
        task: format!("Choose a different proposed change involving {}. Explain how you would preserve the documented condition, and name an independently checkable observation that would show your explanation was wrong. Use a different situation from the prediction activity.", record.title),
        starting_point: scenario, concept_ids,
        hints: vec![hint("Choose a different input or failure boundary, then identify which part of the original condition still applies.".into()),
            hint("Describe what a reviewer would observe in the changed behavior; do not report a test as run.".into()), support.clone()],
        expected: vec![support.clone()], solution: support,
    };
    build_path(
        DraftLearningPath {
            prerequisites: Vec::new(),
            primary,
            transfer,
        },
        concepts,
        snapshot,
    )
    .ok()
}
