# CALIBRATION-FRAMEWORK-009: Cross-currency quote leg construction

- Decision status: **Accepted 2026-11-15 — complete cross-currency quote, FX authority, dated cash, foreign-curve pillar and replay package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent principal sizing, FX bridge, reset cash and dated support checks completed.** Production pricing, calibration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for cross-currency swap calibration and FX authority

1. **Bind the exact quote and contract.** State the ordered pair as
   quote currency per base unit, opposite leg directions, the explicit
   leg bearing the signed Decimal bp quote and the companion spread.
   Qualified dated terms may elect receive base, pay quote with
   zero spread on the quote leg. Require an explicit principal
   exchange mode. In MtM mode require the resetting leg and the
   explicit initial-notional election, fixed or governed by the
   same reset law as later periods.
   [Accepted cross-currency contract](../valuations-rates/16-cross-currency-swaps.md#accepted-decision-for-checked-cross-currency-swap-cashflows-fx-resets-and-risk).

2. **Separate principal sizing from valuation FX.** In the FX-equivalent
   profile, size fixed base principal once as `N_quote / S_P`
   using authorized spot `S_P` at its actual value date `P`.
   Freeze booked amounts and the positive exact quote-currency
   normalization notional. Bind one authorized valuation FX primitive
   on the elected common collateral basis. A directly supplied cash FX
   remains fixed across trials. With a later-value spot primitive,
   hold that quote fixed and compute
   `X_V = S_P × D_quote(V,P) / D_base(V,P)` for each trial at
   valuation `V`. At an eligible pending reset's actual FX
   reference value date `L`, use
   `X_L = X_V × D_base(V,L) / D_quote(V,L)` and the admitted
   reset law. Keep published fixings and booked initial amounts fixed.
   Remove substitution of step cash FX for missing sizing spot.
   [Ordered FX and value dates](../core-conventions/011-fx-date-and-quote-conventions.md) ·
   [BIS covered-interest-parity analysis](https://www.bis.org/publ/qtrpdf/r_qt1609e.pdf).

3. **Resolve each leg once.** Resolve spot from the qualified trade date
   and joint settlement calendar. Distinguish spot-relative tenor,
   nominal far date subject to its elected roll and exact effective
   date. Preserve each leg's accrued periods, payment and fixing rules,
   stub, EOM, lags, calendars and day-count context. Keep FX fixing,
   reference value, notional effective and payment dates distinct.
   Delayed coupons retain their own period notional. Keep actual dates
   fixed in preparation.
   [ARRC interdealer recommendations](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2020/Recommendations_for_Interdealer_Cross-Currency_Swap_Market_Conventions.pdf) ·
   [Strata FX observation dates](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/index/FxIndexObservation.html).

4. **Bind exact economic sources and a common entitlement cut.** Bind
   both native discount evaluators to the elected collateral basis.
   Bind each rate index, published fixing and projection role by its
   actual identity. Select known rate and FX fixings at the qualified
   knowledge cut and project only expressly pending observations.
   Due missing fixings or unresolved active sources fail. Use the
   accepted term and compounded overnight cash rules, including
   spread outside overnight compounding. Emit elected principal
   exchanges once on their actual dates. Include same-day cash only
   under explicit entitlement for known payable amounts at factor one.

5. **Use the latest date required by the fitted foreign curve as the
   quote pillar.** Preserve contractual far date and last scheduled
   cash date separately. Include dates of eligible foreign cash, FX
   spot bridge or pending reset value dates that consume the fitted
   evaluator, and benchmark endpoints where that evaluator explicitly
   supplies projection. A later domestic-only payment does not
   advance the foreign pillar. Compute its positive finite coordinate
   on the explicit target base, clock and context. Require coverage at
   every needed source date in trial and final pricing.
   [Accepted foreign discount calibration](../calibration-targets/07-xccy-calibration.md#accepted-decision-for-collateral-qualified-foreign-discount-calibration).

6. **Use one signed native ledger for fit and exact replay.** Discount
   eligible cash in native currency, translate at valuation-date cash
   FX and divide raw quote-currency PV by the frozen positive
   normalization notional. Preserve contractual cash rounding and
   require a resolvable quoted-leg spread effect and genuine dependence
   on the fitted curve. Freeze exact quote identity, terms, schedules,
   fixings and typed role bindings for qualified replay. Use common
   Rust admission across hosts. The target owns solver and fit policy.
   Dated zero-rate differences remain a separate diagnostic.
   [Accepted calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification) ·
   [Accepted basis diagnostics](../core-market/basis-spreads.md#accepted-decision-source-derived-dated-zero-rate-differences).

## Independent calculation checks

- USD 130 and 1.25 USD/EUR later-value spot size initial principal at
  EUR 104. With factors 0.96 USD and 1.00 EUR to spot, cash FX is
  1.20 USD/EUR and the initial principal exchange has zero PV.
  Using sizing spot for translation gives −USD 5.20. At the next
  FX value date, factors 0.90 USD and 0.975 EUR give 1.30 USD/EUR,
  EUR 100 of replacement notional and a +EUR 4 rebalance for the
  receive-EUR leg.
- From January 5, 2026 under ACT/365F, a June 30 accrual end has
  time `176/365`. A July 2 foreign payment using the candidate
  reaches `178/365`. With all other active candidate dates earlier,
  that is the pillar even when the fixed domestic curve discounts
  another payment dated July 6 at `182/365`.

## Observed choice and rationale at the audit revision

XCCY calibration constructs a two-currency swap with FX-equivalent starting notionals. Projection IDs come from convention index IDs, while discount IDs come from explicit build roles. The design preserves market conventions separately from the curve being solved.

## Ordered construction and units

Validate quote and resolve XCCY convention plus both rate-index conventions. A missing quote-level `spot_fx` fails construction; the target may materialize it from its step parameter before calling this builder. Spot is quote currency per one base unit. Calibration context notional is quote-currency amount, so base notional is `N_quote / spot_fx`.

FX spot date uses pair settlement days, both currencies and supplied base/quote calendars. Far tenor is added unadjusted to spot then adjusted on the resolved joint calendar; explicit far dates are adjusted there too. The base leg receives, carries the basis-spread bp, uses base-index projection and foreign-discount role. The quote leg pays, has zero spread, uses quote-index projection and domestic-discount role. Both use convention payment frequency/day count/BDC, ShortFront stub, EOM false, their currency-specific payment/reset lags and compounding resolved from the index. `fixing_calendar_id=None` leaves downstream fixing-calendar selection to valuation.

The swap reports in quote currency and applies the convention's notional-exchange mode, including MTM modes where specified. Prepared quote pillar is leg-one accrual end, with a target-supplied curve-clock year fraction; payment lags can extend cashflows beyond that pillar.

## Failure and alternatives

Unknown convention/calendar, invalid quote FX, missing roles and invalid leg construction fail. No guessed spot or forward-curve fallback exists in this builder. Alternatives include a direct covered-interest-parity/basis formula or independent per-leg contractual schedules; the former loses reset/exchange richness and the latter needs more explicit data.

## Source and validation evidence

- Implementation: [build/xccy.rs](../../../../finstack-quant/calibration/src/build/xccy.rs) — joint calendars, notional conversion, leg roles and exchange selection.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — XCCY accrual-end pillar.
- Implementation: [build/context.rs](../../../../finstack-quant/calibration/src/build/context.rs) — explicit discount-role lookup.
- Test evidence: [src/build/xccy/tests.rs](../../../../finstack-quant/calibration/src/build/xccy/tests.rs) — FX-equivalent notionals and G10 MTM reset exchange assertions; inspected or located, not executed.
- The independent examples cover the selected date and FX formulas.
  Dated market term-sheet, curve-fit and host qualification remain open.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
