# Frozen public synthetic retrieval registry

This fixture preserves the exact retained registry used to compare the pinned
`6f4898bf1d6e0dc135a24ef792921c3d98af2c24` baseline with the 0.7 candidate.
It contains twelve explicitly authored fictional documents and deterministic
fixture-model outputs. It contains no real provider response, confidential
project material or human participant data.

The database is gzip-compressed and base64-encoded for portable review and Git
transport. `manifest.json` binds its decoded size and SHA-256, all original
source bytes, and independently authored cases. The restoration command checks
those identities before creating a fresh output and bounds decompression.
The restored provider is disabled; no inference or repository execution occurs.

```sh
python evaluation/restore_zoom_fixture.py --output /tmp/lore-07-frozen
LORE_ZOOM_COMPARISON_CONFIG=/tmp/lore-07-frozen/lore.yml \
LORE_ZOOM_COMPARISON_CASES=/tmp/lore-07-frozen/cases.json \
LORE_ZOOM_COMPARISON_OUTPUT=/tmp/lore-07-frozen-comparison.json \
  cargo test --locked --test knowledge_zoom_comparison \
  measured_existing_compiled_project -- --ignored --exact --nocapture
```

The disabled-provider restoration configuration differs from the original
ingestion configuration. Deterministic comparison reads the same SQLite bytes,
identifiers, source revisions, relationships and query cases. Do not run
`lore update` before comparing: that would create a new registry and stop being
the matched frozen experiment. Repeat timing still varies with the host, build
and filesystem caches; exact data identity is not a universal performance claim.

The older published 2026-10-09 report is a different fixture compilation and is
preserved separately. Fresh UUIDs can affect token boundaries near the budget.
The release report compares the new baseline and candidate on this captured
registry rather than mixing independently generated identifiers.
