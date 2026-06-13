//! End-to-end workflow tests

use crate::swaption::common::*;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;

#[test]
fn test_complete_pricing_workflow() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // 1. Basic pricing
    let pv = swaption.value(&market, as_of).unwrap();
    assert!(pv.amount() > 0.0, "Step 1: Basic pricing");

    // 2. Pricing with single metric
    let result_delta = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    assert!(
        result_delta.measures.contains_key("delta"),
        "Step 2: Delta metric"
    );

    // 3. Pricing with all metrics
    let all_metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Rho,
        MetricId::Theta,
        MetricId::Dv01,
        MetricId::ImpliedVol,
    ];
    let result_all = swaption
        .price_with_metrics(
            &market,
            as_of,
            &all_metrics,
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // 7 requested metrics + the `theta_period_days` horizon stamp the theta
    // producer always emits alongside Theta (quant review M3).
    assert_eq!(result_all.measures.len(), 8, "Step 3: All metrics computed");
    assert!(result_all.measures.contains_key("theta_period_days"));

    // 4. Validate all metrics are finite
    for (name, value) in &result_all.measures {
        assert!(value.is_finite(), "Metric {} should be finite", name);
    }
}

#[test]
fn test_portfolio_of_swaptions() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Create a portfolio of swaptions
    let swaptions = vec![
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.03),
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05),
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.07),
        create_standard_receiver_swaption(expiry, swap_start, swap_end, 0.05),
    ];

    // Price entire portfolio
    let mut total_pv: f64 = 0.0;
    let mut total_delta: f64 = 0.0;
    let mut total_vega: f64 = 0.0;

    for swaption in &swaptions {
        let result = swaption
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta, MetricId::Vega],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        total_pv += result.value.amount();
        total_delta += result.measures.get("delta").copied().unwrap_or(0.0);
        total_vega += result.measures.get("vega").copied().unwrap_or(0.0);
    }

    // Portfolio metrics should be finite
    assert!(total_pv.is_finite(), "Portfolio PV should be finite");
    assert!(total_delta.is_finite(), "Portfolio delta should be finite");
    assert!(total_vega.is_finite(), "Portfolio vega should be finite");
}

#[test]
fn test_swaption_lifecycle() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2025 - 01 - 01);
    let swap_start = expiry;
    let swap_end = time::macros::date!(2030 - 01 - 01);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Price at different points in lifecycle
    let dates = vec![
        as_of,                                                 // T-1Y
        as_of.checked_add(time::Duration::days(180)).unwrap(), // T-6M
        as_of.checked_add(time::Duration::days(330)).unwrap(), // T-1M
    ];

    for date in dates {
        if date < expiry {
            let pv = swaption.value(&market, date).unwrap().amount();

            // Value should generally decrease as expiry approaches (time decay)
            // Though this depends on market movements
            assert!(
                pv > 0.0 && pv.is_finite(),
                "PV at {:?} should be positive",
                date
            );
        }
    }
}

#[test]
fn test_market_scenario_stress() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    // Test different market scenarios
    let scenarios = vec![
        ("Mid rate, mid vol", 0.05, 0.30),
        ("High rate, high vol", 0.10, 0.60),
        ("Low vol", 0.05, 0.15),
    ];

    for (name, rate, vol) in scenarios {
        let market = create_flat_market(as_of, rate, vol);
        let pv = swaption.value(&market, as_of).unwrap().amount();

        assert!(
            pv > 0.0 && pv.is_finite(),
            "Scenario '{}' should produce valid PV",
            name
        );
    }
}
