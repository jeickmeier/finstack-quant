## `core::money` — Money and FX primitives

This module provides the **currency‑tagged `Money` type** and **foreign‑exchange (FX) utilities** used throughout Finstack:

- **`Money`**: deterministic, currency‑safe monetary amounts backed by `rust_decimal::Decimal` with an ergonomic `f64` API.
- **`money::fx`**: traits and helpers for FX quotes, matrices, conversion policies, and standard provider implementations.
- **Rounding helpers**: internal utilities that implement the rounding policy defined in `crate::config`.

The design enforces **currency safety** (no implicit cross‑currency math), **accounting‑grade precision**, and **stable, audit‑friendly behavior**.

---

## Module layout

- **`mod.rs`**: re-exports `Money` and the `fx` submodule
- **`types.rs`**: `Money` construction, checked arithmetic, formatting, `money!` macro
- **`rounding.rs`**: internal rounding helpers (`AmountRepr = rust_decimal::Decimal`)
- **`fx/`**: FX traits and matrix
  - `provider.rs` — `FxProvider`, `FxRate`
  - `matrix.rs` — `FxMatrix` with LRU cache and triangulation
  - `types.rs` — `FxConfig`, `FxQuery`, `FxConversionPolicy`, `FxRateResult`, `FxPolicyMeta`
  - `providers.rs` — `SimpleFxProvider`, `BumpedFxProvider`

---

## `Money` — currency‑tagged monetary amounts

### Core behavior and invariants

- **Currency‑tagged**: every `Money` value carries a `Currency` tag; all arithmetic preserves the currency.
- **No implicit FX**: adding or subtracting amounts with different currencies is rejected:
  - `Money::checked_add` / `checked_sub` return `Error::CurrencyMismatch`.
  - The in-place `AddAssign` / `SubAssign` trait impls (`lhs += rhs`, `lhs -= rhs`) assert same-currency and panic on mismatch.
- **Deterministic rounding**:
  - Internally, amounts are stored as `Decimal` (`AmountRepr`).
  - Ingestion and formatting use configurable `RoundingMode` and per‑currency scales from `FinstackConfig`.
- **Numeric surface**: public APIs expose values as `f64`, but all arithmetic and rounding is done on `Decimal` to avoid cumulative error.

### Key APIs

- **Construction**
  - `Money::new(amount: f64, currency: Currency)`
    Preserves the raw finite amount without ISO minor‑unit rounding.
    Internal pricing math relies on retained precision.
  - `Money::new_with_config(amount: f64, currency: Currency, cfg: &FinstackConfig)`
    Uses ingest‑scale and rounding mode from `cfg` — use this for
    ledger‑style construction that must honor per‑currency scales.
  - `Money::try_new(amount: f64, currency: Currency)`
    Fallible constructor that returns `Result<Money, Error>`.
    Same retain‑precision semantics as `new`.
  - `Money::try_new_with_config(amount: f64, currency: Currency, cfg: &FinstackConfig)`
    Fallible constructor using explicit rounding config.
  - `From<(f64, Currency)>`, `From<(i64, Currency)>`, `From<(u64, Currency)>` for convenient tuple construction.
  - `money!(amount, USD)` macro shorthand.
- **Accessors**
  - `amount() -> f64`, `currency() -> Currency`.
  - `into_amount() -> f64`, `into_parts() -> (f64, Currency)`.
- **Formatting**
  - `impl Display for Money`: `"USD 123.45"` using ISO‑4217 decimals.
  - `format(decimals, show_currency)` for custom decimal precision.
  - `format_with_separators(decimals)` for ASCII thousands separators plus currency.
  - `format_with_config(&FinstackConfig)` honoring per‑currency output scales and rounding mode.
- **Arithmetic**
  - Checked: `checked_add`, `checked_sub` return `Result<Money, Error>`.
  - Scalar: `impl Mul<f64>`, `Div<f64>` and corresponding `*Assign` variants keep the currency intact.
  - In‑place: `AddAssign` / `SubAssign` assert same‑currency and panic on mismatch.
- **FX conversion**
  - `Money::convert(to: Currency, on: Date, provider: &impl FxProvider, policy: FxConversionPolicy) -> Result<Money>`:
    - No‑op when `self.currency == to`.
    - Otherwise queries the provider and multiplies the underlying amount by the returned rate.
    - Rejects non‑finite FX rates.

### Basic usage

```rust
use finstack_core::money::Money;
use finstack_core::currency::Currency;

let notional = Money::new(1_000_000.0, Currency::EUR);
assert_eq!(notional.currency(), Currency::EUR);
assert_eq!(format!("{}", notional), "EUR 1000000.00");

let fees = Money::new(2_500.0, Currency::EUR);
let total = notional.checked_add(fees).expect("currencies must match");
assert_eq!(total.currency(), Currency::EUR);
```

Using the macro:

```rust
use finstack_core::{money, currency::Currency};

let premium = money!(500.0, USD);
assert_eq!(premium.currency(), Currency::USD);
```

Formatting with custom rules:

```rust
use finstack_core::money::Money;
use finstack_core::currency::Currency;
use finstack_core::config::FinstackConfig;

let amt = Money::new(10.0, Currency::USD);
let mut cfg = FinstackConfig::default();
cfg.rounding.output_scale.overrides.insert(Currency::USD, 4);

assert_eq!(amt.format_with_config(&cfg), "USD 10.0000");
```

---

## FX utilities (`money::fx`)

The `fx` submodule defines **how FX rates are obtained, cached, and consumed**:

- **`FxProvider`**: trait for any FX quote source (in‑memory, external feeds, prebuilt term structures, etc.).
- **`FxMatrix`**: cache and lookup layer that wraps a provider, adds LRU caching and optional triangulation.
- **`FxConfig`**: controls pivot currency, triangulation, and cache capacity.
- **`FxQuery`**: describes a single FX request (`from`, `to`, `on`, `policy`).
- **`FxConversionPolicy`**: hints how the rate will be used (cashflow date, period end, period average, custom).
- **`FxRateResult`**: contains the resolved rate and a `triangulated` flag.
- **`FxPolicyMeta`**: optional metadata struct to stamp FX policy choices into higher‑level result envelopes.

### `FxProvider`

```rust
pub trait FxProvider: Send + Sync {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        on: Date,
        policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64>;
}
```

Implementations are expected to:

- Return `1.0` for identity conversions when appropriate, or let callers handle identity.
- Respect the supplied `FxConversionPolicy` (e.g., pick spot vs forward vs averaged rate).
- Avoid panics; return `Error::NotFound` / `Error::InvalidInput` as needed.

### `FxMatrix`

`FxMatrix` adds **bounded caching** and **simple triangulation** on top of a provider:

- Caches direct quotes and uses reciprocals when possible.
- Optionally triangulates via a configured pivot (e.g., USD) when a direct pair is missing.
- Exposes convenience methods for seeding, clearing, and introspecting the cache.

Typical usage:

```rust
use finstack_core::money::fx::{FxConfig, FxMatrix, FxProvider, FxConversionPolicy, FxQuery};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use std::sync::Arc;
use time::Month;

struct StaticFx;
impl FxProvider for StaticFx {
    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        Ok(1.1)
    }
}

let provider = Arc::new(StaticFx);
let cfg = FxConfig::default();
let matrix = FxMatrix::with_config(provider, cfg);

let query = FxQuery::new(
    Currency::EUR,
    Currency::USD,
    Date::from_calendar_date(2024, Month::March, 1).expect("valid date"),
);
let result = matrix.rate(query).expect("FX rate lookup");
assert!(result.rate > 1.0);
assert!(!result.triangulated);
```

Seeding quotes directly:

```rust
use finstack_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy, FxQuery};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use std::sync::Arc;
use time::Month;

struct StaticFx;
impl FxProvider for StaticFx {
    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        Ok(1.0)
    }
}

let matrix = FxMatrix::new(Arc::new(StaticFx));
matrix.set_quote(Currency::GBP, Currency::USD, 1.3);

let date = Date::from_calendar_date(2024, Month::April, 1).expect("valid date");
let res = matrix
    .rate(FxQuery::new(Currency::GBP, Currency::USD, date))
    .expect("FX rate lookup");
assert_eq!(res.rate, 1.3);
```

Creating a bumped matrix:

```rust
use finstack_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy, FxQuery};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use std::sync::Arc;
use time::Month;

struct StaticFx;
impl FxProvider for StaticFx {
    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        Ok(1.2)
    }
}

let matrix = FxMatrix::new(Arc::new(StaticFx));
let date = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");

let bumped = matrix
    .with_bumped_rate(Currency::EUR, Currency::USD, 0.01, date)
    .expect("bumped matrix");

let rate = bumped
    .rate(FxQuery::new(Currency::EUR, Currency::USD, date))
    .expect("FX rate lookup")
    .rate;
assert!(rate > 1.2);
```

### Standard providers (`fx::providers`)

- **`SimpleFxProvider`**
  - In‑memory `HashMap<(Currency, Currency), f64>` with `RwLock`.
  - Supports direct quotes, reciprocal lookups, and bulk seeding.
  - Implements `FxProvider` with:
    - Identity handling (`from == to` → `1.0`).
    - `Error::NotFound` when neither direct nor reciprocal quotes exist.
- **`BumpedFxProvider`**
  - Wraps an `Arc<dyn FxProvider>` and overrides a single `(from, to)` pair.
  - Used by `FxMatrix::with_bumped_rate` for scenario/bump analysis.

Example using `SimpleFxProvider` directly:

```rust
use finstack_core::money::fx::SimpleFxProvider;
use finstack_core::money::fx::{FxProvider, FxConversionPolicy};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use time::Month;

let provider = SimpleFxProvider::new();
provider.set_quote(Currency::EUR, Currency::USD, 1.1);

let date = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
let rate = provider
    .rate(Currency::EUR, Currency::USD, date, FxConversionPolicy::CashflowDate)
    .expect("FX rate lookup");
assert_eq!(rate, 1.1);
```

---

## Using `Money` with FX providers

To convert a `Money` value between currencies:

```rust
use finstack_core::money::fx::{FxConversionPolicy, FxProvider};
use finstack_core::money::Money;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use time::Month;

struct StaticFx;
impl FxProvider for StaticFx {
    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        Ok(1.2)
    }
}

let eur = Money::new(100.0, Currency::EUR);
let trade_date = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");

let usd = eur
    .convert(
        Currency::USD,
        trade_date,
        &StaticFx,
        FxConversionPolicy::CashflowDate,
    )
    .expect("FX conversion");

assert_eq!(usd.amount(), 120.0);
assert_eq!(usd.currency(), Currency::USD);
```

This pattern keeps **money arithmetic currency‑safe** and **FX sourcing explicit**.

---

## Extending

Keep currency safety and deterministic `Decimal` arithmetic. New FX providers
implement `FxProvider` under `fx/providers.rs`. New `FxConversionPolicy` variants
must preserve `#[non_exhaustive]` compatibility and stable serde names.
