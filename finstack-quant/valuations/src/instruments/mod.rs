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
//! - **Framework**: the trait, options, parameters and shared kernels are defined in
//!   `finstack-quant-valuations-core` and re-exported here unchanged; `common_impl` is
//!   public but hidden from the rendered docs.
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
//! - [`crate::instruments::Attributes`] for tagging and scenario selection
//! - [`crate::pricer`] for pricing registry and dispatch
//! - [`crate::metrics`] for risk metric calculations
//!
//! # References
//!
//! - Day-count and schedule conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - Bond-market accrued-interest conventions: `docs/REFERENCES.md#icma-rule-book`
//! - Fixed-income risk and hedging intuition: `docs/REFERENCES.md#tuckman-serrat-fixed-income`

pub use finstack_quant_valuations_core::instruments::*;

/// Per-flow cashflow export with DF / survival / PV columns.
///
/// See [`cashflow_export::instrument_cashflows`] for the primary entry
/// point used by the Python and WASM bindings.
pub mod cashflow_export;

/// Commodity derivatives.
pub mod commodity {
    pub use finstack_quant_valuations_exotics::commodity::*;
}
/// Generic cross-asset composite and synthetic instruments.
pub mod composite;
/// Credit derivatives: CDS and related instruments.
pub mod credit_derivatives {
    pub use finstack_quant_valuations_credit::*;
}
/// Equity instruments and equity derivatives.
pub mod equity;
/// Exotic and path-dependent options.
pub mod exotics;
/// Fixed income instruments: bonds, loans, MBS, and structured products.
pub mod fixed_income;
/// FX instruments and FX derivatives.
pub mod fx {
    pub use finstack_quant_valuations_fx::*;
}
/// Interest rate derivatives and money market instruments.
pub mod rates {
    pub use finstack_quant_valuations_rates::*;
}

pub use fixed_income::asset_backed_facility::AssetBackedFacility;
pub use fixed_income::bond::{Bond, BondSettlementConvention};
pub use fixed_income::bond_future::{BondFuture, BondFutureBuilder, DeliverableBond};
pub use fixed_income::cmo::{AgencyCmo, CmoTranche, CmoTrancheType, CmoWaterfall};
pub use fixed_income::convertible::ConvertibleBond;
pub use fixed_income::dollar_roll::DollarRoll;
pub use fixed_income::fi_trs::FiIndexTotalReturnSwap;
pub use fixed_income::inflation_linked_bond::InflationLinkedBond;
pub use fixed_income::mbs_passthrough::{AgencyMbsPassthrough, AgencyProgram, PoolType};
pub use fixed_income::revolving_credit::RevolvingCredit;
pub use fixed_income::structured_credit::StructuredCredit;
pub use fixed_income::tba::{AgencyTba, TbaTerm};
pub use fixed_income::term_loan::TermLoan;

pub use rates::basis_swap::BasisSwap;
pub use rates::cap_floor::{CapFloor, RateOptionType};
pub use rates::cms_option::CmsOption;
pub use rates::cms_spread_option::CmsSpreadOption;
pub use rates::cms_swap::CmsSwap;
pub use rates::deposit::Deposit;
pub use rates::fra::ForwardRateAgreement;
pub use rates::inflation_cap_floor::InflationCapFloor;
pub use rates::inflation_swap::{InflationSwap, YoYInflationSwap};
pub use rates::ir_future::InterestRateFuture;
pub use rates::irs::InterestRateSwap;
pub use rates::repo::{CollateralSpec, CollateralType, Repo, RepoType};
pub use rates::swaption::{BermudanSwaption, Swaption};
pub use rates::xccy_swap::XccySwap;

pub use credit_derivatives::{
    cds::CreditDefaultSwap, cds_index::CdsIndex, cds_option::CdsOption, cds_tranche::CdsTranche,
};

pub use equity::autocallable::Autocallable;
pub use equity::cliquet_option::CliquetOption;
pub use equity::dcf_equity::{DiscountedCashFlow, TerminalValueSpec};
pub use equity::equity_future::EquityFuture;
pub use equity::equity_future_option::EquityFutureOption;
pub use equity::equity_option::EquityOption;
pub use equity::equity_total_return_future::EquityTotalReturnFuture;
pub use equity::equity_trs::EquityTotalReturnSwap;
pub use equity::pe_fund::PrivateMarketsFund;
pub use equity::spot::Equity;
pub use equity::variance_swap::{EquityPriceSeriesPolicy, VarianceSwap};
pub use equity::vol_index_future::VolatilityIndexFuture;
pub use equity::vol_index_future_option::VolatilityIndexFutureOption;
pub use equity::{
    LeveredRealEstateEquity, RealEstateAsset, RealEstateFinancing, RealEstateValuationMethod,
};

pub use fx::{
    fx_barrier_option::FxBarrierOption,
    fx_digital_option::{DigitalPayoutType, FxDigitalOption},
    fx_forward::FxForward,
    fx_future::FxFuture,
    fx_future_option::FxFutureOption,
    fx_option::FxOption,
    fx_spot::FxSpot,
    fx_swap::FxSwap,
    fx_touch_option::{FxTouchOption, TouchType},
    fx_variance_swap::FxVarianceSwap,
    ndf::Ndf,
    quanto_option::QuantoOption,
};

pub use commodity::{
    commodity_asian_option::CommodityAsianOption,
    commodity_forward::CommodityForward,
    commodity_future::{CommodityFuture, CommodityFutureFixing},
    commodity_future_option::CommodityFutureOption,
    commodity_spot_option::{CommoditySpotOption, CommoditySpotOptionExercise},
    commodity_spread_option::CommoditySpreadOption,
    commodity_swap::CommoditySwap,
    commodity_swaption::CommoditySwaption,
};

pub use rates::ir_future_option::InterestRateFutureOption;

pub use composite::{
    CompositeExposureReport, CompositeHistoryRow, CompositeInstrument, CompositeLegSpec,
    CompositeMarketObservation, CompositeRebalanceResult, CompositeSpec, CompositeState,
    CompositeTrade, CompositeValuationDetails, PrimitiveAggregate, PrimitiveExposure,
    RebalanceRule, ResolvedCompositeLeg, WeightingMethod,
};

pub use exotics::{
    asian_option::AsianOption,
    barrier_option::BarrierOption,
    callable_range_accrual::CallableRangeAccrual,
    lookback_option::{LookbackOption, LookbackType},
    range_accrual::RangeAccrual,
    snowball::{Snowball, SnowballVariant},
    tarn::Tarn,
    Basket,
};

pub mod json_loader;

pub use json_loader::{
    instrument_json_of, market_dependencies_of, registry_tags, InstrumentEnvelope, InstrumentJson,
};
