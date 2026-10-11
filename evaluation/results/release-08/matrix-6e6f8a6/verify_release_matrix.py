#!/usr/bin/env python3
"""Count completed CI logs locally and emit only structured, bounded facts."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--logs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    metadata_bytes = args.metadata.read_bytes()
    metadata = json.loads(metadata_bytes)
    workflow = metadata["workflow"]
    checkout = metadata["checkout_commit"]
    expected_targets = metadata["expected_rust_targets"]
    assert expected_targets and len(expected_targets) == len(set(expected_targets))
    assert workflow["status"] == "completed"
    assert checkout["tree_sha"] == workflow["head_tree_sha"]
    rows = {}
    for job in metadata["jobs"]:
        assert job["status"] == "completed"
        platform = {"test (ubuntu-latest)": "linux", "test (macos-latest)": "macos", "test (windows-latest)": "windows"}[job["name"]]
        data = (args.logs / (str(job["id"]) + ".log")).read_bytes()
        original = data.decode("utf-8")
        text = re.sub(r"\x1b\[[0-9;]*m", "", original)
        lines = [re.sub(r"^\ufeff?\S+Z ", "", line) for line in text.splitlines()]
        def unique_version(pattern: str, selected_lines=None):
            values = sorted({match.group(1) for line in (lines if selected_lines is None else selected_lines)
                             if (match := re.fullmatch(pattern, line))})
            assert len(values) <= 1
            return values[0] if values else None
        runner_group = lines.index("##[group]Runner Image") if "##[group]Runner Image" in lines else -1
        runner_lines = lines[runner_group:lines.index("##[endgroup]", runner_group)] if runner_group >= 0 else []
        version_suffix = r"([0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)? \([0-9a-f]+ [0-9]{4}-[0-9]{2}-[0-9]{2}\))"
        python_versions = {match.group(1) for line in lines if (match := re.fullmatch(r"Python ([0-9]+\.[0-9]+\.[0-9]+)", line))}
        python_versions.update(match.group(1) for line in lines if (match := re.match(r'\s*"python_version": "([0-9]+\.[0-9]+\.[0-9]+)(?: |")', line)))
        assert len(python_versions) <= 1
        environment = {"image": unique_version(r"Image: ([A-Za-z0-9_.-]+)", runner_lines),
                       "image_version": unique_version(r"Version: ([0-9.]+)", runner_lines),
                       "runner_version": unique_version(r"Current runner version: '([0-9.]+)'"),
                       "rustc_version": unique_version("rustc " + version_suffix),
                       "cargo_version": unique_version("cargo " + version_suffix),
                       "python_version": next(iter(python_versions)) if python_versions else None}
        environment["exact_toolchain_versions_observed"] = all(environment[key] is not None for key in ("rustc_version", "cargo_version", "python_version"))
        checkout_index = next(i for i, line in enumerate(lines) if " log -1 --format=%H" in line)
        observed_checkout = lines[checkout_index + 1]
        assert observed_checkout == checkout["sha"]
        step = {item["name"]: item for item in job["steps"]}
        rust_marker = "##[group]Run cargo test --all-targets --locked"
        rust_start = text.find(rust_marker)
        next_starts = [index for marker in ("##[group]Run cargo build --release --locked", "##[group]Run python -m unittest")
                       if (index := text.find(marker, max(0, rust_start))) >= 0]
        rust_end = min(next_starts) if next_starts else len(text)
        rust_text = text[rust_start:rust_end] if rust_start >= 0 else ""
        summary_regex = r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out"
        def count(block: str) -> dict:
            summaries = re.findall(summary_regex, block)
            return {"suite_summaries": len(summaries),
                    **{name: sum(int(row[i + 1]) for row in summaries)
                       for i, name in enumerate(("passed", "failed", "ignored", "measured", "filtered_out"))}}
        rust = count(rust_text)
        targets = [path.replace("\\", "/") for path in re.findall(r"\bRunning (?:unittests )?([^\s]+\.rs) \(", rust_text)]
        declared_tests = [int(number) for number in re.findall(r"\brunning (\d+) tests?\b", rust_text)]
        rust["expected_targets"] = len(expected_targets)
        rust["parsed_targets"] = len(targets)
        rust["declared_test_count"] = sum(declared_tests)
        rust["parse_complete"] = (rust_start >= 0 and rust["suite_summaries"] > 0
                                  and len(targets) == len(set(targets)) == len(expected_targets)
                                  and set(targets) == set(expected_targets)
                                  and rust["suite_summaries"] == len(targets) == len(declared_tests)
                                  and sum(declared_tests) == sum(rust[key] for key in ("passed", "failed", "ignored", "measured"))
                                  and rust["filtered_out"] == 0)
        rust["command_conclusion"] = step["Build and run offline Rust tests"]["conclusion"]
        rust["all_targets_gate_passed"] = (rust["command_conclusion"] == "success" and rust["parse_complete"] and rust["failed"] == 0)
        rust["observed_failed_test_lines"] = len(re.findall(r"\btest ([A-Za-z0-9_:]+) \.\.\. FAILED", rust_text))
        python_match = re.search(r"Ran (\d+) tests in ([0-9.]+)s", text)
        python_status = step["Benchmark and assessment tests (no inference)"]["conclusion"]
        python = {"command_conclusion": python_status, "discovered": None, "passed": None, "skipped": None, "elapsed_seconds": None,
                  "reported_failure_outcomes": None, "reported_error_outcomes": None}
        if python_match:
            python["discovered"] = int(python_match.group(1))
            python["elapsed_seconds"] = float(python_match.group(2))
            success = [line for line in lines if re.fullmatch(r"OK(?: \(skipped=\d+\))?", line)]
            if python_status == "success":
                assert len(success) == 1
                skip = re.search(r"skipped=(\d+)", success[0])
                python["skipped"] = int(skip.group(1)) if skip else 0
                python["passed"] = python["discovered"] - python["skipped"]
                python["reported_failure_outcomes"] = 0
                python["reported_error_outcomes"] = 0
            else:
                failed_summaries = [line for line in lines if re.fullmatch(r"FAILED \([a-z =0-9,]+\)", line)]
                if len(failed_summaries) == 1:
                    reported = dict(re.findall(r"([a-z ]+)=(\d+)", failed_summaries[0][8:-1]))
                    reported = {key.strip(): int(value) for key, value in reported.items()}
                    python["reported_failure_outcomes"] = reported.get("failures", 0)
                    python["reported_error_outcomes"] = reported.get("errors", 0)
                    python["skipped"] = reported.get("skipped", 0)
                    python["passed_count_basis"] = "Unknown: failed unittest summaries can count multiple subtest outcomes within one discovered testcase."
        elif python_status == "success":
            raise AssertionError("Successful Python step lacks its test summary")
        release = {"command": ["cargo", "build", "--release", "--locked"],
                   "conclusion": step.get("Build release CLI", {}).get("conclusion"),
                   "release_command_observed": "##[group]Run cargo build --release --locked" in text}
        observed_versions = [line for line in lines if re.fullmatch(r"lore [0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?", line)]
        version = [line for line in observed_versions if line == "lore 0.8.0"]
        unexpected_versions = [line for line in observed_versions if line != "lore 0.8.0"]
        smoke_names = ("Decision and case CLI help", "Project companion CLI help", "CLI help", "Review CLI help",
                       "Context CLI help", "Onboarding CLI help", "Exploration CLI help", "CLI version")
        smoke = {name: step[name]["conclusion"] for name in smoke_names}
        expected_invocations = [["decisions", "--help"], ["cases", "--help"], ["baseline", "--help"],
                                ["changes", "--help"], ["guard", "--help"], ["--help"], ["review", "--help"],
                                ["context", "--help"], ["onboard", "--help"], ["explore", "--help"], ["--version"]]
        release_invocations = []
        unexpected_invocations = []
        for line in lines:
            match = re.search(r"Running `target[/\\]release[/\\]lore(?:\.exe)? ([^`]+)`", line)
            if match:
                arguments = match.group(1).split()
                if arguments in expected_invocations:
                    release_invocations.append(arguments)
                else:
                    # Do not copy arbitrary arguments from even a failed job
                    # into an otherwise public, bounded-facts receipt.
                    unexpected_invocations.append(sha(match.group(1).encode()))
        if job["conclusion"] == "success":
            assert rust["parse_complete"] and rust["failed"] == 0 and rust["all_targets_gate_passed"]
            assert python_status == "success" and python["discovered"] > 0 and python["passed"] >= 0
            assert release["conclusion"] == "success" and release["release_command_observed"]
            assert all(value == "success" for value in smoke.values())
            assert version == ["lore 0.8.0"] and not unexpected_versions
            assert release_invocations == expected_invocations and not unexpected_invocations, platform
        frozen_marker = "##[group]Run cargo test --locked --test knowledge_zoom_comparison measured_existing_compiled_project -- --ignored --exact"
        frozen_start = text.find(frozen_marker)
        frozen = count(text[frozen_start:] if frozen_start >= 0 else "")
        frozen["command_conclusion"] = step["Compare the exact frozen Zoom registry (no inference)"]["conclusion"]
        if job["conclusion"] == "success":
            assert step["Check formatting"]["conclusion"] == step["Strict Rust lint"]["conclusion"] == "success"
            if platform == "linux":
                assert frozen["command_conclusion"] == "success" and frozen_start >= 0
                assert frozen["suite_summaries"] == frozen["passed"] == 1
                assert frozen["failed"] == frozen["ignored"] == frozen["measured"] == 0
            else:
                assert frozen["command_conclusion"] == "skipped" and frozen["suite_summaries"] == 0
        rows[platform] = {"job_id": job["id"], "conclusion": job["conclusion"], "tested_checkout_sha": observed_checkout,
                          "runner_environment": environment,
                          "rust_all_targets": rust, "python": python, "separate_frozen_zoom": frozen,
                          "release_build": release, "release_cli_smoke_steps": smoke,
                          "release_cli_invocations": release_invocations, "version_stdout": version,
                          "unexpected_cli_invocation_count": len(unexpected_invocations),
                          "unexpected_cli_invocation_sha256": unexpected_invocations,
                          "unexpected_version_count": len(unexpected_versions),
                          "format_conclusion": step["Check formatting"]["conclusion"],
                          "clippy_conclusion": step["Strict Rust lint"]["conclusion"],
                          "local_raw_log_binding": {"bytes": len(data), "sha256": sha(data), "included_in_published_receipt": False}}
    assert set(rows) == {"linux", "macos", "windows"}
    all_passed = all(row["conclusion"] == "success" for row in rows.values())
    assert (workflow["conclusion"] == "success") == all_passed
    result = {"schema_version": 1, "status": "verified_success" if all_passed else "verified_failure",
              "kind": "condensed_final_candidate_platform_verification", "repository": "grove/lore", "pull_request": 47,
              "workflow_run": workflow, "checkout_commit": checkout, "matrix": rows,
              "metadata_sha256": sha(metadata_bytes), "verifier_sha256": sha(Path(__file__).read_bytes()),
              "raw_ci_logs_included": False,
              "limits": ["Counts are recomputed from completed public CI jobs and bound to their exact observed checkout commit.",
                         "The GitHub merge-ref tree equals the stated PR-head tree.",
                         "Raw log content remains in a separate local-only directory and is not part of this receipt.",
                         "Ignored or skipped tests remain visible and are not counted as passes; failed commands may stop before every target runs.",
                         "No live inference, source modification or CI rerun was initiated by this verifier.",
                         "This receipt covers only its stated head; later changes require their own checks."]}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    payload = (json.dumps(result, indent=2, sort_keys=True) + "\n").encode()
    with tempfile.NamedTemporaryFile(dir=args.output.parent, delete=False) as stream:
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())
        temporary = stream.name
    os.replace(temporary, args.output)
    print(json.dumps({"path": str(args.output), "bytes": len(payload), "sha256": sha(payload), "status": result["status"],
                      "counts": {platform: {"rust": row["rust_all_targets"], "python": row["python"],
                                            "version": row["version_stdout"]} for platform, row in rows.items()}}, indent=2))


if __name__ == "__main__":
    main()
