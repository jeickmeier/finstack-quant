# VALUATIONS-OTHER-FIXED-INCOME-01: Bond-futures conversion factors, delivery carry and listed marks

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

The registered `BondFutureCleanPriceProxy` uses a caller-selected cheapest-to-deliver bond and its supplied basket conversion factor. It computes a carry-adjusted CTD forward clean price; it does not optimize the delivery basket. The method name acknowledges this proxy scope. [Types](../../../../finstack-quant/valuations/fixed-income/src/bond_future/types.rs) and [pricer](../../../../finstack-quant/valuations/fixed-income/src/bond_future/pricer.rs) own selection and formulas; use as an exchange-complete futures model is not adopted.

## Algorithm

1. Resolve CTD identifier by explicit `ctd_bond_id`, embedded CTD identifier, or the sole basket member. Multiple unresolved deliverables error. The embedded bond must match that identifier and have a positive finite basket conversion factor.
2. `calculate_conversion_factor` is a separate helper, not an automatic replacement for the supplied basket factor. It obtains fixed coupon `c` and the earliest call start or maturity. Measure whole months from the first day of delivery month; long contracts (`standard_maturity_years≥6`) round residual months down to a quarter, short contracts retain whole months. With notional coupon `g`, use `a=(1+g/2)^(−v/6)`, `b=(c/2)(6−v)/6`, `d=(c/g)(1−C)`, where `C=(1+g/2)^(−2n)` or exponent `−(2n+1)` according to residual-month branch. Factor is `a(c/2+C+d)−b`, rounded half-away to four decimals.
3. Set delivery date to `max(delivery_start,as_of)`. Price the embedded bond for spot dirty value; subtract the financing-curve PV of interim cashflows in `(as_of,delivery]`, divide by financing DF to delivery, and subtract delivery accrued. Divide clean money by bond notional, multiply by 100, and divide by conversion factor.
4. Financing uses `repo_curve_id` if supplied, otherwise the future's discount curve. The CTD's own pricing curves still determine spot value.
5. Listed terms convert model points into signed contracts × multiplier P&L relative to entry price and handle final-settlement lifecycle. `FuturesPrice` instead calls `mark_price`, which can follow a supplied live mark or official final settlement. `ConversionFactor` reports the resolved basket factor. Parallel and bucketed DV01 use shared repricing.

## Scope, controls and conventions

Fixed-rate deliverables are required for the CME-style factor helper. Factors are dimensionless; futures prices are points per 100 face; nominal exposure is contracts × multiplier × 100. Date validation requires last trading before delivery start before settlement. Contract repo day count must be supported even though dated financing DFs determine carry. The family registers this proxy only; no runtime gate selects a delivery-option model.

## Limitations, failure and alternatives

No stochastic CTD switching, quality option, timing option, wild-card option or futures/forward convexity adjustment is computed. Conversion factor conventions cannot be assumed identical for every exchange merely because a contract specification fits this function. The price path resolves the embedded CTD before listed lifecycle pricing, even where final settlement could otherwise avoid model inputs. Physical delivery cannot be represented by the generic cashflow exporter and is rejected. An exchange-specific basket optimization or stochastic delivery model is an unimplemented alternative; quoted marks remain distinct from fair-model price.

## Evidence

[Bond-future tests](../../../../finstack-quant/valuations/fixed-income/tests/bond_future/mod.rs) and colocated tests in [types](../../../../finstack-quant/valuations/fixed-income/src/bond_future/types.rs) cover carry, factors, lifecycle and validation examples; these were not rerun. [Hull](../../../REFERENCES.md#hull-options-futures) and [fixed-income handbook](../../../REFERENCES.md#fabozzi-fixed-income-handbook) are context. No independent delivery-option benchmark was established.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
