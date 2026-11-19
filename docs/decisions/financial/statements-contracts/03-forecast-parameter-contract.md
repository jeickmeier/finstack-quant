# STATEMENTS-CONTRACTS-003: Typed forecast parameters and validation staging

**Decision status:** Accepted for the full typed forecast and validation staging contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-18). A JavaScript `bigint` in a plain-object Statements input (a model, a node, a forecast specification or a Monte Carlo configuration) is read as the seed's decimal wire text; the published TypeScript type of a seed is its wire text. The evaluation-time checks run where each method runs: a base that a multiplicative or geometric method cannot use, a history a trend or bootstrap cannot use, and an active correlation in a single run are refused or leave the cell unavailable with its reason. `ForecastSpec` is one closed typed enum, a method with its typed parameters, replacing the free-form parameter map; it is what a model stores and every method runs on. An unknown, duplicate, `null` or method-inapplicable field is rejected when a document is read or a parameter is set. The time-series method, bootstrap mode, seasonal mode and fade shape have no default. A seed is admitted only for a stochastic method and is an exact `u64`: a canonical decimal string in JSON, an integer in Rust and Python. Static requirements are checked at build and load even when observations cover every forecast period: finite numbers, ordered bounds, non-negative standard deviations, conditional parameters (a `half_life` only with the exponential fade, smoothing factors only with the exponential trend, a `window` only with the moving average), a growth curve that covers the model's forecast periods, override keys that are periods of the plan and a correlation peer that is another stochastic forecast.

**Validation status:** Qualified by a linear fade carrying `half_life`, an unknown, a `null` and a duplicate field, a forecast that never runs but is still checked, and seed `9007199254740993` round-tripped through Rust, Python and the JSON wire.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for typed forecasts and validation staging

1. Use one checked, closed, typed forecast specification across Rust, Python, WASM and JSON. Reject duplicate, unknown, null and method-inapplicable fields. Require explicit bootstrap, time-series, seasonal and fade method choices. An omitted optional lower or upper bound leaves that side unbounded. [Strict numeric wire](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity).
2. Validate all static method requirements before publishing a runnable model, including supplied history, finite values, ordered bounds, valid standard deviations and conditional method parameters. At build or load, check units, canonical period keys, required configured coverage and any selected correlation peer. Apply these checks even when explicit observations override the forecast. [Model readiness](01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness).
3. Specify rates, growth and reversion speeds as decimal quantities per model period. Use the method's required units for levels and absolute parameters and require the series' native currency where monetary. Interpret period counts against the model plan. At evaluation, verify the actual horizon, eligible base and history, and support for the selected execution mode. [Observation availability](02-authoring-and-availability.md#accepted-decision-for-authoring-imports-and-observation-availability).
4. Admit seeds only for stochastic methods and require exact `u64` values. Use canonical decimal strings in JSON, integers in Rust and Python, and `bigint` or wire text in JavaScript. For example, the JSON seed `"9007199254740993"` represents the exact integer `9007199254740993`.

A linear fade with a `half_life` fails static validation. A geometric fade checks its eligible anchor when the forecast runs.

## Audited input domain

The sections below describe the implementation audited before reconciliation.

ForecastSpec persists a method plus a free-form JSON parameter map. parameters() decodes that map to a closed ForecastParameters variant; validate() performs this decoding only. Forecast mathematics and numerical/domain validation belong to the forecast/evaluation scope.

The eleven methods are forward_fill, growth_pct, curve_pct, normal, log_normal, override, time_series, seasonal, mean_reverting, bootstrap and fade_to_target. Rates, growth, reversion speeds and shock distribution parameters are per model period, not automatically annualized.

## Audited algorithm and controls

1. Select the typed parameter structure from the ForecastMethod enum.
2. Reject every explicit null-valued top-level parameter; optional values must be omitted. Deserialize with unknown-field rejection and include the nested parameter path in a Forecast error.
3. Required fields remain method-specific: rate; ordered curve; mean/std_dev/seed; period-string overrides; historical series; seasonal length/mode; long-run mean/reversion/std_dev/seed; or target. All variants accept optional min/max bounds in the line's base units.
4. Defaults are bootstrap growth changes, linear time-series trend and linear target fade. Seasonal combination mode is explicit. Time-series exponential/moving-average controls and exponential fade half_life remain optional in the wire shape, with conditional validity checked by the executing method.
5. A seed is either an exact nonnegative integer representable as u64, or a finite nonnegative integer-valued float strictly below 2^64. Fractional, negative and out-of-range float seeds error. Seed mutation is available only for normal, log-normal, mean-reverting and bootstrap parameters; deterministic methods do not acquire a seed.
6. Typed bounds access returns the selected variant's min/max unchanged. It does not clamp, reorder or validate them here.

Convenience constructors serialize their inputs into the parameter map and select the corresponding method. They do not run a forecast or make method-specific conditions true.

## Limitations and alternatives

Structural acceptance does not prove nonnegative standard deviation, valid smoothing/reversion/correlation ranges, sufficient history, positive bootstrap-growth data, compatible min/max, valid period override keys, or a legal conditional parameter combination. These are evaluated later. Correlation controls are present on normal/log-normal/mean-reverting variants for Monte Carlo use; their presence does not mean standalone projection applies correlation.

The accepted contract requires common static method validation before model publication. History and horizon requirements that depend on evaluation inputs are checked at runtime.

## Evidence

[Forecast parameter types](../../../../finstack-quant/statements/src/types/forecast_params.rs), [ForecastSpec conversion](../../../../finstack-quant/statements/src/types/node.rs), [forecast regression tests](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs), and [model regression tests](../../../../finstack-quant/statements/tests/finance_model_regressions.rs) provide contract and selected rejection evidence. The model regression for invalid forecast parameters confirms that shape validation occurs even when actual values win precedence. No forecast simulation or seed reproducibility test was executed here.

Successful parameter decoding does not certify forecast-domain validity, history sufficiency or correlation behavior.

## Follow-up tracking

This scope has no open follow-ups; the [follow-up register](../FOLLOW_UPS.md) tracks the other scopes.
