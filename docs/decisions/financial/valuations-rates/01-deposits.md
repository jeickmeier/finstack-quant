# VALUATIONS-RATES-01-DEPOSITS: Deposit cashflows and quote/discount inversion

- ID: VALUATIONS-RATES-01-DEPOSITS
- Decision status: **Accepted — complete checked deposit cashflows and quote inversion package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked deposit cashflows and quote inversion

1. **Resolve one contractual period.** Use a single-period, deterministic
   lending deposit in one currency with positive exact Money notional `N`.
   Require ordered authored start and maturity dates or qualified terms
   materialized from an explicitly selected convention. Select either exact
   unadjusted dates or a qualified calendar and business-day adjustment.
   Resolve effective dates `S<E` once and preserve the original dates,
   resolution and convention identity. Remove the default Modified
   Following, implicit calendar selection and currency-derived spot rules
   from the product. The quote materializer owns any trade-to-spot lag.
   Reject an unresolved calendar, a date collapse, or an unsupported
   convention. See [dated convention selection](../valuations-framework/convention-selection.md)
   and [UK Money Markets Code, annex 2](https://www.bankofengland.co.uk/markets/money-markets-committee-and-uk-money-markets-code/the-uk-money-markets-code).

2. **Use the same accrual, simple growth and currency throughout.** Compute
   `α` on the effective accrual interval with the selected contractual day
   count and every required context field. Require finite `α>0`; even
   distinct dates can produce zero. A day count requiring a calendar or
   nominal coupon context must receive that exact context or fail. Keep
   the contractual simple annual decimal rate `r` exact in Decimal.
   Quoted and payable terms share the domain `G=1+rα>0`, with checked
   conversion and representability. Negative and zero rates are admissible
   in that domain. Remove the generic rate warning range and arbitrary
   small-accrual cutoff. Admit the full-period accrual once through the
   fixed-coupon Decimal boundary and use that positive represented factor
   for cash, quote and discount inversion, with a checked binary64 view
   where needed. See
   [coupon context](../core-conventions/002-coupon-context-day-counts.md),
   [simple compounding](../core-numerics/compounding.md) and
   [Bank of England Working Paper 356, appendix C](https://www.bankofengland.co.uk/-/media/boe/files/working-paper/2008/measuring-monetary-policy-expectations-from-financial-market-instruments.pdf).

3. **Emit complete contractual cash and principal effects.** For a
   supplied rate emit funding `−N` at `S`, principal redemption `+N` and
   signed simple interest `N r α` at `E`. Calculate the complete interest
   product directly and restore Money once under the accepted bounded
   fixed-coupon arithmetic. Check the emitted terminal sum is positive.
   Preserve a genuine zero interest row and its rate and accrual context.
   Keep opening principal `N` immediately before the start events, zero
   additional principal delta on the funding row, and delta `−N` on
   redemption. Keep the contractual schedule and eligible valuation cash
   as distinct views. See [cashflow event types](../core-conventions/015-cashflow-semantics.md),
   [fixed coupon amounts](../cashflows/05-coupon-cash-and-pik.md)
   and [schedule views](../cashflows/12-schedule-lifecycle.md).

4. **Fix holder and trade settlement cuts.** At valuation date `V` the
   ordinary holder mark includes only settlement after `V`. The explicit
   pre-settlement trade view includes eligible same-day cash at factor one,
   with earlier cash excluded. Inception and calibration use that trade
   basis at or before `S`. Between funding and maturity the holder carries
   only the terminal cash. On `E` the holder mark is zero and the
   pre-settlement trade view still carries same-day redemption and
   interest. After `E` both marks are zero. Validate the admitted
   contract and required rate before shortcuts. Skip unused curve lookups
   when no selected future cash needs discounting. Discount using the
   exact native-currency source and its dated relative factors. Use the
   same eligible contractual cash for raw and Money PV. Raw discounts
   before final PV monetary conversion and still respects the admitted
   contractual cash. See [dated NPV](../core-conventions/016-discounting.md),
   [shared cash valuation](../valuations-framework/cashflow-export.md)
   and [Strata term deposit pricer](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/deposit/DiscountingTermDepositProductPricer.html).

5. **Separate contractual quotes, curve observations and inception inverses.**
   Keep `Yf=α` and `QuoteRate=r` with a checked numeric view of the
   stored quote. `DfStart` and `DfEnd` are `D(V,S)` and `D(V,E)` from
   the exact discount binding on its source clock, including when it
   differs from the contractual clock. An explicit dated factor request
   obeys the source domain and identity rules. Require finite positive
   factors and allow factors above one. At or before start on the
   origination trade basis, use
   `r_par=(D(V,S)/D(V,E)−1)/α` and
   `DfEndFromQuote=D(V,S)/(1+rα)`. The latter returns a factor from `V`
   to `E`, with the observed start factor held fixed. Use checked stable
   arithmetic that retains a near-par difference where representable.
   Check the represented par quote's growth and reprice it against an
   explicit qualified native-currency residual budget on the trade basis.
   After start the origination par and quote-implied end factor are
   unavailable. The standalone dated factors, contractual fraction and
   authored quote keep their own applicability. A rate-less request
   supports the qualified fraction, dated factors and inception par
   through checked quote analytics. Cash, PV, price risk, quote
   passthrough and quote inversion require an authored rate. Keep the
   ordinary price-with-metrics path subject to its headline price
   requirement. See [dated discount sources](../core-market/discount-evaluation.md)
   and [Strata term deposit pricer](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/deposit/DiscountingTermDepositProductPricer.html).

6. **Retain qualified direct discount risk.** Keep parallel and triangular
   key-rate modes as separate signed finite differences in native
   currency per positive bp, using the selected central signed reprices.
   Use the exact active discount source and
   explicit represented shock controls. Reprice the same fixed contract
   under the same date and cash cut. A fully settled holder has zero
   remaining economic sensitivity. Publish any requested rows only with
   a qualified reporting grid. A separately selected market-quote replay
   uses exact quote identity, an admitted center and the authorized
   calibration provider. Missing recipes or failed work propagate and
   direct discount risk stays a separate result. See
   [shared rate risk](../valuations-framework/finite-difference-rates.md).

7. **Keep calibration and public ingress on the same terms.** Discount
   calibration uses the exact materialized deposit and checked raw trade
   PV on its existing normalized residual basis. Preserve the date,
   accrual and convention provenance. Reject a mismatched materialized type
   or basis. Forward calibration may consume the same resolved quote
   period with its qualified index accrual in the separate projection
   growth equation. Choice of quote instruments, solver controls, weights
   and acceptance remain with the calibration records. Reuse native
   validation through the existing builder, direct restore, tagged
   instrument and quote wires, Python and WASM routes and schema. Preserve
   exact rates, date intent and output identity. Align the stated
   `DfEndFromQuote` basis and qualify existing pricing and calibration
   fixtures. See
   [quote materialization](../calibration-framework/04-deposit-fra-materialization.md),
   [rate residuals](../calibration-targets/02-rate-preparation-and-residuals.md)
   and [forward calibration](../calibration-targets/04-forward-calibration.md).

## Observed behavior

### Choice, scope and rationale

Deposits are two-date simple-interest lending cashflows. Contractual fixed rate, curve-implied par rate and quote-implied terminal discount factor are distinct outputs. The instrument permits an absent fixed rate for quote/calibration use, but constructing payable cashflows requires it; there is no implicit zero-rate coupon.

### Algorithm and conventions

1. Require positive notional and raw maturity after start. Adjust both endpoints only if a calendar ID is supplied, using ModifiedFollowing by default; without a calendar leave both dates unchanged. Require adjusted maturity after adjusted start.
2. Compute alpha with the instrument day count on adjusted dates. Generate `-N` at start, `+N` principal and `N*r*alpha` interest at maturity. Rates are decimal. Discounting and valuation-date filtering are delegated to shared schedule-PV helpers.
3. DfStart/DfEnd use the instrument's first declared discount dependency and `D(as_of,effective_date)`. DepositParRate is `(DfStart/DfEnd-1)/alpha`, rejecting `abs(alpha)<1e-6` or nonpositive terminal DF.
4. DfEndFromQuote is `DfStart/(1+r*alpha)`, rejecting a nonpositive denominator. QuoteRate returns the contractual quote, not a market-implied number. YearFraction repeats the adjusted-date calculation. Generic combined DV01 and triangular bucketed DV01 bump the discount curve rather than changing the contractual quote.

### Selection, limitations and failures

The discounting model is implicit for this deterministic product. Missing fixed rate, curve, metric dependency or unresolved supplied calendar errors. Fixed rates outside [-10%,100%] only warn. The cashflow construction does not apply the quote-inversion denominator guard, so highly negative rates can construct a negative terminal redemption while DfEndFromQuote rejects the same quote. The inherited schedule helper's same-day cashflow convention is material and must be preserved when interpreting price versus trade NPV.

### Alternatives and tradeoffs

Compounded or continuously compounded deposit quotes would require explicit conversion, not reuse of the simple-rate formula. A no-calendar unadjusted contract is supported; silently inventing a market calendar is not. Distinguishing curve-derived par from quote-derived DF supports calibration without confusing an observed quote with a value.

## Evidence

- [Deposit implementation](../../../../finstack-quant/valuations/rates/src/deposit/types.rs), [metric registration and year fraction](../../../../finstack-quant/valuations/rates/src/deposit/metrics/mod.rs), [DF lookup](../../../../finstack-quant/valuations/rates/src/deposit/metrics/df.rs), [par rate](../../../../finstack-quant/valuations/rates/src/deposit/metrics/par_rate.rs), [quote inversion](../../../../finstack-quant/valuations/rates/src/deposit/metrics/df_end_from_quote.rs).
- [Deposit pricing tests](../../../../finstack-quant/valuations/tests/instruments/deposit/pricing.rs), [par-rate tests](../../../../finstack-quant/valuations/tests/instruments/deposit/metrics/par_rate.rs), [DF tests](../../../../finstack-quant/valuations/tests/instruments/deposit/metrics/discount_factors.rs): formula and inversion evidence.
- [ISDA day counts](../../../REFERENCES.md#isda-2006-definitions), [Hull](../../../REFERENCES.md#hull-options-futures): reference context; calendar and admissible-rate choices still require adoption.

## Evidence limits

Formula tests are not a complete cross-market convention benchmark.

## Independent review evidence

- From 2025-01-31 to a nominal Sunday 2025-08-31, explicitly selected
  weekends-only Modified Following resolves the end to 2025-08-29.
  Actual/360 is `210/360=7/12`. Saturday–Sunday dates can collapse to one
  Monday under that convention. Also, 30E/360 gives zero from January 30
  to January 31 even though the dates differ.
- For `N=1,000`, `α=0.5`, `r=0.04` and `D(S,E)=50/51`, the
  inception trade mark is zero and the post-funding holder mark at
  `S` is 1,000. On `E` the holder mark is zero and the pre-settlement
  trade cash is 1,020. Neither valuation on `E` needs a curve query.
- With `α=0.5`, a −6% rate pays 970 on notional 1,000 and is admitted.
  Rates of −200% and −210% would pay zero and −50, respectively, and fail.
  For notional `10^10`, rate `10^−18` and `α=0.5` the directly
  calculated interest is `5×10^−9`. Binary64 `1+rα` rounds to one and
  a redemption-minus-notional calculation then loses that interest.
- With accrual `92/360`, `D(V,S)=0.9996` and `D(V,E)=0.9901`, the
  par rate is approximately 3.75456146%. The same quote recovers
  `D(V,E)=0.9901`. Setting the start factor to one would give
  approximately 3.91264826%.

These are bounded checks of dates, cash, lifecycle and algebra. The
existing same-day trade and holder pricing tests, inverse tests and
QuantLib-based host fixture remain production reconciliation evidence
to qualify on the accepted basis. [Existing par tests](../../../../finstack-quant/valuations/tests/instruments/deposit/metrics/par_rate.rs),
[fixture generator](../../../../scripts/golden/quantlib/deposits.py),
[discount calibration adapter](../../../../finstack-quant/calibration/src/build/prepared.rs).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
