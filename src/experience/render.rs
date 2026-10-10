//! Portable, escaped Markdown. Source identifiers always have a usable CLI
//! drill-down, without assuming a web reader or inventing source URLs.

use super::*;
use crate::context::decision::runtime;
use std::fmt::Write;

fn text(value: &str) -> String {
    util::markdown_text(value)
}

fn citations(evidence: &[String], observations: &[String]) -> String {
    let mut links = evidence
        .iter()
        .map(|id| format!("[`{id}`](#source-{id})"))
        .collect::<Vec<_>>();
    links.extend(
        observations
            .iter()
            .map(|id| format!("[`{id}`](#observation-{id})")),
    );
    if links.is_empty() {
        String::new()
    } else {
        format!(" {}", links.join(" "))
    }
}

fn claim(output: &mut String, value: &Claim) {
    let _ = writeln!(
        output,
        "{} *({})*{}\n",
        text(&value.text),
        value.basis.label(),
        citations(&value.evidence_ids, &value.observation_ids)
    );
    for qualification in &value.qualifications {
        let _ = writeln!(output, "> {}", text(qualification));
    }
    if !value.qualifications.is_empty() {
        output.push('\n');
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn render_markdown(result: &ExperienceResult) -> String {
    let mut output = format!("# {}\n\n", text(&result.project));
    match result.mode {
        ExperienceMode::HowTo => {
            if let Some(task) = &result.first_task {
                let _ = writeln!(
                    output,
                    "## {}\n\n**{}** — {}\n",
                    text(&task.task),
                    text(&task.readiness),
                    text(&task.approach)
                );
                if !task.completion_checks.is_empty() {
                    output.push_str("### Completion checks\n\n");
                    for check in &task.completion_checks {
                        let _ = writeln!(output, "- {}", text(check));
                    }
                    output.push('\n');
                }
                let _ = writeln!(output, "{}\n", text(&task.qualification));
            }
        }
        ExperienceMode::Reference => {
            let _ = writeln!(output, "## Exact references: {}\n", text(&result.goal));
            if result.references.is_empty() {
                output.push_str("No exact eligible reference was retrieved for this question. This does not mean that no relevant rule exists.\n\n");
            }
            for reference in &result.references {
                let _ = writeln!(
                    output,
                    "### {}\n\n{} · {}\n",
                    text(&reference.source),
                    text(&reference.basis),
                    if reference.current {
                        "current in the retained source registry"
                    } else {
                        "historical evidence"
                    }
                );
                for line in reference.excerpt.lines() {
                    let _ = writeln!(output, "> {}", text(line));
                }
                let _ = writeln!(
                    output,
                    "\n`lore evidence {}` · revision `{}`\n",
                    reference.evidence_id, reference.revision_id
                );
            }
        }
        _ => {}
    }
    if let Some(purpose) = &result.orientation.purpose {
        output.push_str("## What the project does\n\n");
        claim(&mut output, purpose);
    }
    if let Some(rationale) = &result.orientation.rationale {
        output.push_str("## Why this design exists\n\n");
        claim(&mut output, rationale);
    }
    if !result.orientation.concepts.is_empty() {
        output.push_str("## Key concepts\n\n");
        for concept in &result.orientation.concepts {
            let _ = writeln!(output, "### {}\n", text(&concept.title));
            claim(&mut output, &concept.description);
        }
    }
    if !result.orientation.architecture.is_empty() {
        output.push_str("## How the responsibilities fit together\n\n");
        for value in &result.orientation.architecture {
            claim(&mut output, value);
        }
    }
    if let Some(workflow) = &result.orientation.workflow {
        let _ = writeln!(output, "## Follow a workflow: {}\n", text(&workflow.title));
        for (i, stop) in workflow.stops.iter().enumerate() {
            let _ = writeln!(output, "### {}. {}\n", i + 1, text(&stop.title));
            claim(&mut output, &stop.description);
        }
        for limitation in &workflow.limitations {
            let _ = writeln!(output, "{}\n", text(limitation));
        }
    }
    if !result.orientation.constraints.is_empty() {
        output.push_str("## Conditions to keep in view\n\n");
        for condition in &result.orientation.constraints {
            claim(&mut output, condition);
        }
    }
    if let Some(path) = &result.tutorial {
        let activity = path.activity(path.selected_activity);
        let title = if path.selected_activity == LearningStage::Transfer {
            "Try a distinct transfer activity"
        } else {
            "Learn by doing"
        };
        let _ = writeln!(
            output,
            "## {title}: {}\n\n**Objective:** {}\n\n{}\n",
            text(&activity.title),
            text(&activity.objective),
            text(&activity.task)
        );
        output.push_str(
            "This is a generated practice exercise. You may skip it or change direction.\n\n",
        );
        claim(&mut output, &activity.starting_point);
        if !path.prerequisites.prerequisites.is_empty() {
            output.push_str("### Suggested preparation\n\nThese educational dependencies are optional; they are not project rules or skill gates.\n\n");
            let title_for = |id: &String| {
                result
                    .orientation
                    .concepts
                    .iter()
                    .find(|c| &c.id == id)
                    .map(|c| text(&c.title))
                    .unwrap_or_else(|| text(id))
            };
            for group in &path.prerequisites.suggested_order {
                let _ = writeln!(
                    output,
                    "- {}",
                    group.iter().map(title_for).collect::<Vec<_>>().join(" + ")
                );
            }
            if !path.prerequisites.co_learned.is_empty() {
                output.push_str(
                    "\nCircular prerequisite suggestions are grouped for learning together.\n",
                );
            }
            output.push('\n');
        }
        if !activity.hints.is_empty() {
            output.push_str("### Requested hints\n\n");
            for (i, hint) in activity.hints.iter().enumerate() {
                let _ = writeln!(output, "**Hint {}**\n", i + 1);
                claim(&mut output, hint);
            }
        }
        if let Some(solution) = &activity.solution {
            output.push_str("### One source-grounded example\n\n");
            claim(&mut output, solution);
            for expected in &activity.expected {
                claim(&mut output, expected);
            }
        }
        let _ = writeln!(
            output,
            "Optional controls: `--hint 1` through `--hint 3`, `--show-solution`, `--answer 'your explanation'`, or `--activity transfer`. {} hint(s) available.\n",
            activity.available_hints
        );
        let _ = writeln!(
            output,
            "Pin feedback to this exact activity with `--lesson {}`.\n",
            path.revision_key
        );
        output.push_str("This activity has not been independently assessed. No progress, answers or profile are saved.\n\n");
    }
    if let Some(feedback) = &result.feedback {
        output.push_str("## Feedback on the supplied explanation\n\n");
        let _ = writeln!(output, "{}\n", text(&feedback.message));
        for point in &feedback.comparison_points {
            claim(&mut output, point);
        }
        let _ = writeln!(output, "{}\n", text(&feedback.limitations));
    }
    if !result.orientation.next_exploration.is_empty() {
        output.push_str("## Explore next\n\n");
        for next in &result.orientation.next_exploration {
            let command = format!(
                "lore onboard --topic {} --mode {}",
                shell_quote(&next.goal),
                next.mode.label()
            );
            let _ = writeln!(
                output,
                "- **{}**{}\n\n  `{}`\n",
                text(&next.label),
                citations(&next.evidence_ids, &next.observation_ids),
                command.replace('`', "\\`")
            );
        }
    }
    if !result.orientation.gaps.is_empty() {
        output.push_str("## Scope of this orientation\n\n");
        for gap in &result.orientation.gaps {
            let _ = writeln!(output, "- {}", text(gap));
        }
        output.push('\n');
    }
    if result.mode == ExperienceMode::HowTo {
        output.push_str("## Shared task intelligence\n\n");
        output.push_str(&runtime::render(&result.intelligence));
        output.push('\n');
    }
    output.push_str(&result.source_relationships.render());
    output.push_str("## Exact evidence\n\n");
    match &result.intelligence {
        DecisionContextResult::Brief(shared) => {
            for evidence in &shared.evidence {
                let _ = writeln!(
                    output,
                    "### Source {}\n\n`{}` — {} · {} · revision `{}`\n\n`lore evidence {}`\n",
                    evidence.id,
                    text(&evidence.source),
                    text(&evidence.basis),
                    if evidence.current {
                        "current documentary/import support"
                    } else {
                        "historical support"
                    },
                    evidence.revision_id,
                    evidence.id
                );
            }
            for observation in &shared.inspection.observations {
                let _ = writeln!(
                    output,
                    "### Observation {}\n\n`{}` lines {}–{} · `{}`\n\n{}\n",
                    observation.id,
                    text(&observation.path),
                    observation.start_line,
                    observation.end_line,
                    observation.content_hash,
                    text(&observation.qualification)
                );
                for line in observation.excerpt.lines() {
                    let _ = writeln!(output, "> {}", text(line));
                }
                output.push('\n');
            }
            for warning in &shared.warnings {
                let _ = writeln!(output, "{}\n", text(warning));
            }
        }
        DecisionContextResult::FastFallback(shared) => {
            for evidence in &shared.context.evidence {
                let _ = writeln!(
                    output,
                    "### Source {}\n\n`{}` · revision `{}`\n\n`lore evidence {}`\n",
                    evidence.id,
                    text(&evidence.source),
                    evidence.source_revision_id,
                    evidence.id
                );
            }
            for evidence in &shared.context.imported_evidence {
                let _ = writeln!(
                    output,
                    "### Source {}\n\nImported report `{}` · snapshot `{}`\n\n`lore evidence {}`\n",
                    evidence.id,
                    text(&evidence.source),
                    evidence.snapshot_id,
                    evidence.id
                );
            }
            for warning in &shared.context.warnings {
                let _ = writeln!(output, "{}\n", text(warning));
            }
        }
    }
    for warning in &result.warnings {
        let _ = writeln!(output, "{}\n", text(warning));
    }
    if result.omitted_items > 0 {
        let _ = writeln!(
            output,
            "{} whole optional presentation item(s) omitted to fit the output budget.\n",
            result.omitted_items
        );
    }
    let basis = match result.generation_basis {
        GenerationBasis::ModelAssessed => "source-checked model presentation",
        GenerationBasis::DeterministicFallback => "extractive source presentation",
    };
    let _ = writeln!(
        output,
        "---\n\n{} · status `{}` · {} model call(s), including {} presentation call(s) · {}/{} tokens\n\nSnapshot `{}`\n",
        basis,
        result.presentation_status,
        result.model_calls,
        result.presentation_model_calls,
        result.budget.used_tokens,
        result.budget.max_tokens,
        result.snapshot.registry_revision
    );
    output
}
