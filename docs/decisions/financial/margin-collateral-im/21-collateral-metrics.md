# MARGIN-COLLATERAL-IM-021 — Collateral adequacy, funding spread and haircut sensitivity

**Decision status:** Accepted 2026-11-17 for checked collateral adequacy, static funding cost and haircut sensitivity.

**Implementation status:** Existing behavior is described below. Exact adequacy comparisons, optional ratios, checked funding and haircut calculations, and validated restoration through callers remain pending.

**Validation status:** Source and financial references inspected. Independent Decimal examples checked zero-requirement cases, a shortfall hidden by the displayed ratio, signed funding cost and the full one-basis-point haircut boundary. Runtime and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked collateral metrics

1. **Compare qualified amounts on the same basis.** Require nonnegative exact
   Money for credited collateral `C` and required collateral `R`, in the same
   currency and for the same account, side and cut. The caller supplies the
   credited amount under the elected eligibility and FX rules. Reject invalid
   amounts and currency mismatches.
   [Credited collateral rules](15-collateral-eligibility-and-haircuts.md#accepted-decision-for-checked-collateral-eligibility-and-credited-value).
2. **Decide adequacy from Money and make ratios optional.** Adequacy is `C ≥ R`,
   signed excess is `C − R` and shortfall is `max(R − C, 0)`. For `R > 0`,
   report `C/R` and `(C − R)/R` as decimal fractions. When `R = 0`, both ratios
   are unavailable. The account is still adequate, shortfall is zero and
   monetary excess equals `C`. A ratio displayed as `1.0` can coexist with a
   one-cent shortfall on a large balance.
   [Money comparison and ratio rules](../core-conventions/012-money-decimal-precision.md).
3. **Report static, signed funding cost.** Require a nonnegative funded posted
   balance, signed Decimal annual funding and collateral rates on the same basis,
   and an explicit nonnegative year fraction for period cost. Annual cost is
   `balance × (funding rate − collateral rate)`. Period cost scales that amount
   by the year fraction using checked Money. Positive cost is an expense and
   negative cost is a benefit. Dated expected margin profiles, discounting and
   reuse belong to the valuation model. [Green and Kenyon on MVA](https://arxiv.org/abs/1405.0508).
4. **Define `Haircut01` as the reduction in collateral credit.** Replace
   `pv_change` with `credit_reduction`. Holding gross value, FX and eligibility
   fixed, a full positive one-basis-point increase reduces credit by
   `gross value × 0.0001`. Validate total haircut in `[0, 1]` and report it in
   basis points without whole-basis-point rounding. The full step is available
   through `99.99%`. Above that, the reduction is unavailable. An admissible
   step on 100 million of gross collateral reduces credit by 10,000.
   [Collateral valuation formula](15-collateral-eligibility-and-haircuts.md#accepted-decision-for-checked-collateral-eligibility-and-credited-value) ·
   [EU collateral haircut formula, Annex II](https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A02016R2251-20230214).
5. **Restore results from validated inputs.** Use one checked Rust calculation
   for construction and restoration. Persist primitive inputs and reject supplied
   derived fields in restorable state. Expose derived values through Rust accessors
   in Python, WASM and reports. Preserve exact Money outputs and distinguish
   unavailable diagnostics from zero and from input or arithmetic errors.
   Record 022 owns the wire format. [Wire and provenance](22-wire-tables-and-provenance.md#accepted-decision-for-checked-margin-tables-wire-state-and-provenance).

## Choice and rationale

These metrics describe one observed balance, funding spread or collateral mark. The accepted contract uses exact monetary comparisons and gives each optional ratio or haircut change its own availability rule.

## Input domain and conventions

Money inputs carry currency. MarginUtilization requires same-currency finite nonnegative posted/required amounts. ExcessCollateral checks currency but does not impose nonnegativity. Funding/collateral rates are annual decimals; haircut is a decimal metadata value and period length a year fraction.

## Ordered algorithm

1. Utilization=posted/required for required>0; posted>0, required 0 gives+infinity; posted=required=0 gives1. Adequacy iff ratio≥1; shortfall=required-posted iff ratio<1, otherwise 0.
2. Utilization JSON serializes only posted/required and recomputes ratio on deserialization so infinity need not appear on wire. Fields are public, so a manually inconsistent struct is not validated by every accessor.
3. ExcessCollateral=collateral-required, positive/negative tests report excess/shortfall. Percentage=excess/required when required>0, else0, even if a positive collateral balance exists.
4. Funding spread=funding_rate-collateral_rate; annual cost=posted×spread; period cost=annual_cost×year_fraction. No discounting, changing exposure, accrual calendar, threshold or segregated reuse rule is applied. Negative spread gives a benefit.
5. Haircut01's pv_change=collateral_value×0.0001, independent of current haircut; current_haircut only reports haircut_bp=haircut×10000. This is a positive additional-haircut amount convention, not a full collateral portfolio PV reprice.

## Selection, controls and limitations

Money operations provide currency/finite arithmetic failure behavior, but only utilization enforces a nonnegative input domain at construction. The derived fields of ExcessCollateral, MarginFundingCost and Haircut01 serialize directly and can be inconsistent if constructed/deserialized independently of helpers.

## Alternatives and tradeoffs

The accepted ratios require a positive denominator. Dynamic funding valuation and collateral repricing under changed prices, FX or eligibility belong to the relevant valuation methods.

## Source and validation evidence

Inspected metric tests cover 12 million over 10 million (1.2), an 8 million balance against a 10 million requirement (2 million shortfall), 5 million of excess on a 100 million requirement, 100,000 of annual cost at 20 bp on 50 million, and a 10,000 reduction for one bp on 100 million. The observed zero-requirement JSON roundtrip checks state restoration. Dynamic funding valuation requires separate evidence.

Implementation and data: [src/metrics/mod.rs](../../../../finstack-quant/margin/src/metrics/mod.rs), [src/schema.rs](../../../../finstack-quant/margin/src/schema.rs).

Existing evidence: [src/metrics/mod.rs](../../../../finstack-quant/margin/src/metrics/mod.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
