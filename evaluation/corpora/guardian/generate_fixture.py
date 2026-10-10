#!/usr/bin/env python3
"""Rebuild the explicitly synthetic 60-transition debugging corpus.

The labels and source assertions are authored together here. They are not
independent gold and cannot establish real-project alert efficacy.
"""
from __future__ import annotations

import json
from pathlib import Path


def build() -> dict:
    definitions = [
        ("payments", "payment retry", "3", "5",
         "Payments permit at most {limit} retry attempts except signed offline settlements.",
         "Payments permit at most {limit} retry attempts except signed offline settlements after durable idempotency acknowledgement.",
         "Preserve the retry ceiling and require durable idempotency acknowledgement for the offline exception.",
         "Payment diagnostics must redact complete payment tokens before persistence."),
        ("ledger", "ledger admission", "128", "256",
         "Ledger admission permits {limit} pending writes except operator-authorized drain transactions.",
         "Ledger admission permits {limit} pending writes except operator-authorized drain transactions after every tenant key is acknowledged.",
         "Keep admission bounded and check tenant acknowledgement before the operator drain exception.",
         "Ledger commits must atomically update the balance and its append-only audit entry."),
        ("releases", "artifact admission", "24", "48",
         "Release signatures expire after {limit} hours except pinned recovery artifacts.",
         "Release signatures expire after {limit} hours except pinned recovery artifacts with an independently retained signer certificate.",
         "Check signature age and require the retained signer certificate for pinned recovery artifacts.",
         "Release admission must reject unsigned artifacts before changing the active deployment."),
    ]
    projects = []
    for project, subject, first, second, rule, exception, action, additional in definitions:
        topic = project
        summary = f"The {subject} contract includes a bounded normal path and a qualified emergency exception."

        def record(key, statement, quote=None, *, kind="constraint", lifecycle="accepted", scope="production"):
            return {"key": key, "topic": topic, "topic_title": project.title(), "subject": subject,
                    "statement": statement, "quote": quote or statement, "kind": kind,
                    "lifecycle": lifecycle, "scope": scope, "effective_at": ""}

        def document(heading, entry, *, editorial=""):
            return {"text": f"# {heading}\n\n{entry['quote']}\n{editorial}", "records": [entry]}

        def policy(quote, editorial=""):
            return document("Accepted production contract", record("main", summary, quote), editorial=editorial)

        history = f"A historical development replay of {subject} completed in an isolated 2025 experiment; it does not describe production."
        work = f"A staging-only {subject} replay completed; production behavior and the emergency exception were not verified."
        current = policy(rule.format(limit=first))
        initial = {
            "docs/policy.md": current,
            "docs/history.md": document("Historical experiment", record("history", history, kind="reported_outcome", lifecycle="completed", scope="historical development")),
            "docs/work.md": document("Staging work report", record("work", work, kind="reported_outcome", lifecycle="completed", scope="staging")),
            "src/adapter.py": {"text": f'"""Synthetic {project} source; the guardian never executes it."""\nLIMIT = {int(first)}\n', "records": []},
            "README.md": {"text": f"# Synthetic {project}\n\nEvaluation source only.\n", "records": []},
        }
        events = []

        def add(number, significance, changes, *, scope="current production", alert=None, tags=(), interpretation=None, step=None):
            necessity = alert or ("required" if significance == "consequential" else "forbidden" if significance == "benign" else "optional")
            event = {"id": f"{project}-{number:02}", "sequence": number,
                     "parent": f"{project}-{number-1:02}", "revision": f"{project}-{number:02}",
                     "files": changes, "tags": list(tags), "expected": {
                         "significance": significance, "source_scope": scope,
                         "alert_necessity": necessity, "safe_next_step": step or action,
                         "runtime_verified": False,
                     }}
            if interpretation is not None:
                event["interpretation"] = interpretation
            events.append(event)

        add(1, "benign", {}, tags=["no_op"], step="Keep the named baseline; no source change requires a new assessment.")
        current = policy(rule.format(limit=second))
        add(2, "consequential", {"docs/policy.md": current}, tags=["same_size_edit", "accepted_constraint", "unchanged_summary"])
        proposal = f"Proposed staging-only {subject} experiment; this proposal does not replace the accepted production contract."
        add(3, "ambiguous", {"docs/proposal.md": document("Staging proposal", record("proposal", proposal, kind="proposal", lifecycle="proposed", scope="staging"))}, scope="proposed staging only", tags=["proposed_scope"], step="Keep production policy unchanged and confine any experiment to the documented staging scope.")
        current = policy(rule.format(limit=second), "\nEditorial navigation note.\n")
        add(4, "benign", {"docs/policy.md": current}, tags=["formatting", "outside_quote"])
        current = policy(exception.format(limit=second))
        add(5, "consequential", {"docs/policy.md": current}, tags=["rare_exception", "unchanged_summary", "reversion_B"])
        add(6, "ambiguous", {"docs/proposal.md": None}, scope="withdrawn proposal", tags=["deleted_source", "withdrawn_proposal"], step="Retain the withdrawn proposal as history; do not treat its removal as a production policy reversal.")
        current = policy(exception.format(limit=second), "\nA new citation location does not change the retained rule.\n")
        current["text"] = "\n\n" + current["text"]
        add(7, "benign", {"docs/policy.md": current}, tags=["citation_churn", "unchanged_quote"])
        current = policy(rule.format(limit=second))
        add(8, "consequential", {"docs/policy.md": current}, tags=["source_reversion_A", "unchanged_summary", "GUARD-REVERSION-001"])
        add(9, "ambiguous", {"docs/history.md": None}, scope="historical source withdrawn", tags=["historical_only", "deleted_source"], step="Preserve the historical experiment's qualification; its deletion establishes no current production behavior.")
        add(10, "benign", {}, tags=["no_op_after_history"])
        add(11, "consequential", {"docs/safety.md": document("Additional accepted constraint", record("safety", additional))}, tags=["accepted_constraint", "security_or_correctness"])
        updated_work = f"The staging-only {subject} replay was repeated successfully; production and emergency behavior remain unverified."
        relation = {"action": "set", "pair": "policy-work", "from": "work", "to": "main",
                    "kind": "verification_question", "reason": "The source-reported staging replay does not establish compliance with the accepted production exception.",
                    "qualifications": ["Staging-only source report; no production execution or policy replacement is established."]}
        add(12, "ambiguous", {"docs/work.md": document("Repeated staging report", record("work", updated_work, kind="reported_outcome", lifecycle="completed", scope="staging"))}, scope="current staging report", tags=["scope_only", "cross_source_interpretation_added"], interpretation=relation, step="Compare the staging report with the accepted production contract without promoting the report into production verification.")
        add(13, "benign", {"README.md": {"text": f"# Synthetic {project}\n\nEvaluation source only.\n\n## Navigation\n", "records": []}}, tags=["formatting", "unrelated_source"])
        breach = f"The source owner reports that the production {subject} path currently bypasses its accepted bound; independent runtime verification remains absent."
        add(14, "consequential", {"docs/incident.md": document("Production discrepancy report", record("incident", breach, kind="reported_outcome", lifecycle="completed"))}, tags=["reported_constraint_discrepancy"], step="Preserve the accepted bound and use permitted static inspection to locate the reported bypass; do not claim that the report proves runtime behavior.")
        add(15, "ambiguous", {}, scope="withdrawn interpretation", tags=["relationship_only", "cross_source_interpretation_withdrawn"], interpretation={"action": "withdraw", "pair": "policy-work"}, step="Drop the withdrawn interpretation from current advice while retaining both original sources and their scope; withdrawal does not establish agreement.")
        if project == "releases":
            rejected = f"The proposed {subject} staging exception was rejected and never became production policy."
            add(16, "ambiguous", {"docs/rejected.md": document("Rejected proposal", record("rejected", rejected, kind="proposal", lifecycle="rejected", scope="staging"))}, scope="rejected staging proposal", tags=["rejected_scope"], step="Keep the rejected proposal historical and preserve the accepted production rule.")
        else:
            add(16, "benign", {"README.md": {"text": f"# Synthetic {project}\n\nEvaluation source only.\n\n## Reference \n", "records": []}}, tags=["same_size_editorial", "unrelated_source"])
        if project == "ledger":
            add(17, "ambiguous", {"docs/incident.md": None}, scope="withdrawn production report", tags=["source_report_withdrawn"], step="Do not infer that withdrawing an incident report repaired the ledger; retain the accepted atomicity rule and source-history qualification.")
        else:
            fixed = f"The accepted production {subject} contract now explicitly requires fail-closed admission before any side effect; the emergency qualification remains in policy.md."
            add(17, "consequential", {"docs/fail-closed.md": document("Fail-closed constraint", record("fail-closed", fixed))}, tags=["accepted_constraint", "scope_preservation"])
        revised = dict(relation, reason="The withdrawn comparison is reconsidered as an unresolved scope question, not evidence that production behavior changed.")
        add(18, "ambiguous", {}, scope="reconsidered interpretation", tags=["relationship_only", "cross_source_interpretation_revised"], interpretation=revised, step="Use the retained source qualifiers when reassessing the scope question; the interpretation alone cannot replace accepted policy.")
        current = policy(rule.format(limit=second))
        current["text"] += "\n\n"
        add(19, "benign", {"docs/policy.md": current}, tags=["formatting", "trailing_newlines"])
        add(20, "consequential", {"docs/safety.md": None}, scope="accepted source support withdrawn", tags=["accepted_support_withdrawal", "deleted_source"], step="Retain the accepted constraint's historical evidence and revalidate source-current support; deleting its document does not authorize a policy reversal.")
        projects.append({"id": project, "source_identity": {"kind": "synthetic_debug_fixture", "version": "guardian-events-v1"},
                         "initial_revision": f"{project}-00", "initial_files": initial, "events": events})
    return {"schema_version": 1, "protocol": "guardian-longitudinal-v1", "fixture_only": True,
            "held_out": False, "label_provenance": {"status": "synthetic_debug", "independent_review": False,
                "description": "Source assertions and expected labels were authored together for regression debugging. No human reviewers or real-project outcomes are claimed."},
            "projects": projects}


if __name__ == "__main__":
    destination = Path(__file__).with_name("events.json")
    destination.write_text(json.dumps(build(), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
