#!/usr/bin/env python3
"""External executable checks; this file and its probes must stay outside agent input."""
from __future__ import annotations
import argparse
import importlib.util
import io
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent


def evaluate(case_id: str, workspace: Path) -> dict:
    manifest = json.loads((ROOT / "corpora/coding-real-v1/manifest.json").read_text())
    task = next(item for item in manifest["tasks"] if item["id"] == case_id)
    path = workspace / task["file"]
    spec = importlib.util.spec_from_file_location("candidate_module", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules["candidate_module"] = module
    try:
        spec.loader.exec_module(module)
        imported = True
    except Exception:
        imported = False
    results = []
    for probe in task["probes"]:
        passed = False
        if imported:
            # Expressions are trusted, pinned checker code, never agent output.
            # Exceptions/tracebacks and expected values are not sent to agents.
            try:
                value = eval(probe["expression"], {"m": module, "io": io, "__builtins__": __builtins__})
                passed = "raises" not in probe and value is True
            except Exception as error:
                passed = type(error).__name__ == probe.get("raises")
        results.append({"id": probe["id"], "kind": probe["kind"], "passed": passed})
    return {"schema_version": 1, "checks": results}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", required=True)
    parser.add_argument("--workspace", type=Path, required=True)
    args = parser.parse_args(argv)
    print(json.dumps(evaluate(args.case, args.workspace)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
