//! Repo margin specification types.

use crate::types::{EligibleCollateralSchedule, MarginTenor};
use finstack_quant_core::types::Percentage;

/// Repo margin type.
///
/// Different margin mechanisms offer varying levels of protection
/// and operational complexity.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum RepoMarginType {
    /// No margining - fixed haircut only.
    ///
    /// Simplest approach where the haircut at inception is the only
    /// protection mechanism. No margin calls during the life of the repo.
    #[default]
    None,

    /// Mark-to-market margining with daily/periodic calls.
    ///
    /// Collateral is revalued periodically and margin calls are made
    /// when the coverage ratio falls below the margin ratio.
    MarkToMarket,

    /// Net exposure margining across a netting set.
    ///
    /// Multiple repos with the same counterparty are netted and
    /// margin is calculated on the net position.
    NetExposure,

    /// Tri-party repo managed by a third-party agent.
    ///
    /// A tri-party agent (e.g., Bank of New York Mellon, J.P. Morgan)
    /// manages collateral allocation, substitution, and margin calls.
    Triparty,
}

impl std::fmt::Display for RepoMarginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoMarginType::None => write!(f, "none"),
            RepoMarginType::MarkToMarket => write!(f, "mark_to_market"),
            RepoMarginType::NetExposure => write!(f, "net_exposure"),
            RepoMarginType::Triparty => write!(f, "triparty"),
        }
    }
}

impl std::str::FromStr for RepoMarginType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "none" => Ok(RepoMarginType::None),
            "mark_to_market" | "marktomarket" | "mtm" => Ok(RepoMarginType::MarkToMarket),
            "net_exposure" | "netexposure" | "net" => Ok(RepoMarginType::NetExposure),
            "triparty" | "tri_party" => Ok(RepoMarginType::Triparty),
            other => Err(format!("Unknown repo margin type: {}", other)),
        }
    }
}

/// GMRA 2011 compliant repo margin specification.
///
/// Defines margin maintenance parameters for repurchase agreements
/// following GMRA 2011 standards.
///
/// # Example
///
/// ```ignore
/// use finstack_quant_margin::{EligibleCollateralSchedule, MarginTenor, RepoMarginSpec, RepoMarginType};
///
/// let margin_spec = RepoMarginSpec {
///     margin_type: RepoMarginType::MarkToMarket,
///     margin_ratio: 1.02,           // 2% over-collateralization
///     margin_call_threshold: 0.01,  // 1% deviation triggers call
///     call_frequency: MarginTenor::Daily,
///     settlement_lag: 1,
///     pays_margin_interest: true,
///     margin_interest_rate: Some(0.05),
///     substitution_allowed: true,
///     eligible_substitutes: Some(EligibleCollateralSchedule::us_treasuries()?),
/// };
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # GMRA 2011 References
///
/// - Paragraph 4: Margin Maintenance
/// - Paragraph 5: Income Payments
/// - Paragraph 8: Substitution
/// - Annex I: Margin Ratio and Haircut
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepoMarginSpec {
    /// Type of margin mechanism.
    pub margin_type: RepoMarginType,

    /// Margin ratio (e.g., 1.02 = 102% collateralization required).
    ///
    /// GMRA typically expresses this as the ratio of Market Value
    /// of Securities to Purchase Price.
    pub margin_ratio: f64,

    /// Percentage deviation that triggers a margin call.
    ///
    /// E.g., 0.01 = 1% deviation from margin ratio triggers a call.
    /// If the current ratio falls below `margin_ratio * (1 - threshold)`,
    /// a margin call is generated.
    pub margin_call_threshold: f64,

    /// Tenor of margin valuation and calls.
    pub call_frequency: MarginTenor,

    /// Settlement lag for margin transfers (business days).
    ///
    /// GMRA standard is typically same-day (0) or next-day (1).
    pub settlement_lag: u32,

    /// Whether margin interest is paid on cash margin transfers.
    ///
    /// Under GMRA, the parties may agree to pay interest on
    /// cash margin transfers.
    pub pays_margin_interest: bool,

    /// Margin interest rate (if applicable).
    ///
    /// Typically tied to overnight rates (Fed Funds, SONIA, ESTR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_interest_rate: Option<f64>,

    /// Whether collateral substitution is permitted.
    ///
    /// GMRA Paragraph 8 governs substitution rights.
    pub substitution_allowed: bool,

    /// Eligible collateral for substitution (if allowed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eligible_substitutes: Option<EligibleCollateralSchedule>,
}

impl Default for RepoMarginSpec {
    fn default() -> Self {
        Self {
            margin_type: RepoMarginType::None,
            margin_ratio: 1.02,
            margin_call_threshold: 0.01,
            call_frequency: MarginTenor::Daily,
            settlement_lag: 1,
            pays_margin_interest: false,
            margin_interest_rate: None,
            substitution_allowed: false,
            eligible_substitutes: None,
        }
    }
}

impl RepoMarginSpec {
    /// Create a spec with no margining (haircut only).
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Create a standard mark-to-market margin spec.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn mark_to_market(margin_ratio: f64, threshold: f64) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            margin_type: RepoMarginType::MarkToMarket,
            margin_ratio,
            margin_call_threshold: threshold,
            call_frequency: MarginTenor::Daily,
            settlement_lag: 1,
            pays_margin_interest: true,
            margin_interest_rate: None,
            substitution_allowed: true,
            eligible_substitutes: Some(EligibleCollateralSchedule::us_treasuries()?),
        })
    }

    /// Create a standard mark-to-market margin spec using typed percentages.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn mark_to_market_pct(
        margin_ratio: Percentage,
        threshold: Percentage,
    ) -> finstack_quant_core::Result<Self> {
        Self::mark_to_market(margin_ratio.as_decimal(), threshold.as_decimal())
    }

    /// Create a tri-party repo margin spec.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn triparty(margin_ratio: f64) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            margin_type: RepoMarginType::Triparty,
            margin_ratio,
            margin_call_threshold: 0.005,
            call_frequency: MarginTenor::Daily,
            settlement_lag: 0,
            pays_margin_interest: true,
            margin_interest_rate: None,
            substitution_allowed: true,
            eligible_substitutes: Some(EligibleCollateralSchedule::bcbs_standard()?),
        })
    }

    /// Create a tri-party repo margin spec using a typed percentage.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn triparty_pct(margin_ratio: Percentage) -> finstack_quant_core::Result<Self> {
        Self::triparty(margin_ratio.as_decimal())
    }

    /// Check if this spec has active margin management.
    #[must_use]
    pub fn has_margining(&self) -> bool {
        !matches!(self.margin_type, RepoMarginType::None)
    }

    /// Calculate the required collateral value for a given cash amount.
    ///
    /// Required_Collateral = Cash_Amount × Margin_Ratio
    #[must_use]
    pub fn required_collateral(&self, cash_amount: f64) -> f64 {
        cash_amount * self.margin_ratio
    }

    /// Calculate the minimum acceptable collateral value before a margin call.
    ///
    /// Call_Trigger = Required_Collateral × (1 - Threshold)
    #[must_use]
    pub fn call_trigger_value(&self, cash_amount: f64) -> f64 {
        self.required_collateral(cash_amount) * (1.0 - self.margin_call_threshold)
    }

    /// Check if a margin call is required given current collateral value.
    #[must_use]
    pub fn requires_margin_call(&self, cash_amount: f64, current_collateral: f64) -> bool {
        if !self.has_margining() {
            return false;
        }
        current_collateral < self.call_trigger_value(cash_amount)
    }

    /// Calculate the margin deficit (if any).
    ///
    /// Returns the additional collateral needed, or 0 if adequately margined.
    #[must_use]
    pub fn margin_deficit(&self, cash_amount: f64, current_collateral: f64) -> f64 {
        let required = self.required_collateral(cash_amount);
        if current_collateral < required {
            required - current_collateral
        } else {
            0.0
        }
    }

    /// Calculate excess collateral (if any).
    ///
    /// Returns the excess collateral that could be returned.
    #[must_use]
    pub fn excess_collateral(&self, cash_amount: f64, current_collateral: f64) -> f64 {
        let required = self.required_collateral(cash_amount);
        if current_collateral > required {
            current_collateral - required
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_none() {
        let spec = RepoMarginSpec::default();
        assert_eq!(spec.margin_type, RepoMarginType::None);
        assert!(!spec.has_margining());
    }

    #[test]
    fn mark_to_market_has_margining() {
        let spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        assert!(spec.has_margining());
        assert_eq!(spec.margin_type, RepoMarginType::MarkToMarket);
    }

    #[test]
    fn required_collateral_calculation() {
        let spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        let required = spec.required_collateral(100_000_000.0);
        assert_eq!(required, 102_000_000.0);
    }

    #[test]
    fn margin_call_trigger() {
        let spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        let trigger = spec.call_trigger_value(100_000_000.0);
        // 102M * 0.99 = 100.98M
        assert!((trigger - 100_980_000.0).abs() < 1.0);
    }

    #[test]
    fn requires_margin_call_below_threshold() {
        let spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        // Cash 100M, threshold 102M * 0.99 = 100.98M
        // Collateral at 100M should trigger call
        assert!(spec.requires_margin_call(100_000_000.0, 100_000_000.0));
        // Collateral at 101M should not trigger call
        assert!(!spec.requires_margin_call(100_000_000.0, 101_000_000.0));
    }

    #[test]
    fn margin_deficit_calculation() {
        let spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        let deficit = spec.margin_deficit(100_000_000.0, 100_000_000.0);
        // Need 102M, have 100M, deficit = 2M
        assert_eq!(deficit, 2_000_000.0);
    }

    #[test]
    fn excess_collateral_calculation() {
        let spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        let excess = spec.excess_collateral(100_000_000.0, 105_000_000.0);
        // Need 102M, have 105M, excess = 3M
        assert_eq!(excess, 3_000_000.0);
    }

    #[test]
    fn triparty_spec() {
        let spec = RepoMarginSpec::triparty(1.02).expect("registry should load");
        assert_eq!(spec.margin_type, RepoMarginType::Triparty);
        assert_eq!(spec.settlement_lag, 0); // Same-day for tri-party
        assert!(spec.substitution_allowed);
    }
}
