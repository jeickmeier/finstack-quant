# MODELS-AND-VALUATIONS-CREDIT-12: CDS conditional survival, premium accrual and protection integration

- Decision status: **Accepted — complete checked CDS legs and quote-basis package, 2026-11-13**.
- Implementation status: Current behavior observed below. The accepted contract requires Rust, host, calibration and product reconciliation.
- Validation status: Source, selected tests and primary references inspected. Independent date, integral and quote examples evaluated. Production, host and external-reference qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked CDS legs and quote basis

1. **Keep one reduced-form pricing model and require explicit contract
   selection.** Retain `HazardRate` for single-name fixed-running-coupon
   CDS with or without a dated upfront. Require a typed CDS schedule
   choice, the dated terms and the default settlement rule. Remove
   the implicit Bloomberg selection and the four vendor-named bundles
   from production valuation. Clean and dirty are named output
   calculations on the same legs. Regional convention data and
   documentation clause describe contractual terms, without choosing a
   valuation or quote basis. Record the actual selected terms and
   integration method in output provenance. Historical reference fixtures
   remain evidence for their stated settings. Do not claim vendor parity
   from a model or convention name.

2. **Resolve dates in the CDS owner.** Support a quarterly twentieth
   schedule with explicit first accrual anchor and unadjusted maturity,
   ACT/360 and a supplied Following payment calendar, and a bespoke
   schedule with authored accrual boundaries, payment dates and complete
   coupon day-count context. For the quarterly schedule, intermediate
   accrual periods meet on adjusted coupon dates. The final period ends
   at `M+1` exclusive, so it includes the unadjusted maturity day `M`.
   Roll final payment Following from `M` independently. Use the accepted
   [schedule](../core-conventions/008-schedule-generation.md)
   and [roll-date](../core-conventions/009-contractual-roll-dates.md)
   primitives; do not restore a generic CDS schedule mode or infer a
   trade-tenor regime. The materializer owns first-coupon selection
   (for the documented standard issuance, the first adjusted coupon
   strictly after calendar step-in) and any separately supported stubs.
   Validate strict positive contiguous accrual slices, monotone dated
   payments and payment on or after each slice's last accrual day.
   The bespoke final accrual ends on the same exclusive boundary
   `M+1`; separate protection termination is outside this model.
   Coupon accrual dates are civil half-open intervals and distinct
   from credit-observation boundaries and payment dates.

3. **Adopt the valuation-close clock and include the final covered day.**
   `as_of=v` means close of civil date `v`. A default on date `d`
   occupies the curve cell from close of `d−1` to close of `d`.
   The first modeled covered day is the later of calendar `v+1`
   and the contractual first protection day. Require that authored
   day in the inclusive range from first accrual day to `M`, even
   when it equals an endpoint. The last covered day is `M`,
   inclusive. One first covered day equal to `M` is one day of
   protection; a first modeled day after `M` gives an empty interval.
   Compute date arithmetic with checked bounds. A later contractual
   start reduces covered default mass, without resetting the coupon
   accrual origin or reconditioning the issuer at the forward start.
   The date convention is a model for an alive issuer at valuation,
   and does not value backstop claims or resolve actual default history.
   No extra valuation-day slice or guessed protection start.
   [ISDA converter specification, assumptions](https://cdsmodel.com/assets/cds-model/docs/ISDA%20Standard%20CDS%20Contract%20Converter%20Specification%20-%20Sept%204%2C%202009.pdf).

4. **Keep scheduled premium, accrual on default and earned cash distinct.**
   For a civil accrual interval `[a,b)`, let its contractual day-count
   fraction be `τ`. Use conditional survival to close of `b−1`,
   its last accrual day, and relative discount to the payment date.
   If that accrual has completed at valuation, an eligible unpaid
   coupon is earned under the stated alive-at-valuation condition
   and uses survival one. It can remain after protection expiry.
   Accrual on default (AoD) is always included in this model:
   integrate only where that coupon's risk interval overlaps the
   modeled covered default interval. Use accrued fraction from
   original contractual `a`, even in a clipped or forward-start
   slice. The scheduled premium still depends on survival from
   valuation to its own risk cutoff. No separate termination
   premium is modeled for a default before protection begins.
   Within the cell for default day `d`, interpolate linearly
   between the coupon-clock accrued fractions through the prior day
   and through `d`, using the contractual clock and context.
   Require finite nonnegative and nondecreasing accrued fractions
   up to the full scheduled fraction. Omit inherited half-day shifts
   and end-day surcharges. Full premium is
   the sum of scheduled and AoD annuities times the contractual coupon
   and notional. Validate all supplied cash before filtering. A single
   required same-day cash choice applies to both eligible coupons
   and actual dated upfront, at factor one if included. Earlier
   payments are outside this forward value. Overdue or actual default
   settlement claims require a separate explicit lifecycle state.
   [CORE-CONVENTIONS-016](../core-conventions/016-discounting.md).

5. **Use local hazard, checked relative discount and coherent recovery.**
   Require explicit discount and hazard sources for the trade's
   market roles, coherent frozen inputs, known notional currency,
   positive notional, a finite signed coupon in bp and explicitly
   authored contract recovery `0<=R<1`. The hazard source's recovery
   metadata must agree exactly on the represented scenario input;
   remove the implicit absolute tolerance. No implied curve, override
   or recalibration from a mismatch. Preserve the accepted
   [hazard law](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation):
   `S(t|v)=exp(−J(v,t))` from the local nonnegative intensity, with
   default mass derived from local integrated hazard. Historical
   absolute survival may round to zero without erasing a later
   conditional exposure. Honor each curve's retained base, clock,
   context, interpolation and extrapolation admission. Apply the
   [checked relative discount rules](../core-market/discount-evaluation.md)
   with no unit-factor fallback. Trade currency and any authoritative
   curve currency metadata must agree; no currency inference from
   IDs or implicit FX. Earned coupons do not query hazard. The
   pricing contract still requires the selected
   sources and validates their identity and applicable metadata.

6. **Separate default settlement from upfront and build the integration
   grid forward from dates.** Use explicit contemporaneous payment
   at the modeled default instant, or a positive whole business-day
   lag and an explicitly selected settlement calendar. Apply a
   positive lag strictly after the named default day and use that
   one settlement-date discount for the whole day cell. This timing
   applies to both contingent legs. An explicitly selected weekends-only
   calendar is a calendar, without fallback to `365/252` conversion
   or a hidden weekend adjustment. Quote cash settlement and actual
   dated upfront have independent dates. Walk active civil date cells,
   using the retained source-base clock at each close. Between dates
   interpolate each source coordinate affinely inside its own cell.
   Keep the exact dated boundaries and split within each cell at all
   crossed hazard knots and, for contemporaneous settlement, discount
   source and transform breakpoints. A hazard-clock plateau carries no
   default mass; a clock reversal or unresolved distinct split errors.
   Use log-linear relative discount within each contemporaneous split
   window. Check finite positive source endpoint factors and use stable
   log differences, without materializing an unused endpoint quotient.
   This is exact when the source log discount is affine there and
   otherwise an explicit approximation. A lagged cell uses constant
   log discount. Reject date, calendar or curve coverage failures in
   required inputs. No hazard-time-to-date inversion, fixed 25-step
   mesh, rounded knot translation, or alternate grid fallback. Cap
   active date cells at 100,000 and total split windows at 1,000,000,
   with checked counts and allocation. Those are resource limits,
   without a numerical accuracy guarantee.

7. **Integrate protection and linear accrued premium with the same stable
   moments.** On a split window let `H` be the checked local integrated
   hazard, `G_0=log D(v,s_0)−J(v,t_0)` the initial log weight and
   `z=Δlog D−H` the log-weight increment. Form the increment without
   subtracting large rounded cumulative hazards. Orient the window
   from the larger log weight, swapping accrued endpoints if needed.
   For `b=|z|`, use positive weights
   `w_L=∫_0^1 (1−u)e^(−bu)du` and
   `w_R=∫_0^1 u e^(−bu)du`. Both equal `1/2` at exact zero.
   For nonzero `b`, their formulas are
   `(b+expm1(−b))/b²` and
   `(−expm1(−b)−b exp(−b))/b²`. Use stable small-argument series
   and scaled large-argument evaluation. Protection per unit
   is `(1−R) H exp(max G)(w_L+w_R)`. The unit-coupon AoD
   annuity is `H exp(max G)(τ_L w_L+τ_R w_R)` with accrued
   endpoints in the same orientation. Skip exact zero `H` without
   using rounded survival differences. For active windows, preserve
   required finite contributions with scaled operations and checked
   ordered accumulation. An unresolved active-window hazard integral,
   invalid source value or out-of-range requested result fails.
   No epsilon cutoff, midpoint fallback or silent drop of a
   representable positive contribution. This is analytic for the
   selected window law, without a general curved-source error bound.
   [White, sections 5.2–5.4](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf).

8. **Define economic PV, clean quotes and par on the same affine legs.**
   Let `P` be protection PV per unit notional, `A_s` the scheduled
   annuity and `A_d` the AoD annuity, each per unit coupon and
   notional. Use `A_dirty=A_s+A_d`. On the protection-buyer axis,
   for positive notional `N` and coupon `c=c_bp/10000`,
   product dirty PV before upfront is `N(P−c A_dirty)`. Economic
   trade NPV subtracts eligible actual upfront PV. Positive actual
   upfront is buyer cash paid to the seller, negative is received,
   and its date and currency are explicit. The seller reverses the
   sign once. Economic NPV does not add a display accrual.

   For a clean quote require a named quote settlement date `s>=v`
   and its relative factor `B` (one for an explicit as-of quote).
   Quote accrual fraction `α` comes from the actual current coupon
   through valuation close, i.e. to civil `v+1` exclusive, excluding
   completed periods and giving zero at a new period's opening or
   after final accrual. Do not use the last rolled payment as the
   accrual origin or a forward protection date as the cutoff.
   Clean annuity is `A_clean=A_dirty−Bα`. The buyer's clean mark
   before upfront is `N(P−c A_clean)` and clean upfront fraction
   at `s` is `(P−c A_clean)/B`. Actual break-even settlement cash
   is `N(P−c A_dirty)/B`, or `N` times the clean upfront fraction
   less `Ncα`. Only actual signed cash is carried as contract upfront;
   quote materialization owns creating it from the selected quote
   representation. Do not subtract an existing upfront from a
   zero-upfront par or clean-upfront quote.

   Return running par spread `10000 P/A_selected` on a required
   clean or dirty basis. This zeros the matching pre-upfront mark.
   Require a live protection interval and a finite strictly positive
   selected annuity for par. Zero hazard on a live interval with
   positive annuity gives zero par. No arbitrary small-denominator
   floor. Scheduled-only annuity remains a labelled component, not
   an alternative par definition. The selected premium coefficient
   gives `N A_selected/10000` per coupon bp independently of the
   current coupon; quote bump risk and metric aliases stay with
   record 13. Use one checked raw valuation and finalize the reporting
   amount at the Money boundary, preserving the original cash amount
   and sign on same-day cash. [ISDA converter, cash and upfront](https://cdsmodel.com/assets/cds-model/docs/ISDA%20Standard%20CDS%20Contract%20Converter%20Specification%20-%20Sept%204%2C%202009.pdf).

9. **Freeze validated inputs and share one Rust result across routes.**
   Prepared and ordinary valuation use identical date selection,
   source admission and checked leg formulas. Freeze contract,
   valuation date, cash cutoff, resolved calendar data and discount
   source. Cache fixed dates, discount state and at most one
   bounded hazard topology keyed by the full source-base clock and
   context and exact knot coordinates. Recompute hazard integrals
   and weights for each scenario; changed geometry requires rebuilding.
   Cached and uncached results and required errors agree. Validate
   selected geometry before using it and query contingent discount
   factors only for active nonzero default mass with a requested
   contribution. Unused factor queries cannot introduce an error;
   required ones cannot be suppressed by a cache. Reject malformed
   restore atomically. No separate host mathematics or second raw
   price path. Reconcile builders, wire schemas, Python/WASM,
   valuation details, market recipes and calibration replay with the
   required choices. A contractual cashflow view labels upfront
   and scheduled premium; it does not purport to project the
   contingent leg PV. Index adapters, option forward CDS, and hazard
   materializers pass explicit terms without adopting index
   composition, quote risk, option or fit policy in this record.
   The existing [ADR 0030](0030-cds-survival-and-integration.md)
   points to this detailed selection.

## Observed behavior

### Choice and rationale

A stateless CDS pricer applies instrument valuation conventions and piecewise-constant analytic default integrals. The source explains named bundles through screen reconciliation and retains both curve knot grids with separate discount/hazard clocks. The model label does not itself establish external ISDA implementation parity.

### Scope and input domain

Single-name fixed-coupon CDS, explicit discount/hazard curves, authored contractual interval (the current validator admits equal endpoints) and recovery consistent with hazard bootstrap within 1e−6 absolute decimal recovery. Protection ends at premium maturity; separate protection termination is unsupported.

### Algorithm

Generate CDS-IMM quarterly/short-front or bespoke schedules. Conventions select adjusted accrual boundaries, inclusive ACT/360 final/day behavior, clean/dirty presentation and step-in (ISDA T+1 versus immediate Bloomberg). Payments on as-of are settled. Unpaid rolled coupons after accrual maturity survive and use certain survival if accrual has ended.
Condition survival by S(as-of) and discount by D(as-of); negligible survival makes future contingent legs zero. Integration partitions at interval ends, hazard knots, discount knots converted through dates, and 25 substeps per hazard-curve year. Map hazard times to dates (actual-day direct rounding; other day counts iterative) and shift defaults by business settlement delay; without calendar use rounded 365/252 conversion and weekend adjustment. Each window infers λ from survival endpoints and r from settlement-discount endpoints.
Protection integrates LGD×D1S1λ(1−exp(−(λ+r)Δ))/(λ+r), linear limit near zero. AoD integrates linear coupon accrual from contractual coupon start using I0=(1−exp(−kΔ))/k and I1=(1−exp(−kΔ)(1+kΔ))/k²; near zero uses midpoint mass. ACT/360 convention flags add half-day biases. Scheduled premium adds coupon×accrual×conditional survival×relative DF.
Buyer NPV=protection−premium−unpaid upfront, adding accrued for clean price. Par spread divides protection by selected full-AoD, clean, or scheduled-only annuity; tiny/nonpositive denominator errors. Hazard-bump cache retains discount/schedule but re-integrates protection and AoD.

### Selection and controls

HazardRate family dispatch; convention is instrument state. AoD always enters premium PV. Upfront on as-of remains included although coupons on as-of are dropped. Forward protection starts change the protection interval, while the observed AoD integration can still begin before coverage; an exhausted interval has zero protection value while rolled unpaid coupons can remain.

`CdsValuationConvention` selects these bundles; it exposes no independent Boolean mixtures:

| Bundle | Premium accrual and PV | Protection step-in and par denominator |
| --- | --- | --- |
| `BloombergCdswClean` (default) | Adjusted intermediate accrual dates, final maturity day included, clean principal | From valuation date subject to contractual start; full premium annuity including AoD, less accrued fraction |
| `BloombergCdswCleanFullPremium` | Same Bloomberg accrual and clean presentation | Same step-in; full premium annuity including AoD, without accrued subtraction |
| `IsdaDirty` | Unadjusted accrual dates, final maturity day excluded, dirty PV | One calendar day after valuation subject to contractual start; scheduled risky annuity |
| `QuantLibIsdaParity` | Adjusted dates, dirty PV, every Act/360 period includes its end date, half-day AoD adjustment | From valuation date subject to contractual start; full premium denominator |

Regional `CdsConvention` schedule presets do not automatically select `IsdaDirty`. These bundles specify intended reference comparisons, not independently certified system equivalence.

The [denominator kernel](../../../../finstack-quant/valuations/credit/src/cds/pricing/metrics.rs) includes AoD in both Bloomberg variants. Its clean branch subtracts accrued; the full-premium branch does not. Source prose describing the default as excluding AoD, or the full-premium variant as only adding AoD, does not match that calculation.

### Conventions

Coupon input and par-spread output are bp, converted to decimal rates by dividing by 10,000. Recovery/survival are decimal fractions; notional/PV use the instrument currency. Hazard and discount each use their own base date/day count, with discount factors relative to as-of; coupon accrual uses instrument day count. Buyer is Pay premium, seller Receive.

### Limitations, failure and fallback

Integration is exact only for inferred piecewise-constant window rates, not for arbitrary interpolated curves; date mapping and settlement discontinuities are approximated. Direct exponential differences risk cancellation near but above the small-k branch. Nondeclining survival windows are skipped.

Missing curves and recovery disagreement error; no hazard curve is inferred. General hazard-time inversion iterates at most 30 times and returns its current date estimate if the day-count calculation fails. This and absent-calendar settlement are observed fallbacks requiring agreement, not universal mixed-day-count accuracy guarantees.

### Alternatives and tradeoffs

Daily/exact calendar integration or independent standard-engine replication are alternatives requiring agreement and validation.

## Evidence

Owners: [CDS kernels](../../../../finstack-quant/valuations/credit/src/cds/pricing), [types.rs](../../../../finstack-quant/valuations/credit/src/cds/types.rs). engine.rs tests `cds_unpaid_final_coupon_survives_protection_expiry_and_cache` and `cds_aod_cashflows_match_independent_integral_for_both_clean_denominators` provide lifecycle and independent integral evidence; cache tests compare production paths. See [selection](0030-cds-survival-and-integration.md).

- [Convention definitions](../../../../finstack-quant/valuations/core/src/market/conventions/defs.rs), [integration helpers](../../../../finstack-quant/valuations/credit/src/cds/pricing/helpers.rs), [leg engine](../../../../finstack-quant/valuations/credit/src/cds/pricing/engine.rs), [registry adapter](../../../../finstack-quant/valuations/credit/src/family.rs) and [index delegation](../../../../finstack-quant/valuations/credit/src/cds_index/pricer.rs) own bundles, clocks and dispatch.
- [Pricing tests](../../../../finstack-quant/valuations/credit/src/cds/pricing/tests.rs) locate denominator, forward-start, settlement and premium-linearity regression/property cases. [Convention regressions](../../../../finstack-quant/valuations/tests/instruments/cds/test_cds_metrics.rs) exercise clean pricing, adjusted dates and full-premium par spread.
- [QuantLib decomposition](../../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/cds/cds_quantlib_flat_hazard_decomposition.json) and [Bloomberg CDSW](../../../../finstack-quant/valuations/tests/golden/data/pricing/bloomberg/cds/cds_5y_par_spread.json) are named comparison fixtures. Their provenance, tolerances and broader domain are not qualified under [ADR 0014](../../project/0014-financial-reference-evidence.md); they were not rerun.
- Current post-audit [prepared-grid cache](../../../../finstack-quant/valuations/credit/src/cds/pricing/grid_cache.rs)
  is later than the pinned coverage revision. Its current key does
  not capture the complete source clock context. Reconcile cache
  qualification with frozen dates, calendars and topology.
- Existing callers include the [CDS calibration materializer](../../../../finstack-quant/calibration/src/build/cds.rs),
  [Python builder](../../../../finstack-quant-py/src/bindings/valuations/typed_credit/cds.rs),
  [WASM builder](../../../../finstack-quant-wasm/src/api/valuations/typed/credit.rs)
  and [CDS option adapter](../../../../finstack-quant/valuations/credit/src/cds_option/pricer.rs).
  Current quote materialization derives both upfront spot and default
  payment lag from the convention settlement-days field. Reconcile
  those independent roles and the clean quote/actual cash distinction.
- References: [ISDA](../../../REFERENCES.md#isda-cds-standard-model), [Bloomberg CDS model](../../../REFERENCES.md#bloomberg-cds-model) and [Hull–White CDS](../../../REFERENCES.md#hull-white-2000-cds). Quote construction and fitting have separate [materialization](../calibration-framework/07-cds-materialization.md) and [hazard-target](../calibration-targets/05-hazard-calibration.md) owners.

### Independent review evidence

- The [published standard coupon example](https://www.cdsmodel.com/assets/cds-model/docs/Standard%20CDS%20Examples.pdf)
  accrues from December 21, 2009 through Saturday March 20, 2010.
  The inclusive final period is 90 ACT/360 days. At 100 bp and
  36 million notional the final coupon is 90,000, payable on
  Monday March 22. At Friday March 19 close there is one covered
  calendar day, and from Saturday close the future rolled coupon
  remains earned under alive-at-valuation conditioning.
- In an independent one-day ACT/360 example with 10 million
  notional, 40% recovery, 100 bp coupon, zero discount rate and
  integrated hazard .001, protection PV is about 5,997.0010,
  scheduled premium PV 277.5001 and AoD PV .1387963.
- In a forward-start example the original coupon began 30 days
  before valuation. Coverage begins at the boundary 30 days after
  valuation and ends at day 60. At .20 ACT/365F hazard, .04 ACT/360
  discount, ACT/360 accrual and .40 recovery, protection PV per
  unit is about .009574952028 and the AoD annuity .003322445013.
  Resetting the accrued origin at coverage start instead gives an
  AoD annuity of only .000662736116.
- For a one-business-day settlement lag with a synthetic Monday
  March 17, 2025 holiday, Friday through Monday defaults settle
  Tuesday March 18. Tuesday defaults settle Wednesday March 19.
  This makes settlement discount a sequence of civil-day values.
- With window hazard .02 and discount log growth +.02, the combined
  exponent increment is zero. With 60% LGD and .20/.30 accrued
  endpoints, per-unit protection is .012 and unit-coupon AoD is
  .005. At window hazard 1e−18 and flat discount, protection is
  about 6e−19 even though both endpoint survivals round to one.
- With unit protection .02, scheduled annuity 1, AoD .05, quote
  accrued .10 and settlement factor .99, dirty and clean par are
  about 190.476190 bp and 210.304942 bp respectively. At 100 bp
  coupon and 10 million notional, dirty PV is 95,000 and clean PV
  104,900 before existing upfront. Actual break-even settlement
  cash is about 95,959.60 and clean quote amount about 105,959.60,
  a 10,000 accrued rebate. A scheduled-only ratio of 200 bp
  zeros neither mark.
- These checks used independent date, rational and high-precision
  exponential arithmetic. They do not qualify production integration,
  cache or host parity, curve interpolation error, or vendor equivalence.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
