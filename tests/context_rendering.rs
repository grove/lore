//! Synthetic presentation goldens; trust-boundary and runtime tests validate the
//! underlying decisions. These examples make no recommendation-quality claim.
use lore::context::{
    self, ContextSections,
    adaptive::{self, AdaptiveResult},
};
use serde_json::{Value, json};

fn advice(text: &str, ids: &[&str]) -> Value {
    json!({"text": text, "evidence_ids": ids, "observation_ids": []})
}

fn example(case: &str) -> AdaptiveResult {
    let mut result: Value =
        serde_json::from_str(include_str!("fixtures/context-rendering/supported.json")).unwrap();
    let intelligence = &mut result["intelligence"];
    match case {
        "supported" => {
            intelligence["inspection"]["status"] = json!("complete");
            intelligence["inspection"]["budget"]["files_read"] = json!(2);
            intelligence["inspection"]["budget"]["bytes_read"] = json!(80);
            intelligence["inspection"]["observations"] = json!([{
                "id": "co_queue", "path": "src/queue.rs", "start_line": 1, "end_line": 1,
                "excerpt": "const CAPACITY: usize = 128;", "content_hash": "sha256:current-digest",
                "kind": "static_source", "qualification": "Verified local static read; no runtime behavior or test execution established."
            }, {
                "id": "co_test", "path": "tests/queue.rs", "start_line": 1, "end_line": 1,
                "excerpt": "#[test] fn queue_rejects_overflow() {}", "content_hash": "sha256:test-digest",
                "kind": "static_test", "qualification": "Test declaration read; this test was not run."
            }]);
            intelligence["brief"]["implementation_seams"] = json!([{
                "path": "src/queue.rs", "observation_ids": ["co_queue"],
                "purpose": "Extract allocation behind the observed capacity declaration."
            }]);
            result["capabilities"]["inspection"] = json!("granted");
        }
        "blocker" => {
            let brief = &mut intelligence["brief"];
            brief["readiness"] = json!("blocked");
            brief["change_kind"] = json!("policy_changing");
            brief["preferred_approach"] = advice(
                "Keep the production bound until its owner decides whether to allow an unbounded queue.",
                &["ev_rule", "ev_rejected"],
            );
            brief["next_action"] = advice(
                "Ask the production policy owner to decide on the proposed unbounded queue before changing admission.",
                &["ev_rule"],
            );
            brief["constraints"][0]["disposition"] = json!("proposed_deviation");
            brief["material_blockers"] = json!([{
                "kind": "policy_decision", "knowledge_id": "rule",
                "explanation": "An unbounded production queue contradicts the accepted 128-entry limit.",
                "decision_needed": "Approve or reject changing the production admission contract.",
                "evidence_ids": ["ev_rule"], "observation_ids": []
            }]);
        }
        "conflict" => {
            let mut counterpart = intelligence["brief"]["facts"][0].clone();
            counterpart["record_id"] = json!("counterpart");
            counterpart["statement"] = json!("Production queues hold at most 64 entries.");
            counterpart["evidence_ids"] = json!(["ev_counterpart"]);
            counterpart["qualifications"] = json!([
                "Unresolved disagreement with the 128-entry policy; neither source supersedes the other."
            ]);
            intelligence["brief"]["facts"]
                .as_array_mut()
                .unwrap()
                .push(counterpart);
            let mut evidence = intelligence["evidence"][0].clone();
            evidence["id"] = json!("ev_counterpart");
            evidence["source"] = json!("docs/other-policy.md");
            intelligence["evidence"]
                .as_array_mut()
                .unwrap()
                .push(evidence);
            let brief = &mut intelligence["brief"];
            brief["readiness"] = json!("proceed_after_check");
            brief["constraints"][0]["disposition"] = json!("needs_verification");
            brief["constraints"].as_array_mut().unwrap().push(json!({
                "knowledge_id": "counterpart", "disposition": "needs_verification",
                "explanation": "Check which production limit applies; retain both source claims until resolved.",
                "evidence_ids": ["ev_rule", "ev_counterpart"], "observation_ids": []
            }));
            brief["rationale"] = advice(
                "Both production policies claim current applicability; the staging exception supplies no supersession.",
                &["ev_rule", "ev_counterpart"],
            );
            brief["counterevidence"] = json!([{
                "hypothesis": "The production limit is unambiguously 128 entries.",
                "explanation": "A second current accepted policy says 64 entries for production; resolve both sources before choosing a limit.",
                "evidence_ids": ["ev_rule", "ev_counterpart"], "observation_ids": [], "material": true
            }]);
            brief["preferred_approach"] = advice(
                "Resolve the conflicting production limits before changing queue admission.",
                &["ev_rule", "ev_counterpart"],
            );
            brief["next_action"] = advice(
                "Compare both policy sources with their owner; establish which limit applies to production.",
                &["ev_rule", "ev_counterpart"],
            );
            brief["remaining_uncertainty"] = json!([advice(
                "The production limit is unresolved; the staging exception does not resolve this conflict.",
                &["ev_rule", "ev_counterpart", "ev_exception"]
            )]);
        }
        "no_documents" | "insufficient" => {
            let budget = intelligence["budget"].clone();
            let omissions = intelligence["omissions"].clone();
            *intelligence = json!({
                "schema_version": 4, "task": "Refactor queue allocation", "paths": [],
                "empty": true, "model_calls": 0, "retrieval_truncated": false,
                "budget": budget, "sections": ContextSections::default(),
                "relations": [], "reviews": [], "evidence": [], "suggested_inspection": [],
                "omissions": omissions, "warnings": [], "mode": "fast_fallback",
                "fallback_reason": "No relevant retained knowledge or inspectable source fits this task.",
                "cache_status": "unused", "inspection_status": "disabled",
                "investigation_status": "no_evidence", "model_call_limit": 6,
                "checkout_egress": {"allowed": false, "model_received_checkout": false, "reason": "No checkout egress grant."}
            });
        }
        "stale" => {
            intelligence["inspection"]["status"] = json!("complete");
            intelligence["inspection"]["checkout_freshness"] = json!([{
                "evidence_id": "ev_report", "resource": "repo://src/queue.rs",
                "version": "repo-file-v1:old-digest", "status": "changed_since_snapshot",
                "reason": "content_changed", "checked_path": "src/queue.rs",
                "content_hash": "sha256:current-digest", "observation_id": null,
                "qualification": "Checked bytes differ from the imported snapshot. Inspect current source before relying on the report; no tests were run."
            }]);
            let mut report = intelligence["brief"]["facts"][0].clone();
            report["record_id"] = json!("report");
            report["provenance"] = json!("imported_report");
            report["basis"] = json!("observed");
            report["statement"] =
                json!("The queue implementation enforces a 128-entry production bound.");
            report["kind"] = json!("implementation");
            report["lifecycle"] = json!("reported");
            report["evidence_ids"] = json!(["ev_report"]);
            report["qualifications"] = json!([
                "Imported OpenWiki report; the current checkout has changed since its snapshot."
            ]);
            intelligence["brief"]["facts"]
                .as_array_mut()
                .unwrap()
                .push(report);
            let mut evidence = intelligence["evidence"][0].clone();
            evidence["id"] = json!("ev_report");
            evidence["source"] = json!("openwiki/queue.md");
            evidence["basis"] = json!("imported_report");
            intelligence["evidence"]
                .as_array_mut()
                .unwrap()
                .push(evidence);
            let brief = &mut intelligence["brief"];
            brief["readiness"] = json!("proceed_after_check");
            brief["preferred_approach"] = advice(
                "Preserve the accepted production bound, after checking the current admission path.",
                &["ev_rule", "ev_report"],
            );
            brief["next_action"] = advice(
                "Inspect current src/queue.rs; reconcile the stale implementation report before extracting allocation.",
                &["ev_rule", "ev_report"],
            );
            brief["hypotheses"] = json!([{
                "provenance": "hypothesis", "text": "The current admission path may still enforce the documented bound.",
                "evidence_ids": ["ev_rule"], "observation_ids": [], "confidence": "low",
                "applicability": "Only if current-source inspection confirms the admission check.",
                "historical_only": false, "alternatives": [advice("The implementation may have drifted from the accepted rule.", &["ev_rule"])]
            }]);
            result["capabilities"]["inspection"] = json!("granted");
        }
        "denied" => {
            intelligence["brief"]["readiness"] = json!("proceed_after_check");
            intelligence["brief"]["next_action"] = advice(
                "Check the queue admission path with an authorized local reader before extracting allocation.",
                &["ev_rule"],
            );
            intelligence["brief"]["remaining_uncertainty"] = json!([advice(
                "Inspection was denied by the caller. Retained policy remains usable; current implementation compatibility is unverified.",
                &["ev_rule"]
            )]);
            result["capabilities"]["inspection"] = json!("disabled_by_caller");
        }
        "fallback" => {
            intelligence["mode"] = json!("fast_fallback");
            intelligence["model"] = json!("");
            intelligence["model_calls"] = json!(0);
            intelligence["cache_status"] = json!("unused");
            intelligence["fallback_reason"] = json!(
                "The configured provider is unavailable; retained evidence remains usable without fresh reasoning."
            );
            intelligence["investigation"]["stop_reason"] = json!("inference_unavailable");
            intelligence["investigation"]["budget"]["model_calls"] = json!(0);
            let brief = &mut intelligence["brief"];
            brief["generation_basis"] = json!("deterministic_fallback");
            brief["readiness"] = json!("proceed_after_check");
            brief["preferred_approach"] = advice(
                "Check compatibility with the retained production bound before choosing an implementation.",
                &["ev_rule"],
            );
            brief["next_action"] = advice(
                "Determine whether the queue refactor preserves the production bound and the staging-only exception.",
                &["ev_rule", "ev_exception"],
            );
        }
        _ => panic!("unknown golden: {case}"),
    }
    if result["intelligence"]["brief"]["readiness"] == "proceed_after_check" {
        let brief = &mut result["intelligence"]["brief"];
        brief["checks"][0]["priority"] = json!("required_before_proceeding");
        brief["checks"][0]["action"] = brief["next_action"]["text"].clone();
        brief["checks"][0]["evidence_ids"] = brief["next_action"]["evidence_ids"].clone();
        brief["checks"][0]["decision_impact"] =
            json!("This check determines whether an implementation choice is justified.");
    }
    if case == "insufficient" {
        result["intelligence"]["omissions"]["critical_groups"] = json!(1);
        result["intelligence"]["fallback_reason"] = json!(
            "Task evidence is incomplete: a complete constraint/exception group did not fit. Increase --max-tokens before relying on decision readiness."
        );
        result["intelligence"]["investigation_status"] = json!("incomplete_retrieval");
    }
    if matches!(case, "no_documents" | "insufficient" | "fallback") {
        result["usage"]["model_calls"] = json!(0);
        result["usage"]["provider_request_count"] = json!(0);
        result["usage"]["total_tokens"] = json!(0);
        result["usage"]["billed_cost_usd"] = json!(0.0);
        result["usage"]["event_count"] = json!(0);
    }
    serde_json::from_value(result).unwrap()
}

#[test]
fn schema_five_human_output_goldens() {
    for case in [
        "supported",
        "blocker",
        "conflict",
        "no_documents",
        "insufficient",
        "stale",
        "denied",
        "fallback",
    ] {
        let result = example(case);
        let before = serde_json::to_value(&result).unwrap();
        let rendered = adaptive::render(&result);
        assert!(context::count_tokens(&rendered) <= 3000, "{case}");
        assert!(
            context::count_tokens(&serde_json::to_string(&result).unwrap()) <= 3000,
            "{case}"
        );
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            before,
            "rendering must preserve the JSON contract"
        );
        let expected = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("tests/fixtures/context-rendering/{case}.md")),
        )
        .unwrap();
        assert_eq!(rendered, expected, "{case}");
    }
}

#[test]
fn first_screen_has_action_complete_rules_exception_and_evidence() {
    let rendered = adaptive::render(&example("supported"));
    let first_screen = rendered.lines().take(24).collect::<Vec<_>>().join("\n");
    for required in [
        "## Proceed",
        "**Preferred approach:**",
        "**Next action:**",
        "128",
        "production",
        "256",
        "staging",
        "ev_rule",
        "ev_exception",
        "Expires when the experiment ends",
    ] {
        assert!(
            first_screen.contains(required),
            "missing {required}: {first_screen}"
        );
    }
    assert_eq!(
        rendered
            .matches("Production queues hold at most 128 entries.")
            .count(),
        1
    );
}
