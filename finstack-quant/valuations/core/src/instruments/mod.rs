//! Financial instruments for pricing, risk, and cashflow analysis.
//!
//! This module defines the `Instrument` trait every instrument implements for
//! unified pricing and risk metric computation, with the pricing options, parameter
//! types and shared kernels the instrument families build on. The families
//! themselves (50+ instrument types across fixed income, rates, credit, equity, FX,
//! commodities, and exotic options) live in the umbrella crate.
//!
//! # Documentation Rules For Instrument APIs
//!
//! Instrument docs should make these contracts explicit:
//!
//! - **Use typed rates when the example is about meaning, not convenience**.
//!   Raw literals such as `0.05` are only appropriate when the doc explicitly says
//!   the value is a decimal annual rate.
//! - **Document curve roles and conventions near the constructor**. If an
//!   instrument depends on discount, forward, credit, dividend, or volatility
//!   inputs, its public docs should say which identifiers are required and how
//!   they are interpreted.
//! - **Separate contractual terms from portfolio economics**. Instrument docs
//!   should describe cashflows and market conventions; funding, trade price, and
//!   book-level effects belong in higher-level APIs unless the instrument exposes
//!   them directly.
//!
//! # Core Trait
//!
//! All instruments implement `Instrument`, providing:
//! - `id()`: Unique instrument identifier
//! - `key()`: Type classification for pricer dispatch
//! - `value()`: Fast NPV calculation
//! - `price_with_metrics()`: NPV plus risk metrics (DV01, Greeks, etc.)
//! - `cashflow_schedule()`: Canonical future-dated waterfall schedule
//! - `dated_cashflows()`: Derived flattened `(Date, Money)` convenience view
//!
//! Cashflow policy is universal across instruments. Deterministic products
//! emit contractual or projected schedules, while contingent or exhausted
//! products still return an explicit empty schedule tagged with metadata that
//! distinguishes `Placeholder` from `NoResidual`.
//!
//! # Quick Example
//!
//! The [`Instrument`](crate::instruments::Instrument) documentation implements the trait for a one-payment note
//! and prices it; the umbrella crate's `instruments` module shows the same call on a
//! bond.
//!
//! # API Layers
//!
//! - **Public**: `finstack_quant_valuations_core::instruments::*` — the contract
//!   types (`Instrument`, `Attributes`, `PricingOptions`, market dependencies,
//!   parameter specs, listed-contract terms, pricing overrides) and the
//!   [`pricing`](crate::instruments::pricing) module of shared pricing kernels
//!   used by the families and by `finstack-quant-calibration`.
//! - **Family-facing**: `common_impl` is public but hidden from the rendered docs;
//!   it holds the helpers, volatility resolution and margin glue the instrument
//!   families share.
//!
//! The concrete instrument types live in the family crates and are assembled into
//! one namespace by the umbrella crate's `instruments` module.
//!
//! # See Also
//!
//! - [`crate::instruments::Instrument`] for the core instrument trait
//! - [`crate::instruments::Attributes`] for tagging and scenario selection
//! - [`crate::pricer`] for pricing registry and dispatch
//! - [`crate::metrics`] for risk metric calculations
//!
//! # References
//!
//! - Day-count and schedule conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - Bond-market accrued-interest conventions: `docs/REFERENCES.md#icma-rule-book`
//! - Fixed-income risk and hedging intuition: `docs/REFERENCES.md#tuckman-serrat-fixed-income`

// Common functionality (traits, macros, helpers)
#[macro_use]
#[doc(hidden)]
pub mod common_impl;

pub mod pricing;

/// The persisted instrument-envelope contract: schema marker, size cap,
/// load-error mapping.
pub mod envelope;

/// Per-flow cashflow export with DF / survival / PV columns.
///
/// See [`cashflow_export::export_cashflows`] for the engine behind the
/// umbrella crate's JSON entry points and the Python and WASM bindings.
pub mod cashflow_export;

/// Model parameter snapshots used by attribution.
pub mod model_params;

/// Canonical long/short position direction.
pub mod position;
pub use position::Position;

mod breakeven;
pub use breakeven::{BreakevenConfig, BreakevenMode, BreakevenTarget};
pub use common_impl::dependencies::{
    FxPair, InstrumentCurves, MarketDependencies, RatesCurveKind, VolatilityDependency,
};
pub use common_impl::traits::{
    Attributes, GreekBumps, Instrument, OptionGreekKind, OptionGreeks, OptionGreeksProvider,
    OptionGreeksRequest, PricingOptions,
};

pub use common_impl::parameters::{
    BermudanCallProvision, BondConvention, CommodityConvention, CommodityUnderlyingParams,
    CreditParams, EquityUnderlyingParams, ExerciseStyle, FinancingLegSpec, FixedLegSpec,
    FloatLegSpec, FxUnderlyingParams, IndexUnderlyingParams, Monitoring, NdfQuoteConvention,
    NotionalExchange, OptionMarketParams, OptionType, PayReceive, PremiumLegSpec,
    ProtectionLegSpec, QuantoSpec, ResettingSide, SettlementType, TotalReturnLegSpec,
    VolatilityModel,
};

pub use common_impl::parameters::bond_quote::BondQuoteInput;
pub use common_impl::parameters::convertible::{
    AntiDilutionPolicy, ConversionEvent, ConversionPolicy, ConversionSpec, DilutionEvent,
    DividendAdjustment, PriceTrigger,
};
#[doc(hidden)]
pub use common_impl::parameters::legs::simple_average_unsupported;
pub use common_impl::parameters::merton_mc::{
    BarrierCrossing, CalibrationParameter, MertonMcCalibrationSpec, MertonMcConfig, PikMode,
    PikSchedule,
};
pub use common_impl::parameters::trs_common::TrsScheduleSpec;

pub use common_impl::listed::{
    FutureOptionExercise, FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement,
    FutureOptionTerms, ListedFutureSettlement, ListedFutureTerms,
};

mod pricing_overrides;
#[doc(hidden)]
pub use pricing_overrides::resolve_rates_credit_config;
pub use pricing_overrides::{
    BondRiskBasis, BumpConfig, InstrumentPricingOverrides, MarketQuoteOverrides, MertonMcOverride,
    MetricPricingOverrides, ModelConfig, ScenarioPricingOverrides, ShortRateTreeModel,
    ThetaDayBasis, VolSurfaceExtrapolation,
};
