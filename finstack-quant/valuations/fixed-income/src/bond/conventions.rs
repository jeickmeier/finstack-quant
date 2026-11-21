//! Standard bond market conventions by region/issuer.

use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::math::Compounding;
use serde::{Deserialize, Serialize};

/// Standard bond market conventions by region/issuer.
///
/// Each variant provides market-standard defaults for day count, frequency,
/// settlement days, and other conventions. Use [`BondConvention::settlement_days`]
/// to get the standard settlement lag for each market.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BondConvention {
    /// US Treasury: Semi-annual, ACT/ACT ICMA, T+1 settlement
    UsTreasury,
    /// US Agency (FNMA, FHLMC, FHLB): Semi-annual, 30/360, T+1 settlement
    UsAgency,
    /// German Bund: Annual, ACT/ACT ICMA, T+2 settlement
    GermanBund,
    /// UK Gilt: Semi-annual, ACT/ACT ICMA, T+1 settlement, 7-day ex-coupon
    UkGilt,
    /// French OAT: Annual, ACT/ACT ICMA, T+2 settlement
    FrenchOat,
    /// Japanese Government Bond (cross-border): Semi-annual, ACT/365F, T+2.
    ///
    /// Tokyo quotes the bond on **Japanese simple yield** (`japanese_simple_yield`).
    /// Street [`finstack_quant_valuations_core::metrics::MetricId::Ytm`] remains the
    /// secondary-market IRR and is **not** switched for `Jgb`.
    Jgb,
    /// US corporate cash: Semi-annual, 30/360, T+1 settlement (SIFMA May 2024).
    UsCorporate,
    /// EUR corporate: Annual, ACT/ACT ICMA, T+2 settlement, TARGET2.
    EurCorporate,
}

impl BondConvention {
    /// Day count convention for this market.
    pub fn day_count(&self) -> DayCount {
        match self {
            BondConvention::UsTreasury
            | BondConvention::GermanBund
            | BondConvention::UkGilt
            | BondConvention::FrenchOat
            | BondConvention::EurCorporate => DayCount::ActActIsma,
            BondConvention::UsAgency | BondConvention::UsCorporate => DayCount::Thirty360,
            BondConvention::Jgb => DayCount::Act365F,
        }
    }

    /// Payment frequency for this market.
    pub fn frequency(&self) -> Tenor {
        match self {
            BondConvention::UsTreasury
            | BondConvention::UsAgency
            | BondConvention::UkGilt
            | BondConvention::UsCorporate
            | BondConvention::Jgb => Tenor::semi_annual(),
            BondConvention::GermanBund
            | BondConvention::FrenchOat
            | BondConvention::EurCorporate => Tenor::annual(),
        }
    }

    /// Quote compounding of the z-spread for this market.
    pub fn quote_compounding(&self) -> Compounding {
        match self {
            BondConvention::UsTreasury
            | BondConvention::UsAgency
            | BondConvention::UkGilt
            | BondConvention::UsCorporate
            | BondConvention::Jgb => Compounding::SEMI_ANNUAL,
            BondConvention::GermanBund
            | BondConvention::FrenchOat
            | BondConvention::EurCorporate => Compounding::Periodic(std::num::NonZeroU32::MIN),
        }
    }

    /// Business day convention for this market.
    pub fn business_day_convention(&self) -> BusinessDayConvention {
        match self {
            BondConvention::UsTreasury
            | BondConvention::GermanBund
            | BondConvention::UkGilt
            | BondConvention::FrenchOat
            | BondConvention::Jgb => BusinessDayConvention::Following,
            BondConvention::UsAgency
            | BondConvention::UsCorporate
            | BondConvention::EurCorporate => BusinessDayConvention::ModifiedFollowing,
        }
    }

    /// Default stub convention for convenience construction.
    pub fn stub_convention(&self) -> StubKind {
        StubKind::ShortFront
    }

    /// Settlement days (T+N) for this market.
    pub fn settlement_days(&self) -> u32 {
        match self {
            BondConvention::UsTreasury
            | BondConvention::UsAgency
            | BondConvention::UkGilt
            | BondConvention::UsCorporate => 1,
            BondConvention::EurCorporate
            | BondConvention::GermanBund
            | BondConvention::FrenchOat
            | BondConvention::Jgb => 2,
        }
    }

    /// Ex-coupon days for this market (if applicable).
    pub fn ex_coupon_days(&self) -> Option<u32> {
        match self {
            BondConvention::UkGilt => Some(7),
            _ => None,
        }
    }

    /// Calendar identifier for this market.
    pub fn calendar_id(&self) -> Option<&'static str> {
        match self {
            BondConvention::UsTreasury | BondConvention::UsAgency => Some("sifma"),
            BondConvention::UsCorporate => Some("usny"),
            BondConvention::EurCorporate
            | BondConvention::GermanBund
            | BondConvention::FrenchOat => Some("target2"),
            BondConvention::UkGilt => Some("gblo"),
            BondConvention::Jgb => Some("jpto"),
        }
    }
}

impl std::fmt::Display for BondConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BondConvention::UsTreasury => write!(f, "us_treasury"),
            BondConvention::UsAgency => write!(f, "us_agency"),
            BondConvention::GermanBund => write!(f, "german_bund"),
            BondConvention::UkGilt => write!(f, "uk_gilt"),
            BondConvention::FrenchOat => write!(f, "french_oat"),
            BondConvention::Jgb => write!(f, "jgb"),
            BondConvention::UsCorporate => write!(f, "us_corporate"),
            BondConvention::EurCorporate => write!(f, "eur_corporate"),
        }
    }
}

impl std::str::FromStr for BondConvention {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "us_treasury" => Ok(BondConvention::UsTreasury),
            "us_agency" => Ok(BondConvention::UsAgency),
            "german_bund" => Ok(BondConvention::GermanBund),
            "uk_gilt" => Ok(BondConvention::UkGilt),
            "french_oat" => Ok(BondConvention::FrenchOat),
            "jgb" => Ok(BondConvention::Jgb),
            "us_corporate" => Ok(BondConvention::UsCorporate),
            "eur_corporate" => Ok(BondConvention::EurCorporate),
            _ => Err(format!("Unknown bond convention: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bond_convention_day_counts() {
        assert_eq!(BondConvention::UsTreasury.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::GermanBund.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::UkGilt.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::FrenchOat.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::UsAgency.day_count(), DayCount::Thirty360);
        assert_eq!(BondConvention::UsCorporate.day_count(), DayCount::Thirty360);
        assert_eq!(
            BondConvention::EurCorporate.day_count(),
            DayCount::ActActIsma
        );
        assert_eq!(BondConvention::Jgb.day_count(), DayCount::Act365F);
    }

    #[test]
    fn bond_convention_frequencies() {
        assert_eq!(BondConvention::UsTreasury.frequency(), Tenor::semi_annual());
        assert_eq!(BondConvention::UsAgency.frequency(), Tenor::semi_annual());
        assert_eq!(BondConvention::UkGilt.frequency(), Tenor::semi_annual());
        assert_eq!(
            BondConvention::UsCorporate.frequency(),
            Tenor::semi_annual()
        );
        assert_eq!(BondConvention::EurCorporate.frequency(), Tenor::annual());
        assert_eq!(BondConvention::Jgb.frequency(), Tenor::semi_annual());
        assert_eq!(BondConvention::GermanBund.frequency(), Tenor::annual());
        assert_eq!(BondConvention::FrenchOat.frequency(), Tenor::annual());
    }

    #[test]
    fn bond_convention_settlement_days() {
        assert_eq!(BondConvention::UsTreasury.settlement_days(), 1);
        assert_eq!(BondConvention::UsAgency.settlement_days(), 1);
        assert_eq!(BondConvention::UkGilt.settlement_days(), 1);
        assert_eq!(BondConvention::UsCorporate.settlement_days(), 1);
        assert_eq!(BondConvention::EurCorporate.settlement_days(), 2);
        assert_eq!(BondConvention::GermanBund.settlement_days(), 2);
        assert_eq!(BondConvention::FrenchOat.settlement_days(), 2);
        assert_eq!(BondConvention::Jgb.settlement_days(), 2);
    }

    #[test]
    fn bond_convention_ex_coupon() {
        assert_eq!(BondConvention::UkGilt.ex_coupon_days(), Some(7));
        assert_eq!(BondConvention::UsTreasury.ex_coupon_days(), None);
        assert_eq!(BondConvention::UsAgency.ex_coupon_days(), None);
        assert_eq!(BondConvention::UsCorporate.ex_coupon_days(), None);
        assert_eq!(BondConvention::EurCorporate.ex_coupon_days(), None);
        assert_eq!(BondConvention::GermanBund.ex_coupon_days(), None);
        assert_eq!(BondConvention::FrenchOat.ex_coupon_days(), None);
        assert_eq!(BondConvention::Jgb.ex_coupon_days(), None);
    }

    #[test]
    fn bond_convention_calendar_ids() {
        assert_eq!(BondConvention::UsTreasury.calendar_id(), Some("sifma"));
        assert_eq!(BondConvention::UsAgency.calendar_id(), Some("sifma"));
        assert_eq!(BondConvention::UsCorporate.calendar_id(), Some("usny"));
        assert_eq!(BondConvention::EurCorporate.calendar_id(), Some("target2"));
        assert_eq!(BondConvention::GermanBund.calendar_id(), Some("target2"));
        assert_eq!(BondConvention::FrenchOat.calendar_id(), Some("target2"));
        assert_eq!(BondConvention::UkGilt.calendar_id(), Some("gblo"));
        assert_eq!(BondConvention::Jgb.calendar_id(), Some("jpto"));
    }

    #[test]
    fn bond_convention_from_str() {
        assert_eq!(
            "us_treasury".parse::<BondConvention>().unwrap(),
            BondConvention::UsTreasury
        );
        assert_eq!(
            "us_agency".parse::<BondConvention>().unwrap(),
            BondConvention::UsAgency
        );
        assert_eq!(
            "jgb".parse::<BondConvention>().unwrap(),
            BondConvention::Jgb
        );
        assert_eq!(
            "german_bund".parse::<BondConvention>().unwrap(),
            BondConvention::GermanBund
        );
        assert_eq!(
            "uk_gilt".parse::<BondConvention>().unwrap(),
            BondConvention::UkGilt
        );
        assert_eq!(
            "french_oat".parse::<BondConvention>().unwrap(),
            BondConvention::FrenchOat
        );
        assert_eq!(
            "us_corporate".parse::<BondConvention>().unwrap(),
            BondConvention::UsCorporate
        );
        assert_eq!(
            "eur_corporate".parse::<BondConvention>().unwrap(),
            BondConvention::EurCorporate
        );
        assert!("corporate".parse::<BondConvention>().is_err());

        for retired in ["ust", "agency", "fnma", "japanese", "bund", "gilt"] {
            assert!(retired.parse::<BondConvention>().is_err());
        }
    }

    #[test]
    fn bond_convention_display() {
        assert_eq!(format!("{}", BondConvention::UsTreasury), "us_treasury");
        assert_eq!(format!("{}", BondConvention::UsAgency), "us_agency");
        assert_eq!(format!("{}", BondConvention::Jgb), "jgb");
        assert_eq!(format!("{}", BondConvention::GermanBund), "german_bund");
        assert_eq!(format!("{}", BondConvention::UkGilt), "uk_gilt");
        assert_eq!(format!("{}", BondConvention::FrenchOat), "french_oat");
        assert_eq!(format!("{}", BondConvention::UsCorporate), "us_corporate");
        assert_eq!(format!("{}", BondConvention::EurCorporate), "eur_corporate");
    }
}
