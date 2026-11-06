# MARGIN-REGULATORY-XVA-011: Loss-positive curvature, squared correlations and directional selection

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

The engine aggregates caller-computed up/down curvature losses; it does not reprice instruments under shocks or remove their delta component. Negative CVR is a gain. Correlation parameters are squared **before** scenario transformation in the actual runtime.

## Algorithm and conventions

1. Define `psi(x,y)=0` only when both x and y are strictly negative; otherwise one. For each shock direction compute `K_side=sqrt(max(Σ max(CVR_i,0)² + Σ_{i≠j}rho_curv CVR_i CVR_j psi(CVR_i,CVR_j),0))` and `S_side=Σ CVR_i`.
2. Choose the side with larger K; on a tie choose the larger raw S; an exact K/S tie selects the down branch. Preserve the chosen **uncapped** signed S.
3. For ordinary name/bucket classes (CSR/equity/commodity), use `rho_curv=scenario(name_rho²)`. Inter-bucket use `scenario(bucket_gamma²)×psi(S_b,S_c)` in the shared quadratic, with zero floor. Unlike delta/vega there is no negative-standard-form capped-S retry.
4. Other-sector buckets (CSR 16/25, equity 11) sum positive CVR separately for up/down and use `(max(sum_up,sum_down), same_positive_value)`. Non-CTP then adds other bucket 25 outside the root of ordinary bucket K².
5. GIRR has one CVR pair per currency, so no intra-currency pair correlation; use `scenario(0.50²)` between currencies. FX puts all pair CVRs into one implicit bucket using `scenario(0.60²)`, then the one-bucket result.
6. No risk weight is applied inside any curvature branch. Any FX curvature relief or choice of full shock belongs to the input construction.

## Selection and controls

The class map selects the grouping; finite signed CVR sides and supported buckets/identifiers are checked. Name keys net matching up/down amounts before the nonlinearity. All-negative one-side contributions have zero K, but raw negative S is preserved for ordinary buckets and psi controls their cross terms.

## Limitations and failure behavior

Finite overflow can interact with zero floors. Input shock size, delta subtraction and option coverage are caller-supplied; the aggregation API does not establish their regulatory suitability.

## Alternatives and tradeoffs

Repricing instruments to construct CVR would verify shock conventions but is outside this aggregation API. Always using max(loss,0) before cross terms or always capping S changes the present sign-aware formula. Preserving raw sides allows audit of asymmetric shocks.

## Evidence and role

Owning algorithm: [src/regulatory/frtb/curvature.rs](../../../../finstack-quant/margin/src/regulatory/frtb/curvature.rs); quadratic primitive: [src/regulatory/frtb/aggregation.rs](../../../../finstack-quant/margin/src/regulatory/frtb/aggregation.rs); correlation-scenario parameters: [src/regulatory/frtb/params/correlation_scenarios.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/correlation_scenarios.rs). Inspected [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) derives medium/high equity correlations from squared 0.15; [tests/frtb_sba_charges.rs](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) derives commodity and CSR curvature with squared correlations/raw S. Unit tests pin psi and raw-S behavior, serving branch-contract evidence. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
