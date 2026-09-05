//! Bermudan swaption-specific risk metrics.
//!
//! Provides bump-and-revalue Greeks and exercise analytics for Bermudan swaptions
//! using Hull-White tree pricing.
//!
//! # Metrics
//!
//! - **Delta**: Parallel rate sensitivity (bump discount curve)
//! - **Vega**: Volatility sensitivity (bump HW sigma)
//! - **Gamma**: Second-order rate sensitivity
//! - **Exercise Probabilities**: Risk-neutral exercise distribution
//!
//! # Methodology
//!
//! Since Bermudan swaptions use numerical pricing (tree-based), Greeks are
//! computed via bump-and-revalue:
//!
//! ```text
//! Delta = (V(r+dr) - V(r-dr)) / (2*dr)
//! Gamma = (V(r+dr) - 2*V(r) + V(r-dr)) / (dr^2)
//! Vega = (V(σ+dσ) - V(σ-dσ)) / (2*dσ)
//! ```

use crate::instruments::rates::swaption::pricing::BermudanSwaptionTreeValuator;
use crate::instruments::rates::swaption::{BermudanSwaption, PreparedHullWhiteModel};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::Result;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;

/// Default bump size for parallel rate shift (1 basis point).
pub(crate) const DEFAULT_RATE_BUMP_BP: f64 = 1.0;

/// Default bump size for the second-order (gamma) rate shift (10 basis points).
///
/// Gamma divides by `bump²`; with a ±1bp bump on a recalibrated 50-step tree
/// the exercise-boundary/discretization noise is divided by 1e-8 and dominates
/// the estimate. A 10bp bump trades a small O(bump²) truncation error for a
/// 100× reduction in noise amplification.
pub(crate) const DEFAULT_GAMMA_BUMP_BP: f64 = 10.0;

/// Default bump size for volatility (1% relative).
pub(crate) const DEFAULT_VOL_BUMP_PCT: f64 = 0.01;

/// Default Hull-White mean reversion.
pub(crate) const DEFAULT_KAPPA: f64 = 0.03;

/// Default Hull-White volatility.
pub(crate) const DEFAULT_SIGMA: f64 = 0.01;

/// Default tree steps for Greeks.
pub(crate) const DEFAULT_TREE_STEPS: usize = 50;

/// Validates Hull–White parameters used by Bermudan Greek calculators.
///
/// In release builds, invalid parameters must not be silently accepted: tree
/// calibration and finite-difference vega can otherwise produce NaNs or garbage.
fn validate_hw_greek_params(kappa: f64, sigma: f64) -> Result<()> {
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Hull-White volatility (sigma) must be positive and finite for Bermudan Greeks".into(),
        ));
    }
    if !kappa.is_finite() || kappa < 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Hull-White mean reversion (kappa) must be non-negative and finite for Bermudan Greeks"
                .into(),
        ));
    }
    Ok(())
}

/// Hull-White tree settings shared by the Bermudan bump-and-revalue Greeks.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HwGreekParams {
    /// Hull-White mean reversion κ (inverse years).
    pub(crate) kappa: f64,
    /// Hull-White short-rate volatility σ (absolute, annual).
    pub(crate) sigma: f64,
    /// Number of tree steps.
    pub(crate) tree_steps: usize,
}

impl Default for HwGreekParams {
    fn default() -> Self {
        Self {
            kappa: DEFAULT_KAPPA,
            sigma: DEFAULT_SIGMA,
            tree_steps: DEFAULT_TREE_STEPS,
        }
    }
}

impl HwGreekParams {
    /// Tree settings from calibrated Hull-White parameters with the default step count.
    pub(crate) fn from_calibration(params: HullWhiteCalibrationParams) -> Self {
        Self {
            kappa: params.kappa,
            sigma: params.sigma,
            tree_steps: DEFAULT_TREE_STEPS,
        }
    }
}

/// Price a Bermudan swaption on a freshly prepared Hull-White tree.
///
/// # Arguments
///
/// * `swaption` - Bermudan swaption to value.
/// * `disc` - Discount curve the tree is calibrated to (possibly bumped).
/// * `as_of` - Valuation date.
/// * `hw` - Tree settings; `sigma` may be a bumped volatility.
///
/// # Errors
///
/// Returns a validation error for invalid Hull-White parameters or when the
/// tree/valuator cannot be built.
fn price_bermudan_on_tree(
    swaption: &BermudanSwaption,
    disc: &dyn Discounting,
    as_of: Date,
    hw: HwGreekParams,
) -> Result<f64> {
    let ttm = swaption.time_to_maturity(as_of)?;
    if ttm <= 0.0 {
        return Ok(0.0);
    }
    validate_hw_greek_params(hw.kappa, hw.sigma)?;
    let model = PreparedHullWhiteModel::prepare(
        HullWhiteCalibrationParams::new(hw.kappa, hw.sigma)?,
        hw.tree_steps,
        disc,
        ttm,
    )?;
    let valuator = BermudanSwaptionTreeValuator::new(swaption, &model, disc, as_of)?;
    valuator.price()
}

/// Price the swaption on the discount curve bumped by `+bump_bp` and `-bump_bp`.
///
/// The Hull-White tree is a single-factor short-rate model that derives all
/// rates from the input discount curve, so only the discount curve is bumped:
/// sensitivity to a separate forward curve (multi-curve basis) is not captured.
fn price_bumped_pair(
    swaption: &BermudanSwaption,
    context: &MetricContext,
    hw: HwGreekParams,
    bump_bp: f64,
) -> Result<(f64, f64)> {
    let curve_id = swaption.get_discount_curve_id();
    let mut prices = [0.0; 2];
    for (slot, sign) in prices.iter_mut().zip([1.0, -1.0]) {
        let curves = context.curves.bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec::parallel_bp(sign * bump_bp),
        }])?;
        let disc = curves.get_discount(curve_id.as_str())?;
        *slot = price_bermudan_on_tree(swaption, disc.as_ref(), context.as_of, hw)?;
    }
    Ok((prices[0], prices[1]))
}

// Bermudan Delta Calculator

/// Delta calculator for Bermudan swaptions.
///
/// Computes sensitivity to parallel rate shifts via bump-and-revalue.
#[derive(Debug, Clone)]
pub(crate) struct BermudanDeltaCalculator {
    /// Rate bump size in basis points
    pub(crate) bump_bp: f64,
    /// Hull-White tree settings
    pub(crate) hw: HwGreekParams,
}

impl MetricCalculator for BermudanDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swaption = context.instrument_as::<BermudanSwaption>()?;

        let bump_bp = self.bump_bp.abs();
        let bump = bump_bp / 10000.0;
        if bump <= 0.0 {
            return Ok(0.0);
        }
        let (price_up, price_dn) = price_bumped_pair(swaption, context, self.hw, bump_bp)?;
        Ok((price_up - price_dn) / (2.0 * bump))
    }
}

// Bermudan Vega Calculator

/// Vega calculator for Bermudan swaptions.
///
/// Computes sensitivity to Hull-White volatility changes.
#[derive(Debug, Clone)]
pub(crate) struct BermudanVegaCalculator {
    /// Volatility bump (relative fraction of σ)
    pub(crate) bump_pct: f64,
    /// Hull-White tree settings (base σ)
    pub(crate) hw: HwGreekParams,
}

impl MetricCalculator for BermudanVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swaption = context.instrument_as::<BermudanSwaption>()?;

        let disc = context
            .curves
            .get_discount(swaption.get_discount_curve_id().as_str())?;

        validate_hw_greek_params(self.hw.kappa, self.hw.sigma)?;

        // Bump volatility
        let sigma_up = self.hw.sigma * (1.0 + self.bump_pct);
        let sigma_down = self.hw.sigma * (1.0 - self.bump_pct);
        validate_hw_greek_params(self.hw.kappa, sigma_up)?;
        validate_hw_greek_params(self.hw.kappa, sigma_down)?;

        let denom = 2.0 * self.bump_pct * self.hw.sigma;
        if !denom.is_finite() || denom.abs() <= f64::EPSILON * 1024.0 {
            return Err(finstack_quant_core::Error::Validation(
                "Bermudan vega: bump_pct and sigma must yield a non-zero finite denominator".into(),
            ));
        }

        let up = HwGreekParams {
            sigma: sigma_up,
            ..self.hw
        };
        let down = HwGreekParams {
            sigma: sigma_down,
            ..self.hw
        };
        let price_up = price_bermudan_on_tree(swaption, disc.as_ref(), context.as_of, up)?;
        let price_down = price_bermudan_on_tree(swaption, disc.as_ref(), context.as_of, down)?;

        // Central difference, scaled to a 1% volatility change.
        Ok((price_up - price_down) / denom * 0.01)
    }
}

// Bermudan Gamma Calculator

/// Gamma calculator for Bermudan swaptions.
///
/// Computes second-order rate sensitivity via bump-and-revalue.
#[derive(Debug, Clone)]
pub(crate) struct BermudanGammaCalculator {
    /// Rate bump size in basis points
    pub(crate) bump_bp: f64,
    /// Hull-White tree settings
    pub(crate) hw: HwGreekParams,
}

impl MetricCalculator for BermudanGammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swaption = context.instrument_as::<BermudanSwaption>()?;

        let disc = context
            .curves
            .get_discount(swaption.get_discount_curve_id().as_str())?;
        if swaption.time_to_maturity(context.as_of)? <= 0.0 {
            return Ok(0.0);
        }

        let bump_bp = self.bump_bp.abs();
        let bump = bump_bp / 10000.0;
        if bump <= 0.0 {
            return Ok(0.0);
        }

        let base_price = price_bermudan_on_tree(swaption, disc.as_ref(), context.as_of, self.hw)?;
        let (price_up, price_dn) = price_bumped_pair(swaption, context, self.hw, bump_bp)?;
        Ok((price_up - 2.0 * base_price + price_dn) / (bump * bump))
    }
}

// Exercise Probability

/// Expected exercise time **conditional on exercise**, `E[τ | exercise]`, from
/// the risk-neutral exercise probabilities of a priced tree valuator.
///
/// `E[τ | exercise] = Σ tᵢ·pᵢ / Σ pᵢ`. The unconditional sum `Σ tᵢ·pᵢ` drops
/// the surviving (never-exercised) probability mass and is biased toward 0 for
/// OTM swaptions; normalizing by the total exercise probability removes that
/// bias. If the swaption never exercises on the tree (`Σ pᵢ = 0`) the
/// conditional expectation is undefined and `0.0` is returned.
fn expected_exercise_time(valuator: &BermudanSwaptionTreeValuator) -> f64 {
    let tree_probs = valuator.exercise_probabilities();
    let total_exercise_prob: f64 = tree_probs.iter().map(|(_, p)| p).sum();
    let weighted_time: f64 = tree_probs.iter().map(|(t, p)| t * p).sum();
    if total_exercise_prob > 1e-12 {
        weighted_time / total_exercise_prob
    } else {
        0.0
    }
}

/// Calculator for the conditional expected exercise time.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExerciseProbabilityCalculator {
    /// Hull-White tree settings
    pub(crate) hw: HwGreekParams,
}

impl MetricCalculator for ExerciseProbabilityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swaption = context.instrument_as::<BermudanSwaption>()?;

        let disc = context
            .curves
            .get_discount(swaption.get_discount_curve_id().as_str())?;
        let ttm = swaption.time_to_maturity(context.as_of)?;

        if ttm <= 0.0 {
            return Ok(0.0);
        }

        validate_hw_greek_params(self.hw.kappa, self.hw.sigma)?;
        let model = PreparedHullWhiteModel::prepare(
            HullWhiteCalibrationParams::new(self.hw.kappa, self.hw.sigma)?,
            self.hw.tree_steps,
            disc.as_ref(),
            ttm,
        )?;
        let valuator =
            BermudanSwaptionTreeValuator::new(swaption, &model, disc.as_ref(), context.as_of)?;
        Ok(expected_exercise_time(&valuator))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::OptionType;

    #[test]
    fn validate_hw_greek_params_accepts_typical_values() {
        assert!(validate_hw_greek_params(0.03, 0.01).is_ok());
    }

    #[test]
    fn validate_hw_greek_params_rejects_non_positive_sigma() {
        assert!(validate_hw_greek_params(0.03, 0.0).is_err());
        assert!(validate_hw_greek_params(0.03, -0.01).is_err());
        assert!(validate_hw_greek_params(0.03, f64::NAN).is_err());
    }

    #[test]
    fn validate_hw_greek_params_rejects_negative_kappa() {
        assert!(validate_hw_greek_params(-0.01, 0.01).is_err());
        assert!(validate_hw_greek_params(f64::NAN, 0.01).is_err());
    }

    #[test]
    fn expected_exercise_time_from_valuator_is_non_negative() {
        // Integration test: verify from_valuator uses actual tree probabilities
        use crate::instruments::rates::swaption::{
            BermudanSchedule, BermudanSwaption, PreparedHullWhiteModel,
        };
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::Tenor;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::interp::InterpStyle;
        use finstack_quant_core::money::Money;
        use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
        use time::Month;

        // Create test discount curve
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
            .knots([
                (0.0, 1.0),
                (0.5, 0.985),
                (1.0, 0.97),
                (2.0, 0.94),
                (5.0, 0.85),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Valid curve");

        // Create test Bermudan swaption
        let swap_start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let swap_end = Date::from_calendar_date(2028, Month::January, 1).expect("Valid date");
        let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

        let swaption = BermudanSwaption::new(
            "TEST-BERM",
            OptionType::Call,
            Money::from((10_000_000_i64, Currency::USD)),
            0.03,
            swap_start,
            swap_end,
            BermudanSchedule::co_terminal(first_exercise, swap_end, Tenor::semi_annual())
                .expect("valid Bermudan schedule"),
            "USD-OIS",
            "USD-OIS",
            "USD-VOL",
        )
        .expect("valid Bermudan swaption");

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");

        let model = PreparedHullWhiteModel::prepare(
            HullWhiteCalibrationParams::new(0.03, 0.01).expect("valid HW params"),
            30,
            &curve,
            ttm,
        )
        .expect("Valid model");
        let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of)
            .expect("Valid valuator");

        // Expected exercise time should be reasonable.
        assert!(expected_exercise_time(&valuator) >= 0.0);
    }
}
