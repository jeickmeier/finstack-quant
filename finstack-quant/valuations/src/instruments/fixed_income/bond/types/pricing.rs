//! Bond pricing methods, validation, and cashflow projection.

use crate::instruments::common_impl::validation;
use finstack_quant_core::Result;
use rust_decimal::prelude::ToPrimitive;

use super::definitions::Bond;
use super::CashflowSpec;

impl Bond {
    /// Whether instrument-owned inputs request the joint rates-credit family
    /// on the native default path, including legacy fields that the joint
    /// resolver must reject rather than silently ignore.
    fn has_rates_credit_factor_inputs(&self) -> bool {
        let model = &self.instrument_pricing_overrides.model_config;
        model.hw1f_sigma.is_some()
            || model.hw1f_sigma_schedule.is_some()
            || model.hw1f_mean_reversion.is_some()
            || model.hazard_volatility.is_some()
            || model.hazard_mean_reversion.is_some()
            || model.rate_credit_correlation.is_some()
            || model.mean_reversion.is_some()
            || self
                .instrument_pricing_overrides
                .market_quotes
                .implied_volatility
                .is_some()
    }

    /// Whether an input specifically requests the credit factor rather than a
    /// rates-only tree configuration.
    fn configured_credit_factor_input(&self) -> Option<&'static str> {
        let model = &self.instrument_pricing_overrides.model_config;
        [
            ("hazard_volatility", model.hazard_volatility.is_some()),
            (
                "hazard_mean_reversion",
                model.hazard_mean_reversion.is_some(),
            ),
            (
                "rate_credit_correlation",
                model.rate_credit_correlation.is_some(),
            ),
        ]
        .into_iter()
        .find_map(|(field, configured)| configured.then_some(field))
    }

    /// Whether the contractual bond carries issuer or holder exercise rights.
    pub(crate) fn has_exercise_rights(&self) -> bool {
        self.return_floor.is_some()
            || self
                .call_put
                .as_ref()
                .is_some_and(super::definitions::CallPutSchedule::has_options)
    }

    /// Resolve the instrument-native default without constraining an explicit
    /// registry model selection.
    pub(crate) fn default_pricing_model(&self) -> crate::pricer::ModelKey {
        use crate::pricer::ModelKey;

        if self.configured_credit_factor_input().is_some()
            || (self.credit_curve_id.is_some()
                && (self.has_exercise_rights() || self.has_rates_credit_factor_inputs()))
        {
            ModelKey::RatesCredit
        } else if self.credit_curve_id.is_some() {
            ModelKey::HazardRate
        } else if self.has_exercise_rights() {
            ModelKey::Tree
        } else {
            ModelKey::Discounting
        }
    }

    fn validate_model_contract(
        &self,
        model: crate::pricer::ModelKey,
        curves: &finstack_quant_core::market_data::context::MarketContext,
    ) -> Result<()> {
        use crate::instruments::fixed_income::bond::pricing::engine::hazard::HazardBondEngine;
        use crate::pricer::ModelKey;

        if model == ModelKey::RatesCredit && self.credit_curve_id.is_none() {
            if let Some(field) = self.configured_credit_factor_input() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Bond '{}' configures {field} but has no credit_curve_id; add the hazard curve identifier for 'rates_credit' pricing or explicitly select a rates-only model",
                    self.id
                )));
            }
        }

        match model {
            ModelKey::Discounting | ModelKey::HazardRate if self.has_exercise_rights() => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Bond '{}' has embedded call, put, or return-floor rights; model '{}' is non-callable. Use 'tree' for rates-only optional pricing or 'rates_credit' for joint rates-credit optional pricing.",
                    self.id,
                    model.as_str()
                )));
            }
            ModelKey::HazardRate | ModelKey::RatesCredit => {
                HazardBondEngine::require_hazard_curve(self, curves)?;
            }
            ModelKey::Discounting | ModelKey::Tree => {}
            _ => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Bond '{}' does not support pricing model '{}'",
                    self.id,
                    model.as_str()
                )));
            }
        }
        Ok(())
    }

    /// Price one explicit bond model at a supplied OAS without consulting any
    /// price-driving quote stored on the instrument.
    pub(crate) fn price_at_oas_for_model_outcome(
        &self,
        model: crate::pricer::ModelKey,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        oas_quote_decimal: f64,
    ) -> Result<crate::instruments::fixed_income::bond::pricing::engine::tree::TreePriceOutcome>
    {
        use crate::instruments::fixed_income::bond::pricing::engine::{
            discount::BondEngine,
            hazard::HazardBondEngine,
            tree::{bond_tree_config, TreePriceOutcome, TreePricer},
        };
        use crate::pricer::ModelKey;

        self.validate_model_contract(model, curves)?;
        match model {
            ModelKey::Discounting => Ok(TreePriceOutcome {
                amount: BondEngine::price_with_oas(self, curves, as_of, oas_quote_decimal)?,
                lsmc: None,
            }),
            ModelKey::HazardRate => Ok(TreePriceOutcome {
                amount: HazardBondEngine::price_raw_with_oas(
                    self,
                    curves,
                    as_of,
                    oas_quote_decimal,
                )?,
                lsmc: None,
            }),
            ModelKey::Tree => TreePricer::with_config(bond_tree_config(self)?)
                .price_at_oas_outcome(self, curves, as_of, oas_quote_decimal * 10_000.0),
            ModelKey::RatesCredit => TreePricer::rates_credit(bond_tree_config(self)?)
                .price_at_oas_outcome(self, curves, as_of, oas_quote_decimal * 10_000.0),
            _ => Err(finstack_quant_core::Error::Validation(format!(
                "Bond '{}' does not support pricing model '{}'",
                self.id,
                model.as_str()
            ))),
        }
    }

    /// Price the bond through one explicit model, retaining stochastic
    /// diagnostics from the same run that produced PV.
    pub(crate) fn price_for_model_outcome(
        &self,
        model: crate::pricer::ModelKey,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> Result<crate::instruments::fixed_income::bond::pricing::engine::tree::TreePriceOutcome>
    {
        use crate::instruments::fixed_income::bond::pricing::engine::tree::TreePriceOutcome;
        use crate::instruments::fixed_income::bond::pricing::{quote_conversions, settlement};
        use crate::pricer::ModelKey;

        self.validate_model_contract(model, curves)?;
        self.instrument_pricing_overrides.market_quotes.validate()?;

        // The legacy scenario spread shock is defined only on the scalar
        // discounting model. Other models require a curve or OAS bump so their
        // own state and recovery mechanics remain intact.
        if let Some(shock_bp) = self.scenario_pricing_overrides.scenario_spread_shock_bp {
            if model != ModelKey::Discounting {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "scenario_spread_shock_bp is supported only by the 'discounting' bond model; selected '{}'",
                    model.as_str()
                )));
            }
            if self
                .instrument_pricing_overrides
                .market_quotes
                .has_non_z_price_driver()
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "scenario_spread_shock_bp on bond '{}' conflicts with a price-pinning quote override; remove the quote or quote via quoted_z_spread so the shock can compose additively",
                    self.id
                )));
            }
            let z_eff = self
                .instrument_pricing_overrides
                .market_quotes
                .quoted_z_spread
                .unwrap_or(0.0)
                + shock_bp * 1e-4;
            let dirty_at_quote =
                quote_conversions::price_from_z_spread(self, curves, as_of, z_eff)?;
            let quote_date = settlement::settlement_date(self, as_of)?;
            let amount =
                settlement::quote_dirty_at_as_of(self, curves, as_of, quote_date, dirty_at_quote)?;
            return Ok(TreePriceOutcome { amount, lsmc: None });
        }

        // OAS is a model input. Every supported model applies it inside its
        // own kernel; no model is silently substituted.
        if let Some(oas) = self.instrument_pricing_overrides.market_quotes.quoted_oas {
            return self.price_at_oas_for_model_outcome(model, curves, as_of, oas);
        }

        // Price, yield, and spread quote forms define settlement economics.
        // They pin the quoted value independently of the selected projection
        // model, while risk metrics clear the quote and retain this dispatch.
        if let Some(dirty_at_quote) = quote_conversions::settlement_dirty_from_quote_overrides(
            self, curves, as_of, model, None,
        )? {
            let quote_date = settlement::settlement_date(self, as_of)?;
            let amount =
                settlement::quote_dirty_at_as_of(self, curves, as_of, quote_date, dirty_at_quote)?;
            return Ok(TreePriceOutcome { amount, lsmc: None });
        }

        self.price_at_oas_for_model_outcome(model, curves, as_of, 0.0)
    }

    /// Price the bond through one explicit model as an unrounded scalar.
    pub(crate) fn price_for_model_raw(
        &self,
        model: crate::pricer::ModelKey,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> Result<f64> {
        Ok(self.price_for_model_outcome(model, curves, as_of)?.amount)
    }

    /// Compute the bond's unshocked pricing-policy value without entering the
    /// public instrument lifecycle.
    ///
    /// This is the instrument-native default path used by `base_value` and
    /// `base_value_raw`. Explicit registry pricing bypasses inference and calls
    /// [`Self::price_for_model_raw`] with the caller-selected model.
    pub(crate) fn base_value_raw_impl(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> Result<f64> {
        self.price_for_model_raw(self.default_pricing_model(), curves, as_of)
    }

    /// Pricing-oriented dated cashflows: coupons, amortization, and positive
    /// notional (redemption). Negative notionals (initial draw) and pure PIK
    /// accretion are excluded because they are not discounted receipt flows.
    ///
    /// When the bond has an ex-coupon convention and `as_of` falls inside the
    /// ex-coupon window of a coupon, that coupon is excluded: a buyer settling
    /// in the ex-window does not receive the imminent coupon (market standard,
    /// e.g. UK gilts). Accrued interest is correspondingly negative in that
    /// window (see [`crate::cashflow::accrued_interest_amount`]).
    ///
    /// Internal pricing engines (discount, hazard, spread solvers) should use
    /// this instead of the public `CashflowProvider::dated_cashflows` which
    /// now returns the full signed canonical schedule.
    ///
    /// Cashflows dated exactly on `as_of` are **excluded** (strict
    /// `date > as_of`): a buyer settling on `as_of` does not receive that
    /// day's payment (settlement convention). This matches the tree and YTM
    /// engines, which always filtered `> as_of`.
    pub(crate) fn pricing_dated_cashflows(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> Result<
        Vec<(
            finstack_quant_core::dates::Date,
            finstack_quant_core::money::Money,
        )>,
    > {
        self.pricing_dated_cashflows_at(curves, as_of, as_of)
    }

    /// [`Bond::pricing_dated_cashflows`] with an explicit coupon-entitlement date.
    ///
    /// Market convention (e.g. UK gilts/DMO) determines coupon entitlement by
    /// whether the **settlement** date falls on/after the ex-dividend date, not
    /// the trade/valuation date. Quote-derived metrics (YTM, YTW, Z-spread, DM,
    /// I-spread, ASW) therefore pass the quote/settlement date as
    /// `entitlement_date` so that a trade whose settlement lands inside the
    /// ex-window drops the imminent coupon, consistent with the negative
    /// accrued interest computed at the same date.
    ///
    /// # Arguments
    ///
    /// * `curves` - Market context used to build the full cashflow schedule
    ///   (floating-rate projection, amortization, custom schedules).
    /// * `as_of` - Valuation date; flows dated on or before it are excluded
    ///   (strict `date > as_of`).
    /// * `entitlement_date` - Date at which coupon entitlement is tested
    ///   against each coupon's ex-date; the quote/settlement date for market
    ///   quotes, `as_of` for model PV.
    pub(crate) fn pricing_dated_cashflows_at(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        entitlement_date: finstack_quant_core::dates::Date,
    ) -> Result<
        Vec<(
            finstack_quant_core::dates::Date,
            finstack_quant_core::money::Money,
        )>,
    > {
        let schedule = self.full_cashflow_schedule(curves)?;
        self.pricing_dated_cashflows_from_schedule(&schedule, as_of, entitlement_date)
    }

    /// Filter a prebuilt schedule into holder-view pricing cashflows.
    ///
    /// Reuses one materialized schedule when multiple entitlement dates must
    /// be compared, avoiding repeated projection and schedule construction.
    pub(crate) fn pricing_dated_cashflows_from_schedule(
        &self,
        schedule: &crate::cashflow::builder::CashFlowSchedule,
        as_of: finstack_quant_core::dates::Date,
        entitlement_date: finstack_quant_core::dates::Date,
    ) -> Result<
        Vec<(
            finstack_quant_core::dates::Date,
            finstack_quant_core::money::Money,
        )>,
    > {
        self.pricing_dated_cashflows_from_schedule_with_boundary(
            schedule,
            as_of,
            entitlement_date,
            false,
        )
    }

    /// Filter a prebuilt schedule for an exercise valuation that occurs on
    /// `as_of`, retaining holder cash payable immediately before that decision.
    pub(crate) fn pricing_dated_cashflows_from_schedule_inclusive(
        &self,
        schedule: &crate::cashflow::builder::CashFlowSchedule,
        as_of: finstack_quant_core::dates::Date,
        entitlement_date: finstack_quant_core::dates::Date,
    ) -> Result<
        Vec<(
            finstack_quant_core::dates::Date,
            finstack_quant_core::money::Money,
        )>,
    > {
        self.pricing_dated_cashflows_from_schedule_with_boundary(
            schedule,
            as_of,
            entitlement_date,
            true,
        )
    }

    fn pricing_dated_cashflows_from_schedule_with_boundary(
        &self,
        schedule: &crate::cashflow::builder::CashFlowSchedule,
        as_of: finstack_quant_core::dates::Date,
        entitlement_date: finstack_quant_core::dates::Date,
        include_as_of: bool,
    ) -> Result<
        Vec<(
            finstack_quant_core::dates::Date,
            finstack_quant_core::money::Money,
        )>,
    > {
        use finstack_quant_core::cashflow::CFKind;

        let ex_coupon = self.accrual_config().ex_coupon;
        let mut flows = Vec::with_capacity(schedule.get_flows().len());
        for cf in schedule.get_flows() {
            let after_boundary = cf.date > as_of || (include_as_of && cf.date == as_of);
            let keep = after_boundary
                && cf.kind != CFKind::Pik
                && !(cf.kind == CFKind::Notional && cf.amount.amount() < 0.0);
            if !keep {
                continue;
            }
            if cf.kind.is_interest_like() {
                if let Some(rule) = &ex_coupon {
                    let ex_date = rule.ex_date(cf.date)?;
                    if entitlement_date >= ex_date && entitlement_date < cf.date {
                        continue;
                    }
                }
            }
            flows.push((cf.date, cf.amount));
        }
        Ok(flows)
    }

    /// Validate all bond parameters.
    ///
    /// Performs comprehensive validation of the bond instrument:
    /// - Issue date must be before maturity date
    /// - Notional must be positive
    /// - Coupon rate must be non-negative (for fixed-rate bonds)
    /// - Call/put prices must be positive
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` with a descriptive message if any validation fails.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_valuations::instruments::Bond;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let bond = Bond::example()?;
    /// bond.validate()?; // Validates all parameters
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> Result<()> {
        self.instrument_pricing_overrides.validate()?;
        self.metric_pricing_overrides.validate()?;
        self.scenario_pricing_overrides.validate()?;
        validation::validate_date_range_strict_with(
            self.issue_date,
            self.maturity,
            |start, end| {
                format!(
                    "Bond issue date ({}) must be before maturity date ({})",
                    start, end
                )
            },
        )?;

        validation::validate_money_finite(self.notional, "bond notional")?;
        validation::validate_money_gt_with(self.notional, 0.0, |amount| {
            format!("Bond notional must be positive, got {}", amount)
        })?;

        // Validate coupon rate for fixed-rate bonds (including amortizing with fixed base)
        Self::validate_coupon_rate(&self.cashflow_spec)?;

        // Validate call/put prices and exercise date ranges.
        if let Some(ref call_put) = self.call_put {
            call_put.validate_for_life(self.issue_date, self.maturity, "Bond")?;
        }
        if let Some(ref return_floor) = self.return_floor {
            return_floor.validate()?;
            return_floor.issue_price.resolve(self.notional)?;
        }

        Ok(())
    }

    /// Returns `true` when coupon cashflows depend on forward curve projection (floating FRNs).
    ///
    /// True for [`CashflowSpec::Floating`] and for [`CashflowSpec::Amortizing`] when the
    /// base specification is floating.
    pub fn has_floating_coupons(&self) -> bool {
        match &self.cashflow_spec {
            CashflowSpec::Floating(_) => true,
            CashflowSpec::Amortizing { base, .. } => {
                matches!(base.as_ref(), CashflowSpec::Floating(_))
            }
            _ => false,
        }
    }

    /// Recursively validate that fixed coupon rates are non-negative.
    ///
    /// Handles `Fixed`, `Floating` (no coupon rate to validate), and
    /// `Amortizing` (recurses into the base spec).
    fn validate_coupon_rate(spec: &CashflowSpec) -> Result<()> {
        match spec {
            CashflowSpec::Fixed(s) => {
                let rate = s.rate.to_f64().unwrap_or(0.0);
                if rate < 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Bond fixed coupon rate must be non-negative, got {}",
                        rate
                    )));
                }
            }
            CashflowSpec::StepUp(s) => {
                let rate = s.initial_rate.to_f64().unwrap_or(0.0);
                if rate < 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Bond step-up initial coupon rate must be non-negative, got {}",
                        rate
                    )));
                }
                for (_, step_rate) in &s.step_schedule {
                    let r = step_rate.to_f64().unwrap_or(0.0);
                    if r < 0.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "Bond step-up coupon rate must be non-negative, got {}",
                            r
                        )));
                    }
                }
            }
            CashflowSpec::Amortizing { base, .. } => {
                Self::validate_coupon_rate(base)?;
            }
            CashflowSpec::Floating(_) => {
                // No fixed coupon rate to validate
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::{
        CallPut, CallPutSchedule, IssuePrice, MakeWholeSpec, ReturnFloorSpec,
    };
    use crate::instruments::Instrument;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::math::piecewise::PiecewiseConstantCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_models::trees::two_factor_rates_credit::KAPPA_MAX;
    use time::macros::date;

    fn ex_coupon_bond() -> Bond {
        let mut bond = Bond::fixed(
            "EX-FLOWS",
            Money::new(100.0, Currency::USD),
            finstack_quant_core::types::Rate::from_decimal(0.05),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        bond.settlement_convention = Some(super::super::BondSettlementConvention {
            ex_coupon_days: 7,
            ..Default::default()
        });
        bond
    }

    fn deterministic_credit_bond_and_market(
    ) -> (Bond, MarketContext, finstack_quant_core::dates::Date) {
        let as_of = date!(2025 - 01 - 01);
        let mut bond = Bond::fixed(
            "DETERMINISTIC-CREDIT-BULLET",
            Money::new(100.0, Currency::USD),
            finstack_quant_core::types::Rate::from_decimal(0.05),
            as_of,
            date!(2027 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid deterministic credit bond");
        bond.credit_curve_id = Some("ACME-HZD".into());
        let discount = DiscountCurve::flat("USD-OIS", as_of, 0.03).expect("discount curve");
        let hazard = HazardCurve::flat("ACME-HZD", as_of, 0.02, 0.4).expect("hazard curve");
        (
            bond,
            MarketContext::new().insert(discount).insert(hazard),
            as_of,
        )
    }

    #[test]
    fn pricing_flows_exclude_coupon_inside_ex_window() {
        let bond = ex_coupon_bond();
        let coupon_date = date!(2025 - 07 - 01);
        let market = MarketContext::new();

        // Inside the ex-window (5 days before the coupon): the imminent coupon
        // is not a buyer flow.
        let as_of = coupon_date - time::Duration::days(5);
        let flows = bond.pricing_dated_cashflows(&market, as_of).expect("flows");
        assert!(
            !flows.iter().any(|(d, _)| *d == coupon_date),
            "ex-period coupon must be excluded from buyer flows"
        );

        // Outside the ex-window (8 days before): the coupon is still a buyer flow.
        let as_of = coupon_date - time::Duration::days(8);
        let flows = bond.pricing_dated_cashflows(&market, as_of).expect("flows");
        assert!(
            flows.iter().any(|(d, _)| *d == coupon_date),
            "cum-coupon flows must include the next coupon"
        );
    }

    /// Entitlement is a settlement-date rule: a trade whose settlement lands
    /// inside the ex-window must drop the coupon even when the trade date is
    /// still cum-coupon.
    #[test]
    fn entitlement_date_controls_ex_window_exclusion() {
        let bond = ex_coupon_bond();
        let coupon_date = date!(2025 - 07 - 01);
        let market = MarketContext::new();

        // Trade 8 days before the coupon (cum at trade date), settling 5 days
        // before (inside the 7-day ex-window): the coupon is not a buyer flow.
        let trade_date = coupon_date - time::Duration::days(8);
        let settle_date = coupon_date - time::Duration::days(5);
        let flows = bond
            .pricing_dated_cashflows_at(&market, trade_date, settle_date)
            .expect("flows");
        assert!(
            !flows.iter().any(|(d, _)| *d == coupon_date),
            "coupon must be excluded when the entitlement (settlement) date is in the ex-window"
        );

        // Same trade date with entitlement also at the trade date keeps the coupon.
        let flows = bond
            .pricing_dated_cashflows_at(&market, trade_date, trade_date)
            .expect("flows");
        assert!(
            flows.iter().any(|(d, _)| *d == coupon_date),
            "coupon must be included when the entitlement date is cum-coupon"
        );
    }

    #[test]
    fn validation_rejects_non_finite_call_and_make_whole_quotes() {
        let mut bond = ex_coupon_bond();
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.01);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: date!(2027 - 01 - 01),
                end_date: date!(2027 - 01 - 01),
                price_pct_of_par: f64::NAN,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        assert!(bond
            .validate()
            .expect_err("NaN call price must fail")
            .to_string()
            .contains("finite"));

        let call = &mut bond.call_put.as_mut().expect("schedule").calls[0];
        call.price_pct_of_par = 101.0;
        call.make_whole = Some(MakeWholeSpec {
            reference_curve_id: CurveId::new("USD-TREASURY"),
            spread_bp: f64::INFINITY,
        });
        assert!(bond
            .validate()
            .expect_err("infinite make-whole spread must fail")
            .to_string()
            .contains("make-whole"));
    }

    #[test]
    fn bond_validation_enforces_return_floor_and_resolved_issue_price() {
        assert!(
            Money::try_new(f64::NAN, Currency::USD).is_err(),
            "Money rejects NaN before it can enter IssuePrice::Amount"
        );
        let base = ex_coupon_bond();
        let invalid = [
            ReturnFloorSpec::moic(f64::NAN),
            ReturnFloorSpec::moic(1.0).issue_price(IssuePrice::PctOfPar(f64::NAN)),
            ReturnFloorSpec::moic(1.0).issue_price(IssuePrice::PctOfPar(0.0)),
            ReturnFloorSpec::moic(1.0).issue_price(IssuePrice::PctOfPar(-1.0)),
            ReturnFloorSpec::moic(1.0)
                .issue_price(IssuePrice::Amount(Money::new(0.0, Currency::USD))),
            ReturnFloorSpec::moic(1.0)
                .issue_price(IssuePrice::Amount(Money::new(-1.0, Currency::USD))),
            ReturnFloorSpec::moic(1.0)
                .issue_price(IssuePrice::Amount(Money::new(100.0, Currency::EUR))),
        ];
        for return_floor in invalid {
            let mut bond = base.clone();
            bond.return_floor = Some(return_floor);
            assert!(
                bond.validate().is_err(),
                "invalid return-floor state must fail the Bond invariant boundary"
            );
        }
    }

    #[test]
    fn public_value_enforces_complete_bond_validation() {
        let mut bond = ex_coupon_bond();
        bond.notional = Money::new(0.0, Currency::USD);
        let err = bond
            .value(&MarketContext::new(), date!(2025 - 01 - 02))
            .expect_err("zero bond notional must fail before market lookup");
        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn public_value_rejects_non_positive_dirty_quote() {
        let mut bond = ex_coupon_bond();
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_dirty_price_currency = Some(-1.0);
        let err = bond
            .value(&MarketContext::new(), date!(2025 - 01 - 02))
            .expect_err("negative dirty price must fail before market lookup");
        assert!(err.to_string().contains("must be positive"));
    }

    #[test]
    fn native_joint_credit_bullet_rejects_inert_correlation() {
        let (mut bond, market, as_of) = deterministic_credit_bond_and_market();
        bond.instrument_pricing_overrides
            .model_config
            .rate_credit_correlation = Some(0.5);

        let error = bond
            .value(&market, as_of)
            .expect_err("native joint-model inference must validate inert correlation");
        assert!(error.to_string().contains("inert"), "{error}");
    }

    #[test]
    fn native_joint_credit_bullet_rejects_legacy_and_scheduled_rate_inputs() {
        let (bond, market, as_of) = deterministic_credit_bond_and_market();

        let mut legacy_vol = bond.clone();
        legacy_vol
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);
        let error = legacy_vol
            .value(&market, as_of)
            .expect_err("native joint-model inference must validate legacy implied volatility");
        assert!(error.to_string().contains("implied_volatility"), "{error}");

        let mut legacy_reversion = bond.clone();
        legacy_reversion
            .instrument_pricing_overrides
            .model_config
            .mean_reversion = Some(0.03);
        let error = legacy_reversion
            .value(&market, as_of)
            .expect_err("native joint-model inference must validate legacy mean reversion");
        assert!(error.to_string().contains("hw1f_mean_reversion"), "{error}");

        let mut scheduled = bond;
        scheduled
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma_schedule = Some(
            PiecewiseConstantCurve::new(vec![0.0, 2.0], vec![0.01, 0.012])
                .expect("valid volatility schedule"),
        );
        let error = scheduled.value(&market, as_of).expect_err(
            "native joint-model inference must validate unsupported volatility schedules",
        );
        assert!(error.to_string().contains("hw1f_sigma_schedule"), "{error}");
    }

    #[test]
    fn native_joint_credit_bullet_rejects_excessive_mean_reversion() {
        let (bond, market, as_of) = deterministic_credit_bond_and_market();
        for (label, rate_mean_reversion, hazard_mean_reversion) in [
            ("hw1f_mean_reversion", Some(KAPPA_MAX + 0.01), None),
            ("hazard_mean_reversion", None, Some(KAPPA_MAX + 0.01)),
        ] {
            let mut invalid = bond.clone();
            invalid
                .instrument_pricing_overrides
                .model_config
                .hw1f_mean_reversion = rate_mean_reversion;
            invalid
                .instrument_pricing_overrides
                .model_config
                .hazard_mean_reversion = hazard_mean_reversion;
            let error = invalid
                .value(&market, as_of)
                .expect_err("native joint-model inference must reject excessive mean reversion");
            assert!(error.to_string().contains(label), "{error}");
        }
    }

    #[test]
    fn explicit_hazard_rate_with_attached_joint_config_keeps_scalar_frp_price() {
        use crate::pricer::ModelKey;

        let (mut bond, market, as_of) = deterministic_credit_bond_and_market();
        bond.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
        bond.instrument_pricing_overrides
            .model_config
            .hazard_volatility = Some(0.005);
        bond.instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(KAPPA_MAX.min(0.05));
        bond.instrument_pricing_overrides
            .model_config
            .hazard_mean_reversion = Some(KAPPA_MAX.min(0.04));
        bond.instrument_pricing_overrides
            .model_config
            .rate_credit_correlation = Some(0.25);

        assert_eq!(
            bond.default_pricing_model(),
            ModelKey::RatesCredit,
            "native inference must select the joint model when joint controls are attached"
        );

        let expected = crate::instruments::fixed_income::bond::pricing::engine::hazard::HazardBondEngine::price_raw(
            &bond, &market, as_of,
        )
        .expect("scalar FRP price");
        let actual = bond
            .price_for_model_raw(ModelKey::HazardRate, &market, as_of)
            .expect("explicit hazard-rate price");
        assert!((actual - expected).abs() < 1.0e-12);
    }

    #[test]
    fn native_default_model_is_selected_from_bond_economics() {
        use crate::pricer::ModelKey;

        let vanilla = ex_coupon_bond();
        assert_eq!(vanilla.default_pricing_model(), ModelKey::Discounting);

        let mut credit = vanilla.clone();
        credit.credit_curve_id = Some("ACME-HZD".into());
        assert_eq!(credit.default_pricing_model(), ModelKey::HazardRate);

        let mut callable = vanilla;
        callable.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: date!(2027 - 01 - 01),
                end_date: date!(2027 - 01 - 01),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        assert_eq!(callable.default_pricing_model(), ModelKey::Tree);

        callable.credit_curve_id = Some("ACME-HZD".into());
        assert_eq!(callable.default_pricing_model(), ModelKey::RatesCredit);

        let mut configured_credit = credit;
        configured_credit
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma = Some(0.0);
        assert_eq!(
            configured_credit.default_pricing_model(),
            ModelKey::RatesCredit,
            "an explicit joint-factor input must not be silently ignored by the native default"
        );
    }
}
