# Credit calibration performance

Implemented from `2270da9537` on `codex/credit-calibrator-performance`.
The fixed implementation brief is `.plans/CREDIT-CALIBRATOR-PERFORMANCE-PLAN.md`;
current execution status is recorded below. No solver tolerances, quadrature
budgets, or quote-acceptance thresholds are relaxed by this work.

## Final results

All main phases are accepted: S0, BC1–BC5, H1–H3 and V1–V3. BC6 was evaluated
and rejected because BC1 leaves no representative Gaussian both-binding-cap
workload to justify paired-quadrature complexity. The CDS-option implied-vol
preparation remains the plan's separate follow-up. The fixed plan is unchanged;
the acceptance history and current status are recorded in this report.

| Workload | Baseline phase | Baseline ms | Final ms | Reduction |
|---|---|---:|---:|---:|
| Base correlation, isolated | s0-quiet | 8239.613 | 265.961 | 96.8% |
| Base correlation, full plan | s0-quiet | 8315.811 | 276.656 | 96.7% |
| Hazard, isolated | s0-quiet | 48.914 | 3.479 | 92.9% |
| Hazard, full plan | s0-quiet | 73.644 | 10.843 | 85.3% |
| Five distinct hazard bumps | h2 | 36.078 | 21.417 | 40.6% |
| Credit vol, 3×3 | v0 | 0.538 | 0.298 | 44.6% |
| Credit vol, 10×21 | v0 | 1.964 | 0.889 | 54.8% |

Base/hazard final measurements are a fresh repeat at `779e1ed192`, with ten
samples, a 3 s warmup, a 10 s requested measurement and 10,000 resamples.
S0 used a 1 s warmup; the final repeat deliberately warms longer. Earlier
accepted same-setting slice comparisons are retained below, and the
final values agree with H3's accepted timings. Criterion extended the two base
measurement windows to about 15 s to collect ten linear samples. Volatility
uses V3's twenty-sample, 3 s warmup run. H2 is the new bump-batch baseline and
V0 is the pre-volatility-change baseline; neither is mislabeled as original S0.
All raw samples and confidence intervals are retained in their phase folders.

Final explained reports exactly match V3. Base-correlation solver calls fall
from 177 to 31, plus the same ten commit/final repricings. Hazard calls fall
from 478 to 344. Five distinct nonzero bump requests perform six replay solves
instead of ten. These counts are separate from elapsed-time gains and from
SABR Jacobian work described in V3.

Compared with S0, maximum correlation-knot movement is `1.805e-10`, and maximum
hazard-knot movement is `1.756e-12`. Final maximum absolute quote residuals are
`9.284e-14` and `3.548e-15`, respectively. Original finite-pool references,
S0-derived PV/erosion bounds, solver tolerances, quadrature budgets and final
acceptance policies remain intact. Numerical and memory details for each
accepted slice are documented below.

### Final validation

The complete PR-gate stages passed, followed by `mise run rust-doc`. One
`mise run all` invocation ran the full lint stage; after the issues below were
corrected, the remaining stages were resumed individually rather than repeating
full workspace Rust lint/test.

- Full Rust lint, Python lint/typecheck/docs, WASM lint/boundary checks and
  repository convention/security hooks passed.
- Generation checks passed and confirmed idempotent generated artifacts.
- Workspace Rust tests: **16,269 passed**, 86 intentionally skipped.
- WASM validation and all four TypeScript declaration configurations passed;
  **1,089 facade tests passed**, including a repeat after the fixture refresh.
- Python: **13,945 passed** initially, 112 intentionally skipped, with one stale
  integration fixture failure. After the reviewed refresh, all **seven focused
  valuations-parity tests passed** without update mode, clearing that failure.
- UI: **1,525 tests across 125 files passed**, together with fixture, registry,
  style, token, arithmetic-boundary and type checks.
- Dependency audits passed; Rust documentation, input-argument, deprecation and
  canonical-public-path checks passed.

The first generation check rejected the abbreviated local name `dc` in a new
CDS cache test. Renaming it to `day_count` passed scoped lint and the affected
test; production code did not change. Python then identified the generated
cross-host valuations fixture's old serialized calibrated market. Reviewing its
21 changed leaves found only intended credit-curve/derived-value changes:
maximum curve movement below `9.52e-13`, CDS price movement `2.57e-6`, and index
CS01 movement below `9.57e-7`. The fixture was refreshed through its documented
generator, then verified through Python and WASM with the existing tolerances.
The independent financial references and S0 snapshot were not regenerated.

Final raw samples, numerical reports, integration-fixture deltas and validation
metadata are in `benchmarks/credit-calibration-evidence/final`. Per-slice
acceptance history follows. No performance claim relies on the optional
CDS-option follow-up or the rejected BC6 optimization.

## Reproduction

```sh
FQ_CREDIT_EVIDENCE_DIR=/tmp/credit-evidence \
FQ_BENCH_SAMPLE_SIZE=10 FQ_BENCH_WARM_UP_TIME=1 \
FQ_BENCH_MEASUREMENT_TIME=10 FQ_BENCH_NRESAMPLES=10000 \
mise run rust-bench-crate -- finstack-quant-calibration credit_calibration
mise run rust-bench-crate -- finstack-quant-valuations cds_tranche_pricing
```

The optional evidence directory records an **untimed** explained solve with
fitted curves, quote residuals and objective diagnostics. Criterion retains raw
samples and confidence intervals under `target/criterion`; copy those before
the next slice overwrites them. Run benchmarks sequentially without competing
builds. Release settings come from the workspace bench profile.

Full-plan cases include discount/hazard prerequisite calibration. Isolated cases
calibrate those prerequisites once outside the measurement, then use their real
outputs as the prior market. They still include market restoration, preparation,
solving, final validation and report construction. All timed solves assert fit
success. Parsing is outside both measurements. The hazard bump workload is one
parallel quote bump, not parallel execution.

## Numerical gates fixed before optimization

`credit_performance_baseline.json` was captured from the original engine in S0;
it is a regression snapshot, not independent financial evidence. The separate
SciPy finite-pool fixture and its `2e-10` bound are unchanged.

The snapshot test covers ordinary equity, a 0.1 percentage-point thin tranche,
ten-year high-detachment/recovery-sensitive exposure, distressed hazard,
boundary correlation, realized losses and noncontiguous attachment. It retains
raw capped expectations, every projected period's loss/recovery erosion, PV and
correlation sensitivity. Bounds are fixed from the existing `1e-10` pool budget:

- Raw cap comparison between engines: `2e-10` of pool notional.
- Tranche erosion comparison: `4e-10 / contractual width`.
- PV comparison: notional × erosion bound × `(2 × periods + 0.05 × (years + 1))`.
  The fixture discount factors are at most one. The protection term bounds
  changes in successive loss increments; the coupon term includes accrual.
- Correlation01 comparison: twice the PV bound (a conservative bound for the
  two bumped prices under the existing one-percentage-point central difference).

Test-only PMF counters report actual node work without instrumentation in release
pricing. Existing heterogeneous, Student-t, arbitrage/error, settlement and
independent-reference tests remain acceptance controls. New algorithmic results
are never used to regenerate the original-engine snapshot.

## BC1: exact Gaussian cap expectations

S0 was repeated after unrelated validation workers finished. Use `s0-quiet` for
comparisons; the initial `s0` timings had wider intervals and slower unchanged
controls. An interrupted BC1 run under CPU contention is excluded.

The table reports Criterion mean time in milliseconds, including preparation.
Confidence intervals and individual samples are retained beside each run.

| Workload | S0 quiet (ms) | BC1 (ms) | Change |
| --- | ---: | ---: | ---: |
| Base correlation: isolated | 8239.613 | 4028.962 | -51.1% |
| Base correlation: full plan | 8315.811 | 4114.187 | -50.5% |
| Equity PV | 35.562 | 11.402 | -67.9% |
| Mezzanine PV | 46.740 | 23.804 | -49.1% |
| Thin-tranche PV | 47.451 | 24.475 | -48.4% |
| High-detachment PV | 36.994 | 0.007 | -99.98% |
| Student-t control | 11.921 | 11.946 | 0.2% |
| Heterogeneous control | 437.723 | 429.526 | -1.9% |

BC1 retains no additional per-quote memory and adds no production counters.
PMF calls per price fell from 52,416 to 17,204 for equity, 70,716 to 37,604 for
the thin tranche, and 184,420 to zero for the high-detachment snapshot. Solver
calls changed from 177 to 180, plus ten commit/final repricings in each case;
the improvement is cheaper residual evaluations, not fewer solver iterations.

The largest knot movement was 1.32002e-10. Maximum final quote residual was
6.27884e-13 (S0: 7.89039e-13). Six fixed-correlation snapshot PVs were unchanged;
the ten-year high-detachment PV changed by 0.0000581751 on 10 million notional,
inside the bounds fixed in S0. All 86 scoped tranche tests and 15 calibration
tests passed. All 91 umbrella tranche tests and `mise run gen-write` also passed.
Generated calibration, market-browser and gallery fixtures were refreshed. A transient nextest leaky-process report cleared on a focused rerun.

For subsequent pure preparation slices, the test also supports same-build bit
comparisons: capture `FQ_CREDIT_SNAPSHOT=/tmp/prior.json` with the existing
`credit_performance_numerical_baseline` test, then rerun with
`FQ_CREDIT_BITWISE_REFERENCE=/tmp/prior.json`. Snapshots encode floating-point
bits as strings so JSON parsing does not alter the comparison. These are local
same-build checks, not cross-platform bit-identity promises.

## BC2: shared prepared tranche pricing

The prepared pricing layer retains immutable pool/schedule/discount inputs and
shares the pricer's validated copula through `Arc`. An initial version rebuilt
that copula and regressed the heterogeneous control. That version, plus local
slice/copy experiments, was not accepted. The retained shared-copula experiment
measured 428.35 ms [426.25, 430.45]; its immediately subsequent BC1 executable
control measured 445.32 ms [440.98, 449.72]. Earlier BC1 controls ranged from
419.98 to 433.30 ms. These controls support removing the regression, not a claim
that BC2 speeds up heterogeneous pricing. The final whole-calibration timing and scoped acceptance checks below passed.

Evidence folders named `*-hetero-*` represent only the
`cds_tranche_heterogeneous/npv_hetero/50` measurement. Other Criterion groups
that the snapshot copier retained in `bc2-hetero-shared` and
`bc1-hetero-final-control` are stale and must not be treated as measurements of
those variants. All targeted runs use 20 samples, three seconds warmup, a
20-second measurement target and 10,000 resamples. `bc2-repeat` is the original
BC2 pricing repeat only. The original BC2 full calibration report reproduced
BC1's complete JSON report exactly; the final shared-copula variant also passed the fixed-correlation bitwise gate
and reproduced both complete calibration reports exactly.

BC2 is accepted as a neutral preparation foundation. No financial outputs or
solver work counts changed; generated numerical fixtures therefore remain
unchanged. Means in milliseconds, with samples and confidence intervals in
`credit-calibration-evidence/bc2-final`:

| Workload | S0 quiet | BC1 | BC2 accepted |
| --- | ---: | ---: | ---: |
| Base correlation: isolated | 8239.613 | 4028.962 | 4047.698 |
| Base correlation: full plan | 8315.811 | 4114.187 | 4118.397 |
| Equity PV | 35.562 | 11.402 | 11.424 |
| Thin-tranche PV | 47.451 | 24.475 | 24.514 |
| Student-t control | 11.921 | 11.946 | 11.939 |
| Heterogeneous control | 437.723 | 429.526 | 423.460 |
| Preparation alone | — | — | 0.004636 |
| Prepared reuse, 3–7% | — | — | 23.826 |

The main price/calibration measurements include preparation. Reuse is labeled
separately and is not used as evidence of an end-to-end speedup. Stored inputs
are bounded by coupon count P: homogeneous pools use O(P) state; heterogeneous
pools use O(P×N), including four f64 arrays (32×P×N bytes) plus schedule, pool
headers and discount-date entries. Credit-index snapshots and validated copulas
are shared through Arc. BC2 retains no trial results.

Validation: scoped credit/calibration lint, 88 tranche tests with the BC1 bitwise
reference, 15 base-correlation tests, 91 umbrella tranche tests, and the scoped
public-input documentation check passed. One nextest process-leak report cleared
on a focused rerun. No independent references or tolerances changed.

Execution status: S0 and BC1–BC5 complete; BC6 rejected after workload review; H1–H2 complete; H3 in progress. Remaining slices are
pending. The fixed implementation plan is unchanged.

## BC3: bounded raw attachment reuse

BC3 retains one raw attachment vector per explicitly reusable prepared quote,
keyed by the actual twice-smoothed correlation. Immutable caps and pool inputs
belong to that quote. A changed interpolated attachment recomputes the vector;
failed trials retain no new entry. Tranchelet subtraction, floors, joint caps
and time-monotonicity repair still run on each evaluation. Ordinary one-shot
pricing uses the same kernel with cache storage disabled.

| Workload (mean ms) | S0 quiet | BC2 | BC3 |
| --- | ---: | ---: | ---: |
| Base correlation: isolated | 8239.613 | 4047.698 | 2304.735 |
| Base correlation: full plan | 8315.811 | 4118.397 | 2373.561 |
| Prepared reuse, 3–7% | — | 23.826 | 12.742 |
| Alternating interpolated 4–7% reuse | — | 24.265 | 24.276 |
| Alternating 4–7%, preparation + price | — | 24.322 | 24.370 |

The full-plan improvement is 42.4% over BC2 and 71.5% over S0; gains are not
added across slices. Solver calls, fitted knots and residuals are unchanged:
both complete calibration reports match BC2 exactly. All fixed-correlation
snapshot outputs remain bitwise equal to BC1.

The first pricing run showed common 2–4% drift in Student-t and interpolation
controls. Retained executable comparisons and longer repeats resolved this:
Student-t differed by about 0.4% between adjacent BC2/BC3 runs, and the two
interpolation controls above differed by less than 0.3%. These repeated
controls use 20 samples / 3 s warmup / 20 s measurement / 10,000 resamples;
main runs retain the original 10 / 1 s / 10 s / 10,000 settings. The cheap
high-detachment ordinary price moved from 7.583 to 7.772 microseconds, a small
absolute bookkeeping cost, while the end-to-end calibration includes all
preparation and cache construction costs.

Test-local PMF work fell from 33,400 to 17,408 calls on repeated 3–7% pricing
and from 34,004 to 17,408 on repeated flat-correlation 4–7% pricing. Tests also
exercise changing interpolated attachments, reordered curves and recovery
after arbitrage errors. Permanent cache storage is 16 × P bytes of raw values
plus one Arc/mutex/key entry; a replacement vector is temporary. No historical
trial curves or final loss vectors are accumulated.

Acceptance: 90 tranche tests with bitwise snapshots, 15 calibration tests,
91 umbrella tranche tests, and scoped credit/calibration lint passed. Raw
samples and identical reports are in `credit-calibration-evidence/bc3`;
`bc2-noncontiguous`, `bc2-student-control` and `bc3-control-repeat` retain the
matched control evidence. Financial tolerances and generated outputs did not
change.

## BC4: evaluated-bracket Brent with residual acceptance

The authoritative scan still chooses the same bracket. The shared Brent
implementation now accepts previously evaluated endpoints and can require
strict residual convergence. Bootstrap uses that mode and retains bounded
Illinois recovery from the last feasible bracket. An infeasible interpolation
gets one midpoint attempt charged to the existing recovery budget. Neither
iteration settings nor tolerances changed. Existing Brent entry points retain
their documented width-based behavior.

| Workload (mean ms) | S0 quiet | BC3 | BC4 |
| --- | ---: | ---: | ---: |
| Base correlation: isolated | 8239.613 | 2304.735 | 535.936 |
| Base correlation: full plan | 8315.811 | 2373.561 | 576.218 |
| Hazard: isolated | — | 49.069 | 36.146 |
| Hazard: full plan | — | 71.707 | 43.916 |
| Hazard: parallel rebootstrap | — | 98.612 | 72.219 |
| Discount step | — | 0.981 | 0.241 |
| Discount + forward | — | 0.995 | 0.253 |
| Global hazard, 6 quotes | — | 72.599 | 54.001 |
| Bootstrap hazard, 6 quotes | — | 109.300 | 67.219 |
| Global rate control, 16 quotes | — | 183.987 | 32.373 |
| Bootstrap rate control, 16 quotes | — | 103.857 | 24.739 |

Base calibration is 76.7% faster than BC3 in isolation and 93.5% faster than
S0. Full-plan improvement is 75.7% over BC3 and 93.1% over S0. Base objective
calls fell from 180 to 31, with ten additional commit/final repricings in both
versions. Hazard objective calls fell from 478 to 344. The base maximum quote
residual fell from 6.2788e-13 to 9.2838e-14; the hazard residual fell from
8.1963e-13 to 3.5479e-15. Maximum knot changes versus BC3 were 4.8440e-11
(base correlation) and 1.7559e-12 (hazard). Approximate knots remain absent.

Fixed-price S0 checks and BC1 bitwise snapshots pass. A new off-grid regression
prices the original S0 calibrated smile and the current smile on identical
market inputs at 1–5%, 6–6.1% and 12–25%. It uses the existing S0 pool-error /
tranche-width / cashflow bound, not a tolerance selected from the observed
change. All three pass. This isolates the smile change; the separate fixed-
price checks cover the pricing kernel.

Pricing controls: Student-t was 11.907 ms, unchanged within 1%; ordinary
homogeneous prices were stable or modestly faster. Heterogeneous pricing was
noisy: the first 459.956 ms run suggested a regression. Retained executable
comparisons gave BC3/BC4 means of 429.754/452.871 ms in one order and
426.776/432.051 ms in the reverse order. The latter difference is 1.2%; the
larger slowdown was not stable, and background `mediaanalysisd` was active.
Keep this control under observation in subsequent slices; do not attribute
its fluctuations to a pricing algorithm change. No heterogeneous pricing code
changed in BC4.

Validation: 2,205 core tests, 567 enabled calibration tests, the additional
off-grid test, scoped core/calibration lint, core public-input documentation,
and the fixed-price bitwise test passed. Both full credit/pricing benchmark
targets and the selected rate/global controls ran with successful-fit gates.
The global benchmark improvements include bootstrap-based initialization.

Evidence: `bc4` contains the fresh hazard/base and tranche groups;
`bc4-rate-controls` and `bc4-global-controls` contain the fresh shared-solver
controls. The discount groups incidentally copied into `bc4` are stale BC3
records and are not BC4 results. `bc3-hetero-adjacent`, `bc4-hetero-adjacent`,
`bc3-hetero-reverse` and `bc4-hetero-reverse` retain both longer comparisons.
Raw confidence intervals and samples are preserved. The initial BC4 log
`bench.log` predates the accepted repeat in `calibration.log`; use the saved
samples and the latter log for the table above.

BC4 integration: `mise run gen-write` completed. The affected UI tests passed
(30), Python calibration parity passed after a fresh dev build (42), and
WASM calibration parity/error tests passed (62). The shared cross-host golden
changed only its iteration count from 64 to 13; financial reference values
and tolerances were preserved. Generated UI/gallery residual digits and work
counts were regenerated together with the solver change. BC4 adds no
persistent pricing-cache memory.

## BC5: prepared binomial log coefficients

The core PMF now shares one recurrence implementation between one-off and
prepared callers. Preparation stores the unchanged `ln(N-k) - ln(k+1)`
expressions; conditional probabilities, normalization and accumulation order
are unchanged. A preparation-local Arc shares one lazy coefficient vector
across coupon dates. Fully analytic prices never initialize it. There is no
global pool-size cache.

| Workload (mean ms) | S0 quiet | BC4 | BC5 |
| --- | ---: | ---: | ---: |
| Base correlation: isolated | 8239.613 | 535.936 | 286.340 |
| Base correlation: full plan | 8315.811 | 576.218 | 328.290 |
| Hazard: isolated control | — | 36.146 | 36.005 |
| Hazard: full-plan control | — | 43.916 | 43.422 |
| Reused prepared 3–7% price | — | — | 6.193 |
| Student-t 3–7% price | — | 11.907 | 5.737 |

Isolated base calibration improves 46.6% over BC4; the full plan improves
43.0%, reaching 25.33 times the S0 throughput (96.05% lower latency).
Both complete calibration reports are exactly equal to BC4, including all
knots, residuals and 31/344 base/hazard solver calls. Fixed-correlation
snapshots remain bitwise equal to BC1. Numerical budgets are unchanged.

For N=125, p=0.2 the PMF mean falls from 645.72 ns to 264.56 ns after
preparation; preparation plus one fill costs 726.17 ns, so reuse is required.
N=25 and N=1000 and both extreme-tail cases show comparable reductions.
The one-off fully analytic high-detachment price remains 7.874 microseconds
(less than 1% change). Preparing a tranche costs 4.779 microseconds.
The initial heterogeneous control was 446.028 ms; adjacent retained BC4/BC5
executables measured 441.19/423.16 ms with longer 20-sample runs, resolving
the initial apparent slowdown. No heterogeneous algorithm changed.

Coefficient storage is 8 × N bytes per prepared homogeneous quote, plus one
Arc/OnceLock allocation and one Arc per prepared date. The N=125 vector is
1,000 bytes; it is allocated only when integration is actually needed.
The existing thread-local PMF output buffer remains reused.

Validation: scoped core, credit and calibration lint; 10 binomial tests,
90 tranche tests with bitwise snapshots, 16 base-calibration/off-grid tests,
91 umbrella tranche tests and core public-input documentation passed.
A process-leak sentinel failed once during a multi-process test run and passed
on its focused repeat. The full credit/pricing targets and new PMF target ran.
No numerical fixtures changed. Raw samples/reports are in `bc5`, with adjacent
controls in `bc4-hetero-before-bc5` and `bc5-hetero-repeat`.

## BC6: paired Gaussian quadrature rejected

After BC1 the production homogeneous path uses complementary caps K and 1-K
against exposures 1-R and R. Both caps cannot bind in exact arithmetic:
K < 1-R and 1-K < R contradict each other. Floating-point percent conversion
can create an ULP-scale seam (so this is not claimed to be strictly
unreachable), but no representative both-binding workload remains.
Heterogeneous issuer pools retain their separate integration rules and
non-Gaussian redesign is outside this slice. Adding a second adaptive Simpson
implementation therefore fails the plan's incremental-benefit requirement.
No paired-quadrature production code was added; all existing budgets remain.

## H1: shared prepared CDS workspace

`cds::pricing::PreparedCds` replaces the private risk-only cache. Risk and
hazard calibration now use that single valuations-owned workspace; the
ordinary and prepared paths share protection, premium-period and signed-NPV
calculations. Preparation validates and clones the contract, retains the
immutable discount Arc and caches coupon periods/accruals/DFs, upfront and
clean accrued. Pricing accepts only a trial hazard curve. The old scratch
market insertion path in the hazard target is removed.

The new hazard-phase baseline H0 adds distressed (8× quoted spreads), fixed
100 bp coupon with 0.5–1.7% upfronts, and 5/10/15/20/30-year cases. All fit
successfully. CDS pricing/par-spread benchmarks now reject errors. H0 and H1
include setup in whole-calibration timings; prepared pricing is labeled
separately. H0's base-correlation control was already faster than BC5 with no
production change, so that drift is not attributed to H1.

| Workload (mean ms) | H0 | H1 |
| --- | ---: | ---: |
| Hazard isolated | 35.132 | 35.070 |
| Hazard full plan | 42.760 | 42.495 |
| Distressed | 27.702 | 27.618 |
| Upfront quoted | 35.578 | 35.322 |
| Long tenor | 102.084 | 100.691 |
| Parallel replay | 70.379 | 69.710 |
| Base correlation isolated control | 249.039 | 249.764 |
| Base correlation full control | 294.732 | 292.795 |

H1 is accepted as a neutral preparation foundation for H2, not as a
standalone calibration speedup. All five complete calibration reports match
H0 exactly (standard, distressed, upfront, long-tenor hazard and base
correlation), including work counts, knots and residuals.

- Five-year ordinary price: 87.162 microseconds.
- Five-year setup only: 1.274 microseconds.
- Five-year prepared reuse: 85.711 microseconds.
- Five-year setup plus price: 86.749 microseconds.

Global-hazard six-quote controls: 51.740 ms global and 64.535 ms bootstrap, compared with BC4 controls of 54.001/67.219 ms.

Memory retained is one cloned CDS contract, one discount Arc and P entries of
`CouponPeriod` plus two f64 values (accrual and payment DF). No trial results
or integration windows are cached in H1; storage is O(P). Calibration keeps
one Arc to each prepared quote through bootstrap/global/final validation.

Validation: all 230 credit tests, all 568 enabled calibration tests, 453 CDS-
related umbrella tests, scoped credit/calibration lint and public-input docs
passed. New tests compare raw and prepared values bitwise across four
conventions, three dates, upfronts and hazard bumps; they verify discount
snapshot independence and invalid/missing-input errors. Existing rolled-
coupon, survival-floor and independent AoD integral checks remain intact.
Raw reports/samples are in `h0` and `h1`. No financial outputs or generated
fixtures changed; no new host binding surface is required for this workspace.

## H2: retained ISDA integration windows

Each prepared CDS now keeps discount windows for its last actual hazard
base date, day count and exact knot-time bits. Changing topology replaces the
bounded interval map; changing only rates reuses it. Contract, as-of,
discount snapshot, settlement rules and calendar are fixed by the owner.
The shared integration loop still computes survival and hazard rates for each
trial in the original arithmetic order. Ordinary pricing uses the same loop
without retained windows.

Discount/date errors are stored per window and observed only after the
original positive-default-mass predicate. Survival-floor exits still precede
grid construction. This preserves errors for zero-hazard and exhausted cases.

| Workload (mean ms) | H1 | H2 |
| --- | ---: | ---: |
| Hazard isolated | 35.070 | 3.615 |
| Hazard full plan | 42.495 | 11.053 |
| Distressed | 27.618 | 3.023 |
| Upfront quoted | 35.322 | 4.104 |
| Long tenor | 100.691 | 10.093 |
| Parallel replay | 69.710 | 7.265 |
| Global hazard, six quotes | 51.740 | 13.524 |
| Bootstrap hazard, six quotes | 64.535 | 15.034 |
| CDS bucketed CS01 | 379.332 | 40.262 |
| Base correlation full plan | 292.795 | 278.390 |

Standard isolated hazard is 89.7% faster; its full plan is 74.0% faster. All
five complete calibration reports match H1 exactly, including solver work,
knots, residuals and metadata. The global and umbrella tests remain clean.

Five-year prepared reuse falls from 85.711 to 7.253 microseconds. Preparation
alone remains 1.272 microseconds; cold preparation plus pricing rises from
86.749 to 90.034 microseconds (3.285 microseconds for cache construction).
Ordinary CDS pricing remains within about 1%; the added construction cost is
included in the calibration improvements above.

The initial isolated base-correlation control moved from 249.764 to 268.206
ms. Longer adjacent retained-binary comparisons measured 257.027/263.514 ms for H1/H2, a 2.5% difference rather than the initial 7.4%. This small
control cost is retained explicitly; no claim of unchanged isolated-base
latency is made. The full base plan improves because its prerequisite hazard
calibration benefits. Both runs use 20 samples, 3 s warmup, 20 s measurement
and 10,000 resamples; samples are in `h1-base-adjacent`/`h2-base-adjacent`.

An untimed test records 22 grid builds for the first five-year trial and zero
for a rate-only repeat. It retains 277 windows at 80 bytes of window/boundary
payload each (22,160 bytes), plus extra endpoints, vector capacities, map and
Arc/mutex metadata. Only the last topology is retained. Storage scales with
the contract's coupon/protection windows and knot layout, not trial count.

Validation: 233 credit tests, 568 enabled calibration tests, 453 CDS-related
umbrella tests and scoped credit/calibration lint passed. New tests cover
prefix growth, reordered topologies, exact knot changes, changed curve clocks,
negative rates, settlement delays, zero default mass and deferred DF errors.
No numerical fixtures moved. `h2` contains full raw reports/samples and the
work-count log. It also establishes H3's new batch baseline, with fresh provider
construction and fingerprinting included: one bump 7.235 ms, five ascending
36.218 ms, five reversed 36.178 ms, five concurrent 15.117 ms. Thread creation
is included in the concurrent timing.

## H3 — Share validated hazard source replay

Accepted. The batch-local cache now retains the parsed recipe and validated
identity replay separately from each bumped result. Identity keys include the
complete source content, discount content and hazard seed used by global
calibration, plus requested conventions. Target keys additionally include the
shocked market dependencies, recovery, bump and spread-risk-center mode.
Failures are not retained. Source and target markets remain distinct.

| Workload | H2 mean ms | H3 mean ms | Change |
|---|---:|---:|---:|
| hazard_bucket_batch/one_bump | 7.212 | 7.080 | -1.8% |
| hazard_bucket_batch/five_ascending | 36.078 | 21.354 | -40.8% |
| hazard_bucket_batch/five_reverse | 36.118 | 21.347 | -40.9% |
| hazard_bucket_batch/five_concurrent | 15.137 | 11.331 | -25.1% |
| calibration_hazard_isolated_5_pillars | 3.615 | 3.518 | -2.7% |
| calibration_base_correlation_isolated_5_knots | 268.206 | 266.409 | -0.7% |
| calibration_base_correlation_full_plan_5_knots | 278.390 | 276.280 | -0.8% |

Each timing constructs a fresh provider, including fingerprints/cache setup;
concurrent timings include thread creation. Tests count actual replay solves:
five distinct bumps require six solves, including concurrent repeated requests,
instead of ten. One bump still requires two. All five stored calibration reports
match H2 exactly. Cached and uncached bumped curves match canonical content.

Validation: 571 calibration tests, 453 CDS-related umbrella tests and scoped
calibration lint passed. Coverage includes reordered/zero/invalid bumps,
concurrent deduplication, source/target discount and seed mutations, recovery,
risk-center mode, failed identity retries, and existing recipe/FX checks.
The source cache retains one parsed recipe, quote vectors, settings and base
curve per distinct identity key, for the existing batch cache's lifetime.
It does not retain per-solver-trial objects. Raw samples and confidence intervals
are in `h3`; the ordinary CDS/risk target is retained as a consumer control.

Execution status: S0, BC1–BC5 and H1–H3 complete; BC6 rejected for lack of a
representative incremental workload. V1 is in progress; V2/V3 and final gate
remain pending. The fixed implementation brief is unchanged.

H3 evidence note: `calibration_discount_step` and
`calibration_discount_then_forward_steps` directories were copied from earlier
controls by the broad prefix collector; they are not fresh H3 measurements and
are excluded from H3 conclusions. All H3 table rows and CDS controls are fresh.

## V1 — Correct SABR work accounting

Accepted telemetry foundation. `residual_evaluations` now counts actual residual
closure calls across every start, including failed solves and rejected final
candidates. Winning-start statistics, strict score tie-breaking and all nine
starts remain unchanged. No stopping/acceptance tolerances changed.

| Workload | V0 mean µs | V1 mean µs | Change |
|---|---:|---:|---:|
| credit_vol/small_3x3 | 538.209 | 544.889 | +1.2% |
| credit_vol/large_10x21 | 1964.126 | 1980.468 | +0.8% |
| credit_vol/free_21 | 175.039 | 176.508 | +0.8% |
| credit_vol/pinned_21 | 175.387 | 176.639 | +0.7% |
| sabr_slice_calibration | 229.293 | 229.091 | -0.1% |

These are fresh volatility baselines, with 20 samples, 3 s warmup, 10 s
measurement and 10,000 resamples. The 10×21 surface uses beta 1, ten half-year
expiries and 21 spread strikes, with a flat discount curve and known non-flat
SABR quotes. Prerequisite calibration/parsing is excluded; restoration, quote
preparation, all fits, validation and reports are included. Direct 21-strike
free/ATM-pinned fits and the existing beta-0.5 slice are separate controls.
Every timed fit must succeed. V0/V1 evidence includes raw samples and confidence
intervals, full surface reports, direct fitted parameters and off-grid smiles.

The direct free fit reports 374 calls rather than the old winning-start-only
36; the pinned fit reports 323 rather than 26. The three-expiry fixture reports
2,879 / 1,348 / 1,251 calls instead of 147 / 146 / 131. These increases correct
telemetry; they do not represent additional solver work. All compared outputs
are exactly equal after excluding this counter, including fitted parameters,
selected starts, quote residuals, off-grid vols and iteration counts.

Validation: 2,052 enabled model tests and 571 calibration tests passed, with
scoped models/calibration lint. Controlled starts cover failed solves,
reconstruction rejection, residual rejection and tied winning scores. Twelve
normal/lognormal/shifted free/pinned fit snapshots and 11,664 formula/error
snapshots provide same-build checks for subsequent preparation. Public counter
documentation and generated reports are updated together. The counter adds one
stack-local integer per active solve, with no retained per-evaluation data.

Execution status: V1 complete; V2 in progress. V3 and the final integration gate
remain pending. No performance improvement is claimed for this telemetry slice.

## V2 — Prepare SABR strike constants

Accepted. Free-alpha and ATM-pinned fits prepare forward/strike/beta/time/shift
constants once per expiry and reuse the canonical expansion over all starts.
The ATM branch prepares only the terms it uses; snapped beta-one powers are
exactly one. Initial candidates introduced ordinary-formula overhead, which
was removed before acceptance. `v2`, `v2-refined` and `v2-inline` retain those
experiments; `v2-beta` is the accepted implementation. Inlining alone was
neutral and its annotations were removed.

| Workload | V1 mean µs | V2 mean µs | Change |
|---|---:|---:|---:|
| credit_vol/small_3x3 | 544.889 | 421.716 | -22.6% |
| credit_vol/large_10x21 | 1980.468 | 1324.088 | -33.1% |
| credit_vol/free_21 | 176.508 | 111.903 | -36.6% |
| credit_vol/pinned_21 | 176.639 | 123.033 | -30.3% |
| sabr_slice_calibration | 229.091 | 136.567 | -40.4% |
| sabr_formula/normal_21 | 0.139 | 0.135 | -2.5% |
| sabr_formula/cev_21 | 0.578 | 0.577 | -0.3% |
| sabr_formula/lognormal_21 | 0.384 | 0.382 | -0.5% |

All nine saved calibration/direct-fit reports match V1 exactly, including
residual counts, parameters, selected starts, residuals and off-grid smiles.
The 11,664 value/error and twelve fitted-outcome snapshots match the pre-V1
baseline exactly (excluding only V1's corrected count). The first preparation
passed 2,052 model tests and 571 calibration tests; the ATM refinement passed
all 79 SABR tests and scoped models lint. Calibration lint also passed.

End-to-end fit timing includes preparation. State is one fixed-size private
record per strike, retained only for the active expiry solve, with no per-trial
cache growth. Ordinary public formula controls include preparation on every
call. Raw samples and confidence intervals are retained for both candidates
and the separate V1 formula baseline; timing settings match V1. No native
financial fixture changes are needed because reports are identical.

Execution status: V2 complete; V3 in progress. The final gate remains pending.

## V3 — Analytic free-alpha SABR Jacobian

Accepted. The existing LM analytic-derivative interface now receives the
alpha/nu/rho derivatives of the canonical normal, lognormal, ATM, Taylor and
smoothstep branches, chained through the existing bounded coordinates. All nine
starts, weights, bounds, strict score comparisons and tolerances are unchanged.
ATM-pinned fitting retains its distinct numerical derivative algorithm.

| Workload | V2 mean µs | V3 mean µs | Change |
|---|---:|---:|---:|
| credit_vol/small_3x3 | 421.716 | 297.900 | -29.4% |
| credit_vol/large_10x21 | 1324.088 | 888.515 | -32.9% |
| credit_vol/free_21 | 111.903 | 69.543 | -37.9% |
| credit_vol/pinned_21 | 123.033 | 122.414 | -0.5% |
| sabr_slice_calibration | 136.567 | 80.237 | -41.2% |
| sabr_formula/normal_21 | 0.135 | 0.135 | -0.4% |
| sabr_formula/cev_21 | 0.577 | 0.579 | +0.3% |
| sabr_formula/lognormal_21 | 0.382 | 0.380 | -0.4% |

Validation: 2,060 model tests and 571 calibration tests passed, with scoped
models/calibration lint. Independent standard-library Decimal calculations at
80 digits cover 1001 valid physical-parameter cases,
36 inward boundary cases, and both sides of the chi/ratio
seams. The generator and pins are retained under `models/tests/fixtures`.
Interior references use `h=1e-25`; inward references use `h=1e-18`. Multi-step,
higher-order double-precision differences additionally cover well-conditioned
regions. Initial tiny-nu double-precision difference failures were investigated
against independent values; the analytic derivatives were correct, and the
reference method was improved without loosening derivative tolerances.

Twelve normal/lognormal/shifted free/pinned synthetic fits are deterministic,
recover the known parameters and pass 41 off-grid checks each. Their largest
parameter change from V0 is below `8e-14`; one selected start changes at nearly
machine-zero residual. The ten-expiry benchmark's largest parameter change is
`5.5e-15`; one selected start changes. In the flat three-expiry fixture all
selected starts change, but the same lower-nu bounds remain active and the
largest parameter change is only `3.82e-9` in rho. Its worst quote residual is
`4.82953857e-7`, versus `4.82953826e-7` before. These selections are immaterial
rounding/trajectory differences, not a changed acceptance policy.

The standalone native check in `v3/offgrid-comparison.rs` prices 81 strikes
from 0.002 to 0.020 for each of the thirteen expiry fits, using their actual
discount-derived forwards. Across 1,053 points the maximum absolute decimal-vol
change is `3.303e-12`, below the preset `1e-9` comparison bound. Direct 21-strike
free-fit off-grid changes are below `2.84e-15`. Pinned fits and all five saved
base/hazard calibration reports are exactly unchanged. Public formula/error
snapshots remain bitwise equal to V0.

Actual residual calls fall from 374 to 74 for the direct free fit, from 3,897
to 777 across the large surface, and from 5,478 to 1,194 across the small
surface. These counts exclude derivative work; end-to-end timings include all
Jacobian evaluation and validation costs. Pinned calls remain 323. The Jacobian
adapter borrows existing strike/weight slices and retains no additional trial
history; LM's existing Jacobian allocation is unchanged.

Evidence uses V1's 20 samples, 3 s warmup, 10 s measurement and 10,000 resamples.
The standalone diagnostic links the matching release rlibs with optimization
level 3, fat LTO and one codegen unit; its source and full point results are
retained. No timings include that untimed diagnostic. Relative to V0, the small
and large surface gains are about 45% and 55%, respectively.

Execution status: S0, BC1–BC5, H1–H3 and V1–V3 accepted; BC6 rejected as
documented above. Fixture regeneration and final integration checks are pending.
The separate CDS-option IV proposal remains outside these main phases.

V3 integration preparation: `gen-write` and `all-fmt` passed. Regenerated native
UI/gallery cases change by at most `3.68e-12` in decimal volatility and
`5.06e-9` in fitted parameters. Every success/failure flag is unchanged,
including the deliberately failing calibration fixture.
