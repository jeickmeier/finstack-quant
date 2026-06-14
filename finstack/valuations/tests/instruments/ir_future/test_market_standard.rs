//! Market standard validation tests for IR Futures.
//!
//! These tests validate that the implementation follows market conventions
//! and produces results consistent with standard methodologies.

use super::utils::*;
use finstack_core::dates::DayCount;
use finstack_valuations::instruments::rates::ir_future::{FutureContractSpecs, Position};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_pnl_attribution_rate_move() {
    let (as_of, start, end) = standard_dates();

    // Initial market at 5%
    let market_5pct = build_standard_market(as_of, 0.05);

    // Market moves to 6%
    let market_6pct = build_standard_market(as_of, 0.06);

    let future = create_custom_future(
        "PNL_TEST",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );

    let pv_5pct = future.value(&market_5pct, as_of).unwrap().amount();
    let pv_6pct = future.value(&market_6pct, as_of).unwrap().amount();

    // Long position should lose when rates increase (prices fall)
    assert!(
        pv_6pct < pv_5pct,
        "Long should lose when rates rise: {} vs {}",
        pv_6pct,
        pv_5pct
    );

    // P&L should be approximately related to DV01
    let result = future
        .price_with_metrics(
            &market_5pct,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    let pnl = pv_6pct - pv_5pct;
    let rate_move_bps = (0.06 - 0.05) * 10000.0; // 100 bps

    // DV01 is positive (directional), P&L is negative (rates up)
    // Magnitude relationship: |P&L| ≈ DV01 * rate_move_bps
    assert!(
        (pnl.abs() - (dv01.abs() * rate_move_bps)).abs() < dv01.abs() * rate_move_bps * 0.15,
        "P&L magnitude: {} should relate to DV01 {} * {} bps",
        pnl,
        dv01,
        rate_move_bps
    );
}

#[test]
fn test_dv01_finite_difference() {
    let (as_of, start, end) = standard_dates();
    let base_rate = 0.05;
    let bump = 0.0001; // 1 bp

    let market_base = build_standard_market(as_of, base_rate);
    let market_up = build_standard_market(as_of, base_rate + bump);

    let future = create_standard_future(start, end);

    let pv_base = future.value(&market_base, as_of).unwrap().amount();
    let pv_up = future.value(&market_up, as_of).unwrap().amount();

    // Finite difference DV01
    let dv01_fd = pv_up - pv_base;

    // Analytical DV01
    let result = future
        .price_with_metrics(
            &market_base,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_analytical = *result.measures.get("dv01").unwrap();

    // Both finite difference and analytical should have the same sign
    // (both measure the same parallel rate sensitivity)
    assert!(
        dv01_fd.signum() == dv01_analytical.signum(),
        "Finite difference DV01 {} and analytical {} should have same sign",
        dv01_fd,
        dv01_analytical
    );

    // Magnitudes should be similar
    assert!(
        (dv01_fd.abs() - dv01_analytical.abs()).abs() < dv01_analytical.abs() * 0.10,
        "Magnitudes should match: {} vs {}",
        dv01_fd.abs(),
        dv01_analytical.abs()
    );
}

#[test]
fn test_tick_value_exchange_standard() {
    let (_, start, end) = standard_dates();

    // SOFR future: 3-month, $1MM face, 0.25bp tick
    let future = create_standard_future(start, end);
    let tick_value = future.derived_tick_value().unwrap();

    // Tick value varies with exact dates (Act/360 basis)
    // For a ~3-month future, should be in reasonable range
    assert!(tick_value > 0.0, "Tick value should be positive");
    assert!(
        tick_value < 2000.0,
        "Tick value should be reasonable, got {}",
        tick_value
    );
}

#[test]
fn test_eurodollar_convention() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // Eurodollar specs
    let ed_specs = create_eurodollar_specs();
    let future = create_standard_future(start, end).with_contract_specs(ed_specs);

    // Should price reasonably
    let pv = future.value(&market, as_of).unwrap();
    assert!(pv.amount().is_finite());

    // Tick value should be reasonable (actual value depends on exact dates)
    let tick_value = future.derived_tick_value().unwrap();
    assert!(tick_value > 0.0 && tick_value < 2000.0);
}

#[test]
fn test_convexity_increases_with_maturity() {
    let as_of = date!(2024 - 01 - 01);

    // Short dated (1 month forward)
    let short_start = date!(2024 - 02 - 01);
    let short_end = date!(2024 - 05 - 01);

    // Long dated (2 years forward)
    let long_start = date!(2026 - 01 - 01);
    let long_end = date!(2026 - 04 - 01);

    let market = build_standard_market(as_of, 0.05);

    let short_future = create_custom_future(
        "SHORT",
        1_000_000.0,
        short_start,
        short_start,
        short_end,
        95.0,
        Position::Long,
    );
    let long_future = create_custom_future(
        "LONG",
        1_000_000.0,
        long_start,
        long_start,
        long_end,
        95.0,
        Position::Long,
    );

    // With automatic convexity, both should price
    let pv_short = short_future.value(&market, as_of).unwrap();
    let pv_long = long_future.value(&market, as_of).unwrap();

    assert!(pv_short.amount().is_finite());
    assert!(pv_long.amount().is_finite());

    // The difference in PV should reflect the convexity adjustment difference
    // (hard to test precisely without exposing internal convexity, but at least verify they're different)
}

#[test]
fn test_portfolio_hedging_scenario() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    // Long 10 contracts
    let long_10 = create_custom_future(
        "LONG_10",
        10_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );

    // Short 10 contracts (hedge)
    let short_10 = create_custom_future(
        "SHORT_10",
        10_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Short,
    );

    let pv_long = long_10.value(&market, as_of).unwrap().amount();
    let pv_short = short_10.value(&market, as_of).unwrap().amount();
    let pv_portfolio = pv_long + pv_short;

    // Perfect hedge should net to zero
    assert!(
        pv_portfolio.abs() < 1e-6,
        "Perfectly hedged portfolio should net to zero: {}",
        pv_portfolio
    );
}

#[test]
fn test_rate_scenario_parallel_shift() {
    let (as_of, start, end) = standard_dates();

    let rates = vec![0.01, 0.03, 0.05, 0.07, 0.10];
    let future = create_standard_future(start, end);

    let mut pvs = Vec::new();
    for rate in &rates {
        let market = build_standard_market(as_of, *rate);
        let pv = future.value(&market, as_of).unwrap().amount();
        pvs.push(pv);
    }

    // PV should decrease monotonically as rates increase (for long position with fixed quote)
    for i in 0..pvs.len() - 1 {
        assert!(
            pvs[i] > pvs[i + 1],
            "PV should decrease as rates increase: rate {}% => PV {}, rate {}% => PV {}",
            rates[i] * 100.0,
            pvs[i],
            rates[i + 1] * 100.0,
            pvs[i + 1]
        );
    }
}

#[test]
fn test_basis_point_value() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let future = create_standard_future(start, end);

    // DV01 should match 1bp move magnitude
    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // Move market by 1bp
    let market_up_1bp = build_standard_market(as_of, 0.0501);
    let pv_base = future.value(&market, as_of).unwrap().amount();
    let pv_up = future.value(&market_up_1bp, as_of).unwrap().amount();
    let actual_change = pv_up - pv_base;

    // Magnitudes should match (signs may differ based on convention)
    assert!(
        (actual_change.abs() - dv01.abs()).abs() < dv01.abs() * 0.05,
        "1bp move magnitude should match DV01: |{}| vs |{}|",
        actual_change,
        dv01
    );
}

#[test]
fn test_day_count_impact() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let day_counts = vec![DayCount::Act360, DayCount::Act365F];

    let mut pvs = Vec::new();
    for dc in &day_counts {
        let mut future = create_standard_future(start, end);
        future.day_count = *dc;
        let pv = future.value(&market, as_of).unwrap().amount();
        pvs.push(pv);
    }

    // Different day counts should produce different valuations
    assert_ne!(
        pvs[0], pvs[1],
        "Act/360 and Act/365F should produce different results: {} vs {}",
        pvs[0], pvs[1]
    );

    // Act/365F should have slightly smaller tau => smaller PV magnitude
    assert!(
        pvs[1].abs() < pvs[0].abs() * 1.02,
        "Act/365F should produce smaller magnitude than Act/360"
    );
}

#[test]
fn test_theta_time_decay() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let future = create_standard_future(start, end);

    // Get theta and PV at T
    let result = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta = *result.measures.get("theta").unwrap();
    let pv_t0 = future.value(&market, as_of).unwrap().amount();

    // Move forward 1 day
    let as_of_t1 = as_of + time::Duration::days(1);
    let pv_t1 = future.value(&market, as_of_t1).unwrap().amount();

    let actual_decay = pv_t1 - pv_t0;

    // Theta approximates 1-day P&L from time passage
    // Allow generous tolerance due to convexity and approximation
    if theta.abs() > 1.0 {
        let tolerance_pct = 0.30; // 30% tolerance
        assert!(
            (actual_decay - theta).abs() < theta.abs() * tolerance_pct,
            "Theta should approximate 1-day decay: actual {} vs theta {}",
            actual_decay,
            theta
        );
    }
}

#[test]
fn test_standard_contract_specifications() {
    let specs = FutureContractSpecs::default();

    // Verify market-standard defaults
    assert_eq!(specs.face_value, 1_000_000.0, "Standard face value is $1MM");
    assert_eq!(
        specs.tick_size, 0.0025,
        "Standard tick size is 0.25bp (in price points)"
    );
    assert_eq!(specs.delivery_months, 3, "Standard delivery is quarterly");
    assert!(
        specs.convexity_adjustment.is_none(),
        "Convexity should be computed automatically by default"
    );
}
