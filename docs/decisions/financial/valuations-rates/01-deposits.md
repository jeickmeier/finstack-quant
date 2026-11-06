# Deposit cashflows and quote/discount inversion

- ID: VALUATIONS-RATES-01-DEPOSITS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

Deposits are two-date simple-interest lending cashflows. Contractual fixed rate, curve-implied par rate and quote-implied terminal discount factor are distinct outputs. The instrument permits an absent fixed rate for quote/calibration use, but constructing payable cashflows requires it; there is no implicit zero-rate coupon.

## Algorithm and conventions

1. Require positive notional and raw maturity after start. Adjust both endpoints only if a calendar ID is supplied, using ModifiedFollowing by default; without a calendar leave both dates unchanged. Require adjusted maturity after adjusted start.
2. Compute alpha with the instrument day count on adjusted dates. Generate `-N` at start, `+N` principal and `N*r*alpha` interest at maturity. Rates are decimal. Discounting and valuation-date filtering are delegated to shared schedule-PV helpers.
3. DfStart/DfEnd use the instrument's first declared discount dependency and `D(as_of,effective_date)`. DepositParRate is `(DfStart/DfEnd-1)/alpha`, rejecting `abs(alpha)<1e-6` or nonpositive terminal DF.
4. DfEndFromQuote is `DfStart/(1+r*alpha)`, rejecting a nonpositive denominator. QuoteRate returns the contractual quote, not a market-implied number. YearFraction repeats the adjusted-date calculation. Generic combined DV01 and triangular bucketed DV01 bump the discount curve rather than changing the contractual quote.

## Selection, limitations and failures

The discounting model is implicit for this deterministic product. Missing fixed rate, curve, metric dependency or unresolved supplied calendar errors. Fixed rates outside [-10%,100%] only warn. The cashflow construction does not apply the quote-inversion denominator guard, so highly negative rates can construct a negative terminal redemption while DfEndFromQuote rejects the same quote. The inherited schedule helper's same-day cashflow convention is material and must be preserved when interpreting price versus trade NPV.

## Alternatives and tradeoffs

Compounded or continuously compounded deposit quotes would require explicit conversion, not reuse of the simple-rate formula. A no-calendar unadjusted contract is supported; silently inventing a market calendar is not. Distinguishing curve-derived par from quote-derived DF supports calibration without confusing an observed quote with a value.

## Evidence

- [Deposit implementation](../../../../finstack-quant/valuations/rates/src/deposit/types.rs), [metric registration and year fraction](../../../../finstack-quant/valuations/rates/src/deposit/metrics/mod.rs), [DF lookup](../../../../finstack-quant/valuations/rates/src/deposit/metrics/df.rs), [par rate](../../../../finstack-quant/valuations/rates/src/deposit/metrics/par_rate.rs), [quote inversion](../../../../finstack-quant/valuations/rates/src/deposit/metrics/df_end_from_quote.rs).
- [Deposit pricing tests](../../../../finstack-quant/valuations/tests/instruments/deposit/pricing.rs), [par-rate tests](../../../../finstack-quant/valuations/tests/instruments/deposit/metrics/par_rate.rs), [DF tests](../../../../finstack-quant/valuations/tests/instruments/deposit/metrics/discount_factors.rs): formula and inversion evidence.
- [ISDA day counts](../../../REFERENCES.md#isda-2006-definitions), [Hull](../../../REFERENCES.md#hull-options-futures): reference context; calendar and admissible-rate choices still require adoption.

## Evidence limits

Formula tests are not a complete cross-market convention benchmark.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
