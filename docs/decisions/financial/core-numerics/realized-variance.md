# CORE-NUMERICS-10: Close and OHLC realized variance estimators

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/stats.rs`: equally spaced positive price observations or equal-length open/high/low/close bars. Outputs are annualized decimal variance, not volatility. The code has no timestamps, trading-calendar adjustment, corporate-action adjustment or missing-bar inference.

## Choice and rationale

Five explicit estimators capture different information in close and range observations. The enum default is CloseToClose; the OHLC function's omitted method is YangZhang. These are distinct entry-point defaults, not a universal estimator choice. Formula names identify implementations and do not establish suitability for a given market microstructure.

## Algorithm

Let A be annualization periods per year, n the number of bars, and logs use price ratios.

1. Close-to-close forms `r_i=ln(C_i/C_(i−1))` and returns `A Σr_i²/(n−1)` for n prices. It does **not** subtract mean return or apply sample-variance correction.
2. Parkinson uses all n bars: `A mean[ln(H/L)²/(4 ln 2)]`.
3. Garman–Klass uses `A mean[0.5 ln(H/L)²−(2 ln 2−1)ln(C/O)²]` over all n bars.
4. Rogers–Satchell uses `A mean[ln(H/C)ln(H/O)+ln(L/C)ln(L/O)]` over all n bars.
5. Yang–Zhang requires n≥3, pairs bars i=1..n−1 with previous closes, and sets m=n−1. Compute sample variances of overnight `ln(O_i/C_(i−1))` and open-close `ln(C_i/O_i)`, plus mean Rogers–Satchell over the same m bars. Return `A[Var_overnight+k Var_openclose+(1−k)Mean_RS]`, where `k=0.34/[1.34+(m+1)/(m−1)]`.

## Selection, controls and conventions

The caller supplies method and optional A; omitted A comes from `PeriodKind::Daily.periods_per_year()` (252 in this revision). OHLC validates all four arrays even when CloseToClose is selected. Decimal annualized volatility would be the square root of the returned variance; the helper does not take it.

## Limitations, failure and fallback

A must be finite and positive; every input price must be finite/positive. Bars must satisfy `L≤min(O,C)≤max(O,C)≤H`. Empty arrays return 0; close-to-close with fewer than two prices and Yang–Zhang with fewer than three bars return 0. Final variance below −1e-14 or nonfinite errors; tiny negative values are floored to 0. Price ratios can overflow even when individual prices are finite and positive, producing an error. No automatic estimator substitution exists.

## Alternatives and tradeoffs

All five alternatives are implemented. Range estimators consume intraperiod information, while Yang–Zhang includes overnight jumps and sample demeaning. A squared-return realized-variance contract differs from estimating variance after removing drift. No estimator is selected based on data diagnostics.

## Evidence

- [Owning implementation and inline tests](../../../../finstack-quant/core/src/math/stats.rs).
- `range_estimators_use_each_observed_bar_from_the_first` reconstructs the same formulas (1e-12 absolute) and checks one-bar inclusion; it is a convention regression, not independent market validation. `realized_variance_rejects_invalid_ohlc_and_annualization` and `realized_variance_defaults_are_rust_owned` cover errors and defaults.
- Direct primary citations for Parkinson/Garman–Klass/Rogers–Satchell/Yang–Zhang are not present as dedicated catalog anchors in the inspected reference catalog; adding them is an evidence gap, not an unperformed algorithm inspection.

Independent benchmark data and small-sample/bias assessments are not established by the formula-reconstruction tests.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
