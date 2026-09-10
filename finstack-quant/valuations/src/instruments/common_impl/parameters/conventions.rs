//! Market conventions for standard instrument types.
//!
//! Provides enums and associated methods for common market conventions,
//! eliminating the need for multiple instrument-specific constructors.

use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

use serde::{Deserialize, Serialize};

/// Standard bond market conventions by region/issuer.
///
/// Each variant provides market-standard defaults for day count, frequency,
/// settlement days, and other conventions. Use [`BondConvention::settlement_days`]
/// to get the standard settlement lag for each market.
///
/// # Market Standards Reference
///
/// | Convention | Day Count | Frequency | Settlement | Source |
/// |------------|-----------|-----------|------------|--------|
/// | US Treasury | ACT/ACT ICMA | Semi-annual | T+1 | Treasury Direct |
/// | US Agency | 30/360 | Semi-annual | T+1 | SIFMA |
/// | US Corporate | 30/360 | Semi-annual | T+1 | SIFMA (May 2024) |
/// | EUR Corporate | ACT/ACT ICMA | Annual | T+2 | ICMA / TARGET2 |
/// | German Bund | ACT/ACT ICMA | Annual | T+2 | Eurex |
/// | UK Gilt | ACT/ACT ICMA | Semi-annual | T+1 | DMO |
/// | French OAT | ACT/ACT ICMA | Annual | T+2 | AFT |
/// | JGB | ACT/365F | Semi-annual | T+2 | JSCC (cross-border) |
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
    /// Domestic JGB settlement moved to T+1 in May 2018 (JSCC). Cross-border
    /// transactions settle T+2 (BOJ). This variant uses T+2 as the safer
    /// default for international participants.
    ///
    /// Tokyo quotes the bond on **Japanese simple yield**
    /// (`japanese_simple_yield`, 単利). Street [`crate::metrics::MetricId::Ytm`]
    /// remains the secondary-market IRR and is **not** switched for `Jgb`.
    Jgb,
    /// US corporate cash: Semi-annual, 30/360, T+1 settlement (SIFMA May 2024).
    UsCorporate,
    /// EUR corporate: Annual, ACT/ACT ICMA, T+2 settlement, TARGET2.
    EurCorporate,
}

impl BondConvention {
    /// Day count convention for this market.
    ///
    /// # Market Standards
    ///
    /// - **ACT/ACT ICMA**: US Treasury, German Bund, UK Gilt, French OAT
    /// - **30/360**: US Agency, US Corporate
    /// - **ACT/365F**: JGB
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
    ///
    /// # Market Standards
    ///
    /// - **Semi-annual**: US Treasury, US Agency, UK Gilt, US Corporate, JGB
    /// - **Annual**: German Bund, French OAT, EUR Corporate
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

    /// Business day convention for this market.
    ///
    /// # Market Standards
    ///
    /// - **Following**: Government bonds (US Treasury, Bunds, Gilts, OATs, JGBs)
    /// - **Modified Following**: Corporate and Agency bonds (prevents month-end drift)
    pub fn business_day_convention(&self) -> BusinessDayConvention {
        match self {
            // Government bonds use Following
            BondConvention::UsTreasury
            | BondConvention::GermanBund
            | BondConvention::UkGilt
            | BondConvention::FrenchOat
            | BondConvention::Jgb => BusinessDayConvention::Following,
            // Corporate and Agency use Modified Following
            BondConvention::UsAgency
            | BondConvention::UsCorporate
            | BondConvention::EurCorporate => BusinessDayConvention::ModifiedFollowing,
        }
    }

    /// Default stub convention for convenience construction.
    ///
    /// Schedules are generated backward from maturity, so an off-cycle issue
    /// date produces a short first coupon. Callers with explicit long/front/
    /// back terms override this through [`crate::instruments::Bond::with_stub`].
    pub fn stub_convention(&self) -> StubKind {
        StubKind::ShortFront
    }

    /// Settlement days (T+N) for this market.
    ///
    /// # Market Standards
    ///
    /// | Market | Settlement | Source |
    /// |--------|------------|--------|
    /// | US Treasury | T+1 | Treasury Direct |
    /// | US Agency | T+1 | SIFMA |
    /// | US Corporate | T+1 | SIFMA (May 2024) |
    /// | EUR Corporate | T+2 | ICMA / TARGET2 |
    /// | German Bund | T+2 | Eurex |
    /// | UK Gilt | T+1 | DMO |
    /// | French OAT | T+2 | AFT |
    /// | JGB | T+2 | JSCC (cross-border; domestic is T+1 since May 2018) |
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
    ///
    /// Returns `Some(days)` if the market has an ex-coupon convention, `None` otherwise.
    ///
    /// # Market Standards
    ///
    /// - **UK Gilt**: 7 business days before coupon date
    /// - **Other markets**: No ex-coupon convention (ex-date = record date)
    pub fn ex_coupon_days(&self) -> Option<u32> {
        match self {
            BondConvention::UkGilt => Some(7),
            _ => None,
        }
    }

    /// Default discount curve ID for this market.
    pub fn default_disc_curve(&self) -> &'static str {
        match self {
            BondConvention::UsTreasury => "USD-TREASURY",
            BondConvention::UsAgency | BondConvention::UsCorporate => "USD-OIS",
            BondConvention::EurCorporate => "EUR-OIS",
            BondConvention::GermanBund | BondConvention::FrenchOat => "EUR-BUND",
            BondConvention::UkGilt => "GBP-GILT",
            BondConvention::Jgb => "JPY-JGB",
        }
    }

    /// Calendar identifier for this market.
    ///
    /// Returns the standard holiday calendar for business day adjustments.
    pub fn calendar_id(&self) -> Option<&'static str> {
        match self {
            // UST and Agency use the SIFMA bond-market calendar, which includes
            // early closes and holidays specific to the US fixed-income market.
            BondConvention::UsTreasury | BondConvention::UsAgency => Some("sifma"),
            // Corporate bonds use the standard NYC business-day calendar.
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

/// Standard interest rate swap conventions by region.
///
/// # Market Standards Reference (Post-IBOR Transition)
///
/// | Convention | Index | Fixed DC | Float DC | Fixed Freq | Float Freq | Reset Lag |
/// |------------|-------|----------|----------|------------|------------|-----------|
/// | USD OIS | SOFR | 30/360 | ACT/360 | Semi-annual | Annual | T-2 |
/// | EUR OIS | ESTR | 30/360 | ACT/360 | Annual | Annual | T-2 |
/// | EUR IBOR | EURIBOR | 30/360 | ACT/360 | Annual | Semi-annual | T-2 |
/// | GBP OIS | SONIA | ACT/365F | ACT/365F | Annual | Annual | T-0 |
/// | JPY OIS | TONAR | ACT/365F | ACT/365F | Semi-annual | Annual | T-2 |
///
/// # OIS Compounding
///
/// Note: OIS swaps (SOFR, ESTR, SONIA, TONAR) use **daily compounded** rates
/// with observation shift (typically 2 days lookback). The float frequency
/// indicates the payment/reset frequency, not the compounding frequency.
/// See compounding method guidance below for details.
///
/// # Sources
///
/// - ISDA 2021 IBOR Fallbacks Protocol
/// - Bloomberg SWDF function
/// - QuantLib OvernightIndexedSwap conventions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IRSConvention {
    /// USD SOFR OIS: Annual fixed, annual float, ACT/360
    ///
    /// Standard post-LIBOR USD swap convention using SOFR compounded in arrears.
    UsdSofr,
    /// EUR ESTR OIS: Annual fixed, annual float, ACT/360
    ///
    /// Standard EUR OIS convention using ESTR compounded in arrears.
    /// For EURIBOR swaps, use [`EurEuribor`](Self::EurEuribor).
    EurEstr,
    /// EUR EURIBOR: Annual fixed, semi-annual float, ACT/360
    ///
    /// Legacy EUR swap convention using EURIBOR 6M as the floating index.
    /// This is a term rate (not compounded daily).
    EurEuribor,
    /// GBP SONIA OIS: Annual fixed, annual float, ACT/365F
    ///
    /// Standard GBP swap convention using SONIA compounded in arrears.
    GbpSonia,
    /// JPY TONAR OIS: Annual fixed, annual float, ACT/365F
    ///
    /// Standard JPY swap convention using TONAR compounded in arrears.
    JpyTonar,
}

impl IRSConvention {
    pub(crate) fn conventions(
        &self,
    ) -> finstack_quant_core::Result<&'static crate::market::conventions::RateIndexConventions>
    {
        use crate::market::conventions::ConventionRegistry;
        use finstack_quant_core::types::IndexId;
        let id = match self {
            Self::UsdSofr => "USD-SOFR-OIS",
            Self::EurEstr => "EUR-ESTR-OIS",
            Self::EurEuribor => "EUR-EURIBOR-6M",
            Self::GbpSonia => "GBP-SONIA-OIS",
            Self::JpyTonar => "JPY-TONAR-OIS",
        };
        ConventionRegistry::try_global()?.require_rate_index(&IndexId::new(id))
    }

    /// Fixed-leg accrual day count from the canonical rate-index registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry or required convention is invalid.
    pub fn fixed_day_count(&self) -> finstack_quant_core::Result<DayCount> {
        Ok(self.conventions()?.default_fixed_leg_day_count)
    }

    /// Floating-leg accrual day count from the canonical rate-index registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry or required convention is invalid.
    pub fn float_day_count(&self) -> finstack_quant_core::Result<DayCount> {
        Ok(self.conventions()?.day_count)
    }

    /// Fixed-leg payment frequency from the canonical rate-index registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry or required convention is invalid.
    pub fn fixed_frequency(&self) -> finstack_quant_core::Result<Tenor> {
        Ok(self.conventions()?.default_fixed_leg_frequency)
    }

    /// Floating-leg payment frequency, distinct from daily OIS compounding from the canonical rate-index registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry or required convention is invalid.
    pub fn float_frequency(&self) -> finstack_quant_core::Result<Tenor> {
        Ok(self.conventions()?.default_payment_frequency)
    }

    /// Business days between accrual end and payment from the canonical rate-index registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry or required convention is invalid.
    pub fn payment_lag_days(&self) -> finstack_quant_core::Result<i32> {
        Ok(self.conventions()?.default_payment_lag_days)
    }

    /// Business days between the index fixing and accrual start from the canonical rate-index registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry or required convention is invalid.
    pub fn reset_lag_days(&self) -> finstack_quant_core::Result<i32> {
        Ok(self.conventions()?.default_reset_lag_days)
    }

    /// Business-day adjustment for the swap schedule from the canonical rate-index registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry or required convention is invalid.
    pub fn business_day_convention(&self) -> finstack_quant_core::Result<BusinessDayConvention> {
        Ok(self.conventions()?.market_business_day_convention)
    }

    /// Whether the canonical index uses compounded overnight rates.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded rate-index convention cannot be resolved.
    pub fn uses_daily_compounding(&self) -> finstack_quant_core::Result<bool> {
        Ok(self.conventions()?.ois_compounding.is_some())
    }

    /// Contractual observation lookback or shift in business days.
    /// Standard OIS presets use zero; payment delay is a separate convention.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded rate-index convention cannot be resolved.
    pub fn observation_shift_days(&self) -> finstack_quant_core::Result<i32> {
        use crate::instruments::rates::irs::FloatingLegCompounding;
        Ok(match self.conventions()?.ois_compounding {
            Some(FloatingLegCompounding::CompoundedInArrears { lookback_days }) => lookback_days,
            Some(FloatingLegCompounding::CompoundedWithObservationShift { shift_days }) => {
                shift_days
            }
            _ => 0,
        })
    }

    /// Calendar identifier from the canonical rate-index convention.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded rate-index convention cannot be resolved.
    pub fn calendar_id(&self) -> finstack_quant_core::Result<&'static str> {
        Ok(&self.conventions()?.market_calendar_id)
    }

    /// Conventional OIS discount-curve identifier for this currency.
    pub fn disc_curve_id(&self) -> &'static str {
        match self {
            Self::UsdSofr => "USD-SOFR",
            Self::EurEstr | Self::EurEuribor => "EUR-ESTR",
            Self::GbpSonia => "GBP-SONIA",
            Self::JpyTonar => "JPY-TONAR",
        }
    }

    /// Conventional projection-curve identifier for this index.
    pub fn forward_curve_id(&self) -> &'static str {
        match self {
            Self::UsdSofr => "USD-SOFR",
            Self::EurEstr => "EUR-ESTR",
            Self::EurEuribor => "EUR-EURIBOR-6M",
            Self::GbpSonia => "GBP-SONIA",
            Self::JpyTonar => "JPY-TONAR",
        }
    }
}

impl std::fmt::Display for IRSConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IRSConvention::UsdSofr => write!(f, "usd_sofr"),
            IRSConvention::EurEstr => write!(f, "eur_estr"),
            IRSConvention::EurEuribor => write!(f, "eur_euribor"),
            IRSConvention::GbpSonia => write!(f, "gbp_sonia"),
            IRSConvention::JpyTonar => write!(f, "jpy_tonar"),
        }
    }
}

impl std::str::FromStr for IRSConvention {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "usd_sofr" => Ok(IRSConvention::UsdSofr),
            "eur_estr" => Ok(IRSConvention::EurEstr),
            "eur_euribor" => Ok(IRSConvention::EurEuribor),
            "gbp_sonia" => Ok(IRSConvention::GbpSonia),
            "jpy_tonar" => Ok(IRSConvention::JpyTonar),
            _ => Err(format!("Unknown IRS convention: {s}")),
        }
    }
}

/// Standard commodity market conventions by product type.
///
/// Each variant provides market-standard defaults for settlement days,
/// business day convention, and calendar. Use when constructing commodity
/// forwards, options, or swaps without explicitly specifying these parameters.
///
/// # Market Standards Reference
///
/// | Convention | Settlement | BDC | Calendar | Exchange |
/// |------------|------------|-----|----------|----------|
/// | WTI Crude | T+2 | Following | NYMEX | NYMEX/CME |
/// | Brent Crude | T+2 | Following | ICE | ICE |
/// | Natural Gas | T+2 | Following | NYMEX | NYMEX/CME |
/// | Gold | T+2 | Modified Following | COMEX | CME |
/// | Silver | T+2 | Modified Following | COMEX | CME |
/// | Copper | T+2 | Following | LME | LME |
/// | Corn/Wheat | T+2 | Following | CBOT | CME |
/// | Power | T+1 | Modified Following | NERC | Various |
///
/// # Sources
///
/// - CME Group rulebooks
/// - ICE Futures exchange rules
/// - LME trading procedures
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
    ///
    /// # Market Standards
    ///
    /// | Market | Settlement | Source |
    /// |--------|------------|--------|
    /// | WTI/Brent/NG | T+2 | CME/ICE rulebooks |
    /// | Gold/Silver | T+2 | COMEX |
    /// | Copper (LME) | T+2 | LME |
    /// | Agricultural | T+2 | CBOT |
    /// | Power | T+1 | NERC/ISO |
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
    ///
    /// # Market Standards
    ///
    /// - **Following**: Energy (WTI, Brent, NG), Base metals, Agricultural
    /// - **Modified Following**: Precious metals (Gold, Silver), Power
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

    /// Calendar identifier for this commodity market.
    ///
    /// Returns the standard exchange calendar for business day adjustments.
    pub fn calendar_id(&self) -> &'static str {
        match self {
            CommodityConvention::WtiCrude | CommodityConvention::NaturalGas => "nymex",
            CommodityConvention::BrentCrude => "ice",
            CommodityConvention::Gold | CommodityConvention::Silver => "comex",
            CommodityConvention::Copper => "lme",
            CommodityConvention::Agricultural => "cbot",
            CommodityConvention::Power => "nerc",
        }
    }

    /// Primary currency for this commodity.
    pub fn currency(&self) -> finstack_quant_core::currency::Currency {
        use finstack_quant_core::currency::Currency;
        match self {
            CommodityConvention::WtiCrude
            | CommodityConvention::NaturalGas
            | CommodityConvention::Gold
            | CommodityConvention::Silver
            | CommodityConvention::Agricultural
            | CommodityConvention::Power
            | CommodityConvention::BrentCrude
            | CommodityConvention::Copper => Currency::USD,
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
    use finstack_quant_core::dates::Tenor;

    // IRS Convention Tests

    #[test]
    fn irs_ois_float_frequency_annual() {
        // OIS swaps pay annually with daily compounding
        assert_eq!(
            IRSConvention::UsdSofr.float_frequency().expect("registry"),
            Tenor::annual()
        );
        assert_eq!(
            IRSConvention::EurEstr.float_frequency().expect("registry"),
            Tenor::annual()
        );
        assert_eq!(
            IRSConvention::GbpSonia.float_frequency().expect("registry"),
            Tenor::annual()
        );
        assert_eq!(
            IRSConvention::JpyTonar.float_frequency().expect("registry"),
            Tenor::annual()
        );
    }

    #[test]
    fn irs_ibor_float_frequency_matches_index() {
        // EURIBOR 6M swaps pay semi-annually
        assert_eq!(
            IRSConvention::EurEuribor
                .float_frequency()
                .expect("registry"),
            Tenor::semi_annual()
        );
    }

    #[test]
    fn irs_compounding_method() {
        // OIS swaps use daily compounding
        assert!(IRSConvention::UsdSofr
            .uses_daily_compounding()
            .expect("registry"));
        assert!(IRSConvention::EurEstr
            .uses_daily_compounding()
            .expect("registry"));
        assert!(IRSConvention::GbpSonia
            .uses_daily_compounding()
            .expect("registry"));
        assert!(IRSConvention::JpyTonar
            .uses_daily_compounding()
            .expect("registry"));

        // IBOR swaps use simple rates
        assert!(!IRSConvention::EurEuribor
            .uses_daily_compounding()
            .expect("registry"));
    }

    #[test]
    fn irs_observation_shift() {
        // Standard 2-day lookback for most OIS
        assert_eq!(
            IRSConvention::UsdSofr
                .observation_shift_days()
                .expect("registry"),
            0
        );
        assert_eq!(
            IRSConvention::EurEstr
                .observation_shift_days()
                .expect("registry"),
            0
        );
        assert_eq!(
            IRSConvention::JpyTonar
                .observation_shift_days()
                .expect("registry"),
            0
        );

        // SONIA uses 0-day shift with payment delay
        assert_eq!(
            IRSConvention::GbpSonia
                .observation_shift_days()
                .expect("registry"),
            0
        );

        // IBOR has no observation shift
        assert_eq!(
            IRSConvention::EurEuribor
                .observation_shift_days()
                .expect("registry"),
            0
        );
    }

    #[test]
    fn irs_forward_curve_id() {
        // OIS swaps: forward = discount
        assert_eq!(IRSConvention::UsdSofr.forward_curve_id(), "USD-SOFR");
        assert_eq!(IRSConvention::EurEstr.forward_curve_id(), "EUR-ESTR");
        assert_eq!(IRSConvention::GbpSonia.forward_curve_id(), "GBP-SONIA");
        assert_eq!(IRSConvention::JpyTonar.forward_curve_id(), "JPY-TONAR");

        // IBOR swaps: forward != discount
        assert_eq!(
            IRSConvention::EurEuribor.forward_curve_id(),
            "EUR-EURIBOR-6M"
        );
        assert_eq!(IRSConvention::EurEuribor.disc_curve_id(), "EUR-ESTR");
    }

    #[test]
    fn irs_from_str() {
        // Standard names
        assert_eq!(
            "usd_sofr".parse::<IRSConvention>().unwrap(),
            IRSConvention::UsdSofr
        );
        assert_eq!(
            "eur_estr".parse::<IRSConvention>().unwrap(),
            IRSConvention::EurEstr
        );
        assert_eq!(
            "eur_euribor".parse::<IRSConvention>().unwrap(),
            IRSConvention::EurEuribor
        );
        assert_eq!(
            "gbp_sonia".parse::<IRSConvention>().unwrap(),
            IRSConvention::GbpSonia
        );
        assert_eq!(
            "jpy_tonar".parse::<IRSConvention>().unwrap(),
            IRSConvention::JpyTonar
        );

        for retired in ["sofr", "estr", "euribor", "sonia", "tona", "USD_SOFR"] {
            assert!(retired.parse::<IRSConvention>().is_err());
        }
    }

    #[test]
    fn irs_display() {
        assert_eq!(format!("{}", IRSConvention::UsdSofr), "usd_sofr");
        assert_eq!(format!("{}", IRSConvention::EurEstr), "eur_estr");
        assert_eq!(format!("{}", IRSConvention::EurEuribor), "eur_euribor");
        assert_eq!(format!("{}", IRSConvention::GbpSonia), "gbp_sonia");
        assert_eq!(format!("{}", IRSConvention::JpyTonar), "jpy_tonar");
    }

    // Bond Convention Tests

    // BondConvention tests
    #[test]
    fn bond_convention_day_counts() {
        // ACT/ACT ICMA for government bonds
        assert_eq!(BondConvention::UsTreasury.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::GermanBund.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::UkGilt.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::FrenchOat.day_count(), DayCount::ActActIsma);

        // 30/360 for US agency and corporate
        assert_eq!(BondConvention::UsAgency.day_count(), DayCount::Thirty360);
        assert_eq!(BondConvention::UsCorporate.day_count(), DayCount::Thirty360);
        assert_eq!(
            BondConvention::EurCorporate.day_count(),
            DayCount::ActActIsma
        );

        // ACT/365F for JGB
        assert_eq!(BondConvention::Jgb.day_count(), DayCount::Act365F);
    }

    #[test]
    fn bond_convention_frequencies() {
        // Semi-annual
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
        // T+1 markets
        assert_eq!(BondConvention::UsTreasury.settlement_days(), 1);
        assert_eq!(BondConvention::UsAgency.settlement_days(), 1);
        assert_eq!(BondConvention::UkGilt.settlement_days(), 1);

        // T+2 markets
        assert_eq!(BondConvention::UsCorporate.settlement_days(), 1);
        assert_eq!(BondConvention::EurCorporate.settlement_days(), 2);
        assert_eq!(BondConvention::GermanBund.settlement_days(), 2);
        assert_eq!(BondConvention::FrenchOat.settlement_days(), 2);

        // T+2 markets (JGB cross-border since May 2018)
        assert_eq!(BondConvention::Jgb.settlement_days(), 2);
    }

    #[test]
    fn bond_convention_ex_coupon() {
        // UK Gilt has 7-day ex-coupon
        assert_eq!(BondConvention::UkGilt.ex_coupon_days(), Some(7));

        // Others have no ex-coupon convention
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
        // Standard names
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

    // Commodity Convention Tests

    #[test]
    fn commodity_convention_settlement_days() {
        // Most commodities: T+2
        assert_eq!(CommodityConvention::WtiCrude.settlement_days(), 2);
        assert_eq!(CommodityConvention::BrentCrude.settlement_days(), 2);
        assert_eq!(CommodityConvention::NaturalGas.settlement_days(), 2);
        assert_eq!(CommodityConvention::Gold.settlement_days(), 2);
        assert_eq!(CommodityConvention::Silver.settlement_days(), 2);
        assert_eq!(CommodityConvention::Copper.settlement_days(), 2);
        assert_eq!(CommodityConvention::Agricultural.settlement_days(), 2);

        // Power: T+1
        assert_eq!(CommodityConvention::Power.settlement_days(), 1);
    }

    #[test]
    fn commodity_convention_business_day() {
        use finstack_quant_core::dates::BusinessDayConvention;

        // Energy and base metals: Following
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

        // Precious metals and power: Modified Following
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
    fn commodity_convention_calendar_ids() {
        assert_eq!(CommodityConvention::WtiCrude.calendar_id(), "nymex");
        assert_eq!(CommodityConvention::NaturalGas.calendar_id(), "nymex");
        assert_eq!(CommodityConvention::BrentCrude.calendar_id(), "ice");
        assert_eq!(CommodityConvention::Gold.calendar_id(), "comex");
        assert_eq!(CommodityConvention::Silver.calendar_id(), "comex");
        assert_eq!(CommodityConvention::Copper.calendar_id(), "lme");
        assert_eq!(CommodityConvention::Agricultural.calendar_id(), "cbot");
        assert_eq!(CommodityConvention::Power.calendar_id(), "nerc");
    }

    #[test]
    fn commodity_convention_from_str() {
        // Standard names
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
