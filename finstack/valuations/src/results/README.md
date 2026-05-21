# Results

`ValuationResult` is the standard pricing output: PV, optional risk measures, metadata, covenants, and an optional explanation trace.

## Types

**`ValuationResult`** (`valuation_result.rs`)

| Field | Content |
|-------|---------|
| `value` | NPV as `Money` |
| `measures` | `IndexMap<String, f64>` — metric values; units follow `MetricId` definitions, not all entries are currency |
| `meta` | `ResultsMeta` from `finstack_core::config` (numeric mode, rounding, FX policy, timing) |
| `covenants` | Optional `CovenantReport` map |
| `explanation` | Optional `ExplanationTrace` |

**`ValuationRow`** (`dataframe.rs`) — flat row for tabular export; promotes common bond metrics when present.

## Construction

```rust
use finstack_valuations::results::ValuationResult;
use finstack_core::money::Money;
use finstack_core::currency::Currency;
use finstack_core::dates::create_date;
use time::Month;

let as_of = create_date(2025, Month::January, 15)?;
let pv = Money::new(1_000_000.0, Currency::USD);

let result = ValuationResult::stamped("BOND-001", as_of, pv);

// With metrics (from price_with_metrics or manual insertion)
let result = result.with_measures(measures);
```

For batch pricing, build `ResultsMeta` once via `results_meta(&config)` and use `stamped_with_meta` to avoid repeated config allocation.

## Covenants

```rust
let result = ValuationResult::stamped("LOAN-001", as_of, pv)
    .with_covenant("dscr_test", dscr_report);

assert!(result.all_covenants_passed());
```

## Export

- `to_row()` / `results_to_rows()` — serialize to JSON/CSV-friendly rows
- Measures keys should use `MetricId::as_str()` for stable names

## Flow

1. Instrument pricer or `Instrument::price_with_metrics` produces `ValuationResult`.
2. Portfolio code scales by quantity and converts to base currency with the stamped FX policy.
3. Reporting flattens via `ValuationRow` or custom exporters.

## Related

- [`../metrics/README.md`](../metrics/README.md)
- [`../covenants/README.md`](../covenants/README.md)
