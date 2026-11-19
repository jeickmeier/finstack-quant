//! Determinism tests for interest rate swap pricing.
//!
//! Verifies that IRS valuation produces bitwise-identical results across
//! multiple runs with the same inputs, and validates correctness against
//! market standards.

use crate::support::rates as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_rates::irs::InterestRateSwap;
use finstack_quant_valuations_test_support::tolerances;
use time::Month;

fn create_test_swap() -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 15).unwrap();

    test_utils::usd_irs_swap(
        "IRS-DETERMINISM-TEST",
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        0.04, // 4% fixed rate
        start,
        end,
        PayReceive::Pay,
    )
    .expect("Valid swap construction")
}

fn create_test_market(base_date: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::Linear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base_date)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.995611221459588),
            (0.25, 0.991195240930358),
            (0.375, 0.986752592135641),
            (0.5, 0.982283810160798),
            (0.625, 0.977789431347014),
            (0.75, 0.973269993185277),
            (0.875, 0.968726034210573),
            (1.0, 0.964158093896324),
            (1.125, 0.959574209193318),
            (1.25, 0.954982273933355),
            (1.375, 0.950382612946264),
            (1.5, 0.945775550224929),
            (1.625, 0.941161408888927),
            (1.75, 0.936540511148462),
            (1.875, 0.93191317826863),
            (2.0, 0.927279730533999),
            (2.1875, 0.920324184202576),
            (2.375, 0.913367292399534),
            (2.5625, 0.906409877451143),
            (2.75, 0.899452755535353),
            (2.9375, 0.892496736538075),
            (3.125, 0.885542623912392),
            (3.3125, 0.878591214540736),
            (3.5, 0.871643298600053),
            (3.6875, 0.864699659429976),
            (3.875, 0.857761073404031),
            (4.0625, 0.850828309803906),
            (4.25, 0.843902130696782),
            (4.4375, 0.836983290815769),
            (4.625, 0.830072537443442),
            (4.8125, 0.823170610298508),
            (5.0, 0.81627824142561),
            (5.3125, 0.804840362948709),
            (5.625, 0.793485262077054),
            (5.9375, 0.782213972982446),
            (6.25, 0.771027490817996),
            (6.5625, 0.759926771886431),
            (6.875, 0.74891273382482),
            (7.1875, 0.737986255805403),
            (7.5, 0.727148178752154),
            (7.8125, 0.71639930557274),
            (8.125, 0.705740401405505),
            (8.4375, 0.695172193881119),
            (8.75, 0.684695373398532),
            (9.0625, 0.674310593414836),
            (9.375, 0.664018470748686),
            (9.6875, 0.653819585896884),
            (10.0, 0.643714483363737),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Provide fixings for seasoned swap tests where as_of > start
    let fixings = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![
            (
                Date::from_calendar_date(2025, Month::January, 15).unwrap(),
                0.035,
            ), // First reset fixing
        ],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixings series");

    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve")
        .insert_series(fixings)
}

#[test]
fn test_swap_pv_determinism() {
    let swap = create_test_swap();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Price the swap 100 times
    let prices: Vec<f64> = (0..100)
        .map(|_| swap.value(&market, as_of).unwrap().amount())
        .collect();

    // All prices must be bitwise identical
    for i in 1..prices.len() {
        assert_eq!(
            prices[i], prices[0],
            "Swap PV at iteration {} = {:.15} differs from iteration 0 = {:.15}",
            i, prices[i], prices[0]
        );
    }
}

#[test]
fn test_swap_dv01_determinism() {
    let swap = create_test_swap();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate DV01 50 times
    let dv01s: Vec<f64> = (0..50)
        .map(|_| {
            let result = swap
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::Dv01],
                    finstack_quant_valuations_core::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::Dv01.as_str()]
        })
        .collect();

    // All DV01s must be identical
    for i in 1..dv01s.len() {
        assert_eq!(
            dv01s[i], dv01s[0],
            "DV01 differs at iteration {}: {:.15} vs {:.15}",
            i, dv01s[i], dv01s[0]
        );
    }
}

#[test]
fn test_swap_annuity_determinism() {
    let swap = create_test_swap();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate annuity 50 times
    let annuities: Vec<f64> = (0..50)
        .map(|_| {
            let result = swap
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::Annuity],
                    finstack_quant_valuations_core::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::Annuity.as_str()]
        })
        .collect();

    // All annuities must be identical
    for i in 1..annuities.len() {
        assert_eq!(
            annuities[i], annuities[0],
            "Annuity differs at iteration {}: {:.15} vs {:.15}",
            i, annuities[i], annuities[0]
        );
    }
}

#[test]
fn test_swap_par_rate_determinism() {
    let swap = create_test_swap();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Calculate par rate 50 times
    let par_rates: Vec<f64> = (0..50)
        .map(|_| {
            let result = swap
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::ParRate],
                    finstack_quant_valuations_core::instruments::PricingOptions::default(),
                )
                .unwrap();
            result.measures[MetricId::ParRate.as_str()]
        })
        .collect();

    // All par rates must be identical
    for i in 1..par_rates.len() {
        assert_eq!(
            par_rates[i], par_rates[0],
            "Par rate differs at iteration {}: {:.15} vs {:.15}",
            i, par_rates[i], par_rates[0]
        );
    }

    // Correctness: par rate should be reasonable (between 1% and 10%)
    assert!(
        par_rates[0] > 0.01 && par_rates[0] < 0.10,
        "Par rate {} outside reasonable range [1%, 10%]",
        par_rates[0]
    );
}

#[test]
fn test_swap_multiple_metrics_determinism() {
    let swap = create_test_swap();
    let as_of = Date::from_calendar_date(2025, Month::February, 1).unwrap();
    // Base the curves at the swap start (2025-01-15): seasoned resets strictly
    // before the curve base now error under the default FloatingRateFallback.
    let market = create_test_market(Date::from_calendar_date(2025, Month::January, 15).unwrap());

    let metrics = vec![
        MetricId::Dv01,
        MetricId::Annuity,
        MetricId::ParRate,
        MetricId::PvFixed,
        MetricId::PvFloat,
    ];

    // Calculate all metrics 30 times
    let results: Vec<_> = (0..30)
        .map(|_| {
            swap.price_with_metrics(
                &market,
                as_of,
                &metrics,
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .unwrap()
        })
        .collect();

    // Verify each metric is deterministic
    for metric in &metrics {
        let values: Vec<f64> = results
            .iter()
            .map(|r| r.measures[metric.as_str()])
            .collect();

        for i in 1..values.len() {
            assert_eq!(
                values[i],
                values[0],
                "{} differs at iteration {}: {:.15} vs {:.15}",
                metric.as_str(),
                i,
                values[i],
                values[0]
            );
        }
    }
}

#[test]
fn test_swap_pay_vs_receive_determinism() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Create both pay-fixed and receive-fixed swaps
    let start = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 15).unwrap();

    let swap_pay = test_utils::usd_irs_swap(
        "PAY-FIXED",
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        0.04,
        start,
        end,
        PayReceive::Pay,
    )
    .expect("Valid swap construction");

    let swap_rec = test_utils::usd_irs_swap(
        "RECEIVE-FIXED",
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        0.04,
        start,
        end,
        PayReceive::Receive,
    )
    .expect("Valid swap construction");

    // Price each swap 30 times
    let pay_prices: Vec<f64> = (0..30)
        .map(|_| swap_pay.value(&market, as_of).unwrap().amount())
        .collect();

    let rec_prices: Vec<f64> = (0..30)
        .map(|_| swap_rec.value(&market, as_of).unwrap().amount())
        .collect();

    // Verify determinism for each side
    for i in 1..pay_prices.len() {
        assert_eq!(pay_prices[i], pay_prices[0]);
        assert_eq!(rec_prices[i], rec_prices[0]);
    }

    // Verify symmetry: pay + receive ≈ 0 (and this relationship is deterministic)
    let sym_sum: Vec<f64> = pay_prices
        .iter()
        .zip(rec_prices.iter())
        .map(|(p, r)| p + r)
        .collect();

    for i in 1..sym_sum.len() {
        assert_eq!(
            sym_sum[i], sym_sum[0],
            "Symmetry sum differs at iteration {}: {:.15} vs {:.15}",
            i, sym_sum[i], sym_sum[0]
        );
    }

    // Correctness: pay + receive should sum to approximately zero
    let notional = 10_000_000.0;
    let sym_tolerance = notional * tolerances::NUMERICAL; // 0.01%
    assert!(
        sym_sum[0].abs() < sym_tolerance,
        "Pay + Receive sum {} should be ~0 (tolerance {})",
        sym_sum[0],
        sym_tolerance
    );
}
