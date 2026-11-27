# MARGIN-REGULATORY-XVA-002: FRTB correlation scenarios, quadratic aggregation and binding-total selection

- Decision status: **Accepted 2026-11-17 for checked FRTB scenarios, aggregation and result scope.**
- Implementation status: Existing behavior is described below. Full-capital and diagnostic result separation, checked reductions and caller reconciliation remain pending.
- Validation status: Source and primary references inspected. Independent examples checked scenario scaling and order, the capped retry and its failure case, non-CTP addition, exact ties and scaled norms. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked FRTB scenarios and aggregation

1. **Give full capital and SBA diagnostics distinct results.** A full run
   evaluates Low, Medium and High across all seven risk classes on the elected
   perimeter and requires complete applicable coverage. A diagnostic takes
   explicit, nonempty, duplicate-free scenario and class selections and reports
   only the selected SBA charges. Validate every supplied input under record
   001. Publish full capital only when the required SBA, DRC and RRAO inputs
   are qualified. [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance).
2. **Apply each scenario to the prescribed correlation.** For base correlation
   `r`, use `max(2r − 1, 0.75r)` for Low, `r` for Medium and `min(1.25r, 1)`
   for High. Keep the unit diagonal. For curvature, square the applicable
   delta correlation before the scenario transformation. Validate base and
   transformed coefficients.
   [Basel MAR21.6, MAR21.100 and MAR21.101](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Cap ordinary delta and vega bucket sums only on retry.** Within each
   bucket, calculate `K_b` from the prescribed weighted-sensitivity quadratic
   with its zero floor and retain the raw signed sum `S_b`. Across buckets,
   start with `Q = Σ K_b² + Σ_{b≠c} γ_bc S_b S_c`. Use `√Q` when `Q ≥ 0`.
   For `Q < 0`, cap each `S_b` to `[−K_b, K_b]` and retry once. A negative retry
   is a calculation error. With `K = (5, 5)`, `S = (7, −7)` and `γ = 1`,
   the raw quadratic is `−48` and the retry gives zero.
   [Basel MAR21.4](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
4. **Use the specialised non-CTP and curvature rules.** For non-CTP
   securitisation credit spread risk, combine ordinary buckets as `√Σ K_b²`
   and add the designated other-sector charge outside the root. Curvature
   uses its own signed aggregation and prescribed zero floors. The ordinary
   capped-`S` retry applies to delta and vega.
   [Basel MAR21.5 and MAR21.69–71](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [Curvature record](11-frtb-curvature.md).
5. **Choose one binding scenario from the complete scenario totals.** Sum
   delta, vega and curvature across the included classes within each scenario.
   Select the largest unrounded total. Break exact ties in Low, Medium, High
   order. A fully evaluated zero book binds at Low. Add DRC and RRAO once to
   the binding SBA charge for full capital.
   [Basel MAR21.7](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [Basel MAR20](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/20/inforce/2023-01-01/published/2020-03-27).
6. **Make the arithmetic and result evidence reproducible.** Use canonical
   factor and bucket order, scaled quadratics and checked compensated sums.
   Report arithmetic failures before applying financial floors. Preserve
   every evaluated scenario total, its component charges, explicit zeros,
   inapplicability and any capped retry with both quadratics. Derive the
   binding breakdown from that scenario. Convert completed monetary outputs
   through the accepted Money boundary and carry the same scope and provenance
   through Rust, Python, WASM and wire restoration.
   [Money precision](../core-conventions/012-money-decimal-precision.md) ·
   [Margin wire contract](../margin-collateral-im/22-wire-tables-and-provenance.md#accepted-decision-for-checked-margin-tables-wire-state-and-provenance).

## Choice and domain

The observations below describe the audited design. The accepted contract binds one scenario for complete capital and gives a selected SBA diagnostic its own result scope.

The engine adds delta, vega and curvature charges across the selected risk classes within each selected correlation scenario, selects one largest scenario total, then adds DRC and RRAO. It does not maximize each component independently and does not diversify across risk classes.

## Algorithm and conventions

1. Scenario transformation of a base correlation `r`: medium `r`; low `max(2r−1, 0.75r)`; high `min(1.25r,1)`. Curvature callers supply squared base correlations before this transformation (record 11).
2. Within an ordinary delta/vega bucket, set `K=sqrt(max(Σ_i w_i²+Σ_{i≠j}rho_ij w_i w_j,0))` and preserve `S=Σ_i w_i` uncapped. The diagonal is always one. The implementation loops over both off-diagonal orders.
3. Across ordinary delta/vega buckets, first calculate `Q=Σ_b K_b²+Σ_{b≠c}gamma_bc S_b S_c`. If `Q>=0`, return `sqrt(Q)` using raw S. Otherwise replace each S with `clamp(S,−K,K)` and recompute, finally flooring the quadratic at zero. This fallback is conditional, not an always-applied cap.
4. Non-CTP CSR instead returns `sqrt(Σ ordinary K_b²)+Σ undiversified K_b`; the other bucket is added outside the root. Curvature uses its own sign-aware quadratic and zero floor, without the ordinary delta/vega capped-S retry.
5. Sum positive class charges into each scenario total. Only strict `total>best` replaces the binding scenario, so ties select the first configured scenario. Defaults are `[Low, Medium, High]`: an empty or tied default book reports Low. Store that scenario's component maps and a breakdown for every evaluated scenario. Zero charges are omitted.
6. `total=max(best_SBA,0)+DRC+RRAO`. DRC/RRAO run regardless of the selected SBA risk-class subset. `FrtbSbaEngine::with_selection(Some(vec![scenario]), None)` selects one scenario; `FrtbSbaEngine::default()` selects all.

## Selection and controls

Default construction selects all seven risk classes and all three scenarios. Explicit construction rejects empty lists but neither deduplicates nor rejects duplicates; per-class maps and per-scenario maps overwrite repeated entries, avoiding arithmetic double counting while repeating work. All input fields are validated even if their SBA risk class is excluded. Scenario-list order remains relevant to tie labels.

## Limitations and failure behavior

Parameter and aggregation floors can suppress negative quadratic forms; finite input validation does not prove finite intermediate products or totals. In particular, large finite sensitivities can overflow in squared terms; `max(0)` and positive-charge filtering do not establish a safe nonfinite-intermediate policy. Hash-map traversal remains in several reductions, so serialization determinism alone is not a numerical reproducibility guarantee.

## Alternatives and tradeoffs

Raw signed bucket sums preserve the standard form whenever its quadratic is nonnegative. The accepted cap is a conditional retry. Canonical checked reductions retain the evidence required to reconstruct scenario totals and select the binding result.

## Evidence and role

Owning formulas: [src/regulatory/frtb/aggregation.rs](../../../../finstack-quant/margin/src/regulatory/frtb/aggregation.rs), [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs), [src/regulatory/frtb/types.rs](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs). [tests/frtb_sba_charges.rs](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) includes hand-written commodity scenario totals and cross-class addition; [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates configuration/default and scenario-breakdown reconciliation tests. The latter replays reported components and is not an independent regulatory benchmark. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements) is provenance background.

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
