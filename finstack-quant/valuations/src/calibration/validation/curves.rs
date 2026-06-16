//! Curve validators (discount/forward/hazard/inflation/base-correlation).

use crate::calibration::validation::points::{
    DF_ARBI_POINTS, DF_BOUNDS_POINTS, DF_MONO_POINTS, FWD_ARBI_POINTS, FWD_BOUNDS_POINTS,
    HAZARD_ARBI_POINTS, HAZARD_BOUNDS_POINTS, HAZARD_MONO_POINTS, INFL_ARBI_POINTS,
    INFL_BOUNDS_POINTS, INFL_MONO_POINTS,
};
use crate::calibration::validation::ValidationConfig;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, DiscountCurve, ForwardCurve, HazardCurve, InflationCurve,
};
use finstack_quant_core::{Error, Result};

/// Core validation trait for term structures.
///
/// Implementations of this trait provide standard financial sanity checks
/// for discount, forward, hazard, and inflation curves. These checks
/// ensure that calibrated curves are economically meaningful and
/// arbitrage-free.
pub trait CurveValidator {
    /// Validate that the curve satisfies no-arbitrage constraints.
    ///
    /// For discount curves, this verifies forward rate positivity.
    /// For hazard curves, it verifies survival probability consistency.
    fn validate_no_arbitrage(&self, config: &ValidationConfig) -> Result<()>;

    /// Validate monotonicity constraints.
    ///
    /// Ensures discount factors/survival probabilities are monotonically
    /// decreasing with time (in positive rate environments).
    fn validate_monotonicity(&self, config: &ValidationConfig) -> Result<()>;

    /// Validate that all values are within reasonable financial bounds.
    ///
    /// Checks for extreme rates (hyperinflation, deep negative rates)
    /// or non-physical probabilities.
    fn validate_bounds(&self, config: &ValidationConfig) -> Result<()>;

    /// Run all validations defined in this trait.
    fn validate(&self, config: &ValidationConfig) -> Result<()> {
        self.validate_no_arbitrage(config)?;
        self.validate_monotonicity(config)?;
        self.validate_bounds(config)?;
        Ok(())
    }
}

/// Validation for discount curves.
impl CurveValidator for DiscountCurve {
    fn validate_no_arbitrage(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_arbitrage {
            return Ok(());
        }

        let max_knot_arbi = self.knots().last().copied().unwrap_or(0.0);

        for i in 0..DF_ARBI_POINTS.len() - 1 {
            let t1 = DF_ARBI_POINTS[i];
            let t2 = DF_ARBI_POINTS[i + 1];
            if t2 > max_knot_arbi + 0.01 {
                break;
            }

            let df1 = self.df(t1);
            let df2 = self.df(t2);

            // Calculate instantaneous forward rate using continuous compounding.
            // The simple-compounded form (df1/df2 - 1)/(t2-t1) is wildly imprecise
            // on the coarse DF_ARBI_POINTS grid (gaps up to 10 years); at a 10y
            // gap and 5% rates it overestimates by ~30% relative, producing false
            // "unreasonably high" rejections or masking real arbitrage.
            // The guard `df2 < df1` ensures `df2/df1 < 1`, so `ln(df2/df1) < 0`
            // and the negation produces a positive forward rate.
            if df1 > 0.0 && df2 > 0.0 && df2 < df1 {
                let fwd_rate = -(df2 / df1).ln() / (t2 - t1);

                // Forward rates should be positive (allowing small negative for technical reasons)
                if fwd_rate < config.min_forward_rate {
                    return Err(Error::Validation(format!(
                        "Negative forward rate {:.4}% between t={} and t={} in {} (limit: {:.2}%)",
                        fwd_rate * 100.0,
                        t1,
                        t2,
                        self.id().as_str(),
                        config.min_forward_rate * 100.0
                    )));
                }

                // Forward rates shouldn't be unreasonably high
                if fwd_rate > config.max_forward_rate {
                    return Err(Error::Validation(format!(
                        "Unreasonably high forward rate {:.2}% between t={} and t={} in {} (limit: {:.2}%)",
                        fwd_rate * 100.0,
                        t1,
                        t2,
                        self.id().as_str(),
                        config.max_forward_rate * 100.0
                    )));
                }
            }
        }

        Ok(())
    }

    fn validate_monotonicity(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_monotonicity {
            return Ok(());
        }

        // Per-segment monotonicity: rather than a single "skip everything if 3M
        // is negative" heuristic, walk DF_MONO_POINTS and only allow DF to rise
        // (df > prev_df) on segments where the segment's own continuously-
        // compounded forward rate is negative. This correctly handles mixed
        // regimes (e.g., EUR 2017: -0.5% short, +2% long), which the previous
        // single-3M-point check treated as fully relaxed.
        let mut prev_df = 1.0;
        let mut prev_t = 0.0;
        let max_knot = self.knots().last().copied().unwrap_or(0.0);

        for &t in DF_MONO_POINTS {
            if t > max_knot + 0.01 {
                break;
            }
            let df = self.df(t);

            // Compute this segment's continuously-compounded forward rate.
            // For t == prev_t (the t=0 point), no segment exists yet.
            let segment_negative = if t > prev_t && prev_df > 0.0 && df > 0.0 {
                let fwd = -(df / prev_df).ln() / (t - prev_t);
                fwd < -config.tolerance
            } else {
                false
            };

            // Allow DF to rise only when (a) caller has opted into negative
            // rates, AND (b) this specific segment's forward rate is genuinely
            // negative. Otherwise enforce monotonic decrease.
            let allow_rise = config.allow_negative_rates && segment_negative;

            if !allow_rise && df > prev_df + config.tolerance {
                return Err(Error::Validation(format!(
                    "Discount factor not monotonically decreasing: DF({})={:.6} > DF(prev)={:.6} in {} \
                    (segment forward rate is non-negative; set allow_negative_rates=true and \
                    ensure the segment forward is genuinely negative if this is intentional)",
                    t,
                    df,
                    prev_df,
                    self.id().as_str()
                )));
            }

            prev_df = df;
            prev_t = t;
        }

        Ok(())
    }

    fn validate_bounds(&self, config: &ValidationConfig) -> Result<()> {
        // Check that discount factors are in (0, max_df]
        // For negative rate environments, DF can exceed 1.0 at short end
        let max_df = if config.allow_negative_rates {
            1.5
        } else {
            1.0
        };

        let max_knot_bounds = self.knots().last().copied().unwrap_or(0.0);

        for &t in DF_BOUNDS_POINTS {
            if t > max_knot_bounds + 0.01 {
                break;
            }
            let df = self.df(t);

            if df <= 0.0 {
                return Err(Error::Validation(format!(
                    "Non-positive discount factor {:.6} at t={} in {}",
                    df,
                    t,
                    self.id().as_str()
                )));
            }

            if df > max_df {
                return Err(Error::Validation(format!(
                    "Discount factor {:.6} exceeds {:.1} at t={} in {} (max DF for {} rate environment)",
                    df,
                    max_df,
                    t,
                    self.id().as_str(),
                    if config.allow_negative_rates {
                        "negative"
                    } else {
                        "positive"
                    }
                )));
            }
        }

        for &t in DF_BOUNDS_POINTS {
            if t > max_knot_bounds + 0.01 {
                break;
            }
            let rate = self.zero(t);

            // Allow slightly negative rates but not too extreme
            if rate < config.min_forward_rate * 5.0 {
                // e.g. -5% if min_forward is -1%
                return Err(Error::Validation(format!(
                    "Zero rate {:.2}% too negative at t={} in {}",
                    rate * 100.0,
                    t,
                    self.id().as_str()
                )));
            }

            // Cap at reasonable maximum
            if rate > config.max_forward_rate {
                return Err(Error::Validation(format!(
                    "Zero rate {:.2}% too high at t={} in {}",
                    rate * 100.0,
                    t,
                    self.id().as_str()
                )));
            }
        }

        Ok(())
    }
}

/// Validation for forward curves.
impl CurveValidator for ForwardCurve {
    fn validate_no_arbitrage(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_arbitrage && !config.check_forward_positivity {
            return Ok(());
        }

        // Forward rates should be positive (with small tolerance for negative rates)
        for &t in FWD_ARBI_POINTS {
            let fwd = self.rate(t);

            if fwd < config.min_forward_rate {
                return Err(Error::Validation(format!(
                    "Negative forward rate {:.4}% at t={} in {} (limit: {:.2}%)",
                    fwd * 100.0,
                    t,
                    self.id().as_str(),
                    config.min_forward_rate * 100.0
                )));
            }
        }

        Ok(())
    }

    fn validate_monotonicity(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_monotonicity {
            return Ok(());
        }

        // Forward curves don't have strict monotonicity requirements
        // but we check for reasonable smoothness
        let test_points = [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];

        for i in 0..test_points.len() - 1 {
            let t1 = test_points[i];
            let t2 = test_points[i + 1];

            let fwd1 = self.rate(t1);
            let fwd2 = self.rate(t2);

            // Check for unreasonable jumps (more than 10% absolute change)
            let jump = (fwd2 - fwd1).abs();
            if jump > 0.10 {
                // This is a warning, not necessarily an error
                tracing::warn!(
                    "Large forward rate jump of {:.2}% between t={} and t={} in {}",
                    jump * 100.0,
                    t1,
                    t2,
                    self.id().as_str()
                );
            }
        }

        Ok(())
    }

    fn validate_bounds(&self, config: &ValidationConfig) -> Result<()> {
        for &t in FWD_BOUNDS_POINTS {
            let rate = self.rate(t);

            // Allow slightly negative but bounded
            if rate < config.min_forward_rate * 5.0 {
                return Err(Error::Validation(format!(
                    "Forward rate {:.2}% too negative at t={} in {}",
                    rate * 100.0,
                    t,
                    self.id().as_str()
                )));
            }

            // Cap at reasonable maximum
            if rate > config.max_forward_rate {
                return Err(Error::Validation(format!(
                    "Forward rate {:.2}% too high at t={} in {}",
                    rate * 100.0,
                    t,
                    self.id().as_str()
                )));
            }
        }

        Ok(())
    }
}

/// Validation for hazard curves.
impl CurveValidator for HazardCurve {
    fn validate_no_arbitrage(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_arbitrage {
            return Ok(());
        }

        // Check hazard rates are non-negative using survival probability
        for &t in HAZARD_ARBI_POINTS {
            // Use the curve's native hazard_rate(t) method directly
            // instead of finite-difference approximation.
            let lambda = self.hazard_rate(t);

            if lambda < 0.0 {
                return Err(Error::Validation(format!(
                    "Negative hazard rate {:.4} at t={} in {}",
                    lambda,
                    t,
                    self.id().as_str()
                )));
            }

            // Check for unreasonably high hazard rates (>50% instantaneous default prob)
            if lambda > config.max_hazard_rate {
                return Err(Error::Validation(format!(
                    "Unreasonably high hazard rate {:.2} at t={} in {} (limit: {:.2})",
                    lambda,
                    t,
                    self.id().as_str(),
                    config.max_hazard_rate
                )));
            }
        }

        Ok(())
    }

    fn validate_monotonicity(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_monotonicity {
            return Ok(());
        }

        // Survival probabilities must be monotonically decreasing
        let mut prev_sp = 1.0;

        for &t in HAZARD_MONO_POINTS {
            let sp = self.sp(t);

            // Allow for numerical tolerance
            if sp > prev_sp + config.tolerance {
                return Err(Error::Validation(format!(
                    "Survival probability not monotonically decreasing: SP({})={:.6} > SP(prev)={:.6} in {}",
                    t,
                    sp,
                    prev_sp,
                    self.id().as_str()
                )));
            }

            prev_sp = sp;
        }

        Ok(())
    }

    fn validate_bounds(&self, config: &ValidationConfig) -> Result<()> {
        // Check that survival probabilities are in [0, 1]
        for &t in HAZARD_BOUNDS_POINTS {
            let sp = self.sp(t);

            if sp < 0.0 {
                return Err(Error::Validation(format!(
                    "Negative survival probability {:.6} at t={} in {}",
                    sp,
                    t,
                    self.id().as_str()
                )));
            }

            if sp > 1.0 {
                return Err(Error::Validation(format!(
                    "Survival probability {:.6} exceeds 1.0 at t={} in {}",
                    sp,
                    t,
                    self.id().as_str()
                )));
            }
        }

        // Validate recovery rate
        let recovery = self.recovery_rate();
        if !(0.0..=1.0).contains(&recovery) {
            return Err(Error::Validation(format!(
                "Recovery rate {:.2}% outside [0, 100%] range in {}",
                recovery * 100.0,
                self.id().as_str()
            )));
        }

        // (extra) bounds: keep behavior consistent with previous module
        let _ = config;
        Ok(())
    }
}

/// Validation for inflation curves.
impl CurveValidator for InflationCurve {
    fn validate_no_arbitrage(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_arbitrage {
            return Ok(());
        }

        // CPI levels should be positive
        for &t in INFL_ARBI_POINTS {
            let cpi = self.cpi(t);

            if cpi <= 0.0 {
                return Err(Error::Validation(format!(
                    "Non-positive CPI level {:.2} at t={} in {}",
                    cpi,
                    t,
                    self.id().as_str()
                )));
            }
        }

        Ok(())
    }

    fn validate_monotonicity(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_monotonicity {
            return Ok(());
        }

        // CPI doesn't need to be strictly monotonic (deflation is possible)
        // but check for reasonable growth rates
        let base_cpi = self.cpi(0.0);

        for &t in INFL_MONO_POINTS {
            let cpi = self.cpi(t);
            let annual_inflation = (cpi / base_cpi).powf(1.0 / t) - 1.0;

            // Check for extreme deflation (< -10% annual)
            if annual_inflation < config.min_cpi_growth {
                return Err(Error::Validation(format!(
                    "Extreme deflation {:.2}% per year over {} years in {} (limit: {:.2}%)",
                    annual_inflation * 100.0,
                    t,
                    self.id().as_str(),
                    config.min_cpi_growth * 100.0
                )));
            }

            // Check for hyperinflation (> 50% annual)
            if annual_inflation > config.max_cpi_growth {
                return Err(Error::Validation(format!(
                    "Hyperinflation {:.2}% per year over {} years in {} (limit: {:.2}%)",
                    annual_inflation * 100.0,
                    t,
                    self.id().as_str(),
                    config.max_cpi_growth * 100.0
                )));
            }
        }

        Ok(())
    }

    fn validate_bounds(&self, config: &ValidationConfig) -> Result<()> {
        // Check reasonable inflation expectations
        for &t in INFL_BOUNDS_POINTS {
            // Calculate forward inflation over 1-year period
            let cpi_t = self.cpi(t);
            let cpi_t1 = self.cpi(t + 1.0);
            let fwd_inflation = cpi_t1 / cpi_t - 1.0;

            // Forward inflation should be in reasonable range
            if !(config.min_fwd_inflation..=config.max_fwd_inflation).contains(&fwd_inflation) {
                return Err(Error::Validation(format!(
                    "Forward inflation {:.2}% outside reasonable range [{:.2}%, {:.2}%] at t={} in {}",
                    fwd_inflation * 100.0,
                    config.min_fwd_inflation * 100.0,
                    config.max_fwd_inflation * 100.0,
                    t,
                    self.id().as_str()
                )));
            }
        }

        Ok(())
    }
}

/// Validation for base correlation curves.
impl CurveValidator for BaseCorrelationCurve {
    fn validate_no_arbitrage(&self, config: &ValidationConfig) -> Result<()> {
        if !config.check_arbitrage {
            return Ok(());
        }

        // Base correlations should be monotonically increasing with detachment
        let detachment_points = self.detachment_points();
        let correlations = self.correlations();

        if detachment_points.len() != correlations.len() {
            return Err(Error::Validation(format!(
                "Detachment points and correlations length mismatch in {}",
                self.id().as_str()
            )));
        }

        // Check monotonicity
        for i in 1..correlations.len() {
            if correlations[i] < correlations[i - 1] - config.tolerance {
                return Err(Error::Validation(format!(
                    "Base correlation not monotonically increasing: corr({:.1}%)={:.2}% < corr({:.1}%)={:.2}% in {}",
                    detachment_points[i],
                    correlations[i] * 100.0,
                    detachment_points[i - 1],
                    correlations[i - 1] * 100.0,
                    self.id().as_str()
                )));
            }
        }

        Ok(())
    }

    fn validate_monotonicity(&self, _config: &ValidationConfig) -> Result<()> {
        // Already checked in validate_no_arbitrage
        Ok(())
    }

    fn validate_bounds(&self, _config: &ValidationConfig) -> Result<()> {
        // Correlations must be in [0, 1]
        for (detach, corr) in self
            .detachment_points()
            .iter()
            .zip(self.correlations().iter())
        {
            if *corr < 0.0 || *corr > 1.0 {
                return Err(Error::Validation(format!(
                    "Base correlation {:.2}% at {:.1}% detachment outside [0, 100%] range in {}",
                    corr * 100.0,
                    detach,
                    self.id().as_str()
                )));
            }
        }

        Ok(())
    }
}
