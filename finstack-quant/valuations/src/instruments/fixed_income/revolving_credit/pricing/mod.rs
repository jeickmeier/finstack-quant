//! Revolving-credit pricing.
//!
//! [`RevolvingCreditPricer`] handles deterministic and stochastic facilities.
//! Path generation lives in [`path_generator`].

mod components;
pub mod monte_carlo_discretization;
pub mod monte_carlo_process;
pub mod path_generator;
mod path_pricing;
mod results;
mod stochastic;
pub(crate) mod unified;

pub(crate) use unified::RevolvingCreditPricer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricer::{InstrumentType, ModelKey, Pricer, PricerKey};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use time::Month;

    use super::super::types::{BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees};

    #[test]
    fn test_unified_pricer_key() {
        let pricer = RevolvingCreditPricer::default();
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
            .commitment_amount(Money::from((10_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((5_000_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
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
        use super::super::types::{CreditSpreadProcessSpec, McConfig};
        use crate::instruments::Instrument;

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
            .commitment_amount(Money::from((10_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((5_000_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
                super::super::types::StochasticUtilizationSpec {
                    utilization_process: super::super::types::UtilizationProcess::MeanReverting {
                        target_rate: 0.5,
                        speed: 1.0,
                        volatility: 0.1,
                        spread_sensitivity: 0.0,
                    },
                    num_paths: 100,
                    seed: Some(42),
                    antithetic: false,
                    use_sobol_qmc: false,
                    mc_config: Some(mc_config),
                },
            )))
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
        let registry_value = crate::pricer::shared_standard_registry()
            .price_with_metrics(
                &facility,
                ModelKey::Discounting,
                &market,
                start,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("registry lifecycle")
            .value;

        // Verify we got a valid result
        assert!(pv_unified.currency() == Currency::USD);
        assert_eq!(pv_direct, pv_unified);
        assert_eq!(registry_value, pv_unified);
    }
}
