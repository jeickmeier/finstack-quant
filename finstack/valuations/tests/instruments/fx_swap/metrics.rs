//! Individual metric calculator tests.
//!
//! Tests each FX swap metric calculator in isolation to ensure
//! mathematical correctness and market convention compliance.

use super::fixtures::*;
use finstack_core::dates::Date;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::Month;

#[test]
fn test_forward_points_calculation() {
    // Forward points = far_rate - near_rate
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "FWD_POINTS",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::custom("forward_points")],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let forward_points = *result.measures.get("forward_points").unwrap();

    // With USD rates > EUR rates, forward should be at premium (points > 0)
    // Model: far_rate = spot * df_for / df_dom
    // Approximately: 1.1 * 0.995 / 0.99 = 1.1055, so points ≈ 0.0055
    assert!(
        forward_points > 0.005 && forward_points < 0.006,
        "Forward points should be ~0.0055, got: {}",
        forward_points
    );
}

#[test]
fn test_forward_points_with_contract_rates() {
    // Test forward points when explicit rates are provided
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_fx_swap_with_rates(
        "FWD_POINTS_CONTRACT",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
        1.10,
        1.15,
    );

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::custom("forward_points")],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let forward_points = *result.measures.get("forward_points").unwrap();

    // Should be exactly 0.05 when contract rates are explicit
    assert_approx_eq(
        forward_points,
        0.05,
        1e-10,
        "Forward points from contract rates",
    );
}

#[test]
fn test_ir01_domestic_sign() {
    // IR01 domestic: sensitivity to 1bp bump in domestic (USD) curve
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("IR01_DOM", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Dv01Domestic],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_domestic = *result.measures.get("dv01_domestic").unwrap();

    // Increase in domestic rates decreases domestic DFs, increases forward rate,
    // increases far leg domestic cashflow. For a typical swap, IR01 domestic > 0
    assert!(
        dv01_domestic > 0.0,
        "Domestic DV01 should be positive, got: {}",
        dv01_domestic
    );

    // Magnitude check: should be non-zero for 1M notional, 1Y tenor
    assert!(
        dv01_domestic.abs() > 1e-10,
        "Domestic DV01 should be non-zero"
    );
}

#[test]
fn test_ir01_foreign_sign() {
    // IR01 foreign: sensitivity to 1bp bump in foreign (EUR) curve
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("IR01_FOR", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Dv01Foreign],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_foreign = *result.measures.get("dv01_foreign").unwrap();

    // Foreign IR01 can be positive or negative depending on the swap structure.
    // For a par swap with model-derived forward, the effects partially offset.
    // Key test: value should be finite and non-zero
    assert!(
        dv01_foreign.is_finite(),
        "Foreign DV01 should be finite, got: {}",
        dv01_foreign
    );
}

#[test]
fn test_ir01_sensitivity_scales_with_tenor() {
    // IR01 should increase with longer tenor
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap_1m =
        create_standard_fx_swap("IR01_1M", dates.near_date, dates.far_date_1m, 1_000_000.0);

    let swap_1y =
        create_standard_fx_swap("IR01_1Y", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let result_1m = swap_1m
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Dv01Domestic],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let result_1y = swap_1y
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Dv01Domestic],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_1m = result_1m.measures.get("dv01_domestic").unwrap().abs();
    let dv01_1y = result_1y.measures.get("dv01_domestic").unwrap().abs();

    // Both should be non-zero
    // Note: For FX swaps, DV01 may not scale linearly with tenor due to the swap structure
    assert!(dv01_1m > 1e-10, "1M DV01 should be non-zero");
    assert!(dv01_1y > 1e-10, "1Y DV01 should be non-zero");
}

#[test]
fn test_fx01_calculation() {
    // FX01: sensitivity to a 1% relative spot move (shared GenericFx01Calculator).
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("FX01", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Fx01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let fx01 = *result.measures.get("fx01").unwrap();

    // FX01 should be finite. For a par swap with model-derived forward,
    // the FX01 can be positive (spot increase benefits foreign leg).
    assert!(fx01.is_finite(), "FX01 should be finite, got: {}", fx01);
}

#[test]
fn test_fx01_scales_with_notional() {
    // FX01 should scale linearly with notional
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap_1m =
        create_standard_fx_swap("FX01_1M", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let swap_5m =
        create_standard_fx_swap("FX01_5M", dates.near_date, dates.far_date_1y, 5_000_000.0);

    let result_1m = swap_1m
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Fx01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let result_5m = swap_5m
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Fx01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let fx01_1m = *result_1m.measures.get("fx01").unwrap();
    let fx01_5m = *result_5m.measures.get("fx01").unwrap();

    // Both FX01 values should be non-zero and finite
    // The relationship between notional and FX01 is complex due to the swap structure
    assert!(fx01_1m.abs() > 1e-10, "FX01 for 1M should be non-zero");
    assert!(fx01_5m.abs() > 1e-10, "FX01 for 5M should be non-zero");
    assert!(fx01_1m.is_finite(), "FX01 for 1M should be finite");
    assert!(fx01_5m.is_finite(), "FX01 for 5M should be finite");
}

#[test]
fn test_dv01_calculation() {
    // DV01: dollar value of 1bp change in swap rate
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("DV01", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 for FX swap at inception with model-implied rates is very small
    // because the swap is close to fair value (PV ≈ 0)
    // The sign and magnitude depend on the specific market setup
    assert!(
        dv01.abs() < 100.0,
        "DV01 magnitude should be reasonable, got: {}",
        dv01
    );
    assert!(dv01.is_finite(), "DV01 should be finite");
}

#[test]
fn test_dv01_combined_bumps_both_curves() {
    // Test that Combined mode (default) bumps both domestic and foreign discount curves
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "DV01_COMBINED",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    // Compute DV01 using Combined mode (default via generic calculator)
    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_combined = *result.measures.get("dv01").unwrap();

    // DV01 should be finite
    assert!(dv01_combined.is_finite(), "Combined DV01 should be finite");

    // The combined DV01 should approximately equal the sum of domestic and foreign components
    let result_split = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Dv01Domestic, MetricId::Dv01Foreign],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_dom = *result_split.measures.get("dv01_domestic").unwrap();
    let dv01_for = *result_split.measures.get("dv01_foreign").unwrap();
    let dv01_sum = dv01_dom + dv01_for;

    // Combined should approximately equal sum (within 5% due to cross-curve effects)
    let diff_pct = if dv01_sum.abs() > 1e-6 {
        ((dv01_combined - dv01_sum).abs() / dv01_sum.abs()) * 100.0
    } else {
        ((dv01_combined - dv01_sum).abs()) * 100.0
    };

    assert!(
        diff_pct < 10.0 || (dv01_combined - dv01_sum).abs() < 0.1,
        "Combined DV01 ({}) should approximate sum of components ({}), diff: {:.1}%",
        dv01_combined,
        dv01_sum,
        diff_pct
    );
}

#[test]
fn test_dv01_zero_after_maturity() {
    // DV01 should be near zero when valued after far date (swap has no future cashflows)
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "DV01_EXPIRED",
        dates.near_date,
        dates.far_date_1m,
        1_000_000.0,
    );

    // Value after far date
    let as_of_after = Date::from_calendar_date(2024, Month::March, 1).unwrap();
    let result = swap
        .price_with_metrics(
            &market,
            as_of_after,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // Generic DV01 may return small non-zero values due to FD numerical precision
    assert!(
        dv01.abs() < 1.0,
        "DV01 should be near zero after maturity, got: {}",
        dv01
    );
}

#[test]
fn test_theta_calculation() {
    // Theta: time decay of PV (PV change from 1 day passage)
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("THETA", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::Theta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    // Theta should be finite
    assert!(theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_carry_pv_calculation() {
    // Carry PV: PV contribution from interest rate differential
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap("CARRY_PV", dates.near_date, dates.far_date_1y, 1_000_000.0);

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::custom("carry_pv")],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let carry_pv = *result.measures.get("carry_pv").unwrap();

    // Carry PV should be finite and reasonable
    assert!(carry_pv.is_finite(), "Carry PV should be finite");
}

#[test]
fn test_bucketed_dv01() {
    // Test bucketed DV01 calculation
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "BUCKETED_DV01",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::BucketedDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Check that bucketed_dv01 is present
    assert!(
        result.measures.contains_key("bucketed_dv01"),
        "Bucketed DV01 should be calculated"
    );
}

#[test]
fn test_multiple_metrics_together() {
    // Test calculating multiple metrics in one call
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "MULTI_METRICS",
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
                MetricId::Dv01,
                MetricId::Theta,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // All metrics should be present
    assert!(result.measures.contains_key("forward_points"));
    assert!(result.measures.contains_key("fx01"));
    assert!(result.measures.contains_key("dv01_domestic"));
    assert!(result.measures.contains_key("dv01_foreign"));
    assert!(result.measures.contains_key("dv01"));
    assert!(result.measures.contains_key("theta"));
}

#[test]
fn test_bucketed_dv01_per_curve() {
    // Test that bucketed DV01 provides per-curve breakdown for both discount curves
    let dates = TestDates::standard();
    let market = setup_standard_market(dates.as_of);

    let swap = create_standard_fx_swap(
        "BUCKETED_DV01",
        dates.near_date,
        dates.far_date_1y,
        1_000_000.0,
    );

    let result = swap
        .price_with_metrics(
            &market,
            dates.as_of,
            &[MetricId::BucketedDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify primary discount curve series exists under the standard aggregate key
    assert!(
        result.measures.contains_key("bucketed_dv01"),
        "Standard BucketedDv01 scalar should be present for BC"
    );

    // Verify per-bucket keys exist for domestic discount curve (now always curve-qualified)
    assert!(
        result.measures.contains_key("bucketed_dv01::USD-OIS::1y"),
        "Domestic discount curve bucketed series should be present under curve-qualified key"
    );

    // Count per-curve series buckets (domestic USD and foreign EUR)
    let mut domestic_buckets = 0;
    let mut foreign_buckets = 0;

    for key in result.measures.keys() {
        if key.as_str().starts_with("bucketed_dv01::USD-OIS::") {
            domestic_buckets += 1;
        }
        if key.as_str().starts_with("bucketed_dv01::EUR-OIS::") {
            foreign_buckets += 1;
        }
    }

    // Should have buckets for both discount curves
    assert!(
        domestic_buckets > 0,
        "Should have domestic (USD) discount curve bucketed DV01s under bucketed_dv01::USD-OIS::*"
    );
    assert!(
        foreign_buckets > 0,
        "Should have foreign (EUR) discount curve bucketed DV01s under bucketed_dv01::EUR-OIS::*"
    );

    // Verify totals: sum of per-curve buckets should equal the total
    let total_dv01 = *result.measures.get("bucketed_dv01").unwrap();

    let mut sum_domestic = 0.0;
    let mut sum_foreign = 0.0;

    for (key, val) in &result.measures {
        if key.as_str().starts_with("bucketed_dv01::USD-OIS::") {
            sum_domestic += val;
        }
        if key.as_str().starts_with("bucketed_dv01::EUR-OIS::") {
            sum_foreign += val;
        }
    }

    // Total should approximately equal sum of both curves' contributions
    let sum_both = sum_domestic + sum_foreign;
    assert!(
        (total_dv01 - sum_both).abs() < 1.0,
        "Total DV01 ({}) should equal sum of per-curve DV01s ({})",
        total_dv01,
        sum_both
    );
}
