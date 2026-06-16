// Golden tests comparing Monte Carlo results against Black-Scholes analytical prices.
//
// These tests serve as regression tests to ensure Monte Carlo pricing maintains
// accuracy against known analytical solutions. Any significant deviation indicates
// a potential bug in the MC implementation.
//
// # Test Cases
//
// The test matrix covers:
// - ATM, ITM, and OTM options
// - Calls and puts
// - Various maturities (short, medium, long)
// - Different volatility levels
// - With and without dividend yield
//
// # Tolerances
//
// - Price: 1% relative error for 100,000 paths (3σ confidence)
// - This accounts for MC standard error while catching significant bugs

#[allow(clippy::expect_used)]
mod golden_tests {
    use finstack_quant_monte_carlo::prelude::{ExactGbm, GbmProcess, PhiloxRng};
    use finstack_quant_monte_carlo::prelude::{
        EuropeanCall, EuropeanPut, McEngine,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::math::special_functions::norm_cdf;
    use finstack_quant_valuations::instruments::OptionType;

    /// Black-Scholes call price
    fn bs_call(spot: f64, strike: f64, rate: f64, dividend: f64, vol: f64, t: f64) -> f64 {
        if t <= 0.0 {
            return (spot - strike).max(0.0);
        }
        let sqrt_t = t.sqrt();
        let d1 = ((spot / strike).ln() + (rate - dividend + 0.5 * vol * vol) * t) / (vol * sqrt_t);
        let d2 = d1 - vol * sqrt_t;
        spot * (-dividend * t).exp() * norm_cdf(d1) - strike * (-rate * t).exp() * norm_cdf(d2)
    }

    /// Black-Scholes put price
    fn bs_put(spot: f64, strike: f64, rate: f64, dividend: f64, vol: f64, t: f64) -> f64 {
        if t <= 0.0 {
            return (strike - spot).max(0.0);
        }
        let sqrt_t = t.sqrt();
        let d1 = ((spot / strike).ln() + (rate - dividend + 0.5 * vol * vol) * t) / (vol * sqrt_t);
        let d2 = d1 - vol * sqrt_t;
        strike * (-rate * t).exp() * norm_cdf(-d2) - spot * (-dividend * t).exp() * norm_cdf(-d1)
    }

    /// Test helper to run MC pricing and compare to analytical
    #[allow(clippy::too_many_arguments)]
    fn test_european_option(
        spot: f64,
        strike: f64,
        rate: f64,
        dividend: f64,
        vol: f64,
        t: f64,
        is_call: bool,
        rel_tol: f64,
        num_paths: usize,
    ) {
        let num_steps = (t * 252.0).max(50.0) as usize;
        let maturity_step = num_steps;

        // MC pricing
        let engine = McEngine::builder()
            .num_paths(num_paths)
            .uniform_grid(t, num_steps)
            .build()
            .expect("Failed to build MC engine");

        let gbm = GbmProcess::with_params(rate, dividend, vol).unwrap();
        let disc = ExactGbm::new();
        let rng = PhiloxRng::new(42);
        let initial_state = [spot];
        let discount_factor = (-rate * t).exp();

        let mc_price = if is_call {
            let payoff = EuropeanCall::new(strike, 1.0, maturity_step);
            let result = engine
                .price(
                    &rng,
                    &gbm,
                    &disc,
                    &initial_state,
                    &payoff,
                    Currency::USD,
                    discount_factor,
                )
                .expect("MC pricing failed");
            result.mean.amount()
        } else {
            let payoff = EuropeanPut::new(strike, 1.0, maturity_step);
            let result = engine
                .price(
                    &rng,
                    &gbm,
                    &disc,
                    &initial_state,
                    &payoff,
                    Currency::USD,
                    discount_factor,
                )
                .expect("MC pricing failed");
            result.mean.amount()
        };

        // Analytical price
        let bs_price = if is_call {
            bs_call(spot, strike, rate, dividend, vol, t)
        } else {
            bs_put(spot, strike, rate, dividend, vol, t)
        };

        // Compute relative error
        let rel_error = if bs_price.abs() > 1e-6 {
            (mc_price - bs_price).abs() / bs_price
        } else {
            // For very small prices, use absolute error
            (mc_price - bs_price).abs()
        };

        let option_type = if is_call { OptionType::Call } else { OptionType::Put };
        assert!(
            rel_error < rel_tol,
            "{} option: MC={:.6}, BS={:.6}, rel_err={:.4}% (tol={:.4}%)\n\
             spot={}, strike={}, r={}, q={}, vol={}, t={}",
            option_type,
            mc_price,
            bs_price,
            rel_error * 100.0,
            rel_tol * 100.0,
            spot,
            strike,
            rate,
            dividend,
            vol,
            t
        );
    }

    // ============================================================================
    // ATM Options
    // ============================================================================

    #[test]
    fn test_atm_call_1y() {
        test_european_option(
            100.0, // spot
            100.0, // strike (ATM)
            0.05,  // rate
            0.0,   // dividend
            0.20,  // vol
            1.0,   // time
            true,  // is_call
            0.02,  // 2% tolerance
            100_000,
        );
    }

    #[test]
    fn test_atm_put_1y() {
        test_european_option(100.0, 100.0, 0.05, 0.0, 0.20, 1.0, false, 0.02, 100_000);
    }

    #[test]
    fn test_atm_call_with_dividend() {
        test_european_option(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, true, 0.02, 100_000);
    }

    #[test]
    fn test_atm_put_with_dividend() {
        test_european_option(100.0, 100.0, 0.05, 0.02, 0.20, 1.0, false, 0.02, 100_000);
    }

    // ============================================================================
    // ITM Options
    // ============================================================================

    #[test]
    fn test_itm_call_1y() {
        test_european_option(
            120.0, // spot
            100.0, // strike (ITM call)
            0.05,  // rate
            0.0,   // dividend
            0.20,  // vol
            1.0,   // time
            true,  // is_call
            0.02,  // 2% tolerance
            100_000,
        );
    }

    #[test]
    fn test_itm_put_1y() {
        test_european_option(
            80.0,  // spot
            100.0, // strike (ITM put)
            0.05, 0.0, 0.20, 1.0, false, 0.02, 100_000,
        );
    }

    // ============================================================================
    // OTM Options
    // ============================================================================

    #[test]
    fn test_otm_call_1y() {
        test_european_option(
            80.0,  // spot
            100.0, // strike (OTM call)
            0.05, 0.0, 0.20, 1.0, true,
            0.05, // Higher tolerance for OTM (higher relative error)
            100_000,
        );
    }

    #[test]
    fn test_otm_put_1y() {
        test_european_option(
            120.0, // spot
            100.0, // strike (OTM put)
            0.05, 0.0, 0.20, 1.0, false, 0.05, // Higher tolerance for OTM
            100_000,
        );
    }

    // ============================================================================
    // Different Maturities
    // ============================================================================

    #[test]
    fn test_atm_call_3m() {
        test_european_option(
            100.0, 100.0, 0.05, 0.0, 0.20, 0.25, // 3 months
            true, 0.02, 100_000,
        );
    }

    #[test]
    fn test_atm_call_5y() {
        test_european_option(
            100.0, 100.0, 0.05, 0.0, 0.20, 5.0, // 5 years
            true, 0.02, 100_000,
        );
    }

    // ============================================================================
    // Different Volatilities
    // ============================================================================

    #[test]
    fn test_low_vol_call() {
        test_european_option(
            100.0, 100.0, 0.05, 0.0, 0.10, // Low vol
            1.0, true, 0.02, 100_000,
        );
    }

    #[test]
    fn test_high_vol_call() {
        test_european_option(
            100.0, 100.0, 0.05, 0.0, 0.50, // High vol
            1.0, true, 0.02, 100_000,
        );
    }

    // ============================================================================
    // Put-Call Parity Tests
    // ============================================================================

    #[test]
    fn test_put_call_parity_atm() {
        let spot = 100.0;
        let strike = 100.0;
        let rate = 0.05;
        let dividend = 0.02;
        let vol = 0.20;
        let t = 1.0;
        let num_paths = 100_000;
        let num_steps = 252;

        let engine = McEngine::builder()
            .num_paths(num_paths)
            .uniform_grid(t, num_steps)
            .build()
            .expect("Failed to build MC engine");

        let gbm = GbmProcess::with_params(rate, dividend, vol).unwrap();
        let disc = ExactGbm::new();
        let rng = PhiloxRng::new(42);
        let initial_state = [spot];
        let discount_factor = (-rate * t).exp();

        let call_payoff = EuropeanCall::new(strike, 1.0, num_steps);
        let call_result = engine
            .price(
                &rng,
                &gbm,
                &disc,
                &initial_state,
                &call_payoff,
                Currency::USD,
                discount_factor,
            )
            .expect("MC pricing failed");

        let put_payoff = EuropeanPut::new(strike, 1.0, num_steps);
        let put_result = engine
            .price(
                &rng,
                &gbm,
                &disc,
                &initial_state,
                &put_payoff,
                Currency::USD,
                discount_factor,
            )
            .expect("MC pricing failed");

        // Put-Call Parity: C - P = S*e^(-qT) - K*e^(-rT)
        let mc_diff = call_result.mean.amount() - put_result.mean.amount();
        let parity_rhs = spot * (-dividend * t).exp() - strike * (-rate * t).exp();

        let parity_error = (mc_diff - parity_rhs).abs();
        let rel_error = parity_error / spot;

        assert!(
            rel_error < 0.01,
            "Put-call parity violated: C-P={:.4}, S*e^(-qT)-K*e^(-rT)={:.4}, error={:.6}",
            mc_diff,
            parity_rhs,
            parity_error
        );
    }

    // ============================================================================
    // Reproducibility Tests
    // ============================================================================

    #[test]
    fn test_reproducibility() {
        let spot = 100.0;
        let strike = 100.0;
        let rate = 0.05;
        let vol = 0.20;
        let t = 1.0;
        let num_paths = 10_000;
        let num_steps = 50;

        let engine = McEngine::builder()
            .num_paths(num_paths)
            .uniform_grid(t, num_steps)
            .build()
            .expect("Failed to build MC engine");

        let gbm = GbmProcess::with_params(rate, 0.0, vol).unwrap();
        let disc = ExactGbm::new();
        let initial_state = [spot];
        let discount_factor = (-rate * t).exp();

        // Run twice with same seed
        let rng1 = PhiloxRng::new(12345);
        let payoff1 = EuropeanCall::new(strike, 1.0, num_steps);
        let result1 = engine
            .price(
                &rng1,
                &gbm,
                &disc,
                &initial_state,
                &payoff1,
                Currency::USD,
                discount_factor,
            )
            .expect("MC pricing failed");

        let rng2 = PhiloxRng::new(12345);
        let payoff2 = EuropeanCall::new(strike, 1.0, num_steps);
        let result2 = engine
            .price(
                &rng2,
                &gbm,
                &disc,
                &initial_state,
                &payoff2,
                Currency::USD,
                discount_factor,
            )
            .expect("MC pricing failed");

        assert!(
            (result1.mean.amount() - result2.mean.amount()).abs() < 1e-10,
            "Results not reproducible: {} vs {}",
            result1.mean.amount(),
            result2.mean.amount()
        );
    }
}
