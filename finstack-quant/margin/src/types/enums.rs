//! Shared enums for margin and collateral management.

use std::fmt;

/// Margin call frequency.
///
/// Determines how often margin calls are made and collateral is exchanged.
/// Industry standard for OTC derivatives is daily under BCBS-IOSCO rules.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum MarginTenor {
    /// Daily margin calls (standard for OTC derivatives post-2016)
    #[default]
    Daily,
    /// Weekly margin calls (pre-regulatory period)
    Weekly,
    /// Monthly margin calls (pre-regulatory period)
    Monthly,
    /// On-demand margin calls (used for repos and some bilateral agreements)
    OnDemand,
}

impl fmt::Display for MarginTenor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarginTenor::Daily => write!(f, "daily"),
            MarginTenor::Weekly => write!(f, "weekly"),
            MarginTenor::Monthly => write!(f, "monthly"),
            MarginTenor::OnDemand => write!(f, "on_demand"),
        }
    }
}

impl std::str::FromStr for MarginTenor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "daily" => Ok(MarginTenor::Daily),
            "weekly" => Ok(MarginTenor::Weekly),
            "monthly" => Ok(MarginTenor::Monthly),
            "on_demand" | "ondemand" => Ok(MarginTenor::OnDemand),
            other => Err(format!("Unknown margin frequency: {}", other)),
        }
    }
}

/// Initial margin calculation methodology.
///
/// Different methodologies are used depending on regulatory requirements,
/// product type, and whether trades are cleared or bilateral.
///
/// # BCBS-IOSCO Standards
///
/// For bilateral (uncleared) OTC derivatives, either SIMM or the regulatory
/// schedule approach may be used. SIMM is the industry standard for large
/// dealers due to its risk-sensitivity.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum ImMethodology {
    /// Haircut-based IM calculation (standard for repos and securities financing)
    ///
    /// IM = Collateral_Value × Haircut
    ///
    /// Haircuts are determined by asset class, rating, maturity, and FX mismatch.
    Haircut,

    /// ISDA Standard Initial Margin Model (SIMM)
    ///
    /// Sensitivities-based model calculating IM from delta, vega, and curvature
    /// risk across interest rate, credit, equity, commodity, and FX risk classes.
    ///
    /// Reference: ISDA SIMM Methodology v2.6 (2023)
    #[default]
    Simm,

    /// BCBS-IOSCO regulatory schedule approach
    ///
    /// Grid-based IM calculation using notional × rate based on asset class and maturity.
    /// Simpler but typically more conservative than SIMM.
    ///
    /// Example rates:
    /// - Interest Rate: 0-2yr: 1%, 2-5yr: 2%, 5+yr: 4%
    /// - Credit: 2-5yr: 5%, 5+yr: 10%
    /// - Equity: 15%
    /// - FX: 6%
    Schedule,

    /// Internal model approved by regulator
    ///
    /// Bank's proprietary VaR or ES-based model approved by supervisor.
    /// Must meet regulatory backtesting and validation requirements.
    InternalModel,

    /// Clearing house methodology
    ///
    /// CCP-specific IM calculation (typically VaR or SPAN-based).
    /// Examples: LCH SwapClear, CME, ICE Clear Credit
    ClearingHouse,
}

impl fmt::Display for ImMethodology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImMethodology::Haircut => write!(f, "haircut"),
            ImMethodology::Simm => write!(f, "simm"),
            ImMethodology::Schedule => write!(f, "schedule"),
            ImMethodology::InternalModel => write!(f, "internal_model"),
            ImMethodology::ClearingHouse => write!(f, "clearing_house"),
        }
    }
}

impl std::str::FromStr for ImMethodology {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "haircut" => Ok(ImMethodology::Haircut),
            "simm" => Ok(ImMethodology::Simm),
            "schedule" => Ok(ImMethodology::Schedule),
            "internal_model" | "internalmodel" => Ok(ImMethodology::InternalModel),
            "clearing_house" | "clearinghouse" | "ccp" => Ok(ImMethodology::ClearingHouse),
            other => Err(format!("Unknown IM methodology: {}", other)),
        }
    }
}

/// Clearing status for OTC derivatives.
///
/// Determines whether a trade is cleared through a CCP or remains bilateral
/// under a CSA agreement.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum ClearingStatus {
    /// Bilateral (uncleared) trade governed by CSA
    ///
    /// Subject to BCBS-IOSCO margin requirements for non-centrally cleared derivatives.
    /// Requires both VM and IM (above thresholds).
    #[default]
    Bilateral,

    /// Trade cleared through a central counterparty (CCP)
    ///
    /// Margin requirements set by the CCP. IM is typically VaR-based.
    /// VM is exchanged daily with no threshold.
    Cleared {
        /// CCP identifier (e.g., "LCH", "CME", "ICE", "JSCC")
        ccp: String,
    },
}

impl fmt::Display for ClearingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClearingStatus::Bilateral => write!(f, "bilateral"),
            ClearingStatus::Cleared { ccp } => write!(f, "cleared:{}", ccp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn margin_frequency_display_and_parse() {
        assert_eq!(MarginTenor::Daily.to_string(), "daily");
        assert_eq!(MarginTenor::Weekly.to_string(), "weekly");
        assert_eq!(
            "daily".parse::<MarginTenor>().expect("valid"),
            MarginTenor::Daily
        );
        assert_eq!(
            "on_demand".parse::<MarginTenor>().expect("valid"),
            MarginTenor::OnDemand
        );
    }

    #[test]
    fn im_methodology_display_and_parse() {
        assert_eq!(ImMethodology::Simm.to_string(), "simm");
        assert_eq!(ImMethodology::Schedule.to_string(), "schedule");
        assert_eq!(
            "simm".parse::<ImMethodology>().expect("valid"),
            ImMethodology::Simm
        );
        assert_eq!(
            "clearing_house".parse::<ImMethodology>().expect("valid"),
            ImMethodology::ClearingHouse
        );
    }

    #[test]
    fn clearing_status_display() {
        assert_eq!(ClearingStatus::Bilateral.to_string(), "bilateral");
        assert_eq!(
            ClearingStatus::Cleared {
                ccp: "LCH".to_string()
            }
            .to_string(),
            "cleared:LCH"
        );
    }

    #[test]
    fn defaults_are_correct() {
        assert_eq!(MarginTenor::default(), MarginTenor::Daily);
        assert_eq!(ImMethodology::default(), ImMethodology::Simm);
        assert_eq!(ClearingStatus::default(), ClearingStatus::Bilateral);
    }
}
