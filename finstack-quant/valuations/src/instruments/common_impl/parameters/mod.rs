//! Common parameter types organized by purpose.
//!
//! This module provides shared parameter types used across multiple instruments:
//! - **underlying**: Underlying asset parameters (FX, equity, index)
//! - **legs**: Leg specifications for swaps and structured products
//! - **market**: Market-specific parameters for options and derivatives
//! - **contract**: Contract specifications and general types
//! - **conventions**: Standard market conventions for bonds and swaps

pub mod commodity_settlement;
pub mod contract;
pub mod conventions;
pub mod indexation;
pub mod legs;
pub mod market;
pub mod option_market;
pub mod quanto;
pub mod trs_common;
pub mod underlying;
pub mod volatility;

// Re-export commonly used types for convenience
pub use contract::{ContractSpec, ScheduleSpec};
pub use conventions::{BondConvention, CommodityConvention, IRSConvention};
pub use legs::{
    BasisSwapLeg, FinancingLegSpec, FinancingRateCompounding, FixedLegSpec, FloatLegSpec,
    ParRateMethod, PayReceive, PremiumLegSpec, ProtectionLegSpec, TotalReturnLegSpec,
};
pub use market::{
    CapFloorParams, CreditParams, EquityOptionParams, ExerciseStyle, FxOptionParams, OptionType,
    Position, SettlementType,
};
pub use option_market::OptionMarketParams;
pub use quanto::QuantoSpec;
pub use underlying::{
    CommodityUnderlyingParams, EquityUnderlyingParams, FxUnderlyingParams, IndexUnderlyingParams,
    UnderlyingParams,
};
pub use volatility::{SABRParameters, VolatilityModel};

/// Deserialize a field as `T::default()` when the JSON value is `null`.
///
/// `#[serde(default)]` only kicks in when the key is *absent*; an explicit
/// `null` still fails for non-`Option` types. This helper combines both
/// behaviours so that absent **or** `null` both yield `T::default()`.
pub fn deserialize_null_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

use serde::Deserialize;
