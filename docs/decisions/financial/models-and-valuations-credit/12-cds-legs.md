# MODELS-AND-VALUATIONS-CREDIT-12: CDS conditional survival, premium accrual and protection integration

- Decision status: **Accepted for contractual dates — named CDS date regimes (O7), 2026-11-12**; other content remains Draft (observed implementation, adoption pending).
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

A stateless CDS pricer applies instrument valuation conventions and piecewise-constant analytic default integrals. The source explains named bundles through screen reconciliation and retains both curve knot grids with separate discount/hazard clocks. The model label does not itself establish external ISDA implementation parity.

## Scope and input domain

Single-name fixed-coupon CDS, explicit discount/hazard curves, positive contractual interval and recovery consistent with hazard bootstrap within 1e−6 absolute decimal recovery. Protection ends at premium maturity; separate protection termination is unsupported.

## Algorithm

Generate CDS-IMM quarterly/short-front or bespoke schedules. Conventions select adjusted accrual boundaries, inclusive ACT/360 final/day behavior, clean/dirty presentation and step-in (ISDA T+1 versus immediate Bloomberg). Payments on as-of are settled. Unpaid rolled coupons after accrual maturity survive and use certain survival if accrual has ended.
Condition survival by S(as-of) and discount by D(as-of); negligible survival makes future contingent legs zero. Integration partitions at interval ends, hazard knots, discount knots converted through dates, and 25 substeps per hazard-curve year. Map hazard times to dates (actual-day direct rounding; other day counts iterative) and shift defaults by business settlement delay; without calendar use rounded 365/252 conversion and weekend adjustment. Each window infers λ from survival endpoints and r from settlement-discount endpoints.
Protection integrates LGD×D1S1λ(1−exp(−(λ+r)Δ))/(λ+r), linear limit near zero. AoD integrates linear coupon accrual from contractual coupon start using I0=(1−exp(−kΔ))/k and I1=(1−exp(−kΔ)(1+kΔ))/k²; near zero uses midpoint mass. ACT/360 convention flags add half-day biases. Scheduled premium adds coupon×accrual×conditional survival×relative DF.
Buyer NPV=protection−premium−unpaid upfront, adding accrued for clean price. Par spread divides protection by selected full-AoD, clean, or scheduled-only annuity; tiny/nonpositive denominator errors. Hazard-bump cache retains discount/schedule but re-integrates protection and AoD.

## Selection and controls

HazardRate family dispatch; convention is instrument state. AoD always enters premium PV. Upfront on as-of remains included although coupons on as-of are dropped. Forward protection starts change the protection interval without resetting contractual premium accrual origin; an exhausted interval has zero protection value while rolled unpaid coupons can remain.

`CdsValuationConvention` selects these bundles; it exposes no independent Boolean mixtures:

| Bundle | Premium accrual and PV | Protection step-in and par denominator |
| --- | --- | --- |
| `BloombergCdswClean` (default) | Adjusted intermediate accrual dates, final maturity day included, clean principal | From valuation date subject to contractual start; full premium annuity including AoD, less accrued fraction |
| `BloombergCdswCleanFullPremium` | Same Bloomberg accrual and clean presentation | Same step-in; full premium annuity including AoD, without accrued subtraction |
| `IsdaDirty` | Unadjusted accrual dates, final maturity day excluded, dirty PV | One calendar day after valuation subject to contractual start; scheduled risky annuity |
| `QuantLibIsdaParity` | Adjusted dates, dirty PV, every Act/360 period includes its end date, half-day AoD adjustment | From valuation date subject to contractual start; full premium denominator |

Regional `CdsConvention` schedule presets do not automatically select `IsdaDirty`. These bundles specify intended reference comparisons, not independently certified system equivalence.

The [denominator kernel](../../../../finstack-quant/valuations/credit/src/cds/pricing/metrics.rs) includes AoD in both Bloomberg variants. Its clean branch subtracts accrued; the full-premium branch does not. Source prose describing the default as excluding AoD, or the full-premium variant as only adding AoD, does not match that calculation.

## Conventions

Coupon input and par-spread output are bp, converted to decimal rates by dividing by 10,000. Recovery/survival are decimal fractions; notional/PV use the instrument currency. Hazard and discount each use their own base date/day count, with discount factors relative to as-of; coupon accrual uses instrument day count. Buyer is Pay premium, seller Receive.

## Accepted decision: named CDS contractual date regimes

Accepted on **2026-11-12** as owner resolution O7 of the core-conventions
reconciliation (closing the register's CDS product-boundary Decision entry).
Runtime implementation and independent qualification are tracked in the
register.

1. **No generic CDS grid.** The cashflows roll grid `RollRule::CdsImm` and
   its frequency/stub overrides are removed. A CDS product (single name,
   index, tranche, option underlying) and its calibration builder select a
   named contractual date regime explicitly; nothing falls back to a
   regime, a valuation date or a core date helper.
2. **The product resolves five distinct dates.** Premium accrual start,
   protection start (step-in), upfront cash settlement, maturity and
   default payment are separate roles. The selected regime owns how each is
   derived, the stub selection and the endpoint inclusion; one date never
   stands in for another.
3. **Post-2015 single-name quoting is a named regime.** A tenor quote under
   that regime matures on the latest March/September 20 roll on or before
   the trade date, plus three months, plus the tenor. It is qualified
   against the official December 2015 dated table: a 2015-12-22 trade gives
   2016-03-20 for 3M and 2016-09-20 for 9M, and the whole-year points keep
   their June/December 20 maturities. A maturity on or before the trade
   date is never accepted for a positive tenor. Tenors that the regime and
   its fixtures do not determine are errors.
4. **Literal maturities stay literal.** An explicit contractual maturity
   date is used as given, without rolling to a CDS date.
5. **Other regimes need their own selected rules.** A bespoke schedule
   states its own frequency, stub and boundaries; no other regime is
   inferred.

The standard premium grid resolves the observed contradiction between the
pricer's short-front requirement and the short-back generation: under the
standard regime the grid runs quarterly on CDS roll dates from the accrual
start roll to an on-grid maturity, so no stub arises; an off-grid maturity is
a bespoke schedule that states its own stub.

## Limitations, failure and fallback

Integration is exact only for inferred piecewise-constant window rates, not for arbitrary interpolated curves; date mapping and settlement discontinuities are approximated. Direct exponential differences risk cancellation near but above the small-k branch. Nondeclining survival windows are skipped.

Missing curves and recovery disagreement error; no hazard curve is inferred. General hazard-time inversion iterates at most 30 times and returns its current date estimate if the day-count calculation fails. This and absent-calendar settlement are observed fallbacks requiring agreement, not universal mixed-day-count accuracy guarantees.

## Alternatives and tradeoffs

Daily/exact calendar integration or independent standard-engine replication are alternatives requiring agreement and validation.

## Evidence

Owners: [CDS kernels](../../../../finstack-quant/valuations/credit/src/cds/pricing), [types.rs](../../../../finstack-quant/valuations/credit/src/cds/types.rs). engine.rs tests `cds_unpaid_final_coupon_survives_protection_expiry_and_cache` and `cds_aod_cashflows_match_independent_integral_for_both_clean_denominators` provide lifecycle and independent integral evidence; cache tests compare production paths. See [selection](0030-cds-survival-and-integration.md).

- [Convention definitions](../../../../finstack-quant/valuations/core/src/market/conventions/defs.rs), [integration helpers](../../../../finstack-quant/valuations/credit/src/cds/pricing/helpers.rs), [leg engine](../../../../finstack-quant/valuations/credit/src/cds/pricing/engine.rs), [registry adapter](../../../../finstack-quant/valuations/credit/src/family.rs) and [index delegation](../../../../finstack-quant/valuations/credit/src/cds_index/pricer.rs) own bundles, clocks and dispatch.
- [Pricing tests](../../../../finstack-quant/valuations/credit/src/cds/pricing/tests.rs) locate denominator, forward-start, settlement and premium-linearity regression/property cases. [Convention regressions](../../../../finstack-quant/valuations/tests/instruments/cds/test_cds_metrics.rs) exercise clean pricing, adjusted dates and full-premium par spread.
- [QuantLib decomposition](../../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/cds/cds_quantlib_flat_hazard_decomposition.json) and [Bloomberg CDSW](../../../../finstack-quant/valuations/tests/golden/data/pricing/bloomberg/cds/cds_5y_par_spread.json) are named comparison fixtures. Their provenance, tolerances and broader domain are not qualified under [ADR 0014](../../project/0014-financial-reference-evidence.md); they were not rerun.
- References: [ISDA](../../../REFERENCES.md#isda-cds-standard-model), [Bloomberg CDS model](../../../REFERENCES.md#bloomberg-cds-model) and [Hull–White CDS](../../../REFERENCES.md#hull-white-2000-cds). Quote construction and fitting have separate [materialization](../calibration-framework/07-cds-materialization.md) and [hazard-target](../calibration-targets/05-hazard-calibration.md) owners.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
