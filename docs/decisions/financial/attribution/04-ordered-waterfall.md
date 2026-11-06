# ATTRIBUTION-004: Ordered waterfall revaluation and interaction allocation

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

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
