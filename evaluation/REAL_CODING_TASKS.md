# Source-pinned coding-task candidates

Lore 0.7 includes 30 executable repair candidates, ten each from three real
public Python projects. Their source files and licenses are retained byte for
byte at exact upstream commits. A preparation command removes one function or
method body while preserving its public docstring, surrounding implementation,
README and source context. The task is to restore that documented behavior.

These are **published, constructed development candidates**. The same
implementation agent authored their task prompts and checker probes. Independent
task/checker authorship, gold review, a real-model pilot and held-out outcomes
are pending. Source pins and passing checker controls cannot establish those
properties. The runners recognize their input fingerprints even if a caller
changes fixture or held-out labels.

## Exact upstream inputs

The canonical provenance and per-file SHA-256 values are in
[`corpora/coding-real-v1/manifest.json`](corpora/coding-real-v1/manifest.json).
Each repository's source slice includes its original license. The upstream
links below identify the captured commit, not a moving branch.

| Project | Exact source commit | Selected implementation files |
| --- | --- | --- |
| [mahmoud/boltons](https://github.com/mahmoud/boltons/tree/4e5faa3d7e4008d89e0d8bf1ea87b6d9a061a16d) | `4e5faa3d7e4008d89e0d8bf1ea87b6d9a061a16d` | `boltons/strutils.py`, `boltons/iterutils.py` |
| [more-itertools/more-itertools](https://github.com/more-itertools/more-itertools/tree/b21ec4ee8e139ebe297e2221980a694bebb69523) | `b21ec4ee8e139ebe297e2221980a694bebb69523` | `more_itertools/recipes.py` |
| [theskumar/python-dotenv](https://github.com/theskumar/python-dotenv/tree/0b2880591780a426b0551436f47a17e7a76d954f) | `0b2880591780a426b0551436f47a17e7a76d954f` | `src/dotenv/parser.py`, `src/dotenv/variables.py` |

These source slices allow an offline mechanical study without installing the
projects. They are not full repository builds or a broad sample of programming
languages and application domains. Public upstream implementations are known
answers, so provider memorization is an additional reason to keep this corpus
outside the final held-out study.

| Project | Ten selected targets |
| --- | --- |
| boltons | `slugify`, `ordinalize`, `split_punct_ws`, `bytes2human`, `iter_splitlines`, `indent`, `chunked_iter`, `unique_iter`, `bucketize`, `get_path` |
| more-itertools | `grouper`, `roundrobin`, `unique_everseen`, `nth_combination`, `before_and_after`, `sliding_window`, `all_equal`, `partition`, `polynomial_eval`, `transpose` |
| python-dotenv | `parse_key`, `parse_unquoted_value`, `parse_value`, `parse_binding`, `decode_escapes`, `Position.advance`, `Reader.read`, `Reader.read_regex`, `Variable.resolve`, `parse_variables` |

The probes cover nominal behavior and material exceptions: ordering and
exhaustion, unsupported values and errors, key functions, line/position
tracking, quote/comment distinctions, missing versus empty environment values,
and incomplete inputs. They do not prove arbitrary implementation equivalence.
Checking the exact original source exposed and corrected draft expectations
about byte output from ASCII slugification, terminal empty split lines,
unhashable uniqueness inputs and sized polynomial coefficients. Those
corrections illustrate why this corpus still needs independent review.

## Prepare the six-task pilot or all 30 candidates

Preparation uses Python's standard library and performs no model inference,
network call, Lore command or candidate-code execution. Use a fresh directory.

```bash
python3 evaluation/real_coding_corpus.py prepare --pilot \
  --output evaluation-results/coding-real-pilot-inputs

python3 evaluation/real_coding_corpus.py prepare \
  --output evaluation-results/coding-real-full-inputs
```

The fixed pilot selects `boltons-ordinalize`, `boltons-chunked-iter`,
`more-itertools-grouper`, `more-itertools-unique-everseen`,
`python-dotenv-parse-binding` and `python-dotenv-variable-resolve`. It is six
debugging tasks across three source projects, not six completed model tasks.

Each preparation writes `cases.json`, `prepared.json` and separate task source
copies. Every case binds the original repository/commit and source file hashes,
the exact mutated input digest, the selected function, permitted editable file,
task text, constraints and external checker command. No answer-key manifest or
checker is added to the agent's source tree. The external checker reads the
pinned manifest only in the evaluator process.

To prepare the matched six-arm study and its incomplete preregistration:

```bash
python3 evaluation/adaptive_tasks.py prepare \
  --cases evaluation-results/coding-real-pilot-inputs/cases.json \
  --output evaluation-results/coding-real-pilot-study
```

See [ADAPTIVE_TASKS.md](ADAPTIVE_TASKS.md) for the actual local/hosted model
commands, operator grants, bounded repairs, retained attempts and outcome
assessment. Supply the generated cases file to `adaptive_tasks.py run --cases`.
Use an actual available pinned model and adapter; running a subprocess double
does not constitute model evidence. A completed full six-arm study would have
at least 180 initial attempts plus any allowed repair attempts. No such study
is claimed by these files.

## Validate the external checkers

The validation command intentionally executes candidate Python code in child
processes. Use the same deliberately isolated study environment used for other
coding checks. It executes the checker three times per task: once on the pinned
original source, once on a missing implementation, and once on an implementation
that always returns `None`.

```bash
python3 evaluation/real_coding_corpus.py validate \
  --output evaluation-results/coding-real-checker-controls
```

All 30 originals must pass and all 60 wrong controls must fail. `validation.json`
contains every individual correctness/constraint result, implementation file
hashes, checker-response digest, elapsed time, Python executable identity,
manifest/checker/harness hashes and the inference count of zero. A failing
control produces exit code 2. Preparation/input errors produce exit code 1.
Validation always covers all 30 tasks; `validate --pilot` is rejected.

The recorded development control run is
[`results/coding-controls-2026-10-10.json`](results/coding-controls-2026-10-10.json).
It demonstrates that these checkers accept the exact originals and reject the
two selected wrong bodies. It is neither independent checker authorship nor a
measurement of Lore's effect on coding success, cost or time.

## Preparing genuinely held-out outcomes

Have task authors and separate checker authors select new tasks before running
the final comparison. Record precise source commits, task/constraint/checker
contracts and independent review captures in the preregistration. Gold reviewers
must be distinct from the operator and the authors; each completed model
implementation also needs two blinded reviews with bound evidence captures.
Keep the final task cohort separate from failures used to tune Lore, and retain
the independently captured execution/egress audit. Public candidates may reveal
harness defects, but editing their labels does not satisfy these gates.

All six arms must receive the same source payload, coding model/settings,
editable paths, attempt allowance, wall budget and allowed tools. Report paired
task completion and critical violations together with every repair, preparation,
warm-up and checking cost. Unknown provider tokens or dollars remain null.
The study's proposed 10-percentage-point success gain or 20% time/cost reduction
requires measured paired outcomes and uncertainty. The corpus alone establishes
neither target.
