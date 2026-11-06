# Volatility-index futures and listed equity-option adapters

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-05`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

A volatility-index future reads a prebuilt volatility-index futures curve; it does not derive a futures quote by taking a square root of expected variance. Equity and volatility-index future options are instances of the shared future-option contract/kernel.

## Scope and input domain

Positive volatility-index entry/quote/settlement prices with listed contract terms. Listed options carry `FutureOptionTerms`, including explicit model, premium style, exercise and settlement terms; the equity wrappers add no separate formula.

## Algorithm

For the vol-index future, convert settlement date to the curve's base-date/day-count coordinate, floor time at zero and query `curve.price(t)`. Before settlement, shared mark resolution selects the quote/model. On settlement require `settlement_price`; after settlement PV is zero. PV is signed point delta times mark minus entry; `DeltaVol` is point delta per index point. The cashflow schedule has no residual and an entry-notional hint.

The two option wrappers invoke `define_future_option!`, delegating validation, value, Greeks and lifecycle to valuations-core. Equity's example supplies 20% implied vol; the vol-index example supplies 50%, American exercise, Black76, futures-style premium and delivery into a future. These are examples, not defaults for all contracts.

## Selection, controls and conventions

The volatility-index curve uses its own day count, not a new option clock. `fair_price` ignores the passed as-of and relies on curve base-date state; callers must roll the market if they want a rolled curve. Shared future-option dispatch remains authoritative for Black/Bachelier/early exercise and premium treatment.

## Limitations, failures and alternatives

Constant `DeltaVol` expresses one index-point exposure even when PV is zero after settlement or the mark is fixed. It is not volatility-of-volatility vega. A modeled VIX expectation with convexity is a different calculation. The option example's product name does not establish real exchange eligibility or calendar compliance.

## Evidence

[VIX index reference](../../../REFERENCES.md#cboe-vix-white-paper) and [Black futures-option reference](../../../REFERENCES.md#black-1976); inline tests in [types.rs](../../../../finstack-quant/valuations/equity/src/vol_index_future/types.rs) locate settlement/curve cases. Shared owner [future-option adapter](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/mod.rs) is the implementation boundary for the macro; its owning audit covers that kernel.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/vol_index_future/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/vol_index_future/metrics.rs)
- [equity_future_option.rs](../../../../finstack-quant/valuations/equity/src/equity_future_option.rs)
- [vol_index_future_option.rs](../../../../finstack-quant/valuations/equity/src/vol_index_future_option.rs)
