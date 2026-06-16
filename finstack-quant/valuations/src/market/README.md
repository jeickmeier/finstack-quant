# Market Module

The `market` module owns market-data inputs, convention registries, and quote-to-instrument construction.
It provides stable quote schemas, convention lookup, and builder utilities used by calibration and
pricing pipelines.

## Scope

- Market quotes for rates, credit, inflation, and volatility surfaces
- Convention registries loaded from embedded JSON data
- Quote-to-instrument builders for rates and credit calibration workflows
- Prepared quote envelopes with precomputed maturity time

This module is intentionally focused on market data representation and construction logic, not
pricing models or calibration solvers.

## Layout

- `build/` - Build context and quote-to-instrument constructors
- `conventions/` - Convention definitions, identifiers, and registry
- `quotes/` - Market quote schemas and identifiers

## Data Flow

1. Load conventions via `ConventionRegistry::try_global()` (embedded JSON registries).
2. Parse market quotes into typed structs/enums in `quotes/`.
3. Use builders in `build/` with a `BuildCtx` to create concrete instruments (rates/credit).
4. For inflation and vol, builders live in calibration targets; wrap into `PreparedQuote` there.

## Key Types

- `BuildCtx` (`build/context.rs`)
  - Captures `as_of`, `notional`, curve role mappings, and attributes.
- `ConventionRegistry` (`conventions/registry.rs`)
  - Singleton registry of rate index, CDS, swaption, inflation, option, and IR future conventions.
- `QuoteId`, `Pillar` (`quotes/ids.rs`)
  - Stable identifiers and maturity pillars (tenor or date).
- `MarketQuote` (`quotes/market_quote.rs`)
  - Unified enum across all supported quote types.
- `PreparedQuote` (`build/prepared.rs`)
  - Quote + instrument + pillar date/time for calibration pipelines.

## Example: Build a Rate Instrument from a Quote

```rust
use finstack_quant_valuations::market::BuildCtx;
use finstack_quant_valuations::market::build_rate_instrument;
use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_quant_valuations::market::quotes::rates::RateQuote;
use finstack_quant_valuations::market::conventions::ids::IndexId;
use finstack_quant_core::dates::Date;
use std::collections::HashMap;

let ctx = BuildCtx::new(Date::from_calendar_date(2024, time::Month::January, 2).unwrap(), 1_000_000.0, HashMap::new());
let quote = RateQuote::Deposit {
    id: QuoteId::new("USD-SOFR-DEP-1M"),
    index: IndexId::new("USD-SOFR-1M"),
    pillar: Pillar::Tenor("1M".parse().unwrap()),
    rate: 0.0525,
};

let instrument = build_rate_instrument(&quote, &ctx)?;
```

## Conventions and Embedded Data

Conventions are loaded from embedded JSON registries under `data/conventions/` and are accessed
through `ConventionRegistry::try_global()`. Each loader performs validation and normalization before
inserting entries into the registry.

- Rate index conventions (`rate_index_conventions.json`)
- CDS conventions (`cds_conventions.json`)
- Swaption conventions (`swaption_conventions.json`)
- Inflation swap conventions (`inflation_swap_conventions.json`)
- Option conventions (`option_conventions.json`)
- IR future conventions (`ir_future_conventions.json`)

If a convention lookup fails, the registry returns a validation error. There is no implicit fallback
logic in builders; quote inputs must be aligned with the registry content.

## Extending the Module

- Add a new quote type under `quotes/` with a stable `QuoteId` strategy.
- Add or extend conventions under `conventions/defs.rs` and registries in `data/conventions/`.
- Add a builder in `build/` that maps a quote to a concrete instrument type.
- If calibration needs precomputed time, wrap quotes into `PreparedQuote` and calculate the pillar
  time with the module's date utilities.

## TypeScript Export

When the `ts_export` feature is enabled, quote types annotate the schema for `ts_rs` export.
This is intended for data interchange and validation on the client or in API layers.

## Error Handling

All loaders and builders return `finstack_quant_core::Error` variants with explicit context. These errors
are surfaced early so bad market data or missing conventions are detected before calibration.
