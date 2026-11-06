# ANALYTICS-002: Compounded wealth, relative wealth, cash conversion and dated CAGR

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, domain and rationale

Wealth is accumulated in log space with Neumaier compensation, preserving small returns and avoiding intermediate wealth overflow in drawdown/CAGR calculations. The choice is explicit in source; its input/output-domain policy is not independently approved by that numerical rationale. Returns and annual effective cash rates are decimal fractions. Panel observations are finite simple returns above −1; standalone `max_drawdown` reaches the same wealth engine without panel validation.

## Algorithm

For each finite `r > −1`, accumulate `L += log1p(r)`. Any nonfinite value permanently makes that engine's result NaN. For a finite `r <= −1`, the private engine instead adds `log(1e−18)`: exact wipeout and below-wipeout losses become a tiny positive growth factor. Panel construction rejects that domain, but standalone drawdown exposes it. This is observed repair behavior, not permission to reinterpret losses.

Total return is `expm1(L)` (empty input is zero); cumulative return emits that value after each step; geometric mean is `expm1(L/n)` (empty is NaN). Arithmetic mean is a compensated sum divided by n, annualized by multiplication with frequency A; it is not a compounded return. Terminal reconstruction can still overflow even though intermediate log accumulation remains representable.

CAGR is `expm1(L/T)`, with T measured between the active span's initial and final valuation dates. It divides before exponentiation, so representable annual growth can survive unrepresentable terminal wealth. Empty private CAGR input or nonpositive date span errors; an empty public ticker window yields NaN. Explicit `DayCount` routes to the core year-fraction function with optional calendar; Bus252 without a calendar errors. The current default `Act365_25` uses actual calendar days/365.25, including in Calmar, Martin, Sterling, Burke, Pain and summary. **[Accepted ADR 0008](../conventions/0008-financial-defaults-and-cagr.md) requires Actual/365 Fixed instead; implementation and its current regression remain unreconciled.**

Annual effective cash R is converted to a per-observation rate `q=expm1(log1p(R)/A)` (A≈1 within machine epsilon returns R directly). Annual arithmetic excess return is `A*mean(r)−A*q`, not annual mean minus R. Nonfinite cash returns NaN; scalar boundary conventions at/below −1 follow this formula rather than a common validation error. `Performance.excess_returns` requires a cash-rate vector matching the entire active date grid, then slices it by each ticker's span; omitted A uses frequency and nonpositive/nonfinite A yields aligned NaNs.

Relative cumulative wealth accumulates `log1p(portfolio)−log1p(benchmark)` on their overlapping dates and emits `expm1`; it does not subtract cumulative returns. This avoids dividing two potentially overflowing wealth values.

## Selection, alternatives and limits

The frequency label supplies A; elapsed dates supply CAGR T. Irregular gaps do not alter the equal-per-observation mean/volatility/cash conversion. Explicit alternative day counts are supported, but replacing the accepted default with 365.25 is not an open alternative. A cash-flow-weighted return calculation, resampling to a regular grid, and an absorbing wealth-zero state are not selected here. The previous actual-initial-date direction in [record 01](01-panels-windows-and-wire.md) is necessary for a known first holding interval.

## Implementation and validation evidence

- [Wealth/cash kernels](../../../../finstack-quant/analytics/src/returns.rs), [CAGR/mean kernel](../../../../finstack-quant/analytics/src/risk_metrics/return_based.rs), [public dated calculations](../../../../finstack-quant/analytics/src/performance/scalar.rs), and [relative wealth/cash alignment](../../../../finstack-quant/analytics/src/performance/aggregation.rs).
- Located unit tests cover tiny-return compounding, geometric RF, explicit day counts and linear arithmetic annualization. Inspected [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) name log-growth annualization, extreme identical relative wealth, finite drawdowns and positive near-wipeout growth; these are algebraic/numerical invariants, not external price-return provenance.
- [Correctness regression](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) `performance_cagr_uses_default_act_365_25_convention_for_single_return_window` protects the current default, which conflicts with the accepted convention.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
