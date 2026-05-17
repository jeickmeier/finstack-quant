//! MC-harness validation test against closed-form HW1F results.
//!
//! Under HW1F with constant θ, the terminal short rate `r_T` is Gaussian:
//!
//! ```text
//! r_T ~ N(μ, σ_r²),  μ = r0·e^{-κT} + θ·(1 − e^{-κT}),
//!                    σ_r² = σ²·(1 − e^{-2κT}) / (2κ).
//! ```
//!
//! For a European call struck at `K`,
//! `E[(r_T − K)⁺] = (μ − K)·Φ(d) + σ_r·φ(d)` with `d = (μ − K)/σ_r`.
//!
//! The test runs a large batch of paths through [`RateExoticHw1fMcPricer`]
//! and checks the MC estimator against the closed-form expectation within
//! 5 × stderr. θ is set to 0 so `μ = r0·e^{-κT}` (pure OU).

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use finstack_core::currency::Currency;
use finstack_core::math::special_functions::norm_cdf;
use finstack_core::money::Money;
use finstack_monte_carlo::process::ou::HullWhite1FParams;
use finstack_monte_carlo::traits::{PathState, Payoff, StateKey};
use finstack_valuations::instruments::rates::exotics_shared::hw1f_mc::RateExoticHw1fMcPricer;
use finstack_valuations::instruments::rates::exotics_shared::mc_config::RateExoticMcConfig;

/// Standard-normal PDF φ(x) = exp(−x²/2) / √(2π).
fn normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

#[derive(Debug, Clone, Default)]
struct ShortRateCallPayoff {
    strike: f64,
    notional: f64,
    rate_at_expiry: Option<f64>,
}

impl Payoff for ShortRateCallPayoff {
    fn on_event(&mut self, s: &mut PathState) {
        self.rate_at_expiry = s.get_key(StateKey::ShortRate);
    }
    fn value(&self, ccy: Currency) -> Money {
        let r = self.rate_at_expiry.unwrap_or(0.0);
        Money::new(self.notional * (r - self.strike).max(0.0), ccy)
    }
    fn reset(&mut self) {
        self.rate_at_expiry = None;
    }
}

#[test]
fn short_rate_european_call_matches_analytical() {
    let kappa: f64 = 0.05;
    let sigma: f64 = 0.01;
    let r0: f64 = 0.03;
    let t: f64 = 2.0;
    let strike: f64 = 0.035;
    let notional: f64 = 1_000_000.0;

    // Analytical expectation with θ = 0 (pure OU).
    let mu = r0 * (-kappa * t).exp();
    let var_r = sigma * sigma * (1.0 - (-2.0 * kappa * t).exp()) / (2.0 * kappa);
    let sig_r = var_r.sqrt();
    let d = (mu - strike) / sig_r;
    let expected = notional * ((mu - strike) * norm_cdf(d) + sig_r * normal_pdf(d));

    // Constant θ = 0 ⇒ pure Ornstein-Uhlenbeck, matching the analytical μ above.
    let pricer = RateExoticHw1fMcPricer {
        process_params: HullWhite1FParams::new(kappa, sigma, 0.0),
        r0,
        event_times: vec![t],
        config: RateExoticMcConfig {
            num_paths: 200_000,
            seed: 1,
            ..Default::default()
        },
        currency: Currency::USD,
    };
    let est = pricer
        .price(|| ShortRateCallPayoff {
            strike,
            notional,
            rate_at_expiry: None,
        })
        .expect("MC pricing should succeed");

    let tol = 5.0 * est.stderr;
    assert!(
        (est.mean.amount() - expected).abs() < tol,
        "MC = {:.4}, analytical = {:.4}, stderr = {:.4}, tol = {:.4}, diff = {:.4}",
        est.mean.amount(),
        expected,
        est.stderr,
        tol,
        (est.mean.amount() - expected).abs()
    );

    println!(
        "MC = {:.6}, analytical = {:.6}, stderr = {:.6}, tol = {:.6}, diff = {:.6}",
        est.mean.amount(),
        expected,
        est.stderr,
        tol,
        (est.mean.amount() - expected).abs()
    );
}
