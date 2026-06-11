//! Option pricing formulas for interest rate derivatives.
//!
//! This module provides closed-form pricing formulas for European options under:
//! - **Bachelier (Normal) model**: Used for EUR swaptions (post-2015), negative rates
//! - **Black-76 (Lognormal) model**: Standard for USD/GBP swaptions, caps/floors
//! - **Shifted Black model**: For low/negative rate environments
//!
//! All prices assume a unit annuity (PV01 = 1). To get the actual option price,
//! multiply by the annuity factor: `price = annuity × formula_price`.

mod approximations;
mod bachelier;
mod black;

pub use approximations::{
    brenner_subrahmanyam_approx, implied_vol_initial_guess, manaster_koehler_approx,
};
pub use bachelier::{
    bachelier_call, bachelier_delta_call, bachelier_delta_put, bachelier_gamma, bachelier_put,
    bachelier_vega,
};
pub use black::{
    black_call, black_delta_call, black_delta_put, black_gamma, black_put, black_scholes_spot_call,
    black_scholes_spot_put, black_shifted_call, black_shifted_put, black_shifted_vega, black_vega,
    d1_black76, geometric_asian_call,
};

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_bachelier_put_call_parity() {
        // Put-Call parity: Call - Put = F - K
        let forward = 0.03;
        let strike = 0.025;
        let sigma = 0.005;
        let t = 2.0;

        let call = bachelier_call(forward, strike, sigma, t);
        let put = bachelier_put(forward, strike, sigma, t);

        let parity_diff = call - put - (forward - strike);
        assert!(
            parity_diff.abs() < EPSILON,
            "Bachelier put-call parity violated: diff = {}",
            parity_diff
        );
    }

    #[test]
    fn test_black_put_call_parity() {
        // Put-Call parity: Call - Put = F - K
        let forward = 0.05;
        let strike = 0.04;
        let sigma = 0.20;
        let t = 1.5;

        let call = black_call(forward, strike, sigma, t);
        let put = black_put(forward, strike, sigma, t);

        let parity_diff = call - put - (forward - strike);
        assert!(
            parity_diff.abs() < EPSILON,
            "Black put-call parity violated: diff = {}",
            parity_diff
        );
    }

    #[test]
    fn test_bachelier_atm_symmetry() {
        // ATM: Call = Put when F = K
        let forward = 0.03;
        let strike = 0.03; // ATM
        let sigma = 0.005;
        let t = 1.0;

        let call = bachelier_call(forward, strike, sigma, t);
        let put = bachelier_put(forward, strike, sigma, t);

        assert!(
            (call - put).abs() < EPSILON,
            "Bachelier ATM call != put: {} vs {}",
            call,
            put
        );
    }

    #[test]
    fn test_black_atm_symmetry() {
        // ATM: Call = Put when F = K
        let forward = 0.05;
        let strike = 0.05; // ATM
        let sigma = 0.20;
        let t = 1.0;

        let call = black_call(forward, strike, sigma, t);
        let put = black_put(forward, strike, sigma, t);

        assert!(
            (call - put).abs() < EPSILON,
            "Black ATM call != put: {} vs {}",
            call,
            put
        );
    }

    #[test]
    fn test_bachelier_vega_positive() {
        let forward = 0.03;
        let strike = 0.025;
        let sigma = 0.005;
        let t = 1.0;

        let vega = bachelier_vega(forward, strike, sigma, t);
        assert!(vega > 0.0, "Bachelier vega should be positive");
    }

    #[test]
    fn test_black_vega_positive() {
        let forward = 0.05;
        let strike = 0.045;
        let sigma = 0.20;
        let t = 1.0;

        let vega = black_vega(forward, strike, sigma, t);
        assert!(vega > 0.0, "Black vega should be positive");
    }

    #[test]
    fn test_bachelier_delta_bounds() {
        let forward = 0.03;
        let sigma = 0.005;
        let t = 1.0;

        // ITM call: delta should be close to 1
        let delta_itm = bachelier_delta_call(forward, 0.01, sigma, t);
        assert!(delta_itm > 0.9, "ITM call delta should be close to 1");

        // OTM call: delta should be close to 0
        let delta_otm = bachelier_delta_call(forward, 0.05, sigma, t);
        assert!(delta_otm < 0.1, "OTM call delta should be close to 0");

        // ATM call: delta should be close to 0.5
        let delta_atm = bachelier_delta_call(forward, forward, sigma, t);
        assert!(
            (delta_atm - 0.5).abs() < 0.01,
            "ATM call delta should be ~0.5"
        );
    }

    #[test]
    fn test_black_delta_bounds() {
        let forward = 0.05;
        let sigma = 0.20;
        let t = 1.0;

        // All deltas should be in [0, 1] for calls
        let delta_itm = black_delta_call(forward, 0.02, sigma, t);
        let delta_atm = black_delta_call(forward, forward, sigma, t);
        let delta_otm = black_delta_call(forward, 0.08, sigma, t);

        assert!((0.0..=1.0).contains(&delta_itm));
        assert!((0.0..=1.0).contains(&delta_atm));
        assert!((0.0..=1.0).contains(&delta_otm));

        // Ordering: ITM > ATM > OTM
        assert!(delta_itm > delta_atm);
        assert!(delta_atm > delta_otm);

        // Note: Black-76 ATM delta is NOT 0.5 (unlike Bachelier).
        // At ATM, delta = N(0.5σ√T) > 0.5 due to the drift term in d1.
        // For σ=20%, T=1: delta ≈ N(0.1) ≈ 0.54
        assert!(
            delta_atm > 0.5 && delta_atm < 0.6,
            "Black ATM delta should be ~0.54, got {}",
            delta_atm
        );
    }

    #[test]
    fn test_d1_black76_degenerate_inputs_match_delta_digital_limit() {
        use crate::math::special_functions::norm_cdf;

        // Degenerate inputs (T = 0 here): d1 returns ±∞ by moneyness so that
        // N(d1) reproduces black_delta_call's intrinsic digital limit.
        let sigma = 0.2;

        // ITM / ATM: forward >= strike → +∞ → N(d1) = 1.0
        let d1_itm = d1_black76(0.05, 0.04, sigma, 0.0);
        assert!(d1_itm.is_infinite() && d1_itm > 0.0);
        assert_eq!(norm_cdf(d1_itm), black_delta_call(0.05, 0.04, sigma, 0.0));

        let d1_atm = d1_black76(0.05, 0.05, sigma, 0.0);
        assert!(d1_atm.is_infinite() && d1_atm > 0.0);
        assert_eq!(norm_cdf(d1_atm), black_delta_call(0.05, 0.05, sigma, 0.0));

        // OTM: forward < strike → −∞ → N(d1) = 0.0
        let d1_otm = d1_black76(0.04, 0.05, sigma, 0.0);
        assert!(d1_otm.is_infinite() && d1_otm < 0.0);
        assert_eq!(norm_cdf(d1_otm), black_delta_call(0.04, 0.05, sigma, 0.0));

        // Other degenerate domains follow the same convention.
        assert!(d1_black76(0.05, 0.04, 0.0, 1.0).is_infinite());
        assert!(d1_black76(-0.05, 0.04, sigma, 1.0) == f64::NEG_INFINITY);
    }

    #[test]
    fn test_bachelier_gamma_positive() {
        let forward = 0.03;
        let strike = 0.025;
        let sigma = 0.005;
        let t = 1.0;

        let gamma = bachelier_gamma(forward, strike, sigma, t);
        assert!(gamma > 0.0, "Bachelier gamma should be positive");

        // Gamma should be highest at ATM
        let gamma_atm = bachelier_gamma(forward, forward, sigma, t);
        let gamma_otm = bachelier_gamma(forward, forward + 0.02, sigma, t);
        assert!(
            gamma_atm > gamma_otm,
            "ATM gamma should be higher than OTM gamma"
        );
    }

    #[test]
    fn test_black_gamma_positive() {
        let forward = 0.05;
        let strike = 0.05;
        let sigma = 0.20;
        let t = 1.0;

        let gamma = black_gamma(forward, strike, sigma, t);
        assert!(gamma > 0.0, "Black gamma should be positive");
    }

    #[test]
    fn test_shifted_black_consistency() {
        let forward = 0.02;
        let strike = 0.025;
        let sigma = 0.20;
        let t = 1.0;
        let shift = 0.03;

        // Shifted Black with zero shift should equal regular Black
        let regular = black_call(forward, strike, sigma, t);
        let shifted_zero = black_shifted_call(forward, strike, sigma, t, 0.0);
        assert!(
            (regular - shifted_zero).abs() < EPSILON,
            "Shifted Black with zero shift should equal regular Black"
        );

        // Shifted Black should handle negative forward
        let negative_fwd = -0.01;
        let shifted_price = black_shifted_call(negative_fwd, strike, sigma, t, shift);
        assert!(shifted_price >= 0.0, "Option price should be non-negative");
    }

    #[test]
    fn test_expiry_boundary() {
        // At expiry (t=0), option price = intrinsic value
        let forward = 0.05;
        let strike_itm = 0.03;
        let strike_otm = 0.07;
        let sigma = 0.20;

        // Call at expiry
        assert_eq!(
            bachelier_call(forward, strike_itm, sigma, 0.0),
            forward - strike_itm
        );
        assert_eq!(bachelier_call(forward, strike_otm, sigma, 0.0), 0.0);

        assert_eq!(
            black_call(forward, strike_itm, sigma, 0.0),
            forward - strike_itm
        );
        assert_eq!(black_call(forward, strike_otm, sigma, 0.0), 0.0);

        // Put at expiry
        assert_eq!(bachelier_put(forward, strike_itm, sigma, 0.0), 0.0);
        assert_eq!(
            bachelier_put(forward, strike_otm, sigma, 0.0),
            strike_otm - forward
        );
    }

    #[test]
    fn test_zero_vol_boundary() {
        // At zero vol, option price = intrinsic value
        let forward = 0.05;
        let strike_itm = 0.03;
        let strike_otm = 0.07;
        let t = 1.0;

        assert_eq!(
            bachelier_call(forward, strike_itm, 0.0, t),
            forward - strike_itm
        );
        assert_eq!(bachelier_call(forward, strike_otm, 0.0, t), 0.0);

        assert_eq!(
            black_call(forward, strike_itm, 0.0, t),
            forward - strike_itm
        );
        assert_eq!(black_call(forward, strike_otm, 0.0, t), 0.0);
    }

    // =========================================================================
    // Implied Volatility Approximation Tests
    // =========================================================================

    #[test]
    fn test_brenner_subrahmanyam_atm() {
        // Test ATM case where approximation is most accurate
        let forward = 0.05;
        let strike = 0.05; // ATM
        let sigma_actual = 0.20;
        let t = 1.0;

        let price = black_call(forward, strike, sigma_actual, t);
        let sigma_approx = brenner_subrahmanyam_approx(forward, strike, price, t);

        // For ATM options, approximation should be within 15% relative error
        let rel_error = (sigma_approx - sigma_actual).abs() / sigma_actual;
        assert!(
            rel_error < 0.15,
            "ATM approximation error {:.2}% exceeds 15%",
            rel_error * 100.0
        );
    }

    #[test]
    fn test_brenner_subrahmanyam_various_vols() {
        // Test across range of volatilities
        let forward = 0.05;
        let strike = 0.05; // ATM
        let t = 1.0;

        for sigma_actual in [0.10, 0.20, 0.30, 0.40, 0.50] {
            let price = black_call(forward, strike, sigma_actual, t);
            let sigma_approx = brenner_subrahmanyam_approx(forward, strike, price, t);

            // Should be within 20% relative error for all vols
            let rel_error = (sigma_approx - sigma_actual).abs() / sigma_actual;
            assert!(
                rel_error < 0.20,
                "Approximation for σ={:.0}% has error {:.2}%",
                sigma_actual * 100.0,
                rel_error * 100.0
            );
        }
    }

    #[test]
    fn test_manaster_koehler_otm() {
        // Test OTM case where M-K approximation helps
        let forward = 0.05;
        let strike = 0.07; // OTM
        let t = 1.0;

        let approx = manaster_koehler_approx(forward, strike, t);

        // Should return a reasonable positive volatility
        assert!(approx > 0.0, "Approximation should be positive");
        assert!(approx < 2.0, "Approximation should be reasonable (<200%)");
    }

    #[test]
    fn test_implied_vol_initial_guess_consistency() {
        // Test that combined guess is always in valid range
        let forward = 0.05;
        let t = 1.0;

        for &strike in &[0.03, 0.04, 0.05, 0.06, 0.07] {
            let sigma = 0.25;
            let price = black_call(forward, strike, sigma, t);
            let guess = implied_vol_initial_guess(forward, strike, price, t);

            assert!(
                (0.01..=5.0).contains(&guess),
                "Guess {:.4} for K={:.2} outside valid range",
                guess,
                strike
            );
        }
    }

    #[test]
    fn test_implied_vol_edge_cases() {
        // Edge cases should return default volatility
        assert_eq!(brenner_subrahmanyam_approx(0.05, 0.05, 0.001, 0.0), 0.2); // t = 0
        assert_eq!(brenner_subrahmanyam_approx(0.05, 0.05, 0.0, 1.0), 0.2); // price = 0
        assert_eq!(brenner_subrahmanyam_approx(0.0, 0.05, 0.001, 1.0), 0.2); // forward = 0
        assert_eq!(brenner_subrahmanyam_approx(0.05, 0.0, 0.001, 1.0), 0.2); // strike = 0
    }

    #[test]
    fn test_brenner_subrahmanyam_rates_market() {
        // Test with interest rate market typical values (swaption)
        let forward = 0.03; // 3% forward swap rate
        let strike = 0.03; // ATM
        let sigma_actual = 0.50; // 50% lognormal vol (typical for rates)
        let t = 5.0; // 5Y expiry

        let price = black_call(forward, strike, sigma_actual, t);
        let sigma_approx = brenner_subrahmanyam_approx(forward, strike, price, t);

        // Should be reasonably close
        let rel_error = (sigma_approx - sigma_actual).abs() / sigma_actual;
        assert!(
            rel_error < 0.25,
            "Rates market approximation error {:.2}% exceeds 25%",
            rel_error * 100.0
        );
    }
}
