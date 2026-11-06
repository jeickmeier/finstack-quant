# VALUATIONS-OTHER-FIXED-INCOME-03: Convertible inverse quotes, sensitivities and spot regimes

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[Engine](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/engine.rs) and [metric registration](../../../../finstack-quant/valuations/fixed-income/src/convertible/metrics/mod.rs) compute finite differences and inverse prices on the lattice in [record 02](02-convertible-lattice.md). Quoted clean percent is converted to settlement dirty money; diagnostics preserve distinct price, conversion parity and bond-floor concepts. Method adoption and bump accuracy are open.

## Algorithm and controls

- Settlement advances optional business days on the coupon calendar and applies its business-day convention. Accrued is linear schedule accrual including PIK. Dirty settlement value subtracts survival-weighted interim cash from as-of PV, then divides by the risk-free settlement DF. CleanPrice reports dirty minus accrued in money, not percent.
- Delta/Gamma use centered spot differences; Vega uses absolute vol bumps and is scaled to one volatility percentage point. If the down-vol repricing returns any Validation error, it falls back to the base price and the actual one-sided width. Rho is currency per basis point of risk-free curve bump. Theta is one calendar-day roll-forward PV minus current PV when time remaining exceeds `1/365.25`; direct Greek result carries theta, while registry installation lists Delta/Gamma/Vega/Rho.
- Dividend01 uses ±1 bp dividend yield with a zero floor on the down shock and rescales by actual width; no dividend identifier returns zero. Conversion01 bumps explicit ratio ±1%, or price ∓1% when only price exists, returning half the PV difference. If both ratio and price are populated, it changes ratio alone and can violate the validated ratio/price consistency contract.
- With a credit curve, CS01 delegates to shared hazard repricing. Without one, [CS01](../../../../finstack-quant/valuations/fixed-income/src/convertible/metrics/cs01.rs) creates a synthetic zero-hazard, zero-recovery curve and takes a one-sided +1 bp hazard change, parallel or triangular key-rate buckets. This changes conditional equity drift and survival on the whole defaultable-equity value, not only a separately discounted cash component, despite the fallback log description.
- [Implied volatility](../../../../finstack-quant/valuations/fixed-income/src/convertible/metrics/implied_vol.rs) searches the lattice-admissible lower bound starting at `1e-8`, doubling then bisecting validity for 32 iterations, with upper vol `3.0`. Brent uses `1e-12`, 100 iterations; repricing residual per notional must be ≤`1e-10`. Same-sign endpoint residuals error.
- [OAS](../../../../finstack-quant/valuations/fixed-income/src/convertible/metrics/oas.rs) solves a parallel **hazard-rate** shift when credit is present, otherwise a discount-curve shift. It reprices settlement dirty value on every trial. Lower bounds respect nonnegative hazard/forward-curve floors, upper bound `.50`, tolerance `1e-8`, 100 iterations. This output has different economic meanings in the two branches; it is not a cash-component z-spread.
- Bond floor removes conversion/call/put exercise and rolls contractual payments with survival and recovery. Conversion value is instantaneous payoff; parity divides by face; premium compares price to conversion value. [Spot ladder](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/spot_ladder.rs) defaults to spot multiples `.25,.5,.75,1,1.25,1.5,2,3`; sorts/deduplicates positive supplied spots; computes delta/effective-ratio hedge ratio; labels below `.40` Busted, above `.80` EquitySensitive, otherwise Balanced. Mandatory-variable ratio regimes are absent rather than fabricated.

## Scope, limitations and alternatives

All monetary risks inherit lattice discontinuities and chosen bumps. Greek calculations use the native engine, while inverse-vol metric reprices through the metric context; arbitrary model overrides are not a separate accepted calibration contract. OAS/hazard and synthetic-CS01 terminology differs from the actual whole-value credit behavior described above. The one-sided Vega fallback catches validation more broadly than solely an invalid CRR probability. Spot-regime thresholds are presentation heuristics, not ratings or investment advice.

Analytic derivatives, smoothed Greeks, a finer adaptive lattice, or explicitly named hazard/discount spread metrics are alternatives needing separate approval. A generic cross-factor metric is also registered for spot-vol, spot-credit, rates-credit and credit-vol; the shared metric kernel controls mixed-bump arithmetic.

## Evidence

[Pricing tests](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/tests.rs) and colocated inverse-quote/spot-ladder tests provide finite-difference, round-trip and limiting-case evidence. [Convertible delta bands](../../../REFERENCES.md#convertible-delta-bands) supplies context for labels; [Brent](../../../REFERENCES.md#brent-1973) for inverse solves. No independent entire-surface Greek certificate was established.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
