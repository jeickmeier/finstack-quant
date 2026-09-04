//! Pricing-engine components for fixed-income bonds.
//!
use super::super::super::super::types::Bond;
use super::bond_valuator::BondValuator;
use super::config::{TreeModelChoice, TreePricerConfig};
use super::lsmc::{price_bond_lsmc, BondLsmcConfig};
use crate::cashflow::primitives::is_cash_settlement_kind;
use crate::instruments::common_impl::pricing::rates_credit::build_daily_bond_rates_credit_targets;
use crate::instruments::pricing_overrides::{resolve_rates_credit_config, OasPriceBasis};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::HashMap;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::trees::hull_white_tree::{HullWhiteTree, HullWhiteTreeConfig};
use finstack_quant_models::trees::short_rate_tree::TreeCalibrationResult;
use finstack_quant_models::trees::two_factor_rates_credit::RatesCreditTree;
use finstack_quant_models::{short_rate_keys, ShortRateTree, ShortRateTreeConfig, TreeModel};
use std::borrow::Cow;

/// Tree-based pricer for bonds with embedded options and OAS calculations.
///
/// Provides methods for calculating option-adjusted spread (OAS) for bonds with
/// embedded call/put options.
///
/// # Model routing
///
/// Construction selects either the rates-only or joint rates-credit family.
/// The selected family never changes because a hazard curve happens to be
/// attached to the bond. Curve-naming conventions are never used to infer the
/// routing.
///
/// On the rates-credit path all model inputs come from
/// `resolve_rates_credit_config`, so the four volatility regimes are
/// selected purely by `ModelConfig` (`hw1f_sigma`, `hazard_volatility`, the
/// two mean reversions, and `rate_credit_correlation`), and an unset
/// volatility means a deterministic factor rather than an engine default.
/// The joint path reads only the canonical `hw1f_*` fields: the legacy
/// `implied_volatility` / `mean_reversion` channels are rejected there rather
/// than reinterpreted.
///
/// Direct PV and the OAS objective share one calibrated tree, so the zero-OAS
/// point and a direct valuation cannot disagree about the model.
///
/// On the joint path, positive `hw1f_sigma` makes future term and overnight
/// floating coupons re-fix from the sampled rate path. Known fixings remain
/// deterministic, and overnight observation conventions are replayed through
/// mid-period exercise. The legacy rates-only tree rejects stochastic floating
/// coupons because its valuator carries a projected scalar cashflow schedule.
pub struct TreePricer {
    /// Pricer configuration (tree steps, volatility, convergence settings)
    config: TreePricerConfig,
    /// Explicit rates-only or joint rates-credit family.
    model: BondTreeModel,
}

/// Factor family used by the bond tree kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BondTreeModel {
    /// Short-rate tree with no default factor.
    RatesOnly,
    /// Joint short-rate and hazard-rate tree.
    RatesCredit,
}

/// Direct tree price plus diagnostics from the same stochastic run, when one
/// was required.
#[derive(Debug, Clone)]
pub(crate) struct TreePriceOutcome {
    /// Raw present value in the bond currency.
    pub(crate) amount: f64,
    /// LSMC estimate and simulation counts for a stochastic rates-credit run.
    pub(crate) lsmc: Option<super::lsmc::BondLsmcResult>,
}

impl TreePriceOutcome {
    fn deterministic(amount: f64) -> Self {
        Self { amount, lsmc: None }
    }
}

impl TreePricer {
    fn pricing_bond_for_return_floor<'a>(
        &self,
        bond: &'a Bond,
        market_context: &MarketContext,
        as_of: Date,
    ) -> Result<Cow<'a, Bond>> {
        let model = &bond.instrument_pricing_overrides.model_config;
        let stochastic_rates_credit = self.model == BondTreeModel::RatesCredit
            && (model.hw1f_sigma.unwrap_or(0.0) > 0.0
                || model.hazard_volatility.unwrap_or(0.0) > 0.0);
        if bond.return_floor.is_some() && !stochastic_rates_credit {
            Ok(Cow::Owned(
                bond.effective_for_pricing(market_context, as_of)?,
            ))
        } else {
            Ok(Cow::Borrowed(bond))
        }
    }

    fn final_adjusted_payment_date(
        bond: &Bond,
        market_context: &MarketContext,
        origin: Date,
    ) -> Result<Option<Date>> {
        let schedule = bond.full_cashflow_schedule(market_context)?;
        if let Some(date) = schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.date > origin && is_cash_settlement_kind(flow.kind))
            .map(|flow| flow.date)
            .max()
        {
            return Ok(Some(date));
        }
        // Same-day scheduled cash is excluded by the public settlement
        // convention. A synthetic one-day grid is needed only to represent a
        // live same-day exercise decision, whose inclusive entitlement is
        // handled by the LSMC replay itself.
        if Self::has_exercise_on(bond, origin) {
            return origin.next_day().map(Some).ok_or_else(|| {
                Error::Validation(
                    "rates-credit same-day claim exceeds the supported date range".to_string(),
                )
            });
        }
        Ok(None)
    }

    fn has_exercise_on(bond: &Bond, date: Date) -> bool {
        let contractual = bond.call_put.as_ref().is_some_and(|schedule| {
            schedule
                .calls
                .iter()
                .chain(&schedule.puts)
                .any(|option| option.start_date <= date && date <= option.end_date)
        });
        let floored = bond.return_floor.as_ref().is_some_and(|floor| {
            if date >= bond.maturity {
                return false;
            }
            match floor.window {
                crate::instruments::fixed_income::bond::ProtectionWindow::Full => {
                    date > bond.issue_date
                }
                crate::instruments::fixed_income::bond::ProtectionWindow::From(start) => {
                    date >= start
                }
                crate::instruments::fixed_income::bond::ProtectionWindow::Between {
                    start,
                    end,
                } => start <= date && date <= end,
            }
        });
        contractual || floored
    }

    fn uses_path_dependent_lsmc(tree: &RatesCreditTree, bond: &Bond) -> bool {
        let has_options = bond
            .call_put
            .as_ref()
            .is_some_and(|schedule| schedule.has_options())
            || bond.return_floor.is_some();
        has_options && (tree.config.rate_vol > 0.0 || tree.config.hazard_vol > 0.0)
    }

    fn uses_sampled_bullet(tree: &RatesCreditTree, bond: &Bond) -> bool {
        let has_options = bond
            .call_put
            .as_ref()
            .is_some_and(|schedule| schedule.has_options())
            || bond.return_floor.is_some();
        !has_options && (tree.config.rate_vol > 0.0 || tree.config.hazard_vol > 0.0)
    }

    fn uses_deterministic_short_rate(&self) -> bool {
        match &self.config.tree_model {
            TreeModelChoice::HullWhite { sigma, .. }
            | TreeModelChoice::BlackDermanToy { sigma, .. } => *sigma == 0.0,
            TreeModelChoice::HoLee => self.config.volatility == 0.0,
        }
    }

    fn validate_selected_model_capabilities(&self, bond: &Bond) -> Result<()> {
        fn is_floating(spec: &crate::instruments::fixed_income::bond::CashflowSpec) -> bool {
            match spec {
                crate::instruments::fixed_income::bond::CashflowSpec::Floating(_) => true,
                crate::instruments::fixed_income::bond::CashflowSpec::Amortizing {
                    base, ..
                } => is_floating(base),
                _ => false,
            }
        }

        if self.model == BondTreeModel::RatesOnly
            && !self.uses_deterministic_short_rate()
            && is_floating(&bond.cashflow_spec)
        {
            return Err(Error::Validation(format!(
                "Bond '{}' selects stochastic rates-only tree pricing for a floating coupon, but that legacy tree preprojects coupons. Use 'rates_credit' for pathwise term and overnight reset replay, or set the rates-only volatility to zero.",
                bond.id.as_str()
            )));
        }
        Ok(())
    }

    fn deterministic_short_rate_valuator(
        bond: &Bond,
        market_context: &MarketContext,
        as_of: Date,
    ) -> Result<BondValuator> {
        let mut time_steps = BondValuator::mandatory_grid_times(bond, market_context, as_of)?;
        if time_steps.first().is_none_or(|time| time.abs() > 1.0e-12) {
            time_steps.insert(0, 0.0);
        }
        BondValuator::new_with_time_steps(bond.clone(), market_context, as_of, time_steps)
    }

    fn solve_deterministic_discount_curve_oas(
        &self,
        valuator: &BondValuator,
        dirty_target: f64,
    ) -> Result<f64> {
        let pricing_error = std::cell::RefCell::new(None);
        let objective = |oas_bp: f64| match valuator.price_deterministic_discount_curve(oas_bp) {
            Ok(model_price) => model_price - dirty_target,
            Err(error) => {
                let mut slot = pricing_error.borrow_mut();
                if slot.is_none() {
                    *slot = Some(error);
                }
                1.0e12
            }
        };
        let mut solver = BrentSolver::new()
            .tolerance(self.config.tolerance)
            .initial_bracket_size(self.config.initial_bracket_size_bp);
        solver.max_iterations = self.config.max_iterations;
        let continuous_oas_bp =
            solver.solve(objective, 0.0).map_err(|error| {
                match pricing_error.borrow_mut().take() {
                    Some(pricing) => Error::Validation(format!(
                        "deterministic OAS solve failed: {error}; first pricing error: {pricing}"
                    )),
                    None => error,
                }
            })?;
        Ok(self
            .config
            .oas_quote_compounding
            .quote_from_continuous_decimal(continuous_oas_bp / 10_000.0)
            * 10_000.0)
    }

    /// Resolve the explicitly required hazard curve for joint rates-credit
    /// pricing. Missing opt-in or market data is an error.
    fn resolve_required_hazard_curve(
        bond: &Bond,
        market_context: &MarketContext,
    ) -> Result<std::sync::Arc<finstack_quant_core::market_data::term_structures::HazardCurve>>
    {
        match bond.credit_curve_id.as_ref() {
            None => Err(finstack_quant_core::Error::Validation(format!(
                "Bond '{}' requires credit_curve_id under the rates_credit model",
                bond.id.as_str()
            ))),
            Some(hid) => market_context.get_hazard(hid.as_str()).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "Bond '{}' selects rates_credit via credit_curve_id \
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
    /// A `TreePricer` with default configuration (200 steps, 1% volatility).
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
            model: BondTreeModel::RatesOnly,
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
        Self {
            config,
            model: BondTreeModel::RatesOnly,
        }
    }

    /// Create an explicit joint rates-credit pricer.
    ///
    /// # Arguments
    ///
    /// * `config` - Tree, OAS, and Monte Carlo configuration used by the
    ///   joint short-rate and hazard-rate model.
    ///
    /// # Returns
    ///
    /// A pricer that requires the bond to identify a hazard curve through
    /// `credit_curve_id` and values embedded rights in the joint model.
    pub fn rates_credit(config: TreePricerConfig) -> Self {
        Self {
            config,
            model: BondTreeModel::RatesCredit,
        }
    }

    /// Price a bond with the configured tree at a fixed OAS in basis points.
    #[cfg(test)]
    pub(crate) fn price_at_oas(
        &self,
        bond: &Bond,
        market_context: &MarketContext,
        as_of: Date,
        oas_bp: f64,
    ) -> Result<f64> {
        Ok(self
            .price_at_oas_outcome(bond, market_context, as_of, oas_bp)?
            .amount)
    }

    /// Price once and retain any LSMC diagnostics without a second simulation.
    pub(crate) fn price_at_oas_outcome(
        &self,
        bond: &Bond,
        market_context: &MarketContext,
        as_of: Date,
        oas_bp: f64,
    ) -> Result<TreePriceOutcome> {
        self.validate_selected_model_capabilities(bond)?;
        let effective_bond = self.pricing_bond_for_return_floor(bond, market_context, as_of)?;
        let bond = effective_bond.as_ref();
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
        let hazard_curve = match self.model {
            BondTreeModel::RatesOnly => None,
            BondTreeModel::RatesCredit => {
                Some(Self::resolve_required_hazard_curve(bond, market_context)?)
            }
        };
        if as_of >= bond.maturity && hazard_curve.is_none() {
            // The contractual maturity can roll to a later business-day
            // payment date. A live maturity-date option is exercised before
            // that adjusted future payment, so retain the valuator's payment
            // and exercise ordering while applying OAS on the short interval.
            let flows = tree_bond.pricing_dated_cashflows(market_context, as_of)?;
            if flows.is_empty() {
                return Ok(TreePriceOutcome::deterministic(0.0));
            }
            return Self::deterministic_short_rate_valuator(tree_bond, market_context, as_of)?
                .price_deterministic_discount_curve(continuous_oas_bp)
                .map(TreePriceOutcome::deterministic);
        }
        let time_to_maturity = discount_curve.day_count().year_fraction(
            as_of,
            bond.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if time_to_maturity <= 0.0 && hazard_curve.is_none() {
            return Ok(TreePriceOutcome::deterministic(0.0));
        }

        if let Some(hc) = hazard_curve.as_ref() {
            let Some(horizon_date) =
                Self::final_adjusted_payment_date(tree_bond, market_context, as_of)?
            else {
                return Ok(TreePriceOutcome::deterministic(0.0));
            };
            let targets = build_daily_bond_rates_credit_targets(
                discount_curve.as_ref(),
                hc.as_ref(),
                as_of,
                horizon_date,
                self.config.tree_steps,
            )?;
            let effective_steps = targets.times.len() - 1;
            let cfg =
                resolve_rates_credit_config(&bond.instrument_pricing_overrides, effective_steps)?;
            let mut tree = RatesCreditTree::new(cfg);
            tree.calibrate(&targets)?;
            if Self::uses_path_dependent_lsmc(&tree, tree_bond)
                || Self::uses_sampled_bullet(&tree, tree_bond)
            {
                let lsmc_config = BondLsmcConfig::for_bond(tree_bond, continuous_oas_bp)?;
                let lsmc =
                    price_bond_lsmc(&tree, tree_bond, market_context, as_of, &lsmc_config, None)?;
                return Ok(TreePriceOutcome {
                    amount: lsmc.estimate.mean.amount(),
                    lsmc: Some(lsmc),
                });
            }
            let valuator = BondValuator::new_with_time_steps_and_day_count(
                tree_bond.clone(),
                market_context,
                as_of,
                targets.times.clone(),
                DayCount::Act365F,
            )?;
            return valuator
                .price_deterministic_rates_credit(&tree, continuous_oas_bp)
                .map(TreePriceOutcome::deterministic);
        }
        if self.uses_deterministic_short_rate() {
            return Self::deterministic_short_rate_valuator(tree_bond, market_context, as_of)?
                .price_deterministic_discount_curve(continuous_oas_bp)
                .map(TreePriceOutcome::deterministic);
        }

        let valuator = BondValuator::new(
            tree_bond.clone(),
            market_context,
            as_of,
            time_to_maturity,
            self.config.tree_steps,
        )?;

        let effective_model = self.config.tree_model.clone();

        let amount = match effective_model {
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
                tree.calibrate(discount_curve.as_ref(), time_to_maturity)?;
                validate_bdt_calibration_quality(tree.calibration_result())?;
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, tree.rate_at_node(0, 0)?);
                vars.insert(short_rate_keys::OAS, continuous_oas_bp);
                tree.price(vars, time_to_maturity, market_context, &valuator)
            }
            TreeModelChoice::HoLee => {
                let tree_config = ShortRateTreeConfig {
                    steps: self.config.tree_steps,
                    volatility: self.config.volatility,
                    mean_reversion: 0.0,
                    compounding: self.config.tree_compounding,
                    ..Default::default()
                };
                let mut tree = ShortRateTree::new(tree_config);
                tree.calibrate(discount_curve.as_ref(), time_to_maturity)?;
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, tree.rate_at_node(0, 0)?);
                vars.insert(short_rate_keys::OAS, continuous_oas_bp);
                tree.price(vars, time_to_maturity, market_context, &valuator)
            }
        }?;
        Ok(TreePriceOutcome::deterministic(amount))
    }

    /// Calculate option-adjusted spread (OAS) for a bond.
    ///
    /// Solves for the constant spread that equates the tree price to the market price.
    /// Uses Brent's method for root finding under the factor family selected
    /// when this pricer was constructed.
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
    pub fn calculate_oas(
        &self,
        bond: &Bond,
        market_context: &MarketContext,
        as_of: Date,
        clean_price_pct_of_par: f64,
    ) -> Result<f64> {
        use crate::instruments::fixed_income::bond::pricing::settlement::{
            quote_dirty_at_as_of, QuoteDateContext,
        };

        self.validate_selected_model_capabilities(bond)?;
        let effective_bond = self.pricing_bond_for_return_floor(bond, market_context, as_of)?;
        let bond = effective_bond.as_ref();

        // Dirty target must use accrued at the quote/settlement date to match
        // the market convention used by YTM, Z-spread, and the quote engine.
        let quote_ctx = QuoteDateContext::new(bond, market_context, as_of)?;
        let quote_date = quote_ctx.quote_date;
        let clean_target = clean_price_pct_of_par * bond.notional.amount() / 100.0;
        let dirty_target_at_quote = match self.config.oas_price_basis {
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
        let dirty_target = quote_dirty_at_as_of(
            bond,
            market_context,
            as_of,
            quote_date,
            dirty_target_at_quote,
        )?;
        let use_rates_credit = self.model == BondTreeModel::RatesCredit;
        let mut use_rates_credit_lsmc = false;
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
        let hazard_curve = match self.model {
            BondTreeModel::RatesOnly => None,
            BondTreeModel::RatesCredit => {
                Some(Self::resolve_required_hazard_curve(bond, market_context)?)
            }
        };
        // Align tree time basis with the discount curve's own day-count.
        if as_of >= bond.maturity && hazard_curve.is_none() {
            let flows = tree_bond.pricing_dated_cashflows(market_context, as_of)?;
            if flows.is_empty() {
                return Ok(0.0);
            }
            let valuator =
                Self::deterministic_short_rate_valuator(tree_bond, market_context, as_of)?;
            return self.solve_deterministic_discount_curve_oas(&valuator, dirty_target);
        }
        let dc_curve = discount_curve.day_count();
        let time_to_maturity = dc_curve.year_fraction(
            as_of,
            bond.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if time_to_maturity <= 0.0 && hazard_curve.is_none() {
            return Ok(0.0);
        }
        if let Some(hc) = hazard_curve.as_ref() {
            let Some(horizon_date) =
                Self::final_adjusted_payment_date(tree_bond, market_context, as_of)?
            else {
                return Ok(0.0);
            };
            let targets = build_daily_bond_rates_credit_targets(
                discount_curve.as_ref(),
                hc.as_ref(),
                as_of,
                horizon_date,
                self.config.tree_steps,
            )?;
            let effective_steps = targets.times.len() - 1;
            let cfg =
                resolve_rates_credit_config(&bond.instrument_pricing_overrides, effective_steps)?;
            let mut tree = RatesCreditTree::new(cfg);
            tree.calibrate(&targets)?;
            use_rates_credit_lsmc = Self::uses_path_dependent_lsmc(&tree, tree_bond)
                || Self::uses_sampled_bullet(&tree, tree_bond);
            rc_tree = Some(tree);
        }

        let effective_model = self.config.tree_model.clone();
        let deterministic_short_rate = !use_rates_credit && self.uses_deterministic_short_rate();

        let mut sr_tree: Option<ShortRateTree> = None;
        let mut hw_tree: Option<HullWhiteTree> = None;
        let mut valuation_steps = self.config.tree_steps;

        if !use_rates_credit && !deterministic_short_rate {
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
                        BondValuator::mandatory_grid_times(tree_bond, market_context, as_of)?;
                    hw_tree = Some(HullWhiteTree::calibrate_with_times(
                        hw_config,
                        discount_curve.as_ref(),
                        time_to_maturity,
                        &mandatory,
                    )?);
                }
                TreeModelChoice::HoLee => {
                    let tree_config = ShortRateTreeConfig {
                        steps: self.config.tree_steps,
                        volatility: self.config.volatility,
                        mean_reversion: 0.0,
                        ..Default::default()
                    };
                    let mut tree = ShortRateTree::new(tree_config);
                    tree.calibrate(discount_curve.as_ref(), time_to_maturity)?;
                    sr_tree = Some(tree);
                }
                TreeModelChoice::BlackDermanToy {
                    mean_reversion,
                    sigma,
                } => {
                    valuation_steps = self.effective_steps_for_model(
                        tree_bond,
                        as_of,
                        discount_curve.day_count(),
                        &effective_model,
                    );
                    let tree_config =
                        ShortRateTreeConfig::bdt(valuation_steps, *sigma, *mean_reversion)
                            .with_compounding(self.config.tree_compounding);
                    let mut tree = ShortRateTree::new(tree_config);
                    tree.calibrate(discount_curve.as_ref(), time_to_maturity)?;
                    validate_bdt_calibration_quality(tree.calibration_result())?;
                    sr_tree = Some(tree);
                }
            }
        }

        // The HW path prices on the tree's (possibly non-uniform) grid; all
        // other models use the uniform grid implied by `valuation_steps`.
        let valuator = if deterministic_short_rate {
            Self::deterministic_short_rate_valuator(tree_bond, market_context, as_of)?
        } else if let Some(ref tree) = hw_tree {
            BondValuator::new_with_time_steps(
                tree_bond.clone(),
                market_context,
                as_of,
                tree.time_grid().to_vec(),
            )?
        } else if let Some(ref tree) = rc_tree {
            BondValuator::new_with_time_steps_and_day_count(
                tree_bond.clone(),
                market_context,
                as_of,
                tree.time_grid()?.to_vec(),
                DayCount::Act365F,
            )?
        } else {
            BondValuator::new(
                tree_bond.clone(),
                market_context,
                as_of,
                time_to_maturity,
                valuation_steps,
            )?
        };

        // Get initial short rate for state variables (needed by short-rate tree)
        let initial_rate = if let Some(tree) = sr_tree.as_ref() {
            tree.rate_at_node(0, 0)?
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

        // Reprice on the tree calibrated above. Do not rebuild or recalibrate
        // the short-rate / rates+credit lattice inside the OAS solver loop.
        let objective_fn = |oas: f64| -> f64 {
            if use_rates_credit {
                if use_rates_credit_lsmc {
                    if let Some(tree) = rc_tree.as_ref() {
                        let mut config = match BondLsmcConfig::for_bond(tree_bond, oas) {
                            Ok(config) => config,
                            Err(error) => return record_error(error),
                        };
                        // The CI threshold applies to the final solved estimate,
                        // not noisy intermediate root trials.
                        config.target_ci_half_width = None;
                        return match price_bond_lsmc(
                            tree,
                            tree_bond,
                            market_context,
                            as_of,
                            &config,
                            None,
                        ) {
                            Ok(result) => result.estimate.mean.amount() - dirty_target,
                            Err(error) => record_error(error),
                        };
                    }
                    return record_error(finstack_quant_core::Error::internal(
                        "rates+credit LSMC OAS solve invoked without a calibrated tree",
                    ));
                }
                if let Some(tree) = rc_tree.as_ref() {
                    match valuator.price_deterministic_rates_credit(tree, oas) {
                        Ok(model_price) => model_price - dirty_target,
                        Err(e) => record_error(e),
                    }
                } else {
                    record_error(finstack_quant_core::Error::internal(
                        "rates+credit OAS solve invoked without a calibrated tree",
                    ))
                }
            } else if deterministic_short_rate {
                match valuator.price_deterministic_discount_curve(oas) {
                    Ok(model_price) => model_price - dirty_target,
                    Err(e) => record_error(e),
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
        if use_rates_credit_lsmc {
            if let Some(tree) = rc_tree.as_ref() {
                let final_config = BondLsmcConfig::for_bond(tree_bond, continuous_oas_bp)?;
                if final_config.target_ci_half_width.is_some() {
                    price_bond_lsmc(tree, tree_bond, market_context, as_of, &final_config, None)?;
                }
            }
        }
        Ok(self
            .config
            .oas_quote_compounding
            .quote_from_continuous_decimal(continuous_oas_bp / 10_000.0)
            * 10_000.0)
    }
}

impl Default for TreePricer {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_bdt_calibration_quality(quality: Option<&TreeCalibrationResult>) -> Result<()> {
    let quality = quality.ok_or_else(|| {
        Error::internal("BDT calibration quality is unavailable after calibration")
    })?;

    if quality.is_acceptable() {
        return Ok(());
    }

    Err(Error::Validation(format!(
        "BDT calibration quality is unacceptable: max_error_bp={:.6}, max_error_step={}, fallback_count={}, converged={}",
        quality.max_error_bp, quality.max_error_step, quality.fallback_count, quality.converged
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Attributes;
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule, CashflowSpec};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_models::trees::short_rate_tree::TreeCalibrationResult;

    #[test]
    fn bdt_calibration_quality_rejects_fallbacks_and_large_error() {
        let poor = TreeCalibrationResult {
            max_error_bp: 1.25,
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

    #[test]
    fn rates_only_tree_prices_maturity_exercise_before_rolled_cash_and_inverts_oas() {
        let issue = time::macros::date!(2024 - 03 - 01);
        let maturity = time::macros::date!(2025 - 03 - 01); // Saturday
        let as_of = maturity;
        let bullet = Bond::builder()
            .id("ROLLED_MATURITY_TREE".into())
            .notional(Money::new(100.0, Currency::USD))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Act365F)
                    .expect("finite coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .instrument_pricing_overrides(Default::default())
            .attributes(Attributes::new())
            .build()
            .expect("rolled-maturity test bond");
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(discount);
        let rolled_flows = bullet
            .pricing_dated_cashflows(&market, as_of)
            .expect("rolled cashflows");
        assert!(
            rolled_flows.iter().any(|(date, _)| *date > maturity),
            "test premise: final holder cash must roll beyond maturity"
        );

        let with_call = |price_pct_of_par| {
            let mut bond = bullet.clone();
            bond.call_put = Some(CallPutSchedule {
                calls: vec![CallPut {
                    start_date: maturity,
                    end_date: maturity,
                    price_pct_of_par,
                    make_whole: None,
                }],
                puts: Vec::new(),
            });
            bond
        };
        let with_put = |price_pct_of_par| {
            let mut bond = bullet.clone();
            bond.call_put = Some(CallPutSchedule {
                calls: Vec::new(),
                puts: vec![CallPut {
                    start_date: maturity,
                    end_date: maturity,
                    price_pct_of_par,
                    make_whole: None,
                }],
            });
            bond
        };
        let pricer = TreePricer::new();
        let bullet_pv = pricer
            .price_at_oas(&bullet, &market, as_of, 0.0)
            .expect("rolled bullet price");
        let callable_pv = pricer
            .price_at_oas(&with_call(90.0), &market, as_of, 0.0)
            .expect("maturity callable price");
        assert!(callable_pv > 0.0 && callable_pv < bullet_pv);
        let puttable_pv = pricer
            .price_at_oas(&with_put(200.0), &market, as_of, 0.0)
            .expect("maturity puttable price");
        assert!(puttable_pv > bullet_pv);

        // A far out-of-the-money call keeps the short rolled-payment interval
        // OAS-sensitive, allowing a clean round trip through the public solver.
        let high_strike = with_call(200.0);
        let expected_oas_bp = 125.0;
        let dirty = pricer
            .price_at_oas(&high_strike, &market, as_of, expected_oas_bp)
            .expect("known-OAS price");
        let quote =
            crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext::new(
                &high_strike,
                &market,
                as_of,
            )
            .expect("quote context");
        assert_eq!(quote.quote_date, as_of);
        let clean_pct =
            (dirty - quote.accrued_at_quote_date) / high_strike.notional.amount() * 100.0;
        let implied = pricer
            .calculate_oas(&high_strike, &market, as_of, clean_pct)
            .expect("rolled-maturity OAS");
        assert!(
            (implied - expected_oas_bp).abs() < 1.0e-5,
            "rolled-maturity OAS must round trip: implied={implied}, expected={expected_oas_bp}"
        );
    }
}
