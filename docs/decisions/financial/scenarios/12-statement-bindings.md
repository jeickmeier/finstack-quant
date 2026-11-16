# SCENARIOS-012: Forecast mutation and economic rate-binding conversion

- Decision status: **Accepted 2026-11-15 — complete typed forecast inputs, dated rate conversion, evaluation and atomic reporting package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent signed and Decimal forecast, rate conversion, operation-order and cutoff calculations verified.** Runtime mutation, evaluation, rollback and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked scenario forecast shocks and rate bindings

1. **Change existing forecast inputs.** Bind the exact node in the staged
   runnable model and visit its stored nonactual cells in canonical period
   order. A missing node or zero eligible cells fails, including for a
   zero shock. Preserve historical observations, missing cells, masks,
   availability dates, formulas and forecast methods. The Statements
   evaluator owns visibility, precedence and use of each changed input.
   [Statement timelines and values](../statements-contracts/01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness) ·
   [Evaluation precedence](../statements-evaluation/01-graph-precedence-and-prepared-plans.md#accepted-decision-for-dependency-graphs-cell-precedence-and-prepared-evaluation).

2. **Use checked typed assignment and scaling.** Assign a finite scalar
   or exact Decimal Money in the node’s currency, using the admitted
   Statements wire value. For finite percent `p`, scale signed values
   by `1+p/100` with checked arithmetic and model admission. An
   explicit `−100%` gives zero. Percentages below `−100%` reverse
   the sign of nonzero values. Admit the percent and form the Money
   multiplier in Decimal under the Money contract before checked scaling.
   Any invalid conversion, unit or calculation aborts the transaction.
   [Money arithmetic](../core-conventions/012-money-decimal-precision.md) ·
   [Statements wire contract](../statements-contracts/16-wire-schema-and-errors.md).

3. **Bind a typed source and resolved dates.** Elect an exact discount
   source or an exact index-to-projection source association. Require
   resolved start and end dates with start before end, explicit output
   day count and required context, and explicit Simple, Continuous or
   Periodic compounding. Each endpoint uses the elected source’s own
   clock and tail policy. Retire discount-first lookup, inferred
   calendar adjustment, numeric tenor reconstruction and the adapter
   last-knot tolerance. Assign one annual decimal curve-implied input
   to every eligible cell of the elected scalar node. Fixings,
   contractual coupons and period-specific projections have separate owners.
   [Discount source](../core-market/discount-evaluation.md) ·
   [Projection source](../core-market/forward-projection.md).

4. **Preserve accumulation on the elected quote basis.** Admit both
   positive finite endpoint factors and compute the stable log
   difference `L=log(factor_start)−log(factor_end)`. For positive
   output year fraction `u`, the annual decimal objectives are
   Continuous `L/u`, Simple `expm1(L)/u`, and Periodic(m)
   `m×expm1((L/u)/m)`. Reuse the checked core equivalents and
   enforce each returned rate’s domain and representation. Equal
   admitted source coordinates give zero when output accrual is
   positive. A zero or otherwise invalid output accrual fails.
   [Checked compounding](../core-numerics/compounding.md).

5. **Apply the ordered phase and evaluate once.** Apply configured
   bindings in request order against the final shocked market.
   Apply inline operations in original relative order. Inline
   bindings are local to the run. Evaluate every active phase once,
   including valid identities, using the explicit full snapshot or
   inclusive dated cutoff and applicable staged market inputs.
   Publish the candidate inputs, checked evaluation, provenance and
   completed check outcomes together. Required failures abort the
   full scenario.
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Statement information set](../statements-contracts/02-authoring-and-availability.md#accepted-decision-for-authoring-imports-and-observation-availability).

6. **Report actual input effects and evaluated results.** Retain the
   action, node, period and before-and-after typed values, plus exact
   source and quote basis for a rate binding. Distinguish valid
   identities from changes, configured outcomes from inline operations,
   and stored input effects from evaluated cells and their provenance.
   Use the same Rust validation and typed errors in Python and WASM.
   Record 017 owns report and replay representation.
   [Statements result contract](../statements-evaluation/13-result-units-and-exports.md) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- The signed forecast `−80` changes to `+40` at `−150%`, to
  zero at `−100%`, and to `−100` at `+25%`. The Decimal
  factor for a `+1e−15%` shock to `1e20` adds 1000 currency
  units. Preforming that factor in binary64 rounds it to one.
- Endpoint factors `1` and `.98` over ACT/360 `181/360`
  imply accumulation `1/.98`. Simple gives `4.059082%`,
  Continuous gives `4.018218%`, and Periodic(2) gives
  `4.058855%` to six decimal places in percent.
- Starting with a configured `4%`, `+25%` followed by
  assignment to `6%` ends at `6%`. Reversing the inline
  operations gives `7.5%`. An inclusive April 25 cutoff admits
  an April 25 observation and excludes a May 15 revision.

## Observed choice and rationale at the audit revision

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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#scenarios).
