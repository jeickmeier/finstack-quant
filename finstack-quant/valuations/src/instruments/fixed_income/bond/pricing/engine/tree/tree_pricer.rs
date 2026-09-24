//! Pricing-engine components for fixed-income bonds.
//!
use super::super::super::super::types::Bond;
use super::bond_valuator::BondValuator;
use super::config::{TreeModelChoice, TreePricerConfig};
use super::lsmc::{price_bond_lsmc, BondLsmcConfig};
use crate::cashflow::primitives::is_cash_settlement_kind;
use crate::instruments::common_impl::pricing::rates_credit::build_daily_bond_rates_credit_targets;
use crate::instruments::pricing_overrides::resolve_rates_credit_config;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
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
        // exercise_candidates`) but fall on the regular coupon grid, which
        // uniform steps already approximate well.
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

        // Search up to 4x the configured steps, capped at 1,000 but never
        // below the configured count.
        let max_steps = self
            .config
            .tree_steps
            .saturating_mul(4)
            .min(1000)
            .max(self.config.tree_steps);
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
        self.prepare(bond, market_context, as_of)?.price(oas_bp)
    }

    /// Calibrate the configured model for `bond` once and return a pricer
    /// that revalues it at any OAS without rebuilding the tree or valuator.
    ///
    /// Direct pricing and the OAS metric's root search both go through this,
    /// so they cannot disagree about the model.
    ///
    /// # Arguments
    ///
    /// * `bond` - Bond to value; a return floor is lowered into its issuer
    ///   call schedule first.
    /// * `market_context` - Market data holding the discount (or tree
    ///   discount) curve and, for the rates-credit model, the hazard curve.
    /// * `as_of` - Valuation date at the tree root.
    pub(crate) fn prepare<'a>(
        &self,
        bond: &Bond,
        market_context: &'a MarketContext,
        as_of: Date,
    ) -> Result<OasPricer<'a>> {
        self.validate_selected_model_capabilities(bond)?;
        let effective_bond = self.pricing_bond_for_return_floor(bond, market_context, as_of)?;
        let bond = effective_bond.as_ref();
        let tree_discount_curve_id = self
            .config
            .tree_discount_curve_id
            .as_ref()
            .unwrap_or(&bond.discount_curve_id);
        let discount_curve = market_context.get_discount(tree_discount_curve_id.as_str())?;
        let mut tree_bond = bond.clone();
        tree_bond.discount_curve_id = tree_discount_curve_id.clone();
        let hazard_curve = match self.model {
            BondTreeModel::RatesOnly => None,
            BondTreeModel::RatesCredit => {
                Some(Self::resolve_required_hazard_curve(bond, market_context)?)
            }
        };
        let pricer = |model| OasPricer {
            market: market_context,
            as_of,
            quote_compounding: self.config.oas_quote_compounding,
            bond: tree_bond.clone(),
            model,
        };

        if as_of >= bond.maturity && hazard_curve.is_none() {
            // The contractual maturity can roll to a later business-day
            // payment date. A live maturity-date option is exercised before
            // that adjusted future payment, so retain the valuator's payment
            // and exercise ordering while applying OAS on the short interval.
            let flows = tree_bond.pricing_dated_cashflows(market_context, as_of)?;
            if flows.is_empty() {
                return Ok(pricer(PreparedTree::Zero));
            }
            return Ok(pricer(PreparedTree::Deterministic(
                Self::deterministic_short_rate_valuator(&tree_bond, market_context, as_of)?,
            )));
        }
        let time_to_maturity = discount_curve.day_count().year_fraction(
            as_of,
            bond.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if time_to_maturity <= 0.0 && hazard_curve.is_none() {
            return Ok(pricer(PreparedTree::Zero));
        }

        if let Some(hc) = hazard_curve.as_ref() {
            let Some(horizon_date) =
                Self::final_adjusted_payment_date(&tree_bond, market_context, as_of)?
            else {
                return Ok(pricer(PreparedTree::Zero));
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
            if Self::uses_path_dependent_lsmc(&tree, &tree_bond)
                || Self::uses_sampled_bullet(&tree, &tree_bond)
            {
                return Ok(pricer(PreparedTree::RatesCreditLsmc(tree)));
            }
            let valuator = BondValuator::new_with_time_steps_and_day_count(
                tree_bond.clone(),
                market_context,
                as_of,
                targets.times.clone(),
                DayCount::Act365F,
            )?;
            return Ok(pricer(PreparedTree::RatesCredit(tree, valuator)));
        }
        if self.uses_deterministic_short_rate() {
            return Ok(pricer(PreparedTree::Deterministic(
                Self::deterministic_short_rate_valuator(&tree_bond, market_context, as_of)?,
            )));
        }

        let prepared = match self.config.tree_model.clone() {
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
                    BondValuator::mandatory_grid_times(&tree_bond, market_context, as_of)?;
                let tree = HullWhiteTree::calibrate_with_times(
                    hw_config,
                    discount_curve.as_ref(),
                    time_to_maturity,
                    &mandatory,
                )?;
                let valuator = BondValuator::new_with_time_steps(
                    tree_bond.clone(),
                    market_context,
                    as_of,
                    tree.time_grid().to_vec(),
                )?;
                PreparedTree::HullWhite(tree, valuator)
            }
            model @ TreeModelChoice::BlackDermanToy {
                mean_reversion,
                sigma,
            } => {
                let tree_steps = self.effective_steps_for_model(
                    &tree_bond,
                    as_of,
                    discount_curve.day_count(),
                    &model,
                );
                let tree_config = ShortRateTreeConfig::bdt(tree_steps, sigma, mean_reversion)
                    .with_compounding(self.config.tree_compounding);
                let mut tree = ShortRateTree::new(tree_config);
                tree.calibrate(discount_curve.as_ref(), time_to_maturity)?;
                validate_bdt_calibration_quality(tree.calibration_result())?;
                let valuator = BondValuator::new(
                    tree_bond.clone(),
                    market_context,
                    as_of,
                    time_to_maturity,
                    tree_steps,
                )?;
                PreparedTree::ShortRate {
                    tree,
                    valuator,
                    time_to_maturity,
                }
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
                let valuator = BondValuator::new(
                    tree_bond.clone(),
                    market_context,
                    as_of,
                    time_to_maturity,
                    self.config.tree_steps,
                )?;
                PreparedTree::ShortRate {
                    tree,
                    valuator,
                    time_to_maturity,
                }
            }
        };
        Ok(pricer(prepared))
    }
}

/// Model state calibrated once by [`TreePricer::prepare`].
enum PreparedTree {
    /// Nothing left to value (expired bond or no remaining flows).
    Zero,
    /// Deterministic short rate: the discount curve plus OAS.
    Deterministic(BondValuator),
    /// Joint rates-credit tree priced by path-dependent LSMC.
    RatesCreditLsmc(RatesCreditTree),
    /// Joint rates-credit lattice with backward induction.
    RatesCredit(RatesCreditTree, BondValuator),
    /// Hull-White trinomial tree through the bond's mandatory dates.
    HullWhite(HullWhiteTree, BondValuator),
    /// BDT or Ho-Lee short-rate tree on a uniform grid.
    ShortRate {
        tree: ShortRateTree,
        valuator: BondValuator,
        time_to_maturity: f64,
    },
}

/// A bond with its tree model calibrated once, revalued at any OAS.
///
/// Built by [`TreePricer::prepare`]; the OAS metric's root search reprices
/// through one instance instead of recalibrating per trial.
pub(crate) struct OasPricer<'a> {
    market: &'a MarketContext,
    as_of: Date,
    quote_compounding: crate::instruments::pricing_overrides::OasQuoteCompounding,
    bond: Bond,
    model: PreparedTree,
}

impl OasPricer<'_> {
    /// Value the bond at a quoted OAS.
    ///
    /// # Arguments
    ///
    /// * `oas_bp` - OAS in basis points on the configured quote compounding;
    ///   it is converted to a continuous shift of the short rate.
    pub(crate) fn price(&self, oas_bp: f64) -> Result<TreePriceOutcome> {
        let oas = self
            .quote_compounding
            .continuous_from_quote_decimal(oas_bp / 10_000.0)
            * 10_000.0;
        let amount = match &self.model {
            PreparedTree::Zero => 0.0,
            PreparedTree::Deterministic(valuator) => {
                valuator.price_deterministic_discount_curve(oas)?
            }
            PreparedTree::RatesCreditLsmc(tree) => {
                let config = BondLsmcConfig::for_bond(&self.bond, oas)?;
                let lsmc =
                    price_bond_lsmc(tree, &self.bond, self.market, self.as_of, &config, None)?;
                return Ok(TreePriceOutcome {
                    amount: lsmc.estimate.mean.amount(),
                    lsmc: Some(lsmc),
                });
            }
            PreparedTree::RatesCredit(tree, valuator) => {
                valuator.price_deterministic_rates_credit(tree, oas)?
            }
            PreparedTree::HullWhite(tree, valuator) => valuator.price_with_hw_tree(tree, oas)?,
            PreparedTree::ShortRate {
                tree,
                valuator,
                time_to_maturity,
            } => {
                let mut vars = HashMap::<&'static str, f64>::default();
                vars.insert(short_rate_keys::SHORT_RATE, tree.rate_at_node(0, 0)?);
                vars.insert(short_rate_keys::OAS, oas);
                tree.price(vars, *time_to_maturity, self.market, valuator)?
            }
        };
        Ok(TreePriceOutcome::deterministic(amount))
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
            .notional(Money::from((100_i64, Currency::USD)))
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
        let mut quoted = high_strike;
        quoted
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(clean_pct);
        let implied = crate::instruments::common_impl::traits::Instrument::price_with_metrics(
            &quoted,
            &market,
            as_of,
            &[crate::metrics::MetricId::Oas],
            crate::instruments::PricingOptions::default().with_model(crate::pricer::ModelKey::Tree),
        )
        .expect("rolled-maturity OAS")
        .measures["oas"]
            * 10_000.0;
        assert!(
            (implied - expected_oas_bp).abs() < 1.0e-5,
            "rolled-maturity OAS must round trip: implied={implied}, expected={expected_oas_bp}"
        );
    }
}
