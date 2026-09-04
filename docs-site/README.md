# Analyst program documentation

This is the local static documentation reader. It consumes the existing Rust-backed
Python package and output-free notebooks; it contains no financial calculations in
the browser. Hosting and the unified API-reference rollout remain separate work.

From the repository root:

```sh
npm --prefix docs-site ci
mise run docs-site-build
```

`docs-site-build` is the release entrypoint. It runs `python-sync` and then
`python-build` before executing any course code, so every result is verified
against an extension built from the current checkout. The development profile
keeps this incremental rebuild fast; use a release build only for performance work.

The publication build executes every displayed Python block, replays each exercise
solution from a fresh lesson baseline, executes copies of the notebook collection,
checks curriculum and output freshness, exports the site, and checks local links.
Displayed snippets and copied notebooks are strict: exceptions, error outputs,
Python warnings and non-empty `stderr` streams fail the gate. There is no
runtime-warning allowlist. A library result field or table column named `warnings`
is domain data, not a Python runtime warning; a lesson may display such a record
only when it checks and interprets the expected analytical condition.
The `coverage/` inventories map every original exercise group to its visible
solution and tagged source cells. The publication gate checks their completeness
and records stage runtimes and exit codes in `.build/release.json`; `.build/labs.json`
is the source-fingerprinted notebook execution record. Canonical and learner runs
can open only the lesson's recursively declared notebook closure. Per-lesson
execution records include fresh-baseline exercise time. Numerical failures and
notebook diagnostics stop the build. Notebook kernels need permission to bind local
IPC sockets. Outputs stay under ignored `.build/`, `public/lab-assets/` and
`public/snippet-assets/`; source notebooks remain output-free.

For focused iteration:

```sh
uv run --no-sync python docs-site/gen/run_lesson_snippets.py --lesson 1.1 --require-all
uv run --no-sync python docs-site/gen/build_labs.py --notebook 01_foundations/core_types_and_money.ipynb
mise run docs-site-curriculum
mise run docs-site-test
mise run docs-site-dev
```

`uv run --no-sync python docs-site/gen/verify_prerequisites.py` validates the 33 workflow
contracts against the installed extension and freshly executed notebook copies.
Its `--source-only` mode checks paths and imports without claiming execution.
Evidence fingerprints the interpreter, extension binary, shared factories, data
files, external example inputs, mapped notebooks and declared cross-notebook
dependencies. Each output and generated asset is hashed; a complete successful
build prunes files from deleted lessons or notebooks. Per-result
policy and runner hashes prevent an older permissive run from being relabelled as
strict; executed-copy hashes prevent stale build notebooks from being accepted.
Source notebooks containing saved outputs or execution counts fail the build;
clearing historical outputs is an authoring step, never a side effect of the
execution runner. Expected domain warning records do not waive the
zero-warning/zero-`stderr` execution contract and must not be added to an allowlist
to hide a runtime diagnostic.

The development server uses existing evidence and is not a release gate. Run
`npm --prefix docs-site run typecheck` for the reader's TypeScript checks. `out/`
is the static artifact. No cloud deployment is performed by these commands.

`curriculum.toml` owns lesson order, prerequisites, fixture references, fixture
source files, introduction lessons, labs and examples. A fixture must be introduced
exactly once and the dependency graph must place that owner before every consumer.
Every Python module under `_shared/` has a fixture owner, and imports are checked
against the lesson's full prerequisite closure, including recursively declared labs.
Each owner lesson contains a visible `FixtureBuild` card before its first executable
block. IDs remain strings: `1.1` through the numbered parts, `C1`–`C5`,
`V1`–`V3`, and `capstone`. Capstone variants have separate track prerequisites.
Lesson frontmatter owns presentation and `status: draft | published`.

Executable fences use `python exec id=<stable-id>`. Exercise solutions add
`role=exercise`. Learner-facing fences contain no raw Python assertions. The
canonical tagged cells retain validation-only checks for the release runner: the
combined build sequence includes at least one, every exercise includes its own,
and every individual check must be reached and succeed before the assertion-free
publication source is accepted. A skipped check or caught `AssertionError` does
not count as validation. Imports,
units, assumptions and tolerances belong in the displayed code and nearby prose.
Use `<ExecutedOutput lesson="1.1" block="money-proof" />` to display captured
results. Print tables explicitly; figures created with Matplotlib are captured.
Rust and JavaScript tabs are display-only without a separate verification path.

The notebook cell metadata `analyst_program` identifies reusable Python cells:

```json
{"analyst_program": {"lesson": "2.6", "id": "mc-parallel-replay", "role": "build"}}
```

Insert a source region in the matching lesson, using a notebook listed in that
lesson's `labs` or `examples` mapping:

```md
<!-- notebook-block notebook="07_advanced_quant/monte_carlo/black_scholes_benchmarks.ipynb" cell="mc-parallel-replay" role="build" -->
<!-- /notebook-block -->
```

`uv run --no-sync python docs-site/gen/materialize_notebook_blocks.py --lesson 2.6` replaces
only marked regions with the tagged cell's assertion-free publication source inside
a displayed `python exec` fence. The runner resolves the same stable ID back to the
canonical source and executes all validation checks. The materializer normalizes marker boundaries to MDX-safe comments
(`{/* notebook-block ... */}` and `{/* /notebook-block */}`), and accepts both marker
forms on later runs. The stable metadata ID, not the notebook's internal cell ID,
becomes the executable block ID. Other cell text and indentation are preserved; a
final line break is supplied when a cell lacks one. Surrounding prose is unchanged.

Snippet execution and curriculum freshness validation materialize regions
automatically. `--check` detects stale regions without writing. A changed source
cell changes the lesson hash and invalidates old captured output. Missing or
duplicate tags, mismatched lesson/role metadata, unmapped notebooks, escaping
paths, malformed regions, invalid Python, visible assertions, or missing canonical
validation checks stop validation.

Dependencies remain visible: include imports and setup in earlier build blocks;
the tool does not execute omitted notebook cells or hide them in a wrapper. A
reused exercise cell must itself be tagged `role: exercise`. Its complete solution
belongs inside `<Exercise>`, followed by its `<ExecutedOutput>` component, and runs
from the complete build baseline in a fresh process. Do not introduce another
financial implementation in a generator or component.

All mandatory concepts, labs, exercises and solutions remain part of each unit.
Longer lessons span multiple sessions. Learner completion time is measured during
delivery; it is not inferred from notebook execution time. Release gates and
retrospectives determine scheduling, not a fixed 12–14-week ceiling.
