# Lore foundations

The original foundation library has grown into the end-to-end compiler. See [IMPLEMENTATION.md](IMPLEMENTATION.md) for the implemented CLI, model clients, incremental engine, storage, privacy, recovery behavior, and current limitations. [DESIGN.md](../DESIGN.md) describes the broader architecture and intended evolution.

The central boundaries remain the same: `GenerativeModel` and `DecisionModel` define provider-independent inference contracts; Rust validates and applies semantic proposals; SQLite preserves source observations and exact evidence; and the Markdown wiki is a rebuildable projection. Model-generated prose is not the authoritative store of project history, and a valid citation is not independent proof of implementation.

Run `cargo test --all-targets --locked` to exercise the offline contracts and end-to-end regression scenarios. The HTTP tests use local fixtures and never require real provider credentials. Live environment checks are available through `lore doctor --inference`.
