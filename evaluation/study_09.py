#!/usr/bin/env python3
"""Read-only empirical-study inspection; all protocol gates belong to experiment_08."""
import argparse
import json
from pathlib import Path

import adaptive_tasks as adaptive
import experiment_08 as experiment


def status(bundle: Path) -> dict:
    checked = experiment.preflight(bundle, fresh=False)
    # Do not publish private paths or validation exception excerpts.
    blockers = [{"gate": item["gate"], "passed": False} for item in checked["blockers"]]
    return {"schema_version": 1, "protocol": checked["protocol"],
            "status": "ready_for_external_execution" if checked["preflight_passed"] else "not_run",
            "pilot": "ready_external" if checked["preflight_passed"] else "blocked_external",
            "tasks": checked["tasks"], "arms": list(adaptive.SETUPS),
            "initial_assignments": checked["initial_assignments"],
            "preflight_passed": checked["preflight_passed"], "blockers": blockers,
            "provider_calls": 0, "subprocess_calls": 0,
            "coding_productivity": "unmeasured", "productivity_benefit_established": False}


def markdown(value: dict) -> str:
    lines = ["# Lore 0.9 empirical study", "", f"Status: {value['status']}. Productivity: unmeasured.",
             f"Planned assignments: {value['initial_assignments']} across six mandatory arms.", "",
             "Unmet protocol gates:"]
    lines += ["- " + item["gate"] for item in value["blockers"]] or ["- None; readiness is not an outcome."]
    return "\n".join(lines) + "\n"


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("status", "doctor"))
    parser.add_argument("bundle", type=Path)
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    args = parser.parse_args(argv)
    try:
        value = status(args.bundle)
        print(markdown(value) if args.format == "markdown" else json.dumps(value, indent=2, sort_keys=True))
        return 0 if value["preflight_passed"] else 2
    except (ValueError, OSError, KeyError, TypeError):
        print(json.dumps({"status": "invalid", "coding_productivity": "unmeasured"}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
