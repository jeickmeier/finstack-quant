# VALUATIONS-BONDS-02: Settlement carry, ex-coupon entitlement and model clocks

**Decision status:** Accepted — complete checked bond settlement, quote carry and clocks package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
**Validation status:** Source, published references and independent arithmetic reviewed. Runtime, market, model and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond settlement, quote carry and clocks

1. **Elect an admissible settlement date and preserve the distinct
   cuts.** Retain valuation date `V`, knowledge cut `K`, relevant
   trade date and actual settlement `S`. Use an authored eligible
   date or an expressly selected dated settlement convention with
   its own calendar, nonnegative lag and specified adjustment
   order. Count positive business-day lag excluding its start.
   Apply the elected eligibility and adjustment rules even for
   zero lag. Validate an authored actual date without rolling it
   again. Remove implicit same-day, payment-calendar
   substitution and the issue-date clamp. The standard forward
   quote bridge requires `S≥V` and a settlement eligible for that
   trade and security. Settlement may be no earlier than the
   applicable trade date or the legal first eligible issue date.
   Pre-issue trade needs explicit
   when-issued terms or otherwise qualified actual settlement.
   Keep any different contractual accrual start. A forbidden
   final-period settlement has no buyer quote. A historical
   settlement earlier than `V` belongs to position history.
   Preserve the existing holder and settlement buyer rights and
   payment phases under record 01. Advancing the settlement
   date does not advance the knowledge cut.
   [Business-day rules](../core-conventions/006-business-day-adjustments.md),
   [bond schedule and entitlement](01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash)
   and [UK DMO final settlement restriction](https://www.dmo.gov.uk/responsibilities/gilt-market/about-gilts/).

2. **Quote accrued interest on original cash coupon parents at
   settlement.** Use the accepted cash coupon quotation profile
   with original start, end and payment `A<E≤P`, signed whole
   coupon cash `C`, complete day-count context and per-parent
   first-ex date `X` or explicit all-cum election. For `S<P`,
   one contribution is `C[f(S)−1_{X exists and X≤S}]`.
   Here `f` is zero at or before accrual start, one at or
   after accrual end, and the admitted original-parent prefix
   divided by its full accrual between them. Follow the
   accepted zero-measure cases. At or after that payment
   the contribution resets to zero. A detached parent can
   still determine the quote adjustment. Co-dated parents
   retain distinct terms and rights. Retain actual eligible
   coupon amounts or expressly qualified forecasts as of `K`.
   PIK, fees, principal, recovery and earned accrual outside
   this cash quote law retain their own measures. Preserve
   the same-day payment eligibility decision separately from
   the accrued quote reset.
   [Accepted original-parent accrued and first-ex convention](../cashflows/13-accrued-interest.md)
   and [DMO gilt formulae, section three](https://www.dmo.gov.uk/media/334d05fo/yldeqns_v4.pdf).

3. **Bind quote units, denomination and authority.** Prepare a
   whole-position clean or dirty amount in native currency.
   For a price per 100 use the positive contractual quote
   denomination `B_q` and convert `q B_q/100`. Resolve the
   eligible unit of par for amortizing or capitalizing structures
   under the actual quote convention. Set dirty amount to clean
   amount plus signed accrued Money on a
   matching settlement and denomination. The reverse subtracts
   that same accrued amount. A dirty-only observation or derived
   dirty result needs no accrued lookup when it does not use it.
   Require qualified accrued inputs for a clean conversion.
   Keep observed or pinned quote data, a model-derived quote,
   and the selected raw PV identifiable. Ambiguous quote
   units or conflicting authority fail. Use checked arithmetic
   through the raw transform and apply the elected monetary or
   quote rounding at the prescribed output boundary.
   [Checked Money and rounding](../core-conventions/012-money-decimal-precision.md)
   and [DMO dirty price definition and exceptions](https://www.dmo.gov.uk/help/glossary/).

4. **Finance the quote and value changes in entitlement on their
   actual bases.** Bind a qualified settlement financing
   source, such as the expressly selected repo/funding
   evaluator, for positive dated factor `F=D_fin(V,S)`.
   An explicitly qualified shared evaluator may serve both
   security and financing roles. Require exact role
   qualification even when source IDs match. Let `H` be
   the selected raw holder PV at `V`, and `J` the value
   at `V` of the admitted settlement buyer claims on the
   same snapshot and model basis. Let `Δ=H−J`. Use the
   whole-position dirty amount `Q=J/F=(H−Δ)/F`,
   and the reverse `H=FQ+Δ`. For additive cash under
   deterministic discounting, value the signed difference
   by actual claim identity using the admitted security
   factors. Include intervening payments and claims that
   detach or transfer, including effects of a different
   authorized payment cut. Equal dates give `F=1`.
   Differing claims can still give `Δ≠0`. Measure the
   buyer's eligible cash at the authorized settlement
   phase. A trade NPV also includes its actual signed
   purchase cash, whereas `FQ` is the financed dirty
   amount. Reverse and forward quote transforms reuse the
   same binding and qualify a checked roundtrip within an
   explicit native-currency residual budget where both are
   used. A pinned market quote requires its own authorized
   transform basis and provenance.
   [Issuer and settlement financing roles](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf)
   and [Strata fixed coupon trade valuation](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/bond/DiscountingFixedCouponBondTradePricer.html).

5. **Require the selected model to own contingent settlement
   and recovery effects.** Apply decision 4 directly for
   its additive deterministic domain. For hazard, exercise,
   joint and structural models, require an explicit qualified
   buyer-value bridge on the same selected model, including
   any survival, recovery, early termination or delivery
   changes between `V` and `S`. Absent that bridge the
   selected model's settlement-derived quote is unavailable.
   Retain its independently valid holder PV. Conditional
   delivery and any conditional settlement consideration
   require a separate modeled settlement law. Remove
   the generic practice of discounting all detached cash
   on the bond's ordinary curve and starting a fresh model
   at settlement against the old snapshot. The recovery
   owner can select exposure through actual repayment cash
   dates and specify how PIK/default changes enter it.
   Build that exposure from the authoritative events as a
   separate view. Preserve the original economic principal
   ledger and coupon accrual. Leave default settlement,
   exercise pricing and recovery semantics to their owners.
   [Native bond model scope](03-pv-models.md),
   [exercise model](04-rates-tree-exercise.md),
   [joint replay](06-joint-coupon-replay.md)
   and [recovery risk](17-default-risk.md).

6. **Give yield and yield-based carry their authored quote
   coordinates.** Bind the selected yield convention to its
   required nominal frequency and grid, day count, first
   broken interval and contractual yield date per claim.
   Use actual payment dates for cash eligibility and curve
   valuation. Use unadjusted or contractual accrual coordinates
   for yield only where the selected market convention calls
   for them. Build ICMA nominal reference slices from the
   authorized regular and quasi-coupon grid. Retain distinct
   same-date payments until their own quote coordinates are
   resolved. Detachment, zero cash and internal cash/PIK
   splits never renumber that grid. If an unpaid eligible
   payment has a required quote date before or at `S`,
   use only the market convention's explicit branch for
   that situation. Without one the quote is unavailable
   while the payment can still carry PV. For ACT/365L
   use contiguous original actual coupon periods and sum
   checked contained fractions. Each slice uses the full
   coupon end-year denominator for every frequency. Require
   unique coverage without gaps or overlaps. Payment lag
   does not extend required coupon context. Keep Street,
   Moosmüller and workout formula/inverse choices with their
   owners. Carry reporting binds its interval and yield basis
   explicitly, with a separately qualified valuation-origin
   application of any settlement yield.
   [Accepted ICMA and ACT/365L](../core-conventions/002-coupon-context-day-counts.md),
   [DMO yield formulae](https://www.dmo.gov.uk/media/334d05fo/yldeqns_v4.pdf),
   [quoted yield](10-quoted-yield.md),
   [workout yield](11-workout-yield.md)
   and [carry reporting](19-carry-and-life.md).

7. **Use explicit dated model time and preserve actual cash
   dates.** Use ACT/365F from the prepared model origin
   for the dated rates-only tree, daily joint rates-credit
   route and dated Merton route. Parameters and time steps
   must be qualified on that clock. Retain the joint route's
   exact daily dated coordinates. Sources keep independent
   native clocks. Query dated factors and recovery or
   projection inputs on the actual dates. Where a model
   needs off-date numeric curve evaluation, require the
   authorized adjacent-day native-coordinate mapping.
   Never convert a fractional model time into an invented
   fixing or settlement date. Keep economic events,
   exercise and fixing dates and actual payments as exact
   required nodes where their model needs them. Derive the
   required valuation and discount coverage from relevant
   contractual cash and state events, including lagged principal.
   The model owner selects the default or exercise horizon.
   Later cash alone does not extend that horizon. Preserve
   co-dated identified events and original coupon accrual
   factors. Group split components by parent identity.
   Fit, price and risk use the same admitted grid and
   dated market mapping. Remove coupon-clock inheritance,
   rounded model-day discount knots and near-equality
   substitution. Engine eligibility, step controls, event
   order, drift and recovery belong to the model owners.
   [Accepted dated rate-model clocks](../models-analytic/rate-clock-and-projection.md),
   [checked event grids](../core-numerics/time-grids.md),
   [dated Merton simulation](08-merton-simulation.md)
   and [Merton calibration](09-merton-calibration.md).

8. **Reconcile the quote callers on one preparation.** Reuse the
   actual holder and buyer claim sets, model/market basis,
   settlement and date cuts, accrued authority, finance source,
   and quote convention only when their full inputs match.
   Keep dirty-only, clean, yield and model inputs scoped to
   what each path uses. Reconcile price, accrued, yield,
   workout, spread, calibration and risk consumers with
   existing Rust, Python and WASM routes, schema and export.
   Qualify lagged and same-day cash, final detachment,
   pre-issue settlement, differing funding and security
   bases, incomplete quote periods and independent model
   clocks before reporting broader accuracy.
   [Pricing and lifecycle](../valuations-framework/pricing-lifecycle.md)
   and [metric ownership](../valuations-framework/metric-orchestration.md).

## Observed behavior

### Choice and scope

PV is anchored at valuation date; market quote metrics are anchored at settlement. The distinction is explicit in source and affects coupons paid or detached during the settlement interval. Contract accrual, quoted-yield time and stochastic model time need not be the same clock.

### Algorithm and controls

`settlement_date` adds configured business days using the bond's strict payment-calendar lookup, applies its business-day convention, then clamps to issue. With no configured lag it returns `max(as_of, issue)` without business-day adjustment. Unknown calendars and integer overflow error. Quote accrued is computed from the complete schedule at settlement, including negative accrued inside ex-coupon windows.

Let `D` be the bond discount-curve factor from valuation to settlement, and `C_detached` the discounted difference between valuation-holder cashflows and settlement-buyer cashflows. Carry conversions are `P_settle=(PV_asof−C_detached)/D` and `PV_asof=P_settle D+C_detached`. The difference includes intervening payments and coupons lost by settlement ex-coupon entitlement. It uses the bond discount curve, even if a selected tree uses another curve, and does not apply survival/default recovery to detached cash.

Quoted Street/Moosmüller cashflow times normally use the coupon-spec day count, frequency and ICMA quasi-period context. ACT/365L instead sums actual contractual coupon slices. An adjusted coupon payment is timed at its contractual accrual end for quoted yield; settlement after that end but before payment is rejected. Different contractual ends sharing one payment date are rejected. Custom ACT/365L schedules require explicit full, nonoverlapping coupon metadata. Carry ACT/365L similarly slices actual coupon periods and rejects uncovered intervals.

Rates-only tree time follows the selected discount curve; joint rates-credit uses a daily ACT/365F grid. Merton uses the bond coupon day count/ICMA reference context, except ACT/365L uses ACT/365F model time at actual payment dates. Its remaining coupon grid retains full cash/stub/PIK coupon accrual factors, deduplicates split cash/PIK entries by date and omits ex-coupon dates. Horizon is the later of maturity and final coupon date. Merton discount-factor knots combine rounded calendar simulation-grid dates, exact coupon dates and horizon; equal times prefer exact cashflow knots. The drift/discount distinction is covered in record 08.

### Limitations, alternatives and evidence

Cash-date recovery balance replay clears principal-date overrides on amortization, prepayment, notional and revolving rows, while retaining economic PIK/default dates; see [hazard pricing](03-pv-models.md). This is separate from coupon principal replay.

Owning code: [settlement/carry](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/settlement.rs), [clock helpers](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/time_basis.rs), [ACT/365L quote timing](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [quote accrued metric](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/accrued.rs). Inline settlement/time-basis tests and [settlement convention tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/validation/settlement_conventions.rs) locate carry/clock invariants; not rerun. [ISDA](../../../REFERENCES.md#isda-2006-definitions) and [ICMA](../../../REFERENCES.md#icma-rule-book) provide general convention provenance, not security-specific agreement. Using one clock everywhere is simpler but would change implemented economics.

### Reconciliation evidence

The current accrued preparation builds the full schedule without
its own information cut. The quote bridge nets by payment date,
uses the ordinary bond discount curve and sets detached PV to zero
on a same-date quote. Dirty price depends on the accrued metric,
and its model path starts from the finalized Money headline. The
dated Merton factor helper rounds the step count and calendar
days, while the engine uses ceiling step density. The production
pricing and quote paths need the same cash basis and dated grid.
[Settlement and carry helper](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/settlement.rs),
[price metrics](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/prices.rs),
[Merton grid preparation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/time_basis.rs)
and [Merton engine](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/engine.rs).

### Independent review evidence

- On an illustrative weekdays-only calendar, trade on Friday
  January 9, 2026 with T+1 gives Monday January 12. If the lawful
  first settlement is Wednesday January 14, reject the ordinary
  T+1 result. Use January 14 only under an expressly authorized
  when-issued or actual settlement term.
- Take two coupons of 5 and 7 payable January 31, 2025, each
  with a linear January 1–31 accrual and first-ex dates January
  23 and 24 respectively. A buyer settling January 23 keeps 7,
  while the holder has both coupons. The accrued fraction at
  settlement is 22/30. Signed accrued is
  `5(22/30−1)+7(22/30)=3.80`. A separate 2 paid at settlement
  belongs to the holder under the chosen cuts. With common
  later principal 100 and admitted security factors .99 on
  the 2, .98 on the coupons and .95 on principal, holder PV is
  108.74 and the retained/intervening cash PV is 6.88. Financing
  factor .99 gives dirty amount `926/9≈102.888889` and clean
  amount `4459/45≈99.088889`. The inverse restores 108.74.
- For a separate holder with an intervening 3 discounted at .99,
  a detached 5 discounted at .98 and common buyer cash 110
  discounted at .95, holder PV is 112.37 and the buyer claim
  at valuation is 104.50. The difference is 7.87. A distinct
  financing factor .997 gives dirty 104.81444332999. Accrued at
  an ex-parent fraction .8 is −1, giving clean 105.81444332999.
  Reversing the bridge restores holder PV exactly. At equal
  valuation and settlement dates, a separately retained
  coupon of 5 discounted at .98 still gives a 4.90 difference.
- From November 30, 2023 across actual coupon ends December
  31, 2023 and June 30, 2024, the selected ACT/365L quote clock
  to contractual end gives `31/365+182/366≈.5821992664`.
  Payment on July 1 has dated ACT/365F model time
  `214/365≈.5863013699`. For a separate semiannual ICMA
  short first period starting October 15, 2023 and ending
  January 31, 2024, the nominal July 31–January 31 reference
  gives `108/(2×184)`. Adding the next full half year gives
  `73/92` years.
- On a 37-day dated Merton horizon with density 12 per year,
  rounded density is one while the simulated ceiling count is
  two. A knot at `1/12` year does not reproduce the simulation's
  interior coordinate `37/730` or its event grid. The numerical
  grid and the factors must use the same prepared coordinates.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
