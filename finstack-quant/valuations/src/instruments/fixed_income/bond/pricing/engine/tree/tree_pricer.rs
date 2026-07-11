use super::super::super::super::types::Bond;
use super::bond_valuator::BondValuator;
use super::config::{TreeModelChoice, TreePricerConfig};
use crate::instruments::pricing_overrides::OasPriceBasis;
use crate::models::trees::hull_white_tree::{HullWhiteTree, HullWhiteTreeConfig};
use crate::models::trees::short_rate_tree::CalibrationResult;
use crate::models::trees::two_factor_rates_credit::{RatesCreditConfig, RatesCreditTree};
use crate::models::{short_rate_keys, ShortRateTree, ShortRateTreeConfig, TreeModel};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::HashMap;
use finstack_quant_core::{Error, Result};

/// Tree-based pricer for bonds with embedded options and OAS calculations.
///
/// Provides methods for calculating option-adjusted spread (OAS) for bonds with
/// embedded call/put options. Automatically selects between short-rate and
/// rates+credit tree models based on available market data.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricer;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// let pricer = TreePricer::new();
/// // OAS in basis points
/// let oas_bp = pricer.calculate_oas(&bond, &market, as_of, 98.5)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct TreePricer {
    /// Pricer configuration (tree steps, volatility, convergence settings)
    config: TreePricerConfig,
}

impl TreePricer {
    /// Resolve the hazard curve for credit-risky tree pricing from the bond's
    /// explicit `credit_curve_id` opt-in.
    ///
    /// - `credit_curve_id = None` → `Ok(None)`: risk-free tree pricing.
    ///   Implicit discovery by naming convention (`discount_curve_id` /
    ///   `<discount_curve_id>-CREDIT`) is intentionally not supported — it
    ///   silently switched bonds to credit-risky pricing.
    /// - `credit_curve_id = Some(id)` but the curve is missing → `Err`: the
    ///   instrument opted into credit pricing, so silently degrading to
    ///   risk-free pricing would misprice with no signal.
    fn resolve_opt_in_hazard_curve(
        bond: &Bond,
        market_context: &MarketContext,
    ) -> Result<
        Option<std::sync::Arc<finstack_quant_core::market_data::term_structures::HazardCurve>>,
    > {
        match bond.credit_curve_id.as_ref() {
            None => Ok(None),
            Some(hid) => market_context
                .get_hazard(hid.as_str())
                .map(Some)
                .map_err(|_| {
                    finstack_quant_core::Error::Validation(format!(
                        "Bond '{}' opts into credit-risky tree pricing via credit_curve_id \
                         '{}', but no hazard curve with that id exists in the market context.",
                        bond.id.as_str(),
                        hid.as_str()
                    ))
                }),
        }
    }

    fn effective_steps_for_model(
        &self,
        bond: &Bond,
        as_of: Date,
        day_count: finstack_quant_core::dates::DayCount,
        model: &TreeModelChoice,
    ) -> usize {
        if !matches!(model, TreeModelChoice::BlackDermanToy { .. }) {
            return self.config.tree_steps;
        }

        let Some(call_put) = bond.call_put.as_ref() else {
            return self.config.tree_steps;
        };
        if !call_put.has_options() {
            return self.config.tree_steps;
        }

        // Window endpoints drive the step-count alignment. Interior coupon
        // dates are also exercise dates (see `BondValuator::
        // exercise_dates_for_period`) but fall on the regular coupon grid,
        // which uniform steps already approximate well.
        let exercise_times: Vec<f64> = call_put
            .calls
            .iter()
            .flat_map(|call| [call.start_date, call.end_date])
            .chain(
                call_put
                    .puts
                    .iter()
                    .flat_map(|put| [put.start_date, put.end_date]),
            )
            .filter(|date| *date > as_of && *date < bond.maturity)
            .filter_map(|date| {
                day_count
                    .year_fraction(
                        as_of,
                        date,
                        finstack_quant_core::dates::DayCountContext::default(),
                    )
                    .ok()
            })
            .collect();
        if exercise_times.is_empty() {
            return self.config.tree_steps;
        }

        let Ok(time_to_maturity) = day_count.year_fraction(
            as_of,
            bond.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        ) else {
            return self.config.tree_steps;
        };
        if time_to_maturity <= 0.0 {
            return self.config.tree_steps;
        }

        let max_steps =
            (self.config.tree_steps.saturating_mul(4)).clamp(self.config.tree_steps, 1000);
        (self.config.tree_steps..=max_steps)
            .min_by(|a, b| {
                let score = |steps: usize| {
                    exercise_times
                        .iter()
                        .map(|time| {
                            let raw = time / time_to_maturity * steps as f64;
                            (raw - raw.round()).abs()
                        })
                        .fold(0.0_f64, f64::max)
                };
                score(*a)
                    .partial_cmp(&score(*b))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.cmp(b))
            })
            .unwrap_or(self.config.tree_steps)
    }

    /// Create a new tree pricer with default configuration.
    ///
    /// # Returns
    ///
    /// A `TreePricer` with default configuration (100 steps, 1% volatility).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricer;
    ///
    /// let pricer = TreePricer::new();
    /// ```
    pub fn new() -> Self {
        Self {
            config: TreePricerConfig::default(),
        }
    }

    /// Create a tree pricer with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Custom tree pricer configuration
    ///
    /// # Returns
    ///
    /// A `TreePricer` with the specified configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::{TreePricer, TreePricerConfig};
    ///
    /// let config = TreePricerConfig::high_precision(0.015);
    /// let pricer = TreePricer::with_config(config);
    /// ```
    pub fn with_config(config: TreePricerConfig) -> Self {
        Self { config }
    }

    /// Price a bond with the configured tree at a fixed OAS in basis points.
    pub(crate) fn price_at_oas(
        &self,
        bond: &Bond,
        market_context: &MarketContext,
        as_of: Date,
        oas_bp: f64,
    ) -> Result<f64> {
        let continuous_oas_bp = self
            .config
            .oas_quote_compounding
            .continuous_from_quote_decimal(oas_bp / 10_000.0)
            * 10_000.0;
        let tree_discount_curve_id = self
            .config
            .tree_discount_curve_id
            .as_ref()
            .unwrap_or(&bond.discount_curve_id);
        let discount_curve = market_context.get_discount(tree_discount_curve_id.as_str())?;
        let tree_bond_storage;
        let tree_bond = if tree_discount_curve_id != &bond.discount_curve_id {
            tree_bond_storage = {
                let mut cloned = bond.clone();
                cloned.discount_curve_id = tree_discount_curve_id.clone();
                cloned
            };
            &tree_bond_storage
        } else {
            bond
        };

        if as_of >= bond.maturity {
            // The contractual maturity can roll to a later business-day
            // payment date. Once the option exercise period has ended there
            // is no tree optionality left, but any adjusted future redemption
            // must still be discounted rather than discarded.
            let flows = tree_bond.pricing_dated_cashflows(market_context, as_of)?;
            let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::default();
            for (date, amount) in flows {
                let df = discount_curve.df_between_dates(as_of, date)?;
                pv.add(amount.amount() * df);
            }
            return Ok(pv.total());
        }
        let time_to_maturity = discount_curve.day_count().year_fraction(
            as_of,
            bond.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if time_to_maturity <= 0.0 {
            return Ok(0.0);
        }

        let hazard_curve = Self::resolve_opt_in_hazard_curve(bond, market_context)?;

        let valuator = BondValuator::new(
            tree_bond.clone(),
            market_context,
            as_of,
            time_to_maturity,
            self.config.tree_steps,
        )?;

        if let Some(hc) = hazard_curve.as_ref() {
            let cfg = RatesCreditConfig {
                steps: self.config.tree_steps,
                rate_vol: self.config.volatility,
                ..Default::default()
            };
            let mut tree = RatesCreditTree::new(cfg);
            tree.calibrate(discount_curve.as_ref(), hc.as_ref(), time_to_maturity)?;
            let mut vars = HashMap::<&'static str, f64>::default();
            vars.insert("oas", continuous_oas_bp);
            return tree.price(vars, time_to_maturity, market_context, &valuator);
        }

        let effective_model = match &self.config.tree_model {
            TreeModelChoice::HullWhiteCalibratedToSwaptions {
                swaption_vol_surface_id,
            } => Self::resolve_hw_calibrated(
                market_context,
                &discount_curve,
                swaption_vol_surface_id,
                time_to_maturity,
            ),
            other => other.clone(),
        };

        match effective_model {
            TreeModelChoice::HullWhite { kappa, sigma } => {
                let hw_config = HullWhiteTreeConfig {
                    kappa,
                    sigma,
                    steps: self.config.tree_steps,
                    max_nodes: None,
                    compounding: self.config.tree_compounding,
                };
                // Thread coupon and call/put dates into the tree grid so
                // exercise decisions and cashflows land exactly on nodes,
                // and build the valuator on the tree's (non-uniform) grid.
                let mandatory =
                    BondValuator::mandatory_grid_times(tree_bond, market_context, as_of)?;
                let hw_tree = HullWhiteTree::calibrate_with_times(
                    hw_config,
                    discount_curve.as_ref(),
                    time_to_maturity,
                    &mandatory,
                )?;
                let hw_valuator = BondValuator::new_with_time_steps(
                    tree_bond.clone(),
                    market_context,
                    as_of,
                    hw_tree.time_grid().to_vec(),
                )?;
                hw_valuator.price_with_hw_tree(&hw_tree, continuous_oas_bp)
            }
            TreeModelChoice::BlackDermanToy {
                mean_reversion,
                sigma,
            } => {
                let tree_steps = self.effective_steps_for_model(
                    tree_bond,
                    as_of,
                    discount_curve.day_count(),
                    &TreeModelChoice::BlackDermanToy {
                        mean_reversion,
                        sigma,
                    },
                );
                let valuator = BondValuator::new(
                    tree_bond.clone(),
                    market_context,
                    as_of,
                    time_to_maturity,
                    tree_steps,
                )?;
                let tree_config = ShortRateTreeConfig::bdt(tree_steps, sigma, mean_reversion)
                    .with_compounding(self.config.tree_compounding);
                let mut tree = ShortRateTree::new(tree_config);
                tree.calibrate(
                    tree_discount_curve_id,
                    discount_curve.as_ref(),
                    time_to_maturity,
                )?;
                validate_bdt_calibration_quality(tree.calibration_result())?;
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, tree.rate_at_node(0, 0)?);
                vars.insert(short_rate_keys::OAS, continuous_oas_bp);
                tree.price(vars, time_to_maturity, market_context, &valuator)
            }
            TreeModelChoice::HoLee | TreeModelChoice::HullWhiteCalibratedToSwaptions { .. } => {
                let tree_config = ShortRateTreeConfig {
                    steps: self.config.tree_steps,
                    volatility: self.config.volatility,
                    mean_reversion: None,
                    compounding: self.config.tree_compounding,
                    ..Default::default()
                };
                let mut tree = ShortRateTree::new(tree_config);
                tree.calibrate(
                    tree_discount_curve_id,
                    discount_curve.as_ref(),
                    time_to_maturity,
                )?;
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, tree.rate_at_node(0, 0)?);
                vars.insert(short_rate_keys::OAS, continuous_oas_bp);
                tree.price(vars, time_to_maturity, market_context, &valuator)
            }
        }
    }

    /// Calculate option-adjusted spread (OAS) for a bond.
    ///
    /// Solves for the constant spread that equates the tree price to the market price.
    /// Uses Brent's method for root finding, automatically selecting between short-rate
    /// and rates+credit tree models based on available market data.
    ///
    /// # OAS Convention
    ///
    /// Under either model the OAS is a **parallel shift to the calibrated risk-free
    /// short rate lattice** (in basis points). When the rates+credit two-factor tree
    /// is used, the hazard tree captures the credit spread independently, so the OAS
    /// represents the option-adjusted spread **over the risk-free curve** — consistent
    /// with the Bloomberg OAS convention for risky bonds.
    ///
    /// # Arguments
    ///
    /// * `bond` - The bond to calculate OAS for (must have call/put options)
    /// * `market_context` - Market context with discount and optionally hazard curves
    /// * `as_of` - Valuation date
    /// * `clean_price_pct_of_par` - Market clean price as percentage of par (e.g., 98.5)
    ///
    /// # Returns
    ///
    /// OAS in basis points (e.g., 150.0 means 150 basis points).
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Discount curve is not found
    /// - Tree calibration fails
    /// - Root finding fails to converge
    /// - Bond is already matured
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricer;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::dates::Date;
    ///
    /// # let bond = Bond::example().unwrap();
    /// # let market = MarketContext::new();
    /// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
    /// let pricer = TreePricer::new();
    /// let oas_bp = pricer.calculate_oas(&bond, &market, as_of, 98.5)?;
    /// // oas_bp is in basis points
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn calculate_oas(
        &self,
        bond: &Bond,
        market_context: &MarketContext,
        as_of: Date,
        clean_price_pct_of_par: f64,
    ) -> Result<f64> {
        use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;

        // Dirty target must use accrued at the quote/settlement date to match
        // the market convention used by YTM, Z-spread, and the quote engine.
        let quote_ctx = QuoteDateContext::new(bond, market_context, as_of)?;
        let quote_date = quote_ctx.quote_date;
        let clean_target = clean_price_pct_of_par * bond.notional.amount() / 100.0;
        let dirty_target = match self.config.oas_price_basis {
            OasPriceBasis::SettlementDirty => {
                quote_ctx.dirty_from_clean_pct(clean_price_pct_of_par, bond.notional.amount())
            }
            OasPriceBasis::ForwardAccruedClean => {
                let schedule = bond.full_cashflow_schedule(market_context)?;
                let accrued_at_as_of = crate::cashflow::accrual::accrued_interest_amount(
                    &schedule,
                    as_of,
                    &bond.accrual_config(),
                )?;
                clean_target + quote_ctx.accrued_at_quote_date - accrued_at_as_of
            }
        };
        // Choose model: if the bond opts into credit via `credit_curve_id` and
        // that hazard curve exists, use the rates+credit two-factor tree;
        // otherwise, fall back to short-rate.
        let mut use_rates_credit = false;
        let mut rc_tree: Option<RatesCreditTree> = None;
        let tree_discount_curve_id = self
            .config
            .tree_discount_curve_id
            .as_ref()
            .unwrap_or(&bond.discount_curve_id);
        let discount_curve = market_context.get_discount(tree_discount_curve_id.as_str())?;
        let tree_bond_storage;
        let tree_bond = if tree_discount_curve_id != &bond.discount_curve_id {
            tree_bond_storage = {
                let mut cloned = bond.clone();
                cloned.discount_curve_id = tree_discount_curve_id.clone();
                cloned
            };
            &tree_bond_storage
        } else {
            bond
        };
        // Align tree time basis with the discount curve's own day-count.
        if quote_date >= bond.maturity {
            return Ok(0.0);
        }
        let dc_curve = discount_curve.day_count();
        let time_to_maturity = dc_curve.year_fraction(
            quote_date,
            bond.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if time_to_maturity <= 0.0 {
            return Ok(0.0);
        }
        let hazard_curve = Self::resolve_opt_in_hazard_curve(bond, market_context)?;
        if let Some(hc) = hazard_curve.as_ref() {
            let cfg = RatesCreditConfig {
                steps: self.config.tree_steps,
                rate_vol: self.config.volatility,
                ..Default::default()
            };
            let mut tree = RatesCreditTree::new(cfg);
            tree.calibrate(discount_curve.as_ref(), hc.as_ref(), time_to_maturity)?;
            rc_tree = Some(tree);
            use_rates_credit = true;
        }

        // Resolve the effective HW parameters when using HullWhite model variants.
        // For HullWhiteCalibratedToSwaptions, attempt swaption calibration;
        // on failure, log a warning and fall back to HoLee.
        let effective_model = match &self.config.tree_model {
            TreeModelChoice::HullWhiteCalibratedToSwaptions {
                swaption_vol_surface_id,
            } if !use_rates_credit => Self::resolve_hw_calibrated(
                market_context,
                &discount_curve,
                swaption_vol_surface_id,
                time_to_maturity,
            ),
            other => other.clone(),
        };

        let mut sr_tree: Option<ShortRateTree> = None;
        let mut hw_tree: Option<HullWhiteTree> = None;
        let mut valuation_steps = self.config.tree_steps;

        if !use_rates_credit {
            match &effective_model {
                TreeModelChoice::HullWhite { kappa, sigma } => {
                    let hw_config = HullWhiteTreeConfig {
                        kappa: *kappa,
                        sigma: *sigma,
                        steps: self.config.tree_steps,
                        max_nodes: None,
                        compounding: self.config.tree_compounding,
                    };
                    // Grid through coupon and call/put dates (per-step dt).
                    let mandatory =
                        BondValuator::mandatory_grid_times(tree_bond, market_context, quote_date)?;
                    hw_tree = Some(HullWhiteTree::calibrate_with_times(
                        hw_config,
                        discount_curve.as_ref(),
                        time_to_maturity,
                        &mandatory,
                    )?);
                }
                TreeModelChoice::HoLee | TreeModelChoice::HullWhiteCalibratedToSwaptions { .. } => {
                    let tree_config = ShortRateTreeConfig {
                        steps: self.config.tree_steps,
                        volatility: self.config.volatility,
                        mean_reversion: None,
                        ..Default::default()
                    };
                    let mut tree = ShortRateTree::new(tree_config);
                    tree.calibrate(
                        tree_discount_curve_id,
                        discount_curve.as_ref(),
                        time_to_maturity,
                    )?;
                    sr_tree = Some(tree);
                }
                TreeModelChoice::BlackDermanToy {
                    mean_reversion,
                    sigma,
                } => {
                    valuation_steps = self.effective_steps_for_model(
                        tree_bond,
                        quote_date,
                        discount_curve.day_count(),
                        &effective_model,
                    );
                    let tree_config =
                        ShortRateTreeConfig::bdt(valuation_steps, *sigma, *mean_reversion)
                            .with_compounding(self.config.tree_compounding);
                    let mut tree = ShortRateTree::new(tree_config);
                    tree.calibrate(
                        tree_discount_curve_id,
                        discount_curve.as_ref(),
                        time_to_maturity,
                    )?;
                    validate_bdt_calibration_quality(tree.calibration_result())?;
                    sr_tree = Some(tree);
                }
            }
        }

        // The HW path prices on the tree's (possibly non-uniform) grid; all
        // other models use the uniform grid implied by `valuation_steps`.
        let valuator = if let Some(ref tree) = hw_tree {
            BondValuator::new_with_time_steps(
                tree_bond.clone(),
                market_context,
                quote_date,
                tree.time_grid().to_vec(),
            )?
        } else {
            BondValuator::new(
                tree_bond.clone(),
                market_context,
                quote_date,
                time_to_maturity,
                valuation_steps,
            )?
        };

        // Get initial short rate for state variables (needed by short-rate tree)
        let initial_rate = if let Some(tree) = sr_tree.as_ref() {
            tree.rate_at_node(0, 0).unwrap_or(0.03)
        } else {
            0.0 // Not used for rates+credit or HW tree
        };

        // Capture the first tree-pricing error so a solver failure can report
        // the underlying cause instead of a generic bracket/convergence error.
        let pricing_error: std::cell::RefCell<Option<finstack_quant_core::Error>> =
            std::cell::RefCell::new(None);
        let record_error = |e: finstack_quant_core::Error| -> f64 {
            let mut slot = pricing_error.borrow_mut();
            if slot.is_none() {
                *slot = Some(e);
            }
            // Flat large positive residual — same pattern as the YTM/DM
            // solvers. The model price is monotonically decreasing in OAS and
            // tree pricing fails in the divergent (deeply negative OAS)
            // regime where the true price → +∞, so `price - target` is
            // unambiguously large and positive. The previous `±1e6` keyed to
            // `sign(oas)` flipped sign at oas = 0 and could hand Brent a
            // fabricated bracket around a non-root.
            1.0e12
        };

        let objective_fn = |oas: f64| -> f64 {
            if use_rates_credit {
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert("oas", oas);
                if let Some(tree) = rc_tree.as_ref() {
                    match tree.price(vars, time_to_maturity, market_context, &valuator) {
                        Ok(model_price) => model_price - dirty_target,
                        Err(e) => record_error(e),
                    }
                } else {
                    record_error(finstack_quant_core::Error::internal(
                        "rates+credit OAS solve invoked without a calibrated tree",
                    ))
                }
            } else if let Some(ref tree) = hw_tree {
                // Hull-White trinomial tree: OAS applied inside backward induction
                match valuator.price_with_hw_tree(tree, oas) {
                    Ok(model_price) => model_price - dirty_target,
                    Err(e) => record_error(e),
                }
            } else {
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, initial_rate);
                vars.insert(short_rate_keys::OAS, oas);
                if let Some(tree) = sr_tree.as_ref() {
                    match tree.price(vars, time_to_maturity, market_context, &valuator) {
                        Ok(model_price) => model_price - dirty_target,
                        Err(e) => record_error(e),
                    }
                } else {
                    record_error(finstack_quant_core::Error::internal(
                        "short-rate OAS solve invoked without a calibrated tree",
                    ))
                }
            }
        };

        let mut solver = BrentSolver::new()
            .tolerance(self.config.tolerance)
            .initial_bracket_size(self.config.initial_bracket_size_bp);
        // Respect the configured maximum iteration cap for OAS root-finding.
        solver.max_iterations = self.config.max_iterations;
        let initial_guess = 0.0;
        let continuous_oas_bp = solver.solve(objective_fn, initial_guess).map_err(|e| {
            match pricing_error.borrow_mut().take() {
                Some(tree_err) => finstack_quant_core::Error::Validation(format!(
                    "OAS tree solve failed: {e}; first underlying tree-pricing error: {tree_err}"
                )),
                None => e,
            }
        })?;
        Ok(self
            .config
            .oas_quote_compounding
            .quote_from_continuous_decimal(continuous_oas_bp / 10_000.0)
            * 10_000.0)
    }

    /// Attempt swaption-calibrated Hull-White. On failure, fall back to HoLee.
    ///
    /// Reads the swaption vol surface from the market context, converts grid
    /// points into `SwaptionQuote`s, and runs Levenberg-Marquardt calibration.
    fn resolve_hw_calibrated(
        market_context: &MarketContext,
        discount_curve: &std::sync::Arc<
            finstack_quant_core::market_data::term_structures::DiscountCurve,
        >,
        swaption_vol_surface_id: &str,
        time_to_maturity: f64,
    ) -> TreeModelChoice {
        use crate::calibration::hull_white::{
            calibrate_hull_white_to_swaptions, SwapFrequency, SwaptionQuote,
        };

        let Ok(surface) = market_context.get_surface(swaption_vol_surface_id) else {
            tracing::warn!(
                surface_id = swaption_vol_surface_id,
                "Swaption vol surface not found in market context; \
                 falling back to HoLee tree model"
            );
            return TreeModelChoice::HoLee;
        };

        // Build SwaptionQuote list from the surface grid.
        // Convention: expiries axis = swaption expiry (years),
        //             strikes axis = underlying swap tenor (years).
        // Each grid point is an ATM normal vol.
        let expiries = surface.expiries();
        let tenors = surface.strikes();
        let mut quotes = Vec::with_capacity(expiries.len() * tenors.len());
        for &expiry in expiries {
            // Only use swaptions expiring before the bond maturity
            if expiry > time_to_maturity || expiry <= 0.0 {
                continue;
            }
            for &tenor in tenors {
                if tenor <= 0.0 {
                    continue;
                }
                let vol = surface.value_clamped(expiry, tenor);
                if vol > 0.0 && vol.is_finite() {
                    quotes.push(SwaptionQuote {
                        expiry,
                        tenor,
                        volatility: vol,
                        is_normal_vol: true,
                    });
                }
            }
        }

        if quotes.len() < 2 {
            tracing::warn!(
                surface_id = swaption_vol_surface_id,
                n_valid = quotes.len(),
                "Insufficient swaption quotes from vol surface; \
                 falling back to HoLee tree model"
            );
            return TreeModelChoice::HoLee;
        }

        let dc = std::sync::Arc::clone(discount_curve);
        let df_fn = move |t: f64| dc.df(t);

        match calibrate_hull_white_to_swaptions(&df_fn, &quotes, SwapFrequency::SemiAnnual, None) {
            Ok((params, report)) => {
                if report.success {
                    tracing::info!(
                        kappa = params.kappa,
                        sigma = params.sigma,
                        n_quotes = quotes.len(),
                        "Hull-White calibrated to swaptions"
                    );
                    TreeModelChoice::HullWhite {
                        kappa: params.kappa,
                        sigma: params.sigma,
                    }
                } else {
                    tracing::warn!(
                        reason = report.convergence_reason.as_str(),
                        "Swaption calibration did not converge; \
                         falling back to HoLee tree model"
                    );
                    TreeModelChoice::HoLee
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Swaption calibration failed; falling back to HoLee tree model"
                );
                TreeModelChoice::HoLee
            }
        }
    }
}

impl Default for TreePricer {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_bdt_calibration_quality(quality: Option<&CalibrationResult>) -> Result<()> {
    let quality = quality.ok_or_else(|| {
        Error::internal("BDT calibration quality is unavailable after calibration")
    })?;

    if quality.is_acceptable() {
        return Ok(());
    }

    Err(Error::Validation(format!(
        "BDT calibration quality is unacceptable: max_error_bps={:.6}, max_error_step={}, fallback_count={}, converged={}",
        quality.max_error_bps, quality.max_error_step, quality.fallback_count, quality.converged
    )))
}

/// Calculate option-adjusted spread for a bond given market price.
///
/// Convenience function using default tree configuration. This is a wrapper
/// around `TreePricer::new().calculate_oas()` for simple use cases.
///
/// # Arguments
///
/// * `bond` - The bond to calculate OAS for
/// * `market_context` - Market context with curves
/// * `as_of` - Valuation date
/// * `clean_price` - Market clean price as percentage of par
///
/// # Returns
///
/// OAS in basis points.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::calculate_oas;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// let oas_bp = calculate_oas(&bond, &market, as_of, 98.5)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn calculate_oas(
    bond: &Bond,
    market_context: &MarketContext,
    as_of: Date,
    clean_price: f64,
) -> Result<f64> {
    let calculator = TreePricer::with_config(super::config::bond_tree_config(bond)?);
    calculator.calculate_oas(bond, market_context, as_of, clean_price)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::trees::short_rate_tree::CalibrationResult;

    #[test]
    fn bdt_calibration_quality_rejects_fallbacks_and_large_error() {
        let poor = CalibrationResult {
            max_error_bps: 1.25,
            max_error_step: 4,
            fallback_count: 1,
            converged: true,
        };

        let err = validate_bdt_calibration_quality(Some(&poor))
            .expect_err("poor BDT calibration should be rejected");
        let msg = err.to_string();

        assert!(
            msg.contains("BDT calibration quality is unacceptable"),
            "unexpected error: {msg}"
        );
    }
}
