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
//!   *Journal of Portfolio Management*, 15(3), 9-14. `docs/REFERENCES.md#richard-roll-1989`

use super::super::{clamped_cpr_to_smm, expected_shocked_smm};
use super::traits::StochasticPrepayment;

/// Richard-Roll prepayment model for RMBS.
///
/// Full stochastic prepayment model with refinancing incentive,
/// seasoning, burnout, and optional seasonality.
#[derive(Debug, Clone)]
pub struct RichardRollPrepay {
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
    pub fn new(base_cpr: f64, refi_sensitivity: f64, pool_coupon: f64, burnout_rate: f64) -> Self {
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

        let z = factors.first().copied().unwrap_or(0.0);
        let shock = (self.factor_loading * z * self.cpr_volatility).exp();
        let shocked_cpr = (base_conditional_cpr * shock).clamp(0.0, 1.0);

        clamped_cpr_to_smm(shocked_cpr)
    }

    fn expected_smm(&self, seasoning: u32) -> f64 {
        // Expected SMM assuming the market rate sits at the pool coupon
        // (zero refi incentive). The refi multiplier is kept explicit so the
        // expectation stays consistent with `conditional_smm` if the
        // at-the-money convention changes.
        let refi_mult = self.refi_multiplier(self.pool_coupon);
        let season_mult = self.seasoning_multiplier(seasoning);
        let month_mult = self.seasonality_multiplier((seasoning % 12) + 1);
        // Average the clipped monthly rate, not the annual CPR before its
        // nonlinear conversion. Burnout is one under this baseline convention.
        let base_cpr = self.base_cpr * refi_mult * season_mult * month_mult;
        expected_shocked_smm(base_cpr, self.factor_loading * self.cpr_volatility)
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
    #[test]
    fn expected_smm_integrates_the_monthly_rate_used_for_burnout() {
        let model =
            RichardRollPrepay::with_all_params(0.20, 0.0, 20.0, 0.05, 0.10, 0.0, 1.0, 1.0, 30);
        let step = 0.0001;
        let integrated: f64 = (0..200_000)
            .map(|i| {
                let z = -10.0 + (f64::from(i) + 0.5) * step;
                let density = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
                model.conditional_smm(30, &[z], 0.05, 1.0) * density * step
            })
            .sum();
        assert!((model.expected_smm(30) - integrated).abs() < 2e-6);
        assert!(model.has_burnout());
    }

    /// Refi incentive moves SMM in the right direction: above-market coupons
    /// prepay faster; below-market coupons prepay slower.
    #[test]
    fn refi_incentive_moves_prepayment_speed_in_the_right_direction() {
        let pool_coupon = 0.06_f64;
        let model = RichardRollPrepay::new(0.06, 2.0, pool_coupon, 0.0);

        // No incentive: coupon exactly at market.
        let at_market = model.refi_multiplier(pool_coupon);
        // In the money: market rates 200bp BELOW the pool coupon.
        let in_the_money = model.refi_multiplier(pool_coupon - 0.02);
        // Out of the money: market rates 200bp ABOVE the pool coupon.
        let out_of_the_money = model.refi_multiplier(pool_coupon + 0.02);

        assert!(
            (at_market - 1.0).abs() < 1e-12,
            "at market the refi multiplier must be exactly 1, got {at_market}"
        );
        assert!(
            in_the_money > at_market * 1.05,
            "a pool 200bp in the money must prepay faster: \
             multiplier {in_the_money} vs {at_market} at market"
        );
        assert!(
            out_of_the_money < at_market * 0.95,
            "a pool 200bp OUT OF THE MONEY must prepay materially slower: \
             multiplier {out_of_the_money} vs {at_market} at market"
        );
        assert!(
            in_the_money > out_of_the_money,
            "refi response must be monotone in the incentive"
        );
    }

    use super::*;

    #[test]
    fn test_richard_roll_creation() {
        let model = RichardRollPrepay::new(0.06, 2.0, 0.045, 0.10);

        assert!((model.base_cpr - 0.06).abs() < 1e-10);
        assert!((model.refi_sensitivity - 2.0).abs() < 1e-10);
        assert!((model.burnout_rate - 0.10).abs() < 1e-10);
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
}
