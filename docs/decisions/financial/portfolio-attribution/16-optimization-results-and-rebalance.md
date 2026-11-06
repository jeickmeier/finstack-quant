# PORTFOLIO-ATTRIBUTION-016: Optimization reconstruction, diagnostics and portfolio rebalance

- ID: PORTFOLIO-ATTRIBUTION-016
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and reconstruction

A successful solver result is translated from continuous weights back into position quantities, trade suggestions and scalar diagnostics. It is not executed in a trading system and does not round to lots. Only `OptimizationStatus::Optimal` is treated as feasible; the result object can be deserialized, so downstream validation remains material.

For held existing positions retain exact current raw quantity. Otherwise ValueWeight reconstructs scale as `w*existing_gross_PV/PV_per_scale`; NotionalWeight uses `w*existing_gross_notional/abs_deal_notional`; then `PositionUnit::quantity_of` restores raw quantity units. If the relevant per-scale denominator is at most `1e-12` in magnitude (or notional not greater than that), quantity is zero. UnitScaling multiplies current raw quantity by weight for existing positions and uses weight directly as candidate raw quantity.

## Reporting and application algorithm

1. Read solved weights, subtract current weights and compute objective with compensated accumulation. Total turnover is `sum |weight_delta|`, without halving or currency conversion.
2. Report slacks only for named LP rows, plus turnover when present. Inequality slack is signed distance in the requested direction; equality slack is absolute residual. Average bounds are recomputed in metric units, with NaN when the denominator is near zero. Duplicate labels overwrite earlier entries in the ordered map; variable bounds are not independently reported as slack rows.
3. Mark a named constraint binding when absolute slack is below `1e-6`, irrespective of its economic units. NaN is not binding. The binding accessor returns NaN for a named entry absent from the slack map.
4. Build trades only when finite absolute weight change is at least `1e-9`. Classify candidate nonzero weights as new positions and existing near-zero weights as closeouts. Buy/Sell direction follows raw quantity change, and sorting is decreasing absolute quantity change, not cash trade size.
5. `rebalance` requires Optimal. Update existing quantities from the result map, add candidates only when both absolute target weight and quantity are at least `1e-9`, create their entities if needed, and validate the resulting portfolio. Existing zero-quantity positions remain in the portfolio. `rebalance_from_spec` additionally rejects result IDs absent from the supplied current/candidate universe before delegating.
6. Failure results use the core explicit nonfinite wire contract for NaN objective/turnover and slack entries. A runtime serialization module after the test module applies that contract to every slack value.

## Alternatives and limitations

Trade-lot integer optimization, costs, cash settlement and a second revaluation of the reconstructed portfolio would be separate layers. The current reporting tolerances are fixed and shared across heterogeneous units. A skipped tiny-weight trade can still correspond to material quantity/currency for unusual unit scales; the rebalance result map and displayed trade list are different products.

## Evidence

[Result/trade/rebalance owner](../../../../finstack-quant/portfolio/src/optimization/result.rs), [reconstruction and diagnostics](../../../../finstack-quant/portfolio/src/optimization/lp_solver.rs), [spec ID validation](../../../../finstack-quant/portfolio/src/optimization/spec.rs), [integration fixes](../../../../finstack-quant/portfolio/tests/test_optimization_fixes.rs) give source and regression evidence; no solver or portfolio test ran.

No provenance tie currently proves a deserialized Optimal result belongs to the exact market/spec being rebalanced.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
