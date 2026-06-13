# CDS Index

## Features

- Handles CDS indices in two modes: single-curve synthetic index pricing or constituent-level aggregation with per-name curves.
- Par-spread calculation configurable via `ParSpreadMethod` (risky annuity or full premium with accrual-on-default).
- Supports index factors, weight normalization, and standard IMM schedule conventions.

## Methodology & References

- Delegates leg PVs and risky annuity calculations to the single-name CDS pricer to maintain parity with ISDA standards.
- Constituents mode aggregates weighted single-name results; optional normalization to handle slight weight sum drift.
- Deterministic hazard/discount curves; no copula-style correlation modeled inside the pricer.

## Usage Example

```rust
use finstack_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndex, CDSIndexConstituent, CDSIndexParams,
};
use finstack_valuations::instruments::CreditParams;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 5)?;
let start = Date::from_calendar_date(2024, Month::March, 20)?;
let end = Date::from_calendar_date(2029, Month::December, 20)?;

// Build from a well-known preset; convention is bundled with the preset.
let idx = CDSIndex::from_preset(
    &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
    "CDX-IG-42",
    Money::new(10_000_000.0, Currency::USD),
    PayReceive::Pay,
    start,
    end,
    0.40,
    "USD-OIS",
    "CDX.NA.IG.HAZARD",
)?
// Optional trade-state setters can be chained:
.with_index_factor(0.96)
.with_constituents(vec![
    CDSIndexConstituent::active(CreditParams::corporate_standard("ACME", "ACME-HAZ"), 0.5),
    CDSIndexConstituent::active(CreditParams::corporate_standard("WIDGET", "WIDGET-HAZ"), 0.5),
]);

let pv = idx.value(&market_context, as_of)?;
let par = idx.par_spread(&market_context, as_of)?;
```

## Limitations / Known Issues

- No default correlation or contagion modeling; constituents mode sums deterministic single-name valuations.
- Index roll mechanics beyond the provided schedule/series must be managed externally.
- Relies on supplied hazard and discount curves; no calibration built into the pricer.

## Metrics

- PV, par spread, index RPV01, upfront for given quote, and CS01 (parallel/bucketed) via constituent aggregation.
- Leg PV breakdown (premium vs protection) and accrued components.
- Expected loss uses constituents (if provided) with per-name curves/recoveries; otherwise index-level curve.
- Jump-to-default uses constituent weights when present; otherwise infers name count from standard index name.
- Weight diagnostics (sum/normalization) for data quality checks.
