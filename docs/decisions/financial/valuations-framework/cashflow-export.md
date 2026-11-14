# VALUATIONS-FRAMEWORK-15 — Schedule PV, term fixing coordinates and reconciled cashflow export

- Decision status: **Accepted — complete checked cashflow valuation and export package, 2026-11-14**.
- Implementation status: **Observed, with reconciliation pending** across schedule adapters, model-owned decomposition, reporting, restore and hosts.
- Validation status: Source and primary references inspected. Independent bounded settlement, discounting, accrual, projection, credit and reconciliation examples checked. Runtime, product and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked cashflow valuation and export

1. **Prepare one admitted cash stream for dated schedule PV.** Separate
   complete contractual events from the owner-selected signed settlement
   cash. Bind currency, holder or trade basis, effective date, settlement
   phase, entitlements and active discount source. Both raw and Money
   paths use the same qualified cash selection and require explicit
   same-day inclusion. A calibration trade may include its admitted
   initial exchange; it is not part of an ordinary holder mark unless
   that owner includes the claim. Validate the complete cash request and
   native currency before date and zero shortcuts, including before
   converting Money rows to scalars. Mixed currencies require qualified
   component pricing. Reuse checked core NPV and the source's dated
   relative `D(as_of,T)` with its actual clock and domain. Same-day
   entitled cash contributes at one. An admitted empty, excluded or
   zero stream returns zero in the prepared currency without fetching
   unused factors or curves. Require positive finite future factors;
   factors above one are valid. Preserve checked raw and exact Money
   paths and do not emulate raw with a finalized price.
   [Dated NPV](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting),
   [qualified schedule views](../cashflows/12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views).

2. **Resolve term observations, then select actual or projected rates.**
   Bind the exact contractual index, currency, calendar tenor, fixing
   date, effective date `S`, maturity `E` and positive index fraction
   `α`. Keep the coupon fraction separate. Use the required exact
   observed fixing when eligible under the information phase; a missing
   required fixing errors. Do not look up unused curves. When projection
   is admitted, bind the exact index projection source and use
   `[P(S)/P(E)−1]/α` with the source's checked dated factors and
   stable arithmetic. Preserve signed source coordinates and admitted
   tails. Remove float-tenor matching, reset-date-only curve evaluation
   and invented day-count context. Product owners select in-period,
   stub or interpolated-index observations.
   [Term projection](../cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures),
   [projection factors](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
   [Ibor observation fields](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/index/IborIndexObservation.html).

3. **Use the actual coupon reference grid and payment lag.** For
   ACT/ACT ICMA, require the owner's unadjusted nominal grid,
   frequency and roll rule with exact contained slices, including
   nominal boundaries inside long stubs. Sum the slices with the
   accepted single-period kernel. For ACT/365L, use each actual
   enclosing contractual calculation period and its own denominator.
   Never infer either from the first future payment or adjusted
   cash dates. Remove the quasi-grid fallback. For shared payment
   lag use an explicit nonnegative whole business-day count and the
   exact selected calendar, including an explicit weekends-only
   selection where intended. Start from the actual product-selected
   payment reference date; use the schedule's adjustment-then-lag
   ordering where applicable. Zero lag preserves that date after
   control admission; positive lag excludes the start, with no second
   roll. Reject negative lags, unknown calendars and failed stepping.
   [Coupon day counts](../core-conventions/002-coupon-context-day-counts.md#accepted-decision-coupon-context-day-counts),
   [schedule grids](../core-conventions/008-schedule-generation.md#accepted-decision-schedule-grids-events-and-failure-boundaries),
   [business-day stepping](../core-conventions/006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions),
   [Strata ICMA](https://strata.opengamma.io/day_counts/#actualactual-icma).

4. **Use the actual annuity and check the returned par quote.**
   Eliminate the shared `1e−12` floor. Use the owner-qualified
   annuity on the same currency, sign, quantities, eligible cash
   and price basis. For a positive-weight annuity, finite positive
   values, including small representable values, use a checked
   scaled quotient. An exact zero coefficient gives no unique affine
   par quote;
   nonfinite, unresolved or invalid-sign results fail. Retain
   signed quote units and check the returned quote by repricing
   against an explicit owner-qualified native-currency residual
   budget. Do not apply affine par formulas to nonlinear cash laws.
   [Accepted TRS replacement spread basis](trs-legs.md#accepted-decision-for-checked-trs-legs-and-par-spread).

5. **Require explicit decomposition support and full pricing admission.**
   Each registered product/model pair must explicitly supply the
   complete components of its selected price. A model name or
   a schedule alone does not establish support. Default to
   unsupported. Standalone export passes full public Rust validation
   first, including controls, date, selected pricer and currency basis.
   The capability gate is separate. Use one prepared valuation and
   one admitted result, with trace reusing them privately. Do not
   bless eligibility with a trusted flag or reproject through a
   different model, default input set or market snapshot. Tree,
   stochastic and exercise results remain unavailable here unless
   the exact owner can supply a complete meaningful attribution.
   [Accepted pricing lifecycle](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).

6. **Keep contractual events and actual model contributions distinct.**
   Preserve cash/event identity, dates, leg or component scope, exact
   native amount, kind, principal effects and accrual metadata.
   Bind each attributed PV to the actual selected price component
   and exact active discount and credit bindings, including multiple
   bindings within one currency. Never use the first declared curve,
   a foreign default or hazard recovery metadata as authority. Label
   excluded, settled, and noncash audit rows. Do not invent a unit
   factor, FX or a PV contribution for those rows. Preserve actual
   evaluated zero distinctly. For ordinary admitted future cash
   use the dated core factor; disclose source-clock coordinates only
   if actually known. For credit, the selected product owns exposure,
   expected amounts, default-time integration, recovery, protection
   and accrual-on-default. Use source-qualified conditional survival
   `exp(−J(as_of,T))` and owner-defined interval default mass where
   those annotations apply. An integrated contribution can have an
   owner-defined attribution date distinct from a contractual payment.
   Remove the generic credit PV adapter,
   absolute survival ratio, clamps and previous-row default probability.
   Do not fabricate recovery dates or double weight expected cash.
   [Core cash/event semantics](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata),
   [product credit PV](../cashflows/15-credit-adjusted-pv.md#accepted-decision-product-owned-credit-valuation-and-qualified-pv-reporting),
   [local source probabilities](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation),
   [Strata cash and PV amounts](https://strata.opengamma.io/apidocs/com/opengamma/strata/market/amount/CashFlow.html).

7. **Match native, translated and adjusted amounts to the price.** Use
   the selected price's reporting currency and actual qualified FX
   basis. For valuation-date translation use the same exact ordered
   dated query, policy and frozen source the pricer used. Payment-date
   conversion is a different basis; do not choose it implicitly.
   Report FX provenance from the actual conversion, with no second
   query for display. Same-currency and unvalued rows need no FX
   quotes. Preserve pretranslation, pre-generic-adjustment native
   PV separately from reporting PV. Apply the generic multiplier
   exactly once at the admitted boundary. Match the canonical row,
   subtotal and finalization boundaries; if independent displayed
   row rounding would change the basis, retain the exact underlying
   amount or disclose that the rounded display is not the reconciled
   measure. No implicit minor-unit rounding, Money-to-float comparison
   or float-to-Money round trip. [Exact money](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
   [directional FX](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks).

8. **Reconcile the complete export on a stated price basis.** Require
   complete, nonoverlapping component attribution and verify the
   unadjusted decomposition before comparing the corresponding final
   adjusted total, even when the multiplier is zero. Both comparisons
   use the same prepared headline basis and checked differences.
   Use exact equality whenever the common arithmetic is exact.
   Otherwise require an explicit owner finite reconciliation budget,
   with absolute cash and any relative allowances and their reference
   scale stated, including admitted numerical, conversion and rounding
   allowances. Validate nonnegative finite budget components and the
   resulting bound. Report the actual reference, total, signed residual
   and applied bound in the common currency. Remove the universal
   floor/one-part-per-million rule. Return an error for an incomplete
   decomposition, incompatible currency or basis, arithmetic failure
   or excess residual. No plugs or rescaling. A qualified empty
   decomposition requires proven absence of contributing claims;
   missing schedules or zero price alone do not prove this.
   Reconciliation checks consistency of the decomposition, and can
   pass even if a model or component allocation is wrong.

9. **Attach only qualified balance, pool and index metadata.** Use
   exact leg, row, component or period identities from the selected
   projection. Distinguish schedule date-level closing principal,
   owner-supplied before/after state, actual coupon accrual notional
   or slices, and pool-period state and mortality. Do not recreate
   generic per-flow balances, copy a closing balance to an accrual
   start, or let pool data overwrite schedule semantics. Bind CPI
   to actual reference observations and the applied row treatment,
   distinguishing raw ratios from floored ratios. Same-day rows
   may differ. Omit a field when the owner establishes it does not
   apply or is outside the explicitly selected scope. Missing or
   failed required metadata errors; do not erase a failure as null.
   See [schedule stock](../cashflows/12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views).

10. **Keep ladders, trace and host state faithful to their basis.**
    Nominal ladders group signed settlement amounts by payment year
    and native currency under [cashflow reporting](../cashflows/14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life).
    Keep the valuation native-PV ladder separate in meaning and
    selection, with the owner's actual PV attribution dates.
    Use exact native Money totals and the nominal kernel's canonical
    year and currency keys. Currency panes are a presentation view.
    An excluded/no-PV row cannot create a dummy PV bucket.
    Reporting-adjusted PV is not a native ladder column. For generic
    trace reconcile the full export first, then return the bounded
    projection with total row count and explicit truncation.
    Unavailable optional trace preserves the valid price and carries
    status and reason outside the row budget, including at zero rows.
    Standalone export does not succeed on an unavailable decomposition.
    Require strict canonical Rust restoration for existing envelopes
    and reports; check dates, row identities, amounts, fields, stated
    totals and stored reconciliation against the supplied artifact.
    Restoration does not rerun the price or certify stored market
    facts. Reconcile native producers, Python/WASM results and tables,
    schemas, generated declarations, UI and parity. Portfolio event
    ladders retain their separate eligibility and meaning.

## Observed behavior

### Schedule, clock and fixing algorithm

Ordinary schedule PV delegates dated flows to core NPV on the discount curve's own day count, excluding payment dates on/before valuation. An empty schedule returns zero only when the provider supplies a notional and therefore a currency; otherwise it errors. The raw variant extracts amounts and avoids final Money rounding. The distinct calibration/trade raw helper includes same-day initial exchanges at unit DF, excludes only earlier flows, and uses compensated summation. It still requires the discount curve for an empty schedule.

Term-index validation compares the curve's tenor with the contractual tenor in years, tolerance `1e−8`. A projected term fixing converts its reset date to signed time from the **curve base date** using the curve day count, then reads `curve.rate(t)`. This helper performs no historical-series lookup itself. For ACT/ACT ICMA, the first supplied payment date strictly after valuation anchors a quasi-coupon period one contractual month/year frequency earlier, preserving end-of-month. Unsupported day/week frequencies, no future payment, date arithmetic failure or non-ICMA day count return no inferred period; the downstream frequency-only day-count path can then fail. Input dates are assumed ordered; the helper takes the first matching date, not the minimum.

Payment delay adds strictly positive business days through an explicitly named strict calendar, or weekdays when the calendar is absent. Nonpositive delay returns the original date without resolving a supplied calendar. The shared par-annuity threshold is `1e−12`.

The current Money and scalar schedule helpers now take explicit
`include_today`. The deposit's trade raw path uses that same helper with
same-day cash included. The generic export still treats all dates on or
before valuation as excluded; the narrower helper change does not
reconcile the export or fully qualify raw currency admission.

### Export algorithm and conventions

1. Accept only a registered Discounting or HazardRate pair that passes the instrument's cashflow-export eligibility hook. Price once through that selected registry and use its effective date and scenario-adjusted Money PV as reconciliation target. The direct exporter passes `mark_instrument_validated` after the export hook, relying on the documented already-validated-instrument precondition; it does not independently invoke generic `validate_for_pricing` here.
2. Build one schedule and its optional pool/index/balance metadata. Require a declared discount curve even if terms also specify an override. Select per-currency discount curves where supplied; otherwise use the selected default. Hazard mode selects the first declared credit curve and its recovery metadata.
3. Compute date-based relative DFs. Retain past/same-day rows as audit history with DF one and native PV zero. Delegate actual native credit-adjusted row PV to the cashflows kernel. For surviving rows, display conditional survival `Q(T)=clamp(S(T)/S(as_of),0,1)` and nonnegative sequential probability differences `max(Q_previous−Q(T),0)`. Zero/nonfinite survival at valuation fails. These columns follow schedule order; duplicate payment dates have zero subsequent displayed increment, while the delegated kernel owns allocation of default recovery.
4. Convert each native PV through Money into reporting currency using **valuation-date FX**, then apply scenario price shock. Default reporting currency is the first flow's currency; explicit terms can provide it for an empty schedule. Per-flow conversion can round Money independently. Sum row PV with Neumaier compensation and require currency equality plus `|sum−canonical|≤max(1e−6,|canonical|×1e−6)`. Otherwise fail; the exporter never labels a nonreconciling envelope successful.
5. Enrich pool state by payment date, schedule balances by flow position, accrual notional by schedule balance at accrual start, and index ratios by flow date. Pool balances take precedence over schedule balances. Accrual-start balance lookup converts **any** replay error into absent metadata. Missing index-ratio entries remain absent; the adjacent comment promises logging but this lookup contains no warning. These missing fields do not independently invalidate a reconciling PV.
6. Calendar-year ladders group by row currency in first-appearance order and aggregate unshocked native PV, preserving its unit with native amounts. They do not sum reporting-currency/scenario-adjusted PV into native ladders. Pricing explanations use the same export; an unavailable trace becomes a computation-step reason instead of failing the already obtained price.

## Limitations, alternatives

The export is a static decomposition; tree/MC/PDE/exercise value cannot generally be represented by these rows. Reconciliation is a production-kernel consistency check, not independent price validation, and the relative tolerance is one part per million rather than a universal minor-currency-unit guarantee. Possible stricter metadata failures, cashflow-date FX, or model-specific stochastic attribution require separate decisions. Generic raw PV excludes currency checking inherent in the Money path and requires consistent single-currency provider economics.

## Evidence

[cashflow_export.rs](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs), [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs), [term_fixings.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/term_fixings.rs), [coupon_period.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/coupon_period.rs), [swap_legs.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/swap_legs.rs), and [instrument trait](../../../../finstack-quant/valuations/core/src/instruments/common_impl/traits/instrument.rs) own the inspected behavior. Inline tests cover ladder currency grouping, compensated sums and calendar failures; product export tests supply reconciliation evidence. No test or external market comparison was executed. Underlying credit integration is covered by the [cashflows credit-PV record](../cashflows/15-credit-adjusted-pv.md).

The [hazard bond pricer](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/hazard.rs)
integrates recovery over daily intervals. It does not authorize generic
payment-row recovery allocation. The current
[Python reporting projection](../../../../finstack-quant-py/finstack_quant/reporting/instrument.py)
can group reporting PV under the native row currency. The existing
[Python restore](../../../../finstack-quant-py/src/bindings/valuations/pricing.rs)
and [WASM ladder ingest](../../../../finstack-quant-wasm/src/api/valuations/pricing.rs)
need canonical semantic validation as well as structural parsing.

### Independent review evidence

- With a same-day outflow of `100` and future inflow `105` at
  relative factor `.95`, excluding the same-day row gives `99.75`
  holder PV; including it gives `−.25` trade PV. With source factors
  `.95` at valuation and `.90` at payment, the relative factor is
  `18/19`, not `.90`.
- A qualified term observation with `P(S)=.98`, `P(E)=.96` and
  `α=.25` gives `1/12` annual decimal rate. For a separately
  admitted four-day ICMA measurement after July 31, 2025, the
  semiannual nominal July 31–January 31 span gives `4/(2×184)`.
  Inferring the reference from an August 4 payment gives
  `4/(2×181)`. An annuity of `1e−14` with a price difference
  of `2e−16` can give a representable replacement quote of 200 bp.
- Absolute integrated intensities of `1000` and `1002` give
  absolute survivals that both round to zero in binary64, but local
  conditional survival is `exp(−2)≈.13533528`. Local default mass
  over intensity `1e−20` is nonzero even when subtracting rounded
  survivals gives zero. At raw CPI ratio `.95`, a coupon on base
  `2` pays `1.90` while a principal-only floor keeps redemption
  of `100` at par on the same payment date.
- Native PV components of `10` and `5` EUR total `15` EUR.
  At USD/EUR `1.20` and scenario multiplier `1.10` the matching
  adjusted total is `19.80` USD. With illustrative cent rounding,
  rounding `.004+.004` only at finalization gives `.01`;
  rounding each row first gives zero. Money values `10^16` and
  `10^16+1` differ by one but convert to the same binary64.
  An adjustment multiplier of zero also hides any unadjusted
  discrepancy from a final-total-only check.

These are standalone exact or bounded numerical examples. They do not
run the production pricers, exporters, trace, or hosts.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
