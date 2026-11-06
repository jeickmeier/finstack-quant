# FEATURES-010: Unannualized rolling period Sharpe feature

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

`RollingSharpe` is a research-period feature, distinct from the analytics Sharpe API. `window` defaults to 1, `min_periods` to window, and effective finite count is at least two. `risk_free` defaults to 0 and must be finite; it is in the same per-period return units as the observations, not an annual cash rate. The kernel does not verify that numeric input actually represents returns.

## Computation and boundaries

For finite observations in the inclusive trailing row window, compute scaled arithmetic mean `m` and sample SD `s` with denominator n-1. Return `(m-risk_free)/s` when s>0 and 0 when s=0. Insufficient observations produce None. Aggregation can emit at a missing current row. There is no geometric cash-rate conversion, square-root annualization, compounding, excess-return time series or date-frequency parameter.

Constant positive excess returns therefore give 0 rather than infinity; constant negative excess returns also give 0. An overflowing subtraction or final ratio is rejected by output validation. A nonfinite intermediate SD can be consumed before the final scan, so the scan alone is not proof that every degenerate finite result is economically intended.

## Rationale, alternatives and evidence

The API and crate README explicitly distinguish this feature from annualized analytics risk/reward. Proposed rationale is a simple standardized rolling signal. Constant-window zero, raw versus excess volatility and fixed-period cash input need feature-specific agreement; analytics conventions do not silently override this method. Alternatives include undefined zero-volatility output or an explicit annualization/frequency bridge, neither implemented here.

- Owning [advanced helper](../../../../finstack-quant/features/src/timeseries/advanced.rs) `rolling_sharpe`; selected and count-gated in [time-series rolling](../../../../finstack-quant/features/src/timeseries.rs).
- Inspected `transform_timeseries_supports_advanced_rolling_signal_ops` in [transform tests](../../../../finstack-quant/features/tests/transforms.rs): [1,2,3] yields 2 with zero cash and 1 with cash=1, checked at 1e-12 absolute tolerance. These verify a period formula rather than realistic return provenance or zero-volatility economics.
- [Shared scaled SD](04-scaled-statistics-and-quantiles.md) defines the numerical route and sample convention.

No standalone annualized-performance conclusion follows from this output.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
