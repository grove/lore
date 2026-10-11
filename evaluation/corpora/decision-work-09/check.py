#!/usr/bin/env python3
"""External development checker. Execute only in an authorized test environment.

Neither this program nor controls.json belongs in the coding agent's source tree.
This process executes candidate code; it is not an OS sandbox.
"""
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True


def check(name, workspace):
    control = json.loads(Path(__file__).with_name("controls.json").read_text())[name]
    namespace = {}
    try:
        exec(compile((workspace / "src/policy.py").read_bytes(), "policy.py", "exec"), namespace)
        function = namespace[control["correct"].split("(", 1)[0].removeprefix("def ")]
    except Exception:
        function = None
    checks = []
    for i, (arguments, expected) in enumerate(control["probes"]):
        try:
            actual = function(*arguments)
            passed = type(actual) is type(expected) and actual == expected
        except Exception as error:
            passed = isinstance(expected, dict) and expected.get("raises") == type(error).__name__
        checks.append({"id": f"probe-{i}", "kind": "correctness" if i == 0 else "constraint",
                       "passed": passed})
    return {"schema_version": 1, "checks": checks}


if __name__ == "__main__":
    print(json.dumps(check(sys.argv[1], Path(sys.argv[2]))))
