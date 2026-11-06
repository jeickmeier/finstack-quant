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
