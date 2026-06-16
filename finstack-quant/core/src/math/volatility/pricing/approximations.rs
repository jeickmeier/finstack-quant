/// Brenner-Subrahmanyam approximation for Black-76 implied volatility.
#[inline]
pub fn brenner_subrahmanyam_approx(forward: f64, strike: f64, option_price: f64, t: f64) -> f64 {
    const TWO_PI: f64 = 2.0 * std::f64::consts::PI;
    const DEFAULT_VOL: f64 = 0.2;

    if t <= 0.0 || option_price <= 0.0 || forward <= 0.0 || strike <= 0.0 {
        return DEFAULT_VOL;
    }

    let sqrt_2pi_over_t = (TWO_PI / t).sqrt();
    let sigma = sqrt_2pi_over_t * option_price / forward;
    sigma.clamp(0.01, 5.0)
}

/// Manaster-Koehler approximation for Black-76 implied volatility.
#[inline]
pub fn manaster_koehler_approx(forward: f64, strike: f64, t: f64) -> f64 {
    const DEFAULT_VOL: f64 = 0.2;

    if t <= 0.0 || forward <= 0.0 || strike <= 0.0 {
        return DEFAULT_VOL;
    }

    let moneyness = (forward / strike).ln().abs();
    if moneyness < 1e-10 {
        return DEFAULT_VOL;
    }

    let sigma = (2.0 * moneyness / t).sqrt();
    sigma.clamp(0.01, 5.0)
}

/// Combined initial guess for implied volatility solvers.
#[inline]
pub fn implied_vol_initial_guess(forward: f64, strike: f64, option_price: f64, t: f64) -> f64 {
    const DEFAULT_VOL: f64 = 0.2;

    if t <= 0.0 || option_price <= 0.0 || forward <= 0.0 || strike <= 0.0 {
        return DEFAULT_VOL;
    }

    let bs_approx = brenner_subrahmanyam_approx(forward, strike, option_price, t);
    let moneyness = (forward / strike).ln().abs();

    if moneyness > 0.2 {
        let mk_approx = manaster_koehler_approx(forward, strike, t);
        (bs_approx + mk_approx) / 2.0
    } else {
        bs_approx
    }
}
