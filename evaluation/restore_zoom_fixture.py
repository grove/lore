#!/usr/bin/env python3
"""Restore the public synthetic registry used for the matched 0.7 comparison."""
from __future__ import annotations

import argparse
import base64
import gzip
import hashlib
import io
import json
from pathlib import Path
import sys

import cross_source as cross

CORPUS = Path(__file__).resolve().parent / "corpora" / "zoom-frozen-v1"
MAX_DATABASE_BYTES = 16 * 1024 * 1024


def restore(output: Path, corpus: Path = CORPUS) -> dict:
    output = cross.selected_path(output)
    if output.exists():
        raise ValueError("Use a fresh fixture output directory")
    manifest = cross.read_json(corpus / "manifest.json")
    if manifest.get("schema_version") != 1 or manifest.get("classification") != "public_synthetic_retained_registry":
        raise ValueError("Unknown retained fixture format")
    expected_size = manifest["registry_bytes"]
    if type(expected_size) is not int or not 0 < expected_size <= MAX_DATABASE_BYTES:
        raise ValueError("Retained fixture exceeds the database limit")
    files = {}
    for name, expected in manifest["source_files_sha256"].items():
        path = Path(name)
        if path.is_absolute() or ".." in path.parts or not path.parts:
            raise ValueError("Invalid fixture file path")
        source = corpus / path
        cross.reject_symlink_path(source)
        if source.stat().st_size > MAX_DATABASE_BYTES:
            raise ValueError("Oversized fixture input")
        value = source.read_bytes()
        if hashlib.sha256(value).hexdigest() != expected:
            raise ValueError("Retained fixture source digest changed")
        files[name] = value
    encoded = files["registry.sqlite.gz.b64"]
    compressed = base64.b64decode(b"".join(encoded.split()), validate=True)
    with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as stream:
        database = stream.read(expected_size + 1)
    if len(database) != expected_size or hashlib.sha256(database).hexdigest() != manifest["registry_sha256"]:
        raise ValueError("Decoded retained registry does not match its bound bytes")
    if not database.startswith(b"SQLite format 3\x00"):
        raise ValueError("Retained registry is not a SQLite database")
    output.mkdir(parents=True)
    for name, value in files.items():
        if name.startswith("docs/") or name == "cases.json":
            target = output / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(value)
    (output / ".lore").mkdir()
    (output / ".lore/state.db").write_bytes(database)
    cross.write_json(output / "lore.yml", manifest["configuration"])
    result = {"schema_version": 1, "phase": "restored_public_synthetic_registry",
              "baseline_commit": manifest["baseline_commit"], "registry_sha256": manifest["registry_sha256"],
              "registry_bytes": len(database), "provider_calls": 0,
              "config": str(output / "lore.yml"), "cases": str(output / "cases.json")}
    cross.write_json(output / "restored.json", result)
    return result


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        print(json.dumps(restore(args.output), indent=2))
        return 0
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(f"Fixture restoration rejected: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
