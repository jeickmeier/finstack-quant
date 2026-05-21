## Dates Module (core)

The `dates` module in `finstack-core` provides **time and calendar primitives** for all other crates. It wraps the `time` crate and adds finance‑specific behavior:

- **Re‑exports** of `time` types (`Date`, `OffsetDateTime`, `PrimitiveDateTime`)
- **Holiday calendars** and business‑day conventions
- **Day‑count conventions** for accrual and discounting
- **Schedule generation** for coupons, cashflows, and CDS IMM ladders
- **Tenors and period systems** (quarterly/monthly/weekly/fiscal)
- **IMM and option expiry helpers**
- **FX settlement helpers** (joint calendars and spot rolls)

For compounding-convention rate conversions (simple ↔ periodic ↔ continuous), use
[`finstack_core::math::Compounding::convert_rate`](../math/compounding.rs).

Everything is accessible via `finstack_core::dates`, and is designed to be:

- **Deterministic**: No randomness; date math is pure and repeatable
- **Panic‑free in public APIs**: functions return `crate::Result<T>` for invalid input
- **Wire‑stable**: public enums and DTOs are serde-ready with stable field names

---

## Module Structure

- **`mod.rs`**
  - Public entrypoint for the dates module.
  - Re‑exports:
    - `time::{Date, OffsetDateTime, PrimitiveDateTime}`
    - Extension traits: `DateExt`, `OffsetDateTimeExt`
    - Day‑count types: `DayCount`, `DayCountContext`, `DayCountContextState`, `Thirty360Convention`
    - Calendars and business days:
      - `HolidayCalendar`, `BusinessDayConvention`, `adjust`, `available_calendars`
      - `CompositeCalendar`, `CalendarRegistry`
    - Schedule types: `Tenor`, `Schedule`, `ScheduleBuilder`, `ScheduleSpec`, `StubKind`
    - Tenor and IMM helpers: `Tenor`, `TenorUnit`, `next_imm`, `next_cds_date`, `third_wednesday`, etc.
    - FX helpers: joint-calendar adjustments and spot roll helpers (`fx::*`)
    - Period system: `Period`, `PeriodId`, `PeriodKind`, `PeriodPlan`, `FiscalConfig`,
      `build_periods`, `build_fiscal_periods`
    - Safe constructor `create_date(year, month, day) -> Result<Date>`
- **`date_extensions.rs`**
  - Extension traits for `Date` and `OffsetDateTime`:
    - Calendar helpers: `is_weekend`, `is_business_day`, `end_of_month`, `next_imm`
    - Fiscal helpers: `quarter`, `fiscal_year(config: FiscalConfig)`
    - Arithmetic: `add_months`, `add_weekdays`, `add_business_days`
    - Analytics: `months_until(other)`
    - `BusinessDayIter` iterator over business days in `[start, end)`.
- **`calendar/`**
  - Holiday calendar system:
    - `business_days.rs`: `HolidayCalendar` trait, `BusinessDayConvention`, `adjust`, `seek_business_day`
    - `rule.rs`: rule DSL for holiday definitions (fixed dates, nth weekday, Easter offsets, lunar rules, etc.)
    - `generated.rs` + `algo.rs`: build‑time compiled bitsets for 1970–2150 and shared helpers
    - `composite.rs`: `CompositeCalendar` for unions of calendars
    - `types.rs`: metadata types (`CalendarMetadata`, IDs, etc.)
    - `registry.rs`: `CalendarRegistry` and global registry, calendar lookup by ID
- **`daycount.rs`**
  - Industry‑standard day‑count conventions:
    - `DayCount` enum with variants:
      - `Act360`, `Act365F`, `Act365L`
      - `Thirty360`, `ThirtyE360`
      - `ActAct`, `ActActIsma`
      - `Bus252`
    - `DayCountContext` / `DayCountContextState` to supply calendars and coupon frequency (for `Bus252`, `ActActIsma` regular periods).
    - Core API:
      - `DayCount::year_fraction(start, end, ctx) -> Result<f64>`
    - Helpers for 30/360, Act/Act (ISDA + ISMA), Act/365L, and business‑day counting.
    - For irregular ICMA coupons (short/long stubs), use
      `act_act_isma_year_fraction_with_reference_period(...)` with explicit
      reference coupon boundaries rather than relying on the frequency-only helper.
- **`schedule_iter.rs`**
  - Schedule generation engine:
    - `Tenor` (time periods with units: Days, Weeks, Months, Years, with helpers like `monthly()`, `quarterly()`, `weekly()`, etc.)
    - `StubKind` (None, ShortFront/Back, LongFront/Back)
    - `Schedule` (vector of monotonically increasing `Date`s)
    - `ScheduleBuilder`:
      - Tenor configuration
      - Stub rules
      - End‑of‑month convention
      - Business‑day adjustment via calendar or calendar ID
      - CDS IMM mode (`cds_imm`)
      - Graceful fallback mode (empty schedule on error)
    - `ScheduleSpec` (serde DTO) → reconstructs a `ScheduleBuilder` at runtime.
- **`tenor.rs`**
  - Market‑standard tenor handling:
    - `TenorUnit` (Days, Weeks, Months, Years)
    - `Tenor { count, unit }` with:
      - Parsing from strings (`"3M"`, `"6M"`, `"1Y"`)
      - Conversions:
        - `to_years_simple()` – simple approximation
        - `to_years_with_context(as_of, calendar, bdc, day_count)`
        - `add_to_date(date, calendar, bdc)`
      - Constructors: `overnight`, `one_week`, `one_month`, `three_months`, `six_months`, `one_year`
- **`periods.rs`**
  - Financial reporting period system for statements and time‑series:
    - `PeriodKind`: `Quarterly`, `Monthly`, `Weekly`, `SemiAnnual`, `Annual`
    - `PeriodId`: typed IDs like `2025Q1`, `2025M03`, `2025W10`
      - Ordering by actual calendar spans (even across mixed kinds)
      - Display/parse and serde support
    - `FiscalConfig`: description of fiscal year start (month/day) with presets (US federal, UK, Japan, etc.)
    - `Period`: `(id, start, end, is_actual)` with inclusive/exclusive bounds
    - `PeriodPlan { periods: Vec<Period> }` with iteration helpers
    - Builders:
      - `build_periods("2025Q1..Q4", Some("2025Q2"))`
      - `build_periods("2024M11..2025M02", None)`
      - `build_fiscal_periods("2025Q1..Q4", FiscalConfig::us_federal(), Some("2025Q2"))`
- **`imm.rs`**
  - IMM and option expiry helpers:
    - `third_wednesday(month, year)`
    - `next_imm(date)` – next IMM (3rd Wed of Mar/Jun/Sep/Dec) strictly after `date`
    - `next_cds_date(date)` – next CDS roll (20‑Mar/Jun/Sep/Dec)
    - `imm_option_expiry(month, year)` – Friday before third Wednesday
    - `third_friday(month, year)`
    - `next_imm_option_expiry(date)` – next IMM option expiry (quarterly)
    - `next_equity_option_expiry(date)` – next equity option expiry (3rd Friday monthly)

Compounding-convention rate conversions live in
[`finstack_core::math::Compounding`](../math/compounding.rs), not in `dates`.

---

## Core Types and Traits

### `Date`, `OffsetDateTime` and `DateExt` / `OffsetDateTimeExt`

The module re‑exports `time::Date` and `time::OffsetDateTime` as the canonical date types, and augments them via extension traits:

- **Calendar awareness**:
  - `is_weekend() -> bool`
  - `is_business_day(&C) -> bool` (using a `HolidayCalendar`)
  - `end_of_month() -> Date`
  - `next_imm() -> Date`
- **Fiscal logic**:
  - `quarter() -> u8`
  - `fiscal_year(FiscalConfig) -> i32`
- **Arithmetic**:
  - `add_months(months: i32) -> Date`
  - `add_weekdays(n: i32) -> Date` (weekends only)
  - `add_business_days(n: i32, &C) -> Result<Date>` (weekends + holidays)
  - `months_until(other) -> u32`
- **Iterator**:
  - `BusinessDayIter<'a, C>` over business days in `[start, end)`.

Use the **safe constructor**:

```rust
use finstack_core::dates::{create_date, Date};
use time::Month;

let d = create_date(2025, Month::January, 15)?;        // Ok
let invalid = create_date(2025, Month::February, 30);  // Err(InputError::InvalidDate { .. })
```

### `HolidayCalendar` and `BusinessDayConvention`

Calendars are read‑only, deterministic objects backed by generated bitsets plus runtime rules. The core trait is:

- `HolidayCalendar` (in `calendar::business_days`):
  - `is_holiday(date: Date) -> bool`
  - `is_business_day(date: Date) -> bool`

Business‑day adjustments use:

- `BusinessDayConvention` enum (`Following`, `ModifiedFollowing`, `Preceding`, etc.)
- `adjust(date, convention, &cal) -> Result<Date>`
- `available_calendars() -> impl Iterator<Item = &str>` for discovery

Lookup is done via `CalendarRegistry`:

```rust
use finstack_core::dates::{Date, BusinessDayConvention, adjust};
use finstack_core::dates::CalendarRegistry;
use time::Month;

let base = Date::from_calendar_date(2025, Month::December, 25)?;
let nyse = CalendarRegistry::global()
    .resolve_str("nyse")
    .ok_or("missing NYSE calendar")?;

let adj = adjust(base, BusinessDayConvention::Following, nyse)?;
```

For multi‑market instruments, use `CompositeCalendar` to union calendars.

### `DayCount` and `DayCountContext`

`DayCount` encodes market day‑count conventions. The main API:

```rust
use finstack_core::dates::{Date, DayCount, DayCountContext};
use time::Month;

let start = Date::from_calendar_date(2025, Month::January, 1)?;
let end = Date::from_calendar_date(2026, Month::January, 1)?;

let yf = DayCount::ActAct.year_fraction(start, end, DayCountContext::default())?;
```

Context carries:

- `calendar: Option<&dyn HolidayCalendar>` for `Bus252`
- `frequency: Option<Tenor>` for `ActActIsma`
- `bus_basis: Option<u16>` for custom `Bus/N` denominators

`DayCountContextState` is a serde DTO that can be serialized (e.g. to JSON) and re‑hydrated using a `CalendarRegistry`.

### `Tenor`, `ScheduleBuilder`, and `Schedule`

`Tenor` is a **time period** used for payment frequencies and schedule intervals:

- Year/Month‑based: `Tenor::annual()`, `semi_annual()`, `quarterly()`, `bimonthly()`, `monthly()`
- Week/Day‑based: `Tenor::biweekly()`, `weekly()`, `daily()`
- Or parse from string: `Tenor::parse("1Y")`, `Tenor::parse("3M")`, `Tenor::parse("1W")`

Schedules are built via `ScheduleBuilder`:

```rust
use finstack_core::dates::{ScheduleBuilder, Tenor, BusinessDayConvention};
use finstack_core::dates::CalendarRegistry;
use time::{Date, Month};

let start = Date::from_calendar_date(2025, Month::January, 15)?;
let end = Date::from_calendar_date(2025, Month::December, 15)?;
let nyse = CalendarRegistry::global()
    .resolve_str("nyse")
    .ok_or("nyse not found")?;

let schedule = ScheduleBuilder::new(start, end)?
    .frequency(Tenor::quarterly())
    .stub_rule(finstack_core::dates::StubKind::ShortBack)
    .end_of_month(false)
    .adjust_with(BusinessDayConvention::ModifiedFollowing, nyse)
    .build()?;

for d in schedule.into_iter() {
    println!("{d}");
}
```

Key invariants:

- Dates are strictly increasing and deduplicated after EOM and adjustment.
- `build()` returns `Result<Schedule>`; invalid ranges (`start > end`) yield `Error::Input(InputError::InvalidDateRange)`.
- With `.graceful_fallback(true)`, `build()` returns an **empty** schedule with a `ScheduleWarning::GracefulFallback` warning instead of an error. Always check `schedule.has_warnings()` to detect suppressed errors and avoid silent PV=0 scenarios.

`ScheduleSpec` is a serde‑capable spec you can persist and later call `.build()` on to rebuild schedules.

### `Tenor` and `TenorUnit`

Tenors encapsulate relative time periods with finance semantics:

```rust
use finstack_core::dates::{Tenor, TenorUnit, Date, DayCount, DayCountContext, BusinessDayConvention};
use finstack_core::dates::calendar::TARGET2;
use time::Month;

let as_of = Date::from_calendar_date(2025, Month::January, 31)?;
let tenor = Tenor::parse("1M")?;

// Date math with EOM + business‑day adjustment
let end = tenor.add_to_date(as_of, Some(&TARGET2), BusinessDayConvention::ModifiedFollowing)?;

// Calendar‑aware year fraction
let yf = tenor.to_years_with_context(
    as_of,
    Some(&TARGET2),
    BusinessDayConvention::ModifiedFollowing,
    DayCount::ActAct,
)?;
```

Use `Tenor::from_years(years, DayCount)` when converting continuous year fractions back into market‑style tenors.

### Periods and Fiscal Configurations

The period system is designed for statements and forecasting:

- Use `PeriodId` to represent a single period (`2025Q1`, `2025M03`, `2025W10`).
- Use `build_periods` to expand range expressions:

```rust
use finstack_core::dates::{build_periods, Period};

let plan = build_periods("2025Q1..Q3", Some("2025Q2"))?;
for p in plan.periods {
    println!("{}: {}..{} (actual={})", p.id, p.start, p.end, p.is_actual);
}
```

For fiscal years, use `FiscalConfig` and `build_fiscal_periods`:

```rust
use finstack_core::dates::{FiscalConfig, build_fiscal_periods};

let config = FiscalConfig::us_federal(); // Oct 1 start
let fiscal = build_fiscal_periods("2025Q1..Q4", config, Some("2025Q2"))?;
```

Range strings may mix absolute and relative right‑hand sides (`"2024M11..2025M02"`, `"2025Q1..Q4"`). Mixed frequencies in the same year are ordered by actual calendar spans.

### IMM and Expiry Helpers

Use IMM utilities instead of ad‑hoc “third Wednesday” logic:

```rust
use finstack_core::dates::{third_wednesday, next_imm, next_cds_date, next_imm_option_expiry};
use time::{Date, Month};

let imm_march = third_wednesday(Month::March, 2025);
let next_futures_roll = next_imm(Date::from_calendar_date(2025, Month::March, 20)?);
let next_cds_roll = next_cds_date(Date::from_calendar_date(2025, Month::March, 10)?);
let next_imm_option = next_imm_option_expiry(Date::from_calendar_date(2025, Month::March, 15)?);
```

For equity options, use `next_equity_option_expiry(date)` (3rd Friday of each month).

### Rate Conversion Utilities

Use [`Compounding::convert_rate`] for all rate convention conversions:

```rust
use finstack_core::math::Compounding;

// Money‑market simple rate → swap (semi‑annual) → continuous → back to simple
let simple = 0.035;
let yf = 0.25;                // 3M
let periodic = Compounding::Simple.convert_rate(simple, yf, &Compounding::SEMI_ANNUAL);
let continuous = Compounding::SEMI_ANNUAL.convert_rate(periodic, 1.0, &Compounding::Continuous);
let simple_back = Compounding::Continuous.convert_rate(continuous, yf, &Compounding::Simple);
```

The `Compounding` enum:

- Supports `Continuous`, `Annual`, `Periodic(n)`, and `Simple` conventions
- Provides `convert_rate`, `df_from_rate`, and `rate_from_df` methods
- Preserves precision under realistic rates (round‑trip tests)
- Supports negative rates, which are common in modern markets

---

## Usage Examples

### Business‑Day Adjustments

```rust
use finstack_core::dates::{Date, DateExt, BusinessDayConvention, adjust};
use finstack_core::dates::calendar::TARGET2;
use time::Month;

let cal = TARGET2;
let trade_date = Date::from_calendar_date(2025, Month::June, 27)?; // Friday

// 3 business days forward via extension
let settlement = trade_date.add_business_days(3, &cal)?;

// Equivalent explicit adjustment from “raw” date
let raw_settlement = trade_date + time::Duration::days(3);
let adjusted = adjust(raw_settlement, BusinessDayConvention::Following, &cal)?;
```

### Reporting Period Plans

```rust
use finstack_core::dates::{build_periods, build_fiscal_periods, FiscalConfig};

// Calendar quarters with actual/forecast split
let plan = build_periods("2025Q1..Q4", Some("2025Q2"))?;
assert_eq!(plan.periods.len(), 4);
assert!(plan.periods[0].is_actual);
assert!(!plan.periods[3].is_actual);

// US federal fiscal quarters (FY2025)
let cfg = FiscalConfig::us_federal();
let fiscal = build_fiscal_periods("2025Q1..Q4", cfg, None)?;
```

### Curve‑Compatible Day Counts

You should use `DayCount` consistently between curves and cashflow accruals:

```rust
use finstack_core::dates::{Date, DayCount, DayCountContext};
use time::Month;

let start = Date::from_calendar_date(2025, Month::January, 1)?;
let end   = Date::from_calendar_date(2025, Month::July, 1)?;

let dc = DayCount::Act360;
let yf = dc.year_fraction(start, end, DayCountContext::default())?;
```

For `Bus252`, provide a calendar:

```rust
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::dates::calendar::TARGET2;
use time::Month;

let start = Date::from_calendar_date(2025, Month::January, 2)?;
let end   = Date::from_calendar_date(2025, Month::January, 9)?;

let ctx = DayCountContext { calendar: Some(&TARGET2), frequency: None, bus_basis: None, coupon_period: None };
let yf = DayCount::Bus252.year_fraction(start, end, ctx)?;
```

---

## Extending

Calendars: JSON under `data/calendars/`, rebuild, tests in `tests/dates/`.
Day-count: new `DayCount` variant in `daycount.rs` with `DayCountContext` support.
Schedules: extend `ScheduleBuilder` in `schedule_iter.rs`; preserve strictly
increasing dates. Rate conversions belong in `math::Compounding`, not `dates`.
