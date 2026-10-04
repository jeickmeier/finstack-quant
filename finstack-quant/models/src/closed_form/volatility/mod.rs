//! Option pricing formulas for interest rate derivatives.
//!
//! This module provides closed-form pricing formulas for European options under:
//! - **Bachelier (Normal) model**: Used for EUR swaptions (post-2015), negative rates
//! - **Black-76 (Lognormal) model**: Standard for USD/GBP swaptions, caps/floors
//! - **Shifted Black model**: For low/negative rate environments
//!
//! All prices assume a unit annuity (PV01 = 1). To get the actual option price,
//! multiply by the annuity factor: `price = annuity × formula_price`.

mod bachelier;
mod black;

pub use bachelier::{
    bachelier_call, bachelier_delta_call, bachelier_delta_put, bachelier_gamma, bachelier_put,
    bachelier_vega,
};
pub use black::{
    black_call, black_delta_call, black_delta_put, black_gamma, black_put, black_shifted_call,
    black_vega,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closed_form::vanilla::bs_price;
    use crate::types::OptionType;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn spot_price_at_zero_volatility_is_the_discounted_forward_payoff() {
        let spot_call = |spot, strike, rate, div_yield, vol, expiry| {
            bs_price(spot, strike, rate, div_yield, vol, expiry, OptionType::Call)
                .expect("valid Black-Scholes inputs")
        };
        let spot_put = |spot, strike, rate, div_yield, vol, expiry| {
            bs_price(spot, strike, rate, div_yield, vol, expiry, OptionType::Put)
                .expect("valid Black-Scholes inputs")
        };
        let call = spot_call(100.0, 102.0, 0.05, 0.0, 0.0, 1.0);
        let put = spot_put(100.0, 102.0, 0.05, 0.0, 0.0, 1.0);
        let discounted_intrinsic = 100.0 - 102.0 * (-0.05_f64).exp();
        assert!((call - 2.974_598_700_927_174_4).abs() < 1e-12);
        assert_eq!(put, 0.0);
        assert!((call - put - discounted_intrinsic).abs() < 1e-12);
        let negative_carry_call = spot_call(100.0, 100.0, 0.0, 0.05, 0.0, 1.0);
        let negative_carry_put = spot_put(100.0, 100.0, 0.0, 0.05, 0.0, 1.0);
        assert_eq!(negative_carry_call, 0.0);
        assert!((negative_carry_put - 100.0 * (1.0 - (-0.05_f64).exp())).abs() < 1e-12);
        assert_eq!(spot_call(110.0, 100.0, 0.05, 0.02, 0.0, 0.0), 10.0);
    }

    #[test]
    fn raw_forward_formulas_do_not_mask_invalid_inputs_at_boundaries() {
        type ForwardFormula = fn(f64, f64, f64, f64) -> f64;
        let formulas: [ForwardFormula; 12] = [
            black_call,
            black_put,
            black_delta_call,
            black_delta_put,
            black_gamma,
            black_vega,
            bachelier_call,
            bachelier_put,
            bachelier_delta_call,
            bachelier_delta_put,
            bachelier_gamma,
            bachelier_vega,
        ];
        for formula in formulas {
            for base in [[100.0, 100.0, 0.2, 0.0], [100.0, 100.0, 0.0, 1.0]] {
                for index in 0..4 {
                    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                        let mut args = base;
                        args[index] = invalid;
                        assert!(formula(args[0], args[1], args[2], args[3]).is_nan());
                    }
                }
            }
            assert!(formula(100.0, 100.0, -0.2, 1.0).is_nan());
            assert!(formula(100.0, 100.0, 0.2, -1.0).is_nan());
        }
    }

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

        let delta_itm = bachelier_delta_call(forward, 0.01, sigma, t);
        assert!(delta_itm > 0.9, "ITM call delta should be close to 1");

        let delta_otm = bachelier_delta_call(forward, 0.05, sigma, t);
        assert!(delta_otm < 0.1, "OTM call delta should be close to 0");

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

        let delta_itm = black_delta_call(forward, 0.02, sigma, t);
        let delta_atm = black_delta_call(forward, forward, sigma, t);
        let delta_otm = black_delta_call(forward, 0.08, sigma, t);

        assert!((0.0..=1.0).contains(&delta_itm));
        assert!((0.0..=1.0).contains(&delta_atm));
        assert!((0.0..=1.0).contains(&delta_otm));

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
    fn test_bachelier_gamma_positive() {
        let forward = 0.03;
        let strike = 0.025;
        let sigma = 0.005;
        let t = 1.0;

        let gamma = bachelier_gamma(forward, strike, sigma, t);
        assert!(gamma > 0.0, "Bachelier gamma should be positive");

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
}
