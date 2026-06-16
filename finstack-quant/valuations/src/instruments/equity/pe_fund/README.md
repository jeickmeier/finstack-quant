# Private Markets Fund

## Features

- Models private-market fund cashflows using configurable equity waterfall (`WaterfallSpec`) and ordered `FundEvent`s (contributions, proceeds, distributions).
- Supports European/US-style waterfalls with preferred return, catch-up, and promote tiers; outputs LP cashflows via waterfall engine.
- Optional discount curve for PV; otherwise values on latest event date.

## Methodology & References

- Waterfall processing performed by `EquityWaterfallEngine`, allocating cashflows through return-of-capital, pref, catch-up, and promote tiers.
- PV computed via deterministic discounting of LP cashflows; no stochastic NAV modeling.
- Aligns with common PE/VC waterfall structures (preferred return IRR hurdles, promote splits).

## Usage Example

```rust
use finstack_quant_valuations::instruments::equity::pe_fund::PrivateMarketsFund;

let fund = PrivateMarketsFund::example().unwrap();
let pv = fund.value(&market_context, fund.events.last().unwrap().date)?;
```

## Limitations / Known Issues

- No simulation of underlying asset performance; relies on provided events.
- Waterfall styles limited to implemented spec; bespoke clawbacks/escrow mechanics require extension.
- Currency must match across events; cross-currency funds need explicit FX handling outside the module.

## Metrics

- LP/GP cashflow breakdown, distributed vs undistributed capital, and achieved IRR/MOIC per ledger outputs.
- PV and discount-rate sensitivity via bumping; preferred-return shortfall or promote earned diagnostics.
- Period-level cashflow timelines for reporting.
