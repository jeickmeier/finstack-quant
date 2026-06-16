//! Margin-specific metrics.
//!
//! This module provides metrics calculators for margin-related analysis:
//!
//! - Margin utilization
//! - Excess collateral
//! - Margin funding cost
//! - Haircut sensitivity (Haircut01)
//! - Instrument-level IM and VM calculations

/// Instrument-level IM, VM, and total-margin metrics.
pub mod instrument;

pub use instrument::{
    calculate_instrument_margins, InitialMarginMetric, TotalMarginMetric, VariationMarginMetric,
};

use finstack_quant_core::money::Money;

/// Margin utilization result.
///
/// Ratio of posted margin to required margin.
#[derive(Debug, Clone, PartialEq)]
pub struct MarginUtilization {
    /// Posted margin amount
    pub posted: Money,
    /// Required margin amount
    pub required: Money,
    /// Utilization ratio (posted / required)
    pub ratio: f64,
}

impl MarginUtilization {
    /// Create a new margin utilization result.
    pub fn new(posted: Money, required: Money) -> Self {
        let ratio = if required.amount() > 0.0 {
            posted.amount() / required.amount()
        } else if posted.amount() > 0.0 {
            f64::INFINITY
        } else {
            1.0
        };

        Self {
            posted,
            required,
            ratio,
        }
    }

    /// Check if margin is adequate (ratio >= 1.0).
    #[must_use]
    pub fn is_adequate(&self) -> bool {
        self.ratio >= 1.0
    }

    /// Get the shortfall amount (if any).
    #[must_use]
    pub fn shortfall(&self) -> Money {
        if self.ratio < 1.0 {
            Money::new(
                self.required.amount() - self.posted.amount(),
                self.posted.currency(),
            )
        } else {
            Money::new(0.0, self.posted.currency())
        }
    }
}

/// Excess collateral result.
///
/// Amount of collateral above the required level.
#[derive(Debug, Clone, PartialEq)]
pub struct ExcessCollateral {
    /// Collateral value
    pub collateral_value: Money,
    /// Required value
    pub required_value: Money,
    /// Excess amount (positive) or shortfall (negative)
    pub excess: Money,
}

impl ExcessCollateral {
    /// Create a new excess collateral result.
    pub fn new(collateral_value: Money, required_value: Money) -> Self {
        let currency = collateral_value.currency();
        let excess = Money::new(
            collateral_value.amount() - required_value.amount(),
            currency,
        );

        Self {
            collateral_value,
            required_value,
            excess,
        }
    }

    /// Check if there is excess collateral.
    #[must_use]
    pub fn has_excess(&self) -> bool {
        self.excess.amount() > 0.0
    }

    /// Check if there is a shortfall.
    #[must_use]
    pub fn has_shortfall(&self) -> bool {
        self.excess.amount() < 0.0
    }

    /// Get the excess percentage.
    #[must_use]
    pub fn excess_percentage(&self) -> f64 {
        if self.required_value.amount() > 0.0 {
            self.excess.amount() / self.required_value.amount()
        } else {
            0.0
        }
    }
}

/// Margin funding cost result.
///
/// Cost of funding posted margin collateral.
#[derive(Debug, Clone, PartialEq)]
pub struct MarginFundingCost {
    /// Posted margin amount
    pub margin_posted: Money,
    /// Funding rate (annualized)
    pub funding_rate: f64,
    /// Collateral return rate (e.g., Fed Funds)
    pub collateral_rate: f64,
    /// Net funding cost (annualized)
    pub annual_cost: Money,
}

impl MarginFundingCost {
    /// Calculate margin funding cost.
    ///
    /// # Formula
    ///
    /// ```text
    /// Annual_Cost = Margin × (Funding_Rate - Collateral_Rate)
    /// ```
    pub fn calculate(margin_posted: Money, funding_rate: f64, collateral_rate: f64) -> Self {
        let spread = funding_rate - collateral_rate;
        let annual_cost = margin_posted * spread;

        Self {
            margin_posted,
            funding_rate,
            collateral_rate,
            annual_cost,
        }
    }

    /// Get the funding spread (funding rate - collateral rate).
    #[must_use]
    pub fn spread(&self) -> f64 {
        self.funding_rate - self.collateral_rate
    }

    /// Calculate cost for a specific period.
    pub fn cost_for_period(&self, year_fraction: f64) -> Money {
        self.annual_cost * year_fraction
    }
}

/// Haircut sensitivity (Haircut01) result.
///
/// Change in PV for a 1bp change in haircut.
#[derive(Debug, Clone, PartialEq)]
pub struct Haircut01 {
    /// Collateral value
    pub collateral_value: Money,
    /// Current haircut (decimal)
    pub current_haircut: f64,
    /// PV change for +1bp haircut
    pub pv_change: Money,
}

impl Haircut01 {
    /// Calculate Haircut01.
    ///
    /// # Formula
    ///
    /// ```text
    /// Haircut01 = Collateral_Value × 0.0001
    /// ```
    pub fn calculate(collateral_value: Money, current_haircut: f64) -> Self {
        const ONE_BP: f64 = 0.0001;
        let pv_change = collateral_value * ONE_BP;

        Self {
            collateral_value,
            current_haircut,
            pv_change,
        }
    }

    /// Get the haircut in basis points.
    #[must_use]
    pub fn haircut_bps(&self) -> f64 {
        self.current_haircut * 10_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn margin_utilization() {
        let posted = Money::new(12_000_000.0, Currency::USD);
        let required = Money::new(10_000_000.0, Currency::USD);
        let util = MarginUtilization::new(posted, required);

        assert!(util.is_adequate());
        assert_eq!(util.ratio, 1.2);
        assert_eq!(util.shortfall().amount(), 0.0);
    }

    #[test]
    fn margin_shortfall() {
        let posted = Money::new(8_000_000.0, Currency::USD);
        let required = Money::new(10_000_000.0, Currency::USD);
        let util = MarginUtilization::new(posted, required);

        assert!(!util.is_adequate());
        assert_eq!(util.ratio, 0.8);
        assert_eq!(util.shortfall().amount(), 2_000_000.0);
    }

    #[test]
    fn excess_collateral() {
        let collateral = Money::new(105_000_000.0, Currency::USD);
        let required = Money::new(100_000_000.0, Currency::USD);
        let excess = ExcessCollateral::new(collateral, required);

        assert!(excess.has_excess());
        assert!(!excess.has_shortfall());
        assert_eq!(excess.excess.amount(), 5_000_000.0);
        assert_eq!(excess.excess_percentage(), 0.05);
    }

    #[test]
    fn margin_funding_cost() {
        let margin = Money::new(50_000_000.0, Currency::USD);
        let funding_rate = 0.055; // 5.5%
        let collateral_rate = 0.053; // 5.3%

        let cost = MarginFundingCost::calculate(margin, funding_rate, collateral_rate);

        assert!((cost.spread() - 0.002).abs() < 1e-10); // 20bp
        assert!((cost.annual_cost.amount() - 100_000.0).abs() < 0.01); // 50M × 0.2%
    }

    #[test]
    fn haircut01() {
        let collateral = Money::new(100_000_000.0, Currency::USD);
        let h01 = Haircut01::calculate(collateral, 0.02);

        assert_eq!(h01.pv_change.amount(), 10_000.0); // 100M × 0.01%
        assert_eq!(h01.haircut_bps(), 200.0); // 2% = 200bp
    }
}
