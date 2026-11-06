# FEATURES-008: Finite-sample rolling skewness and excess kurtosis

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and actual selection

`RollingSkew` and `RollingKurtosis` use finite values inside the inclusive trailing row window. Defaults are window=1/min_periods=window, but skewness needs at least max(min_periods,3) and kurtosis max(min_periods,4). They may emit at a missing current row. There is no annualization or weighting, and outputs are dimensionless.

## Ordered formulas

Scale and center the finite sample using the common helper, giving deviations `d_i`. Form `S2=sum(d_i²)` and `s²=S2/(n-1)`. Unlike the shared SD kernel, higher-moment reductions use ordinary floating additions.

- Skewness computes `S3=sum(d_i³)` and `G1=n*S3/[(n-1)(n-2)*s³]`.
- Excess kurtosis computes `S4=sum(d_i⁴)` and `G2=n(n+1)*S4/[(n-1)(n-2)(n-3)*s⁴] - 3(n-1)²/[(n-2)(n-3)]`.

If sample variance is nonpositive, both return 0. This includes truly constant data and any spread lost to rounding/underflow. The sample-count minimum produces None, while a nonfinite emitted moment fails the entire transform. No bias-correction option, raw kurtosis or alternate estimator is exposed.

## Rationale and alternatives

The enums identify Fisher G1/G2; the implementation explicitly includes their finite-sample factors. The source documents scale independence as a design objective but does not provide a source-specific adoption rationale for zero output on constant samples. Population moments, uncorrected central-moment ratios and missing constant-window results are meaningful alternatives. Scaling protects large magnitudes but does not establish reliability when normalized spreads are very small.

## Evidence

- Owning [advanced helpers](../../../../finstack-quant/features/src/timeseries/advanced.rs): `skewness`, `excess_kurtosis`; minimum-count dispatch in [time-series rolling](../../../../finstack-quant/features/src/timeseries.rs).
- Inspected `transform_timeseries_supports_advanced_rolling_signal_ops` in [transform tests](../../../../finstack-quant/features/tests/transforms.rs): symmetric skew=0, a window of three cannot produce kurtosis, and [1,2,3,4] has expected excess kurtosis -1.2, tolerance 1e-12.
- Inspected `dimensionless_features_do_not_depend_on_measurement_units` in [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): scales 1e-150 through 1e150 and a three-point expected skew sqrt(3). Same-kernel scale comparisons are not independent estimator validation.

No external estimator reference or high-precision oracle was authenticated in this audit.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
