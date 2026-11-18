# MARGIN-COLLATERAL-IM-003 — SIMM sensitivity identity, scaling and netting

**Decision status:** Accepted 2026-11-16 for checked SIMM sensitivity identity, scaling and netting.

**Implementation status:** Existing behavior is described below. Rust, valuation adapter, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Independent examples checked units, FX translation and pair normalization, compensated cancellation, curvature expiry separation and product underflow. Runtime, source-pricing and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM sensitivity identity, scaling and netting

1. Bind the elected SIMM profile, valuation and information cut, calculation currency, qualified netting scope and product-class partition. Keep amount currency, risk-factor currency and signed holding basis distinct. Preserve exact factor identities, including IR subcurve and vertex, qualifying-credit issuer, seniority and payment currency, and nonqualifying-credit issuer and tranche where applicable. Allow explicitly qualified reductions in the historical approximation. Return a typed unsupported error when the elected path cannot represent a required coordinate. [Accepted margin repricing and identity contract](../valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity).
2. Admit signed rates and credit delta as qualified P&L response to an upward 1 bp shock. Admit equity, commodity and FX delta on an upward 1% relative shock. Vega and curvature use signed `σ × ∂PV/∂σ` before historical volatility ratios, risk weights, concentration and expiry scaling. Use matching ATM volatility and derivative units for rates and credit and the prescribed lognormal volatility proxy and compatible derivative units for equity, commodity and FX. Product repricing, proxy choices and vertex mapping stay with the valuation owners. [ISDA SIMM v2.6, §§B and C](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
3. Retain the 12 IR vertices and restrict credit risk tenors to `1Y`, `2Y`, `3Y`, `5Y` and `10Y`. Ordinary IR vega uses option expiry. Ordinary qualifying- and nonqualifying-credit vega use option expiry on the five-vertex grid after source ATM volatility weighting and retain underlying maturity separately in source evidence. These clarifications were accepted on 2026-11-16 in [record 005](05-simm-qualifying-credit.md#accepted-decision-for-checked-simm-qualifying-credit-delta-and-vega) and [record 006](06-simm-nonqualifying-credit.md#accepted-decision-for-checked-simm-nonqualifying-credit-delta-and-vega). The credit underlying risk tenor and curvature option expiry are separate coordinates. Use numeric commodity IDs `1–17` in canonical state and require qualified classification. Reject missing or inapplicable coordinates and unsupported vertices. An expiry label supplies no authority for underlying-tenor snapping. [ISDA SIMM v2.6, §§B and C.1](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
4. Require complete FX sensitivities on the elected calculation basis. Historical USD FX delta uses an upward relative shock to USD per unit of each non-USD risk currency and includes translation exposure. Reject the USD self-factor. Retire generic scalar rebasing of whole sensitivity vectors. The valuation owner supplies qualified repricing or a complete equivalent remap. Qualified amount conversion with unchanged risk coordinates may occur upstream with dated FX evidence. FX vega and curvature require two distinct currencies and an unordered pair canonicalized by alphabetic currency code. Preserve signs of contributions on the same qualified volatility basis. [Accepted portfolio IM contract](../portfolio-execution/16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin) · [ISDA SIMM v2.6, §§C.1–C.3](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
5. Preserve each curvature contribution's factor, applicable risk tenor and option expiry through scaling, merge and export until expiry weighting under [record 010](10-simm-curvature.md). Retain row multiplicity so opposite raw vegas at different expiries receive their own weights before netting. [ISDA SIMM v2.6, §B ¶11](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
6. Apply the finite signed holding multiplier once, including explicit zero. Validate before zero or identity shortcuts. Check products for overflow and detectable nonzero-to-zero underflow. Add ordinary contributions by canonical key with checked Neumaier accumulation in the declared contribution order. Commit complete valid updates atomically. Preserve explicit zero rows and distinguish complete zero risk from missing inputs. Raw IR and equity sums remain checked diagnostics. Retire unchecked merge and public mutation paths that bypass admission. [Accepted ordered summation](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty).
7. Raw contribution rows may add repeated qualified ordinary keys. Canonical snapshots carry one sorted net value per ordinary key and reject duplicates on restore. Curvature retains its row multiplicity. Require explicit currency and sensitivity basis for detached rows and reject populated inapplicable columns. Apply the checked Rust admission contract to JSON, rows, pricing, nested portfolio restoration, Python and WASM. [Record 022](22-wire-tables-and-provenance.md#accepted-decision-for-checked-margin-tables-wire-state-and-provenance) owns detailed table and wire formats.

EUR 100 at 1.25 USD/EUR has USD 1.25 of translation sensitivity to an upward 1% spot shock. Qualified FX vega contributions `+70` and `−60` on opposite pair spellings combine to `+10`. Ordered compensated addition of `1e16`, `1` and `−1e16` retains the `1` in this example. Equal opposite raw vegas at 2W and 1Y give a nonzero weighted curvature sum under their distinct expiry factors.

## Choice and rationale

The observed container adds signed monetary responses by key. Curvature is retained as separate expiry rows. A key match achieves a valid hedge only when both responses share the qualified economic basis.

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

Preserving separate factor dimensions requires the contributing adapters to identify them. A supported historical reduction is part of the elected profile and retains its approximation qualification. Checked construction and restoration make incomplete coordinates visible before netting or calculation.

## Source and validation evidence

Inspected type tests cover canonical JSON ordering, all amount-map scaling/merging, invalid tenors/buckets, and transactional rejection of cross-currency merge. These are transport and accounting invariants, not independent validation of the source pricing sensitivities.

Implementation and data: [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [src/types/simm_curvature.rs](../../../../finstack-quant/margin/src/types/simm_curvature.rs), [src/constants.rs](../../../../finstack-quant/margin/src/constants.rs), [src/traits.rs](../../../../finstack-quant/margin/src/traits.rs), [src/table.rs](../../../../finstack-quant/margin/src/table.rs).

Existing evidence: [src/types/simm_types.rs](../../../../finstack-quant/margin/src/types/simm_types.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
