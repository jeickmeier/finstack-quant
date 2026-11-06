# Repo lending cashflows, collateral and margin proxies

- ID: VALUATIONS-RATES-15-REPO
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

A repo is valued from the cash lender's contractual cashflows, with simple interest and an optional special-collateral rate adjustment. Collateral affects coverage and margin reporting, not the contractual cashflow PV. Term, overnight and open contracts use the supplied effective maturity; open-repo termination optionality is not modeled.

## Algorithm and controls

1. Require finite positive cash and collateral quantity, haircut in `[0,1)`, finite special adjustment, and ordered raw/adjusted dates. A resolvable calendar is mandatory. Adjust start and maturity with the selected business-day convention.
2. Effective decimal rate is `repo_rate + special_adjustment_bp/10000` when present. Interest is `cash*rate*year_fraction(adjusted_start,adjusted_end)`; the notional repayment is cash plus interest.
3. Export negative initial cash, positive terminal principal and positive signed interest. Discount remaining dated flows with the discount curve relative to valuation; PV is zero on/after adjusted maturity.
4. Required collateral is `cash/(1-haircut)`. Actual collateral value is quantity times a currency-bearing price; unitless prices error. Coverage is actual/required value. The named implied collateral return is `(actual/required-1)/remaining_tau`, returning zero when tau or required value is zero/nonpositive; it is a static collateral excess annualization, not a forecast of collateral total return.
5. Accrued interest clamps the accrual endpoint to the adjusted contract interval. FundingRisk is the one-sided PV difference after +1 bp contractual repo rate; DV01 instead bumps market rate curves.
6. Margin uses a bilateral synthetic netting-set identifier and an IR delta proxy `-cash*remaining_calendar_years*1bp`, placed entirely in 3M or 6M according to remaining tenor. This proxy ignores the market and is not the repriced DV01. VM uses actual repo PV.

## Conventions, selection and failures

Convenience constructors obtain USD general-collateral defaults from the embedded contract registry; other currencies require explicit conventions. Overnight starts Following-adjusted and matures one business day later. Negative finite repo rates are allowed. Haircut is a fraction. The signed rate adjustment is in bp. Day count governs interest; margin uses calendar-year scaling and unadjusted maturity.

## Alternatives and tradeoffs

A cashflow-derived curve sensitivity could replace the margin duration proxy; contractual funding-rate sensitivity and discount-curve DV01 should remain distinguishable. Open termination optionality and collateral income/price dynamics are unimplemented, economically distinct models.

## Evidence

- [Repo implementation](../../../../finstack-quant/valuations/rates/src/repo/types.rs), [metrics](../../../../finstack-quant/valuations/rates/src/repo/metrics/mod.rs), [accrued interest](../../../../finstack-quant/valuations/rates/src/repo/metrics/accrued_interest.rs), [collateral return](../../../../finstack-quant/valuations/rates/src/repo/metrics/implied_collateral_return.rs), [margin proxy](../../../../finstack-quant/valuations/rates/src/repo/marginable.rs).
- [Repo pricing](../../../../finstack-quant/valuations/tests/instruments/repo/pricing.rs), [collateral tests](../../../../finstack-quant/valuations/tests/instruments/repo/collateral.rs), [margin tests](../../../../finstack-quant/valuations/rates/tests/repo/margin.rs): formula, convention and proxy behavior evidence.
- [Hull](../../../REFERENCES.md#hull-options-futures) provides general financing context. A specific adopted repo margin-proxy validation reference was not established; the existence of [SIMM](../../../REFERENCES.md#isda-simm) does not validate this approximation.

## Evidence limits

Collateral and required collateral metrics discard currency before division; no conversion or same-currency check is present in the local coverage/return calculators.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
