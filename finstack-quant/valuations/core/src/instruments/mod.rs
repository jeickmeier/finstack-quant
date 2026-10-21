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
//! # Organization
//!
//! Instruments are organized by asset class:
//!
//! - `fixed_income`: Bonds, loans, MBS, CMOs, structured credit
//! - `rates`: Swaps, caps/floors, swaptions, deposits, repos
//! - `credit_derivatives`: CDS, indices, tranches, options
//! - `equity`: Options, variance swaps, TRS, DCF, private markets
//! - `fx`: Spots, forwards, swaps, options, barriers, quantos
//! - `commodity`: Forwards, swaps, options
//! - `exotics`: Asian, barrier, lookback, basket options
//! - `composite`: Cross-asset baskets, spreads, and synthetics with frozen quantities
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
//! Cashflow policy is now universal across instruments. Deterministic products
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
//! - **Public**: `finstack_quant_valuations_core::instruments::*` — shared traits
//!   (`Instrument`, ...), parameter types, and the `pricing`
//!   submodule for shared pricing infrastructure used by this crate and by
//!   `finstack-quant-calibration` (`swap_legs`, overnight conventions, time helpers).
//! - **Family-facing**: `common_impl` is public but hidden from the rendered docs; the
//!   instrument families reach their shared kernels through it. A few of its modules are
//!   re-exported through `pricing` and the instrument root.
//!
//! # Supported Instrument Types
//!
//! ## Fixed Income
//! | Type | Description |
//! |------|-------------|
//! | `Bond` | Fixed/floating-rate bonds with embedded options |
//! | `InflationLinkedBond` | TIPS, index-linked gilts |
//! | `ConvertibleBond` | Bonds with equity conversion |
//! | `TermLoan` | Bilateral term loans |
//! | `RevolvingCredit` | Revolving credit facilities |
//! | `AssetBackedFacility` | Warehouse lines against collateral pools |
//! | `StructuredCredit` | ABS, CLO, RMBS, CMBS |
//! | `AgencyMbsPassthrough` | Agency MBS pass-throughs |
//! | `AgencyCmo` | Collateralized mortgage obligations |
//!
//! ## Interest Rates
//! | Type | Description |
//! |------|-------------|
//! | `InterestRateSwap` | Plain vanilla IRS |
//! | `BasisSwap` | Floating-for-floating swaps |
//! | `Swaption` | Options on swaps |
//! | `CapFloor` | Caps, floors, collars |
//! | `Deposit` | Money market deposits |
//! | `Repo` | Repurchase agreements |
//!
//! ## Credit Derivatives
//! | Type | Description |
//! |------|-------------|
//! | `CreditDefaultSwap` | Single-name CDS |
//! | `CdsIndex` | Credit indices (CDX, iTraxx) |
//! | `CdsTranche` | Synthetic CDO tranches |
//! | `CdsOption` | Options on CDS spreads |
//!
//! ## Equity & FX
//! | Type | Description |
//! |------|-------------|
//! | `EquityOption` | Vanilla equity options |
//! | `FxOption` | FX options (Garman-Kohlhagen) |
//! | `VarianceSwap` | Variance/volatility swaps |
//! | `FxSwap` | FX forwards and swaps |
//!
//! ## Composite
//! | Type | Description |
//! |------|-------------|
//! | `CompositeInstrument` | Cross-asset basket or synthetic with frozen quantities |
//!
//! # See Also
//!
//! - [`crate::instruments::Instrument`] for the core instrument trait
//! - [`finstack_quant_core::types::Attributes`] for tagging and scenario selection
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
mod legs;

/// Shared pricing helpers used by this crate and `finstack-quant-calibration`.
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
pub(crate) mod position;
pub use position::Position;

mod breakeven;
pub use breakeven::{BreakevenConfig, BreakevenMode, BreakevenTarget};
pub use common_impl::dependencies::{
    FxPair, InstrumentCurves, MarketDependencies, RatesCurveKind, VolatilityDependency,
};
pub use common_impl::pricing::trs::{TotalReturnLegParams, TrsEngine, TrsReturnModel};
pub use common_impl::traits::{
    GreekBumps, Instrument, OptionGreekKind, OptionGreeks, OptionGreeksProvider,
    OptionGreeksRequest, PricingOptions,
};

pub use common_impl::parameters::bermudan_call::BermudanCallProvision;
pub use common_impl::parameters::conventions::BondConvention;
pub use common_impl::parameters::conventions::NdfQuoteConvention;
pub use common_impl::parameters::conventions::NotionalExchange;
pub use common_impl::parameters::conventions::ResettingSide;
pub use common_impl::parameters::market::CreditParams;
pub use common_impl::parameters::market::SettlementType;
pub use common_impl::parameters::monitoring::Monitoring;
pub use common_impl::parameters::quanto::QuantoSpec;
pub use common_impl::parameters::underlying::CommodityUnderlyingParams;
pub use common_impl::parameters::underlying::EquityUnderlyingParams;
pub use common_impl::parameters::underlying::FxUnderlyingParams;
pub use common_impl::parameters::underlying::IndexUnderlyingParams;
pub use common_impl::parameters::volatility::VolatilityModel;
pub use legs::FinancingLegSpec;
pub use legs::FixedLegSpec;
pub use legs::FloatLegSpec;
pub use legs::PayReceive;
pub use legs::PremiumLegSpec;
pub use legs::ProtectionLegSpec;
pub use legs::TotalReturnLegSpec;

pub use common_impl::parameters::bond_quote::BondQuoteInput;
pub use common_impl::parameters::convertible::{
    AntiDilutionPolicy, ConversionEvent, ConversionPolicy, ConversionSpec, DilutionEvent,
    DividendAdjustment, PriceTrigger,
};
pub use common_impl::parameters::merton_mc::{
    BarrierCrossing, CalibrationParameter, MertonMcCalibrationSpec, MertonMcConfig, PikMode,
    PikSchedule,
};
pub use common_impl::parameters::trs_common::TrsScheduleSpec;

pub use common_impl::listed::future_option::FutureOptionExercise;
pub use common_impl::listed::future_option::FutureOptionModel;
pub use common_impl::listed::future_option::FutureOptionPremiumStyle;
pub use common_impl::listed::future_option::FutureOptionSettlement;
pub use common_impl::listed::future_option::FutureOptionTerms;
pub use common_impl::listed::terms::ListedDeliveryObligation;
pub use common_impl::listed::terms::ListedFutureSettlement;
pub use common_impl::listed::terms::ListedFutureTerms;

/// Pricing overrides module.
pub mod pricing_overrides;
