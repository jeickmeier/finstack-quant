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
