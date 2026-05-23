# finstack-core `src/` overview

Low-level building blocks for the Finstack workspace:

- **Deterministic** — serial and parallel runs match
- **Currency-safe** — no implicit FX; cross-currency math is explicit
- **Serde-stable** — public types have versioned wire formats

## Top-level modules

| Module | Role |
|--------|------|
| `lib.rs` | Crate entry point and public module declarations |
| `config.rs` | Numeric mode, rounding policy, `FinstackConfig`, `ResultsMeta` |
| `currency.rs` | ISO-4217 currency enum; generated tables under `generated/` |
| `money/` | `Money`, rounding, FX matrix and providers — see [`money/README.md`](money/README.md) |
| `dates/` | Calendars, day-count, schedules, tenors, periods — see [`dates/README.md`](dates/README.md) |
| `market_data/` | Term structures, surfaces, scalars, `MarketContext` — see [`market_data/README.md`](market_data/README.md) |
| `math/` | Interpolation, solvers, integration, statistics — see [`math/README.md`](math/README.md) |
| `expr/` | Scalar expression engine — see [`expr/README.md`](expr/README.md) |
| `cashflow/` | Cashflow primitives, NPV, IRR/XIRR — see [`cashflow/README.md`](cashflow/README.md) |
| `types/` | Phantom-typed IDs, rates, ratings — see [`types/README.md`](types/README.md) |
| `credit/` | PD/LGD/migration primitives |
| `math/volatility/` | Volatility models and option pricing formulas |
| `error.rs` | Unified `Error` type |
| `explain.rs` | Computation tracing |
| `generated/` | Build-time currency and calendar tables |

## Quick examples

### Currency-safe money

```rust
use finstack_core::currency::Currency;
use finstack_core::money::Money;

fn main() -> finstack_core::Result<()> {
    let subtotal = Money::new(49.50, Currency::EUR);
    let tax = Money::new(9.90, Currency::EUR);
    let total = subtotal.checked_add(tax)?;
    assert_eq!(format!("{}", total), "EUR 59.40");
    Ok(())
}
```

### Day count

```rust
use finstack_core::dates::{create_date, DayCount, DayCountContext};
use time::Month;

fn main() -> finstack_core::Result<()> {
    let start = create_date(2025, Month::January, 1)?;
    let end = create_date(2026, Month::January, 1)?;
    let yf = DayCount::ActAct
        .year_fraction(start, end, DayCountContext::default())?;
    assert!((yf - 1.0).abs() < 1e-9);
    Ok(())
}
```

### Discount curve

```rust
use finstack_core::dates::create_date;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use time::Month;

fn main() -> finstack_core::Result<()> {
    let base_date = create_date(2025, Month::January, 1)?;
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .interp(InterpStyle::MonotoneConvex)
        .build()?;
    assert!(curve.df(3.0) < 1.0);
    Ok(())
}
```

## Extending the crate

Extend the module that owns the domain primitive. Preserve determinism,
currency safety, and stable serde field names. Add unit tests beside the
implementation and integration tests under `finstack/core/tests/` when behavior
spans modules.

Common patterns:

- **New calendar** — JSON under `data/calendars/`, rebuild, tests in `tests/dates/`
- **New day-count** — variant in `dates/daycount.rs` with tests
- **New interpolation** — implementation under `math/interp/`, wired through `InterpStyle`
- **New term structure** — module under `market_data/term_structures/` with builder, traits, and `MarketContext` integration

See module READMEs and `AGENTS.md` for binding and naming conventions.

## Related files

- Module READMEs linked above
- Integration tests: `finstack/core/tests/`
- Workspace architecture: `AGENTS.md`
