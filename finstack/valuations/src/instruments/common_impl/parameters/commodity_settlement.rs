//! Commodity settlement specification.

use crate::instruments::common_impl::parameters::SettlementType;
use finstack_core::dates::BusinessDayConvention;

/// Settlement parameters for commodity contracts.
///
/// Groups settlement-related fields that are often set together or derived
/// from a commodity convention.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CommoditySettlementSpec {
    /// Settlement type (physical or cash).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_type: Option<SettlementType>,
    /// Settlement lag in business days (T+N). Defaults to convention or T+2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lag_days: Option<u32>,
    /// Calendar ID for settlement date adjustments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    /// Business day convention for settlement date adjustment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bdc: Option<BusinessDayConvention>,
    /// Exchange identifier (e.g., "NYMEX", "ICE").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
    /// Contract month (e.g., "2025M03" for March 2025).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_month: Option<String>,
}
