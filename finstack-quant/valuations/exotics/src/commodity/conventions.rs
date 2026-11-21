//! Standard commodity market conventions by product type.

use finstack_quant_core::dates::BusinessDayConvention;
use serde::{Deserialize, Serialize};

/// Standard commodity market conventions by product type.
///
/// Each variant provides market-standard defaults for settlement days and
/// business day convention. Use when constructing commodity forwards, options,
/// or swaps without explicitly specifying these parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CommodityConvention {
    /// WTI Crude Oil: T+2, Following, NYMEX calendar
    WtiCrude,
    /// Brent Crude Oil: T+2, Following, ICE calendar
    BrentCrude,
    /// Henry Hub Natural Gas: T+2, Following, NYMEX calendar
    NaturalGas,
    /// COMEX Gold: T+2, Modified Following, COMEX calendar
    Gold,
    /// COMEX Silver: T+2, Modified Following, COMEX calendar
    Silver,
    /// LME Copper: T+2, Following, LME calendar
    Copper,
    /// CBOT Agricultural (Corn, Wheat, Soybeans): T+2, Following, CBOT calendar
    Agricultural,
    /// Power/Electricity: T+1, Modified Following, NERC calendar
    Power,
}

impl CommodityConvention {
    /// Settlement days (T+N) for this commodity market.
    pub fn settlement_days(&self) -> u32 {
        match self {
            CommodityConvention::Power => 1,
            CommodityConvention::WtiCrude
            | CommodityConvention::BrentCrude
            | CommodityConvention::NaturalGas
            | CommodityConvention::Gold
            | CommodityConvention::Silver
            | CommodityConvention::Copper
            | CommodityConvention::Agricultural => 2,
        }
    }

    /// Business day convention for this commodity market.
    pub fn business_day_convention(&self) -> BusinessDayConvention {
        match self {
            CommodityConvention::Gold
            | CommodityConvention::Silver
            | CommodityConvention::Power => BusinessDayConvention::ModifiedFollowing,
            CommodityConvention::WtiCrude
            | CommodityConvention::BrentCrude
            | CommodityConvention::NaturalGas
            | CommodityConvention::Copper
            | CommodityConvention::Agricultural => BusinessDayConvention::Following,
        }
    }
}

impl std::fmt::Display for CommodityConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommodityConvention::WtiCrude => write!(f, "wti_crude"),
            CommodityConvention::BrentCrude => write!(f, "brent_crude"),
            CommodityConvention::NaturalGas => write!(f, "natural_gas"),
            CommodityConvention::Gold => write!(f, "gold"),
            CommodityConvention::Silver => write!(f, "silver"),
            CommodityConvention::Copper => write!(f, "copper"),
            CommodityConvention::Agricultural => write!(f, "agricultural"),
            CommodityConvention::Power => write!(f, "power"),
        }
    }
}

impl std::str::FromStr for CommodityConvention {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "wti_crude" => Ok(CommodityConvention::WtiCrude),
            "brent_crude" => Ok(CommodityConvention::BrentCrude),
            "natural_gas" => Ok(CommodityConvention::NaturalGas),
            "gold" => Ok(CommodityConvention::Gold),
            "silver" => Ok(CommodityConvention::Silver),
            "copper" => Ok(CommodityConvention::Copper),
            "agricultural" => Ok(CommodityConvention::Agricultural),
            "power" => Ok(CommodityConvention::Power),
            _ => Err(format!("Unknown commodity convention: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commodity_convention_settlement_days() {
        assert_eq!(CommodityConvention::WtiCrude.settlement_days(), 2);
        assert_eq!(CommodityConvention::BrentCrude.settlement_days(), 2);
        assert_eq!(CommodityConvention::NaturalGas.settlement_days(), 2);
        assert_eq!(CommodityConvention::Gold.settlement_days(), 2);
        assert_eq!(CommodityConvention::Silver.settlement_days(), 2);
        assert_eq!(CommodityConvention::Copper.settlement_days(), 2);
        assert_eq!(CommodityConvention::Agricultural.settlement_days(), 2);
        assert_eq!(CommodityConvention::Power.settlement_days(), 1);
    }

    #[test]
    fn commodity_convention_business_day() {
        assert_eq!(
            CommodityConvention::WtiCrude.business_day_convention(),
            BusinessDayConvention::Following
        );
        assert_eq!(
            CommodityConvention::BrentCrude.business_day_convention(),
            BusinessDayConvention::Following
        );
        assert_eq!(
            CommodityConvention::NaturalGas.business_day_convention(),
            BusinessDayConvention::Following
        );
        assert_eq!(
            CommodityConvention::Copper.business_day_convention(),
            BusinessDayConvention::Following
        );
        assert_eq!(
            CommodityConvention::Agricultural.business_day_convention(),
            BusinessDayConvention::Following
        );
        assert_eq!(
            CommodityConvention::Gold.business_day_convention(),
            BusinessDayConvention::ModifiedFollowing
        );
        assert_eq!(
            CommodityConvention::Silver.business_day_convention(),
            BusinessDayConvention::ModifiedFollowing
        );
        assert_eq!(
            CommodityConvention::Power.business_day_convention(),
            BusinessDayConvention::ModifiedFollowing
        );
    }

    #[test]
    fn commodity_convention_from_str() {
        assert_eq!(
            "wti_crude".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::WtiCrude
        );
        assert_eq!(
            "natural_gas".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::NaturalGas
        );
        assert_eq!(
            "gold".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::Gold
        );
        for retired in ["wti", "cl", "ng", "gc", "xau", "hg", "corn"] {
            assert!(retired.parse::<CommodityConvention>().is_err());
        }
    }

    #[test]
    fn commodity_convention_display() {
        assert_eq!(format!("{}", CommodityConvention::WtiCrude), "wti_crude");
        assert_eq!(
            format!("{}", CommodityConvention::BrentCrude),
            "brent_crude"
        );
        assert_eq!(
            format!("{}", CommodityConvention::NaturalGas),
            "natural_gas"
        );
        assert_eq!(format!("{}", CommodityConvention::Gold), "gold");
        assert_eq!(format!("{}", CommodityConvention::Silver), "silver");
        assert_eq!(format!("{}", CommodityConvention::Copper), "copper");
        assert_eq!(
            format!("{}", CommodityConvention::Agricultural),
            "agricultural"
        );
        assert_eq!(format!("{}", CommodityConvention::Power), "power");
    }
}
