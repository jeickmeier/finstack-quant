# PORTFOLIO-ATTRIBUTION-014: Factor stress repricing and proportional position what-if

- ID: PORTFOLIO-ATTRIBUTION-014
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

Two scenario mechanisms are distinct: factor stress builds a bumped market and fully reprices affected positions; position what-if rescales an existing sensitivity matrix at a fixed market/covariance. Neither refits factor covariance or credit calibration parameters statistically from new observations.

## Factor stress algorithm

1. Reject unknown factor IDs. Collect supplied shifts in a map, so duplicate factor entries retain the last supplied shift. Iterate configured factors in definition order, applying market bumps sequentially; ordering matters when mappings overlap or are nonlinear.
2. Static mappings use the factor type's bump units and portfolio mapping adapter. Assignment-driven credit shocks locate matched curves, select each curve's beta and rebootstrap its spread quotes by `beta*shift` basis points. The lookup retains the first matching entry for a factor on a dependency and the first beta encountered per curve across positions.
3. Exact changed-factor keys are available only with a complete portfolio dependency index and mappings with exact key resolution. Otherwise reprice the whole portfolio. Any FX key also forces full repricing, since reporting-currency translation can move positions that declare no native FX dependency.
4. Evaluate base raw endpoints, then stressed endpoints, reusing the same position-indexed base vector only for proven unaffected positions. P&L per position is `(stressed_endpoint-base_endpoint)*scale_factor`, in portfolio base currency; aggregate with compensated summation. The raw executor's seed is a caller consistency contract, not independent provenance validation.
5. `factor_stress_pnl` stops at repriced P&L and does not run the full baseline assignment-completeness pass used by analysis. Credit stress matching ignores dependencies without the requested factor; matcher errors still propagate. `factor_stress` additionally recomputes sensitivities and decomposition at the stressed market using the **same** factor covariance/risk measure.

## Position what-if algorithm

Analyze once, retain the sensitivity matrix and create scenario positions. Reject repeated/unknown position changes and nonfinite new quantities. Remove zeros that sensitivity row and removes the position. Resize rejects absolute original quantity below machine epsilon and multiplies the row by `new_quantity/original_quantity`, allowing sign reversal. Recompute decomposition from the rescaled matrix and original covariance; recompute issuer residual risk from the scenario positions so issuer hedges net correctly. Report factor contribution changes in original factor order; relative change is the difference in relative-risk shares, not percentage growth in risk.

## Discrepancies, alternatives and evidence

The sensitivity overlay applies each position's beta, while exact credit stress uses the first beta for a shared curve. If positions sharing a curve resolve to different betas, summed factor sensitivity and small-stress repricing need not agree. The code does not resolve that conflict in this adapter. A shared-curve consistency requirement or different market representation is an adoption/implementation question, not a claimed reproduced pricing error.

A full rebuild/reprice for quantity-only what-if would cost more but could support nonlinear position-dependent terms. Proportional sensitivity scaling assumes instrument value/sensitivity is linear in external position scale; no new positions or changed instrument economics are accepted.

[Stress/what-if source and inline tests](../../../../finstack-quant/portfolio/src/factor_model/whatif.rs), [market bump orchestration](../../../../finstack-quant/portfolio/src/factor_model/model.rs) and [sensitivity engine tests](../../../../finstack-quant/portfolio/tests/factor_model_engines.rs) provide implementation and invariant evidence, not executed independent validation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
