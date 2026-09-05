//! Bermudan swaption pricer using LMM/BGM Monte Carlo dynamics.
//!
//! Wraps the standalone `price_bermudan_lmm` engine in the `Pricer` trait
//! so it can be dispatched via the pricing registry under
//! `(BermudanSwaption, LmmMonteCarlo)`.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::hw1f::RateExoticMcConfig;
use crate::instruments::rates::swaption::pricing::lmm_bermudan::price_bermudan_lmm;
use crate::instruments::rates::swaption::BermudanSwaption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::process::lmm::LmmParams;

/// Bermudan swaption pricer using LMM/BGM Monte Carlo with LSMC exercise.
///
/// Builds [`LmmParams`] from the swaption's canonical underlying fixed-leg
/// schedule and projection/discount curves, then delegates to `price_bermudan_lmm` for
/// LSMC-based Bermudan exercise valuation.
///
/// # Parameter Construction
///
/// Forward rates come from the floating leg's projection curve when it differs
/// from the discount curve; single-curve instruments use discount-implied
/// forwards. A flat 2-factor loading structure is used
/// (a linear-decay proxy for the first two principal components of the
/// forward-rate correlation matrix). The *shape* of the loadings is fixed,
/// and their overall scale comes exclusively from the positive, finite
/// `model_config.lmm_base_vol` input. Calibration is an explicit upstream
/// operation; this pricer never queries a volatility surface.
pub struct BermudanSwaptionLmmPricer {
    config: RateExoticMcConfig,
}

impl Default for BermudanSwaptionLmmPricer {
    fn default() -> Self {
        Self::with_config(RateExoticMcConfig::lmm_bermudan())
    }
}

impl BermudanSwaptionLmmPricer {
    /// Create a pricer with an explicit configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Monte Carlo settings; see `price_bermudan_lmm` for the
    ///   path-count constraints. [`RateExoticMcConfig::lmm_bermudan`] gives
    ///   the registry defaults.
    pub fn with_config(config: RateExoticMcConfig) -> Self {
        Self { config }
    }

    /// Build the canonical Bermudan LMM structure with an explicit loading scale.
    ///
    /// # Arguments
    ///
    /// * `swaption` - Bermudan contract whose fixed schedule and curve roles define the tenor model.
    /// * `disc` - Discount curve used for single-curve forward initialization.
    /// * `market` - Market supplying the optional projection curve.
    /// * `as_of` - Valuation date used for year-fraction coordinates.
    /// * `base_vol` - Positive finite annualized decimal loading scale.
    ///
    /// # Errors
    ///
    /// Returns a pricing error for invalid schedules, missing curves, invalid
    /// forwards, or a non-positive/non-finite loading scale.
    pub fn build_lmm_params(
        swaption: &BermudanSwaption,
        disc: &dyn Discounting,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
        base_vol: f64,
    ) -> std::result::Result<LmmParams, PricingError> {
        if !base_vol.is_finite() || base_vol <= 0.0 {
            return Err(PricingError::model_failure_with_context(
                format!("LMM base_vol must be positive and finite, got {base_vol}"),
                PricingErrorContext::default(),
            ));
        }
        let periods = swaption.fixed_schedule_periods().map_err(|e| {
            PricingError::model_failure_with_context(
                format!("LMM tenor schedule construction failed: {e}"),
                PricingErrorContext::default(),
            )
        })?;
        let Some(first_period) = periods.first() else {
            return Err(PricingError::model_failure_with_context(
                "LMM requires at least one fixed-leg schedule period".to_string(),
                PricingErrorContext::default(),
            ));
        };
        let mut tenor_dates = Vec::with_capacity(periods.len() + 1);
        tenor_dates.push(first_period.accrual_start);
        tenor_dates.extend(periods.iter().map(|period| period.accrual_end));

        let tenors: Vec<f64> = tenor_dates
            .iter()
            .map(|&date| year_fraction(swaption.get_day_count(), as_of, date))
            .collect::<finstack_quant_core::Result<Vec<_>>>()
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let num_forwards = tenors.len() - 1;
        if num_forwards == 0 {
            return Err(PricingError::model_failure_with_context(
                "LMM requires at least one forward rate period".to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Contractual accruals come from the canonical fixed-leg schedule and
        // therefore retain its day count, stubs, EOM rule, and calendar policy.
        let accrual_factors = periods
            .iter()
            .map(|period| period.accrual_year_fraction)
            .collect::<Vec<_>>();

        let projection = if swaption.get_forward_curve_id() == swaption.get_discount_curve_id() {
            None
        } else {
            Some(
                market
                    .get_forward(swaption.get_forward_curve_id().as_ref())
                    .map_err(|e| {
                        PricingError::missing_market_data_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?,
            )
        };

        // Initialize each LMM tenor forward from its canonical market role.
        let mut initial_forwards: Vec<f64> = Vec::with_capacity(num_forwards);
        for i in 0..num_forwards {
            let tau = accrual_factors[i];
            if !tau.is_finite() || tau <= 0.0 {
                return Err(PricingError::model_failure_with_context(
                    format!("LMM schedule has invalid accrual in period {i}: {tau}"),
                    PricingErrorContext::default(),
                ));
            }
            let fwd = if let Some(projection) = projection.as_deref() {
                crate::instruments::common_impl::pricing::time::rate_between_on_dates(
                    projection,
                    tenor_dates[i],
                    tenor_dates[i + 1],
                )
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?
            } else {
                let df_start = disc.df_between_dates(as_of, tenor_dates[i]).map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
                let df_end = disc
                    .df_between_dates(as_of, tenor_dates[i + 1])
                    .map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
                if !df_start.is_finite() || !df_end.is_finite() || df_start <= 0.0 || df_end <= 0.0
                {
                    return Err(PricingError::model_failure_with_context(
                        format!(
                            "LMM forward bootstrap has invalid discount factors in period {i}: \
                             df_start={df_start}, df_end={df_end}"
                        ),
                        PricingErrorContext::default(),
                    ));
                }
                (df_start / df_end - 1.0) / tau
            };
            if !fwd.is_finite() {
                return Err(PricingError::model_failure_with_context(
                    format!("LMM forward initialization is non-finite in period {i}: {fwd}"),
                    PricingErrorContext::default(),
                ));
            }
            initial_forwards.push(fwd);
        }

        // Displacement (shifted-lognormal shift). A small positive shift is
        // needed only when forwards can approach or cross zero; for a
        // comfortably-positive curve a pure lognormal model (zero shift) is
        // consistent with the lognormal Black swaption surface the
        // calibration targets. Pick the shift from the realised forwards
        // instead of hardcoding a magic constant.
        let min_forward = initial_forwards
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let shift = if min_forward > 0.01 {
            0.0
        } else {
            // Lift the most negative/near-zero forward to a +1% effective
            // floor so the displaced-lognormal diffusion stays well posed.
            (0.01 - min_forward).max(0.0)
        };
        let displacements = vec![shift; num_forwards];

        // Flat 2-factor loading structure with linear decay (the *shape*):
        //   ĝ_i = [1 - alpha * i/N, alpha * i/N, 0]
        // This approximates the first two principal components of swaption
        // correlation matrices. The full loading is `lambda_i = base_vol * ĝ_i`.
        let alpha = 0.4; // decay parameter; scale is supplied explicitly
        let loading_shapes: Vec<[f64; 3]> = (0..num_forwards)
            .map(|i| {
                let frac = i as f64 / num_forwards.max(1) as f64;
                [1.0 - alpha * frac, alpha * frac, 0.0]
            })
            .collect();

        let vol_row: Vec<[f64; 3]> = loading_shapes
            .iter()
            .map(|g| [base_vol * g[0], base_vol * g[1], base_vol * g[2]])
            .collect();
        let vol_values = vec![vol_row]; // single vol period (no breakpoints)
        let vol_times: Vec<f64> = vec![]; // empty => single period

        LmmParams {
            num_forwards,
            num_factors: 2,
            tenors,
            accrual_factors,
            displacements,
            vol_times,
            vol_values,
            initial_forwards,
        }
        .validate()
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })
    }
}

impl Pricer for BermudanSwaptionLmmPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BermudanSwaption, ModelKey::LmmMonteCarlo)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let swaption = crate::pricer::expect_inst::<BermudanSwaption>(
            instrument,
            InstrumentType::BermudanSwaption,
        )?;

        let disc = market
            .get_discount(swaption.get_discount_curve_id().as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let ttm = swaption.time_to_maturity(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        if ttm <= 0.0 {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::from((0_i64, swaption.notional.currency())),
            ));
        }

        let base_vol = swaption
            .instrument_pricing_overrides
            .model_config
            .lmm_base_vol
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| {
                PricingError::model_failure_with_context(
                    format!(
                        "Bermudan swaption '{}' requires positive finite \
                         pricing_overrides.model_config.lmm_base_vol; calibrate it upstream",
                        swaption.id
                    ),
                    PricingErrorContext::default(),
                )
            })?;
        let lmm_params = Self::build_lmm_params(swaption, disc.as_ref(), market, as_of, base_vol)?;

        let exercise_times = swaption
            .bermudan_schedule
            .exercise_times(as_of, swaption.get_day_count())
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if exercise_times.is_empty() {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::from((0_i64, swaption.notional.currency())),
            ));
        }

        // Strike and payer/receiver flag
        let strike = swaption.strike_f64().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let is_payer =
            swaption.option_type == crate::instruments::common_impl::parameters::OptionType::Call;
        let notional = swaption.notional.amount();
        let currency = swaption.notional.currency();

        // Terminal discount factor P(0, T_N) for the last tenor
        let df_terminal = disc
            .df_between_dates(as_of, swaption.get_swap_end())
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Price via LSMC with LMM dynamics
        let estimate = price_bermudan_lmm(
            &lmm_params,
            &exercise_times,
            strike,
            is_payer,
            notional,
            df_terminal,
            currency,
            &self.config,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let mut result = ValuationResult::stamped(swaption.id.as_str(), as_of, estimate.mean);
        if estimate.stderr > 0.0 {
            result.measures.insert(
                crate::metrics::MetricId::custom("mc_stderr"),
                estimate.stderr,
            );
        }
        Ok(result)
    }
}
