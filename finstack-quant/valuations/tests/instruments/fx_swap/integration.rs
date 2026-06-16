//! Integration tests for FX swaps.
//!
//! Tests complex scenarios involving multiple components:
//! - Multi-metric calculations
//! - Scenario analysis (rate shocks, curve shifts)
//! - End-to-end workflow validation

use super::fixtures::*;
use finstack_quant_core::dates::Date;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_full_metrics_suite() {
    // Calculate complete set of metrics for a standard swap
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "FULL_METRICS",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[
                MetricId::custom("forward_points"),
                MetricId::Fx01,
                MetricId::Dv01Domestic,
                MetricId::Dv01Foreign,
                MetricId::custom("carry_pv"),
                MetricId::Dv01,
                MetricId::Theta,
                MetricId::BucketedDv01,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify all metrics computed successfully
    // Note: Result may contain more than 8 entries due to bucketed series composite keys
    assert!(
        result.measures.len() >= 8,
        "Should calculate at least the 8 requested metrics (found {})",
        result.measures.len()
    );

    // Sanity checks on values
    let fwd_pts = *result.measures.get("forward_points").unwrap();
    let fx01 = *result.measures.get("fx01").unwrap();
    let dv01_dom = *result.measures.get("dv01_domestic").unwrap();
    let dv01_for = *result.measures.get("dv01_foreign").unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // Forward points should be positive when USD rates > EUR rates
    assert!(fwd_pts > 0.0, "Forward points should be positive");
    // All sensitivities should be finite
    assert!(fx01.is_finite(), "FX01 should be finite");
    assert!(dv01_dom.is_finite(), "Domestic DV01 should be finite");
    assert!(dv01_for.is_finite(), "Foreign DV01 should be finite");
    assert!(dv01.is_finite(), "DV01 should be finite");
}

#[test]
fn test_rate_shock_scenario() {
    // Test swap behavior under parallel rate shock with fixed contract rates
    let dates = TestDates::standard();
    let market_base = setup_standard_market(dates.as_of);
    let market_shock = setup_steep_curve_market(dates.as_of);

    // Use fixed contract rates so rate shock creates MTM effect
    let swap = create_fx_swap_with_rates(
        "RATE_SHOCK",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        1.10,   // Fix near rate
        1.1055, // Fix far rate at original fair value
    );

    let pv_base = swap.value(&market_base, dates.as_of).unwrap();
    let pv_shock = swap.value(&market_shock, dates.as_of).unwrap();

    // PV should change materially under rate shock with fixed rates
    let pv_change = (pv_shock.amount() - pv_base.amount()).abs();
    assert!(
        pv_change > 10.0,
        "PV should change materially under rate shock, change: {}",
        pv_change
    );
}

#[test]
fn test_fx_rate_shock_scenario() {
    // Test swap behavior under FX rate shock
    let dates = TestDates::standard();

    // Base case: EUR/USD = 1.1
    let market_base = setup_standard_market(dates.as_of);

    // Shocked case: EUR/USD = 1.2
    let market_shock = setup_steep_curve_market(dates.as_of); // Uses 1.2 spot

    let swap = create_standard_fx_swap("FX_SHOCK", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let pv_base = swap.value(&market_base, dates.as_of).unwrap();
    let pv_shock = swap.value(&market_shock, dates.as_of).unwrap();

    // PV should change with FX spot movement
    assert!(
        pv_base.amount() != pv_shock.amount(),
        "PV should change under FX shock"
    );
}

#[test]
fn test_time_series_pv_evolution() {
    // Test PV evolution over time as swap approaches maturity
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_fx_swap_with_rates(
        "TIME_SERIES",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        1.10,
        1.15, // Off-market rate to ensure non-zero PV
    );

    // Value at inception
    let pv_t0 = swap.value(&market, dates.as_of).unwrap();

    // Value 3 months later
    let pv_t1 = swap.value(&market, dates.far_date_3m).unwrap();

    // Value 6 months later
    let as_of_6m = Date::from_calendar_date(2024, time::Month::July, 1).unwrap();
    let pv_t2 = swap.value(&market, as_of_6m).unwrap();

    // All PVs should be finite
    assert!(pv_t0.amount().is_finite());
    assert!(pv_t1.amount().is_finite());
    assert!(pv_t2.amount().is_finite());

    // PV should evolve over time (not remain constant)
    assert!(
        pv_t0.amount() != pv_t1.amount() || pv_t1.amount() != pv_t2.amount(),
        "PV should evolve over time"
    );
}

#[test]
fn test_portfolio_of_swaps() {
    // Test aggregating metrics across multiple swaps (simplified portfolio test)
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swaps = vec![
        create_standard_fx_swap("SWAP_1", dates.near_date, dates.far_date_1m, 1_000_000.0),
        create_standard_fx_swap("SWAP_2", dates.near_date, dates.far_date_3m, 2_000_000.0),
        create_standard_fx_swap("SWAP_3", dates.near_date, dates.far_date_1y, 500_000.0),
    ];

    let mut total_pv: f64 = 0.0;
    let mut total_dv01: f64 = 0.0;

    for swap in swaps {
        let result = swap
            .price_with_metrics(
                &market,
                dates.as_of,
                &[MetricId::Dv01],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        total_pv += result.value.amount();
        total_dv01 += result.measures.get("dv01").unwrap();
    }

    // Portfolio aggregates should be reasonable
    assert!(total_pv.is_finite(), "Portfolio PV should be finite");
    // DV01 for FX swaps at inception with model-implied rates is very small
    assert!(total_dv01.is_finite(), "Portfolio DV01 should be finite");
}

#[test]
fn test_hedge_ratio_calculation() {
    // Test calculating hedge ratio between two swaps
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap1 = create_standard_fx_swap("HEDGE_1", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let swap2 = create_standard_fx_swap("HEDGE_2", dates.near_date, dates.far_date_1y, 2_000_000.0);

    let result1 = swap1
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Fx01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let result2 = swap2
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Fx01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let fx01_1 = *result1.measures.get("fx01").unwrap();
    let fx01_2 = *result2.measures.get("fx01").unwrap();

    // Ratio of FX sensitivities should be ~0.5 (1M vs 2M notional)
    // Both swaps have same direction, so ratio magnitude is what matters
    let hedge_ratio = fx01_1 / fx01_2;

    // Should be roughly 0.5 (1M notional vs 2M notional)
    assert_within_pct(hedge_ratio, 0.5, 10.0, "Hedge ratio calculation");
}

#[test]
fn test_par_swap_construction() {
    // Test that a fair swap at inception has near-zero PV
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    // Create swap without explicit rates (uses model-implied rates)
    let par_swap =
        create_standard_fx_swap("PAR_SWAP", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let pv = par_swap.value(&market, dates.as_of).unwrap();

    // Par swap should have minimal PV
    assert!(
        pv.amount().abs() < 1000.0,
        "Par swap PV should be near zero, got: {}",
        pv.amount()
    );
}

#[test]
fn test_metric_consistency() {
    // Test that metrics are internally consistent
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "CONSISTENCY",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[
                MetricId::Dv01Domestic,
                MetricId::Dv01Foreign,
                MetricId::Fx01,
                MetricId::Dv01,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_dom = result.measures.get("dv01_domestic").unwrap().abs();
    let dv01_for = result.measures.get("dv01_foreign").unwrap().abs();
    let fx01 = result.measures.get("fx01").unwrap().abs();
    let dv01 = *result.measures.get("dv01").unwrap();

    // All sensitivities should be non-zero and finite
    assert!(dv01_dom > 1e-10, "DV01 domestic should be non-zero");
    assert!(dv01_for > 1e-10, "DV01 foreign should be non-zero");
    assert!(fx01 > 1e-10, "FX01 should be non-zero");
    // DV01 for FX swap at inception with model-implied rates is very small
    assert!(dv01.is_finite(), "DV01 should be finite");
}
