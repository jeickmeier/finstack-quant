# FEATURES-008: Finite-sample rolling skewness and excess kurtosis

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Fisher estimators and inclusive window selection are implemented. Constant-window availability and checked central moments require reconciliation.

**Validation status:** Exact arithmetic checked both moments for a symmetric four-point sample. Rust fixtures were inspected. Constant-window and near-constant accuracy still require qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Use finite observations from the inclusive trailing window. Adjusted Fisher G1 skewness requires at least max(min_periods, 3) observations, and adjusted Fisher G2 excess kurtosis requires at least max(min_periods, 4). A missing current row can receive an estimate when the window qualifies. Both results are dimensionless.
2. Return missing for insufficient or genuinely constant samples. Preserve a zero estimate from a varying sample. For [1, 2, 3, 4], skewness is 0 and excess kurtosis is −6/5.
3. Use checked, scaled central moments and preserve small representable spreads. When numerical loss prevents estimating a varying sample, fail the whole call under [FEATURES-002](02-parameters-missing-and-failure.md).

## Observed scope and actual selection

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

## Follow-up tracking

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
