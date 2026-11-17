# PORTFOLIO-ATTRIBUTION-014: Factor stress repricing and proportional position what-if

- ID: PORTFOLIO-ATTRIBUTION-014
- Decision status: Accepted 2026-11-16 for checked factor stress and position what-if.
- Implementation status: Observed for the stated owning source scope. Runtime, replay, selective reuse and host reconciliation with the accepted contract are pending.
- Validation status: Independent checks covered joint shock repricing, the comparison center, reporting-only FX, issuer re-netting and resize range. Production and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked factor stress and position what-if

1. Admit a simultaneous factor stress vector with unique declared factor IDs and finite signed moves in each factor's declared unit. Bind the typed targets and assignments once on the qualified opening state. Validate explicit zero moves and reject duplicate IDs. Compose overlapping moves only where the source owner defines a compatible operation. Intentionally sequential actions use the ordered scenario engine. [Factor routing](../portfolio-execution/12-factor-shocks-and-routing.md#accepted-decision-for-checked-portfolio-factor-shocks-and-routing).
2. Compute each affected credit quote's move as `Σ beta × factor move`. Use [record 012](12-factor-assignment-and-credit-exposures.md#accepted-decision-for-checked-factor-assignment-and-credit-exposures) for shared-source beta consistency. Fit each affected source once against the selected target market after combining compatible moves. Verify delivery and atomically rebind affected index views. Reject incompatible overlaps, including direct intensity and par-quote shocks on the same source. When rates also move, bind the target discount state and the credit owner's explicitly selected response. If the original opening mark differs from the qualified par-risk center, require an explicit center-based comparison and report the opening-to-center difference separately. [Hazard replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).
3. Compute P&L from qualified same-date full-product marks on fixed holdings and a common selected valuation setup. Qualify stochastic draw coupling where applicable. Convert each unscaled raw endpoint with its qualified cash FX. Compute `signed scale × (stressed PV − opening PV)` in checked arithmetic and sum in holding order. Reuse an endpoint only with proof that native and reporting dependencies are unchanged. A proven reporting-only move may reuse native PV and refresh FX. Reprice when the proof is incomplete. [Raw execution](../portfolio-execution/04-execution-order-and-reduction.md#accepted-decision-for-checked-portfolio-execution-and-nav-reduction) · [Selective reuse](../portfolio-execution/05-dependencies-and-selective-reuse.md#accepted-decision-for-checked-portfolio-dependencies-and-selective-repricing).
4. Share one P&L calculation across both stress entry points. Both require complete routing and pricing for the requested shock. Full stress additionally recomputes local sensitivities and any elected issuer residuals on the exact stressed state after revalidating coverage. Keep factor definitions, calibrated betas, covariance, issuer forecasts, horizon, risk measure and applicable confidence fixed. [Accepted residual basis](13-credit-residual-risk-and-reporting.md#accepted-decision-for-checked-issuer-residual-risk-and-hierarchy-reporting).
5. Before pricing, admit at most one Remove or absolute signed Resize per existing holding. Preserve its authored sizing unit and enforce the holding owner's range, including `[-100, 100]` for percentage points. A zero resize retains the holding and a removal drops it from the after-state axis. Allow zero-to-nonzero resize and sign reversal for qualified externally scalable holdings. New holdings and changes to the instrument's economics are outside this operation. [Holding and sizing contract](../portfolio-execution/01-position-sizing-and-validation.md#accepted-decision-for-checked-portfolio-holdings-and-signed-sizing).
6. Compute target exposures from qualified unscaled factor and issuer pricing evidence and the absolute target scale. Retire the target-to-opening quantity ratio and machine-epsilon cutoff. Reuse unscaled evidence only on the same complete basis. Obtain fresh evidence at the fixed market when needed. Rebuild after-state coverage and re-net issuer hedges before risk decomposition on the same forecast and measure.
7. Publish checked Rust results with requested and delivered shocks or edits, ordered axes, currency, valuation and forecast basis, and coverage. Require identical factor axes for what-if and compute absolute contribution and relative share changes as after minus before. Fail atomically on invalid input, pricing or results. Existing Python and WASM entry points consume the checked result and shared restore rules. [Scenario report and replay contract](../scenarios/17-reports-and-replay.md#accepted-decision-for-checked-scenario-reports-and-replay).

Independent examples gave joint P&L of `−3` for combined `+2` and `−1 bp` quote moves on a nonlinear price, versus `+1` for the sum of separate P&Ls. Removing the `+10` leg from a same-issuer `+10/−6` hedge raised residual variance from `64` to `144 USD²` in the example.

## Observed choice and scope

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
