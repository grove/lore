//! Hash authoritative source objects once, then bind each view to its own
//! membership and incident documentary relationships. The hashes accelerate
//! dependency comparison; they never authenticate cache-supplied prose.

use super::*;

const DEPENDENCY_VERSION: &str = "knowledge-view-dependencies-v2";

pub(in crate::knowledge) struct Dependencies {
    pub records: BTreeMap<String, String>,
}

impl Dependencies {
    pub fn new(graph: &KnowledgeGraph) -> Result<Self> {
        let evidence: BTreeMap<_, _> = graph.evidence.iter().map(|e| (&e.id, e)).collect();
        let sources: BTreeMap<_, _> = graph
            .source_revisions
            .iter()
            .map(|source| (&source.id, source))
            .collect();
        let mut evidence_hashes = BTreeMap::new();
        for (id, item) in &evidence {
            let source = sources
                .get(&item.source_revision_id)
                .context("missing dependency source revision")?;
            evidence_hashes.insert(*id, util::json_digest(&(item, source))?);
        }
        let mut originals = BTreeMap::new();
        for record in &graph.knowledge {
            let support = record
                .evidence
                .iter()
                .map(|item| {
                    Ok((
                        &item.id,
                        evidence_hashes
                            .get(&item.id)
                            .context("missing dependency evidence")?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            originals.insert(
                &record.id,
                util::json_digest(&(DEPENDENCY_VERSION, record, support))?,
            );
        }
        let mut incident = BTreeMap::<&String, BTreeMap<&String, String>>::new();
        for relation in &graph.relations {
            let digest = util::json_digest(&(
                relation,
                evidence_hashes
                    .get(&relation.evidence_id)
                    .context("missing relationship dependency evidence")?,
                originals
                    .get(&relation.from)
                    .context("missing relationship source endpoint")?,
                originals
                    .get(&relation.to)
                    .context("missing relationship target endpoint")?,
            ))?;
            for endpoint in [&relation.from, &relation.to] {
                incident
                    .entry(endpoint)
                    .or_default()
                    .insert(&relation.id, digest.clone());
            }
        }
        let records = originals
            .into_iter()
            .map(|(id, original)| {
                Ok((
                    id.clone(),
                    util::json_digest(&(original, incident.get(id)))?,
                ))
            })
            .collect::<Result<_>>()?;
        Ok(Self { records })
    }

    pub fn revision(&self, node: &ViewNode) -> Result<String> {
        let records = node
            .knowledge_ids
            .iter()
            .map(|id| Ok((id, self.records.get(id).context("unknown view dependency")?)))
            .collect::<Result<Vec<_>>>()?;
        util::json_digest(&(
            DEPENDENCY_VERSION,
            GROUPING_VERSION,
            &node.id,
            &node.kind,
            &node.title,
            &node.summary,
            &node.summary_knowledge_ids,
            &node.summary_coverage,
            &node.grouping_basis,
            &node.knowledge_ids,
            &node.critical_knowledge_ids,
            &node.child_view_ids,
            &node.parent_view_ids,
            &node.coverage,
            records,
        ))
    }
}
