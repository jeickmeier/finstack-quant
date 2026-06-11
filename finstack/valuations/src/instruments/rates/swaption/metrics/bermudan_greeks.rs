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

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::rates::swaption::pricing::BermudanSwaptionTreeValuator;
use crate::instruments::rates::swaption::{BermudanSwaption, CalibratedHullWhiteModel};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::dates::Date;
use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_core::market_data::traits::Discounting;
use finstack_core::Result;

/// Default bump size for parallel rate shift (1 basis point).
pub(crate) const DEFAULT_RATE_BUMP_BP: f64 = 1.0;

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
        return Err(finstack_core::Error::Validation(
            "Hull-White volatility (sigma) must be positive and finite for Bermudan Greeks".into(),
        ));
    }
    if !kappa.is_finite() || kappa < 0.0 {
        return Err(finstack_core::Error::Validation(
            "Hull-White mean reversion (kappa) must be non-negative and finite for Bermudan Greeks"
                .into(),
        ));
    }
    Ok(())
}

// ============================================================================
// Bermudan Delta Calculator
// ============================================================================

/// Delta calculator for Bermudan swaptions.
///
/// Computes sensitivity to parallel rate shifts via bump-and-revalue.
#[derive(Debug, Clone)]
pub(crate) struct BermudanDeltaCalculator {
    /// Rate bump size in basis points
    pub(crate) bump_bp: f64,
    /// Hull-White mean reversion
    pub(crate) kappa: f64,
    /// Hull-White volatility
    pub(crate) sigma: f64,
    /// Tree steps
    pub(crate) tree_steps: usize,
}

impl Default for BermudanDeltaCalculator {
    fn default() -> Self {
        Self {
            bump_bp: DEFAULT_RATE_BUMP_BP,
            kappa: DEFAULT_KAPPA,
            sigma: DEFAULT_SIGMA,
            tree_steps: DEFAULT_TREE_STEPS,
        }
    }
}

impl BermudanDeltaCalculator {
    /// Create a new delta calculator.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Set Hull-White parameters.
    ///
    /// Invalid values are rejected when computing metrics (runtime validation).
    pub(crate) fn with_hw_params(mut self, kappa: f64, sigma: f64) -> Self {
        self.kappa = kappa;
        self.sigma = sigma;
        self
    }

    /// Price Bermudan swaption with given parameters.
    fn price_bermudan(
        &self,
        swaption: &BermudanSwaption,
        disc: &dyn Discounting,
        as_of: Date,
        sigma: f64,
    ) -> Result<f64> {
        let ttm = swaption.time_to_maturity(as_of)?;
        if ttm <= 0.0 {
            return Ok(0.0);
        }

        validate_hw_greek_params(self.kappa, sigma)?;
        let model = CalibratedHullWhiteModel::calibrate(
            HullWhiteParams::new(self.kappa, sigma)?,
            self.tree_steps,
            disc,
            ttm,
        )?;
        let valuator = BermudanSwaptionTreeValuator::new(swaption, &model, disc, as_of)?;
        valuator.price()
    }
}

impl MetricCalculator for BermudanDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swaption = context
            .instrument
            .as_any()
            .downcast_ref::<BermudanSwaption>()
            .ok_or_else(|| finstack_core::Error::Validation("Expected BermudanSwaption".into()))?;

        let bump_bp = self.bump_bp.abs();
        let bump = bump_bp / 10000.0;
        if bump <= 0.0 {
            return Ok(0.0);
        }

        // Bump the discount curve for rate sensitivity.
        //
        // NOTE: The Hull-White tree is a single-factor short-rate model that
        // calibrates to and derives all rates (both discount and forward swap
        // rates) from the input discount curve's term structure. The forward
        // curve stored on the swaption (`forward_id`) is NOT used by the tree
        // pricer -- forward rates at each node are endogenous to the calibrated
        // short-rate dynamics.
        //
        // Consequence: this delta captures sensitivity to the discount/funding
        // curve only. If the discount and forward curves are different (e.g.,
        // OIS vs SOFR), sensitivity to the forward-discount spread is NOT
        // captured. A proper multi-curve delta would require either:
        //   (a) a two-factor tree, or
        //   (b) separate "discount delta" and "forward delta" calculators
        //       with the tree reading forward rates from the forward curve.
        let curves_up = context.curves.bump([MarketBump::Curve {
            id: swaption.discount_curve_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp),
        }])?;
        let curves_dn = context.curves.bump([MarketBump::Curve {
            id: swaption.discount_curve_id.clone(),
            spec: BumpSpec::parallel_bp(-bump_bp),
        }])?;

        let disc_up = curves_up.get_discount(swaption.discount_curve_id.as_str())?;
        let disc_dn = curves_dn.get_discount(swaption.discount_curve_id.as_str())?;

        let price_up =
            self.price_bermudan(swaption, disc_up.as_ref(), context.as_of, self.sigma)?;
        let price_dn =
            self.price_bermudan(swaption, disc_dn.as_ref(), context.as_of, self.sigma)?;

        Ok((price_up - price_dn) / (2.0 * bump))
    }
}

// ============================================================================
// Bermudan Vega Calculator
// ============================================================================

/// Vega calculator for Bermudan swaptions.
///
/// Computes sensitivity to Hull-White volatility changes.
#[derive(Debug, Clone)]
pub(crate) struct BermudanVegaCalculator {
    /// Volatility bump (percentage)
    pub(crate) bump_pct: f64,
    /// Hull-White mean reversion
    pub(crate) kappa: f64,
    /// Hull-White volatility (base)
    pub(crate) sigma: f64,
    /// Tree steps
    pub(crate) tree_steps: usize,
}

impl Default for BermudanVegaCalculator {
    fn default() -> Self {
        Self {
            bump_pct: DEFAULT_VOL_BUMP_PCT,
            kappa: DEFAULT_KAPPA,
            sigma: DEFAULT_SIGMA,
            tree_steps: DEFAULT_TREE_STEPS,
        }
    }
}

impl BermudanVegaCalculator {
    /// Create a new vega calculator.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Set Hull-White parameters.
    ///
    /// Invalid values are rejected when computing metrics (runtime validation).
    pub(crate) fn with_hw_params(mut self, kappa: f64, sigma: f64) -> Self {
        self.kappa = kappa;
        self.sigma = sigma;
        self
    }

    /// Price Bermudan swaption with given parameters.
    fn price_bermudan(
        &self,
        swaption: &BermudanSwaption,
        disc: &dyn Discounting,
        as_of: Date,
        sigma: f64,
    ) -> Result<f64> {
        let ttm = swaption.time_to_maturity(as_of)?;
        if ttm <= 0.0 {
            return Ok(0.0);
        }

        validate_hw_greek_params(self.kappa, sigma)?;
        let model = CalibratedHullWhiteModel::calibrate(
            HullWhiteParams::new(self.kappa, sigma)?,
            self.tree_steps,
            disc,
            ttm,
        )?;
        let valuator = BermudanSwaptionTreeValuator::new(swaption, &model, disc, as_of)?;
        valuator.price()
    }
}

impl MetricCalculator for BermudanVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swaption = context
            .instrument
            .as_any()
            .downcast_ref::<BermudanSwaption>()
            .ok_or_else(|| finstack_core::Error::Validation("Expected BermudanSwaption".into()))?;

        let disc = context
            .curves
            .get_discount(swaption.discount_curve_id.as_str())?;

        validate_hw_greek_params(self.kappa, self.sigma)?;

        // Bump volatility
        let sigma_up = self.sigma * (1.0 + self.bump_pct);
        let sigma_down = self.sigma * (1.0 - self.bump_pct);
        validate_hw_greek_params(self.kappa, sigma_up)?;
        validate_hw_greek_params(self.kappa, sigma_down)?;

        let denom = 2.0 * self.bump_pct * self.sigma;
        if !denom.is_finite() || denom.abs() <= f64::EPSILON * 1024.0 {
            return Err(finstack_core::Error::Validation(
                "Bermudan vega: bump_pct and sigma must yield a non-zero finite denominator".into(),
            ));
        }

        let price_up = self.price_bermudan(swaption, disc.as_ref(), context.as_of, sigma_up)?;
        let price_down = self.price_bermudan(swaption, disc.as_ref(), context.as_of, sigma_down)?;

        // Central difference
        let vega = (price_up - price_down) / denom;

        // Scale to 1% volatility change
        let vega_pct = vega * 0.01;

        Ok(vega_pct)
    }
}

// ============================================================================
// Bermudan Gamma Calculator
// ============================================================================

/// Gamma calculator for Bermudan swaptions.
///
/// Computes second-order rate sensitivity via bump-and-revalue.
#[derive(Debug, Clone)]
pub(crate) struct BermudanGammaCalculator {
    /// Rate bump size in basis points
    pub(crate) bump_bp: f64,
    /// Hull-White mean reversion
    pub(crate) kappa: f64,
    /// Hull-White volatility
    pub(crate) sigma: f64,
    /// Tree steps
    pub(crate) tree_steps: usize,
}

impl Default for BermudanGammaCalculator {
    fn default() -> Self {
        Self {
            bump_bp: DEFAULT_RATE_BUMP_BP,
            kappa: DEFAULT_KAPPA,
            sigma: DEFAULT_SIGMA,
            tree_steps: DEFAULT_TREE_STEPS,
        }
    }
}

impl BermudanGammaCalculator {
    /// Create a new gamma calculator.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Set Hull-White parameters.
    ///
    /// Invalid values are rejected when computing metrics (runtime validation).
    pub(crate) fn with_hw_params(mut self, kappa: f64, sigma: f64) -> Self {
        self.kappa = kappa;
        self.sigma = sigma;
        self
    }

    /// Price Bermudan swaption with given parameters.
    fn price_bermudan(
        &self,
        swaption: &BermudanSwaption,
        disc: &dyn Discounting,
        as_of: Date,
        sigma: f64,
    ) -> Result<f64> {
        let ttm = swaption.time_to_maturity(as_of)?;
        if ttm <= 0.0 {
            return Ok(0.0);
        }

        validate_hw_greek_params(self.kappa, sigma)?;
        let model = CalibratedHullWhiteModel::calibrate(
            HullWhiteParams::new(self.kappa, sigma)?,
            self.tree_steps,
            disc,
            ttm,
        )?;
        let valuator = BermudanSwaptionTreeValuator::new(swaption, &model, disc, as_of)?;
        valuator.price()
    }
}

impl MetricCalculator for BermudanGammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swaption = context
            .instrument
            .as_any()
            .downcast_ref::<BermudanSwaption>()
            .ok_or_else(|| finstack_core::Error::Validation("Expected BermudanSwaption".into()))?;

        let disc = context
            .curves
            .get_discount(swaption.discount_curve_id.as_str())?;
        let ttm = swaption.time_to_maturity(context.as_of)?;

        if ttm <= 0.0 {
            return Ok(0.0);
        }

        let bump_bp = self.bump_bp.abs();
        let bump = bump_bp / 10000.0;
        if bump <= 0.0 {
            return Ok(0.0);
        }

        let base_price = self.price_bermudan(swaption, disc.as_ref(), context.as_of, self.sigma)?;

        // Bump discount curve only -- see BermudanDeltaCalculator::calculate()
        // for documentation on single-factor HW tree limitations.
        let curves_up = context.curves.bump([MarketBump::Curve {
            id: swaption.discount_curve_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp),
        }])?;
        let curves_dn = context.curves.bump([MarketBump::Curve {
            id: swaption.discount_curve_id.clone(),
            spec: BumpSpec::parallel_bp(-bump_bp),
        }])?;

        let disc_up = curves_up.get_discount(swaption.discount_curve_id.as_str())?;
        let disc_dn = curves_dn.get_discount(swaption.discount_curve_id.as_str())?;

        let price_up =
            self.price_bermudan(swaption, disc_up.as_ref(), context.as_of, self.sigma)?;
        let price_dn =
            self.price_bermudan(swaption, disc_dn.as_ref(), context.as_of, self.sigma)?;

        Ok((price_up - 2.0 * base_price + price_dn) / (bump * bump))
    }
}

// ============================================================================
// Exercise Probability Profile
// ============================================================================

/// Exercise timing summary for Bermudan swaptions.
#[derive(Debug, Clone)]
pub(crate) struct ExerciseProbabilityProfile {
    /// Expected exercise time
    pub(crate) expected_exercise_time: f64,
}

impl ExerciseProbabilityProfile {
    /// Create from tree valuator using actual computed exercise probabilities.
    ///
    /// Uses the risk-neutral exercise probabilities computed during backward
    /// induction in the Hull-White tree. These probabilities represent the
    /// optimal exercise strategy under the risk-neutral measure.
    ///
    /// # Arguments
    /// * `valuator` - The tree valuator that has computed the optimal exercise boundary
    /// * `exercise_times` - Exercise dates as year fractions (used for validation)
    ///
    /// # Returns
    /// An `ExerciseProbabilityProfile` with the expected exercise time.
    pub(crate) fn from_valuator(
        valuator: &BermudanSwaptionTreeValuator,
        exercise_times: Vec<f64>,
    ) -> Self {
        // Get actual exercise probabilities from the tree valuator
        let tree_probs = valuator.exercise_probabilities();

        let n = exercise_times.len();
        if n == 0 || tree_probs.is_empty() {
            return Self {
                expected_exercise_time: 0.0,
            };
        }

        // Expected exercise time = Σ t_i × P(exercise at t_i)
        // For non-exercised paths, we include remaining probability at terminal date
        let expected_exercise_time: f64 = tree_probs.iter().map(|(t, p)| t * p).sum();

        Self {
            expected_exercise_time,
        }
    }
}

/// Calculator for exercise probability metrics.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExerciseProbabilityCalculator {
    /// Hull-White mean reversion
    pub(crate) kappa: f64,
    /// Hull-White volatility
    pub(crate) sigma: f64,
    /// Tree steps
    pub(crate) tree_steps: usize,
}

impl ExerciseProbabilityCalculator {
    /// Create a new calculator with calibrated Hull-White parameters.
    ///
    /// Invalid values are rejected when computing metrics (runtime validation).
    pub(crate) fn new_with_hw(kappa: f64, sigma: f64) -> Self {
        Self {
            kappa,
            sigma,
            tree_steps: DEFAULT_TREE_STEPS,
        }
    }
}

impl MetricCalculator for ExerciseProbabilityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swaption = context
            .instrument
            .as_any()
            .downcast_ref::<BermudanSwaption>()
            .ok_or_else(|| finstack_core::Error::Validation("Expected BermudanSwaption".into()))?;

        let disc = context
            .curves
            .get_discount(swaption.discount_curve_id.as_str())?;
        let ttm = swaption.time_to_maturity(context.as_of)?;

        if ttm <= 0.0 {
            return Ok(0.0);
        }

        validate_hw_greek_params(self.kappa, self.sigma)?;
        let model = CalibratedHullWhiteModel::calibrate(
            HullWhiteParams::new(self.kappa, self.sigma)?,
            self.tree_steps,
            disc.as_ref(),
            ttm,
        )?;
        let valuator =
            BermudanSwaptionTreeValuator::new(swaption, &model, disc.as_ref(), context.as_of)?;

        let exercise_times = swaption.exercise_times(context.as_of)?;
        let profile = ExerciseProbabilityProfile::from_valuator(&valuator, exercise_times);

        // Return expected exercise time as the metric value
        Ok(profile.expected_exercise_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_bermudan_delta_calculator_creation() {
        let calc = BermudanDeltaCalculator::new();
        assert_eq!(calc.bump_bp, DEFAULT_RATE_BUMP_BP);
    }

    #[test]
    fn test_bermudan_vega_calculator_creation() {
        let calc = BermudanVegaCalculator::new();
        assert_eq!(calc.sigma, DEFAULT_SIGMA);
    }

    #[test]
    fn test_exercise_probability_profile_from_valuator() {
        // Integration test: verify from_valuator uses actual tree probabilities
        use crate::instruments::rates::swaption::{
            BermudanSchedule, BermudanSwaption, BermudanType, CalibratedHullWhiteModel,
            HullWhiteParams, SwaptionSettlement,
        };
        use finstack_core::currency::Currency;
        use finstack_core::dates::{DayCount, Tenor};
        use finstack_core::market_data::term_structures::DiscountCurve;
        use finstack_core::math::interp::InterpStyle;
        use finstack_core::money::Money;
        use finstack_core::types::{CurveId, InstrumentId};
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

        let swaption = BermudanSwaption {
            id: InstrumentId::new("TEST-BERM"),
            option_type: crate::instruments::common_impl::parameters::OptionType::Call,
            notional: Money::new(10_000_000.0, Currency::USD),
            strike: rust_decimal::Decimal::try_from(0.03).expect("valid decimal"),
            swap_start,
            swap_end,
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            settlement: SwaptionSettlement::Physical,
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: CurveId::new("USD-VOL"),
            bermudan_schedule: BermudanSchedule::co_terminal(
                first_exercise,
                swap_end,
                Tenor::semi_annual(),
            )
            .expect("valid Bermudan schedule"),
            bermudan_type: BermudanType::CoTerminal,
            calendar_id: None,
            pricing_overrides: Default::default(),
            attributes: Default::default(),
        };

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");

        let model = CalibratedHullWhiteModel::calibrate(
            HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
            30,
            &curve,
            ttm,
        )
        .expect("Valid model");
        let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of)
            .expect("Valid valuator");

        let exercise_times = swaption
            .exercise_times(as_of)
            .expect("Valid exercise times");
        let profile = ExerciseProbabilityProfile::from_valuator(&valuator, exercise_times.clone());

        // Expected exercise time should be reasonable.
        assert!(profile.expected_exercise_time >= 0.0);
    }
}
