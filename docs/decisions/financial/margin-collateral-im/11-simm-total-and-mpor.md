# MARGIN-COLLATERAL-IM-011 — SIMM component assembly, MPOR scaling and diagnostics

**Decision status:** Accepted 2026-11-17 for product-class SIMM assembly, supported horizons and checked results.

**Implementation status:** Existing behavior is described below. Product partitioning, coverage, horizon selection, checked assembly and caller reconciliation with the accepted contract are pending.

**Validation status:** Primary references and caller paths inspected. Independent arithmetic checked product separation and the within-product class norm. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for product-class SIMM assembly and supported horizons

1. Bind a complete calculation context to the exact edition and immutable parameter snapshot, valuation date and information cut, netting scope, USD calculation basis and collect or post perspective. Input owners qualify observations for that cut. Select the calculation currency from the elected basis and remove the instrument MTM currency lookup. Recalculate from explicitly converted signed sensitivities for the opposite side. [Parameter selection](01-registry-and-defaults.md#accepted-decision-for-explicit-margin-parameter-snapshots-and-contractual-terms) · [Sensitivity identity and scaling](03-sensitivity-inputs.md#accepted-decision-for-checked-simm-sensitivity-identity-scaling-and-netting) · [Curvature perspective](10-simm-curvature.md#accepted-decision-for-checked-simm-expiry-scaling-and-curvature-aggregation).
2. Require complete applicability and coverage. Distinguish evaluated zero, prescribed zero, qualified inapplicability and missing required risk. Check every component for finite nonnegative value before filtering or assembly. An empty set returns zero only after validation establishes complete scope and coverage. Required qualifying-credit base correlation, Section L add-ons or product multipliers return typed unsupported errors. A successful basic profile requires explicit inapplicability for those features. Withhold the total for an incomplete set. [Qualifying credit](05-simm-qualifying-credit.md#accepted-decision-for-checked-simm-qualifying-credit-delta-and-vega) · [ISDA SIMM v2.6, §§B.5 and L](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
3. Assign each trade once to RatesFX, Credit, Equity or Commodity and retain all its risk in that product class. Net and compute component charges within each product. For each risk class in that product sum delta, vega, curvature and applicable base correlation. Apply `sqrt(mᵀΨm)` to that product's six risk-class subtotals using the admitted unit-diagonal matrix with all fifteen class pairs. Sum the four product totals. Product separation governs both exposure netting and diversification. [ISDA SIMM v2.6, §§B.5–6 and K](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf) · [Portfolio scope](../portfolio-execution/16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin).
4. Require an elected positive integer model MPOR in business days that matches the immutable supported calibration. The bundled historical profile supports ten business days. A different model horizon requires its own supported calibration and method. Return a typed error for an unsupported horizon. Remove `sqrt(MPOR/10)` scaling, the SIMM `with_mpor` override and the `mpor_scale` diagnostic. Keep the agreement's required horizon, the curvature formula's fourteen calendar days and MVA year fractions explicit. [Parameter admission](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation) · [ORE v2.6 horizon configuration](https://github.com/OpenSourceRisk/Engine/blob/master/OREAnalytics/orea/simm/simmconfigurationisdav2_6.cpp#L73-L86).
5. Use fixed component, risk-class and product order, checked compensated sums and scaled quadratics. Apply record 002's dependence validation and return typed errors for negative computed variance, invalid dependence and unusable arithmetic. Convert each completed monetary output once through `Money::from_model_value`. Compute subtotals and the total from checked model intermediates before their conversion to Money. [Dependence validation](02-simm-parameter-validation.md#accepted-decision-for-checked-simm-parameter-profiles-and-correlation-validation) · [Money model output boundary](../core-conventions/012-money-decimal-precision.md).
6. Return one authoritative, validated Rust result. Always retain method, edition, snapshot and source identity, dates and cut, USD basis, netting and product scope, perspective, model horizon, component coverage and approximation status. Preserve evaluated zeros and prescribed-zero reasons. Optional details come from the same reference-horizon calculation. Standalone component charges and risk-class subtotals precede diversification. Validate construction and restoration and leave the wire layout to record 022. [Result transport](22-wire-tables-and-provenance.md#accepted-decision-for-checked-margin-tables-wire-state-and-provenance).
7. Retire `calculate_from_sensitivities_parts` and propagate the common result through portfolio, MVA, Python and WASM. The historical indicative profile carries `approximation=true` with its coverage and provenance. Record 017 owns contractual IM thresholds and transfers. The XVA owner controls future IM projection and funding calculations. [Portfolio aggregation](../portfolio-execution/16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin) · [Contractual IM](17-csa-selection-and-im-terms.md) · [MVA projection](../margin-regulatory-xva/22-simm-deterministic-im-profile.md).

Two separate product margins of USD 100 sum to USD 200. IR and equity risk-class subtotals of USD 100 within one product give USD 146.29 under the historical class correlation of 0.07. This example checks the scope of the correlation step.

## Choice and rationale

The observed calculation adds delta, vega and curvature within each risk class and applies a single pooled class norm. The accepted calculation uses a separate norm per product class so only risks within that product diversify.

## Input domain and conventions

All sensitivity amounts and requested output must be USD; any other output or base-currency mismatch errors. MPOR is a positive number of business days. A caller-provided as-of date labels the result; this stage does not project future exposure or inspect market-data age.

## Ordered algorithm

1. Validate the complete sensitivity set, including otherwise-unused zero entries, then enforce USD normalization.
2. Compute each populated delta/vega class and all curvature classes. Only strictly positive components are included in breakdown/detail and added to the class subtotal.
3. Sort class subtotals and compute `sqrt(max(0,sum_r,s rho_rs*M_r*M_s))`. Missing cross-class lookup uses 1.0, subject to parameter construction checks; the embedded table provides all 15 distinct pairs.
4. Require MPOR>0; multiply total and monetary component breakdown by `sqrt(mpor_days/10)`. No extra horizon evolution, VaR simulation or calibration occurs.
5. Detailed weighted sensitivities, bucket K/S and class/component margins remain at the 10-day basis; detail carries `mpor_scale` separately. Component breakdowns are standalone charges and do not sum to the diversified total.
6. The full result sets `methodology=Simm`, `approximation=true`, ordered string breakdown and optional detailed replay data. The `parts` helper returns only total and breakdown. Trait calculation first calls instrument MTM to select its currency, then requests its sensitivities.

## Selection, controls and limitations

Default MPOR 10 is registry-backed; `with_mpor` mutates it without validation until pricing. Empty valid USD sensitivities produce zero. The ordinary trait path therefore rejects a non-USD MTM even if a supplied sensitivity container is normalized to USD; direct normalized APIs are the supported route for that case. No CSA threshold/MTA/segregation is applied here.

## Alternatives and tradeoffs

The selected historical profile evaluates its calibrated horizon directly. Additional horizons require qualified methods and data. The supported indicative result carries coverage and approximation status through every consumer.

## Source and validation evidence

Inline formula tests check IR/CQ correlation 0.04 and IR/equity 0.07, horizon scaling and detailed replay behavior. `audit_regressions.rs` checks approximate marking and rejected non-USD output. These are formula/invariant tests, not regulatory approval or a stochastic horizon backtest.

Implementation and data: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [src/calculators/traits.rs](../../../../finstack-quant/margin/src/calculators/traits.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [src/constants.rs](../../../../finstack-quant/margin/src/constants.rs).

Existing evidence: [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
