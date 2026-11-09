# CASHFLOWS-016 — Projected fixing metadata and realized-forward rolls

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped to `fixings.rs` and producer normalization. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

Projected schedules carry raw index observations so a realized-forward market roll can materialize observations that become historical. Existing exact market fixings take precedence. This preserves the pre-roll rate path rather than projecting already-crossed dates from rolled curves. It is a scenario realization policy, not a claim that a forward prediction equals the eventual published fixing.

## Algorithm and controls

1. Store `ProjectedFixing {series_id,date,value}` per needed observation. Rate values are decimal index quotes **before** spread/gearing/caps/floors; FX consumers may use the same structure for quote-currency-per-base-currency rates, with orientation carried by the series identifier. `None` marks an observation whose missing dependency was masked by coupon fallback.
2. Normalize by series ID, date and optional value; sort None before finite values. Collapse duplicate identical values and let a Some value replace None. Distinct conflicting Some values remain present for later conflict checking rather than being averaged.
3. `materialize_fixings` requires `new_date>=old_date`. Equal dates return an unchanged market clone and perform no materialization. An advancing window includes observations at **both** old_date and new_date; earlier dates are not invented.
4. Require nonempty `FIXING:`-prefixed series IDs for crossed observations. If the input market already has an exact value on that date, retain it and skip projected-value validation for that observation.
5. Group missing observations across all schedules. A finite projected value must exist; None-only observations error. Multiple projections must agree within `1e-12×max(abs(values),1)`. Larger differences or nonfinite values error. Values within tolerance replace the accumulated candidate in iteration order; there is no averaging rule.
6. Merge additions into existing observations without overwriting existing dates; retain existing currency and interpolation metadata, or default interpolation for a new series. Build all replacement series first, then return a market clone containing them. Input market is immutable; failures do not partially mutate it.

## Selection and limitations

The method is explicitly called by roll/scenario consumers with schedules projected on the pre-roll market at the old date. It does not rebuild schedules or verify that their metadata truly came from that market/date. Raw fixing generation follows [CASHFLOWS-006](06-floating-rate-projection.md) and [CASHFLOWS-007](07-overnight-compounding.md). SpreadOnly/fixed fallback is not silently promoted to a realized fixing when value is None. An existing exact fixing can satisfy such an observation.

Only observations needed by supplied schedules are materialized. No missing historical gap before old_date is interpolated. SeriesInterpolation is preserved but exact-date lookup still governs precedence. Near-equal conflicting values can produce a tolerance-sized order effect; exact conflicting values are deliberately retained during normalization.

## Alternatives and evidence

Supported alternative is to supply authoritative exact fixings first. Holding fixings unchanged while rolling curves, sampling stochastic realizations or using a forecast source unrelated to pre-roll forwards are different policies, not modes of this function.

- [fixings.rs](../../../../finstack-quant/cashflows/src/fixings.rs), [producer metadata](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs), [final metadata normalization](../../../../finstack-quant/cashflows/src/builder/pipeline.rs), [merge normalization](../../../../finstack-quant/cashflows/src/builder/schedule.rs).
- Located invariant/regression evidence in [production_fixing_roll.rs](../../../../finstack-quant/cashflows/tests/production_fixing_roll.rs): preservation of existing values/metadata, conflict and missing-projection failures, inclusion of both advancing endpoints and no-op equal dates. [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs) locates one raw fixing under weekend balance splits.
- No external standard mandates realized-forward scenario rolls or the conflict tolerance. No tests executed.

## Relationship to the accepted fixing dataset boundary

[CORE-MARKET-12's complete accepted package](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) binds exact rate/CMS
observation identities and separates published fixing qualification from date
matching. Materialized pre-roll model values require explicitly hypothetical
dataset provenance; namespace insertion or a later valuation date cannot confer
actual publication authority. Reconcile strict series construction/policies
without retaining default Step or ordinary generic bumps of fixing history.
The conflict/window/realization policy above remains observed and Draft;
record12 does not adopt it. Record12 was accepted on **2026-11-08**, with
reconciliation and independent qualification pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
