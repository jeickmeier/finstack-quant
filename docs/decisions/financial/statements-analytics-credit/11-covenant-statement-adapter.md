# STATEMENTS-ANALYTICS-CREDIT-11: Statement-period covenant evaluation and forecast delegation

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

The statements adapter forwards computed scalar metrics and the model's actual period boundaries to the covenant engine. It owns period/date selection and delegation, not the covenant threshold or stochastic forecast algorithms themselves.

## Scope and input domain

`StatementResult`, covenant engine/specifications and forecast configuration. The result's complete period sequence is passed, including actual periods. Values are exposed as raw optional scalars.

## Algorithm

1. `StatementsAdapter::get_scalar` delegates to the statement result lookup. For a period identifier, find the corresponding stored period; use its start date and `end.previous_day()` as inclusive end. Missing boundaries error.
2. `evaluate_covenants` collects all result period IDs without sorting or filtering actuals, wraps the result and calls `evaluate_series_from_model` in the covenants crate. The delegated path uses configured applicability/waivers and lazily requests metrics needed for active tests; it does not append evaluation history in this adapter.
3. `forecast_breaches` passes the same complete ordered period list, adapter and caller configuration to the generic covenant forecast path. The delegated path uses an explicit reference date or the day before the first period start; it validates ordered periods and requires explicit volatility where probabilistic forecasting uses it.
4. Exact threshold comparisons, probability models, scope selection, breach sorting and validation inside the covenants crate are a separate source-ownership boundary.

## Selection and controls

Model period dates take precedence over the nominal dates implied by `PeriodId`, enabling fiscal boundaries. The adapter does not silently restrict forecast input to forecast-only periods and does not create volatility defaults.

## Conventions

Stored periods use exclusive ends; the covenant adapter converts them to inclusive testing dates by subtracting one day. Scalar metrics must already use the units required by each covenant definition. Calendar date handling is distinct from trading-day/day-count calculations.

## Limitations, failure and fallback

Calling this wrapper does not independently validate accounting units or standardize node meanings. The result sequence must satisfy the delegated engine's ordering contract. This audit covers the assigned adapter and traced call boundary; it does not certify every production algorithm in the separate covenants crate.

## Alternatives and tradeoffs

A forecast-only wrapper could filter periods but would alter historical/actual applicability semantics. Explicit caller-supplied period selection is more flexible but requires a separate API contract. Keeping calendar resolution in the adapter avoids fiscal-date inference by generic engines.

## Evidence

[covenants.rs](../../../../finstack-quant/statements-analytics/src/analysis/credit/covenants.rs) and inline `fiscal_horizon_uses_model_boundaries`, `covenant_volatility_must_be_explicit`, partial-metric forecast tests establish adapter/date behavior. [analysis_covenants.rs](../../../../finstack-quant/statements-analytics/tests/analysis_covenants.rs) is integration evidence. Delegated owners are [covenant series](../../../../finstack-quant/covenants/src/series.rs) and [forward forecasting](../../../../finstack-quant/covenants/src/forward.rs); their algorithm inventory remains outside this 30-file scope.

The separate [covenant catalogue](../covenants/README.md) records engine source coverage, including [dated series](../covenants/07-dated-series.md), [forecast composition](../covenants/08-forecast-composition.md) and [analytic](../covenants/09-analytic-probability.md)/[Monte Carlo](../covenants/10-monte-carlo.md) probabilities. Adapter coverage does not establish engine qualification.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
