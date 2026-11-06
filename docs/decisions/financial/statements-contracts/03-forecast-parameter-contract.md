# STATEMENTS-CONTRACTS-003: Typed forecast parameters and validation staging

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

ForecastSpec persists a method plus a free-form JSON parameter map. parameters() decodes that map to a closed ForecastParameters variant; validate() performs this decoding only. Forecast mathematics and numerical/domain validation belong to the forecast/evaluation scope.

The eleven methods are forward_fill, growth_pct, curve_pct, normal, log_normal, override, time_series, seasonal, mean_reverting, bootstrap and fade_to_target. Rates, growth, reversion speeds and shock distribution parameters are per model period, not automatically annualized.

## Algorithm and controls

1. Select the typed parameter structure from the ForecastMethod enum.
2. Reject every explicit null-valued top-level parameter; optional values must be omitted. Deserialize with unknown-field rejection and include the nested parameter path in a Forecast error.
3. Required fields remain method-specific: rate; ordered curve; mean/std_dev/seed; period-string overrides; historical series; seasonal length/mode; long-run mean/reversion/std_dev/seed; or target. All variants accept optional min/max bounds in the line's base units.
4. Defaults are bootstrap growth changes, linear time-series trend and linear target fade. Seasonal combination mode is explicit. Time-series exponential/moving-average controls and exponential fade half_life remain optional in the wire shape, with conditional validity checked by the executing method.
5. A seed is either an exact nonnegative integer representable as u64, or a finite nonnegative integer-valued float strictly below 2^64. Fractional, negative and out-of-range float seeds error. Seed mutation is available only for normal, log-normal, mean-reverting and bootstrap parameters; deterministic methods do not acquire a seed.
6. Typed bounds access returns the selected variant's min/max unchanged. It does not clamp, reorder or validate them here.

Convenience constructors serialize their inputs into the parameter map and select the corresponding method. They do not run a forecast or make method-specific conditions true.

## Limitations and alternatives

Structural acceptance does not prove nonnegative standard deviation, valid smoothing/reversion/correlation ranges, sufficient history, positive bootstrap-growth data, compatible min/max, valid period override keys, or a legal conditional parameter combination. These are evaluated later. Correlation controls are present on normal/log-normal/mean-reverting variants for Monte Carlo use; their presence does not mean standalone projection applies correlation.

Moving all domain checks into construction could reject a model earlier but would need one canonical method validator to avoid divergent schema/build/evaluation rules. The existing shape/domain split supports persisted configuration, with the risk that an unused or hidden branch is structurally valid but economically invalid.

## Evidence

[Forecast parameter types](../../../../finstack-quant/statements/src/types/forecast_params.rs), [ForecastSpec conversion](../../../../finstack-quant/statements/src/types/node.rs), [forecast regression tests](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs), and [model regression tests](../../../../finstack-quant/statements/tests/finance_model_regressions.rs) provide contract and selected rejection evidence. The model regression for invalid forecast parameters confirms that shape validation occurs even when actual values win precedence. No forecast simulation or seed reproducibility test was executed here.

Successful parameter decoding does not certify forecast-domain validity, history sufficiency or correlation behavior.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
