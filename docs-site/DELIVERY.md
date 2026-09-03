# Analyst Program delivery record

The program contains 33 required units and two independent capstone reference
submissions. All original exercise groups are tracked in `coverage/`; concepts,
solutions and executable source identities remain reviewable together. Longer
units use mandatory sessions. The original implementation plans are unchanged;
the canonical syllabus and site specification reflect the approved corrections.

## Prerequisite checkpoint

P1 passed on 3 September 2026 at 03:50 UTC against Python 3.12.13 and the built
finstack-quant 0.8.0 extension. The checkpoint executed 98 copied notebooks,
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
98 output-free notebooks, including the five new notebooks required by the
handoff. Coverage maps 303 displayed executable blocks: 215 BuildIt blocks and
88 numerical exercise solutions. One additional exercise group has a complete
written solution. Each numerical solution executes from a fresh lesson baseline.

The complete publication command is `mise run docs-site-build`. The full run
executed all 300 then-current blocks in 1,085.20 seconds and all 98 notebooks in
625.82 seconds. Principal workflows, coverage and evidence checks passed. Site
compilation then found display-math delimiter and plugin-ordering issues. Their
focused correction and successful 30.06-second continuation are retained in
`.build/release.json`, including the original failed build stage.

Final visual review added three required Monte Carlo II blocks: held-out prices
against European/intrinsic values, an explicitly illustrative policy schematic,
and a path-count study. It also improved the one-page PM report's tables and
six-KPI layout and isolated test captures from published assets. The affected
lessons and notebook copies were re-executed; the final publication continuation
checks all source/runtime fingerprints before exporting the reader. Its status,
stage results and times are recorded in `.build/release.json`.

The final continuation passed at 05:22:26 UTC on 3 September 2026 in 23.84 seconds.
It verified all 33 principal workflows and current execution evidence, built all
138 static routes and passed the rendered local-link/asset gate. Both capstone
references passed from independent fresh processes. Browser inspection covered
the overview, desktop/mobile lesson layout, equations, expanded solutions,
captured curve/Monte Carlo figures, notebook rendering, the one-page PM report
and both capstone reports. `.build/visual-review.json` records that representative
review; it does not claim exhaustive page-by-page or print/PDF verification.

The final focused suite passed 82 tests in 100.17 seconds, covering fixture
pricing and reconciliation, the original notebook runner, snippets, source
materialization, coverage and dependency/output publication checks. Ruff passed
on all changed Python tooling, fixtures and tests. The reader's TypeScript check
passed. After test-asset isolation changed, all 44 documentation-tool tests passed
again in 2.51 seconds.

That evidence predates the strict notebook diagnostic gate and is superseded for
zero-diagnostic release proof. Runtime Python warnings and non-empty kernel
`stderr` are now release failures; they are not informational and have no
allowlist. A structured model or scenario result named `warnings` remains domain
data rather than a runtime diagnostic, and is acceptable only when the notebook
asserts and explains the expected condition. Do not claim the strict gate passed
until `mise run docs-site-build` has rerun all copied notebooks and recorded the
warning-free evidence.

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
| Part I | 03:49:51–04:13:42 | 26 | 12.11 s | Unmeasured |
| Monte Carlo II | 03:52:23–04:11:19; final visual additions completed 05:21:24 | 11 | 34.64 s | Unmeasured |
| C3 | 03:50:02–04:40:50 | 15 | 6.39 s | Unmeasured |

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
