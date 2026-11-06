# ANALYTICS-012: Sliding-window alpha and beta with recentered compensated sums

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and domain

Rolling alpha/beta are observation-window OLS point estimates over benchmark/ticker overlap, using the same Jensen cash convention as batch regression. Window zero errors publicly; oversized windows yield empty vectors; one-point/constant-benchmark windows yield NaN. Dates label window ends. There are no rolling confidence intervals, R² or adaptive bandwidths.

## Algorithm and controls

For each window of w points, recenter portfolio and benchmark observations around shift_r/shift_b, initially the window's first values. Maintain Neumaier sums Sr, Sb, Srb and Sb² of shifted returns and products. Compute denominator `D=w*Sb²−Sb*Sb` and slope `(w*Srb−Sb*Sr)/D`; reconstruct unshifted means and annualized Jensen alpha. A window with w<2 or nonpositive/nonfinite D yields NaN for both alpha and beta.

Sliding updates subtract outgoing shifted values/products and add incoming ones. The implementation tracks maximum variance, covariance and return-sum scales since the last rebuild. It fully recenters/recomputes when denominator, covariance numerator, shifted return sum or recovered mean suffers the configured sqrt(machine epsilon) relative cancellation test; when any sum is nonfinite; or after 64 updates. This trigger rebuilds an estimate; it does not classify every tiny positive benchmark variance as zero. Nonfinite input in private callers can affect its windows and recover once it exits; public Performance normally prevents such inputs.

The 64-update interval is justified in source as a drift/performance tradeoff for four cross-term sums and compared with the 1,024-step scalar rolling kernels. The decision is internal and not user-tunable. Typical updates are constant work, but recurring cancellation triggers can force full-window work for many points.

## Conventions, wire limitations and alternatives

Alpha is annual arithmetic excess return, beta dimensionless, cash an annual effective rate converted to per-observation q. Frequency and cash do not determine the rolling dates or ensure equal elapsed intervals. Full-window independent recomputation is a numerical alternative; exponentially weighted regression or robust covariance estimates are different unimplemented methods.

`RollingGreeks` derives serde on plain `Vec<f64>` alphas/betas, unlike `DatedSeries`' explicit nonfinite sequence adapter and batch regression's explicit nonfinite fields. The kernel intentionally produces NaN at undefined windows. Therefore lossless roundtrip/host representation of those values is not established by this output type; the adapter discrepancy is statically confirmed, while a complete wire failure was not executed. This wire boundary remains unqualified under [ADR 0009](../conventions/0009-missing-and-invalid-data.md); that does not permit silent substitution of zero.

## Implementation and validation evidence

- [Rolling implementation and output type](../../../../finstack-quant/analytics/src/benchmark.rs) and [public overlap/window guard](../../../../finstack-quant/analytics/src/performance/benchmark.rs).
- Located inline `rolling_greeks_stays_close_to_exact_recomputation_on_long_series`, constant-window and NaN-exit tests compare to fresh OLS windows. [Audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) cover outlier/constant spans and tiny nonzero benchmark variance. They are numerical consistency cases, not independent regression-oracle validation or wire roundtrips for RollingGreeks.
- [Accuracy/stability reference](../../../REFERENCES.md#higham-accuracy-and-stability) is context, not provenance for the exact 64-step/relative-trigger settings.

No tests, serialization experiments or benchmarks were run.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
