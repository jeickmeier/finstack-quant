//! Common parameter types organized by purpose.
//!
//! This module provides shared parameter types used across multiple instruments:
//! - **underlying**: Underlying asset parameters (FX, equity, index)
//! - **legs**: Leg specifications for swaps and structured products
//! - **market**: Market-specific parameters for options and derivatives
//! - **conventions**: Standard market conventions for bonds and swaps
//! - **bermudan_call**: Issuer call provision shared by callable rate exotics

pub mod bermudan_call;
pub mod conventions;
pub mod convertible;
pub mod legs;
pub mod market;
pub mod merton_mc;
pub mod monitoring;
pub mod quanto;
pub mod trs_common;
pub mod underlying;
pub mod volatility;

pub use bermudan_call::BermudanCallProvision;
pub use conventions::{
    BondConvention, CommodityConvention, NdfQuoteConvention, NotionalExchange, ResettingSide,
};
pub use finstack_quant_models::types::OptionMarketParams;
pub use legs::{
    FinancingLegSpec, FixedLegSpec, FloatLegSpec, PayReceive, PremiumLegSpec, ProtectionLegSpec,
    TotalReturnLegSpec,
};
pub use market::{CreditParams, ExerciseStyle, OptionType, SettlementType};
pub use monitoring::Monitoring;
pub use quanto::QuantoSpec;
pub use underlying::{
    CommodityUnderlyingParams, EquityUnderlyingParams, FxUnderlyingParams, IndexUnderlyingParams,
};
pub use volatility::VolatilityModel;
