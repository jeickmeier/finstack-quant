//! SABR calibration and smile tests

use crate::swaption::common::*;
use finstack_valuations::instruments::rates::swaption::SABRParameters;
use finstack_valuations::instruments::Instrument;

#[test]
fn test_sabr_parameters_validation() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Valid SABR parameters
    let valid_params = SABRParameters {
        alpha: 0.20,
        beta: 0.5,
        rho: -0.3,
        nu: 0.4,
        shift: None,
    };

    let swaption =
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05).with_sabr(valid_params);

    let pv = swaption.value(&market, as_of).unwrap().amount();
    assert!(
        pv > 0.0 && pv.is_finite(),
        "Valid SABR parameters should work"
    );
}

#[test]
fn test_sabr_beta_range() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Test different beta values (0 = normal, 1 = lognormal). β≈0 SABR
    // produces a normal (Bachelier) vol, so the instrument's vol_model must
    // be set to Normal — feeding a normal vol into the Black model is now a
    // hard error rather than a silent unit mismatch.
    for beta in [0.0, 0.5, 1.0] {
        let params = SABRParameters {
            alpha: if beta == 0.0 { 0.01 } else { 0.20 },
            beta,
            rho: 0.0,
            nu: 0.3,
            shift: None,
        };

        let mut swaption =
            create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05).with_sabr(params);
        if beta == 0.0 {
            swaption.vol_model =
                finstack_valuations::instruments::rates::swaption::VolatilityModel::Normal;
        }

        let pv = swaption.value(&market, as_of).unwrap().amount();
        assert!(
            pv > 0.0 && pv.is_finite(),
            "SABR with beta={} should work",
            beta
        );
    }
}

#[test]
fn test_sabr_rho_effect() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.25);

    // Negative rho (typical for rates)
    let params_neg = SABRParameters {
        alpha: 0.25,
        beta: 0.5,
        rho: -0.5,
        nu: 0.4,
        shift: None,
    };

    // Positive rho
    let params_pos = SABRParameters {
        alpha: 0.25,
        beta: 0.5,
        rho: 0.5,
        nu: 0.4,
        shift: None,
    };

    let swaption_neg =
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.07).with_sabr(params_neg);
    let swaption_pos =
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.07).with_sabr(params_pos);

    let pv_neg = swaption_neg.value(&market, as_of).unwrap().amount();
    let pv_pos = swaption_pos.value(&market, as_of).unwrap().amount();

    // Different rho values should produce different prices (smile effect)
    assert!(
        pv_neg.is_finite() && pv_pos.is_finite(),
        "Both should be finite"
    );
    assert!((pv_neg - pv_pos).abs() > 0.0, "Rho should affect pricing");
}

#[test]
fn test_sabr_nu_volatility_of_volatility() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.25);

    // Low vol-of-vol
    let params_low = SABRParameters {
        alpha: 0.25,
        beta: 0.5,
        rho: -0.3,
        nu: 0.1,
        shift: None,
    };

    // High vol-of-vol
    let params_high = SABRParameters {
        alpha: 0.25,
        beta: 0.5,
        rho: -0.3,
        nu: 0.8,
        shift: None,
    };

    let swaption_low =
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05).with_sabr(params_low);
    let swaption_high =
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05).with_sabr(params_high);

    let pv_low = swaption_low.value(&market, as_of).unwrap().amount();
    let pv_high = swaption_high.value(&market, as_of).unwrap().amount();

    // Higher nu should increase option value (more volatility uncertainty)
    assert!(pv_high > pv_low, "Higher nu should increase option value");
}
