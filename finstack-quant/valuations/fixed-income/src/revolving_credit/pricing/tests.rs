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
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
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
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");

    let market = MarketContext::new()
        .try_insert(disc_curve)
        .expect("insert market curve");

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
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
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
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");

    let market = MarketContext::new()
        .try_insert(disc_curve)
        .expect("insert market curve");

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

#[test]
fn b3_forward_start_recovery_and_upfront_fee_condition_on_issue_date() {
    use crate::loan_terms::UpfrontFee;
    use crate::revolving_credit::types::{CreditSpreadProcessSpec, McConfig};
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::market_data::traits::Discounting;
    use finstack_quant_valuations_core::instruments::Instrument;

    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid as_of");
    let issue = Date::from_calendar_date(2026, Month::January, 1).expect("valid issue");
    let maturity = Date::from_calendar_date(2028, Month::January, 1).expect("valid maturity");

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.03f64).exp()),
            (4.0, (-0.03f64 * 4.0).exp()),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("disc");

    let hazard = HazardCurve::builder("ACME-HZD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.5, 0.05), (5.0, 0.05)])
        .recovery_rate(0.40)
        .build()
        .expect("hazard");

    let market = MarketContext::new()
        .try_insert(disc_curve.clone())
        .expect("disc")
        .try_insert(hazard.clone())
        .expect("hazard");

    let make_det = |upfront: Option<UpfrontFee>| {
        let fees = RevolvingCreditFees {
            upfront_fee: upfront,
            ..Default::default()
        };
        RevolvingCredit::builder()
            .id("RC-FWD-DET".into())
            .commitment(Money::from((10_000_000_i64, Currency::USD)))
            .drawn(Money::from((5_000_000_i64, Currency::USD)))
            .issue_date(issue)
            .maturity(maturity)
            .rate(RateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act360)
            .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
            .frequency(Tenor::quarterly())
            .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
            .fees(fees)
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id("ACME-HZD".into())
            .recovery_rate(0.4)
            .build()
            .expect("det facility")
    };

    let fee_amount = Money::from((100_000_i64, Currency::USD));
    let pv_no_fee = make_det(None).value(&market, as_of).expect("pv no fee");
    let pv_with_fee = make_det(Some(UpfrontFee::Amount(fee_amount)))
        .value(&market, as_of)
        .expect("pv with fee");

    let df_issue = disc_curve
        .df_between_dates(as_of, issue)
        .expect("df to issue");
    let t_issue = hazard
        .day_count()
        .year_fraction(
            hazard.base_date(),
            issue,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .expect("t_issue");
    let s_issue = hazard.sp(t_issue).expect("sp issue");
    let expected_fee_pv = fee_amount.amount() * df_issue * s_issue;
    let actual_fee_pv = pv_with_fee.amount() - pv_no_fee.amount();
    assert!(
        (actual_fee_pv - expected_fee_pv).abs() < 1e-6,
        "expected survival-weighted upfront fee PV {expected_fee_pv}, got {actual_fee_pv}"
    );

    // Stochastic forward-start must price cleanly without grid/range errors.
    let stoch = RevolvingCredit::builder()
        .id("RC-FWD-STOCH".into())
        .commitment(Money::from((10_000_000_i64, Currency::USD)))
        .drawn(Money::from((5_000_000_i64, Currency::USD)))
        .issue_date(issue)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
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
                mc_config: Some(McConfig {
                    credit_spread_process: CreditSpreadProcessSpec::Constant(0.02),
                    interest_rate_process: None,
                    correlation_matrix: None,
                    util_credit_corr: None,
                }),
            },
        )))
        .instrument_pricing_overrides(
            finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::default()
                .with_mc_paths(64),
        )
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .expect("stoch facility");

    let pv_stoch = stoch.value(&market, as_of).expect("stoch forward-start PV");
    assert!(pv_stoch.amount().is_finite());
}

#[test]
fn h24_constant_spread_has_zero_diffusion_and_expected_loss_handles_mc_credit() {
    use crate::revolving_credit::types::{CreditSpreadProcessSpec, McConfig};
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::metrics::MetricId;

    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let end = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (2.0, (-0.03f64 * 2.0).exp())])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("disc");
    let market = MarketContext::new()
        .try_insert(disc_curve)
        .expect("insert market curve");

    let mc_config = McConfig {
        credit_spread_process: CreditSpreadProcessSpec::Constant(0.02),
        interest_rate_process: None,
        correlation_matrix: None,
        util_credit_corr: None,
    };

    let facility = RevolvingCredit::builder()
        .id("RC-MC-EL".into())
        .commitment(Money::from((10_000_000_i64, Currency::USD)))
        .drawn(Money::from((5_000_000_i64, Currency::USD)))
        .issue_date(start)
        .maturity(end)
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
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
                .with_mc_paths(64),
        )
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .expect("mc el facility");

    let mc_paths = RevolvingCreditPricer::price_with_paths(&facility, &market, start)
        .expect("price with paths");
    assert!(!mc_paths.path_results.is_empty());
    for path in &mc_paths.path_results {
        let data = path.path_data.as_ref().expect("path_data");
        for &s in &data.credit_spread_path {
            assert!(
                (s - 0.02).abs() == 0.0,
                "constant spread must have zero diffusion across all paths/steps, got {s}"
            );
        }
    }

    assert!(facility.credit_curve_id.is_none());
    let priced = facility
        .price_with_metrics(
            &market,
            start,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .expect("price with metrics");
    let el = *priced
        .measures
        .get(&MetricId::ExpectedLoss)
        .expect("expected loss metric");
    assert!(
        el > 0.0,
        "stochastic MC credit spread must produce positive expected loss without credit_curve_id, got {el}"
    );
}
