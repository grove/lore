use super::{CaseResult, DecisionLenses, DocumentedFacet, InsightCoverage};
use crate::{context, util};

fn text(value: &str) -> String {
    util::markdown_text(
        &value
            .chars()
            .flat_map(|character| {
                if character.is_control() {
                    character.escape_default().collect::<Vec<_>>()
                } else {
                    vec![character]
                }
            })
            .collect::<String>(),
    )
}

fn quote(value: &str) -> String {
    value
        .lines()
        .map(|line| format!("> {}\n", text(line)))
        .collect::<String>()
}

fn facets(values: &[DocumentedFacet], output: &mut String) {
    for facet in values {
        output.push_str(&format!("### {}\n\n", facet.kind.label()));
        output.push_str(&format!(
            "Source: {}:{}; evidence `{}`; {} retained support.\n\n",
            text(&facet.evidence.root_id),
            text(&facet.evidence.path),
            text(&facet.evidence.id),
            if facet.current {
                "current"
            } else {
                "historical"
            }
        ));
        for record in &facet.records {
            output.push_str(&format!(
                "Record `{}`: {}; lifecycle {} (original {}); scope {}; support {}.\n\n",
                text(&record.knowledge_id),
                text(&record.kind),
                text(&record.lifecycle),
                text(&record.original_lifecycle),
                text(&record.scope),
                text(&record.support_state)
            ));
        }
        output.push_str(&quote(&facet.evidence.excerpt));
        if !facet.evidence.context_before.is_empty() || !facet.evidence.context_after.is_empty() {
            output.push_str(
                "\nRetained surrounding context (bounded, same captured source revision):\n\n",
            );
            output.push_str(&quote(&format!(
                "{}{}{}",
                facet.evidence.context_before, facet.evidence.excerpt, facet.evidence.context_after
            )));
        }
        output.push('\n');
    }
}

fn coverage(value: &InsightCoverage, output: &mut String) {
    output.push_str(&format!(
        "\nCoverage: {} selected records; {} complete views omitted. {}\n",
        value.selected_records, value.omitted_complete_items, value.qualification
    ));
}

pub fn render_decisions(result: &DecisionLenses) -> String {
    let mut output = format!("# Decisions: {}\n\n", text(&result.query));
    if result.lenses.is_empty() {
        output.push_str("No complete decision lens was selected within this budget. The relevant retained context and its coverage follow.\n\n");
    }
    for lens in &result.lenses {
        output.push_str(&format!(
            "## {}\n\n{}\n\nKind: {}. Lifecycle: {} (original: {}). Scope: {}. Support: {}.\n\n",
            text(&lens.record.subject),
            text(&lens.record.statement),
            text(&lens.record.kind),
            text(&lens.record.lifecycle),
            text(&lens.original_lifecycle),
            text(&lens.record.scope),
            text(&lens.record.support_state)
        ));
        for qualification in &lens.record.qualifications {
            output.push_str(&format!("{}\n\n", text(qualification)));
        }
        facets(&lens.facets, &mut output);
        if !lens.history.is_empty() {
            output.push_str("### Documentary history and disagreements\n\n");
            for relation in &lens.history {
                output.push_str(&format!(
                    "- `{}` {} `{}`; evidence `{}` ({} witness).\n",
                    text(&relation.from),
                    text(&relation.kind),
                    text(&relation.to),
                    text(&relation.evidence_id),
                    if relation.current {
                        "current"
                    } else {
                        "historical"
                    }
                ));
            }
            output.push('\n');
        }
    }
    if !result.negative_knowledge.is_empty() {
        output.push_str("## Relevant negative knowledge\n\n");
        for item in &result.negative_knowledge {
            if let Some(record) = result
                .context
                .sections
                .items()
                .find(|record| record.id == item.knowledge_id)
            {
                output.push_str(&format!(
                    "- {} — {}; scope: {}; evidence: {}.\n",
                    text(&record.statement),
                    text(&item.basis),
                    text(&record.scope),
                    item.evidence_ids
                        .iter()
                        .map(|id| format!("`{}`", text(id)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        output.push('\n');
    }
    output.push_str("## Shared source context\n\n");
    output.push_str(&context::render_context(&result.context));
    coverage(&result.coverage, &mut output);
    output.push_str(&format!(
        "\nFull view budget: {}/{} {} tokens. Model calls: 0.\n",
        result.budget.used_tokens, result.budget.max_tokens, result.budget.tokenizer
    ));
    output
}

pub fn render_cases(result: &CaseResult) -> String {
    let mut output = format!(
        "# Cases: {}\n\nThese are retained source cases. Lore has not executed a repository command, test or replay.\n\n",
        text(&result.query)
    );
    if result.cases.is_empty() {
        output.push_str("No complete explicit case was selected within this budget. Relevant source context remains available below.\n\n");
    }
    for case in &result.cases {
        output.push_str(&format!(
            "## {}\n\n{}\n\n",
            text(&case.title),
            case.kind.label()
        ));
        if let Some(record) = &case.record {
            output.push_str(&format!(
                "{}\n\nLifecycle: {}. Scope: {}. {}\n\n",
                text(&record.statement),
                text(&record.lifecycle),
                text(&record.scope),
                text(&record.documentary_basis)
            ));
            for qualification in &record.qualifications {
                output.push_str(&format!("{}\n\n", text(qualification)));
            }
        }
        if let Some(record) = &case.imported_record {
            output.push_str(&format!(
                "{}\n\nSource-owned lifecycle: {}.\n\n",
                text(&record.statement),
                text(&record.lifecycle)
            ));
            for qualification in &record.qualifications {
                output.push_str(&format!("{}\n\n", text(qualification)));
            }
        }
        output.push_str(&format!("{}\n\n", case.qualification));
        facets(&case.facets, &mut output);
    }
    output.push_str("## Shared source context\n\n");
    output.push_str(&context::render_context(&result.context));
    coverage(&result.coverage, &mut output);
    output.push_str(&format!(
        "\nFull view budget: {}/{} {} tokens. Model calls: 0. Tests run: 0.\n",
        result.budget.used_tokens, result.budget.max_tokens, result.budget.tokenizer
    ));
    output
}
