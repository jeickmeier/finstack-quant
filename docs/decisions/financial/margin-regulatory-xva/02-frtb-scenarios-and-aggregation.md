# MARGIN-REGULATORY-XVA-002: FRTB correlation scenarios, quadratic aggregation and binding-total selection

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

The engine adds delta, vega and curvature charges across the selected risk classes within each selected correlation scenario, selects one largest scenario total, then adds DRC and RRAO. It does not maximize each component independently and does not diversify across risk classes.

## Algorithm and conventions

1. Scenario transformation of a base correlation `r`: medium `r`; low `max(2r−1, 0.75r)`; high `min(1.25r,1)`. Curvature callers supply squared base correlations before this transformation (record 11).
2. Within an ordinary delta/vega bucket, set `K=sqrt(max(Σ_i w_i²+Σ_{i≠j}rho_ij w_i w_j,0))` and preserve `S=Σ_i w_i` uncapped. The diagonal is always one. The implementation loops over both off-diagonal orders.
3. Across ordinary delta/vega buckets, first calculate `Q=Σ_b K_b²+Σ_{b≠c}gamma_bc S_b S_c`. If `Q>=0`, return `sqrt(Q)` using raw S. Otherwise replace each S with `clamp(S,−K,K)` and recompute, finally flooring the quadratic at zero. This fallback is conditional, not an always-applied cap.
4. Non-CTP CSR instead returns `sqrt(Σ ordinary K_b²)+Σ undiversified K_b`; the other bucket is added outside the root. Curvature uses its own sign-aware quadratic and zero floor, without the ordinary delta/vega capped-S retry.
5. Sum positive class charges into each scenario total. Only strict `total>best` replaces the binding scenario, so ties select the first configured scenario. Defaults are `[Low, Medium, High]`: an empty or tied default book reports Low. Store that scenario's component maps and a breakdown for every evaluated scenario. Zero charges are omitted.
6. `total=max(best_SBA,0)+DRC+RRAO`. DRC/RRAO run regardless of the selected SBA risk-class subset. `frtb_sba_charge(..., Some(scenario))` selects one scenario; `None` selects all.

## Selection and controls

Default construction selects all seven risk classes and all three scenarios. Explicit construction rejects empty lists but neither deduplicates nor rejects duplicates; per-class maps and per-scenario maps overwrite repeated entries, avoiding arithmetic double counting while repeating work. All input fields are validated even if their SBA risk class is excluded. Scenario-list order remains relevant to tie labels.

## Limitations and failure behavior

Parameter and aggregation floors can suppress negative quadratic forms; finite input validation does not prove finite intermediate products or totals. In particular, large finite sensitivities can overflow in squared terms; `max(0)` and positive-charge filtering do not establish a safe nonfinite-intermediate policy. Hash-map traversal remains in several reductions, so serialization determinism alone is not a numerical reproducibility guarantee.

## Alternatives and tradeoffs

Always capping S would change valid nonnegative standard forms. Taking componentwise scenario maxima would be more conservative in some books but is a different algorithm. Canonical sorted reduction or compensated sums would improve reproducibility at a cost; no such change is adopted here.

## Evidence and role

Owning formulas: [src/regulatory/frtb/aggregation.rs](../../../../finstack-quant/margin/src/regulatory/frtb/aggregation.rs), [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs), [src/regulatory/frtb/types.rs](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs). [tests/frtb_sba_charges.rs](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) includes hand-written commodity scenario totals and cross-class addition; [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates configuration/default and scenario-breakdown reconciliation tests. The latter replays reported components and is not an independent regulatory benchmark. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements) is provenance background.

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
