//! Common parameter types organized by purpose.
//!
//! This module provides shared parameter types used across multiple instruments:
//! - **underlying**: Underlying asset parameters (FX, equity, index)
//! - **legs**: Leg specifications for swaps and structured products
//! - **market**: Market-specific parameters for options and derivatives
//! - **contract**: Contract specifications and general types
//! - **conventions**: Standard market conventions for bonds and swaps

pub mod contract;
pub mod conventions;
pub mod legs;
pub mod market;
pub mod monitoring;
pub mod quanto;
pub mod trs_common;
pub mod underlying;
pub mod volatility;

pub use contract::{ContractSpec, ScheduleSpec};
pub use conventions::{BondConvention, CommodityConvention, IRSConvention};
pub use finstack_quant_models::types::OptionMarketParams;
pub use legs::{
    BasisSwapLeg, FinancingLegSpec, FinancingRateCompounding, FixedLegSpec, FloatLegSpec,
    ParRateMethod, PayReceive, PremiumLegSpec, ProtectionLegSpec, TotalReturnLegSpec,
};
pub use market::{CreditParams, ExerciseStyle, OptionType, SettlementType};
pub use monitoring::Monitoring;
pub use quanto::QuantoSpec;
pub use underlying::{
    CommodityUnderlyingParams, EquityUnderlyingParams, FxUnderlyingParams, IndexUnderlyingParams,
};
pub use volatility::VolatilityModel;
