//! A topology hit is conditional on current grouping inputs and source-derived
//! memberships. Cached titles, labels, leaf priority and scope are not trusted
//! merely because a file retained an old digest.

use super::*;

pub(in crate::knowledge) fn grouping_signature(graph: &KnowledgeGraph) -> Result<String> {
    let records = graph
        .knowledge
        .iter()
        .map(|record| {
            (
                &record.id,
                &record.topic,
                &record.topic_title,
                &record.subject,
                critical(record),
            )
        })
        .collect::<Vec<_>>();
    let relations = graph
        .relations
        .iter()
        .map(|relation| (&relation.id, &relation.kind, &relation.from, &relation.to))
        .collect::<Vec<_>>();
    util::json_digest(&(
        graph.schema_version,
        &graph.grouping_version,
        &graph.root_id,
        &graph.options,
        records,
        relations,
    ))
}

/// Canonical topology validation deliberately repeats the bounded membership,
/// intersection and nearest-parent planning work. A checksum cannot prove that
/// an edited cache retained every derived group or semantic parent. This plan
/// is also retained for a full rebuild if any later cache validation fails.
pub(in crate::knowledge) fn validate_reusable_topology(
    graph: &KnowledgeGraph,
    previous_report: &BuildReport,
    expected: &Layout,
) -> Result<()> {
    ensure!(
        graph.grouping_version == GROUPING_VERSION
            && graph.root_id == stable_id("root", "project")
            && graph.model_calls == 0,
        "incompatible cached grouping contract"
    );
    ensure!(
        graph.nodes.len() == expected.nodes.len() && graph.edges == expected.edges,
        "cached topology differs from the complete canonical source layout"
    );
    let records = record_map(graph);
    for (node, canonical) in graph.nodes.iter().zip(&expected.nodes) {
        ensure!(
            node.id == canonical.id
                && node.kind == canonical.kind
                && node.title == canonical.title
                && node.grouping_basis == canonical.grouping_basis
                && node.knowledge_ids.iter().eq(canonical.ids.iter()),
            "cached node identity, membership or source labels differ from the canonical layout"
        );
        let critical_ids = canonical
            .ids
            .iter()
            .filter(|id| critical(records[id.as_str()]))
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            node.critical_knowledge_ids == critical_ids
                && node.coverage
                    == Coverage {
                        eligible_units: canonical.ids.len(),
                        included_units: canonical.ids.len(),
                        omitted_units: 0,
                        omission_reasons: Vec::new(),
                    },
            "cached node lost source scope or critical conditions"
        );
    }
    // Unsupported input is re-counted from the current registry, independently
    // of stable navigation membership. Never publish its cached value. Every
    // layout-derived report field must match the complete current plan.
    ensure!(
        previous_report.work_used == expected.report.work_used
            && previous_report.work_limit == expected.report.work_limit
            && previous_report.truncated == expected.report.truncated
            && previous_report.omitted_navigation_units == expected.report.omitted_navigation_units
            && previous_report.warnings == expected.report.warnings,
        "cached build report differs from the canonical source layout"
    );
    Ok(())
}
