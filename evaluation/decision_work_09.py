#!/usr/bin/env python3
"""Prepare public decision fixtures or execute their positive/negative controls.

Uses the existing six-arm study and external checker protocol. No inference.
Control execution runs only the bundled, authored development implementations.
"""
import argparse
import json
from pathlib import Path
import tempfile

import adaptive_tasks as adaptive
import coding_tasks as coding
import cross_source as cross

CORPUS = Path(__file__).resolve().parent / "corpora/decision-work-09"
CASES = CORPUS / "cases.json"


def validate(output):
    output = cross.selected_path(output)
    if output.exists():
        raise ValueError("Use a fresh output directory")
    controls = cross.read_json(CORPUS / "controls.json")
    rows = []
    for case in coding.load_cases(CASES)["cases"]:
        for variant in ("correct", "wrong", "unimplemented"):
            with tempfile.TemporaryDirectory(prefix="lore-decision-control-") as temp:
                workspace = Path(temp).resolve() / "source"
                cross.copy_snapshot(CORPUS / case["source_root"], workspace)
                if variant != "unimplemented":
                    (workspace / "src/policy.py").write_text(controls[case["id"]][variant], encoding="utf-8")
                before = cross.fingerprint(workspace)
                result, elapsed = coding.execute_checks(case, workspace, CORPUS, 30)
                preserved = cross.fingerprint(workspace) == before
                passed = all(item["passed"] for item in result["checks"])
                rows.append({"case": case["id"], "variant": variant, "checks": result,
                             "source_sha256": before["sha256"], "elapsed_seconds": elapsed,
                             "source_preserved": preserved,
                             "control_passed": preserved and passed == (variant == "correct")})
    report = {"schema_version": 1, "classification": "public_development_controls",
              "status": "passed" if all(row["control_passed"] for row in rows) else "failed",
              "cases_sha256": coding.hash_file(CASES), "arms": list(adaptive.SETUPS),
              "provider_calls": 0, "productivity_benefit_established": False, "controls": rows}
    output.mkdir(parents=True)
    cross.write_json(output / "controls.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "validate"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = adaptive.prepare(args.output, CASES) if args.command == "prepare" else validate(args.output)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 2 if report.get("status") == "failed" else 0


if __name__ == "__main__":
    raise SystemExit(main())
