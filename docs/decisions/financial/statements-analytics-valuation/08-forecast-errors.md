# STATEMENTS-ANALYTICS-VALUATION-08: Forecast-error pairing, metrics and coverage

- **Decision status:** Accepted for the full forecast-error pairing, metrics and coverage contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for forecast-error pairing and metrics

1. Pair the forecast as issued with an observation for the same metric and target period, using compatible units and currency. Record forecast issue date, horizon, cutoff and actual snapshot. Reject duplicate or mismatched pairs. Require complete coverage of the selected periods by default. An explicit partial-coverage election records every missing pair.
2. Use equal-weight MAE and RMSE over all admitted pairs, in the input unit and currency. Use checked scaled arithmetic to avoid premature overflow.
3. MAPE divides absolute error by absolute actual and excludes only exact-zero actuals. sMAPE divides absolute error by the average of the actual and forecast magnitudes and excludes only pairs where both values are exactly zero. A pair with exactly one zero contributes 200% to sMAPE. Signed actuals and forecasts are valid. Encode percentages in percentage points, so a reported value of `10` means 10%.
4. Report expected and matched pair counts, separate usable MAPE and sMAPE counts, and reasons for exclusions. A metric with no usable pairs is unavailable with a reason. Apply the same checked values and counts in Rust, Python, WASM and the wire representation. [Typed results and exports](../statements-evaluation/13-result-units-and-exports.md#accepted-decision-for-typed-results-and-exports).

For example, actuals of −100, 0 and 0 paired with forecasts of −90, 10 and 0 give MAE of 6.67 and RMSE of 8.16 on three pairs, MAPE of 10% on one pair, and sMAPE of 105.26% on two pairs.

## Audited choice, domain and formulas

[Backtesting](../../../../finstack-quant/statements-analytics/src/analysis/backtesting.rs) consumes two aligned, equal-length, nonempty finite arrays. Alignment, frequency, currency and observation weights are caller responsibilities. For actual `aᵢ`, forecast `fᵢ`, error `eᵢ=aᵢ−fᵢ`:

- `MAE = sum(abs(eᵢ))/n` and `RMSE = sqrt(sum(eᵢ²)/n)` use every pair and report the input unit.
- `MAPE = mean(100×abs(eᵢ)/abs(aᵢ))` excludes observations with `abs(aᵢ)<1e−10`; its retained count is `mape_effective_n`.
- `sMAPE = mean(100×abs(eᵢ)/(0.5abs(aᵢ)+0.5abs(fᵢ)))` excludes denominators `<1e−10`. Its retained count is used internally but is not separately returned.

Either percentage measure is `None` when all corresponding pairs are excluded. Percentages are percentage points, not decimal fractions. Zero actual with nonzero forecast is excluded from MAPE but contributes to sMAPE; negative actuals use absolute denominators. There is no bias metric, scale normalization, forecast horizon partition, rolling refit or statistical significance test in this helper.

## Audited numerical and wire controls

Sums use ordinary floating-point accumulation. The result is rejected if MAE, RMSE or an available percentage metric is nonfinite, including finite-input arithmetic overflow. No clipping masks extreme errors. JSON represents unavailable percentage metrics as null. MAE/RMSE fields have the shared explicit nonfinite wire adapter for constructed/deserialized result values; successful computation still enforces finite outputs. A summary renders percentages with two decimals and prints MAPE's effective count.

## Audited alternatives and evidence

The absolute `1e−10` exclusion is scale-dependent: currency rescaling can alter the retained MAPE population. Weighted errors, MASE, log errors and explicit zero-denominator error policies are alternative methods, not variants selected here.

Colocated perfect-forecast, constant-error, outlier, MAPE and near-zero cases provide hand-derived formulas/invariants; nonfinite-wire/null tests cover serialization. [Forecast backtesting integration](../../../../finstack-quant/statements-analytics/tests/forecast/forecast_backtesting_tests.rs) is additional located evidence. No primary reference for threshold selection or out-of-sample statistical qualification was established, and no tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
