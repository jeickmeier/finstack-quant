# FEATURES-014: Cross-sectional centering, population and robust standardization

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

`Zscore`, `Demean`, `RobustZscore` and `MinmaxScale` accept no parameters. They operate independently on finite values in each exact time partition or grouped `(time_key,group)` partition, with missing rows excluded and preserved as None. There is no weighting, minimum population size, sector inference or financial-unit conversion.

## Ordered variants

- **Population z-score:** use [scaled centered values](04-scaled-statistics-and-quantiles.md), calculate their population SD (denominator n), and divide each centered value by that SD. If SD<=0 emit 0 for every finite row. Singleton partitions therefore emit 0. The output is dimensionless and differs from the sample denominator used by rolling z-score.
- **Demean:** compute scaled Kahan raw mean, then emit raw `x-mean`. Output retains input units. Finite raw values can still have an unrepresentable centered difference; final validation rejects it.
- **Robust z-score:** scale the sample by max(max_abs, f64::MIN_POSITIVE), sort, and calculate Type-7 median c. Sort absolute deviations from c, take their Type-7 median MAD, and emit `0.6744897501960817*(x/scale-c)/MAD`. If MAD<=0 emit 0 for *all* finite rows, including outliers in a majority-flat sample. The normal-consistency constant is not a claim that the sample is normally distributed.
- **Min-max:** divide values by max_abs (use scale 1 for all-zero input), find normalized minimum/maximum, and emit `(x/scale-min)/(max-min)`. A nonpositive range gives 0. Otherwise output is dimensionless in [0,1] up to floating arithmetic.

All-empty finite partitions leave missing outputs. Each transform is recomputed independently; population z-score and robust z-score do not carry state. Robust-zero-MAD behavior differs deliberately in code from Hampel's replacement rule in [FEATURES-022](22-hampel-filter.md); one must not infer a shared zero-MAD policy from their shared constant.

## Rationale and alternatives

The source documents population normalization and MAD normal-consistency scaling; comments explain scale stability. Proposed rationale is comparable cross-sectional signals, but no adoption rationale for population rather than sample dispersion or all-zero robust results was found. Alternatives include sample z-scores, zero-MAD missing/errors, or preserving the sign of zero-MAD outliers. They require explicit decisions.

## Evidence

- Owning [cross-sectional helpers](../../../../finstack-quant/features/src/cross_sectional.rs): `zscore`, `demean`, `robust_zscore`, `minmax_scale`; shared constants/statistics in [types](../../../../finstack-quant/features/src/types.rs).
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs): tied sample [1,2,2,4] has specified robust scores and min-max results; singleton and grouped population z-scores are checked. These small numerical fixtures use 1e-12 absolute tolerance.
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): dimensionless unit sweeps and opposite-sign 1e308 z-score avoid raw-scale overflow. Same-method comparisons cannot certify independent accuracy.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
