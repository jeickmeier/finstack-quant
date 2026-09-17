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
//! ```rust
//! use finstack_quant_valuations::instruments::Bond;
//! use finstack_quant_valuations::instruments::Instrument;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::dates::create_date;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let issue = create_date(2025, Month::January, 15)?;
//! let maturity = create_date(2030, Month::January, 15)?;
//!
//! let bond = Bond::fixed(
//!     "US-TREASURY-5Y",
//!     Money::from((1_000_000_i64, Currency::USD)),
//!     finstack_quant_core::types::Rate::from_percent(4.5).expect("valid rate fixture"),
//!     issue,
//!     maturity,
//!     finstack_quant_core::dates::StubKind::None,
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
//! - **Public**: `finstack_quant_valuations::instruments::*` — instrument types, shared traits
//!   (`Instrument`, ...), parameter types, and the `pricing`
//!   submodule for shared pricing infrastructure used by this crate and by
//!   `finstack-quant-calibration` (`swap_legs`, overnight conventions, time helpers).
//! - **Internal**: `common_impl` is crate-private. A few of its modules are
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
//! ## Composite
//! | Type | Description |
//! |------|-------------|
//! | `CompositeInstrument` | Cross-asset basket or synthetic with frozen quantities |
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

/// Shared pricing helpers used by this crate and `finstack-quant-calibration`.
pub mod pricing {
    pub use super::common_impl::pricing::overnight_conventions;
    pub use super::common_impl::pricing::swap_legs;
    pub use super::common_impl::pricing::time;
    #[doc(hidden)]
    pub use super::common_impl::pricing::GenericInstrumentPricer;
}

/// Per-flow cashflow export with DF / survival / PV columns.
///
/// See [`cashflow_export::instrument_cashflows_json`] for the primary entry
/// point used by the Python and WASM bindings.
pub mod cashflow_export {
    pub use super::common_impl::cashflow_export::*;
}

pub use common_impl::fx_dates::{
    add_joint_business_days, adjust_joint_calendar, fx_spot_date_for_pair, ResolvedCalendarPair,
};
pub use finstack_quant_core::dates::fx::resolve_calendar;

/// Model parameter snapshots used by attribution.
pub mod model_params;

/// Canonical long/short position direction.
pub mod position;
pub use position::Position;

/// Commodity derivatives.
pub mod commodity;
/// Generic cross-asset composite and synthetic instruments.
pub mod composite;
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

pub use fixed_income::{
    AgencyCmo, AgencyMbsPassthrough, AgencyProgram, AgencyTba, AssetBackedFacility, Bond,
    BondFuture, BondFutureBuilder, BondFutureSpecs, BondSettlementConvention, CmoTranche,
    CmoTrancheType, CmoWaterfall, ConvertibleBond, DeliverableBond, DollarRoll,
    FIIndexTotalReturnSwap, InflationLinkedBond, PoolType, RevolvingCredit, StructuredCredit,
    TbaTerm, TermLoan,
};

pub use rates::{
    BasisSwap, BermudanSwaption, CapFloor, CmsOption, CmsSpreadOption, CmsSpreadOptionType,
    CmsSwap, CollateralSpec, CollateralType, Deposit, ForwardRateAgreement, InflationCapFloor,
    InflationCapFloorType, InflationSwap, InterestRateFuture, InterestRateSwap,
    RateAveragingMethod, RateOptionType, Repo, RepoType, Swaption, XccySwap, YoYInflationSwap,
};

pub use credit_derivatives::{CDSIndex, CDSOption, CDSTranche, CreditDefaultSwap};

pub use equity::{
    Autocallable, CliquetOption, DiscountedCashFlow, Equity, EquityFuture, EquityFutureOption,
    EquityFutureQuantoSpec, EquityOption, EquityPriceSeriesPolicy, EquityTotalReturnFuture,
    EquityTotalReturnSwap, FinalPayoffType, LeveredRealEstateEquity, PrivateMarketsFund,
    RealEstateAsset, RealEstateFinancing, RealEstateValuationMethod, TerminalValueSpec,
    VarianceSwap, VolIndexContractSpecs, VolatilityIndexFuture, VolatilityIndexFutureOption,
};

pub use fx::FxVarianceSwap;
pub use fx::{
    BarrierDirection, DigitalPayoutType, FxDigitalOption, FxTouchOption, PayoutTiming, TouchType,
};
pub use fx::{
    FxBarrierOption, FxForward, FxFuture, FxFutureOption, FxOption, FxSpot, FxSwap, Ndf,
    QuantoOption,
};

pub use commodity::{
    CommodityAsianOption, CommodityForward, CommodityFuture, CommodityFutureOption,
    CommodityFutureSettlement, CommodityOption, CommoditySpreadOption, CommoditySwap,
    CommoditySwaption,
};

pub use rates::InterestRateFutureOption;

pub use common_impl::listed::{
    FutureOptionExercise, FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement,
    FutureOptionTerms, ListedDeliveryObligation, ListedFutureSettlement, ListedFutureTerms,
};

pub use composite::{
    CompositeExposureReport, CompositeHistoryEngine, CompositeHistoryRow, CompositeInstrument,
    CompositeLegSpec, CompositeMarketObservation, CompositeRebalanceResult, CompositeSpec,
    CompositeState, CompositeTrade, CompositeValuationDetails, PrimitiveAggregate,
    PrimitiveExposure, RebalanceFrequency, RebalanceRule, ResolvedCompositeLeg, WeightingMethod,
};

pub use exotics::{
    AsianOption, AveragingMethod, BarrierOption, Basket, CallableRangeAccrual, LookbackOption,
    LookbackType, RangeAccrual, Snowball, SnowballVariant, Tarn,
};

mod breakeven;
pub use breakeven::{BreakevenConfig, BreakevenMode, BreakevenTarget};
pub use common_impl::dependencies::{
    FxPair, InstrumentCurves, MarketDependencies, RatesCurveKind, VolatilityDependency,
};
pub use common_impl::pricing::{TotalReturnLegParams, TrsEngine, TrsReturnModel};
pub use common_impl::traits::{
    Attributes, Instrument, OptionGreekKind, OptionGreeks, OptionGreeksProvider,
    OptionGreeksRequest, PricingOptions,
};

pub use common_impl::parameters::{
    BasisSwapLeg, BondConvention, CommodityUnderlyingParams, ContractSpec, CreditParams,
    EquityUnderlyingParams, ExerciseStyle, FinancingLegSpec, FinancingRateCompounding,
    FixedLegSpec, FloatLegSpec, FxUnderlyingParams, IRSConvention, IndexUnderlyingParams,
    Monitoring, OptionMarketParams, OptionType, ParRateMethod, PayReceive, PremiumLegSpec,
    ProtectionLegSpec, QuantoSpec, ScheduleSpec, SettlementType, TotalReturnLegSpec,
};

pub use common_impl::parameters::trs_common::{TrsScheduleSpec, TrsSide};

/// Pricing overrides module.
pub mod pricing_overrides;
pub use pricing_overrides::{
    BondRiskBasis, BumpConfig, InstrumentPricingOverrides, MarketQuoteOverrides,
    MetricPricingOverrides, ModelConfig, ScenarioPricingOverrides,
};

pub mod json_loader;

pub use json_loader::{
    cashflow_provider_from_value, registry_tags, InstrumentEnvelope, InstrumentJson,
    INSTRUMENT_CONTRACT,
};
