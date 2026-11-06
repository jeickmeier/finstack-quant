# STATEMENTS-EVALUATION-009 — Linear, damped Holt and moving-average trend forecasts

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice, scope and input conventions

Time-series trend methods extrapolate a supplied finite historical vector of length at least two. The evaluator still supplies/requires an anchor, but these kernels derive level/trend from the historical vector rather than the base argument. Observations are equally spaced by vector position; actual dates, financial calendar, seasonality and source-node consistency are not inferred. Method choice and parameters are explicit, not estimated by error minimization.

## Algorithms and controls

- **Linear trend:** set historical indices `t=1..n`, calculate means and ordinary least-squares slope `b=sum((t-tbar)*(y-ybar))/sum((t-tbar)^2)`, intercept `a=ybar-b*tbar`, and predict `a+b*(n+h)`. Arithmetic uses ordinary sums/centered products. There is no robust weighting, regularization or uncertainty interval.
- **Exponential smoothing:** require alpha and beta strictly between zero and one; damping phi defaults to one and must be in `(0,1]`. Phi is rejected for other trend methods. Initialize level with the first observation and trend from the slope between the first and the last of at most the first ten observations. Recursively update `level=alpha*y+(1-alpha)*(old_level+phi*old_trend)` and `trend=beta*(level-old_level)+(1-beta)*phi*old_trend`. Forecast `level+trend*sum(phi^j,j=1..h)`; phi one reduces to h times trend. This is the implemented Holt-style discrete trend recursion, not a statistical fit of optimal parameters.
- **Moving average:** require integer window from one through history length. Compute the trailing window mean. With an additional preceding observation, compute the immediately previous window mean and their difference as trend; forecast `last_mean+trend*((window-1)/2+h)`. Without that extra history, forecast the constant mean. The half-window adjustment centers the moving average before extrapolating.

All generated results must be finite; optional forecast bounds apply afterward without refitting or state feedback. Some local comparisons, such as alpha/beta open-range checks, do not separately reject NaN, but subsequent finite-output checks still reject nonfinite results for nonempty runs. Typed wire parsing imposes additional parameter constraints in the contracts scope.

## Rationale, limitations and alternatives

The code aims for simple transparent trend extrapolation and a centered moving-average forecast. This explains structure, not suitability for a financial account. The supplied history can differ from the evaluator's dated past; no spacing or economic regime check is applied. Ordinary summation can lose small changes around large levels or overflow finite partial sums. This is a numerical qualification concern under the accepted accumulation direction, not a quantified failure from this audit.

Alternatives include compensated/centered sufficient statistics, robust regression, fitted smoothing coefficients and uncertainty bands. A method using elapsed time would require dated inputs and different scaling. The accepted [numerical accumulation policy](../../standards/0042-numerical-accumulation-and-comparisons.md) remains applicable to these ordinary sums.

## Evidence

[Time-series implementation](../../../../finstack-quant/statements/src/forecast/timeseries.rs), [dispatch](../../../../finstack-quant/statements/src/forecast/mod.rs). Located inline `test_linear_trend`, `test_exponential_smoothing`, `test_moving_average_forecast_has_no_half_window_lag`, and [time-series tests](../../../../finstack-quant/statements/tests/forecast/time_series_tests.rs). [Forecast regressions](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs) include bounded constant moving-average overflow. These are formula/regression evidence; parameter estimation, empirical backtesting and independent production reference comparisons remain unestablished. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
