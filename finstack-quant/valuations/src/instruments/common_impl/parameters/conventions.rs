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
/// | US Corporate | 30/360 | Semi-annual | T+2 | SIFMA |
/// | German Bund | ACT/ACT ICMA | Annual | T+2 | Eurex |
/// | UK Gilt | ACT/ACT ICMA | Semi-annual | T+1 | DMO |
/// | French OAT | ACT/ACT ICMA | Annual | T+2 | AFT |
/// | JGB | ACT/365F | Semi-annual | T+2 | JSCC (cross-border) |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BondConvention {
    /// US Treasury: Semi-annual, ACT/ACT ICMA, T+1 settlement
    USTreasury,
    /// US Agency (FNMA, FHLMC, FHLB): Semi-annual, 30/360, T+1 settlement
    USAgency,
    /// German Bund: Annual, ACT/ACT ICMA, T+2 settlement
    GermanBund,
    /// UK Gilt: Semi-annual, ACT/ACT ICMA, T+1 settlement, 7-day ex-coupon
    UKGilt,
    /// French OAT: Annual, ACT/ACT ICMA, T+2 settlement
    FrenchOAT,
    /// Japanese Government Bond (cross-border): Semi-annual, ACT/365F, T+2.
    ///
    /// Domestic JGB settlement moved to T+1 in May 2018 (JSCC). Cross-border
    /// transactions settle T+2 (BOJ). This variant uses T+2 as the safer
    /// default for international participants.
    JGB,
    /// Standard US corporate: Semi-annual, 30/360, T+2 settlement
    Corporate,
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
            BondConvention::USTreasury
            | BondConvention::GermanBund
            | BondConvention::UKGilt
            | BondConvention::FrenchOAT => DayCount::ActActIsma,
            BondConvention::USAgency | BondConvention::Corporate => DayCount::Thirty360,
            BondConvention::JGB => DayCount::Act365F,
        }
    }

    /// Payment frequency for this market.
    ///
    /// # Market Standards
    ///
    /// - **Semi-annual**: US Treasury, US Agency, UK Gilt, Corporate, JGB
    /// - **Annual**: German Bund, French OAT
    pub fn frequency(&self) -> Tenor {
        match self {
            BondConvention::USTreasury
            | BondConvention::USAgency
            | BondConvention::UKGilt
            | BondConvention::Corporate
            | BondConvention::JGB => Tenor::semi_annual(),
            BondConvention::GermanBund | BondConvention::FrenchOAT => Tenor::annual(),
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
            BondConvention::USTreasury
            | BondConvention::GermanBund
            | BondConvention::UKGilt
            | BondConvention::FrenchOAT
            | BondConvention::JGB => BusinessDayConvention::Following,
            // Corporate and Agency use Modified Following
            BondConvention::USAgency | BondConvention::Corporate => {
                BusinessDayConvention::ModifiedFollowing
            }
        }
    }

    /// Stub convention for this market.
    ///
    /// Default is no stub (full first coupon period).
    pub fn stub_convention(&self) -> StubKind {
        StubKind::None
    }

    /// Settlement days (T+N) for this market.
    ///
    /// # Market Standards
    ///
    /// | Market | Settlement | Source |
    /// |--------|------------|--------|
    /// | US Treasury | T+1 | Treasury Direct |
    /// | US Agency | T+1 | SIFMA |
    /// | US Corporate | T+2 | SIFMA |
    /// | German Bund | T+2 | Eurex |
    /// | UK Gilt | T+1 | DMO |
    /// | French OAT | T+2 | AFT |
    /// | JGB | T+2 | JSCC (cross-border; domestic is T+1 since May 2018) |
    pub fn settlement_days(&self) -> u32 {
        match self {
            BondConvention::USTreasury | BondConvention::USAgency | BondConvention::UKGilt => 1,
            BondConvention::Corporate
            | BondConvention::GermanBund
            | BondConvention::FrenchOAT
            | BondConvention::JGB => 2,
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
            BondConvention::UKGilt => Some(7),
            _ => None,
        }
    }

    /// Default discount curve ID for this market.
    pub fn default_disc_curve(&self) -> &'static str {
        match self {
            BondConvention::USTreasury => "USD-TREASURY",
            BondConvention::USAgency | BondConvention::Corporate => "USD-OIS",
            BondConvention::GermanBund | BondConvention::FrenchOAT => "EUR-BUND",
            BondConvention::UKGilt => "GBP-GILT",
            BondConvention::JGB => "JPY-JGB",
        }
    }

    /// Calendar identifier for this market.
    ///
    /// Returns the standard holiday calendar for business day adjustments.
    pub fn calendar_id(&self) -> Option<&'static str> {
        match self {
            // UST and Agency use the SIFMA bond-market calendar, which includes
            // early closes and holidays specific to the US fixed-income market.
            BondConvention::USTreasury | BondConvention::USAgency => Some("sifma"),
            // Corporate bonds use the standard NYC business-day calendar.
            BondConvention::Corporate => Some("usny"),
            BondConvention::GermanBund | BondConvention::FrenchOAT => Some("target2"),
            BondConvention::UKGilt => Some("gblo"),
            BondConvention::JGB => Some("jpto"),
        }
    }
}

impl std::fmt::Display for BondConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BondConvention::USTreasury => write!(f, "us_treasury"),
            BondConvention::USAgency => write!(f, "us_agency"),
            BondConvention::GermanBund => write!(f, "german_bund"),
            BondConvention::UKGilt => write!(f, "uk_gilt"),
            BondConvention::FrenchOAT => write!(f, "french_oat"),
            BondConvention::JGB => write!(f, "jgb"),
            BondConvention::Corporate => write!(f, "corporate"),
        }
    }
}

impl std::str::FromStr for BondConvention {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "us_treasury" | "ust" | "treasury" => Ok(BondConvention::USTreasury),
            "us_agency" | "agency" | "fnma" | "fhlmc" | "fhlb" => Ok(BondConvention::USAgency),
            "german_bund" | "bund" => Ok(BondConvention::GermanBund),
            "uk_gilt" | "gilt" => Ok(BondConvention::UKGilt),
            "french_oat" | "oat" => Ok(BondConvention::FrenchOAT),
            "jgb" | "japanese" | "japan" => Ok(BondConvention::JGB),
            "corporate" | "corp" => Ok(BondConvention::Corporate),
            other => Err(format!("Unknown bond convention: {}", other)),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IRSConvention {
    /// USD SOFR OIS: Semi-annual fixed, annual float, ACT/360
    ///
    /// Standard post-LIBOR USD swap convention using SOFR compounded in arrears.
    USDStandard,
    /// EUR ESTR OIS: Annual fixed, annual float, ACT/360
    ///
    /// Standard EUR OIS convention using ESTR compounded in arrears.
    /// For legacy EURIBOR swaps, use [`EURIborStandard`](Self::EURIborStandard).
    EURStandard,
    /// EUR EURIBOR: Annual fixed, semi-annual float, ACT/360
    ///
    /// Legacy EUR swap convention using EURIBOR 6M as the floating index.
    /// This is a term rate (not compounded daily).
    EURIborStandard,
    /// GBP SONIA OIS: Annual fixed, annual float, ACT/365F
    ///
    /// Standard GBP swap convention using SONIA compounded in arrears.
    GBPStandard,
    /// JPY TONAR OIS: Semi-annual fixed, annual float, ACT/365F
    ///
    /// Standard JPY swap convention using TONAR compounded in arrears.
    JPYStandard,
}

impl IRSConvention {
    /// Fixed leg day count for this convention.
    ///
    /// # Market Standards
    ///
    /// - **30/360**: USD, EUR (both OIS and IBOR)
    /// - **ACT/365F**: GBP, JPY
    pub fn fixed_day_count(&self) -> DayCount {
        match self {
            IRSConvention::USDStandard
            | IRSConvention::EURStandard
            | IRSConvention::EURIborStandard => DayCount::Thirty360,
            IRSConvention::GBPStandard | IRSConvention::JPYStandard => DayCount::Act365F,
        }
    }

    /// Float leg day count for this convention.
    ///
    /// # Market Standards
    ///
    /// - **ACT/360**: USD SOFR, EUR ESTR, EUR EURIBOR
    /// - **ACT/365F**: GBP SONIA, JPY TONAR
    pub fn float_day_count(&self) -> DayCount {
        match self {
            IRSConvention::USDStandard
            | IRSConvention::EURStandard
            | IRSConvention::EURIborStandard => DayCount::Act360,
            IRSConvention::GBPStandard | IRSConvention::JPYStandard => DayCount::Act365F,
        }
    }

    /// Fixed leg frequency for this convention.
    ///
    /// # Market Standards
    ///
    /// - **Semi-annual**: USD, JPY
    /// - **Annual**: EUR, GBP
    pub fn fixed_frequency(&self) -> Tenor {
        match self {
            IRSConvention::USDStandard | IRSConvention::JPYStandard => Tenor::semi_annual(),
            IRSConvention::EURStandard
            | IRSConvention::EURIborStandard
            | IRSConvention::GBPStandard => Tenor::annual(),
        }
    }

    /// Float leg frequency (payment/reset frequency) for this convention.
    ///
    /// # Market Standards
    ///
    /// - **Annual**: EUR ESTR OIS, GBP SONIA, JPY TONAR, USD SOFR (for OIS payment)
    /// - **Semi-annual**: EUR EURIBOR 6M
    ///
    /// # Note on OIS Compounding
    ///
    /// For OIS swaps, this is the **payment frequency**, not the compounding frequency.
    /// OIS rates are compounded daily. Use `uses_daily_compounding()`
    /// to determine whether daily compounding applies.
    pub fn float_frequency(&self) -> Tenor {
        match self {
            // OIS swaps: annual payment with daily compounding
            IRSConvention::USDStandard
            | IRSConvention::EURStandard
            | IRSConvention::GBPStandard
            | IRSConvention::JPYStandard => Tenor::annual(),
            // IBOR swaps: frequency matches index tenor
            IRSConvention::EURIborStandard => Tenor::semi_annual(), // EURIBOR 6M
        }
    }

    /// Returns the compounding method for the floating leg.
    ///
    /// # Market Standards
    ///
    /// - **OIS swaps** (SOFR, ESTR, SONIA, TONAR): Daily compounding in arrears
    ///   with observation shift (lookback)
    /// - **IBOR swaps** (EURIBOR): Simple (no compounding within period)
    ///
    /// # Returns
    ///
    /// `true` if the swap uses daily compounded rates (OIS),
    /// `false` if it uses simple term rates (IBOR).
    pub fn uses_daily_compounding(&self) -> bool {
        match self {
            IRSConvention::USDStandard
            | IRSConvention::EURStandard
            | IRSConvention::GBPStandard
            | IRSConvention::JPYStandard => true, // OIS
            IRSConvention::EURIborStandard => false, // Term rate
        }
    }

    /// Observation shift (lookback) in business days for OIS swaps.
    ///
    /// For OIS swaps, rates are typically observed with a lookback to allow
    /// payment calculation before the payment date.
    ///
    /// # Market Standards
    ///
    /// - **2 days**: USD SOFR, EUR ESTR, JPY TONAR
    /// - **0 days**: GBP SONIA (payment delay instead)
    /// - **N/A**: IBOR swaps (not compounded)
    ///
    /// # Returns
    ///
    /// Number of business days for observation shift, or 0 for non-OIS swaps.
    pub fn observation_shift_days(&self) -> i32 {
        match self {
            IRSConvention::USDStandard
            | IRSConvention::EURStandard
            | IRSConvention::JPYStandard => 2,
            IRSConvention::GBPStandard | IRSConvention::EURIborStandard => 0, // Uses payment delay instead or is not applicable
        }
    }

    /// Payment delay in business days for this convention.
    ///
    /// # Market Standards
    ///
    /// - **2 days**: Most OIS swaps (USD, EUR, JPY)
    /// - **0 days**: GBP SONIA (uses same-day payment)
    /// - **2 days**: EUR EURIBOR
    pub fn payment_lag_days(&self) -> i32 {
        match self {
            IRSConvention::USDStandard
            | IRSConvention::EURStandard
            | IRSConvention::EURIborStandard
            | IRSConvention::JPYStandard => 2,
            IRSConvention::GBPStandard => 0,
        }
    }

    /// Business day convention for this convention.
    ///
    /// All standard IRS conventions use Modified Following.
    pub fn business_day_convention(&self) -> BusinessDayConvention {
        BusinessDayConvention::ModifiedFollowing
    }

    /// Calendar identifier for this convention.
    pub fn calendar_id(&self) -> Option<String> {
        match self {
            IRSConvention::USDStandard => Some("usny".to_string()),
            IRSConvention::EURStandard | IRSConvention::EURIborStandard => {
                Some("target2".to_string())
            }
            IRSConvention::GBPStandard => Some("gblo".to_string()),
            IRSConvention::JPYStandard => Some("jpto".to_string()),
        }
    }

    /// Discount curve ID for this convention.
    ///
    /// Returns the OIS curve for discounting (post-crisis standard).
    pub fn disc_curve_id(&self) -> &'static str {
        match self {
            IRSConvention::USDStandard => "USD-SOFR",
            IRSConvention::EURStandard | IRSConvention::EURIborStandard => "EUR-ESTR",
            IRSConvention::GBPStandard => "GBP-SONIA",
            IRSConvention::JPYStandard => "JPY-TONAR",
        }
    }

    /// Forward/projection curve ID for this convention.
    ///
    /// For OIS swaps, this is the same as the discount curve.
    /// For IBOR swaps, this is the IBOR curve.
    pub fn forward_curve_id(&self) -> &'static str {
        match self {
            IRSConvention::USDStandard => "USD-SOFR",
            IRSConvention::EURStandard => "EUR-ESTR",
            IRSConvention::EURIborStandard => "EUR-EURIBOR-6M",
            IRSConvention::GBPStandard => "GBP-SONIA",
            IRSConvention::JPYStandard => "JPY-TONAR",
        }
    }

    /// Reset lag in business days for this convention.
    ///
    /// For OIS swaps, this is the fixing offset before the accrual period.
    /// For IBOR swaps, this is the fixing lag before period start.
    ///
    /// # Market Standards
    ///
    /// - **2 days (T-2)**: USD, EUR, JPY
    /// - **0 days (T-0)**: GBP SONIA
    pub fn reset_lag_days(&self) -> i32 {
        match self {
            IRSConvention::USDStandard
            | IRSConvention::EURStandard
            | IRSConvention::EURIborStandard
            | IRSConvention::JPYStandard => 2,
            IRSConvention::GBPStandard => 0, // Same-day fixing
        }
    }
}

impl std::fmt::Display for IRSConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IRSConvention::USDStandard => write!(f, "usd_sofr"),
            IRSConvention::EURStandard => write!(f, "eur_estr"),
            IRSConvention::EURIborStandard => write!(f, "eur_euribor"),
            IRSConvention::GBPStandard => write!(f, "gbp_sonia"),
            IRSConvention::JPYStandard => write!(f, "jpy_tonar"),
        }
    }
}

impl std::str::FromStr for IRSConvention {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            // USD SOFR
            "usd_standard" | "usd_sofr" | "usd" | "sofr" => Ok(IRSConvention::USDStandard),
            // EUR ESTR (OIS)
            "eur_standard" | "eur_estr" | "eur_ois" | "estr" => Ok(IRSConvention::EURStandard),
            // EUR EURIBOR (IBOR)
            "eur_ibor_standard" | "eur_euribor" | "euribor" => Ok(IRSConvention::EURIborStandard),
            // GBP SONIA
            "gbp_standard" | "gbp_sonia" | "gbp" | "sonia" => Ok(IRSConvention::GBPStandard),
            // JPY TONAR
            "jpy_standard" | "jpy_tonar" | "jpy" | "tonar" | "tona" => {
                Ok(IRSConvention::JPYStandard)
            }
            other => Err(format!("Unknown IRS convention: {}", other)),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CommodityConvention {
    /// WTI Crude Oil: T+2, Following, NYMEX calendar
    WTICrude,
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
            CommodityConvention::WTICrude
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
            CommodityConvention::WTICrude
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
            CommodityConvention::WTICrude | CommodityConvention::NaturalGas => "nymex",
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
            CommodityConvention::WTICrude
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
            CommodityConvention::WTICrude => write!(f, "wti_crude"),
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
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "wti_crude" | "wti" | "cl" | "crude" => Ok(CommodityConvention::WTICrude),
            "brent_crude" | "brent" | "co" | "ice_brent" => Ok(CommodityConvention::BrentCrude),
            "natural_gas" | "ng" | "henry_hub" | "nat_gas" => Ok(CommodityConvention::NaturalGas),
            "gold" | "gc" | "xau" => Ok(CommodityConvention::Gold),
            "silver" | "si" | "xag" => Ok(CommodityConvention::Silver),
            "copper" | "hg" | "lme_copper" => Ok(CommodityConvention::Copper),
            "agricultural" | "agri" | "corn" | "wheat" | "soybeans" => {
                Ok(CommodityConvention::Agricultural)
            }
            "power" | "electricity" | "elec" => Ok(CommodityConvention::Power),
            other => Err(format!("Unknown commodity convention: {}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::Tenor;

    // =======================================================================
    // IRS Convention Tests
    // =======================================================================

    #[test]
    fn irs_ois_float_frequency_annual() {
        // OIS swaps pay annually with daily compounding
        assert_eq!(
            IRSConvention::USDStandard.float_frequency(),
            Tenor::annual()
        );
        assert_eq!(
            IRSConvention::EURStandard.float_frequency(),
            Tenor::annual()
        );
        assert_eq!(
            IRSConvention::GBPStandard.float_frequency(),
            Tenor::annual()
        );
        assert_eq!(
            IRSConvention::JPYStandard.float_frequency(),
            Tenor::annual()
        );
    }

    #[test]
    fn irs_ibor_float_frequency_matches_index() {
        // EURIBOR 6M swaps pay semi-annually
        assert_eq!(
            IRSConvention::EURIborStandard.float_frequency(),
            Tenor::semi_annual()
        );
    }

    #[test]
    fn irs_compounding_method() {
        // OIS swaps use daily compounding
        assert!(IRSConvention::USDStandard.uses_daily_compounding());
        assert!(IRSConvention::EURStandard.uses_daily_compounding());
        assert!(IRSConvention::GBPStandard.uses_daily_compounding());
        assert!(IRSConvention::JPYStandard.uses_daily_compounding());

        // IBOR swaps use simple rates
        assert!(!IRSConvention::EURIborStandard.uses_daily_compounding());
    }

    #[test]
    fn irs_observation_shift() {
        // Standard 2-day lookback for most OIS
        assert_eq!(IRSConvention::USDStandard.observation_shift_days(), 2);
        assert_eq!(IRSConvention::EURStandard.observation_shift_days(), 2);
        assert_eq!(IRSConvention::JPYStandard.observation_shift_days(), 2);

        // SONIA uses 0-day shift with payment delay
        assert_eq!(IRSConvention::GBPStandard.observation_shift_days(), 0);

        // IBOR has no observation shift
        assert_eq!(IRSConvention::EURIborStandard.observation_shift_days(), 0);
    }

    #[test]
    fn irs_forward_curve_id() {
        // OIS swaps: forward = discount
        assert_eq!(IRSConvention::USDStandard.forward_curve_id(), "USD-SOFR");
        assert_eq!(IRSConvention::EURStandard.forward_curve_id(), "EUR-ESTR");
        assert_eq!(IRSConvention::GBPStandard.forward_curve_id(), "GBP-SONIA");
        assert_eq!(IRSConvention::JPYStandard.forward_curve_id(), "JPY-TONAR");

        // IBOR swaps: forward != discount
        assert_eq!(
            IRSConvention::EURIborStandard.forward_curve_id(),
            "EUR-EURIBOR-6M"
        );
        assert_eq!(IRSConvention::EURIborStandard.disc_curve_id(), "EUR-ESTR");
    }

    #[test]
    fn irs_from_str() {
        // Standard names
        assert_eq!(
            "usd_sofr".parse::<IRSConvention>().unwrap(),
            IRSConvention::USDStandard
        );
        assert_eq!(
            "eur_estr".parse::<IRSConvention>().unwrap(),
            IRSConvention::EURStandard
        );
        assert_eq!(
            "eur_euribor".parse::<IRSConvention>().unwrap(),
            IRSConvention::EURIborStandard
        );
        assert_eq!(
            "gbp_sonia".parse::<IRSConvention>().unwrap(),
            IRSConvention::GBPStandard
        );
        assert_eq!(
            "jpy_tonar".parse::<IRSConvention>().unwrap(),
            IRSConvention::JPYStandard
        );

        // Aliases
        assert_eq!(
            "sofr".parse::<IRSConvention>().unwrap(),
            IRSConvention::USDStandard
        );
        assert_eq!(
            "estr".parse::<IRSConvention>().unwrap(),
            IRSConvention::EURStandard
        );
        assert_eq!(
            "euribor".parse::<IRSConvention>().unwrap(),
            IRSConvention::EURIborStandard
        );
        assert_eq!(
            "sonia".parse::<IRSConvention>().unwrap(),
            IRSConvention::GBPStandard
        );
        assert_eq!(
            "tona".parse::<IRSConvention>().unwrap(),
            IRSConvention::JPYStandard
        );
    }

    #[test]
    fn irs_display() {
        assert_eq!(format!("{}", IRSConvention::USDStandard), "usd_sofr");
        assert_eq!(format!("{}", IRSConvention::EURStandard), "eur_estr");
        assert_eq!(format!("{}", IRSConvention::EURIborStandard), "eur_euribor");
        assert_eq!(format!("{}", IRSConvention::GBPStandard), "gbp_sonia");
        assert_eq!(format!("{}", IRSConvention::JPYStandard), "jpy_tonar");
    }

    // =======================================================================
    // Bond Convention Tests
    // =======================================================================

    // BondConvention tests
    #[test]
    fn bond_convention_day_counts() {
        // ACT/ACT ICMA for government bonds
        assert_eq!(BondConvention::USTreasury.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::GermanBund.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::UKGilt.day_count(), DayCount::ActActIsma);
        assert_eq!(BondConvention::FrenchOAT.day_count(), DayCount::ActActIsma);

        // 30/360 for US agency and corporate
        assert_eq!(BondConvention::USAgency.day_count(), DayCount::Thirty360);
        assert_eq!(BondConvention::Corporate.day_count(), DayCount::Thirty360);

        // ACT/365F for JGB
        assert_eq!(BondConvention::JGB.day_count(), DayCount::Act365F);
    }

    #[test]
    fn bond_convention_frequencies() {
        // Semi-annual
        assert_eq!(BondConvention::USTreasury.frequency(), Tenor::semi_annual());
        assert_eq!(BondConvention::USAgency.frequency(), Tenor::semi_annual());
        assert_eq!(BondConvention::UKGilt.frequency(), Tenor::semi_annual());
        assert_eq!(BondConvention::Corporate.frequency(), Tenor::semi_annual());
        assert_eq!(BondConvention::JGB.frequency(), Tenor::semi_annual());

        // Annual
        assert_eq!(BondConvention::GermanBund.frequency(), Tenor::annual());
        assert_eq!(BondConvention::FrenchOAT.frequency(), Tenor::annual());
    }

    #[test]
    fn bond_convention_settlement_days() {
        // T+1 markets
        assert_eq!(BondConvention::USTreasury.settlement_days(), 1);
        assert_eq!(BondConvention::USAgency.settlement_days(), 1);
        assert_eq!(BondConvention::UKGilt.settlement_days(), 1);

        // T+2 markets
        assert_eq!(BondConvention::Corporate.settlement_days(), 2);
        assert_eq!(BondConvention::GermanBund.settlement_days(), 2);
        assert_eq!(BondConvention::FrenchOAT.settlement_days(), 2);

        // T+2 markets (JGB cross-border since May 2018)
        assert_eq!(BondConvention::JGB.settlement_days(), 2);
    }

    #[test]
    fn bond_convention_ex_coupon() {
        // UK Gilt has 7-day ex-coupon
        assert_eq!(BondConvention::UKGilt.ex_coupon_days(), Some(7));

        // Others have no ex-coupon convention
        assert_eq!(BondConvention::USTreasury.ex_coupon_days(), None);
        assert_eq!(BondConvention::USAgency.ex_coupon_days(), None);
        assert_eq!(BondConvention::Corporate.ex_coupon_days(), None);
        assert_eq!(BondConvention::GermanBund.ex_coupon_days(), None);
        assert_eq!(BondConvention::FrenchOAT.ex_coupon_days(), None);
        assert_eq!(BondConvention::JGB.ex_coupon_days(), None);
    }

    #[test]
    fn bond_convention_calendar_ids() {
        assert_eq!(BondConvention::USTreasury.calendar_id(), Some("sifma"));
        assert_eq!(BondConvention::USAgency.calendar_id(), Some("sifma"));
        assert_eq!(BondConvention::Corporate.calendar_id(), Some("usny"));
        assert_eq!(BondConvention::GermanBund.calendar_id(), Some("target2"));
        assert_eq!(BondConvention::FrenchOAT.calendar_id(), Some("target2"));
        assert_eq!(BondConvention::UKGilt.calendar_id(), Some("gblo"));
        assert_eq!(BondConvention::JGB.calendar_id(), Some("jpto"));
    }

    #[test]
    fn bond_convention_from_str() {
        // Standard names
        assert_eq!(
            "us_treasury".parse::<BondConvention>().unwrap(),
            BondConvention::USTreasury
        );
        assert_eq!(
            "us_agency".parse::<BondConvention>().unwrap(),
            BondConvention::USAgency
        );
        assert_eq!(
            "jgb".parse::<BondConvention>().unwrap(),
            BondConvention::JGB
        );

        // Aliases
        assert_eq!(
            "ust".parse::<BondConvention>().unwrap(),
            BondConvention::USTreasury
        );
        assert_eq!(
            "agency".parse::<BondConvention>().unwrap(),
            BondConvention::USAgency
        );
        assert_eq!(
            "fnma".parse::<BondConvention>().unwrap(),
            BondConvention::USAgency
        );
        assert_eq!(
            "japanese".parse::<BondConvention>().unwrap(),
            BondConvention::JGB
        );
        assert_eq!(
            "bund".parse::<BondConvention>().unwrap(),
            BondConvention::GermanBund
        );
        assert_eq!(
            "gilt".parse::<BondConvention>().unwrap(),
            BondConvention::UKGilt
        );
    }

    #[test]
    fn bond_convention_display() {
        assert_eq!(format!("{}", BondConvention::USTreasury), "us_treasury");
        assert_eq!(format!("{}", BondConvention::USAgency), "us_agency");
        assert_eq!(format!("{}", BondConvention::JGB), "jgb");
        assert_eq!(format!("{}", BondConvention::GermanBund), "german_bund");
        assert_eq!(format!("{}", BondConvention::UKGilt), "uk_gilt");
        assert_eq!(format!("{}", BondConvention::FrenchOAT), "french_oat");
        assert_eq!(format!("{}", BondConvention::Corporate), "corporate");
    }

    // =======================================================================
    // Commodity Convention Tests
    // =======================================================================

    #[test]
    fn commodity_convention_settlement_days() {
        // Most commodities: T+2
        assert_eq!(CommodityConvention::WTICrude.settlement_days(), 2);
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
            CommodityConvention::WTICrude.business_day_convention(),
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
        assert_eq!(CommodityConvention::WTICrude.calendar_id(), "nymex");
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
            CommodityConvention::WTICrude
        );
        assert_eq!(
            "natural_gas".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::NaturalGas
        );
        assert_eq!(
            "gold".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::Gold
        );

        // Ticker aliases
        assert_eq!(
            "wti".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::WTICrude
        );
        assert_eq!(
            "cl".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::WTICrude
        );
        assert_eq!(
            "ng".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::NaturalGas
        );
        assert_eq!(
            "gc".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::Gold
        );
        assert_eq!(
            "xau".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::Gold
        );
        assert_eq!(
            "hg".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::Copper
        );
        assert_eq!(
            "corn".parse::<CommodityConvention>().unwrap(),
            CommodityConvention::Agricultural
        );
    }

    #[test]
    fn commodity_convention_display() {
        assert_eq!(format!("{}", CommodityConvention::WTICrude), "wti_crude");
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
