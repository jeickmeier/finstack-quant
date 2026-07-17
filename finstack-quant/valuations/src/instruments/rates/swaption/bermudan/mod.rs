//! Bermudan swaption pricer implementations.

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::exotics_shared::{
    resolve_hw1f_params, Hw1fCalibrationFlavor, Hw1fParamSource, Hw1fResolveRequest,
};
use crate::instruments::rates::swaption::pricing::BermudanSwaptionTreeValuator;
use crate::instruments::rates::swaption::BermudanSwaption;
use crate::models::trees::HullWhiteTree;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use std::sync::Arc;

fn hw1f_overrides_json(swaption: &BermudanSwaption) -> Option<serde_json::Value> {
    let kappa = swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion?;
    let sigma = swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma?;
    Some(serde_json::json!({ "hw1f_kappa": kappa, "hw1f_sigma": sigma }))
}

// LSMC imports (gated by feature)
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::rates::swaption::pricing::monte_carlo_lsmc::{
    SwaptionLsmcConfig, SwaptionLsmcPricer as SharedSwaptionLsmcPricer,
};
use crate::instruments::rates::swaption::pricing::monte_carlo_payoff::{
    BermudanSwaptionPayoff, SwapSchedule, SwaptionType,
};
use finstack_quant_monte_carlo::pricer::basis::PolynomialBasis;
use finstack_quant_monte_carlo::process::ou::{calibrate_theta_from_curve, HullWhite1FProcess};

/// Pricing method for Bermudan swaptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BermudanPricingMethod {
    /// Hull-White trinomial tree (industry standard, faster)
    #[default]
    HullWhiteTree,
    /// Longstaff-Schwartz Monte Carlo (more flexible)
    LSMC,
}

impl std::fmt::Display for BermudanPricingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BermudanPricingMethod::HullWhiteTree => write!(f, "hull_white_tree"),
            BermudanPricingMethod::LSMC => write!(f, "lsmc"),
        }
    }
}

impl std::str::FromStr for BermudanPricingMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "hull_white_tree" | "tree" | "hw" => Ok(Self::HullWhiteTree),
            "lsmc" | "monte_carlo" | "mc" => Ok(Self::LSMC),
            other => Err(format!(
                "Unknown Bermudan pricing method: '{}'. Valid: hull_white_tree, lsmc",
                other
            )),
        }
    }
}

/// Opaque calibrated Hull-White model for Bermudan swaption pricing.
#[derive(Debug, Clone)]
pub struct CalibratedHullWhiteModel {
    tree: Arc<HullWhiteTree>,
}

impl CalibratedHullWhiteModel {
    /// Calibrate a Hull-White tree model from a discount curve and horizon.
    pub fn calibrate(
        params: HullWhiteParams,
        steps: usize,
        disc: &dyn Discounting,
        ttm: f64,
    ) -> std::result::Result<Self, PricingError> {
        Self::calibrate_with_times(params, steps, disc, ttm, &[])
    }

    /// Calibrate a Hull-White tree model whose grid passes exactly through
    /// the supplied mandatory times (e.g. Bermudan exercise dates), so
    /// exercise decisions land on grid points instead of nearest-step
    /// approximations.
    pub fn calibrate_with_times(
        params: HullWhiteParams,
        steps: usize,
        disc: &dyn Discounting,
        ttm: f64,
        mandatory_times: &[f64],
    ) -> std::result::Result<Self, PricingError> {
        if steps == 0 {
            return Err(PricingError::model_failure_with_context(
                "Tree steps must be positive".to_string(),
                PricingErrorContext::default(),
            ));
        }
        let config = params.tree_config(steps);
        let tree = HullWhiteTree::calibrate_with_times(config, disc, ttm, mandatory_times)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
        Ok(Self {
            tree: Arc::new(tree),
        })
    }

    pub(crate) fn tree(&self) -> &Arc<HullWhiteTree> {
        &self.tree
    }
}

/// Pricer for Bermudan swaptions using Hull-White tree or LSMC.
///
/// # Model Reuse
///
/// For portfolio pricing, calibrate the Hull-White model once and reuse it
/// across multiple instruments by putting the calibrated tree on
/// [`BermudanSwaptionPricerConfig`]:
///
/// ```text
/// use finstack_quant_valuations::instruments::rates::swaption::{
///     BermudanSwaptionPricer, BermudanSwaptionPricerConfig, HullWhiteParams,
/// };
/// use finstack_quant_valuations::instruments::rates::swaption::CalibratedHullWhiteModel;
/// use finstack_quant_core::market_data::traits::Discounting;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// // Calibrate once (discount curve and horizon omitted here)
/// # let disc: &dyn Discounting = todo!("provide a discount curve from MarketContext");
/// let ttm = 5.0;
/// let tree = CalibratedHullWhiteModel::calibrate(
///     HullWhiteParams::default(),
///     100,
///     disc,
///     ttm,
/// )?;
///
/// // Reuse across many instruments
/// let pricer = BermudanSwaptionPricer::tree_with_config(BermudanSwaptionPricerConfig {
///     pre_calibrated_model: Some(tree.clone()),
///     ..Default::default()
/// });
/// # let _ = pricer;
/// # Ok(())
/// # }
/// ```
///
/// # Example
///
/// ```text
/// use finstack_quant_valuations::instruments::rates::swaption::{
///     BermudanSwaptionPricer, BermudanPricingMethod,
/// };
///
/// // Create tree-based pricer with default parameters
/// let pricer = BermudanSwaptionPricer::tree();
///
/// // Create LSMC pricer
/// let lsmc_pricer = BermudanSwaptionPricer::lsmc();
/// ```
pub struct BermudanSwaptionPricer {
    /// Pricing method
    method: BermudanPricingMethod,
    /// Pricer configuration.
    config: BermudanSwaptionPricerConfig,
}

/// Configuration for Bermudan swaption Hull-White tree and LSMC pricers.
#[derive(Debug, Clone)]
pub struct BermudanSwaptionPricerConfig {
    /// Hull-White model parameters.
    pub hw_params: HullWhiteParams,
    /// Number of tree steps for Hull-White tree pricing.
    pub tree_steps: usize,
    /// Number of Monte Carlo paths for LSMC pricing.
    pub mc_paths: usize,
    /// Random seed for LSMC pricing.
    pub mc_seed: u64,
    /// Pre-calibrated Hull-White tree for model reuse.
    ///
    /// When set, the tree pricer skips calibration and uses this model
    /// directly. This enables O(1) pricing per instrument instead of
    /// O(Steps × Time) calibration.
    pub pre_calibrated_model: Option<CalibratedHullWhiteModel>,
    /// When true, refuse to price with uncalibrated default HW parameters.
    ///
    /// The pricer registry (`finstack_quant_valuations::pricer::exotics`) sets
    /// this on registered Bermudan pricers so callers reaching the registry
    /// with uncalibrated params receive a clear error rather than a
    /// silently-wrong price. Direct constructor callers retain the
    /// permissive default (`false`) for testing and bespoke workflows.
    pub enforce_calibration: bool,
}

impl BermudanSwaptionPricerConfig {
    /// Default number of Hull-White tree steps.
    pub const DEFAULT_TREE_STEPS: usize = 100;
    /// Default number of Monte Carlo paths.
    ///
    /// 100,000 paths balances accuracy and performance for typical Bermudan
    /// swaptions. For production pricing requiring tight standard errors
    /// (<0.05% of option value), increase to 500,000 paths.
    pub const DEFAULT_MC_PATHS: usize = 100_000;
    /// Default Monte Carlo random seed.
    pub const DEFAULT_MC_SEED: u64 = 42;
}

impl Default for BermudanSwaptionPricerConfig {
    fn default() -> Self {
        Self {
            hw_params: HullWhiteParams::default(),
            tree_steps: Self::DEFAULT_TREE_STEPS,
            mc_paths: Self::DEFAULT_MC_PATHS,
            mc_seed: Self::DEFAULT_MC_SEED,
            pre_calibrated_model: None,
            enforce_calibration: false,
        }
    }
}

impl BermudanSwaptionPricer {
    /// Default number of Monte Carlo paths.
    ///
    /// 100,000 paths balances accuracy and performance for typical Bermudan
    /// swaptions. For production pricing requiring tight standard errors
    /// (<0.05% of option value), increase to 500,000 paths.
    pub const DEFAULT_MC_PATHS: usize = BermudanSwaptionPricerConfig::DEFAULT_MC_PATHS;

    /// Create a Hull-White tree pricer with default configuration.
    pub fn tree() -> Self {
        Self::tree_with_config(BermudanSwaptionPricerConfig::default())
    }

    /// Create an LSMC pricer with default configuration.
    pub fn lsmc() -> Self {
        Self::lsmc_with_config(BermudanSwaptionPricerConfig::default())
    }

    /// Create a Hull-White tree pricer with explicit configuration.
    ///
    /// Set `pre_calibrated_model` on the config to reuse a calibrated
    /// Hull-White tree across a portfolio.
    pub fn tree_with_config(config: BermudanSwaptionPricerConfig) -> Self {
        Self {
            method: BermudanPricingMethod::HullWhiteTree,
            config,
        }
    }

    /// Create an LSMC pricer with explicit configuration.
    ///
    /// The default config uses 100,000 paths. For 10M notional Bermudan
    /// swaptions, this typically produces standard errors of ~0.1-0.5% of the
    /// option value. Increase to 500,000 paths for production-grade accuracy
    /// (<0.05% SE).
    pub fn lsmc_with_config(config: BermudanSwaptionPricerConfig) -> Self {
        Self {
            method: BermudanPricingMethod::LSMC,
            config,
        }
    }

    /// Get the pre-calibrated model, if set.
    pub fn calibrated_model(&self) -> Option<&CalibratedHullWhiteModel> {
        self.config.pre_calibrated_model.as_ref()
    }

    fn effective_tree_steps(&self, swaption: &BermudanSwaption) -> usize {
        swaption
            .instrument_pricing_overrides
            .model_config
            .tree_steps
            .unwrap_or(self.config.tree_steps)
    }

    fn effective_mc_paths(&self, swaption: &BermudanSwaption) -> usize {
        swaption
            .instrument_pricing_overrides
            .model_config
            .mc_paths
            .unwrap_or(self.config.mc_paths)
    }

    fn effective_hw_params(
        &self,
        swaption: &BermudanSwaption,
        market: &MarketContext,
        ttm: f64,
    ) -> std::result::Result<(HullWhiteParams, Hw1fParamSource), PricingError> {
        let context_label = format!("BermudanSwaption {}", swaption.id);
        let overrides = hw1f_overrides_json(swaption);
        let surface = super::hw1f_swaption_surface_calibration(
            swaption.vol_surface_id.as_str(),
            Some(ttm),
            swaption.underlying_fixed_leg.frequency,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let req = Hw1fResolveRequest {
            curve_id: swaption.get_discount_curve_id().as_str(),
            flavor: Hw1fCalibrationFlavor::Swaption,
            overrides: overrides.as_ref(),
            surface: Some(surface),
            fallback: Some(self.config.hw_params),
            context: context_label.as_str(),
        };

        resolve_hw1f_params(&req, market).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })
    }

    fn enforce_resolved_hw_params(
        &self,
        swaption: &BermudanSwaption,
        hw_params: HullWhiteParams,
        source: Hw1fParamSource,
    ) -> std::result::Result<(), PricingError> {
        if self.config.enforce_calibration
            && source == Hw1fParamSource::DefaultFallback
            && hw_params.is_uncalibrated_default()
        {
            return Err(PricingError::model_failure_with_context(
                format!(
                    "Bermudan swaption {} received uncalibrated HullWhiteParams::default() \
                     (κ={:.3}, σ={:.3}) and no calibrated HW1F overrides, surface, market scalars, \
                     or pre-calibrated model. Supply pricing_overrides.hw1f_mean_reversion and \
                     pricing_overrides.hw1f_sigma, calibrated market scalars, or a pre-calibrated \
                     tree on `BermudanSwaptionPricerConfig`.",
                    swaption.id, hw_params.kappa, hw_params.sigma,
                ),
                PricingErrorContext::default(),
            ));
        }
        Ok(())
    }

    /// Price using Hull-White tree.
    ///
    /// If a pre-calibrated model is set on the config, it will be used
    /// directly, skipping the calibration step.
    fn price_tree(
        &self,
        swaption: &BermudanSwaption,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        if swaption.get_forward_curve_id() != swaption.get_discount_curve_id() {
            return Err(PricingError::model_failure_with_context(
                "Bermudan tree pricing is currently single-curve only. \
                 Set forward_curve_id equal to discount_curve_id or use a multi-curve-capable engine."
                    .to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Calculate time to maturity
        let ttm = swaption.time_to_maturity(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        if ttm <= 0.0 {
            // Expired - return zero
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::new(0.0, swaption.notional.currency()),
            ));
        }

        // Once the last exercise date has passed there is no remaining
        // optionality.  Treat the instrument as settled rather than
        // calibrating a tree with an empty exercise grid (which previously
        // produced a misleading model failure and required market data for a
        // position that had already expired).
        let exercise_times = swaption.exercise_times(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        if exercise_times.is_empty() {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::new(0.0, swaption.notional.currency()),
            ));
        }

        // Get discount curve only after lifecycle checks so a post-exercise
        // valuation does not fail because an otherwise unused curve is absent.
        let disc = market
            .get_discount(swaption.get_discount_curve_id().as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Use pre-calibrated model if available, otherwise calibrate a new one
        let (pv, used_cached_model) =
            if let Some(ref cached_tree) = self.config.pre_calibrated_model {
                // Use pre-calibrated model (O(1) per instrument)
                let valuator =
                    BermudanSwaptionTreeValuator::new(swaption, cached_tree, disc.as_ref(), as_of)
                        .map_err(|e| {
                            PricingError::model_failure_with_context(
                                e.to_string(),
                                PricingErrorContext::default(),
                            )
                        })?;
                let pv = valuator.price().map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
                (pv, true)
            } else {
                // Calibrate new model (O(Steps × Time) per instrument)
                let (hw_params, hw_source) = self.effective_hw_params(swaption, market, ttm)?;
                self.enforce_resolved_hw_params(swaption, hw_params, hw_source)?;
                // Thread exercise dates into the tree grid so Bermudan exercise
                // decisions land exactly on grid points.
                let tree_steps = self.effective_tree_steps(swaption);
                let model = CalibratedHullWhiteModel::calibrate_with_times(
                    hw_params,
                    tree_steps,
                    disc.as_ref(),
                    ttm,
                    &exercise_times,
                )?;

                let valuator =
                    BermudanSwaptionTreeValuator::new(swaption, &model, disc.as_ref(), as_of)
                        .map_err(|e| {
                            PricingError::model_failure_with_context(
                                e.to_string(),
                                PricingErrorContext::default(),
                            )
                        })?;
                let pv = valuator.price().map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
                (pv, false)
            };

        let mut result = ValuationResult::stamped(
            swaption.id.as_str(),
            as_of,
            Money::new(pv, swaption.notional.currency()),
        );

        // Record whether cached model was used (1.0 = true, 0.0 = false)
        result.measures.insert(
            crate::metrics::MetricId::custom("used_cached_model"),
            if used_cached_model { 1.0 } else { 0.0 },
        );

        Ok(result)
    }

    /// Price using LSMC (Longstaff-Schwartz Monte Carlo).
    ///
    /// Uses Hull-White 1F simulation with curve-calibrated θ(t) and
    /// Longstaff-Schwartz backward induction for optimal exercise decisions.
    ///
    /// # Features
    ///
    /// - Hull-White 1F short rate simulation with exact discretization
    /// - Curve-derived piecewise θ(t) for initial curve consistency
    /// - Polynomial basis functions for regression
    /// - Antithetic variates for variance reduction
    /// - Standard error estimation in results
    fn price_lsmc(
        &self,
        swaption: &BermudanSwaption,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        if swaption.get_forward_curve_id() != swaption.get_discount_curve_id() {
            return Err(PricingError::model_failure_with_context(
                "Bermudan Hull-White pricing is currently single-curve only. \
                 Set forward_curve_id equal to discount_curve_id or use a multi-curve-capable engine."
                    .to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Calculate time to maturity
        let ttm = swaption.time_to_maturity(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        if ttm <= 0.0 {
            // Expired - return zero
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::new(0.0, swaption.notional.currency()),
            ));
        }

        let exercise_times = swaption.exercise_times(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        if exercise_times.is_empty() {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::new(0.0, swaption.notional.currency()),
            ));
        }

        let disc = market
            .get_discount(swaption.get_discount_curve_id().as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let (hw_params, hw_source) = self.effective_hw_params(swaption, market, ttm)?;
        self.enforce_resolved_hw_params(swaption, hw_params, hw_source)?;

        // Get exercise times in years
        // Filter exercise times to be within [0, ttm]
        let valid_exercise_times: Vec<f64> = exercise_times
            .into_iter()
            .filter(|&t| t > 0.0 && t <= ttm)
            .collect();

        if valid_exercise_times.is_empty() {
            return Err(PricingError::model_failure_with_context(
                "No exercise dates before maturity".to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Build swap schedule (payment times and accrual fractions)
        let (payment_dates, accrual_fractions) =
            swaption.build_swap_schedule(as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Convert payment dates to year fractions
        let ctx = finstack_quant_core::dates::DayCountContext::default();
        let payment_times: Vec<f64> = payment_dates
            .iter()
            .map(|&d| swaption.get_day_count().year_fraction(as_of, d, ctx))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Create swap schedule for MC pricer
        let swap_start_time = swaption
            .get_day_count()
            .year_fraction(as_of, swaption.get_swap_start(), ctx)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let swap_schedule = SwapSchedule::new(
            swap_start_time,
            ttm,
            payment_times,
            accrual_fractions,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Determine option type for payoff
        let option_type = match swaption.option_type {
            OptionType::Call => SwaptionType::Payer,
            OptionType::Put => SwaptionType::Receiver,
        };
        let strike = swaption.strike_f64().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Create Bermudan payoff
        let payoff = BermudanSwaptionPayoff::new(
            valid_exercise_times.clone(),
            swap_schedule,
            strike,
            option_type,
            swaption.notional.amount(),
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Build exercise-aligned time grid
        let (time_grid, exercise_indices) = SwaptionLsmcConfig::build_exercise_aligned_grid(
            &valid_exercise_times,
            ttm,
            2, // Minimum steps between exercise dates
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Build θ(t) times for calibration (use grid times)
        let theta_times: Vec<f64> = (0..=time_grid.num_steps())
            .map(|i| time_grid.time(i.min(time_grid.num_steps() - 1)))
            .filter(|&t| t <= ttm)
            .collect();

        // Discount-curve closure giving `P(as_of, as_of + t)`.
        //
        // The HW1F simulation measures time from `t = 0 ≡ as_of`, but the
        // discount curve is anchored at its own `base_date`. Passing
        // `|t| disc.df(t)` unrebased treats curve-base time as as_of time:
        // when `as_of ≠ curve.base_date` the θ(t) calibration, the initial
        // short rate, and the HW1F bond reconstruction `P(t,T)` are all wrong.
        // Re-base to `as_of` exactly as `exotics_shared::hw1f_curve` does:
        //
        //   P(as_of, as_of + t) = DF_curve(t_asof + t) / DF_curve(t_asof)
        //
        // (the closure is built inline, capturing the `Arc<DiscountCurve>`, so
        // it stays `Send + Sync` as the LSMC engine requires).
        let curve_base = disc.base_date();
        let curve_dc = disc.day_count();
        let t_asof = if as_of == curve_base {
            0.0
        } else {
            curve_dc
                .year_fraction(
                    curve_base,
                    as_of,
                    finstack_quant_core::dates::DayCountContext::default(),
                )
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?
        };
        let df_asof = disc.df(t_asof);
        if !df_asof.is_finite() || df_asof <= 0.0 {
            return Err(PricingError::model_failure_with_context(
                format!(
                    "Bermudan LSMC: discount factor at as_of ({as_of}) is non-positive ({df_asof})"
                ),
                PricingErrorContext::default(),
            ));
        }
        let disc_for_fn = std::sync::Arc::clone(&disc);
        let discount_fn = move |t: f64| {
            let df = disc_for_fn.df(t_asof + t);
            if df.is_finite() && df > 0.0 {
                df / df_asof
            } else {
                0.0
            }
        };

        // Calibrate Hull-White parameters from discount curve
        let hw_params = calibrate_theta_from_curve(
            hw_params.kappa,
            hw_params.sigma,
            &discount_fn,
            &theta_times,
        );

        // Initial short rate from the `as_of`-rebased curve: a one-sided
        // forward difference f(0) = −ln P(as_of, as_of+dt) / dt.
        let dt_small = 0.01; // Small time step for initial rate
        let initial_rate = if dt_small > 0.0 {
            -discount_fn(dt_small).ln() / dt_small
        } else {
            0.03
        };

        // Create Hull-White process
        let hw_process = HullWhite1FProcess::new(hw_params);

        // Create LSMC config
        let mc_paths = self.effective_mc_paths(swaption);
        let lsmc_config = SwaptionLsmcConfig::new(mc_paths, self.config.mc_seed)
            .with_basis_degree(3)
            .with_antithetic(true);

        // Create the shared LSMC pricer
        let lsmc_pricer = SharedSwaptionLsmcPricer::with_config(lsmc_config, hw_process);

        // Create basis functions
        let basis = PolynomialBasis::new(3);

        // Price using the shared LSMC engine with custom grid
        let estimate = lsmc_pricer
            .price_bermudan_with_grid(
                &payoff,
                initial_rate,
                &time_grid,
                &exercise_indices,
                &basis,
                discount_fn,
                swaption.notional.currency(),
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Build result with diagnostics
        let mut result = ValuationResult::stamped(swaption.id.as_str(), as_of, estimate.mean);

        // Add LSMC diagnostics to measures
        result.measures.insert(
            crate::metrics::MetricId::custom("mc_stderr"),
            estimate.stderr,
        );
        result.measures.insert(
            crate::metrics::MetricId::custom("lsmc_num_paths"),
            mc_paths as f64,
        );
        result.measures.insert(
            crate::metrics::MetricId::custom("lsmc_seed"),
            self.config.mc_seed as f64,
        );
        let (ci_low, ci_high) = estimate.ci_95;
        result.measures.insert(
            crate::metrics::MetricId::custom("lsmc_ci95_low"),
            ci_low.amount(),
        );
        result.measures.insert(
            crate::metrics::MetricId::custom("lsmc_ci95_high"),
            ci_high.amount(),
        );

        Ok(result)
    }
}

impl Default for BermudanSwaptionPricer {
    fn default() -> Self {
        Self::tree()
    }
}

impl Pricer for BermudanSwaptionPricer {
    fn key(&self) -> PricerKey {
        match self.method {
            BermudanPricingMethod::HullWhiteTree => {
                PricerKey::new(InstrumentType::BermudanSwaption, ModelKey::HullWhite1F)
            }
            BermudanPricingMethod::LSMC => PricerKey::new(
                InstrumentType::BermudanSwaption,
                ModelKey::MonteCarloHullWhite1F,
            ),
        }
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        // Type-safe downcasting
        let swaption = instrument
            .as_any()
            .downcast_ref::<BermudanSwaption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::BermudanSwaption, instrument.key())
            })?;

        match self.method {
            BermudanPricingMethod::HullWhiteTree => self.price_tree(swaption, market, as_of),
            BermudanPricingMethod::LSMC => self.price_lsmc(swaption, market, as_of),
        }
    }
}
