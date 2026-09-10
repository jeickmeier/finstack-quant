//! Bermudan swaption pricer implementations.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::hw1f::{resolve_hw1f_params, Hw1fParamFamily};
use crate::instruments::rates::swaption::pricing::BermudanSwaptionTreeValuator;
use crate::instruments::rates::swaption::BermudanSwaption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_models::rates::clock::ModelDiscountCurve;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use finstack_quant_models::trees::HullWhiteTree;
use finstack_quant_models::trees::HullWhiteTreeConfig;
use std::sync::Arc;

// LSMC imports (gated by feature)
use crate::instruments::rates::hw1f::hw1f_mc::build_event_aligned_grid;
use crate::instruments::rates::hw1f::RateExoticMcConfig;
use crate::instruments::rates::swaption::pricing::hw_cashflows::{
    HwExerciseTerms, HwSwaptionCashflows,
};
use crate::instruments::rates::swaption::pricing::monte_carlo_lsmc::SwaptionLsmcPricer as SharedSwaptionLsmcPricer;
use crate::instruments::rates::swaption::pricing::swap_rate_utils::HullWhiteBondPrice;
use crate::instruments::rates::swaption::CashSettlementMethod;
use finstack_quant_models::monte_carlo::pricer::basis::PolynomialBasis;
use finstack_quant_models::monte_carlo::process::ou::{
    calibrate_theta_from_curve, HullWhite1FProcess,
};

/// Pricing method for Bermudan swaptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BermudanPricingMethod {
    /// Hull-White trinomial tree (industry standard, faster)
    #[default]
    HullWhiteTree,
    /// Longstaff-Schwartz Monte Carlo (more flexible)
    Lsmc,
}

impl std::fmt::Display for BermudanPricingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BermudanPricingMethod::HullWhiteTree => write!(f, "hull_white_tree"),
            BermudanPricingMethod::Lsmc => write!(f, "lsmc"),
        }
    }
}

impl std::str::FromStr for BermudanPricingMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "hull_white_tree" => Ok(Self::HullWhiteTree),
            "lsmc" => Ok(Self::Lsmc),
            _ => Err(format!(
                "Unknown Bermudan pricing method: '{}'. Valid: hull_white_tree, lsmc",
                s
            )),
        }
    }
}

/// Opaque Hull-White tree prepared from already-fitted model parameters.
#[derive(Debug, Clone)]
pub struct PreparedHullWhiteModel {
    tree: Arc<HullWhiteTree>,
    as_of: Date,
    curve_snapshot: serde_json::Value,
}

impl PreparedHullWhiteModel {
    /// Prepare a dated Hull-White tree from explicit fitted parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - Fitted mean reversion in inverse years and short-rate volatility per square-root year.
    /// * `steps` - Positive tree interval count before mandatory dates are inserted.
    /// * `disc` - Source discount curve; its full content is retained for reuse validation.
    /// * `as_of` - Valuation date, defining model time zero and discount normalization.
    /// * `ttm` - Positive ACT/365F horizon from `as_of`, covering all contractual cashflows.
    /// * `mandatory_times` - ACT/365F exercise times required exactly on the grid.
    ///
    /// # Errors
    ///
    /// Returns a pricing error for invalid parameters, curve, horizon or mandatory dates.
    pub fn prepare(
        params: HullWhiteCalibrationParams,
        steps: usize,
        disc: &DiscountCurve,
        as_of: Date,
        ttm: f64,
        mandatory_times: &[f64],
    ) -> std::result::Result<Self, PricingError> {
        if steps == 0 {
            return Err(PricingError::model_failure_with_context(
                "Tree steps must be positive".to_string(),
                PricingErrorContext::default(),
            ));
        }
        let config = HullWhiteTreeConfig::new(params.kappa, params.sigma, steps);
        let discount = ModelDiscountCurve::new(disc, as_of)
            .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?;
        let tree = HullWhiteTree::calibrate_with_times(config, &discount, ttm, mandatory_times)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
        Ok(Self {
            tree: Arc::new(tree),
            as_of,
            curve_snapshot: serde_json::to_value(disc).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?,
        })
    }

    pub(crate) fn validate_reuse(
        &self,
        disc: &DiscountCurve,
        as_of: Date,
        horizon: f64,
    ) -> finstack_quant_core::Result<()> {
        let snapshot = serde_json::to_value(disc)
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
        if as_of != self.as_of || snapshot != self.curve_snapshot {
            return Err(finstack_quant_core::Error::Validation("prepared Hull-White model valuation date or curve content differs from the pricing request".into()));
        }
        if horizon > self.tree.time_at_step(self.tree.num_steps()) + 1e-9 {
            return Err(finstack_quant_core::Error::Validation(
                "prepared Hull-White model horizon does not cover contractual cashflows".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn tree(&self) -> &Arc<HullWhiteTree> {
        &self.tree
    }
}

/// Pricer for Bermudan swaptions using Hull-White tree or LSMC.
///
/// # Model Reuse
///
/// For portfolio pricing, prepare the Hull-White tree once and reuse it
/// across multiple instruments by putting the prepared tree on
/// [`BermudanSwaptionPricerConfig`]:
///
/// ```
/// use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
/// use finstack_quant_valuations::instruments::rates::swaption::{
///     BermudanSwaption, BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
///     PreparedHullWhiteModel,
/// };
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use time::macros::date;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let as_of = date!(2026-01-15);
/// let swaption = BermudanSwaption::example();
/// let disc = DiscountCurve::flat("USD-OIS", as_of, 0.03)?;
/// let model = PreparedHullWhiteModel::prepare(
///     HullWhiteCalibrationParams::default(), 100, &disc, as_of,
///     swaption.time_to_maturity(as_of)?, &swaption.exercise_times(as_of)?,
/// )?;
/// let pricer = BermudanSwaptionPricer::tree_with_config(BermudanSwaptionPricerConfig {
///     prepared_model: Some(model),
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
    /// Original prepared-model grid retained by request-local risk repricing.
    risk_template: Option<PreparedHullWhiteModel>,
}

/// Configuration for Bermudan swaption Hull-White tree and LSMC pricers.
#[derive(Debug, Clone)]
pub struct BermudanSwaptionPricerConfig {
    /// Number of tree steps for Hull-White tree pricing.
    pub tree_steps: usize,
    /// Monte Carlo settings for LSMC pricing: path count (overridable per
    /// instrument via `model_config.mc_paths`), seed, antithetic sampling,
    /// minimum sub-steps between exercise dates and regression basis degree.
    pub mc: RateExoticMcConfig,
    /// Prepared Hull-White tree for model reuse.
    ///
    /// When set, the pricer reuses this prepared tree directly. This avoids
    /// repeating O(Steps × Time) deterministic tree preparation per instrument.
    /// Base valuation rejects a changed curve, valuation date, parameter set,
    /// horizon, or unsupported exercise date. Risk calculations rebuild the
    /// tree with the resolved parameters and original grid on each bumped
    /// market; theta translates mandatory model times to its valuation date.
    pub prepared_model: Option<PreparedHullWhiteModel>,
}

impl BermudanSwaptionPricerConfig {
    /// Default number of Hull-White tree steps.
    pub const DEFAULT_TREE_STEPS: usize = 100;
    /// Default Monte Carlo settings for LSMC pricing.
    ///
    /// 100,000 antithetic paths balance accuracy and performance for typical
    /// Bermudan swaptions (standard errors of ~0.1-0.5% of option value at
    /// 10M notional). For production pricing requiring tight standard errors
    /// (<0.05% of option value), increase to 500,000 paths. The regression
    /// uses a cubic polynomial basis with at least two simulation sub-steps
    /// between exercise dates.
    pub const DEFAULT_MC: RateExoticMcConfig = RateExoticMcConfig {
        num_paths: 100_000,
        seed: 42,
        antithetic: true,
        min_steps_between_events: 2,
        basis_degree: 3,
        oos_lsmc: false,
    };
}

impl Default for BermudanSwaptionPricerConfig {
    fn default() -> Self {
        Self {
            tree_steps: Self::DEFAULT_TREE_STEPS,
            mc: Self::DEFAULT_MC,
            prepared_model: None,
        }
    }
}

impl BermudanSwaptionPricer {
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
    /// Set `prepared_model` on the config to reuse a prepared
    /// Hull-White tree across a portfolio.
    pub fn tree_with_config(config: BermudanSwaptionPricerConfig) -> Self {
        Self {
            method: BermudanPricingMethod::HullWhiteTree,
            config,
            risk_template: None,
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
            method: BermudanPricingMethod::Lsmc,
            config,
            risk_template: None,
        }
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
            .unwrap_or(self.config.mc.num_paths)
    }

    fn effective_hw_params(
        &self,
        swaption: &BermudanSwaption,
        market: &MarketContext,
    ) -> std::result::Result<HullWhiteCalibrationParams, PricingError> {
        resolve_hw1f_params(
            Hw1fParamFamily::Swaption,
            swaption.get_discount_curve_id().as_str(),
            &swaption.instrument_pricing_overrides.model_config,
            None,
            &format!("BermudanSwaption {}", swaption.id),
            market,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })
    }

    /// Price using Hull-White tree.
    ///
    /// If a prepared model is set on the config, it will be used
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

        let ttm = swaption.time_to_maturity(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        if ttm <= 0.0 {
            // Expired - return zero
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::from((0_i64, swaption.notional.currency())),
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
                Money::from((0_i64, swaption.notional.currency())),
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

        // Use a prepared model if available, otherwise prepare a request-local tree.
        let (pv, used_cached_model) = if let Some(ref cached_tree) = self.config.prepared_model {
            let cached_config = cached_tree.tree.config();
            let resolved = resolve_hw1f_params(
                Hw1fParamFamily::Swaption,
                swaption.get_discount_curve_id().as_str(),
                &swaption.instrument_pricing_overrides.model_config,
                Some(
                    HullWhiteCalibrationParams::new(cached_config.kappa, cached_config.sigma)
                        .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?,
                ),
                &format!("BermudanSwaption {}", swaption.id),
                market,
            )
            .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?;
            if resolved.kappa.to_bits() != cached_config.kappa.to_bits()
                || resolved.sigma.to_bits() != cached_config.sigma.to_bits()
                || swaption
                    .instrument_pricing_overrides
                    .model_config
                    .tree_steps
                    .is_some_and(|steps| steps != cached_config.steps)
            {
                return Err(PricingError::model_failure_with_context(
                    "prepared Hull-White model parameters or tree configuration differ from the pricing request",
                    PricingErrorContext::from_instrument(swaption),
                ));
            }
            // Validate and reuse the prepared model.
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
            // Prepare a request-local tree (O(Steps × Time) per instrument).
            let hw_params = self.effective_hw_params(swaption, market)?;
            // Thread exercise dates into the tree grid so Bermudan exercise
            // decisions land exactly on grid points.
            let tree_steps = self.effective_tree_steps(swaption);
            let mut mandatory_times = exercise_times;
            let horizon = if let Some(template) = &self.risk_template {
                let elapsed =
                    finstack_quant_models::rates::clock::model_time(template.as_of, as_of);
                mandatory_times.extend(
                    template
                        .tree
                        .time_grid()
                        .iter()
                        .map(|time| time - elapsed)
                        .filter(|time| *time > 0.0),
                );
                (template.tree.time_at_step(template.tree.num_steps()) - elapsed).max(ttm)
            } else {
                ttm
            };
            let model = PreparedHullWhiteModel::prepare(
                hw_params,
                tree_steps,
                disc.as_ref(),
                as_of,
                horizon,
                &mandatory_times,
            )?;

            let valuator =
                BermudanSwaptionTreeValuator::new(swaption, &model, disc.as_ref(), as_of).map_err(
                    |e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    },
                )?;
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
            Money::new(pv, swaption.notional.currency()).map_err(|error| {
                crate::pricer::PricingError::from_core(
                    error,
                    crate::pricer::PricingErrorContext::from_instrument(swaption),
                )
            })?,
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

        let ttm = swaption.time_to_maturity(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        if ttm <= 0.0 {
            // Expired - return zero
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::from((0_i64, swaption.notional.currency())),
            ));
        }

        let exercise_times = swaption.exercise_times(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        if exercise_times.is_empty() {
            return Ok(ValuationResult::stamped(
                swaption.id.as_str(),
                as_of,
                Money::from((0_i64, swaption.notional.currency())),
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

        let hw_params = self.effective_hw_params(swaption, market)?;

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

        let exercise_cashflows = swaption
            .bermudan_schedule
            .effective_dates()
            .into_iter()
            .filter(|date| *date > as_of)
            .map(|date| {
                HwSwaptionCashflows::new(
                    &swaption.underlying_fixed_leg,
                    &swaption.underlying_float_leg,
                    date,
                    as_of,
                )
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()
            .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?;
        let terms = HwExerciseTerms {
            strike: swaption
                .strike_f64()
                .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?,
            notional: swaption.notional.amount(),
            option_type: swaption.option_type,
            settlement: swaption.settlement,
            cash_method: CashSettlementMethod::CollateralizedCashPrice,
        };

        // Build exercise-aligned time grid
        let (time_grid, exercise_indices) = build_event_aligned_grid(
            &valid_exercise_times,
            ttm,
            self.config.mc.min_steps_between_events,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Build θ(t) times for calibration (use grid times)
        let theta_times: Vec<f64> = time_grid
            .times()
            .iter()
            .copied()
            .filter(|&t| t <= ttm)
            .collect();

        let model_curve = ModelDiscountCurve::new(disc.as_ref(), as_of)
            .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?;
        let discount_fn = |t| model_curve.get_df(t).unwrap_or(f64::NAN);

        // Calibrate Hull-White parameters from discount curve
        let hw_params =
            calibrate_theta_from_curve(hw_params.kappa, hw_params.sigma, discount_fn, &theta_times)
                .map_err(|error| {
                    PricingError::model_failure_with_context(
                        error.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;

        // Initial short rate from the `as_of`-rebased curve: a one-sided
        // forward difference f(0) = −ln P(as_of, as_of+dt) / dt.
        let dt_small = 0.01; // Small time step for initial rate
        let initial_rate = if dt_small > 0.0 {
            -discount_fn(dt_small).ln() / dt_small
        } else {
            0.03
        };

        let exercise_value = |step: usize, short_rate: f64| {
            let slot = exercise_indices.binary_search(&step).map_err(|_| {
                finstack_quant_core::Error::Validation(
                    "LSMC exercise is absent from the prepared schedule".into(),
                )
            })?;
            let t = time_grid.time(step);
            Ok(exercise_cashflows[slot].evaluate(
                t,
                hw_params.kappa,
                hw_params.sigma_at_time(t),
                |maturity| {
                    HullWhiteBondPrice::bond_price(&hw_params, short_rate, t, maturity, discount_fn)
                },
                terms,
            ))
        };
        let hw_process = HullWhite1FProcess::new(hw_params.clone());

        let mc_paths = self.effective_mc_paths(swaption);
        let lsmc_config = RateExoticMcConfig {
            num_paths: mc_paths,
            ..self.config.mc
        };

        let lsmc_pricer = SharedSwaptionLsmcPricer::with_config(lsmc_config, hw_process);

        let basis = PolynomialBasis::new(self.config.mc.basis_degree);

        let estimate = lsmc_pricer
            .price_bermudan_with_grid(
                exercise_value,
                initial_rate,
                &time_grid,
                &exercise_indices,
                &basis,
                swaption.notional.currency(),
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let mut result = ValuationResult::stamped(swaption.id.as_str(), as_of, estimate.mean);

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
            self.config.mc.seed as f64,
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
    fn seed_metric_context(
        &self,
        context: &mut crate::metrics::MetricContext,
        metrics: &[crate::metrics::MetricId],
    ) -> finstack_quant_core::Result<()> {
        let mut swaption = context.instrument_as::<BermudanSwaption>()?.clone();
        let ttm = swaption.time_to_maturity(context.as_of)?;
        if ttm <= 0.0 || swaption.exercise_times(context.as_of)?.is_empty() {
            return Ok(());
        }
        let cached = self.config.prepared_model.as_ref();
        let fallback = cached
            .map(|model| {
                let cfg = model.tree.config();
                HullWhiteCalibrationParams::new(cfg.kappa, cfg.sigma)
            })
            .transpose()?;
        let params = resolve_hw1f_params(
            Hw1fParamFamily::Swaption,
            swaption.get_discount_curve_id().as_str(),
            &swaption.instrument_pricing_overrides.model_config,
            fallback,
            &format!("BermudanSwaption {}", swaption.id),
            &context.curves,
        )?;
        let steps = cached.map_or_else(
            || self.effective_tree_steps(&swaption),
            |model| model.tree.config().steps,
        );
        let config = &mut swaption.instrument_pricing_overrides.model_config;
        config.hw1f_mean_reversion = Some(params.kappa);
        config.hw1f_sigma = Some(params.sigma);
        config.tree_steps = Some(steps);
        let exercise_metric = crate::metrics::MetricId::custom("exercise_probability");
        if metrics.contains(&exercise_metric) {
            if self.method != BermudanPricingMethod::HullWhiteTree {
                return Err(finstack_quant_core::Error::Validation(
                    "exercise_probability requires the Hull-White tree exercise policy".into(),
                ));
            }
            let disc = context
                .curves
                .get_discount(swaption.get_discount_curve_id().as_str())?;
            let fresh;
            let model = if let Some(model) = cached {
                model
            } else {
                fresh = PreparedHullWhiteModel::prepare(
                    params,
                    steps,
                    disc.as_ref(),
                    context.as_of,
                    ttm,
                    &swaption.exercise_times(context.as_of)?,
                )
                .map_err(finstack_quant_core::Error::from)?;
                &fresh
            };
            let valuator =
                BermudanSwaptionTreeValuator::new(&swaption, model, disc.as_ref(), context.as_of)?;
            let expected = super::metrics::bermudan_greeks::expected_exercise_time(&valuator);
            context.computed.insert(exercise_metric, expected);
        }
        context.set_instrument_overrides(Some(swaption.instrument_pricing_overrides.clone()));
        context.set_instrument(Arc::new(swaption));
        if let crate::pricer::PricingDispatch::Registered { model, registry } =
            context.clone_pricer_dispatch()
        {
            let mut risk_registry = (*registry).clone();
            let mut config = self.config.clone();
            config.prepared_model = None;
            config.tree_steps = steps;
            risk_registry.replace(Self {
                method: self.method,
                config,
                risk_template: cached.cloned(),
            });
            context.set_pricer_dispatch(crate::pricer::PricingDispatch::registered(
                model,
                Arc::new(risk_registry),
            ));
        }
        Ok(())
    }

    fn key(&self) -> PricerKey {
        match self.method {
            BermudanPricingMethod::HullWhiteTree => {
                PricerKey::new(InstrumentType::BermudanSwaption, ModelKey::HullWhite1F)
            }
            BermudanPricingMethod::Lsmc => PricerKey::new(
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
        let swaption = crate::pricer::expect_inst::<BermudanSwaption>(
            instrument,
            InstrumentType::BermudanSwaption,
        )?;

        match self.method {
            BermudanPricingMethod::HullWhiteTree => self.price_tree(swaption, market, as_of),
            BermudanPricingMethod::Lsmc => self.price_lsmc(swaption, market, as_of),
        }
    }
}
