//! Effective duration and convexity for bonds with embedded options.
//!
//! For bonds with call, put, or return-floor rights, yield-based modified
//! duration and convexity are inappropriate because they assume fixed
//! cashflows. Effective duration and convexity measure price sensitivity by
//! bumping the discount curve and repricing through the option model, which
//! accounts for changes in exercise behavior as rates move.
//!
//! # Formulas
//!
//! ```text
//! D_eff = (P_down - P_up) / (2 * P_base * shock)
//! C_eff = (P_up + P_down - 2 * P_base) / (P_base * shock^2)
//! ```
//!
//! where `shock` is the parallel rate bump in decimal (e.g., 0.0025 for 25 bp).

use crate::instruments::Bond;
use crate::metrics::MetricContext;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::market_data::context::BumpSpec;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

const DEFAULT_SHOCK_BPS: f64 = 25.0;

/// Internal effective duration and convexity result.
#[derive(Debug, Clone)]
pub(crate) struct EffectiveDurationResult {
    pub duration: f64,
    pub convexity: f64,
}

/// Calculate effective duration for a bond using parallel curve bumps.
///
/// For bonds without embedded options, this produces results very close to
/// modified duration. For bonds with call, put, or return-floor rights, the
/// option model captures the change in exercise behavior as rates shift.
pub(crate) fn effective_duration(
    bond: &Bond,
    context: &MetricContext,
    shock_bp: Option<f64>,
) -> Result<f64> {
    Ok(effective_duration_convexity(bond, context, shock_bp)?.duration)
}

/// Calculate effective convexity for a bond using parallel curve bumps.
pub(crate) fn effective_convexity(
    bond: &Bond,
    context: &MetricContext,
    shock_bp: Option<f64>,
) -> Result<f64> {
    Ok(effective_duration_convexity(bond, context, shock_bp)?.convexity)
}

/// Calculate both effective duration and convexity in one pass (three pricings).
pub(crate) fn effective_duration_convexity(
    bond: &Bond,
    context: &MetricContext,
    shock_bp: Option<f64>,
) -> Result<EffectiveDurationResult> {
    let shock_bp = shock_bp.unwrap_or(DEFAULT_SHOCK_BPS);
    let shock = shock_bp / 10_000.0;

    let (risk_bond, base_price) = option_risk_bond_and_base_price(bond, context)?;

    if base_price.abs() < 1e-10 {
        return Ok(EffectiveDurationResult {
            duration: 0.0,
            convexity: 0.0,
        });
    }

    let curve_id = option_risk_curve_id(&risk_bond);
    let market_up = context.curves.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(shock_bp),
    }])?;
    let market_down = context.curves.bump([MarketBump::Curve {
        id: curve_id,
        spec: BumpSpec::parallel_bp(-shock_bp),
    }])?;

    let price_up = context.reprice_instrument_raw(&risk_bond, &market_up, context.as_of)?;
    let price_down = context.reprice_instrument_raw(&risk_bond, &market_down, context.as_of)?;

    let duration = (price_down - price_up) / (2.0 * base_price * shock);
    let convexity = (price_up + price_down - 2.0 * base_price) / (base_price * shock * shock);

    Ok(EffectiveDurationResult {
        duration,
        convexity,
    })
}

pub(crate) fn option_risk_bond_and_base_price(
    bond: &Bond,
    context: &MetricContext,
) -> Result<(Bond, f64)> {
    use crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides;
    let mut risk_bond = bond.clone();
    if let Some(oas) = bond.instrument_pricing_overrides.market_quotes.quoted_oas {
        clear_price_driving_overrides(&mut risk_bond);
        risk_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_oas = Some(oas);
        let base =
            context.reprice_instrument_raw(&risk_bond, context.curves.as_ref(), context.as_of)?;
        return Ok((risk_bond, base));
    }

    if !bond
        .instrument_pricing_overrides
        .market_quotes
        .has_price_driver()
    {
        let base =
            context.reprice_instrument_raw(&risk_bond, context.curves.as_ref(), context.as_of)?;
        return Ok((risk_bond, base));
    }

    let oas_decimal =
        super::price_yield_spread::oas::oas_decimal_from_quote_overrides(bond, context)?
            .ok_or_else(|| {
                finstack_quant_core::Error::internal(
                    "bond option risk found a price-driving quote but could not resolve its OAS",
                )
            })?;

    clear_price_driving_overrides(&mut risk_bond);
    risk_bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_oas = Some(oas_decimal);
    let base =
        context.reprice_instrument_raw(&risk_bond, context.curves.as_ref(), context.as_of)?;
    Ok((risk_bond, base))
}

pub(crate) fn option_risk_curve_id(bond: &Bond) -> CurveId {
    bond.instrument_pricing_overrides
        .model_config
        .tree_discount_curve_id
        .clone()
        .unwrap_or_else(|| bond.discount_curve_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule, CashflowSpec};
    use crate::instruments::{BondRiskBasis, InstrumentPricingOverrides, PricingOptions};
    use crate::metrics::{standard_registry, MetricContext, MetricId};
    use crate::pricer::{
        expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingDispatch,
        PricingError,
    };
    use crate::results::ValuationResult;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use time::Month;

    fn test_market(as_of: finstack_quant_core::dates::Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, 0.96),
                (3.0, 0.88),
                (5.0, 0.80),
                (10.0, 0.65),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("valid curve");
        MarketContext::new().insert(disc)
    }

    fn bullet_bond(as_of: finstack_quant_core::dates::Date) -> Bond {
        let maturity = as_of + time::Duration::days(5 * 365);
        Bond::builder()
            .id("BULLET".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Default::default())
            .build()
            .expect("valid bond")
    }

    fn callable_bond(as_of: finstack_quant_core::dates::Date) -> Bond {
        let maturity = as_of + time::Duration::days(5 * 365);
        let call_date = as_of + time::Duration::days(2 * 365);
        let mut bond = Bond::builder()
            .id("CALLABLE".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Default::default())
            .build()
            .expect("valid bond");

        let mut schedule = CallPutSchedule::default();
        schedule.calls.push(CallPut {
            start_date: call_date,
            end_date: maturity,
            price_pct_of_par: 100.0,
            make_whole: None,
        });
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.01);
        bond.call_put = Some(schedule);
        bond
    }

    fn test_context(
        bond: &Bond,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> MetricContext {
        let base = bond.value(market, as_of).expect("base bond value");
        MetricContext::new(
            Arc::new(bond.clone()),
            Arc::new(market.clone()),
            as_of,
            base,
            MetricContext::default_config(),
        )
    }

    struct CurveSensitiveTreePricer {
        calls: Arc<AtomicUsize>,
    }

    impl CurveSensitiveTreePricer {
        fn price(
            &self,
            instrument: &dyn Instrument,
            market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> f64 {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)
                .expect("effective-risk test pricer expects a bond");
            let discount = market
                .get_discount(bond.discount_curve_id.as_str())
                .expect("test discount curve");
            let df = discount
                .df_between_dates(as_of, bond.maturity)
                .expect("test discount factor");
            let oas = bond
                .instrument_pricing_overrides
                .market_quotes
                .quoted_oas
                .expect("effective-risk clone must retain OAS");
            bond.notional.amount() * df * (-oas * 5.0).exp()
        }
    }

    impl Pricer for CurveSensitiveTreePricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::Tree)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            Ok(ValuationResult::stamped(
                instrument.id(),
                as_of,
                Money::new(self.price(instrument, market, as_of), Currency::USD),
            ))
        }

        fn price_raw_dyn(
            &self,
            instrument: &dyn Instrument,
            market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<f64, PricingError> {
            Ok(self.price(instrument, market, as_of))
        }
    }

    /// Item 10 regression: effective duration/convexity must put all three
    /// prices (base, up, down) on a single valuation date.
    ///
    /// The bumped legs are repriced with `value(.., as_of)`. The base price
    /// must therefore also be the `as_of`-anchored value of the same risk bond
    /// — `risk_bond.value(market, as_of)` — not the settlement-anchored
    /// quote/OAS price. When the bond carries a settlement lag those differ by
    /// the accrued-interest carry, which would otherwise contaminate the
    /// finite-difference ratio.
    #[test]
    fn effective_duration_uses_single_valuation_date() {
        use crate::instruments::fixed_income::bond::BondSettlementConvention;

        let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
            .expect("ok");
        let market = test_market(as_of);

        // Quoted callable bond with a 2-day settlement lag: the quote is
        // interpreted at settlement, but PV (and the bumped legs) anchor at as_of.
        let maturity = as_of + time::Duration::days(5 * 365);
        let call_date = as_of + time::Duration::days(2 * 365);
        let mut bond = Bond::builder()
            .id("CALLABLE-QUOTED-LAG".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .instrument_pricing_overrides(
                InstrumentPricingOverrides::default().with_quoted_clean_price(98.0),
            )
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .attributes(Default::default())
            .build()
            .expect("valid bond");
        let mut schedule = CallPutSchedule::default();
        schedule.calls.push(CallPut {
            start_date: call_date,
            end_date: maturity,
            price_pct_of_par: 100.0,
            make_whole: None,
        });
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.01);
        bond.call_put = Some(schedule);

        let shock_bp = 25.0;
        let context = test_context(&bond, &market, as_of);
        let result = effective_duration_convexity(&bond, &context, Some(shock_bp))
            .expect("effective duration/convexity");

        // Reconstruct the expected finite difference with all three prices on
        // the single `as_of` valuation date.
        let (risk_bond, _) = option_risk_bond_and_base_price(&bond, &context).expect("risk bond");
        let curve_id = option_risk_curve_id(&risk_bond);
        let market_up = market
            .bump([MarketBump::Curve {
                id: curve_id.clone(),
                spec: BumpSpec::parallel_bp(shock_bp),
            }])
            .expect("bump up");
        let market_down = market
            .bump([MarketBump::Curve {
                id: curve_id,
                spec: BumpSpec::parallel_bp(-shock_bp),
            }])
            .expect("bump down");
        let base = risk_bond.value(&market, as_of).expect("base").amount();
        let up = risk_bond.value(&market_up, as_of).expect("up").amount();
        let down = risk_bond.value(&market_down, as_of).expect("down").amount();
        let shock = shock_bp / 10_000.0;
        let expected_duration = (down - up) / (2.0 * base * shock);
        let expected_convexity = (up + down - 2.0 * base) / (base * shock * shock);

        assert!(
            (result.duration - expected_duration).abs() < 1e-9,
            "effective duration must use the as_of-anchored base price: \
             got {}, expected {}",
            result.duration,
            expected_duration
        );
        // The calculator's base price is the quote-implied price, while the
        // reconstruction reprices through the solved OAS; they agree only up
        // to the OAS-solver residual, which the convexity finite difference
        // amplifies by 1/shock² (~1.6e5). Tolerance sized accordingly.
        assert!(
            (result.convexity - expected_convexity).abs() < 1e-6,
            "effective convexity must use the as_of-anchored base price: \
             got {}, expected {}",
            result.convexity,
            expected_convexity
        );
    }

    /// Item 10 regression for the OAS-quote path: even with a settlement lag,
    /// direct quoted-OAS valuation and both bumped legs must place all three
    /// prices on the single `as_of` valuation date.
    #[test]
    fn effective_duration_uses_single_valuation_date_oas_quote() {
        use crate::instruments::fixed_income::bond::BondSettlementConvention;

        let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
            .expect("ok");
        let market = test_market(as_of);

        let maturity = as_of + time::Duration::days(5 * 365);
        let call_date = as_of + time::Duration::days(2 * 365);
        let mut bond = Bond::builder()
            .id("CALLABLE-OAS-LAG".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            // Quote via OAS directly (decimal: 0.005 = 50 bp).
            .instrument_pricing_overrides(
                InstrumentPricingOverrides::default().with_quoted_oas(0.005),
            )
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .attributes(Default::default())
            .build()
            .expect("valid bond");
        let mut schedule = CallPutSchedule::default();
        schedule.calls.push(CallPut {
            start_date: call_date,
            end_date: maturity,
            price_pct_of_par: 100.0,
            make_whole: None,
        });
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.01);
        bond.call_put = Some(schedule);

        let shock_bp = 25.0;
        let context = test_context(&bond, &market, as_of);
        let result = effective_duration_convexity(&bond, &context, Some(shock_bp))
            .expect("effective duration/convexity");

        let (risk_bond, _) = option_risk_bond_and_base_price(&bond, &context).expect("risk bond");
        let curve_id = option_risk_curve_id(&risk_bond);
        let market_up = market
            .bump([MarketBump::Curve {
                id: curve_id.clone(),
                spec: BumpSpec::parallel_bp(shock_bp),
            }])
            .expect("bump up");
        let market_down = market
            .bump([MarketBump::Curve {
                id: curve_id,
                spec: BumpSpec::parallel_bp(-shock_bp),
            }])
            .expect("bump down");
        let base = risk_bond.value(&market, as_of).expect("base").amount();
        let up = risk_bond.value(&market_up, as_of).expect("up").amount();
        let down = risk_bond.value(&market_down, as_of).expect("down").amount();
        let shock = shock_bp / 10_000.0;
        let expected_duration = (down - up) / (2.0 * base * shock);

        assert!(
            (result.duration - expected_duration).abs() < 1e-9,
            "effective duration (OAS quote) must use the as_of-anchored base: \
             got {}, expected {}",
            result.duration,
            expected_duration
        );
    }

    #[test]
    fn quoted_oas_risk_does_not_preprice_after_the_final_cashflow() {
        use crate::instruments::fixed_income::bond::BondSettlementConvention;

        let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
            .expect("valid valuation date");
        let maturity = as_of + time::Duration::days(1);
        let mut overrides = InstrumentPricingOverrides::default()
            .with_quoted_oas(0.005)
            .with_tree_steps(1)
            .with_mc_paths(8);
        overrides.model_config.hw1f_sigma = Some(0.01);
        overrides.model_config.hazard_volatility = Some(0.01);
        let mut bond = Bond::builder()
            .id("CALLABLE-OAS-AFTER-FINAL-CASH".into())
            .notional(Money::new(1_000.0, Currency::USD))
            .issue_date(as_of - time::Duration::days(365))
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(Some(CurveId::new("ACME-HZD")))
            .instrument_pricing_overrides(overrides)
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 3,
                ..Default::default()
            }))
            .attributes(Default::default())
            .build()
            .expect("valid bond");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: maturity,
                end_date: maturity,
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let market = test_market(as_of)
            .insert(HazardCurve::flat("ACME-HZD", as_of, 0.02, 0.4).expect("hazard curve"));

        let context = test_context(&bond, &market, as_of);
        let (risk_bond, base) = option_risk_bond_and_base_price(&bond, &context)
            .expect("quoted OAS risk setup must not invoke an unused post-maturity kernel");
        let direct = risk_bond
            .value(&market, as_of)
            .expect("direct as-of price")
            .amount();
        assert!(base > 0.0);
        assert_eq!(base, direct);
    }

    #[test]
    fn bullet_effective_duration_matches_modified() {
        let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
            .expect("ok");
        let market = test_market(as_of);
        let bond = bullet_bond(as_of);

        let risk_context = test_context(&bond, &market, as_of);
        let eff =
            effective_duration_convexity(&bond, &risk_context, Some(25.0)).expect("effective calc");

        // Compute modified duration via the metrics registry
        let base_pv = bond.value(&market, as_of).expect("value");
        let instrument_arc: Arc<dyn Instrument> = Arc::new(bond);
        let curves_arc = Arc::new(market);
        let registry = standard_registry();
        let mut ctx = MetricContext::new(
            instrument_arc,
            curves_arc,
            as_of,
            base_pv,
            MetricContext::default_config(),
        );
        registry
            .compute(
                &[
                    MetricId::Accrued,
                    MetricId::Ytm,
                    MetricId::DurationMac,
                    MetricId::DurationMod,
                ],
                &mut ctx,
            )
            .expect("metrics");

        let d_mod = ctx
            .computed
            .get(&MetricId::DurationMod)
            .copied()
            .expect("DurationMod metric should be computed");

        // For a bullet bond, effective duration ≈ modified duration (within ~0.5 due to bump size)
        assert!(
            (eff.duration - d_mod).abs() < 0.5,
            "Effective duration ({:.4}) should be close to modified duration ({:.4})",
            eff.duration,
            d_mod,
        );
    }

    #[test]
    fn callable_effective_duration_lower_than_bullet() {
        let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
            .expect("ok");
        let market = test_market(as_of);

        let bullet = bullet_bond(as_of);
        let callable = callable_bond(as_of);
        let bullet_context = test_context(&bullet, &market, as_of);
        let callable_context = test_context(&callable, &market, as_of);

        let eff_bullet =
            effective_duration(&bullet, &bullet_context, Some(25.0)).expect("bullet eff dur");
        let eff_callable =
            effective_duration(&callable, &callable_context, Some(25.0)).expect("callable eff dur");

        // Callable bond effective duration <= bullet (call caps upside)
        assert!(
            eff_callable <= eff_bullet + 0.01,
            "Callable effective duration ({:.4}) should be <= bullet ({:.4})",
            eff_callable,
            eff_bullet,
        );
    }

    #[test]
    fn callable_effective_convexity_lower_than_bullet() {
        let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
            .expect("ok");
        let market = test_market(as_of);

        let bullet = bullet_bond(as_of);
        let callable = callable_bond(as_of);
        let bullet_context = test_context(&bullet, &market, as_of);
        let callable_context = test_context(&callable, &market, as_of);

        let eff_bullet =
            effective_duration_convexity(&bullet, &bullet_context, Some(25.0)).expect("bullet");
        let eff_callable = effective_duration_convexity(&callable, &callable_context, Some(25.0))
            .expect("callable");

        // Callable convexity should be lower (possibly negative) relative to bullet
        assert!(
            eff_callable.convexity <= eff_bullet.convexity + 1.0,
            "Callable effective convexity ({:.4}) should be <= bullet ({:.4})",
            eff_callable.convexity,
            eff_bullet.convexity,
        );
    }

    #[test]
    fn effective_risk_reprices_all_legs_through_selected_custom_model() {
        let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
            .expect("valid date");
        let market = test_market(as_of);
        let mut bond = callable_bond(as_of);
        bond.instrument_pricing_overrides.market_quotes.quoted_oas = Some(0.002);

        let calls = Arc::new(AtomicUsize::new(0));
        let mut registry = PricerRegistry::new();
        registry
            .register(CurveSensitiveTreePricer {
                calls: Arc::clone(&calls),
            })
            .expect("unique custom pricer");
        let mut context = MetricContext::new(
            Arc::new(bond.clone()),
            Arc::new(market),
            as_of,
            Money::new(900.0, Currency::USD),
            MetricContext::default_config(),
        );
        context.set_pricer_dispatch(PricingDispatch::registered(
            ModelKey::Tree,
            Arc::new(registry),
        ));

        let result = effective_duration_convexity(&bond, &context, Some(25.0))
            .expect("selected-model effective risk");
        assert!(result.duration > 0.0);
        assert!(result.convexity > 0.0);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "base, up, and down must each use the selected registry pricer"
        );
    }

    #[test]
    fn callable_oas_metrics_requested_alone_preserve_selected_tree_dispatch() {
        let as_of = finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
            .expect("valid date");
        let market = test_market(as_of);
        let mut bond = callable_bond(as_of);
        bond.instrument_pricing_overrides.market_quotes.quoted_oas = Some(0.002);
        bond.metric_pricing_overrides = bond
            .metric_pricing_overrides
            .with_bond_risk_basis(BondRiskBasis::CallableOas);

        for metric in [MetricId::DurationMod, MetricId::Convexity, MetricId::Dv01] {
            let calls = Arc::new(AtomicUsize::new(0));
            let mut registry = PricerRegistry::new();
            registry
                .register(CurveSensitiveTreePricer {
                    calls: Arc::clone(&calls),
                })
                .expect("unique custom pricer");

            let result = registry
                .price_with_metrics(
                    &bond,
                    ModelKey::Tree,
                    &market,
                    as_of,
                    std::slice::from_ref(&metric),
                    PricingOptions::default(),
                )
                .expect("standalone callable-OAS metric");

            assert!(result.measures[&metric].is_finite());
            assert_eq!(
                calls.load(Ordering::SeqCst),
                4,
                "{metric} must use the selected Tree pricer for base, risk base, up, and down only"
            );
        }
    }
}
