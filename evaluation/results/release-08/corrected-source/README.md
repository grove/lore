# Human fixture correction: source and evidence binding

The bounded review found **no unresolved material runtime or functionality gap** in the PR4/PR7 and release-documentation scope. Final native platform and Guardian gates remain required. This handoff records source identity and earlier observed results; it does not claim that a corrected-head CI run has passed.

## Exact revisions

| Role | Commit or tree |
| --- | --- |
| Previously built/captured public candidate | `a55cfeead56775af4e0ae9d171cbdcf191af2007` |
| Candidate tree | `a0ebeecb3970a14abd10a4bcf15ea96d4bbc2049` |
| Isolated reviewed fixture correction | `165e97dfc9d8a4e79f3892d670b5a91564c1a320` |
| Corrected published release head | `6e6f8a6af871bdeca53dcc79b63a60fd2af52743` |
| Corrected published tree | `1e77e84d9f7e69b89968c6b60a6beef7f9afec82` |
| Local commit with the same corrected tree | `2680a3c69c9c5d2b2d3c38942aa2d0d96572ce54` |

`corrected-head-binding.json` verifies every selected path, Git mode, blob identity and SHA-256. All **70 runtime inputs** (Rust source, Cargo inputs and embedded migrations) and **145 Guardian programs/corpus inputs** are identical between the observed candidate and corrected public head. It inventories all 53 changed paths. The only changed test file is the exact reviewed human fixture; the other changes are release evidence/documentation and two source/toolchain-reporting workflow steps. Removing each exact six-line diagnostic block restores its prior workflow bytes. Existing runtime or Guardian observations keep their original source and environment pins.

## Fixture correction and validation

The fixture used host-native separators for a nested reviewer capture reference. Windows produced backslashes, and the production validator correctly rejected the noncanonical evidence path. The correction uses `Path.as_posix()` and adds an explicit malformed-backslash rejection assertion under both assisted and unassisted transfer cases. It changes no product guard, scoring rule, consent or withdrawal requirement, source/checker binding, execution permission or test skip.

The authored patch is 11 added lines and one removed line. Its corrected test-file SHA-256 is `4a999f0cabfa5ab73b1a605cda0dc4878288f9f30393f5d82f29430a3e6242a5`, identical at the corrected public head. The actual local targeted test passed in 3.674 seconds; all 20 human-package tests passed in 11.348 seconds with no failures or skips. The exact diff received a separate implementation-agent review. These are engineering checks, not participant outcomes or a corrected native Windows CI result. No unrelated tests were rerun for this audit.

## Retained failures and successful evidence

`windows-ci-failures.json` is the unchanged v2 condensed receipt (SHA-256 `e3784af33b5b2a2fc1e1883a34d15852573e2624262f7cab01ef429873ebadbd`). Both previous Windows jobs passed their Rust suites and failed the same two assisted/unassisted Python subtests. Their Python passed counts remain unknown because subtest failures are not separate discovered test cases. PR47 also completed an explicit release build; PR46 had no separate release-build step and skipped its later release smoke after Python failed. The v2 receipt corrects only the earlier derived overstatement about PR46 release-build evidence. Original failure identities, counts and raw-log hashes are preserved.

The actual a55 release binary has SHA-256 `f9ee5b81b14a16df4a7c5fba6ced41d8a1111ec3701db38bbc433d69fdebeff8`. Its retained release build, 11 help/version smoke commands and five contract captures remain attributed to that source. The Guardian run [38092630241](https://github.com/grove/lore/actions/runs/38092630241) used a different, explicitly recorded debug candidate binary: `59f20591127dbd04ac350ae0bd7e9651be0f33daf7a24d3dbcd9bfd05c6c83f8`. Its verified archive covers 190 replay events, 1,891 source-preserving, source-observer-complete command intervals and 240 no-Lore control intervals. Logical model calls are zero; independently observed physical provider attempts remain unknown without sidecar ledgers. The historical PR41 capture has 1,889 command intervals and remains separate.

## Documentation and release decision

The corrected-head audit verified the committed artifact inventory's hash, all 68 selected artifact byte lengths/hashes and every gate's artifact hash. The guide, README, tracker, scorecard and ledger retain the distinction between passed component evidence, the failed a55 platform attempt and pending corrected-head release gates. Actual human participants and genuine coding attempts remain zero; empirical productivity, contribution, transfer and independent Guardian alert quality remain unmeasured as the implementation plan requires when real prerequisites are absent.

Public head 6e6 still includes the original human-failure receipt. The parent has accepted the v2 correction and current working scorecard/index text explicitly supersedes it. Final integration must retain the original as superseded, add v2 and update the current references and artifact/ledger hashes. The published-source audit records this precise boundary rather than treating uncommitted documentation as published content.

## Files and reproduction

- `fix-change-manifest.json` and `fix-validation.json`: isolated authored change and actual local checks.
- `runtime-inputs.json`, `guardian-inputs.json` and `corrected-head-binding.json`: full input hashes, exact corrected public source and bounded changed-path inventory.
- `guardian-evidence-binding.json`: actual a55 Guardian source/run/binary/receipt bindings and limits.
- `windows-ci-failures.json`: corrected v2 historical failure receipt; no raw log content.
- `documentation-audit.json`: explicitly historical audit at the earlier integration snapshot.
- `documentation-audit-corrected.json`: current published-source findings, artifact verification and accepted v2 integration action.
- `workflow-snapshot.json`: historical observation of the separate workflow metadata changes.
- `artifact-manifest.json`: SHA-256 and size of every selected handoff file, excluding itself.

The runtime equality check is reproducible from the published commits:

```sh
git diff --exit-code a55cfeead56775af4e0ae9d171cbdcf191af2007 \
  6e6f8a6af871bdeca53dcc79b63a60fd2af52743 \
  -- src migrations Cargo.toml Cargo.lock build.rs .cargo

git diff a55cfeead56775af4e0ae9d171cbdcf191af2007 \
  6e6f8a6af871bdeca53dcc79b63a60fd2af52743 \
  -- evaluation/tests/test_human_onboarding.py
```

No root repository files were edited by this audit. No raw CI logs, credentials, provider response bodies or participant records are included. Hash equality establishes byte identity; it does not substitute for final required CI, independent outcome assessment or provider billing evidence.
