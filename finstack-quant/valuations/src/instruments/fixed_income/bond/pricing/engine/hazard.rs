//! Hazard-rate (intensity) bond pricer with fractional recovery of par (FRP).
//!
//! This engine prices non-callable defaultable bonds using a reduced-form
//! hazard-rate model with piecewise-constant hazard curve and **fractional
//! recovery of par**. Embedded exercise rights belong to the explicit
//! rates-credit model.
//!
//! Let:
//! - `D(as_of, t)` be the risk-free discount factor from valuation date to t.
//! - `S(t)` be the survival probability from the hazard curve.
//! - `R` be the recovery rate (fraction of outstanding notional).
//! - `CF_i` be signed canonical schedule cashflows (coupons + principal) at dates `T_i`.
//! - `N(t)` be the outstanding notional process (including amortization).
//!
//! Under deterministic rates and credit and FRP, the price at `as_of` is:
//! ```text
//! PV = Σ_i CF_i · D(as_of, T_i) · S(T_i)
//!    + R · Σ_k D(as_of, t_k) · S(t_k) · N(t_k) · W_k
//! ```
//! where:
//! - `W_k = λ_k · (1 - exp(-(r_k + s + λ_k)Δt_k)) /
//!   (r_k + s + λ_k)` is the exact within-step discounted default weight,
//! - `N(t_k)` is the canonical after-event balance at the start of the step,
//! - the ACT/365F grid contains every calendar date through the final adjusted
//!   payment date and treats `tree_steps` as a minimum resolution.
//!
//! Recovery is taken as a fraction of **outstanding notional** (par) during
//! each interval, which matches the fractional recovery of par convention used
//! in the two-factor rates+credit tree (`BondValuator`).
//!
//! # Settlement Convention
//!
//! Settlement days affect quote interpretation (accrued interest at settlement),
//! but the PV is always anchored at `as_of`. The quote engine handles
//! settlement-date accrued interest separately.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::primitives::is_cash_settlement_kind;
use crate::instruments::common_impl::pricing::rates_credit::{
    build_daily_bond_rates_credit_targets, continuous_frp_weight,
};

use super::super::super::types::Bond;

/// Hazard-rate bond pricing engine using FRP and `HazardCurve`.
///
/// This straight-bond leaf prices defaultable bonds using a reduced-form
/// hazard-rate model with fractional recovery of par (FRP). It returns an
/// error if no hazard curve is available or if the bond has embedded call,
/// put, or return-floor rights.
pub struct HazardBondEngine;

impl HazardBondEngine {
    /// Resolve the hazard curve for the bond from its explicit
    /// `credit_curve_id` opt-in.
    ///
    /// Credit-risky pricing requires the instrument to name its hazard curve.
    /// The previous implicit discovery (`discount_curve_id` /
    /// `<discount_curve_id>-CREDIT` naming magic) could silently switch a
    /// bond to credit-risky pricing just because a similarly-named hazard
    /// curve existed in the market context.
    pub(crate) fn require_hazard_curve(
        bond: &Bond,
        market: &MarketContext,
    ) -> Result<std::sync::Arc<HazardCurve>> {
        let credit_id = bond.credit_curve_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "credit-consuming bond pricing for '{}' requires an explicit credit_curve_id",
                bond.id.as_str()
            ))
        })?;
        market.get_hazard(credit_id.as_str())
    }

    /// Reject option-bearing bonds at the deterministic straight-bond leaf.
    fn reject_embedded_options(bond: &Bond) -> Result<()> {
        let has_call_put = bond
            .call_put
            .as_ref()
            .is_some_and(crate::instruments::fixed_income::bond::CallPutSchedule::has_options);
        if has_call_put || bond.return_floor.is_some() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "HazardBondEngine is a straight-bond leaf and cannot price embedded options \
                 for bond '{}'; use the 'rates_credit' model for joint rates-credit optional pricing",
                bond.id.as_str()
            )));
        }
        Ok(())
    }

    /// Build pricing cashflows and the full internal schedule.
    fn build_schedules(
        bond: &Bond,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<(Vec<(Date, Money)>, CashFlowSchedule)> {
        let flows = bond.pricing_dated_cashflows(market, as_of)?;
        let schedule = bond.full_cashflow_schedule(market)?;
        Ok((flows, schedule))
    }

    #[cfg(test)]
    fn price(bond: &Bond, market: &MarketContext, as_of: Date) -> Result<Money> {
        Ok(Money::new(
            Self::price_raw(bond, market, as_of)?,
            bond.notional.currency(),
        )
        .expect("valid money fixture"))
    }

    /// Price a bond using a hazard curve and return the unrounded PV.
    #[cfg(test)]
    pub(crate) fn price_raw(bond: &Bond, market: &MarketContext, as_of: Date) -> Result<f64> {
        Self::price_raw_with_oas(bond, market, as_of, 0.0)
    }

    /// Price a non-callable hazard-rate bond with a constant OAS.
    ///
    /// `oas_quote_decimal` follows the bond's configured OAS quote
    /// compounding. The scalar FRP kernel converts it to a continuous spread
    /// and applies it consistently to promised cash and within-step recovery.
    pub(crate) fn price_raw_with_oas(
        bond: &Bond,
        market: &MarketContext,
        as_of: Date,
        oas_quote_decimal: f64,
    ) -> Result<f64> {
        Self::reject_embedded_options(bond)?;
        if !oas_quote_decimal.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "hazard-rate OAS must be finite, got {oas_quote_decimal}"
            )));
        }
        let continuous_oas = bond
            .instrument_pricing_overrides
            .model_config
            .oas_quote_compounding
            .continuous_from_quote_decimal(oas_quote_decimal);
        let disc = market.get_discount(&bond.discount_curve_id)?;

        // Explicit hazard-rate pricing must fail loudly when credit market data
        // is missing; risk-free pricing belongs on the discounting model path.
        let hazard = Self::require_hazard_curve(bond, market)?;
        let (flows, schedule) = Self::build_schedules(bond, market, as_of)?;
        let Some(final_payment_date) = schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.date > as_of && is_cash_settlement_kind(flow.kind))
            .map(|flow| flow.date)
            .max()
        else {
            return Ok(0.0);
        };
        let minimum_steps = bond
            .instrument_pricing_overrides
            .model_config
            .tree_steps
            .unwrap_or(1);
        let targets = build_daily_bond_rates_credit_targets(
            disc.as_ref(),
            hazard.as_ref(),
            as_of,
            final_payment_date,
            minimum_steps,
        )?;
        let steps = targets.times.len() - 1;
        let span_days =
            usize::try_from((final_payment_date - as_of).whole_days()).map_err(|_| {
                finstack_quant_core::Error::Validation(
                    "hazard bond payment horizon exceeds supported grid size".to_string(),
                )
            })?;
        let substeps_per_day = steps / span_days;

        let mut cash_by_step = vec![0.0; steps + 1];
        for (date, amount) in flows {
            let day = usize::try_from((date - as_of).whole_days()).map_err(|_| {
                finstack_quant_core::Error::Validation(
                    "hazard bond cashflow predates the valuation date".to_string(),
                )
            })?;
            let step = day.checked_mul(substeps_per_day).ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "hazard bond cashflow grid index overflow".to_string(),
                )
            })?;
            if let Some(total) = cash_by_step.get_mut(step) {
                *total += amount.amount();
            }
        }

        // `outstanding_by_date` is the canonical after-event balance replay;
        // it includes historical and future amortization, PIK, draws, and
        // repayments in schedule order.
        let outstanding_path = schedule.outstanding_by_date()?;
        let mut outstanding = schedule.get_notional().initial.amount();
        let mut balance_cursor = 0;
        while balance_cursor < outstanding_path.len() && outstanding_path[balance_cursor].0 <= as_of
        {
            outstanding = outstanding_path[balance_cursor].1.amount();
            balance_cursor += 1;
        }

        let recovery = targets.recovery_rate.clamp(0.0, 1.0);
        let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::default();
        for (step, cash) in cash_by_step.iter().copied().enumerate() {
            let oas_discount = (-continuous_oas * targets.times[step]).exp();
            pv.add(
                cash * targets.discount_factors[step]
                    * targets.survival_probabilities[step]
                    * oas_discount,
            );
            if step == steps {
                break;
            }

            let whole_day_boundary = step % substeps_per_day == 0;
            if whole_day_boundary {
                let day = step / substeps_per_day;
                let date = as_of + time::Duration::days(day as i64);
                while balance_cursor < outstanding_path.len()
                    && outstanding_path[balance_cursor].0 <= date
                {
                    outstanding = outstanding_path[balance_cursor].1.amount();
                    balance_cursor += 1;
                }
            }

            if recovery > 0.0 && outstanding > 0.0 {
                let interval_discount = targets.discount_factors[step + 1]
                    / targets.discount_factors[step]
                    * (-continuous_oas * (targets.times[step + 1] - targets.times[step])).exp();
                let interval_survival =
                    targets.survival_probabilities[step + 1] / targets.survival_probabilities[step];
                let default_weight = continuous_frp_weight(interval_discount, interval_survival)?;
                pv.add(
                    targets.discount_factors[step]
                        * targets.survival_probabilities[step]
                        * oas_discount
                        * recovery
                        * outstanding
                        * default_weight,
                );
            }
        }

        Ok(pv.total())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::CouponType;
    use crate::instruments::common_impl::traits::Attributes;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::bond::pricing::engine::discount::BondEngine;
    use crate::instruments::fixed_income::bond::pricing::engine::tree::{
        bond_tree_config, TreePricer,
    };
    use crate::instruments::fixed_income::bond::{
        BondSettlementConvention, CallPut, CallPutSchedule, CashflowSpec,
    };
    use crate::pricer::{ModelKey, PricingError};
    use crate::results::ValuationDetails;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_core::{dates::Date, money::Money};
    use time::Month;

    fn build_test_bond(issue: Date, maturity: Date) -> Bond {
        Bond::builder()
            .id("TEST_BOND_HAZARD".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT")))
            .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("Bond builder should succeed in hazard engine test")
    }

    fn build_flat_discount(issue: Date) -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (10.0, 0.8)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("DiscountCurve builder should succeed in hazard engine test")
    }

    fn build_flat_hazard(id: &str, issue: Date, lambda: f64, recovery: f64) -> HazardCurve {
        HazardCurve::builder(id)
            .base_date(issue)
            .recovery_rate(recovery)
            .knots([(0.0, lambda), (10.0, lambda)])
            .build()
            .expect("HazardCurve builder should succeed in hazard engine test")
    }

    fn build_pik_bond(issue: Date, maturity: Date) -> Bond {
        let mut spec = CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
            .expect("finite test coupon");
        if let CashflowSpec::Fixed(ref mut inner) = spec {
            inner.coupon_type = CouponType::Pik;
        }
        Bond::builder()
            .id("TEST_PIK_BOND_HAZARD".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(spec)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT")))
            .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("PIK bond builder should succeed in hazard engine test")
    }

    #[test]
    fn hazard_zero_matches_discounting_for_plain_bond() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

        let bond = build_test_bond(issue, maturity);
        let disc = build_flat_discount(issue);
        let hazard_zero = build_flat_hazard("USD-CREDIT", issue, 0.0, 0.4);

        let market = MarketContext::new().insert(disc).insert(hazard_zero);

        let pv_rf = BondEngine::price(&bond, &market, issue).expect("RF price should succeed");
        let pv_haz =
            HazardBondEngine::price(&bond, &market, issue).expect("Hazard price should succeed");

        let diff = (pv_rf.amount() - pv_haz.amount()).abs();
        assert!(
            diff < 1e-6,
            "Hazard price with zero intensity should match risk-free price; diff={}",
            diff
        );
    }

    /// Same-day cashflow handling must be consistent across the discount and
    /// hazard engines. The unified convention **excludes** cashflows dated
    /// exactly on `as_of` (settlement convention, strict `d > as_of`),
    /// matching the tree and YTM engines. With a zero hazard intensity the
    /// two engines must agree to floating-point precision even when a coupon
    /// lands exactly on the valuation date.
    #[test]
    fn hazard_zero_matches_discounting_with_cashflow_on_as_of() {
        // Issue one year before `as_of` with an annual coupon, so a coupon
        // falls exactly on `as_of`.
        let issue = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        let bond = Bond::builder()
            .id("TEST_BOND_HAZARD_ONDATE".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT")))
            .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("Bond builder should succeed");

        let disc = build_flat_discount(issue);
        let hazard_zero = build_flat_hazard("USD-CREDIT", issue, 0.0, 0.4);
        let market = MarketContext::new().insert(disc).insert(hazard_zero);

        // The unified settlement convention excludes the on-`as_of` coupon
        // from buyer flows.
        let flows = bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("cashflows");
        assert!(
            flows.iter().all(|(d, _)| *d > as_of),
            "settlement convention: cashflows dated on as_of must be excluded"
        );

        let pv_rf = BondEngine::price(&bond, &market, as_of).expect("RF price should succeed");
        let pv_haz =
            HazardBondEngine::price(&bond, &market, as_of).expect("Hazard price should succeed");

        let diff = (pv_rf.amount() - pv_haz.amount()).abs();
        assert!(
            diff < 1e-6,
            "zero-hazard price must match the discounting engine when a coupon \
             lands on as_of; pv_rf={}, pv_haz={}, diff={}",
            pv_rf.amount(),
            pv_haz.amount(),
            diff
        );
    }

    #[test]
    fn hazard_pricing_requires_hazard_curve() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

        let bond = build_test_bond(issue, maturity);
        let market = MarketContext::new().insert(build_flat_discount(issue));

        let err = HazardBondEngine::price(&bond, &market, issue)
            .expect_err("hazard pricing should reject missing hazard curve");

        assert!(
            err.to_string().contains("USD-CREDIT"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn straight_hazard_leaf_rejects_embedded_options() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        let mut callable = build_test_bond(issue, maturity);
        callable.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: Date::from_calendar_date(2027, Month::January, 1)
                    .expect("valid call date"),
                end_date: Date::from_calendar_date(2027, Month::January, 1)
                    .expect("valid call date"),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let market = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, 0.02, 0.4));

        let err = HazardBondEngine::price_raw(&callable, &market, issue)
            .expect_err("straight hazard leaf must reject embedded options");
        assert!(
            err.to_string().contains("straight-bond leaf"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn explicit_registry_model_matrix_preserves_rates_and_credit_boundaries() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        let exercise =
            Date::from_calendar_date(2027, Month::January, 1).expect("valid exercise date");
        let bullet = build_test_bond(issue, maturity);
        let mut callable = bullet.clone();
        callable.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 90.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });

        let discount_only = MarketContext::new().insert(build_flat_discount(issue));
        let credit_market = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, 0.02, 0.4));
        let registry = crate::pricer::standard_pricer_registry();
        let options = crate::instruments::PricingOptions::default();

        let discounting = registry
            .price_with_metrics(
                &bullet,
                ModelKey::Discounting,
                &discount_only,
                issue,
                &[],
                options.clone(),
            )
            .expect("straight bond must support explicit discounting");
        let hazard = registry
            .price_with_metrics(
                &bullet,
                ModelKey::HazardRate,
                &credit_market,
                issue,
                &[],
                options.clone(),
            )
            .expect("straight bond must support explicit hazard pricing");
        assert!(discounting.value.amount().is_finite());
        assert!(hazard.value.amount().is_finite());

        for model in [ModelKey::Discounting, ModelKey::HazardRate] {
            let err = registry
                .price_with_metrics(
                    &callable,
                    model,
                    &credit_market,
                    issue,
                    &[],
                    options.clone(),
                )
                .expect_err("straight-bond model must reject callable bonds");
            assert!(
                matches!(err, PricingError::InvalidInput { .. }),
                "{model} callable rejection must remain typed, got {err:?}"
            );
        }

        let tree_with_credit_id = registry
            .price_with_metrics(
                &callable,
                ModelKey::Tree,
                &discount_only,
                issue,
                &[],
                options.clone(),
            )
            .expect("rates-only tree must not require an attached credit curve");
        let mut rates_only_callable = callable.clone();
        rates_only_callable.credit_curve_id = None;
        let tree_without_credit_id = registry
            .price_with_metrics(
                &rates_only_callable,
                ModelKey::Tree,
                &discount_only,
                issue,
                &[],
                options.clone(),
            )
            .expect("rates-only tree must price without a credit curve id");
        assert!(
            (tree_with_credit_id.value.amount() - tree_without_credit_id.value.amount()).abs()
                < 1.0e-9,
            "Tree must remain rates-only when the instrument also names a credit curve"
        );

        let missing_hazard = registry
            .price_with_metrics(
                &callable,
                ModelKey::RatesCredit,
                &discount_only,
                issue,
                &[],
                options,
            )
            .expect_err("joint rates-credit pricing must require its hazard curve");
        assert!(
            matches!(missing_hazard, PricingError::MissingMarketData { .. }),
            "missing joint-model hazard data must remain typed, got {missing_hazard:?}"
        );
    }

    #[test]
    fn explicit_rates_credit_registry_values_calls_puts_and_raw_path() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        let exercise =
            Date::from_calendar_date(2027, Month::January, 1).expect("valid exercise date");
        let mut bullet = build_test_bond(issue, maturity);
        bullet.instrument_pricing_overrides.model_config.tree_steps = Some(20);

        let mut callable = bullet.clone();
        callable.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 80.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });

        let mut puttable = bullet.clone();
        puttable.call_put = Some(CallPutSchedule {
            calls: Vec::new(),
            puts: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 130.0,
                make_whole: None,
            }],
        });

        let market = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, 0.02, 0.4));
        let registry = crate::pricer::standard_pricer_registry();
        let price = |bond: &Bond| {
            registry
                .price_with_metrics(
                    bond,
                    ModelKey::RatesCredit,
                    &market,
                    issue,
                    &[],
                    crate::instruments::PricingOptions::default(),
                )
                .expect("explicit rates-credit pricing")
                .value
                .amount()
        };

        let bullet_pv = price(&bullet);
        let callable_pv = price(&callable);
        let puttable_pv = price(&puttable);
        assert!(
            callable_pv < bullet_pv,
            "issuer call must lower holder value: callable={callable_pv}, bullet={bullet_pv}"
        );
        assert!(
            puttable_pv > bullet_pv,
            "holder put must raise holder value: puttable={puttable_pv}, bullet={bullet_pv}"
        );

        let default_pv = callable
            .value(&market, issue)
            .expect("default callable valuation")
            .amount();
        assert!(
            (callable_pv - default_pv).abs() < 1e-9,
            "explicit rates-credit and default bond policy must share the option-aware tree"
        );

        let raw_registered = registry
            .price_raw(&callable, ModelKey::RatesCredit, &market, issue)
            .expect("registered raw rates-credit valuation");
        let raw_default = callable
            .value_raw(&market, issue)
            .expect("default raw callable valuation");
        assert!(
            (raw_registered - raw_default).abs() < 1e-9,
            "registered and default raw paths must share the option-aware rates-credit tree"
        );
    }

    #[test]
    fn stochastic_rates_credit_bullet_surfaces_reproducibility_diagnostics() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let mut bond = build_test_bond(issue, maturity);
        bond.instrument_pricing_overrides
            .model_config
            .hazard_volatility = Some(0.01);
        bond.instrument_pricing_overrides.model_config.mc_paths = Some(8);
        bond.instrument_pricing_overrides.model_config.tree_steps = Some(4);
        let market = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, 0.02, 0.4));

        let result = crate::pricer::standard_pricer_registry()
            .price_with_metrics(
                &bond,
                ModelKey::RatesCredit,
                &market,
                issue,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("stochastic rates-credit bullet price");
        assert!(result.value.amount().is_finite());
        assert_eq!(result.metric_str("mc_num_paths"), Some(8.0));
        assert_eq!(result.metric_str("mc_num_simulated_paths"), Some(16.0));
        let Some(ValuationDetails::MonteCarlo(details)) = result.details else {
            panic!("stochastic rates-credit result must include Monte Carlo details");
        };
        assert_eq!(details.model_key, ModelKey::RatesCredit);
        assert_eq!(details.training_paths, 0);
        assert_eq!(details.training_simulated_paths, 0);
        assert_eq!(details.make_whole_training_paths, 0);
        assert_eq!(details.make_whole_training_simulated_paths, 0);
        assert_eq!(details.estimator_paths, 8);
        assert_eq!(details.simulated_paths, 16);
        assert!(details.time_grid.len() >= 366);
        assert!(details.antithetic);
    }

    #[test]
    fn stochastic_quoted_oas_surfaces_reproducibility_diagnostics() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let mut bond = build_test_bond(issue, maturity);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: Date::from_calendar_date(2025, Month::July, 1)
                    .expect("valid call date"),
                end_date: Date::from_calendar_date(2025, Month::July, 1).expect("valid call date"),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        bond.settlement_convention = Some(BondSettlementConvention {
            settlement_days: 2,
            ..Default::default()
        });
        bond.instrument_pricing_overrides
            .model_config
            .hazard_volatility = Some(0.01);
        bond.instrument_pricing_overrides.model_config.mc_paths = Some(32);
        bond.instrument_pricing_overrides.model_config.tree_steps = Some(4);
        bond.instrument_pricing_overrides.market_quotes.quoted_oas = Some(0.0025);
        let market = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, 0.02, 0.4));

        let result = crate::pricer::standard_pricer_registry()
            .price_with_metrics(
                &bond,
                ModelKey::RatesCredit,
                &market,
                issue,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("stochastic quoted-OAS hazard price");

        assert!(result.value.amount().is_finite());
        let expected_direct = TreePricer::rates_credit(
            bond_tree_config(&bond).expect("quoted-OAS tree configuration"),
        )
        .price_at_oas(&bond, &market, issue, 25.0)
        .expect("direct as-of quoted-OAS value");
        assert!(
            (result.value.amount() - expected_direct).abs() < 1.0e-10,
            "quoted OAS must price on the as-of tree kernel rather than a settlement-date carry"
        );
        let expected_amount = bond
            .value_raw(&market, issue)
            .expect("canonical quoted-OAS value");
        assert!((result.value.amount() - expected_amount).abs() < 1.0e-10);
        assert_eq!(result.metric_str("mc_num_paths"), Some(32.0));
        assert_eq!(result.metric_str("mc_num_simulated_paths"), Some(64.0));
        let Some(ValuationDetails::MonteCarlo(details)) = result.details else {
            panic!("quoted OAS remains a stochastic model run with diagnostics");
        };
        assert_eq!(details.training_paths, 32);
        assert_eq!(details.training_simulated_paths, 64);
        assert_eq!(details.make_whole_training_paths, 0);
        assert_eq!(details.make_whole_training_simulated_paths, 0);
        assert_eq!(details.estimator_paths, 32);
        assert_eq!(details.simulated_paths, 64);
        assert!(details.standard_error.is_finite());

        let mut clean_price = bond;
        clean_price
            .instrument_pricing_overrides
            .market_quotes
            .quoted_oas = None;
        clean_price
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(100.0);
        let clean_result = crate::pricer::standard_pricer_registry()
            .price_with_metrics(
                &clean_price,
                ModelKey::RatesCredit,
                &market,
                issue,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("quoted clean-price rates-credit value");
        assert!(clean_result.details.is_none());
    }

    #[test]
    fn explicit_hazard_validates_credit_before_quote_short_circuit() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        let mut quoted = build_test_bond(issue, maturity);
        quoted
            .instrument_pricing_overrides
            .market_quotes
            .quoted_dirty_price_currency = Some(1_000_000.0);
        let market = MarketContext::new().insert(build_flat_discount(issue));
        let registry = crate::pricer::standard_pricer_registry();

        let missing_curve = registry
            .price_with_metrics(
                &quoted,
                ModelKey::HazardRate,
                &market,
                issue,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect_err("quoted hazard bond must still require its market hazard curve");
        assert!(
            matches!(missing_curve, PricingError::MissingMarketData { .. }),
            "missing hazard curve must retain its typed error, got {missing_curve:?}"
        );

        quoted.credit_curve_id = None;
        let missing_id = registry
            .price_with_metrics(
                &quoted,
                ModelKey::HazardRate,
                &market,
                issue,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect_err("explicit hazard model must require credit_curve_id");
        assert!(
            matches!(missing_id, PricingError::InvalidInput { .. }),
            "missing credit_curve_id must be invalid input, got {missing_id:?}"
        );
    }

    #[test]
    fn higher_hazard_produces_lower_price() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

        let bond = build_test_bond(issue, maturity);
        let hazard_low = build_flat_hazard("USD-CREDIT", issue, 0.01, 0.4);
        let hazard_high = build_flat_hazard("USD-CREDIT", issue, 0.05, 0.4);

        let market_low = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(hazard_low);
        let market_high = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(hazard_high);

        let pv_low =
            HazardBondEngine::price(&bond, &market_low, issue).expect("Low hazard price succeeds");
        let pv_high = HazardBondEngine::price(&bond, &market_high, issue)
            .expect("High hazard price succeeds");

        assert!(
            pv_high.amount() < pv_low.amount(),
            "Price with higher hazard should be lower (pv_high={}, pv_low={})",
            pv_high.amount(),
            pv_low.amount()
        );
    }

    #[test]
    fn higher_recovery_increases_price() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

        let bond = build_test_bond(issue, maturity);
        let hazard_low_recovery = build_flat_hazard("USD-CREDIT", issue, 0.03, 0.0);
        let hazard_high_recovery = build_flat_hazard("USD-CREDIT", issue, 0.03, 1.0);

        let market_low_r = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(hazard_low_recovery);
        let market_high_r = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(hazard_high_recovery);

        let pv_low_r = HazardBondEngine::price(&bond, &market_low_r, issue)
            .expect("Low recovery hazard price succeeds");
        let pv_high_r = HazardBondEngine::price(&bond, &market_high_r, issue)
            .expect("High recovery hazard price succeeds");

        assert!(
            pv_high_r.amount() > pv_low_r.amount(),
            "Price with higher recovery should be higher (pv_high={}, pv_low={})",
            pv_high_r.amount(),
            pv_low_r.amount()
        );
    }

    #[test]
    fn quote_engine_works_for_bond_with_hazard_curve() {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
            compute_quotes, BondQuoteInput,
        };
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

        let bond = build_test_bond(issue, maturity);
        let market = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, 0.02, 0.4));

        // Use a simple clean price quote; the quote engine should handle bonds
        // with hazard curves present in the MarketContext without error.
        let quotes = compute_quotes(
            &bond,
            &market,
            issue,
            BondQuoteInput::CleanPricePct(99.5),
            crate::instruments::PricingOptions::default().with_model(ModelKey::HazardRate),
        )
        .expect("Quote engine should work for bonds with hazard curves");

        assert!(
            (quotes.clean_price_pct - 99.5).abs() < 1e-9,
            "Clean price pct should reflect the input quote"
        );
        // Basic sanity: core yield/spread metrics should be populated.
        assert!(quotes.ytm.is_some(), "YTM should be computed");
        assert!(
            quotes.z_spread.is_some(),
            "Z-spread should be computed for quoted bond"
        );
    }

    #[test]
    fn hazard_zero_matches_discounting_for_pik_bond() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

        let bond = build_pik_bond(issue, maturity);
        let disc = build_flat_discount(issue);
        let hazard_zero = build_flat_hazard("USD-CREDIT", issue, 0.0, 0.4);

        let market = MarketContext::new().insert(disc).insert(hazard_zero);

        let pv_rf = BondEngine::price(&bond, &market, issue).expect("RF price should succeed");
        let pv_haz =
            HazardBondEngine::price(&bond, &market, issue).expect("Hazard price should succeed");

        let diff = (pv_rf.amount() - pv_haz.amount()).abs();
        assert!(
            diff < 1e-6,
            "PIK hazard price with zero intensity should match risk-free price; \
             pv_rf={}, pv_haz={}, diff={}",
            pv_rf.amount(),
            pv_haz.amount(),
            diff
        );
    }

    #[test]
    fn pik_recovery_uses_accreted_notional() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

        let cash_bond = build_test_bond(issue, maturity);
        let pik_bond = build_pik_bond(issue, maturity);

        let lambda = 0.05;
        let recovery = 0.4;
        let market = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, lambda, recovery));

        let pv_cash =
            HazardBondEngine::price(&cash_bond, &market, issue).expect("Cash bond price succeeds");
        let pv_pik =
            HazardBondEngine::price(&pik_bond, &market, issue).expect("PIK bond price succeeds");

        // With zero recovery, isolate the alive-leg difference.
        let market_no_rec = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, lambda, 0.0));

        let pv_cash_norec = HazardBondEngine::price(&cash_bond, &market_no_rec, issue)
            .expect("Cash bond no-recovery price");
        let pv_pik_norec = HazardBondEngine::price(&pik_bond, &market_no_rec, issue)
            .expect("PIK bond no-recovery price");

        // Recovery benefit = PV(with recovery) - PV(without recovery)
        let rec_benefit_cash = pv_cash.amount() - pv_cash_norec.amount();
        let rec_benefit_pik = pv_pik.amount() - pv_pik_norec.amount();

        // PIK accretes notional above par, so its recovery base is larger on
        // average → the PIK recovery benefit must exceed the cash bond's.
        assert!(
            rec_benefit_pik > rec_benefit_cash,
            "PIK recovery benefit ({:.2}) should exceed cash bond recovery benefit ({:.2}) \
             because PIK accretes notional above par",
            rec_benefit_pik,
            rec_benefit_cash
        );
    }

    #[test]
    fn pik_higher_hazard_produces_lower_price() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

        let bond = build_pik_bond(issue, maturity);

        let market_low = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, 0.01, 0.4));
        let market_high = MarketContext::new()
            .insert(build_flat_discount(issue))
            .insert(build_flat_hazard("USD-CREDIT", issue, 0.05, 0.4));

        let pv_low =
            HazardBondEngine::price(&bond, &market_low, issue).expect("Low hazard price succeeds");
        let pv_high = HazardBondEngine::price(&bond, &market_high, issue)
            .expect("High hazard price succeeds");

        assert!(
            pv_high.amount() < pv_low.amount(),
            "PIK price with higher hazard should be lower (pv_high={}, pv_low={})",
            pv_high.amount(),
            pv_low.amount()
        );
    }

    /// Regression test: when the hazard curve implies the bond has already
    /// defaulted at `as_of` (survival probability at as_of underflows to 0),
    /// the engine must return an explicit error rather than silently returning
    /// PV = 0.0. The previous behavior masked malformed credit curves as
    /// valid zero valuations.
    #[test]
    fn hazard_engine_errors_on_prior_default() {
        // Push as_of well past the curve's last knot so FlatForward
        // extrapolation drives `S(as_of) = exp(-lambda*t)` below f64's
        // underflow threshold (~exp(-745)). This keeps the curve builder
        // happy (lambda stays inside the default sanity bound) while still
        // producing a true zero at evaluation time.
        let issue = Date::from_calendar_date(2020, Month::January, 1).expect("valid date");
        let as_of = Date::from_calendar_date(2100, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2110, Month::January, 1).expect("valid date");

        let bond = build_test_bond(issue, maturity);
        // Need a discount curve that extends past `as_of` so PV anchoring
        // works at a far-future as_of.
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (100.0, 0.01)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("DiscountCurve builder should succeed");
        // lambda = 10/year, FlatForward extrapolation. S(80y) = exp(-800) = 0.
        let hazard = build_flat_hazard("USD-CREDIT", issue, 10.0, 0.4);
        let market = MarketContext::new().insert(disc).insert(hazard);

        let result = HazardBondEngine::price(&bond, &market, as_of);
        let err = result.expect_err("prior-default curve must error, not return Ok(0.0)");
        let msg = err.to_string();
        assert!(
            msg.contains("zero survival probability")
                || msg.contains("already-defaulted")
                || msg.contains("survival probability at origin must be positive"),
            "error should explain prior default; got: {msg}"
        );
    }
}
