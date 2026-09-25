# common_impl — instrument plumbing

**This directory is internal.** `instruments/mod.rs` declares it
`#[macro_use] pub(crate) mod common_impl;`. Nothing here is reachable by path
from outside `finstack-quant-valuations`; the supported items are re-exported
flat from `instruments::*` or through the two public sub-namespaces
`instruments::pricing` and `instruments::cashflow_export`. Add items here when
two or more instruments need them, and export them deliberately — a `pub fn` in
a `pub(crate) mod` is still crate-private. The one exception is
`#[macro_export]`, which hoists a macro to the crate root and ignores module
visibility entirely; see "What escapes to the public API" below.

This is the layer every instrument implements against: the `Instrument` trait
and its boilerplate macros, the shared parameter vocabulary (leg specs,
underlying params, market conventions), the shared pricing kernels (swap legs,
overnight compounding, TRS, variance replication), and the validation /
numeric-serde helpers that keep instrument invariants uniform.

## Layout

| Path | Responsibility |
|------|----------------|
| `mod.rs` | Module declarations; `example_constants::FAR_EXPIRY`, the single date every long-dated `example()` constructor rolls forward from |
| `traits/` | The `Instrument` trait, `Attributes`, `PricingOptions`, `OptionGreeks*`, and the `impl_*` macros |
| `traits/instrument.rs` | `Instrument` definition — required methods, default methods, and the override contract for each |
| `traits/macros.rs` | `impl_instrument_base!`, `impl_focused_pricing_overrides!`, `impl_empty_cashflow_provider!` |
| `traits/pricing_options.rs` | `PricingOptions` — config, market history, model override, registry override, plus the optional recalibration-provider port |
| `traits/option_greeks.rs` | `OptionGreekKind`, `OptionGreeksRequest`, `OptionGreeks`, `OptionGreeksProvider`, and `impl_equity_exotic_traits!` (a fourth `#[macro_export]` macro, filed here rather than in `traits/macros.rs`) |
| `parameters/` | Shared parameter types (see table below) |
| `pricing/` | Shared pricing kernels (see table below) |
| `dependencies.rs` | `MarketDependencies`, `InstrumentCurves`, `RatesCurveKind`, `FxPair`, `VolatilityDependency` |
| `cashflow_export.rs` | `instrument_cashflows_json` + `InstrumentCashflowEnvelope` / `CashflowRow` — the per-flow DF/survival/PV export behind the Python and WASM bindings |
| `helpers.rs` | `ValidatedPricingLifecycle`, metric-context construction, `schedule_pv*`, GBM drift schedules, market-scalar lookups, Black-Scholes input collection, inflation-lag resolution |
| `validation.rs` | Structural invariant checks (`validate_date_range_strict`, `validate_money_gt`, `validate_recovery_rate`, `validate_rate_magnitude`, …) |
| `numeric.rs` | `decimal_to_f64` plus the serde adapters that reject non-finite / out-of-range `f64` on the wire |
| `vol_resolution.rs` | `resolve_sigma_at` — the override-then-surface σ lookup precedence |
| `fx_dates.rs` | `fx_spot_date_for_pair` plus re-exports of the core joint-calendar helpers |

### `parameters/`

| File | Contents |
|------|----------|
| `legs.rs` | `PayReceive`, `ParRateMethod`, `FixedLegSpec`, `FloatLegSpec`, `BasisSwapLeg`, `PremiumLegSpec`, `ProtectionLegSpec`, `FinancingLegSpec`, `FinancingRateCompounding`, `TotalReturnLegSpec` |
| `underlying.rs` | `FxUnderlyingParams`, `EquityUnderlyingParams`, `CommodityUnderlyingParams`, `IndexUnderlyingParams` |
| `market.rs` | `OptionType`, `ExerciseStyle`, `SettlementType`, `CreditParams` |
| `conventions.rs` | `BondConvention`, `IRSConvention`, `CommodityConvention` |
| `contract.rs` | `ContractSpec`, `ScheduleSpec` |
| `quanto.rs` | `QuantoSpec` (validated correlation in `[-1, 1]`) |
| `trs_common.rs` | `TrsSide`, `TrsScheduleSpec` — shared by equity and fixed-income TRS |
| `volatility.rs` | `VolatilityModel` plus a re-export of `crate::models::volatility::SABRParameters` |

`OptionMarketParams` is re-exported directly from `finstack-quant-models`, its
canonical owner.

### `pricing/`

| File | Visibility | Contents |
|------|-----------|----------|
| `swap_legs.rs` | `pub` (reachable as `instruments::pricing::swap_legs`) | `pv_floating_leg`, `pv_fixed_leg`, `add_payment_delay`, `FloatingLegParams`, `FixedLegParams`, `LegPeriod`, `CompoundingMethod` |
| `time.rs` | `pub` | Curve-consistent time mapping: `relative_df_discount_curve`, `relative_df_discounting`, `curve_time`, `rate_between_on_dates`, `rate_period_on_dates` |
| `variance_replication.rs` | `pub` | `carr_madan_forward_variance` — shared by equity and FX variance swaps |
| `generic.rs` | private module, `GenericInstrumentPricer` re-exported `#[doc(hidden)]` | Downcasts and calls `Instrument::base_value`; the registry applies scenario shocks around it |
| `trs.rs` | private module, types re-exported | `TrsEngine`, `TrsReturnModel`, `TotalReturnLegParams`, `PeriodReturnInputs` |
| `overnight.rs` | `pub(crate)` | Compounded overnight-RFR projection: lookback, observation shift, rate cutoff, fixings |
| `floating_reset_descriptors.rs` | `pub(crate)` | Turns future-reset floating coupons into node-coupon descriptors for the rates-credit lattice |
| `variance_observations.rs` | `pub(crate)` | Observation schedules for realized-variance instruments, single or joint FX calendars |

## What escapes to the public API

Everything below is re-exported by `instruments/mod.rs`; the module paths under
`common_impl` are not.

- Flat at `instruments::*` — `Instrument`, `Attributes`, `PricingOptions`,
  `OptionGreekKind`, `OptionGreeks`, `OptionGreeksProvider`,
  `OptionGreeksRequest`; `MarketDependencies`, `InstrumentCurves`,
  `RatesCurveKind`, `FxPair`, `VolatilityDependency`; `TrsEngine`,
  `TrsReturnModel`, `TotalReturnLegParams`; `TrsScheduleSpec`, `TrsSide`;
  `fx_spot_date_for_pair`, `add_joint_business_days`, `adjust_joint_calendar`,
  `ResolvedCalendarPair`; and the `parameters` subset listed in
  `instruments/mod.rs` (`FixedLegSpec`, `FloatLegSpec`, `BasisSwapLeg`,
  `PremiumLegSpec`, `ProtectionLegSpec`, `FinancingLegSpec`,
  `FinancingRateCompounding`, `TotalReturnLegSpec`, `PayReceive`,
  `ParRateMethod`, `OptionType`, `ExerciseStyle`, `SettlementType`,
  `CreditParams`, `OptionMarketParams`, `ContractSpec`, `ScheduleSpec`,
  `BondConvention`, `IRSConvention`, and the four `*UnderlyingParams`).
- `instruments::pricing` — glob of `common_impl::pricing`'s public items.
- `instruments::cashflow_export` — `instrument_cashflows_json`,
  `InstrumentCashflowEnvelope`, `CashflowRow`.
- Crate root, via `#[macro_export]` — `impl_instrument_base!`,
  `impl_focused_pricing_overrides!`, `impl_empty_cashflow_provider!` (all three
  in `traits/macros.rs`) and `impl_equity_exotic_traits!` (in
  `traits/option_greeks.rs`; it implements `crate::metrics::HasExpiry` and
  `HasDayCount` for a type with `expiry` and `day_count` fields). `#[macro_export]`
  hoists a macro to the crate root regardless of the module's visibility, so all
  four are public API even though `common_impl` is not. In-crate call sites bring
  them in with `use crate::impl_instrument_base;`.

Not re-exported, and therefore crate-private: `QuantoSpec`,
`CommodityConvention`, `parameters::volatility::VolatilityModel`,
everything in `helpers.rs`, `validation.rs`, `numeric.rs`,
`vol_resolution.rs`, and the `pub(crate)` modules under `pricing/`.

Three of those appear in the type of a `pub` field on a public item and are
therefore visible in rustdoc and in the JSON schema but not nameable by a
downstream crate: `exotics::RangeAccrualTerms::quanto: Option<QuantoSpec>`,
`commodity::{CommodityForward, CommodityOption}::convention:
Option<CommodityConvention>`, and
`instruments::pricing_overrides::ModelConfig::vol_model:
Option<VolatilityModel>`. Adding a `pub` field whose type lives only under
`common_impl` extends that list — export the type from `instruments/mod.rs` in
the same change, or keep the field private.

## The `Instrument` contract

`Instrument: CashflowProvider + Send + Sync`. Required methods, which every
implementor must write:

| Method | Notes |
|--------|-------|
| `id()`, `key()`, `as_any()`, `as_any_mut()`, `attributes()`, `attributes_mut()`, `clone_box()` | All seven come from `impl_instrument_base!(InstrumentType::X)` |
| `base_value(market, as_of) -> Result<Money>` | The unshocked model kernel. Do **not** apply scenario overrides here and do not re-validate — the wrapper lifecycle owns both |
| `market_dependencies() -> Result<MarketDependencies>` | Must declare every curve, surface and FX pair the pricer actually reads; the `*_dependency_completeness.rs` tests enforce this |

The public pricing entry points are `value`, `value_raw`,
`value_raw_with_currency` and `price_with_metrics`. Each runs
`ValidatedPricingLifecycle` (which calls `validate_for_pricing`) — the first
three directly, `price_with_metrics` through `PricerRegistry`. The lifecycle
resolves the effective date through `resolve_pricing_as_of`, invokes
`base_value` (or `base_value_raw` / `base_value_raw_with_currency`, both of
which default to delegating to `base_value`), and applies the instrument's
`ScenarioPricingOverrides` exactly once. That is why `base_value` must stay
shock-free: applying a shock there double-counts it. `cashflow_schedule` and
`dated_cashflows` come from the `CashflowProvider` supertrait in
`finstack-quant-cashflows`, not from `Instrument`, and do not go through this
lifecycle.

Optional hooks with defaults worth knowing about:

| Hook | Default | Override when |
|------|---------|---------------|
| `default_model()` | `ModelKey::Discounting` | The instrument's native path is Black-76, a tree, MC, replication, … |
| `validate_invariants()` | `Ok(())` | Serde cannot express the invariant (positive strike, ordered schedule, distinct indices) |
| `resolve_pricing_as_of()` | returns `requested` | The instrument derives its own valuation date (e.g. a fund anchored to its curve base date) |
| `valuation_details()` | `None` | The result must carry typed details, e.g. the FX-triangulation flag |
| `seed_metric_context()` | no-op | Cashflow generation is expensive and the metrics pipeline would redo it |
| `metrics_equivalent()` / `has_custom_metrics_equivalent()` | `clone_box()` / `false` | Spread and yield metrics need a normalized cashflow basis (PIK → cash) |
| `as_marginable()` | `None` | The type implements `finstack_quant_margin::Marginable` (impls live in `../marginable.rs`) |
| `model_params_snapshot()` / `with_model_params()` | `ModelParamsSnapshot::None` (the variant, not `Option::None`) / clone-or-error | Attribution must revalue the instrument with isolated model parameters |
| `scenario_spread_shock_supported()` | `false` | The pricer consumes `scenario_spread_shock_bp` exactly, with no silent no-op |
| `fx_exposure()`, `expiry()`, `effective_start_date()`, `dividend_schedule_id()`, `funding_curve_id()`, `to_instrument_json()` | `None` | The instrument has the corresponding concept |

`Instrument` sits behind `Arc<dyn Instrument>` across portfolio, scenario and
binding code, so it is a stability surface. New optional capabilities go in
focused provider traits — follow `OptionGreeksProvider` — not as new required
methods.

## Conventions for contributors

- **Numerics.** `f64` for model internals, `Money` for monetary results.
  Convert `Decimal` with `numeric::decimal_to_f64` so the failure is a typed
  `Error::Validation`, not a silent default.
- **Serde.** Wire structs use `#[serde(deny_unknown_fields)]`. Floats with a
  domain constraint use the `numeric.rs` adapter pair
  (`deserialize_positive_f64` / `serialize_positive_f64`, and the
  non-negative / unit-interval / correlation variants) rather than a manual
  check after deserialization.
- **Validation split.** `validation.rs` is deliberately convention-agnostic: it
  enforces ordering, finiteness, positivity and currency agreement only.
  Market-specific defaults (spot lags, business-day conventions, plausible rate
  bounds) belong in the instrument's own `validate_invariants`.
- **Errors.** Instrument code returns `finstack_quant_core::Result`, not
  `crate::Result`. `finstack_quant_valuations::Error` is the wider wrapper
  (`Core`, `Pricing`, `Correlation`, `WaterfallValidation`) used by the pricer
  and calibration layers; it converts one-way into the core error. No
  `unwrap`/`expect` on user input.
- **Two clocks.** A pricer that mixes an instrument day count with a curve day
  count must derive the model rate with `helpers::zero_rate_from_df` (so that
  `exp(-r·t_vol) = df`) or go through `pricing::time`, never divide a
  discount factor's log by an unrelated year fraction.
- **Volatility precedence.** Surface-driven pricers call
  `vol_resolution::resolve_sigma_at` so the `MarketQuoteOverrides::implied_volatility`
  override always wins uniformly.
- **Example dates.** `example()` constructors that need a far expiry use
  `example_constants::FAR_EXPIRY` so they all roll forward together.

## Adding an instrument

The end-to-end checklist lives in [`../README.md`](../README.md#adding-an-instrument).
The parts this directory owns:

1. `impl Instrument for X { impl_instrument_base!(InstrumentType::X); … }` —
   requires fields `id: InstrumentId` and `attributes: Attributes`, and `Clone`.
2. Store the three override bags and add `impl_focused_pricing_overrides!()` if
   the instrument accepts pricing/metric/scenario overrides.
3. Implement `finstack_quant_cashflows::traits::CashflowScheduleSource`.
   `CashflowProvider` — the `Instrument` supertrait that owns
   `cashflow_schedule` and `dated_cashflows` — is blanket-implemented for it, so
   normalization and dated-flow meaning cannot be overridden per instrument.
   Products whose waterfall is intentionally empty use
   `impl_empty_cashflow_provider!(X, representation)`; contingent or exhausted
   products still return an explicit empty schedule tagged so `Placeholder` and
   `NoResidual` stay distinguishable.
4. Reuse `parameters/` types instead of inventing a parallel leg or underlying
   struct, and reuse `pricing/swap_legs` / `pricing/time` instead of open-coding
   discounting.

## Tests

There is no `tests/instruments/common_impl/` directory. This layer is covered
by unit tests colocated in `#[cfg(test)]` modules here, and by the
cross-cutting contract tests in the `instruments` integration target:
`registry_coverage.rs`, `serde_skip_guard.rs`, the five
`*_dependency_completeness.rs` files, and `cashflow_export_schema` (its own
target).

```bash
# whole integration target
cargo nextest run -p finstack-quant-valuations --test instruments

# the contract tests this layer is judged by
cargo nextest run -p finstack-quant-valuations --test instruments dependency_completeness
cargo nextest run -p finstack-quant-valuations --test cashflow_export_schema

# unit tests colocated under src/
cargo nextest run -p finstack-quant-valuations --lib instruments::common_impl
```

Do not run `cargo test` here — it would also run doc tests, which this project
keeps out of the normal loop. Lint with `mise run rust-lint`.

## Related

- [`../README.md`](../README.md) — the `Instrument` contract, JSON registry, and the add-an-instrument checklist
- [`../fixed_income/README.md`](../fixed_income/README.md), [`../rates/README.md`](../rates/README.md), [`../fx/README.md`](../fx/README.md), [`../commodity/README.md`](../commodity/README.md) — family indexes
- [`../../metrics/README.md`](../../metrics/README.md) — `MetricId`, calculators, `MetricRegistry`
- [`../../results/README.md`](../../results/README.md) — `ValuationResult` and `ValuationDetails`
- [`../../market/README.md`](../../market/README.md) — quotes and the convention registry
- Rustdoc carries the per-method arguments, errors and examples; this file
  carries what rustdoc cannot show: which module owns what, and what is
  internal.
