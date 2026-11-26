# finstack-quant-valuations-macros

Procedural macro crate backing instrument construction in
`finstack-quant-valuations`. It provides one derive, `FinancialBuilder`, which
generates a fluent, validating builder for an instrument struct.

Directory: `finstack-quant/valuations/macros`. Package / import name:
`finstack-quant-valuations-macros` / `finstack_quant_valuations_macros`.

## Position in the workspace

This is a supporting crate, not one of the 14 domain crates:

- **not** re-exported by the `finstack-quant` umbrella crate — depend on it
  directly
- depends only on `syn`, `quote`, and `proc-macro2`; it has no finstack
  dependency and no runtime footprint
- the only consumer is `finstack-quant-valuations`, where 65 instrument structs
  under `src/instruments/` derive `FinancialBuilder`

Generated code names `finstack_quant_core` and `finstack_quant_valuations_core`
directly, so a deriving crate must depend on both under their canonical names.

```toml
# finstack-quant/valuations/Cargo.toml
[dependencies]
finstack-quant-valuations-macros = { path = "macros", version = "0.11.0" }
finstack-quant-valuations-core = { path = "core", version = "0.11.0" }
finstack-quant-core = { path = "../core", version = "0.11.0" }
```

## `FinancialBuilder`

```rust
#[derive(
    Clone,
    Debug,
    PartialEq,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = FxForward::validate)]
#[serde(deny_unknown_fields, try_from = "FxForwardUnchecked")]
pub struct FxForward { /* … */ }
```

For a struct `T` with named fields, the derive emits:

| Generated item | Signature |
|----------------|-----------|
| `TBuilder` | `pub struct TBuilder { … }`, `#[derive(Default)]` |
| field setters | `pub fn <field>(self, value: <Ty>) -> Self` (consuming, chainable) |
| optional setters | `pub fn <field>_opt(self, value: Option<Inner>) -> Self` for `Option<Inner>` fields |
| `TBuilder::build` | `pub fn build(self) -> finstack_quant_core::Result<T>` |
| `T::builder` | `pub fn builder() -> TBuilder`, `#[must_use]` |

`builder()` takes **no arguments**: instruments have many required fields, and
named setters are clearer than a long positional list. This differs from the
hand-written curve builders in `finstack-quant-core`
(`DiscountCurve::builder("USD-OIS")`), which take the one natural key as an
argument.

`T::builder()` is the only entry point, as
[`.agents/rules/rust/code-standards.md`](../../../.agents/rules/rust/code-standards.md)
requires: the derive emits no `TBuilder::new()` alias.

For an `Option<Inner>` field the derive generates two setters: `field(inner)`
wraps the value in `Some`, and `field_opt(Option<Inner>)` assigns the option
directly (used when a caller is forwarding an already-optional value).

### Required vs optional fields

Whether a field is *optional* is decided by its type and name alone — no
attribute makes a non-`Option` field optional. `#[builder(default …)]` only
supplies a fallback for a field that stays required:

| Field | Treated as | Behavior when unset |
|-------|-----------|---------------------|
| `Option<Inner>` | optional | stays `None` |
| named `attributes` | optional | `Default::default()` |
| anything else | required | `build()` returns a validation error naming every unset field |
| non-`Option` field with `#[builder(default …)]` | required, but defaulted | the default expression |

A missing required field produces `finstack_quant_core::Error::Validation`
with the message `"<Builder>: missing required field '<field>'"`. When several
are missing, the message names the first in declaration order and appends the
others: `"<Builder>: missing required field 'a' (also missing: 'b', 'c')"`.
The check runs before any default is resolved.

### Attributes

Struct-level:

| Attribute | Effect |
|-----------|--------|
| `#[builder(validate = <path>)]` | Calls `<path>(&built)?` as the last step of `build()`. When omitted, `build()` calls `Instrument::validate_invariants(&built)?`. |

Field-level:

| Attribute | Effect |
|-----------|--------|
| `#[builder(default)]` | Field falls back to `Default::default()` when unset. |
| `#[builder(default = <expr>)]` | As above with an explicit expression, e.g. `#[builder(default = BusinessDayConvention::ModifiedFollowing)]`. Rejected at compile time on `Option<T>` and `attributes` fields. |
| `#[builder(skip)]` | Omits setters and initializes the field directly from `#[builder(default = ...)]` or `Default::default()`. |

Unrecognized struct- or field-level keys inside `#[builder(...)]` are rejected at compile time.

### Companion test

[`valuations/tests/default_attribute_consistency.rs`](../tests/default_attribute_consistency.rs)
holds two source-scanning tests over every `src/instruments/**/types.rs`:

- `builder_default_requires_serde_default_in_instrument_types` fails when a
  field carries `#[builder(default)]` without a matching `#[serde(default)]`,
  keeping builder defaults and wire defaults from drifting apart. The three
  `*_pricing_overrides` fields are exempt — their wire serde is generated.
- `instrument_types_do_not_store_the_legacy_full_override_bag` fails on any
  `pub pricing_overrides:` field, holding instruments to the focused override
  categories.

## Example

Lifted from
[`Deposit`](../src/instruments/rates/deposit/types.rs) and its construction
tests. Its fourteen fields classify as:

- required: `id`, `notional`, `start_date`, `maturity`, `day_count`,
  `discount_curve_id`
- `Option<_>`, so optional: `fixed_rate`, `settlement_days`, `calendar_id`
- required-but-defaulted: `business_day_convention`
  (`#[builder(default = BusinessDayConvention::ModifiedFollowing)]`) and the
  three override bags `instrument_pricing_overrides`,
  `metric_pricing_overrides`, `scenario_pricing_overrides` (bare
  `#[builder(default)]`)
- `attributes`, which defaults by name

Only the six required fields have to be set:

```rust
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use time::Month;

fn six_month_deposit() -> finstack_quant_core::Result<Deposit> {
    Deposit::builder()
        .id(InstrumentId::new("DEP-001"))
        .notional(Money::new(1_000_000.0, Currency::USD)?)
        .start_date(create_date(2025, Month::January, 1)?)
        .maturity(create_date(2025, Month::July, 1)?)
        .day_count(DayCount::Act360)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
}

assert!(six_month_deposit().unwrap().fixed_rate.is_none());
```

Cross-field invariants live in `Instrument::validate_invariants` (called automatically by `build()`) or an explicit `#[builder(validate = ...)]` hook.

## Verification

The derive has no unit tests of its own; it is exercised through the
instruments that use it.

```bash
cargo clippy -p finstack-quant-valuations-macros --lib --bins --tests --examples --all-features -- -D warnings
cargo nextest run -p finstack-quant-valuations --test valuations instruments
cargo nextest run -p finstack-quant-valuations --test valuations default_attribute_consistency
```

Or the whole Rust layer: `mise run rust-test` and `mise run rust-lint`.

Implementation: [`src/financial_builder.rs`](src/financial_builder.rs).
