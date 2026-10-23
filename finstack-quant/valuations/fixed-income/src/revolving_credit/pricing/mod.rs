//! Revolving-credit pricing.
//!
//! [`crate::revolving_credit::RevolvingCreditPricer`] handles deterministic and stochastic facilities.
//! Path generation lives in [`path_generator`].

pub(super) mod path_generator;
mod path_pricing;
pub(super) mod results;
mod stochastic;
pub(super) mod unified;

#[cfg(test)]
mod tests {
    use crate::revolving_credit::RevolvingCreditPricer;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::pricer::{InstrumentType, ModelKey, Pricer, PricerKey};
    use time::Month;

    use crate::revolving_credit::types::{DrawRepaySpec, RevolvingCredit, RevolvingCreditFees};
    use finstack_quant_cashflows::builder::RateSpec;

    #[test]
    fn test_unified_pricer_key() {
        let pricer = RevolvingCreditPricer::new(ModelKey::Discounting);
        assert_eq!(
            pricer.key(),
            PricerKey::new(InstrumentType::RevolvingCredit, ModelKey::Discounting)
        );
    }

    #[test]
    fn test_unified_pricer_deterministic() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let facility = RevolvingCredit::builder()
            .id("RC-UNIFIED-DET".into())
            .commitment(Money::from((10_000_000_i64, Currency::USD)))
            .drawn(Money::from((5_000_000_i64, Currency::USD)))
            .issue_date(start)
            .maturity(end)
            .rate(RateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.0)
            .build()
            .expect("should succeed");

        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03f64).exp()),
                (5.0, (-0.03f64 * 5.0).exp()),
            ])
            .build()
            .expect("should succeed");

        let market = MarketContext::new().insert(disc_curve);

        // Test unified pricing
        let pv_unified =
            RevolvingCreditPricer::price(&facility, &market, start).expect("should succeed");

        // Verify we got a valid result
        assert!(pv_unified.currency() == Currency::USD);
        // For a fixed-rate facility with no fees and positive interest, value should be negative
        // (lender deploys capital initially and receives interest back over time, NPV depends on rates)
        // With 5% facility rate and 3% discount rate, NPV should be negative
    }

    #[test]
    fn test_unified_pricer_stochastic() {
        use crate::revolving_credit::types::{CreditSpreadProcessSpec, McConfig};
        use finstack_quant_valuations_core::instruments::Instrument;

        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let mc_config = McConfig {
            credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
            interest_rate_process: None,
            correlation_matrix: None,
            util_credit_corr: None,
        };

        let facility = RevolvingCredit::builder()
            .id("RC-UNIFIED-STOCH".into())
            .commitment(Money::from((10_000_000_i64, Currency::USD)))
            .drawn(Money::from((5_000_000_i64, Currency::USD)))
            .issue_date(start)
            .maturity(end)
            .rate(RateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                crate::revolving_credit::types::StochasticUtilizationSpec {
                    utilization_process:
                        crate::revolving_credit::types::UtilizationProcess::MeanReverting {
                            theta: 0.5,
                            kappa: 1.0,
                            sigma: 0.1,
                            spread_sensitivity: 0.0,
                        },
                    mc_config: Some(mc_config),
                },
            )))
            .instrument_pricing_overrides(
                finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::default()
                    .with_mc_paths(100),
            )
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.4)
            .build()
            .expect("should succeed");

        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03f64).exp()),
                (5.0, (-0.03f64 * 5.0).exp()),
            ])
            .build()
            .expect("should succeed");

        let market = MarketContext::new().insert(disc_curve);

        // Test unified pricing
        let pv_unified =
            RevolvingCreditPricer::price(&facility, &market, start).expect("should succeed");

        let pv_direct = facility
            .value(&market, start)
            .expect("direct instrument lifecycle");
        let registry_value = crate::FIXED_INCOME
            .shared_pricers()
            .price_with_metrics(
                &facility,
                ModelKey::Discounting,
                &market,
                start,
                &[],
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .expect("registry lifecycle")
            .value;

        // Verify we got a valid result
        assert!(pv_unified.currency() == Currency::USD);
        assert_eq!(pv_direct, pv_unified);
        assert_eq!(registry_value, pv_unified);
    }
}
