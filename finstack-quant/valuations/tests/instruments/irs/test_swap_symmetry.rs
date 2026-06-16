//! Property-based tests for interest rate swap symmetry.
//!
//! Key Property: For any swap at inception with notional N and fixed rate F:
//! - DV01(Pay) = -DV01(Receive)
//! - PV(Pay) + PV(Receive) ≈ 0 (at par rate)

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::irs::PayReceive;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use proptest::prelude::*;
use time::Month;

fn create_test_market(base_date: Date, short_rate: f64, long_rate: f64) -> MarketContext {
    // Create a reasonable discount curve
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, (-short_rate).exp()),
            (5.0, (-short_rate * 3.0 - long_rate * 2.0).exp()), // Blend to long rate
            (10.0, (-long_rate * 10.0).exp()),
        ])
        .build()
        .unwrap();

    // Forward curve with slightly different rates
    let fwd_rate = (short_rate + long_rate) / 2.0;
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base_date)
        .knots([
            (0.0, fwd_rate),
            (1.0, fwd_rate * 1.05),
            (5.0, fwd_rate * 1.10),
            (10.0, fwd_rate * 1.15),
        ])
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(fwd)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_swap_dv01_symmetry(
        notional in 1_000_000.0..100_000_000.0,
        fixed_rate in 0.01..0.10,
        tenor_years in 1..=10,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let start = base_date;
        let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 15).unwrap();

        let swap_pay = test_utils::usd_irs_swap(
            "PAY-FIXED",
            Money::new(notional, Currency::USD),
            fixed_rate,
            start,
            end,
            PayReceive::Pay,
        )
        .expect("Valid swap construction");

        let swap_rec = test_utils::usd_irs_swap(
            "RECEIVE-FIXED",
            Money::new(notional, Currency::USD),
            fixed_rate,
            start,
            end,
            PayReceive::Receive,
        )
        .expect("Valid swap construction");

        // Create market with reasonable rates
        let market = create_test_market(base_date, 0.03, 0.05);

        let result_pay = swap_pay.price_with_metrics(&market, base_date, &[MetricId::Dv01], finstack_quant_valuations::instruments::PricingOptions::default()).unwrap();
        let result_rec = swap_rec.price_with_metrics(&market, base_date, &[MetricId::Dv01], finstack_quant_valuations::instruments::PricingOptions::default()).unwrap();

        let dv01_pay = result_pay.measures[MetricId::Dv01.as_str()];
        let dv01_rec = result_rec.measures[MetricId::Dv01.as_str()];

        // Property: DV01(pay) + DV01(receive) ≈ 0
        let dv01_sum = dv01_pay + dv01_rec;

        prop_assert!(
            dv01_sum.abs() < 1e-6,
            "DV01 symmetry violated: Pay DV01 = {:.6}, Receive DV01 = {:.6}, Sum = {:.6}",
            dv01_pay, dv01_rec, dv01_sum
        );
    }

    #[test]
    fn prop_swap_pv_symmetry_at_par_rate(
        notional in 1_000_000.0..100_000_000.0,
        tenor_years in 1..=10,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let start = base_date;
        let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 15).unwrap();

        let market = create_test_market(base_date, 0.04, 0.05);

        // First, find the par rate for this maturity
        let temp_swap = test_utils::usd_irs_swap(
            "PAR-FINDER",
            Money::new(notional, Currency::USD),
            0.04, // temporary rate
            start,
            end,
            PayReceive::Pay,
        )
        .expect("Valid swap construction");

        let par_result = temp_swap
            .price_with_metrics(&market, base_date, &[MetricId::ParRate], finstack_quant_valuations::instruments::PricingOptions::default())
            .unwrap();
        let par_rate = par_result.measures[MetricId::ParRate.as_str()];

        // Skip if par rate is unreasonable
        prop_assume!(par_rate > 0.001 && par_rate < 0.20);

        // Create swaps at par rate
        let swap_pay = test_utils::usd_irs_swap(
            "PAY-AT-PAR",
            Money::new(notional, Currency::USD),
            par_rate,
            start,
            end,
            PayReceive::Pay,
        )
        .expect("Valid swap construction");

        let swap_rec = test_utils::usd_irs_swap(
            "REC-AT-PAR",
            Money::new(notional, Currency::USD),
            par_rate,
            start,
            end,
            PayReceive::Receive,
        )
        .expect("Valid swap construction");

        let pv_pay = swap_pay.value(&market, base_date).unwrap().amount();
        let pv_rec = swap_rec.value(&market, base_date).unwrap().amount();

        // Property: At par rate, PV(pay) + PV(receive) ≈ 0
        let pv_sum = pv_pay + pv_rec;

        prop_assert!(
            pv_sum.abs() < 1.0, // Within $1 for million+ notionals
            "PV symmetry at par violated: Pay PV = {:.2}, Receive PV = {:.2}, Sum = {:.2}",
            pv_pay, pv_rec, pv_sum
        );
    }

    #[test]
    fn prop_swap_annuity_positive(
        notional in 1_000_000.0..100_000_000.0,
        fixed_rate in 0.01..0.10,
        tenor_years in 1..=10,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let start = base_date;
        let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 15).unwrap();

        let swap = test_utils::usd_irs_swap(
            "ANNUITY-TEST",
            Money::new(notional, Currency::USD),
            fixed_rate,
            start,
            end,
            PayReceive::Pay,
        )
        .expect("Valid swap construction");

        let market = create_test_market(base_date, 0.03, 0.05);

        let result = swap.price_with_metrics(&market, base_date, &[MetricId::Annuity], finstack_quant_valuations::instruments::PricingOptions::default()).unwrap();
        let annuity = result.measures[MetricId::Annuity.as_str()];

        // Property: Annuity must always be positive
        prop_assert!(
            annuity > 0.0,
            "Annuity must be positive, got: {:.6}",
            annuity
        );

        // Property: Annuity should be less than tenor (due to discounting)
        prop_assert!(
            annuity < tenor_years as f64,
            "Annuity {:.6} should be less than tenor {} due to discounting",
            annuity, tenor_years
        );
    }
}
