//! Richard-Roll prepayment model for RMBS.
//!
//! The industry-standard stochastic prepayment model that captures:
//! - Refinancing incentive (rate sensitivity)
//! - Seasoning ramp
//! - Burnout effects
//! - Seasonality patterns
//!
//! # Mathematical Model
//!
//! ```text
//! CPR(t, r, B) = refi_incentive(r) × seasoning(t) × burnout(B) × seasonality(month)
//! ```
//!
//! ## Refinancing Incentive
//!
//! The arctangent-based refi function:
//! ```text
//! refi(incentive) = base_cpr × (1 + γ × arctan(λ × (coupon - market_rate)))
//! ```
//!
//! where incentive = coupon - market_rate.
//!
//! ## Burnout
//!
//! Multiplicative burnout that decays based on cumulative prepayments:
//! ```text
//! B(t) = B(t-1) × (1 - decay_rate × prepay_fraction)
//! ```
//!
//! # References
//!
//! - Richard, S.F., & Roll, R. (1989). "Prepayments on Fixed-Rate Mortgage-Backed Securities."
//!   *Journal of Portfolio Management*, 15(3), 9-14.

#![allow(dead_code)]

use super::super::calibrations::rmbs_standard;
use super::traits::StochasticPrepayment;
use crate::instruments::fixed_income::structured_credit::utils::rates::cpr_to_smm;
use finstack_core::types::{Percentage, Rate};

/// Richard-Roll prepayment model for RMBS.
///
/// Full stochastic prepayment model with refinancing incentive,
/// seasoning, burnout, and optional seasonality.
#[derive(Debug, Clone)]
pub(crate) struct RichardRollPrepay {
    /// Base CPR at full seasoning (post-ramp)
    base_cpr: f64,
    /// Refinancing sensitivity parameter (gamma)
    refi_sensitivity: f64,
    /// Refinancing slope parameter (lambda)
    refi_slope: f64,
    /// AssetPool coupon rate (WAC)
    pool_coupon: f64,
    /// Burnout decay rate per prepayment
    burnout_rate: f64,
    /// Seasonality amplitude (0 = no seasonality)
    seasonality_amplitude: f64,
    /// Factor loading for correlation
    factor_loading: f64,
    /// CPR volatility
    cpr_volatility: f64,
    /// Ramp months (typically 30 for PSA-like ramp)
    ramp_months: u32,
}

impl RichardRollPrepay {
    /// Create a Richard-Roll prepayment model.
    ///
    /// # Arguments
    /// * `base_cpr` - Base CPR at full seasoning
    /// * `refi_sensitivity` - Sensitivity to refinancing incentive (gamma)
    /// * `pool_coupon` - AssetPool weighted average coupon
    /// * `burnout_rate` - Burnout decay rate
    pub(crate) fn new(
        base_cpr: f64,
        refi_sensitivity: f64,
        pool_coupon: f64,
        burnout_rate: f64,
    ) -> Self {
        Self {
            base_cpr: base_cpr.clamp(0.0, 1.0),
            refi_sensitivity: refi_sensitivity.clamp(0.0, 10.0),
            refi_slope: 20.0, // Standard slope
            pool_coupon,
            burnout_rate: burnout_rate.clamp(0.0, 1.0),
            seasonality_amplitude: 0.0,
            factor_loading: 0.4,
            cpr_volatility: 0.20,
            ramp_months: 30,
        }
    }

    /// Create a Richard-Roll prepayment model using typed rates.
    pub(crate) fn new_typed(
        base_cpr: Percentage,
        refi_sensitivity: f64,
        pool_coupon: Rate,
        burnout_rate: Percentage,
    ) -> Self {
        Self {
            base_cpr: base_cpr.as_decimal().clamp(0.0, 1.0),
            refi_sensitivity: refi_sensitivity.clamp(0.0, 10.0),
            refi_slope: 20.0,
            pool_coupon: pool_coupon.as_decimal(),
            burnout_rate: burnout_rate.as_decimal().clamp(0.0, 1.0),
            seasonality_amplitude: 0.0,
            factor_loading: 0.4,
            cpr_volatility: 0.20,
            ramp_months: 30,
        }
    }

    /// Create with full customization.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_all_params(
        base_cpr: f64,
        refi_sensitivity: f64,
        refi_slope: f64,
        pool_coupon: f64,
        burnout_rate: f64,
        seasonality_amplitude: f64,
        factor_loading: f64,
        cpr_volatility: f64,
        ramp_months: u32,
    ) -> Self {
        Self {
            base_cpr: base_cpr.clamp(0.0, 1.0),
            refi_sensitivity: refi_sensitivity.clamp(0.0, 10.0),
            refi_slope: refi_slope.clamp(1.0, 100.0),
            pool_coupon,
            burnout_rate: burnout_rate.clamp(0.0, 1.0),
            seasonality_amplitude: seasonality_amplitude.clamp(0.0, 0.5),
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            cpr_volatility: cpr_volatility.clamp(0.0, 1.0),
            ramp_months: ramp_months.max(1),
        }
    }

    /// Create with full customization using typed rates.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_all_params_typed(
        base_cpr: Percentage,
        refi_sensitivity: f64,
        refi_slope: f64,
        pool_coupon: Rate,
        burnout_rate: Percentage,
        seasonality_amplitude: Percentage,
        factor_loading: f64,
        cpr_volatility: Percentage,
        ramp_months: u32,
    ) -> Self {
        Self {
            base_cpr: base_cpr.as_decimal().clamp(0.0, 1.0),
            refi_sensitivity: refi_sensitivity.clamp(0.0, 10.0),
            refi_slope: refi_slope.clamp(1.0, 100.0),
            pool_coupon: pool_coupon.as_decimal(),
            burnout_rate: burnout_rate.as_decimal().clamp(0.0, 1.0),
            seasonality_amplitude: seasonality_amplitude.as_decimal().clamp(0.0, 0.5),
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            cpr_volatility: cpr_volatility.as_decimal().clamp(0.0, 1.0),
            ramp_months: ramp_months.max(1),
        }
    }

    /// RMBS agency standard calibration.
    ///
    /// Typical parameters for conforming agency MBS:
    /// - Base CPR: 6%
    /// - Refi sensitivity: 2.0
    /// - Burnout rate: 0.10
    pub(crate) fn agency_standard(pool_coupon: f64) -> Self {
        let calibration = rmbs_standard();
        Self::with_all_params(
            calibration.base_cpr,
            calibration.refi_sensitivity,
            20.0,
            pool_coupon,
            calibration.burnout_rate,
            0.0,
            calibration.prepay_factor_loading,
            calibration.cpr_volatility,
            30,
        )
    }

    /// RMBS agency standard calibration using a typed pool coupon.
    pub(crate) fn agency_standard_rate(pool_coupon: Rate) -> Self {
        Self::agency_standard(pool_coupon.as_decimal())
    }

    /// RMBS non-agency calibration (higher voluntary prepay).
    pub(crate) fn non_agency_standard(pool_coupon: f64) -> Self {
        Self::new(0.08, 2.5, pool_coupon, 0.15)
    }

    /// RMBS non-agency calibration using a typed pool coupon.
    pub(crate) fn non_agency_standard_rate(pool_coupon: Rate) -> Self {
        Self {
            base_cpr: 0.08,
            refi_sensitivity: 2.5,
            refi_slope: 20.0,
            pool_coupon: pool_coupon.as_decimal(),
            burnout_rate: 0.15,
            seasonality_amplitude: 0.0,
            factor_loading: 0.4,
            cpr_volatility: 0.20,
            ramp_months: 30,
        }
    }

    /// Get the base CPR.
    pub(crate) fn base_cpr(&self) -> f64 {
        self.base_cpr
    }

    /// Get the refinancing sensitivity.
    pub(crate) fn refi_sensitivity(&self) -> f64 {
        self.refi_sensitivity
    }

    /// Get the burnout rate.
    pub(crate) fn burnout_rate(&self) -> f64 {
        self.burnout_rate
    }

    /// Calculate the refinancing incentive multiplier.
    ///
    /// Uses arctangent function for smooth, bounded response:
    /// ```text
    /// refi_mult = 1 + γ × arctan(λ × incentive) / (π/2)
    /// ```
    fn refi_multiplier(&self, market_rate: f64) -> f64 {
        let incentive = self.pool_coupon - market_rate;
        let atan_term = (self.refi_slope * incentive).atan();
        let normalized = atan_term / (std::f64::consts::PI / 2.0);
        (1.0 + self.refi_sensitivity * normalized).max(0.0)
    }

    /// Calculate the seasoning ramp multiplier.
    fn seasoning_multiplier(&self, seasoning: u32) -> f64 {
        if seasoning >= self.ramp_months {
            1.0
        } else {
            seasoning as f64 / self.ramp_months as f64
        }
    }

    /// Calculate the seasonality multiplier.
    ///
    /// Mortgage prepayments are higher in spring/summer (home sales).
    fn seasonality_multiplier(&self, month_of_year: u32) -> f64 {
        if self.seasonality_amplitude < 1e-10 {
            return 1.0;
        }

        // Peak in June (month 6), trough in December (month 12)
        let angle = 2.0 * std::f64::consts::PI * (month_of_year as f64 - 6.0) / 12.0;
        1.0 + self.seasonality_amplitude * angle.cos()
    }
}

impl StochasticPrepayment for RichardRollPrepay {
    fn conditional_smm(
        &self,
        seasoning: u32,
        factors: &[f64],
        market_rate: f64,
        burnout: f64,
    ) -> f64 {
        // Base CPR with multipliers
        let refi_mult = self.refi_multiplier(market_rate);
        let season_mult = self.seasoning_multiplier(seasoning);
        let month_mult = self.seasonality_multiplier((seasoning % 12) + 1);

        let base_conditional_cpr = self.base_cpr * refi_mult * season_mult * month_mult * burnout;

        // Apply factor shock
        let z = factors.first().copied().unwrap_or(0.0);
        let shock = (self.factor_loading * z * self.cpr_volatility).exp();
        let shocked_cpr = (base_conditional_cpr * shock).clamp(0.0, 1.0);

        cpr_to_smm(shocked_cpr)
    }

    fn expected_smm(&self, seasoning: u32) -> f64 {
        // Expected SMM at current market rate (assume pool coupon)
        let season_mult = self.seasoning_multiplier(seasoning);
        let month_mult = self.seasonality_multiplier((seasoning % 12) + 1);

        let base_cpr = self.base_cpr * season_mult * month_mult;
        cpr_to_smm(base_cpr)
    }

    fn factor_loading(&self) -> f64 {
        self.factor_loading
    }

    fn model_name(&self) -> &'static str {
        "Richard-Roll Prepayment Model"
    }

    fn has_burnout(&self) -> bool {
        self.burnout_rate > 0.0
    }

    fn is_rate_sensitive(&self) -> bool {
        self.refi_sensitivity > 0.0
    }

    fn update_burnout(&self, current_burnout: f64, realized_smm: f64, expected_smm: f64) -> f64 {
        if self.burnout_rate < 1e-10 {
            return current_burnout;
        }

        let ratio = if expected_smm > 1e-10 {
            realized_smm / expected_smm
        } else {
            1.0
        };

        // Bidirectional burnout: burnout factor decreases when prepayments
        // exceed expectations (fast prepayers leave the pool) and *increases*
        // when actual prepayment is below expected (pool rejuvenation — the
        // surviving borrowers are less rate-sensitive than assumed).
        //
        //   burnout_change = burnout_rate × (ratio - 1)
        //   new_burnout = current × (1 - burnout_change)
        //
        // When ratio > 1: burnout_change > 0 → factor decreases (more burned out).
        // When ratio < 1: burnout_change < 0 → factor increases (rejuvenation).
        let burnout_change = self.burnout_rate * (ratio - 1.0);
        let new_burnout = current_burnout * (1.0 - burnout_change);
        new_burnout.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_richard_roll_creation() {
        let model = RichardRollPrepay::new(0.06, 2.0, 0.045, 0.10);

        assert!((model.base_cpr() - 0.06).abs() < 1e-10);
        assert!((model.refi_sensitivity() - 2.0).abs() < 1e-10);
        assert!((model.burnout_rate() - 0.10).abs() < 1e-10);
        assert!(model.has_burnout());
        assert!(model.is_rate_sensitive());
    }

    #[test]
    fn test_refi_incentive_increases_prepay() {
        let model = RichardRollPrepay::new(0.06, 2.0, 0.045, 0.10);

        // When market rate is below pool coupon (refi incentive)
        let smm_low_rate = model.conditional_smm(36, &[0.0], 0.03, 1.0);
        let smm_at_coupon = model.conditional_smm(36, &[0.0], 0.045, 1.0);
        let smm_high_rate = model.conditional_smm(36, &[0.0], 0.06, 1.0);

        assert!(
            smm_low_rate > smm_at_coupon,
            "Low rate should increase prepay"
        );
        assert!(
            smm_high_rate < smm_at_coupon,
            "High rate should decrease prepay"
        );
    }

    #[test]
    fn test_seasoning_ramp() {
        let model = RichardRollPrepay::new(0.06, 0.0, 0.045, 0.0);

        let smm_early = model.conditional_smm(6, &[0.0], 0.045, 1.0);
        let smm_late = model.conditional_smm(36, &[0.0], 0.045, 1.0);

        // Early seasoning should be ~20% of late (6/30)
        let ratio = smm_early / smm_late;
        assert!((ratio - 0.2).abs() < 0.05);
    }

    #[test]
    fn test_burnout_update() {
        let model = RichardRollPrepay::new(0.06, 2.0, 0.045, 0.10);

        // When realized prepayments exceed expected
        let new_burnout = model.update_burnout(1.0, 0.02, 0.01);
        assert!(
            new_burnout < 1.0,
            "Burnout should decrease when prepay is high"
        );

        // When realized prepayments are below expected, burnout factor increases
        // (pool rejuvenation: surviving borrowers are less rate-sensitive)
        let new_burnout2 = model.update_burnout(0.8, 0.005, 0.01);
        // ratio = 0.5, burnout_change = 0.10 * (0.5 - 1) = -0.05
        // new_burnout = 0.8 * (1 - (-0.05)) = 0.84
        assert!(
            new_burnout2 > 0.8,
            "Burnout should increase (rejuvenate) when prepay is below expected, got {}",
            new_burnout2
        );
        assert!(
            (new_burnout2 - 0.84).abs() < 1e-10,
            "Expected burnout of 0.84, got {}",
            new_burnout2
        );
    }

    #[test]
    fn test_factor_shock() {
        let model = RichardRollPrepay::new(0.06, 0.0, 0.045, 0.0);

        let smm_neg = model.conditional_smm(36, &[-2.0], 0.045, 1.0);
        let smm_zero = model.conditional_smm(36, &[0.0], 0.045, 1.0);
        let smm_pos = model.conditional_smm(36, &[2.0], 0.045, 1.0);

        assert!(smm_pos > smm_zero);
        assert!(smm_neg < smm_zero);
    }

    #[test]
    fn test_standard_calibrations() {
        let agency = RichardRollPrepay::agency_standard(0.045);
        assert!((agency.base_cpr() - 0.06).abs() < 1e-10);

        let non_agency = RichardRollPrepay::non_agency_standard(0.055);
        assert!(non_agency.base_cpr() > agency.base_cpr());
    }
}
