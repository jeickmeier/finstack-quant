# PORTFOLIO-ATTRIBUTION-016: Optimization reconstruction, diagnostics and portfolio rebalance

- ID: PORTFOLIO-ATTRIBUTION-016
- Decision status: Accepted 2026-11-16 for checked optimization results and rebalance previews.
- Implementation status: Observed for the stated owning source scope. Solver, construction, replay and host reconciliation with the accepted contract are pending.
- Validation status: Independent arithmetic covered signed reconstruction, small-weight trades, original-unit slacks, rounded quantity deltas and stale-basis application. Production and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked optimization results and rebalance

1. Read and require the backend's actual Optimal status before inspecting solution values. Publish solution quantities, objective and trades only after independent reconstruction and verification pass. Stopped, infeasible, unbounded and failed outcomes carry typed diagnostics with solution fields absent. [Accepted LP contract](11-linear-program-formulation.md#accepted-decision-for-checked-linear-optimization).
2. Reconstruct absolute quantities on the frozen solve basis. For ValueWeight use `w × G / p` with signed PV per unit scale `p`. For NotionalWeight use `w × G / n` with qualified positive notional per unit scale `n`. TargetScale supplies scale directly. Convert once to the authored quantity unit. Preserve exact held quantities, including held zero-PV positions. Reject unrepresentable quantities and hard-bound breaches. Recompute the objective, net, gross and full L1 turnover on the actual admitted quantities and original reference. TargetScale needs a qualified common monetary map for aggregate exposure and turnover. [Accepted decision coordinates](10-optimization-decision-space.md#accepted-decision-for-checked-optimization-decision-space).
3. Report every original check in its own units with a stable ordered ID, including unnamed constraints, position bounds, holds and average-denominator controls. Display labels may repeat. Report upper-bound slack as `rhs − lhs`, lower-bound slack as `lhs − rhs` and signed equality residual as `lhs − rhs`. Inequality binding is exact by default and any elected binding band is explicit in row units. Identify any violation admitted by an authorized feasibility tolerance. Use the original average and denominator checks.
4. Derive one suggestion for every nonzero quantity change in existing-holding order followed by declared candidate order. Buy or Sell follows the sign of `target_quantity − opening_quantity`, including a sign reversal. A nonzero candidate opens and an existing nonzero holding closes at exact zero. Retire weight and quantity thresholds. Use admitted absolute targets for application.
5. Bind application to the exact verified solve, including ordered opening holdings and candidates, objective and constraints, decision coordinate and reference, market snapshot, valuation date, effective settings, FX and producing build. Restore reports for inspection through the checked result contract. Applying a restored result requires trusted verification evidence or a replay on the exact historical inputs that verifies the same target quantities. Decoded status and matching IDs alone are insufficient. [Auditable results and replay](../../project/0025-auditable-results-and-replay.md#decision).
6. Construct and publish the target portfolio atomically through canonical holding admission. Retain existing zero-sized closeouts and add exactly the nonzero candidates with their admitted ownership and instrument definitions. Rebuild indices and evaluation identity. Verify the constructed holdings on the frozen basis using qualified coefficients or exact-basis pricing when needed. The result is a continuous allocation preview. Lot rounding, transaction costs, cash funding and settlement require their own modeled terms. [Accepted holding admission](../portfolio-execution/01-position-sizing-and-validation.md#accepted-decision-for-checked-portfolio-holdings-and-signed-sizing).
7. Use one bounded checked Rust result and restoration contract. Verify ordered identities and the correspondence among absolute quantities, trades, diagnostics and outcome. Python and WASM use the same admission and application checks.

Independent arithmetic gave `0.3 years` of slack for a `3.2-year` achieved duration against a `3.5-year` cap. A weight change of `5 × 10⁻¹⁰` on a USD `10 trillion` reference represents USD `5,000`. Applying a rounded binary64 quantity delta to an opening quantity of `2⁵³` can produce zero when the verified absolute target is `−1`.

## Observed choice and reconstruction

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
