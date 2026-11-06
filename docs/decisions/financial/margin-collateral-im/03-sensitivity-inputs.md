# MARGIN-COLLATERAL-IM-003 — SIMM sensitivity identity, scaling and netting

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: sensitivities are signed monetary responses indexed by economically distinct factors, and matching keys add. Curvature is retained as separate expiry rows. Proposed rationale: preserve actual hedges while keeping concentrations and option-expiry scaling computable.

## Input domain and conventions

IR/CQ/CNQ delta is currency P&L per 1 bp; equity/FX/commodity delta is currency P&L per 1% relative price increase. Vega is the model-compatible `sigma*dPV/dsigma` amount before prescribed weighting; factor shapes collapse some expiry/underlying-maturity dimensions. IR uses currency/tenor, CQ sector/name/tenor, CNQ name/tenor, equity underlier, FX currency or currency pair, and commodity bucket. The public calculator requires USD-normalized amounts and USD output.

## Ordered algorithm

1. `add_*` accumulates signed values under exact keys; `add_curvature` appends without premature netting. Empty maps are distinct from populated zero-valued maps for `is_empty`.
2. `merge` checks only equal base currencies before calling `merge_unchecked`; maps add and curvature vectors append. No FX lookup or amount-domain validation occurs in merge.
3. `scaled(factor)` multiplies all map and curvature amounts, retaining signs and base currency. `scaled_to_currency(target,fx_rate)` changes the currency and uniformly scales every amount; if target equals source, it returns a clone and ignores the supplied rate. Risk-factor keys do not change.
4. Validation rejects nonfinite map amounts, empty equity/credit identifiers, unsupported tenors and unknown commodity labels; curvature uses its stricter dedicated validator. Ordinary credit delta/vega accepts all 12 SIMM tenor labels, whereas credit curvature accepts only 1Y/2Y/3Y/5Y/10Y.
5. Commodity labels accept numeric 1–17 or normalized names (trimmed, ASCII alphanumeric, case-insensitive; indices/indexes alias 17). FX vega pair order remains distinct in the container but is canonically netted during pricing. The ordinary vega validator does not reject same-currency pairs.
6. JSON arrays are sorted deterministically on export. Import calls adders so duplicate entries sum, then `from_json` validates. Direct wire conversion, `to_json`, row import and unchecked merge have different validation boundaries.

## Selection, controls and limitations

`total_ir_delta` and `total_equity_delta` are raw signed sums, not margin estimates. Constants also publish 0.9 duration proxy, 5-year standard CDS maturity, 365 days/year and tenor breakpoints for consumers; their use in instrument bridges belongs to those owners. Scaling accepts arbitrary factors and FX values; final pricing catches nonfinite sensitivities but does not verify a positive/economically valid FX rate.

Deterministic serialization does not establish numerical accuracy or insertion-order invariance of floating-point accumulation.

## Alternatives and tradeoffs

A fully typed factor grid with separate expiry, underlying tenor, product class and subcurve dimensions would prevent current information loss at the cost of broader adapters. Requiring validation on every mutation would fail earlier but change builder ergonomics. Neither applies automatically from the SIMM label.

## Source and validation evidence

Inspected type tests cover canonical JSON ordering, all amount-map scaling/merging, invalid tenors/buckets, and transactional rejection of cross-currency merge. These are transport and accounting invariants, not independent validation of the source pricing sensitivities.

Implementation and data: [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [src/types/simm_curvature.rs](../../../../finstack-quant/margin/src/types/simm_curvature.rs), [src/constants.rs](../../../../finstack-quant/margin/src/constants.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [src/table.rs](../../../../finstack-quant/margin/src/table.rs).

Existing evidence: [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
