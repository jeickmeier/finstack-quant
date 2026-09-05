//! Embedded option value calculator for callable, putable, and return-floor bonds.
//!
//! Computes the theoretical value of embedded exercise rights by pricing:
//! 1. With call, put, and return-floor constraints → P_embedded
//! 2. Without those constraints under the same caller-selected model → P_straight
//!
//! The embedded option value is the difference between these prices.
//!
//! # Option Value Decomposition
//!
//! ## Callable Bonds (Issuer Owns the Call)
//!
//! ```text
//! V_call_holder = P_callable - P_straight  (negative)
//! ```
//!
//! The call option has positive value to the issuer, reducing the price
//! the investor pays.
//!
//! ## Putable Bonds (Investor Owns the Put)
//!
//! ```text
//! P_putable = P_straight + V_put
//! V_put = P_putable - P_straight  (positive)
//! ```
//!
//! The put option has positive value to the investor, increasing the price.
//!
//! ## Bonds with Both Options
//!
//! For bonds with both calls and puts, the metric returns the **net** option
//! value from the investor's perspective:
//!
//! ```text
//! V_net = P_embedded - P_straight
//! ```
//!
//! - Positive: Put value dominates (benefits investor)
//! - Negative: Call value dominates (benefits issuer)
//!
//! # Examples
//!
//! ```text
//! use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
//! use finstack_quant_valuations::metrics::{MetricRegistry, MetricId};
//!
//! # let bond = Bond::example().unwrap();
//! // Register metrics and compute
//! // V_call_holder will be negative for callable bonds
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

fn price_bond_with_context_model(
    context: &MetricContext,
    bond: &Bond,
    model: crate::pricer::ModelKey,
) -> finstack_quant_core::Result<f64> {
    if context.pricing_model().is_some() {
        context.reprice_instrument_raw(bond, context.curves.as_ref(), context.as_of)
    } else {
        bond.price_for_model_raw(model, context.curves.as_ref(), context.as_of)
    }
}

/// Calculates the embedded option value for callable, putable, and
/// return-floor bonds.
///
/// Computes the difference between:
/// - The option-embedded bond price (with call/put/return-floor exercise decisions)
/// - The straight-bond price under the same caller-selected model and OAS
///
/// # Returns
///
/// - For **callable bonds**: Negative holder value (call reduces holder value)
/// - For **putable bonds**: Positive holder value (put increases holder value)
/// - For **bonds with both**: Net option value from investor perspective
/// - For **return-floor bonds**: Holder value of the protected redemption path
/// - For **straight bonds**: Zero (no embedded options)
///
/// The value is returned in **currency units** (same as bond notional).
///
/// # Dependencies
///
/// None. The metric reprices both legs through the caller-selected model and
/// pricer registry.
///
#[derive(Debug, Clone, Default)]
pub(crate) struct EmbeddedOptionValueCalculator;

impl MetricCalculator for EmbeddedOptionValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: Bond = context.instrument_as::<Bond>()?.clone();

        // If bond has no embedded options, return 0
        let has_options = bond.return_floor.is_some()
            || bond.call_put.as_ref().is_some_and(|cp| cp.has_options());

        if !has_options {
            return Ok(0.0);
        }

        let oas_decimal = if let Some(oas) = context.computed.get(&MetricId::Oas) {
            *oas
        } else {
            super::oas::oas_decimal_from_quote_overrides(&bond, context)?.unwrap_or(0.0)
        };

        let model = context
            .pricing_model()
            .unwrap_or_else(|| bond.default_pricing_model());
        let mut optioned_bond = bond.clone();
        clear_price_driving_overrides(&mut optioned_bond);
        optioned_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_oas = Some(oas_decimal);
        let price_with_options = price_bond_with_context_model(context, &optioned_bond, model)?;
        let mut straight_bond = bond;
        straight_bond.call_put = None;
        straight_bond.return_floor = None;
        clear_price_driving_overrides(&mut straight_bond);
        straight_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_oas = Some(oas_decimal);
        let price_straight = price_bond_with_context_model(context, &straight_bond, model)?;

        Ok(price_with_options - price_straight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::bond::pricing::engine::tree::{
        bond_tree_config, TreePricer,
    };
    use crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_oas;
    use crate::instruments::fixed_income::bond::BondSettlementConvention;
    use crate::instruments::fixed_income::bond::CashflowSpec;
    use crate::instruments::fixed_income::bond::{
        CallPut, CallPutSchedule, ProtectionWindow, ReturnFloorSpec,
    };
    use crate::instruments::InstrumentPricingOverrides;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::Month;

    fn create_test_market() -> MarketContext {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.85), (10.0, 0.70)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Valid curve");
        MarketContext::new().insert(discount_curve)
    }

    fn create_callable_bond() -> Bond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
        let call_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

        let mut call_put = CallPutSchedule::default();
        call_put.calls.push(CallPut {
            start_date: call_date,
            end_date: call_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        });

        Bond::builder()
            .id("CALLABLE_BOND".into())
            .notional(Money::from((
                1000_i64,
                finstack_quant_core::currency::Currency::USD,
            )))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::semi_annual(),
                    finstack_quant_core::dates::DayCount::Act365F,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides {
                market_quotes: crate::instruments::MarketQuoteOverrides {
                    implied_volatility: Some(0.01),
                    ..Default::default()
                },
                ..Default::default()
            })
            .call_put_opt(Some(call_put))
            .custom_cashflows_opt(None)
            .attributes(Default::default())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()
            .expect("Valid bond")
    }

    fn create_putable_bond() -> Bond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
        let put_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

        let mut call_put = CallPutSchedule::default();
        call_put.puts.push(CallPut {
            start_date: put_date,
            end_date: put_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        });

        Bond::builder()
            .id("PUTABLE_BOND".into())
            .notional(Money::from((
                1000_i64,
                finstack_quant_core::currency::Currency::USD,
            )))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::semi_annual(),
                    finstack_quant_core::dates::DayCount::Act365F,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides {
                market_quotes: crate::instruments::MarketQuoteOverrides {
                    implied_volatility: Some(0.01),
                    ..Default::default()
                },
                ..Default::default()
            })
            .call_put_opt(Some(call_put))
            .custom_cashflows_opt(None)
            .attributes(Default::default())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()
            .expect("Valid bond")
    }

    fn create_straight_bond() -> Bond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");

        Bond::builder()
            .id("STRAIGHT_BOND".into())
            .notional(Money::from((
                1000_i64,
                finstack_quant_core::currency::Currency::USD,
            )))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::semi_annual(),
                    finstack_quant_core::dates::DayCount::Act365F,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides {
                market_quotes: crate::instruments::MarketQuoteOverrides {
                    implied_volatility: Some(0.01),
                    ..Default::default()
                },
                ..Default::default()
            })
            .call_put_opt(None)
            .custom_cashflows_opt(None)
            .attributes(Default::default())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            }))
            .build()
            .expect("Valid bond")
    }

    #[test]
    fn test_straight_bond_returns_zero() {
        let bond = create_straight_bond();
        let market = create_test_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let calc = EmbeddedOptionValueCalculator;
        let base_value = bond.value(&market, as_of).expect("Should price");

        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let option_value = calc.calculate(&mut context).expect("Should calculate");

        assert!(
            option_value.abs() < 1e-10,
            "Straight bond should have zero option value, got {}",
            option_value
        );
    }

    #[test]
    fn test_callable_bond_positive_option_value() {
        let bond = create_callable_bond();
        let market = create_test_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let calc = EmbeddedOptionValueCalculator;
        let base_value = bond.value(&market, as_of).expect("Should price");

        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let option_value = calc.calculate(&mut context).expect("Should calculate");

        assert!(
            option_value < 0.0,
            "Callable bond holder option value should be negative, got {}",
            option_value
        );
    }

    #[test]
    fn test_putable_bond_positive_option_value() {
        let bond = create_putable_bond();
        let market = create_test_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

        let calc = EmbeddedOptionValueCalculator;
        let base_value = bond.value(&market, as_of).expect("Should price");

        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let option_value = calc.calculate(&mut context).expect("Should calculate");

        assert!(
            option_value > 0.0,
            "Putable bond should have positive put option value, got {}",
            option_value
        );
    }

    #[test]
    fn floor_only_bond_has_option_value_against_fully_straight_leg() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut bond = create_straight_bond();
        bond.return_floor = Some(
            ReturnFloorSpec::moic(1.0).window(ProtectionWindow::Between {
                start: Date::from_calendar_date(2027, Month::January, 1).expect("valid date"),
                end: Date::from_calendar_date(2027, Month::January, 2).expect("valid date"),
            }),
        );
        let market = create_test_market();
        let optioned = price_from_oas(&bond, &market, as_of, crate::pricer::ModelKey::Tree, 0.0)
            .expect("floor-only price");
        let mut straight = bond.clone();
        straight.call_put = None;
        straight.return_floor = None;
        let straight_price = price_from_oas(
            &straight,
            &market,
            as_of,
            crate::pricer::ModelKey::Tree,
            0.0,
        )
        .expect("fully straight price");

        let base_value = bond.value(&market, as_of).expect("base price");
        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let actual = EmbeddedOptionValueCalculator
            .calculate(&mut context)
            .expect("floor-only embedded option value");

        assert!(
            actual.abs() > 1.0e-6,
            "a binding return floor must not be classified as a straight bond"
        );
        assert!(
            (actual - (optioned - straight_price)).abs() < 1.0e-8,
            "straight leg must remove call_put and return_floor: actual={actual}, expected={}",
            optioned - straight_price
        );
    }

    #[test]
    fn standalone_eov_normalizes_equivalent_clean_and_dirty_quotes() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let market = create_test_market();
        let mut clean_bond = create_callable_bond();
        clean_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(100.0);
        clean_bond
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(16);
        let quote_context =
            crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext::new(
                &clean_bond,
                &market,
                as_of,
            )
            .expect("quote context");
        let dirty_amount = quote_context.dirty_from_clean_pct(100.0, clean_bond.notional.amount());
        let mut dirty_bond = clean_bond.clone();
        dirty_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = None;
        dirty_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_dirty_price_currency = Some(dirty_amount);

        let calculate = |bond: Bond| {
            let base_value = bond.value(&market, as_of).expect("quoted base value");
            let mut context = MetricContext::new(
                Arc::new(bond),
                Arc::new(market.clone()),
                as_of,
                base_value,
                MetricContext::default_config(),
            );
            EmbeddedOptionValueCalculator
                .calculate(&mut context)
                .expect("standalone EOV")
        };
        let from_clean = calculate(clean_bond);
        let from_dirty = calculate(dirty_bond);
        assert!(
            (from_clean - from_dirty).abs() < 1.0e-8,
            "equivalent settlement clean and dirty quotes must resolve the same OAS: clean={from_clean}, dirty={from_dirty}"
        );
    }

    #[test]
    fn credit_option_value_uses_same_rates_credit_model_for_straight_leg() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut bond = create_callable_bond();
        bond.credit_curve_id = Some("USD-CREDIT".into());
        bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
            .with_quoted_oas(0.0025)
            .with_hw1f_sigma(0.0)
            .with_hazard_volatility(0.0)
            .with_tree_steps(16);
        let market = create_test_market().insert(
            HazardCurve::builder("USD-CREDIT")
                .base_date(as_of)
                .recovery_rate(0.4)
                .knots([(0.0, 0.02), (5.0, 0.02)])
                .build()
                .expect("valid hazard curve"),
        );
        let pricer = TreePricer::rates_credit(bond_tree_config(&bond).expect("tree config"));
        let optioned = pricer
            .price_at_oas(&bond, &market, as_of, 25.0)
            .expect("optioned rates-credit price");
        let mut straight = bond.clone();
        straight.call_put = None;
        let bullet = pricer
            .price_at_oas(&straight, &market, as_of, 25.0)
            .expect("straight rates-credit price");

        let base_value = bond.value(&market, as_of).expect("base price");
        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let actual = EmbeddedOptionValueCalculator
            .calculate(&mut context)
            .expect("embedded option value");

        assert!(
            (actual - (optioned - bullet)).abs() < 1e-8,
            "both legs must retain the same hazard/recovery model: actual={actual}, expected={}",
            optioned - bullet
        );
    }
}
