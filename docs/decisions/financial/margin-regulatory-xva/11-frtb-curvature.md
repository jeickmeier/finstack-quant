# MARGIN-REGULATORY-XVA-011: Loss-positive curvature, squared correlations and directional selection

- Decision status: **Accepted 2026-11-17 for checked FRTB curvature.**
- Implementation status: Existing behavior is described below. Qualified reprices and coverage, per-pair FX direction selection, tranche-pair evidence, residual diagnostics and checked result paths remain pending.
- Validation status: Primary references and independent arithmetic checked for signed loss preparation, shock adjustments, direction and tie selection, scenario correlations and aggregation. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked FRTB curvature

1. **Elect the calculation scope explicitly.** Include required instruments
   with optionality or nonlinear payoffs. A profile that elects broader
   curvature coverage must apply it to all instruments in scope of the
   sensitivities-based method and consistently over time. Missing required
   curvature makes the affected charge unavailable.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR21.2](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
2. **Require two prepared, loss-positive amounts per factor.** For each
   direction use `CVR = signed linear delta P&L − shocked PV change` in
   the reporting currency. Use the same delta and pricing assumptions
   used for delta capital. Sum the relevant tenor deltas for GIRR,
   credit and commodity curve shocks. Net matching factors separately
   by direction. The prepared amounts already reflect the full shock.
   [Basel MAR21.5 and FAQ 1](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Apply the prescribed shocks to complete factors.** Shock all
   qualifying GIRR yield curves in a currency together. Apply additive
   credit spread shocks to the qualified issuer, CTP reference name or
   index, or non-CTP tranche curve. GIRR and credit use the highest
   applicable delta weight for the curve's bucket. Apply relative
   shocks to the complete commodity curve, equity spot and FX reporting
   pairs using their applicable delta weights. Keep index treatment
   consistent with delta.
   [GIRR delta](03-frtb-girr-delta.md#accepted-decision-for-checked-girr-delta) ·
   [Credit spread delta](04-frtb-credit-delta.md#accepted-decision-for-checked-credit-spread-delta) ·
   [Equity delta](05-frtb-equity-delta.md#accepted-decision-for-checked-equity-spot-and-repo-delta) ·
   [Commodity delta](06-frtb-commodity-delta.md#accepted-decision-for-checked-commodity-delta) ·
   [FX delta](07-frtb-fx-delta.md#accepted-decision-for-checked-fx-delta) ·
   [Basel MAR21.5, MAR21.8–14, MAR21.32 and MAR21.97–99](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
4. **Make shock adjustments explicit.** An elected CSR zero floor uses the
   effective distance to zero for both the affected downward reprice and
   its delta deduction. The upward shock uses the full weight. On the
   direct FX reporting basis, require both legs for options on two
   non-reporting currencies. When `1.5` relief is elected, divide each
   eligible option's two directional amounts before netting with other
   exposures.
   The all-FX relief route requires supervisory approval and qualified
   coverage of shocks to the reporting currency against all other
   currencies.
   [Basel MAR21.14, MAR21.98 and MAR21.99 FAQ 2](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Square correlations before applying the scenario.** Use the relevant
   delta correlation for the dimensions that survive in curvature,
   including the qualified non-CTP tranche relationship. Square the
   completed coefficient, then apply Low, Medium or High. For FX this
   gives cross-bucket coefficients of `0.27`, `0.36` and `0.45`.
   [Basel MAR21.6 and MAR21.100–101](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
6. **Select the direction independently for each bucket and correlation
   scenario.** Use squared positive losses on the diagonal and signed
   products off the diagonal. The `ψ` indicator is zero only when both
   amounts are strictly negative. Choose the larger `K`, then the larger
   raw signed sum `S` on a tie, then down on exact equality. Use one
   bucket per GIRR currency and one per canonical FX reporting pair.
   Two FX buckets each losing `100` only in opposite directions give a
   Medium charge of approximately `164.9242`. The observed single-bucket
   calculation gives `100` for this case.
   [Basel MAR21.5 and MAR21.97](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
7. **Retain the other-sector rules.** Sum positive CVRs separately by
   direction in non-sec and CTP bucket 16, equity bucket 11 and non-CTP
   bucket 25. Keep the selected direction and its raw signed sum.
   Commodity bucket 11 uses its ordinary within-bucket formula. Add
   non-CTP bucket 25 outside the ordinary root as an explicit profile
   decision, consistent with the accepted delta and vega treatment.
   [Accepted bucketed vega](09-frtb-bucketed-vega.md#accepted-decision-for-checked-credit-equity-and-commodity-vega) ·
   [Basel MAR21.56–57, MAR21.69–71 and MAR21.79–85](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
8. **Aggregate with raw signed sums and checked arithmetic.** Apply
   cross-bucket `ψ` to the selected sums and the prescribed zero floor
   to the finite quadratic. Preserve both directional charges, the
   selected direction, raw sums, applied shocks and relief, coverage
   and scenario evidence through Rust, wire, Python and WASM. Align
   source comments with the raw-sum and square-before-scenario rules.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) ·
   [Basel MAR21.5 and MAR21.101](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).

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

Finite inputs can overflow during aggregation before the zero floor. The current API relies on the caller for shock size, delta subtraction and instrument coverage. The source overview describes a cap despite the runtime retaining raw signed sums for ordinary buckets. The FX path selects one direction for the whole map. Other-sector paths return the selected positive sum as both `K` and `S`.

## Alternatives and tradeoffs

The valuation owner supplies evidence for the full reprices and delta deductions. Preserving both directions and the selected raw sum makes asymmetric losses and the applicable curvature formula auditable.

## Evidence and role

Inspected tests cover squared equity, commodity and credit correlations, the sign indicator and raw bucket sums. Independent examples checked signed delta removal, the CSR floor and FX relief, zero floors, exact ties, residual addition and the opposite-direction FX case. Runtime verification remains pending.
[Runtime and unit tests](../../../../finstack-quant/margin/src/regulatory/frtb/curvature.rs) ·
[Quadratic aggregation](../../../../finstack-quant/margin/src/regulatory/frtb/aggregation.rs) ·
[Scenario parameters](../../../../finstack-quant/margin/src/regulatory/frtb/params/correlation_scenarios.rs) ·
[Audit regressions](../../../../finstack-quant/margin/tests/audit_regressions.rs) ·
[Charge examples](../../../../finstack-quant/margin/tests/frtb_sba_charges.rs) ·
[FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
