# STATEMENTS-EVALUATION-005 — Forecast segment anchoring, overrides and output bounds

Decision status: **Accepted for the full forecast segment, anchoring and bounds contract (2026-11-15)**. Implementation status: **Reconciled (2026-11-17)**. A node has one forecast suffix per evaluation run or simulation path: the model's forecast periods, generated once and advanced one step per slot whether or not the slot is masked or publishes an explicit value. A method that steps from a base uses the `start` level of its specification, else the visible published value of the slot immediately before the suffix; trend, seasonal and keyed-override methods use their own inputs. A forecast without a base is unavailable with reason `missing_forecast_base`, and an actual hidden by the cutoff is unavailable (`hidden_by_cutoff`) and is never forecast or used as a base. Bounds clip published outputs only; the raw series stays the state for later steps, and each clipped cell is recorded as a `forecast_clipped` warning carrying the raw and published values. Validation status: **Qualified** by an explicit value mid-suffix that leaves the growth recurrence, the seasonal phase and the stochastic draws unchanged, a start level and a preceding-slot base, a missing base giving a typed unavailable forecast, and clipping that does not feed the next step. The sections after the accepted decision describe the implementation audited before reconciliation. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Accepted decision for forecast segments, anchoring and output bounds

1. Use one canonical forecast suffix per node in each evaluation run or simulation path. Advance one forecast step per slot, including masked slots and slots where explicit values are published. [Cell precedence and forecast eligibility](01-graph-precedence-and-prepared-plans.md#accepted-decision-for-dependency-graphs-cell-precedence-and-prepared-evaluation).
2. For a method that needs a base, use its selected explicit start level at the opening of the suffix. When no start level is selected, use the value in the immediately preceding slot if it is eligible, visible and unmasked. Methods that use supplied history use their own aligned history. Missing required inputs produce a typed unavailable forecast with a reason. [Forecast parameters and validation](../statements-contracts/03-forecast-parameter-contract.md#accepted-decision-for-typed-forecasts-and-validation-staging).
3. Publish visible explicit values according to cell precedence. Keep raw forecast recurrence, phase and shocks independent of those published cells. The `Override` forecast method maintains its own period-keyed carry within its raw series. [Deterministic forecast methods](06-deterministic-forecasts-and-fades.md).
4. Apply compatible finite ordered bounds only to published forecast outputs and record any clipping. Preserve typed unavailability and computation errors. The raw generated series remains the basis for subsequent forecast state. [Forecast units](../statements-contracts/05-currency-dimensions.md#accepted-decision-for-formula-units-and-currency).

## Audited scope and rationale

A forecast generates a complete run from a prior anchor, caches it by node/period and lets explicit cells override selected outputs. This preserves elapsed forecast steps, random draws and seasonal phase across overrides. It is not a recurrence restarted from each explicitly supplied forecast value. The rule applies to all method-specific kernels in records 06–10; even a time-series method that ignores its supplied base is reached through this anchor requirement.

## Audited algorithm and controls

1. Return an existing cached forecast cell when available. Otherwise find the current model index and the latest preceding visible actual anchor. If none exists, find the first preceding permitted explicit value that can anchor the run.
2. Define the run from just after the anchor until the next visible actual period. Explicit forecast values inside the run do not split it. Resolve the base strictly before the first projected period: prefer a visible actual's explicit value, otherwise its evaluated history, then the latest earlier historical value. Missing usable base errors.
3. Parse the typed method parameters and generate the entire run in model-period order. In Monte Carlo, optionally construct correlated shocks and cache latent shocks for the peer mechanism. Cache the full generated series even when explicit cells will ultimately win for part of it.
4. Apply optional minimum and maximum to **finite generated outputs**. Bounds must be finite and ordered. Nonfinite generated values remain nonfinite for diagnostic handling; bounds do not repair them. Clipped outputs are not fed back into the stochastic/deterministic recurrence. Latent shocks remain available before clipping and explicit replacement.
5. The evaluator's overall precedence selects visible explicit value before forecast, as described in [record 01](01-graph-precedence-and-prepared-plans.md). Hidden actual cells under an as-of cutoff can be projected; availability, not merely the stored actual flag, controls eligibility.

## Selection and conventions

The node's explicit forecast specification selects the kernel; there is no fitted automatic model selection. Model entries count forecast steps, regardless of duration. Ordinary single evaluation validates correlation parameters and emits tracing guidance but generates independent series; correlation is applied only in Monte Carlo. Forecast warnings and raw-series errors have different channels: kernel errors propagate, while some advisories are tracing messages rather than result-table warnings.

## Limitations and alternatives

The audited kernels generate their latent trajectory before explicit-cell selection and output clipping. The accepted contract requires separate raw state and published forecast outputs, with recorded clipping.

The audited dispatcher requires a base even when a method uses its own supplied history. The accepted contract makes the requirement method-specific. Correlation ordering and peer configurations are tracked in [record 08](08-correlated-shocks.md).

## Evidence

[Forecast integration](../../../../finstack-quant/statements/src/evaluator/forecast_eval.rs), [method dispatch and bounds](../../../../finstack-quant/statements/src/forecast/mod.rs), [precedence](../../../../finstack-quant/statements/src/evaluator/precedence.rs). Inspected [production reporting regression](../../../../finstack-quant/statements/tests/production_reporting_audit.rs) replaces the first forecast with 999 and verifies later growth/seasonal cells equal the baseline. Inspected [forecast regressions](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs) verify peer clipping and explicit replacement preserve follower latent paths. These are strong behavior characterizations, not independent forecasting validation. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
