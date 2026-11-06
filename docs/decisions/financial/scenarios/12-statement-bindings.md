# SCENARIOS-012: Forecast mutation and economic rate-binding conversion

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: adjust only stored values belonging to periods marked non-actual, preserving actual historical values and money currencies. Rate bindings calculate one dimensionless scalar from a curve and assign it to every existing forecast entry in the selected node. They do not derive period-specific forward rates for each forecast period.

## Forecast algorithm and outcome

Collect non-actual period IDs from the model. Look up the exact node. If `values=None`, return `false`; otherwise visit stored values whose period ID is in that set. Percentage adjustment multiplies by `1+pct/100`; scalar values use f64 multiplication and money values use checked money multiplication. Assignment preserves scalar versus amount representation and the amount's currency. No missing forecast cells are created, and formula/forecast-method definitions are not rewritten.

**Observed report discrepancy:** `Some(values)` returns `true` even for an empty map, an actual-only map, or a map with no matching forecast IDs. Those calls can count as applied and trigger reevaluation without changing a forecast value. A money error can occur after earlier cells have been changed; the engine records a warning and continues. Finite spec inputs do not themselves prevent overflow of unchecked scalar multiplication.

## Rate-binding algorithm and conventions

Reject a target node declared monetary or containing any monetary value, including an actual-period amount. Prefer a discount curve with the exact ID; otherwise try a forward curve. The same scalar-rate node may carry previous values, but the binding does not require them to be within a rate range.

For a discount curve, convert the binding tenor from the **curve base date**, ModifiedFollowing and optional context calendar, on the curve's day count. Reject times beyond the last visible knot by more than `1e-8`. Read continuous zero rate `z` at native time `t`. Recompute the same tenor's year fraction `u` using the requested output day count (default curve day count). Convert continuous rate `z*t/u` to requested compounding over `u`, preserving `exp(z*t)` accumulation.

For a forward curve, the binding tenor identifies the forward **start** from curve base, with the same adjustment. Read the simple forward `F` at that start. Reconstruct an accrual tenor from `curve.tenor()` on its native day count, advance from the start, and calculate native accrual `τ` and output accrual `u`. Require finite positive `τ` and `1+F*τ>0`. Convert the continuous equivalent `log(1+F*τ)/u` to requested simple/periodic/continuous compounding over `u`. The output conversion requires positive finite year fraction and finite positive accumulation and resulting rate.

The tenor boundary check concerns the queried start for forwards, not a separate end-of-curve forecast horizon. Without a calendar, the underlying tenor helper does not add holiday awareness. Day count and compounding come from `RateBindingSpec`; omitted compounding selects **Continuous** through `Compounding::default()`.

## Selection and evidence

Existing context bindings run after market shocks; inline bindings and percent/assignment operations then run in their original order. Binding failures become warnings; absence of any statement model is a hard error. `reevaluate_model` invokes a new default `Evaluator`, returns its warnings and discards calculated results. The model remains the shocked input specification, not a materialized output statement.

Owning source: [adapters/statements.rs](../../../../finstack-quant/scenarios/src/adapters/statements.rs), [engine ordering](../../../../finstack-quant/scenarios/src/engine/mod.rs). Inspected unit tests preserve actual values and compare discount/forward conversions to accumulation formulas, including Act365F-to-Act360 output. [Statement integration tests](../../../../finstack-quant/scenarios/tests/integration/statement_test.rs) locate monetary-node rejection and nonmutating invalid-rate-conversion assertions. These are conversion/contract checks; they do not validate an accounting forecast policy.

Period-specific binding, creating forecast cells, changing forecast assumptions rather than values, and explicit result return are alternatives.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
