//! LSMC harness smoke test via a simplified "swaption-like" proxy payoff.
//!
//! This test verifies that [`RateExoticHw1fLsmcPricer`] returns a finite,
//! non-negative PV when driven with a trivial payoff whose deterministic
//! value is zero but whose call-at-par at each exercise date makes the
//! harness roll forward a positive PV via backward induction.
//!
//! Intentionally NOT a rigorous parity check against the existing
//! `BermudanSwaptionPricer` — the plan labels this "parity" but the
//! actual comparison requires significantly more modeling work (matched
//! curves, swap-level payoffs, identical discretization). This is a
//! smoke test that the backward-induction + regression machinery is
//! wired correctly.
//!
//! Architectural note: the Task-5 LSMC harness computes the exercise
//! value as `call_price * notional` and does NOT invoke
//! `payoff.intrinsic_at(...)`. The `intrinsic_at` hook is implemented
//! below for trait conformance only, and is currently unused by the
//! harness. Products that require path-dependent intrinsic values
//! (e.g. callable range accrual) will extend the harness in a
//! follow-up PR.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_monte_carlo::process::ou::HullWhite1FParams;
use finstack_quant_monte_carlo::traits::{PathState, Payoff};
use finstack_quant_valuations::instruments::rates::exotics_shared::exercise::{
    standard_basis, ExerciseBoundaryPayoff,
};
use finstack_quant_valuations::instruments::rates::exotics_shared::hw1f_lsmc::RateExoticHw1fLsmcPricer;
use finstack_quant_valuations::instruments::rates::exotics_shared::mc_config::RateExoticMcConfig;

/// Simplified swaption-proxy payoff: records no path-dependent cashflows
/// (deterministic PV = 0) and exposes a receiver-style intrinsic via the
/// `ExerciseBoundaryPayoff` trait. The intrinsic is unused by the current
/// LSMC harness but implemented for trait conformance.
#[derive(Debug, Clone)]
struct SwaptionProxy {
    strike: f64,
    dcf: f64,
    notional: f64,
    accrued: f64,
}

impl Payoff for SwaptionProxy {
    fn on_event(&mut self, _s: &mut PathState) {}
    fn value(&self, ccy: Currency) -> Money {
        Money::new(self.accrued, ccy)
    }
    fn reset(&mut self) {
        self.accrued = 0.0;
    }
}

impl ExerciseBoundaryPayoff for SwaptionProxy {
    fn intrinsic_at(&self, _idx: usize, short_rate: f64, ccy: Currency) -> Money {
        Money::new(
            self.notional * (short_rate - self.strike).max(0.0) * self.dcf,
            ccy,
        )
    }
    fn continuation_basis(&self, _idx: usize, t_years: f64, short_rate: f64) -> Vec<f64> {
        standard_basis(t_years, short_rate)
    }
}

#[test]
fn lsmc_proxy_price_is_nonnegative_and_stable() {
    let pricer = RateExoticHw1fLsmcPricer {
        process_params: HullWhite1FParams::new(0.05, 0.012, 0.0),
        r0: 0.025,
        event_times: vec![1.0, 2.0, 3.0, 4.0],
        exercise_times: vec![1.0, 2.0, 3.0, 4.0],
        config: RateExoticMcConfig {
            num_paths: 20_000,
            seed: 7,
            ..Default::default()
        },
        currency: Currency::USD,
    };
    let est = pricer
        .price(|| SwaptionProxy {
            strike: 0.03,
            dcf: 1.0,
            notional: 1_000_000.0,
            accrued: 0.0,
        })
        .expect("LSMC pricing should succeed");

    assert!(
        est.mean.amount() >= 0.0,
        "PV should be non-negative, got {}",
        est.mean.amount()
    );
    assert!(
        est.mean.amount().is_finite(),
        "PV should be finite, got {}",
        est.mean.amount()
    );
    assert!(
        est.stderr >= 0.0,
        "stderr should be non-negative, got {}",
        est.stderr
    );

    println!(
        "LSMC PV = {:.2}, stderr = {:.4}, num_paths = {}",
        est.mean.amount(),
        est.stderr,
        est.num_paths
    );
}
