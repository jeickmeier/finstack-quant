# CASHFLOWS-006 — Term projection, fixing selection and rate constraints

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

Term coupons use the forward curve's embedded-tenor quote at their actual reset date, with annualization converted to the contractual coupon basis. They do not use an average forward over the payment period. Coupons then apply index bounds, spread/gearing and all-in bounds in that order. Source comments motivate avoiding changes in the economic index when payment frequency changes. Cashflows receive projection curves, not discount curves; discounting is separate.

## Formula and ordered selection

For curve clock `t(reset)`, curve quote `F(t)`, curve-basis accrual `αc(start,end)` and contractual accrual `αk`, project `I = F(t(reset)) × αc/αk`. Require positive-length accrual and finite positive `αk` and `αc`; reset before curve base is an error rather than short-end extrapolation. Exact historical fixings are already in the index convention and are not re-annualized.

Given decimal raw index `I`, spread `s=spread_bp×10^-4` and gearing `g`:

1. `J = min(max(I,index_floor),index_cap)` for present bounds.
2. Default `gearing_includes_spread=true`: `R0=g(J+s)`. Explicit affine mode: `R0=gJ+s`.
3. `R=min(max(R0,all_in_floor),all_in_cap)` for present bounds.

All bounds are input in basis points and converted by `10^-4`. Validation requires finite spread/bounds, finite **positive** gearing, floor no greater than cap, and nonnegative reset lag. Negative rates/spreads are allowed. `calculate_floating_rate` and `apply_bp_bounds` themselves are infallible arithmetic helpers and do not call validation; checked callers must establish their domain.

## Historical fixings, fallback and alternative entry point

Builder emission resolves `FIXING:{forward_curve_id}` and the curve per leg. `build_as_of` supplies the observation cutoff; ordinary `build` uses curve base date. Observation before cutoff requires an exact-date fixing; observation on cutoff prefers an exact fixing but projects if absent; future supplied observations are ignored. Fixing calendar overrides payment calendar; reset lag counts business days, with zero lag retaining accrual start.

Projection currently resolves a forward curve **before** choosing historical fixings, so even a fully fixed builder coupon requires a curve unless an explicit fallback is used. Default fallback is Error. SpreadOnly substitutes raw index zero; FixedRate substitutes the supplied decimal index. Normal constraints/spread/gearing still apply. A projection failure falls back for the whole coupon, not just the missing segment. If both curve and fixing series exist, a second check requires all historical fixings; a supplied but incomplete series cannot be silently repaired by fallback. Missing entire series/curve can reach the configured fallback. Earlier raw observations appended before a later failure can remain in metadata; unresolved observations retain `value=None` for [CASHFLOWS-016](16-fixing-materialization.md).

`RateSpec::try_rate_for_period`, used for facility/tranche-style term coupons, is a materially different path:

- Fixed rate only requires finiteness; the supplied period inputs are otherwise unused.
- Floating rejects **any** explicit compounding value, including `Some(Simple)`; unset compounding is required.
- Fixing calendar defaults to weekends-only, not a supplied payment calendar.
- Reset **on or before** `as_of` requires an exact fixing; same-day projection is disallowed.
- Future projection calls the same rate kernel. Configured FloatingRateFallback is not consumed by this method.

## Controls and limitations

Floating spec defaults: gearing one, spread-inclusive gearing, reset lag two, daily overnight index bounds, no bounds, no explicit compounding and Error fallback. The SOFR/SONIA presets choose compounded-in-arrears with zero lookback, zero reset lag and ACT/360/ACT/365F index basis respectively. EURIBOR 3M is term, 3M tenor, two-business-day lag and TARGET2 calendar.

`reset_frequency` does not create a separate fixing grid in this builder: term coupons reset once per generated coupon period. `index_tenor` is checked only for a warning when it differs from resolved curve tenor by more than 10%; projection uses the curve's embedded tenor. No interpolation between index tenors or multiple-reset coupon aggregation is implemented here. The separate overnight engine is [CASHFLOWS-007](07-overnight-compounding.md).

## Alternatives and evidence

Supported alternatives are the two gearing conventions and explicit fallback policies. Averaged period forwards, interpolation between term tenors, and fixing-only builder valuation without a curve are possible unimplemented alternatives.

- [rate_helpers.rs](../../../../finstack-quant/cashflows/src/builder/rate_helpers.rs), [rate specs/defaults](../../../../finstack-quant/cashflows/src/builder/specs/coupon.rs), [RateSpec](../../../../finstack-quant/cashflows/src/builder/specs/rate.rs), [emission selection/fallback](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs), [curve-tenor warning](../../../../finstack-quant/cashflows/src/builder/pipeline.rs).
- Located formula/regression tests: `term_projection_uses_the_reset_date_fixing_not_the_period_average`, `term_projection_converts_curve_basis_before_coupon_constraints` in rate_helpers; [floating_rate.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/floating_rate.rs) covers fallback modes, reset-date fixings and same-day projection. Emission unit test `overnight_fixing_policy_uses_valuation_cutoff_not_curve_base` was inspected; it explicitly expects future observations to be ignored and missing same-day fixing to project.
- [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) is the repository convention pointer. No independent vendor tolerance study or contractual adoption rationale for every fallback/entry-point difference was located.

## Relationship to the accepted exact fixing boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) resolves observed IndexId
independently of projection CurveId and verifies exact configured series
association before lookup. Exact retrieval supplies no publication authority;
qualify the actual information phase and selected source/version before using
a fixing, without presence-only same-day or swallowed-error projection. Reconcile
both builder and RateSpec paths and raw fixing metadata together. Accepted02's
projection contract is unchanged. Record12 was accepted on **2026-11-08**,
with reconciliation/qualification pending; this record's remaining cutoff/
fallback/coupon choices stay Draft.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
