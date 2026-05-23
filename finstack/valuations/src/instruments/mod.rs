//! Financial instruments for pricing, risk, and cashflow analysis.
//!
//! This module provides 50+ instrument types across fixed income, rates, credit,
//! equity, FX, commodities, and exotic options. All instruments implement the
//! `Instrument` trait for unified pricing and risk metric computation.
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
//! ```rust
//! use finstack_valuations::instruments::Bond;
//! use finstack_valuations::instruments::Instrument;
//! use finstack_core::currency::Currency;
//! use finstack_core::money::Money;
//! use finstack_core::dates::create_date;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let issue = create_date(2025, Month::January, 15)?;
//! let maturity = create_date(2030, Month::January, 15)?;
//!
//! let bond = Bond::fixed(
//!     "US-TREASURY-5Y",
//!     Money::new(1_000_000.0, Currency::USD),
//!     finstack_core::types::Rate::from_percent(4.5),
//!     issue,
//!     maturity,
//!     "USD-OIS",
//! )?;
//!
//! // Access via Instrument trait
//! assert_eq!(bond.id(), "US-TREASURY-5Y");
//! # Ok(())
//! # }
//! ```
//!
//! # API Layers
//!
//! - **Public**: `finstack_valuations::instruments::*` — instrument types, shared traits
//!   (`Instrument`, `CurveDependencies`, ...), parameter types, and the `pricing`
//!   submodule for shared pricing infrastructure.
//! - **Internal**: `common_impl` is crate-private plumbing; nothing below it needs to be
//!   referenced from outside the crate.
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
//! | `CDSIndex` | Credit indices (CDX, iTraxx) |
//! | `CDSTranche` | Synthetic CDO tranches |
//! | `CDSOption` | Options on CDS spreads |
//!
//! ## Equity & FX
//! | Type | Description |
//! |------|-------------|
//! | `EquityOption` | Vanilla equity options |
//! | `FxOption` | FX options (Garman-Kohlhagen) |
//! | `VarianceSwap` | Variance/volatility swaps |
//! | `FxSwap` | FX forwards and swaps |
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
pub(crate) mod common_impl;

mod marginable;

/// Shared pricing infrastructure (schedules, generic pricers, TRS engine, etc.).
pub mod pricing {
    pub use super::common_impl::pricing::*;
}

/// Per-flow cashflow export with DF / survival / PV columns.
///
/// See [`cashflow_export::instrument_cashflows_json`] for the primary entry
/// point used by the Python and WASM bindings.
pub mod cashflow_export {
    pub use super::common_impl::cashflow_export::*;
}

pub use common_impl::fx_dates::{
    add_joint_business_days, adjust_joint_calendar, roll_spot_date, ResolvedCalendarPair,
};
pub use finstack_core::dates::fx::resolve_calendar;

/// Flatten an instrument's structured `MarketDependencies` into a flat list of
/// `MarketDependency` entries. Used by portfolio factor-model orchestration.
pub mod dependencies_flatten;

// === Category Modules ===
/// Commodity derivatives.
pub mod commodity;
/// Credit derivatives: CDS and related instruments.
pub mod credit_derivatives;
/// Equity instruments and equity derivatives.
pub mod equity;
/// Exotic and path-dependent options.
pub mod exotics;
/// Fixed income instruments: bonds, loans, MBS, and structured products.
pub mod fixed_income;
/// FX instruments and FX derivatives.
pub mod fx;
/// Interest rate derivatives and money market instruments.
pub mod rates;

// === Core Instrument Types ===
// Fixed Income
pub use fixed_income::{
    AgencyCmo, AgencyMbsPassthrough, AgencyProgram, AgencyTba, Bond, BondFuture, BondFutureBuilder,
    BondFutureSpecs, BondSettlementConvention, CmoTranche, CmoTrancheType, CmoWaterfall,
    ConvertibleBond, DeliverableBond, DollarRoll, FIIndexTotalReturnSwap, InflationLinkedBond,
    PoolType, RevolvingCredit, StructuredCredit, TbaTerm, TermLoan,
};

// Rates
pub use rates::{
    BasisSwap, BermudanSwaption, CallableRangeAccrual, CapFloor, CmsOption, CmsSpreadOption,
    CmsSpreadOptionType, CmsSwap, CollateralSpec, CollateralType, Deposit, ForwardRateAgreement,
    InflationCapFloor, InflationCapFloorType, InflationSwap, InterestRateFuture,
    InterestRateOption, InterestRateSwap, IrFutureOption, RangeAccrual, RateOptionType, Repo,
    RepoType, Snowball, SnowballVariant, Swaption, Tarn, XccySwap, YoYInflationSwap,
};

// Credit Derivatives
pub use credit_derivatives::{CDSIndex, CDSOption, CDSTranche, CreditDefaultSwap};

// Equity
pub use equity::{
    Autocallable, CliquetOption, DiscountedCashFlow, Equity, EquityFutureSpecs, EquityIndexFuture,
    EquityOption, EquityTotalReturnSwap, FinalPayoffType, LeveredRealEstateEquity,
    PrivateMarketsFund, RealEstateAsset, RealEstateValuationMethod, TerminalValueSpec,
    VarianceSwap, VolIndexContractSpecs, VolIndexOptionSpecs, VolatilityIndexFuture,
    VolatilityIndexOption,
};

// FX
pub use fx::FxVarianceSwap;
pub use fx::{
    BarrierDirection, DigitalPayoutType, FxDigitalOption, FxTouchOption, PayoutTiming, TouchType,
};
pub use fx::{FxBarrierOption, FxForward, FxOption, FxSpot, FxSwap, Ndf, QuantoOption};

// Commodity
pub use commodity::{
    CommodityAsianOption, CommodityForward, CommodityOption, CommoditySpreadOption, CommoditySwap,
    CommoditySwaption,
};

// Exotics
pub use exotics::{
    AsianOption, AveragingMethod, BarrierOption, BarrierType, Basket, LookbackOption, LookbackType,
};

// === Common Functionality ===
pub use common_impl::dependencies::{FxPair, MarketDependencies};
pub use common_impl::discountable::Discountable;
pub use common_impl::pricing::{TotalReturnLegParams, TrsEngine, TrsReturnModel};
pub use common_impl::traits::{
    Attributes, CurveDependencies, CurveIdVec, DynInstrument, EquityDependencies,
    EquityInstrumentDeps, EquityInstrumentDepsBuilder, Instrument, InstrumentCurves,
    InstrumentCurvesBuilder, OptionGreekKind, OptionGreeks, OptionGreeksProvider,
    OptionGreeksRequest, PricingOptions, RatesCurveKind,
};

// === Parameter Types ===
pub use common_impl::parameters::{
    BasisSwapLeg, BondConvention, CommodityUnderlyingParams, ContractSpec, CreditParams,
    EquityOptionParams, EquityUnderlyingParams, ExerciseStyle, FinancingLegSpec,
    FinancingRateCompounding, FixedLegSpec, FloatLegSpec, FxOptionParams, FxUnderlyingParams,
    IRSConvention, IndexUnderlyingParams, InterestRateOptionParams, OptionMarketParams, OptionType,
    ParRateMethod, PayReceive, PremiumLegSpec, ProtectionLegSpec, ScheduleSpec, SettlementType,
    TotalReturnLegSpec, UnderlyingParams,
};

// Re-export TRS common types
pub use common_impl::parameters::trs_common::{TrsScheduleSpec, TrsSide};

/// Pricing overrides module.
pub mod pricing_overrides;
pub use pricing_overrides::{
    BondRiskBasis, BreakevenConfig, BreakevenMode, BreakevenTarget, BumpConfig,
    InstrumentPricingOverrides, MarketQuoteOverrides, MetricPricingOverrides, ModelConfig,
    PricingOverrides, ScenarioPricingOverrides,
};

// === JSON Import/Export ===
pub mod json_loader;

pub use json_loader::{cashflow_provider_from_value, InstrumentEnvelope, InstrumentJson};
