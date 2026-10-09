//! Compact human entry point for imported knowledge, without model-generated
//! summaries or a second mirror of every external issue and memory.
use crate::{storage::StoredPage, util};
use anyhow::Result;
use rusqlite::Connection;
use std::collections::BTreeMap;

const START: &str = "<!-- lore:native-imports:start -->";
const END: &str = "<!-- lore:native-imports:end -->";

pub fn augment(conn: &Connection, pages: &mut BTreeMap<String, StoredPage>) -> Result<()> {
    let observations = super::views(conn)?;
    let relations = super::relationships::relations(conn)?;
    let review_status: BTreeMap<_, _> = crate::reviews::list(conn, true)?
        .into_iter()
        .map(|review| (review.id, review.status))
        .collect();
    let mut counts = BTreeMap::<(String, String), (usize, usize)>::new();
    for observation in &observations {
        let count = counts
            .entry((
                observation.import_id.clone(),
                observation.origin.as_str().into(),
            ))
            .or_default();
        if observation.current {
            count.0 += 1;
        } else {
            count.1 += 1;
        }
    }
    let mut warnings = super::warnings(conn)?;
    warnings.extend(super::relationships::warnings(conn)?);
    let has_imports = !counts.is_empty() || !warnings.is_empty();
    if let Some(index) = pages.get_mut("index.md") {
        if let Some(start) = index.content.find(START) {
            if let Some(end) = index.content[start..].find(END) {
                index
                    .content
                    .replace_range(start..start + end + END.len(), "");
            }
        }
        if has_imports {
            index.content = format!(
                "{}\n\n{START}\n## Connected project knowledge\n\n[Native observations and cross-source questions](imports.md) connects imported implementation reports, work history, and agent recollections with documentary knowledge. Use `lore context \"your task\"` to retrieve related evidence.\n{END}\n",
                index.content.trim_end()
            );
        }
        index.output_digest = util::digest(&index.content);
    }
    if !has_imports {
        return Ok(());
    }
    let mut content = String::from(
        "# Connected project knowledge\n\n[Project overview](index.md)\n\nEach upstream system owns its records. Lore retains snapshots and qualified relationships. Current below means present in the last successful import; it does not verify the current checkout or a running service.\n\n| Import | Source | Current records | Withdrawn records |\n| --- | --- | ---: | ---: |\n",
    );
    for ((id, kind), (current, retired)) in &counts {
        content.push_str(&format!(
            "| {} | {} | {current} | {retired} |\n",
            util::markdown_text(id),
            kind
        ));
    }
    content.push_str("\n## Questions and relationships\n\nThese are evidence-bound interpretations. A possible discrepancy requires verification; a closed issue or a recollection never establishes accepted policy or deployed behavior.\n\n");
    let shown = relations
        .iter()
        .filter(|r| r.is_question())
        .take(100)
        .collect::<Vec<_>>();
    if shown.is_empty() {
        content.push_str("No current cross-source questions were recorded. This does not establish that all sources agree or that relevant evidence is complete.\n\n");
    }
    let mut displayed_questions = 0;
    for relation in shown {
        let mut entry = format!(
            "### {}\n\n{}\n\n",
            util::markdown_text(&relation.kind.replace('_', " ")),
            util::markdown_text(&relation.reason)
        );
        for evidence in &relation.evidence_ids {
            entry.push_str(&format!(
                "- Evidence: `{evidence}` — inspect with `lore --json evidence {evidence}`.\n"
            ));
        }
        if let Some(review) = &relation.review_id {
            entry.push_str(&format!("- Review: `{review}` — {}. A disposition does not establish implementation correctness.\n",review_status.get(review).map(String::as_str).unwrap_or("unknown")));
        }
        entry.push('\n');
        for qualification in &relation.qualifications {
            entry.push_str(&format!("{}\n\n", util::markdown_text(qualification)));
        }
        // Publication reads managed pages under a four-megabyte limit. Native
        // relation payloads and evidence fan-out can be much larger; keep each
        // displayed group complete or omit it, leaving room for diagnostics.
        if content.len() + entry.len() <= 3_000_000 {
            content.push_str(&entry);
            displayed_questions += 1;
        }
    }
    let question_count = relations.iter().filter(|r| r.is_question()).count();
    if question_count > displayed_questions {
        content.push_str(&format!(
            "{} additional questions were omitted from this compact entry point because of its size or item budget. They remain available through task context and the review queue.\n\n",
            question_count - displayed_questions
        ));
    }
    content.push_str("## Inspect task-specific evidence\n\nRun `lore context \"describe the change\"` to select complete relevant groups within an explicit output budget. Native evidence IDs resolve to the original parsed record, content hash, source locators, and verification metadata. Imported records are retained without generating a prose summary for every record.\n\n");
    let mut abbreviated = warnings.len().saturating_sub(20);
    for warning in warnings.iter().take(20) {
        let sample: String = warning.chars().take(400).collect();
        let shortened = sample.len() < warning.len();
        abbreviated += usize::from(shortened);
        content.push_str(&format!(
            "- {}{}\n",
            util::markdown_text(&sample),
            if shortened { "…" } else { "" }
        ));
    }
    if abbreviated > 0 {
        content.push_str(&format!(
            "\n{abbreviated} source diagnostics were omitted or shortened in this compact entry point. Full diagnostics remain in the registry and update report.\n"
        ));
    }
    let page = StoredPage {
        path: "imports.md".into(),
        input_digest: util::digest(&content),
        output_digest: util::digest(&content),
        content,
    };
    pages.insert(page.path.clone(), page);
    Ok(())
}
