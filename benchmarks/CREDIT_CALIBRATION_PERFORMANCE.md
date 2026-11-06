# Credit calibration performance

Implementation starts from `2270da9537` on `codex/credit-calibrator-performance`.
The fixed implementation brief is `.plans/CREDIT-CALIBRATOR-PERFORMANCE-PLAN.md`;
execution status is tracked separately there. No solver tolerances, quadrature
budgets, or quote-acceptance thresholds are relaxed by this work.

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

Execution status: S0 and BC1–BC4 complete; BC5 in progress. Later slices remain
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
