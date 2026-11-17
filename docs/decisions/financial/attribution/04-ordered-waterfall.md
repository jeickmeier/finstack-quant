# ATTRIBUTION-004: Ordered waterfall revaluation and interaction allocation

- Decision status: Accepted 2026-11-16 for checked ordered Waterfall revaluation.
- Implementation status: Observed for the source scope listed below. Runtime, pricing, cash coverage, credit detail and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked order dependence, offsetting effects, price and cash closure, reporting FX and nested credit detail. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked ordered Waterfall revaluation

1. Require an explicit, nonempty and unique factor order beginning with Carry. Execute the supplied order serially. Every applicable changed factor is required for a complete Waterfall. A proven unchanged or irrelevant factor may be omitted or shown as zero with its reason. Equal marks alone do not establish irrelevance or completeness. Each interaction belongs to the stage that realizes it.
2. Start from the qualified opening mark and apply the held-market Carry step under [record 003](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding). At the resolved closing cut, apply each later stage's exact typed source changes to the running state and price that state through the selected evaluator. ModelParameters changes only qualified parameters of the same instrument. Retain the actual after-mark for the next step. A required pricing or parameter failure aborts the run. [Accepted restoration](02-market-factor-restoration.md#accepted-decision-for-checked-economic-factor-families-and-market-restoration).
3. Use checked Money for the price and cash changes of each stage. Carry contributes `V_H − V0 + C_held`. Later stages contribute `V_after − V_before` plus any qualified change in eligible cash on that stage's path. Retain the price-only increment and the cash change. Allocate `C_close − C_held` once to supported stages. An unsupported required cash difference fails complete attribution.
4. Verify terminal economic bindings, final mark and eligible cash independently against the qualified closing state. Then verify the stage totals against headline P&L. Residual tolerance cannot qualify a missing change or failed stage. [Record 013](13-target-currency-and-scaling.md) owns reporting translation, rounding and residual tolerances.
5. Price both sides of a native pricing FX stage at the closing cut under their respective staged markets. Retain the native audit trail. Apply reporting translation through record 013 using dated endpoint rates and the opening-value FX effect. Label any translated step view as closing-equivalent. [Accepted composable reporting FX](13-target-currency-and-scaling.md#accepted-portfolio-requirement-for-composable-reporting-fx).
6. Keep one authoritative CreditCurves parent step using exact closing credit restoration. An elected cascade supplies nested quote replays from its fixed pre-credit state. Reconcile detail to the same parent endpoint and price change. Show any par-risk center bridge and final difference to the actual closing credit state explicitly. [Record 011](11-credit-factor-hierarchy.md) owns hierarchy eligibility and quote moves. Required detail failure aborts. Explicit advisory detail may be unavailable with a typed cause. [Accepted quote replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).
7. Publish the checked Rust audit trail with applied order, source and state identities, cuts, actual before-and-after marks, cash allocations, actual pricing work and skip reasons. Validate continuity and reconciliation on restore. Python and WASM consume the checked result. Source-backed replay verifies the underlying prices. [Accepted execution contract](01-execution-and-endpoints.md#accepted-decision-for-checked-instrument-attribution-execution-and-endpoints).

Independent exact arithmetic checked `V(x,y) = 100 + 2x + 3y + 4xy`. Moving both inputs from zero to one allocates `2` then `7` in one order and `3` then `6` in reverse. Total P&L is `9` in either order. A separate mark-and-cash example reconciled `1` of Carry and a later `2` effect, including `1` of cash realization, to headline P&L of `3`.

## Observed choice and domain

Waterfall moves a running opening instrument/market state to closing state at the closing date, assigning each marginal price change to the factor that is applied. Interactions therefore depend on order. The explicit list must be nonempty, begin with Carry, and contain no duplicate factor; it need not contain every factor.

## Algorithm and selection

Default order is Carry → RatesCurves → CreditCurves → InflationCurves → Correlations → Fx → Volatility → ModelParameters → MarketScalars. Execution is always serial. Begin with opening parameters/market/PV. Carry changes only the date and uses the shared gross total-return cash partition. Each market factor restores its complete closing family and reprices the running instrument. ModelParameters swaps to the closing instrument while retaining the running market. Keep `pv_before`, `pv_after` and marginal `step_pnl` for every requested step; running PV updates via checked Money addition.

If a credit factor model can plan a cascade, replace the ordinary credit step with cumulative par-spread moves from the same fixed pre-credit market, repricing successive endpoints and telescoping differences; the last curve-shape step snaps issuer hazards to closing curves. Otherwise retain ordinary hazard-family restoration. Carry cash is outside the price-only waterfall-step PV difference. Factors declared unused produce zero without a reprice.

## Conventions, controls and failure

Ordinary differences convert both values at closing FX; the FX step uses pre/post hybrid FX contexts. Normal same-currency instrument prices make those conversions identities. The default residual tolerances are 0.01 currency units and 0.001 percent, later overrideable. An omitted factor remains in residual. The `strict_validation=false` branch only softens closing model-parameter repricing failure to zero with a tracing warning; ordinary factor and credit-cascade pricing failures propagate. It does not make all failures soft or necessarily add a result note.

## Rationale and alternatives

Ordered marginal allocation gives an auditable price bridge, but neither a small residual nor telescoping proves that the order is economically unique. Parallel attribution and a different user-supplied waterfall order are supported alternatives; a permutation-averaged/Shapley allocation is not implemented.

Located tests enforce default/order behavior and checked decimal running-value arithmetic at large notional; they establish structural identities rather than independent interaction-allocation accuracy.

## Source and evidence

- [waterfall.rs](../../../../finstack-quant/attribution/src/waterfall.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
