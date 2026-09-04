# Analyst Program delivery record

The program contains 33 required units and two independent capstone reference
submissions. All original exercise groups are tracked in `coverage/`; concepts,
solutions and executable source identities remain reviewable together. Longer
units use mandatory sessions. The original implementation plans are unchanged;
the canonical syllabus and site specification reflect the approved corrections.

## Prerequisite checkpoint

The initial P1 checkpoint passed on 3 September 2026 at 03:50 UTC against Python
3.12.13 and the built finstack-quant 0.8.0 extension. The checkpoint executed 98 copied notebooks,
verified 33 principal workflows, passed 63 focused tests and built 138 static
routes with resolving local links. Representative inspection covered the program
overview, equations, Monte Carlo plots and a rich reporting tear sheet.

The immutable fixture sequence is foundations, rates, options, base and common.
The common stage introduces the priced term loan in 4.3. Credit and volatility
factories extend it independently. The credit BBB sleeve retains its own holding,
deal and tranche identities and is repriced separately before combining subtotals.
Runtime, fixture and source fingerprints are retained in `.build/p1-exit.json`.

## Release verification

All 33 lessons are authored and marked published in the local reader, including
both independent capstone reference submissions. The source inventory contains
100 output-free notebooks, including the five new notebooks required by the
handoff. Coverage maps 303 displayed executable blocks: 215 BuildIt blocks and
88 numerical exercise solutions. One additional exercise group has a complete
written solution. Each numerical solution executes from a fresh lesson baseline.

The complete publication command is `mise run docs-site-build`. A fresh strict
run passed at 04:33:59 UTC on 4 September 2026. The release-profile native build
completed in 23 minutes 56 seconds; after installation, the recorded nine-stage
site gate passed in 1,155.30 seconds. It executed all 303 displayed blocks across
33 lessons in 484.46 seconds and all 100 copied notebooks in 639.44 seconds. Both
capstone references ran from independent fresh processes. The remaining
prerequisite, coverage, curriculum, production-build and rendered-link stages
all passed; the reader built 126 static routes.

A post-release assertion-filter hardening replay completed at 05:21 UTC. It
repeated all 303 lesson blocks and all 100 notebooks, rebuilt the 126 static
routes, and passed the curriculum, evidence, and rendered-link gates. The
learner-source filter now rejects both Python `assert` statements and explicit
`raise AssertionError` fallbacks.

The executed notebook inventory contains 861 code cells. Every non-empty code
cell has an execution count, with zero error outputs and zero non-empty `stderr`
streams. Python warnings are promoted to exceptions during authored cell and
snippet execution, so no runtime warning can produce passing evidence. All 100
source notebooks validate against the notebook schema, have complete unique cell
IDs, and retain zero outputs and zero execution counts. The canonical executions
reached and passed 828 private acceptance checks. All 86 learner-facing notebook
downloads contain zero assertion constructs; 14 repository-only notebooks
remain execution gated without public artifacts.

Evidence is bound to the Python interpreter, native extension, Python package
sources, fixtures, notebook sources and declared dependencies. Each lab entry
also records the strict policy, runner and builder hashes, and the executed-copy
hash. Standalone validators reject failed, partial, duplicate, stale or relabelled
lab reports. Snippet records similarly bind the policy, runner and worker hashes.
The nine-stage release record and timings are in `.build/release.json`.

The final aggregate suite passed 185 tests in 346.67 seconds with warnings
treated as errors. It covers fixture pricing and reconciliation, notebook schema
and kernel identity, warning and `stderr` failure propagation, snippets, source
materialization, evidence provenance, coverage and publication checks. Ruff and
the reader's TypeScript build passed.

The earlier visual review added the required Monte Carlo II diagnostics, improved
the one-page PM report and inspected the overview, desktop/mobile lesson layout,
equations, expanded solutions, captured figures, notebook rendering and both
capstone reports. `.build/visual-review.json` records that representative review;
the strict execution-policy change did not repeat an exhaustive visual or
print/PDF review.

Runtime Python warnings and non-empty `stderr` are release failures and have no
allowlist. A structured model or scenario result named `warnings` remains domain
data rather than a runtime diagnostic and is acceptable only when the lesson
checks and explains the expected condition.

Seven distinct external URLs authored directly in lesson prose were checked.
None was confirmed dead. The FASB pages require JavaScript, IFRS redirects to
sign-in, and ISDA returned 403, so their full content was not verified. The local
link gate checks all published internal routes, anchors and asset paths.

## Required retrospectives and schedule

Authoring timestamps in `coverage/` describe overlapping agent work, including
some focused validation. They are not measured human labour or learner time.
Execution records measure machine runtime, including fresh-baseline exercise
replays; they are not estimates of how long learning takes.

The completed lesson executions in this release run give the following checkpoint
measurements. These are local desktop wall times, not production benchmarks.
The authoring window can span overlapping batch work and later verification.

| Checkpoint | Recorded authoring window (UTC, 3 September 2026) | Displayed blocks | Execution including fresh exercise baselines | Learner time |
| --- | --- | ---: | ---: | --- |
| Part I | 03:49:51–04:13:42 | 26 | 23.02 s | Unmeasured |
| Monte Carlo II | 03:52:23–04:11:19; final visual additions completed 05:21:24 | 11 | 13.46 s | Unmeasured |
| C3 | 03:50:02–04:40:50 | 15 | 6.77 s | Unmeasured |

The Part I retrospective retains the complete money/date/curve coverage and
separates precision, market convention and calibration checks. The lessons now
use real valuation metadata and quote-repricing proofs. Budget convention and
fixture work separately from writing prose.

The Monte Carlo II checkpoint retains all path-dependent studies. Shared paths
and nested monitoring grids separate discretization effects from random-sample
differences; independent LSMC pricing separates fitting from policy valuation.
Reserve multiple sessions for interpretation and experiments. Runtime alone
cannot establish the learner schedule.

The C3 checkpoint retains actual pool records, reconciled one-period waterfalls,
full multi-period deal studies, stress cases and the separately priced BBB sleeve.
Reserve distinct sessions for collateral/waterfall reconciliation, valuation and
stress interpretation. Toy-deal receipts are reconciled calculations, not an
exported engine trace.

Learner completion times are unmeasured. At each checkpoint, record each learner's
required-session time and unresolved concepts, then revise the remaining delivery
calendar. Keep all exercises and solutions; extend the schedule when necessary.
The earlier 42–43-session and 12–14-week estimates are not commitments. Site
prerequisites have their own budget and are excluded from learner duration.

## Delivery boundaries

The implementation changes authoring, fixtures and documentation tooling. It adds
no production pricing APIs or binding capabilities. Supported replacements and
remaining model limitations are explicit in the syllabus gaps register and the
relevant lessons. Hosting and the complete unified API reference remain under the
original website plan; this record concerns the local static artifact.
