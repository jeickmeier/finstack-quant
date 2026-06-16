//! Tests for option Greeks with market standard validations.
//!
//! Greeks are tested for:
//! - Correct sign and magnitude
//! - Market standard bounds
//! - Relationships between Greeks
//! - Moneyness behavior

use super::helpers::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

// ==================== DELTA TESTS ====================

#[test]
fn test_atm_call_delta_near_fifty() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();

    // ATM call delta should be ~0.5-0.65 per share, * 100 contract size = ~50-65
    // Allow wider range due to discounting, time value, and dividend yield effects
    assert_in_range(delta, 40.0, 70.0, "ATM call delta");
}

#[test]
fn test_call_delta_always_positive() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let spot = 100.0;

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    for strike in strikes {
        let call = create_call(as_of, expiry, strike);
        let result = call
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let delta = *result.measures.get("delta").unwrap();
        assert!(
            delta >= 0.0,
            "Call delta should be non-negative at strike {}, got {}",
            strike,
            delta
        );
        assert!(
            delta <= 100.0,
            "Call delta should be <= contract_size (100) at strike {}, got {}",
            strike,
            delta
        );
    }
}

#[test]
fn test_put_delta_always_negative() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let spot = 100.0;

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    for strike in strikes {
        let put = create_put(as_of, expiry, strike);
        let result = put
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let delta = *result.measures.get("delta").unwrap();
        assert!(
            delta <= 0.0,
            "Put delta should be non-positive at strike {}, got {}",
            strike,
            delta
        );
        assert!(
            delta >= -100.0,
            "Put delta should be >= -contract_size (-100) at strike {}, got {}",
            strike,
            delta
        );
    }
}

#[test]
fn test_itm_call_delta_approaches_one() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 80.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();
    let delta_per_share = delta / 100.0; // Normalize by contract size

    // Deep ITM call delta should be high (approaching 1.0 per share)
    assert_in_range(delta_per_share, 0.75, 1.0, "Deep ITM call delta per share");
}

#[test]
fn test_otm_call_delta_approaches_zero() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 130.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();
    let delta_per_share = delta / 100.0;

    // OTM call delta should be low
    assert_in_range(delta_per_share, 0.0, 0.35, "OTM call delta per share");
}

#[test]
fn test_delta_increases_with_spot() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;

    let call = create_call(as_of, expiry, strike);

    let spots = [90.0, 95.0, 100.0, 105.0, 110.0];
    let mut prev_delta = 0.0;

    for spot in spots {
        let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);
        let result = call
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let delta = *result.measures.get("delta").unwrap();

        if spot > 90.0 {
            assert!(
                delta > prev_delta,
                "Call delta should increase with spot: spot={}, delta={}, prev_delta={}",
                spot,
                delta,
                prev_delta
            );
        }
        prev_delta = delta;
    }
}

// ==================== GAMMA TESTS ====================

#[test]
fn test_gamma_always_positive() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let spot = 100.0;

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    for strike in strikes {
        let call = create_call(as_of, expiry, strike);
        let result = call
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Gamma],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let gamma = *result.measures.get("gamma").unwrap();
        assert_non_negative(gamma, &format!("Gamma at strike {}", strike));
    }
}

#[test]
fn test_atm_gamma_highest() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    // Use strikes further apart to ensure ATM has highest gamma
    // Maximum gamma occurs when d1 ≈ 0, which for r=0.05, q=0.0, σ=0.25, T=1.0
    // occurs at K ≈ S * exp(0.08125) ≈ 108.46. Using 80/100/120 ensures ATM is highest.
    let atm_call = create_call(as_of, expiry, 100.0);
    let itm_call = create_call(as_of, expiry, 80.0);
    let otm_call = create_call(as_of, expiry, 120.0);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let atm_gamma = *atm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    let itm_gamma = *itm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    let otm_gamma = *otm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    assert!(
        atm_gamma > itm_gamma,
        "ATM gamma ({}) should exceed ITM gamma ({})",
        atm_gamma,
        itm_gamma
    );
    assert!(
        atm_gamma > otm_gamma,
        "ATM gamma ({}) should exceed OTM gamma ({})",
        atm_gamma,
        otm_gamma
    );
}

#[test]
fn test_gamma_same_for_call_and_put() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let call_gamma = *call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    let put_gamma = *put
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    assert_approx_eq_tol(call_gamma, put_gamma, LOOSE_TOL, "Gamma parity");
}

// ==================== VEGA TESTS ====================

#[test]
fn test_vega_always_positive() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let spot = 100.0;

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    for strike in strikes {
        let call = create_call(as_of, expiry, strike);
        let result = call
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Vega],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let vega = *result.measures.get("vega").unwrap();
        assert_positive(vega, &format!("Vega at strike {}", strike));
    }
}

#[test]
fn test_atm_vega_highest() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    // Use strikes further apart to ensure ATM has highest vega
    // Maximum vega occurs when d1 ≈ 0, which for r=0.05, q=0.0, σ=0.25, T=1.0
    // occurs at K ≈ S * exp(0.08125) ≈ 108.46. Using 80/100/120 ensures ATM is highest.
    let atm_call = create_call(as_of, expiry, 100.0);
    let itm_call = create_call(as_of, expiry, 80.0);
    let otm_call = create_call(as_of, expiry, 120.0);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let atm_vega = *atm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    let itm_vega = *itm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    let otm_vega = *otm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    assert!(
        atm_vega > itm_vega,
        "ATM vega ({}) should exceed ITM vega ({})",
        atm_vega,
        itm_vega
    );
    assert!(
        atm_vega > otm_vega,
        "ATM vega ({}) should exceed OTM vega ({})",
        atm_vega,
        otm_vega
    );
}

#[test]
fn test_vega_same_for_call_and_put() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let call_vega = *call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    let put_vega = *put
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    assert_approx_eq_tol(call_vega, put_vega, LOOSE_TOL, "Vega parity");
}

#[test]
fn test_vega_increases_with_maturity() {
    let as_of = date!(2024 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let short_call = create_call(as_of, date!(2024 - 04 - 01), strike); // 3M
    let long_call = create_call(as_of, date!(2025 - 01 - 01), strike); // 1Y

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let short_vega = *short_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    let long_vega = *long_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    assert!(
        long_vega > short_vega,
        "Longer maturity vega ({}) should exceed shorter ({})",
        long_vega,
        short_vega
    );
}

// ==================== THETA TESTS ====================

#[test]
fn test_theta_negative_for_long_options() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    // Theta represents daily time decay and should be negative for long options
    assert!(
        theta < 0.0,
        "Long option theta should be negative, got {}",
        theta
    );
}

#[test]
fn test_atm_theta_magnitude_highest() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    let atm_call = create_call(as_of, expiry, 100.0);
    let otm_call = create_call(as_of, expiry, 120.0);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let atm_theta = *atm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("theta")
        .unwrap();

    let otm_theta = *otm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("theta")
        .unwrap();

    // ATM should have higher theta magnitude (more negative)
    assert!(
        atm_theta.abs() > otm_theta.abs(),
        "ATM theta magnitude ({}) should exceed OTM ({})",
        atm_theta.abs(),
        otm_theta.abs()
    );
}

// ==================== RHO TESTS ====================

#[test]
fn test_call_rho_positive() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let rho = *result.measures.get("rho").unwrap();

    assert_positive(rho, "Call rho");
}

#[test]
fn test_put_rho_negative() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let put = create_put(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let result = put
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let rho = *result.measures.get("rho").unwrap();

    assert!(rho < 0.0, "Put rho should be negative, got {}", rho);
}

#[test]
fn test_rho_increases_with_maturity() {
    let as_of = date!(2024 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let short_call = create_call(as_of, date!(2024 - 04 - 01), strike); // 3M
    let long_call = create_call(as_of, date!(2025 - 01 - 01), strike); // 1Y

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let short_rho = *short_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("rho")
        .unwrap();

    let long_rho = *long_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("rho")
        .unwrap();

    assert!(
        long_rho > short_rho,
        "Longer maturity rho ({}) should exceed shorter ({})",
        long_rho,
        short_rho
    );
}

// ==================== COMBINED GREEKS TESTS ====================

#[test]
fn test_all_greeks_computed_together() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
        MetricId::Rho,
    ];

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify all Greeks are present
    assert!(result.measures.contains_key("delta"));
    assert!(result.measures.contains_key("gamma"));
    assert!(result.measures.contains_key("vega"));
    assert!(result.measures.contains_key("theta"));
    assert!(result.measures.contains_key("rho"));

    // Verify signs
    let delta = *result.measures.get("delta").unwrap();
    let gamma = *result.measures.get("gamma").unwrap();
    let vega = *result.measures.get("vega").unwrap();
    let theta = *result.measures.get("theta").unwrap();
    let rho = *result.measures.get("rho").unwrap();

    assert!(delta > 0.0, "Call delta should be positive");
    assert!(gamma > 0.0, "Gamma should be positive");
    assert!(vega > 0.0, "Vega should be positive");
    assert!(theta < 0.0, "Theta should be negative for long option");
    assert!(rho > 0.0, "Call rho should be positive");
}
