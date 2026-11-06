# MARGIN-REGULATORY-XVA-020: Unilateral and bilateral CVA/DVA bucket quadrature

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

CVA and DVA are discounted loss integrals over the supplied expected positive/negative exposure profiles. The bilateral branch weights a default leg by the other name's average survival, an independence-style first-to-default approximation; it does not model joint default or wrong-way exposure dependence.

## Algorithm and conventions

For node i let an overbar be the arithmetic average of previous/current node values, with initial DF/survival one and initial exposure held from the first node. Validate recovery in `[0,1]` and set LGD=1−recovery.

1. CVA increment is `LGD_cp×avg(EPE)×avg(DF)×max(S_cp_previous−S_cp_current,0)×avg(S_own)`. Unilateral CVA uses the last factor one.
2. DVA increment mirrors it: `LGD_own×avg(effective_ENE)×avg(DF)×max(S_own_previous−S_own_current,0)×avg(S_cp)`. Standalone DVA uses the last factor one and no posted IM.
3. In bilateral calculation with an IM profile, linearly interpolate posted IM to each exposure node with flat extrapolation before/after the IM grid; use `effective_ENE=max(ENE−IM,0)` at each node **before** averaging. The opening ENE is reduced by IM evaluated at zero. EPE/CVA remain unchanged.
4. Sum contributions; CVA is positive loss and DVA positive own-default benefit under ordinary nonnegative curve factors. Bilateral rows retain time, averaged exposure, DF, marginal default probability, other-name survival and contribution for replay. Public scalar DVA drops those rows.

## Selection and controls

Standalone CVA accepts a counterparty hazard curve/recovery and returns an XVA result with other legs absent. Standalone DVA accepts own hazard/recovery and returns only the amount. Bilateral routing uses both hazards in each credit leg; no survival grid is built independently of exposure nodes. Evaluations use raw year fractions and fail on nonfinite curve values.

## Limitations and failure behavior

Negative survival increments are clamped to zero rather than rejected here; this observation does not imply that malformed curves bypass their constructors. A product of arithmetic averages is not exact integration of the product and has no local error estimate. Flat back-extrapolation of first exposure and posted IM affects coarse opening intervals. Posted-IM subtraction is observed behavior, not adoption of a collateral-availability or legal close-out convention. No joint-default/wrong-way calibration is performed, and row replay only reproduces this approximation.

## Alternatives and tradeoffs

Exact integration under piecewise-constant hazards/exposure or adaptive quadrature can reduce deterministic integration error. Pathwise joint exposure/default simulation addresses a different dependence problem at much higher cost. Treating IM as a funding-only input would change DVA; that economic choice should be explicit.

## Evidence and role

Runtime: [src/xva/cva.rs](../../../../finstack-quant/margin/src/xva/cva.rs); row/result contracts: [src/xva/types.rs](../../../../finstack-quant/margin/src/xva/types.rs). Inspected flat-hazard CVA/DVA tests compare against `LGD×E×lambda/(lambda+r)×(1−exp(−(lambda+r)T))` with quarterly grids and 2% tolerance: independent closed-form shape evidence, not an executed result. Bilateral tests inspected only establish lower/equal survival-weighted adjustments; row replay and posted-IM examples were located. [Gregory XVA](../../../REFERENCES.md#gregory-xva-challenge), [Green XVA](../../../REFERENCES.md#green-xva).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
