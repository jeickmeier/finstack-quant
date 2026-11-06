# STATEMENTS-ANALYTICS-VALUATION-08: Forecast-error metrics and near-zero exclusion

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice, domain and formulas

[Backtesting](../../../../finstack-quant/statements-analytics/src/analysis/backtesting.rs) consumes two aligned, equal-length, nonempty finite arrays. Alignment, frequency, currency and observation weights are caller responsibilities. For actual `aᵢ`, forecast `fᵢ`, error `eᵢ=aᵢ−fᵢ`:

- `MAE = sum(abs(eᵢ))/n` and `RMSE = sqrt(sum(eᵢ²)/n)` use every pair and report the input unit.
- `MAPE = mean(100×abs(eᵢ)/abs(aᵢ))` excludes observations with `abs(aᵢ)<1e−10`; its retained count is `mape_effective_n`.
- `sMAPE = mean(100×abs(eᵢ)/(0.5abs(aᵢ)+0.5abs(fᵢ)))` excludes denominators `<1e−10`. Its retained count is used internally but is not separately returned.

Either percentage measure is `None` when all corresponding pairs are excluded. Percentages are percentage points, not decimal fractions. Zero actual with nonzero forecast is excluded from MAPE but contributes to sMAPE; negative actuals use absolute denominators. There is no bias metric, scale normalization, forecast horizon partition, rolling refit or statistical significance test in this helper.

## Numerical and wire controls

Sums use ordinary floating-point accumulation. The result is rejected if MAE, RMSE or an available percentage metric is nonfinite, including finite-input arithmetic overflow. No clipping masks extreme errors. JSON represents unavailable percentage metrics as null. MAE/RMSE fields have the shared explicit nonfinite wire adapter for constructed/deserialized result values; successful computation still enforces finite outputs. A summary renders percentages with two decimals and prints MAPE's effective count.

## Alternatives and evidence

The absolute `1e−10` exclusion is scale-dependent: currency rescaling can alter the retained MAPE population. Weighted errors, MASE, log errors and explicit zero-denominator error policies are alternative methods, not variants selected here.

Colocated perfect-forecast, constant-error, outlier, MAPE and near-zero cases provide hand-derived formulas/invariants; nonfinite-wire/null tests cover serialization. [Forecast backtesting integration](../../../../finstack-quant/statements-analytics/tests/forecast/forecast_backtesting_tests.rs) is additional located evidence. No primary reference for threshold selection or out-of-sample statistical qualification was established, and no tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
