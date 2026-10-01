use super::characteristic_fn::{heston_pj_characteristic_function, HestonCfStatus};
use super::fourier_prices::validate_heston_inputs;
use super::params::HESTON_TAIL_DIAGNOSTIC_THRESHOLD;
use super::quadrature::{composite_gauss_legendre_grid, HESTON_TAIL_WINDOW_FRACTION};
use super::{HestonFourierSettings, HestonPricingParams};
use finstack_quant_core::{Error, Result};
use num_complex::Complex;
use std::f64::consts::PI;

/// Cached Heston Fourier data for pricing multiple strikes with shared parameters.
///
/// The characteristic function portion of the Gil-Pelaez integrand is independent
/// of strike, so it can be precomputed once on the composite Gauss-Legendre grid
/// and reused across a strike strip.
#[derive(Debug, Clone)]
pub struct HestonStripPricer {
    spot: f64,
    time: f64,
    params: HestonPricingParams,
    /// Composite quadrature grid as `(phi, weight)` pairs.
    grid: Vec<(f64, f64)>,
    /// Start of the tail window `u_max * (1 - HESTON_TAIL_WINDOW_FRACTION)`,
    /// precomputed from `grid` so `probability` does not rescan it per call.
    tail_window_start: f64,
    /// Largest CF magnitude in the final tenth of the integration interval.
    tail_cf_magnitude: f64,
    /// Cached `psi_1(phi) / (i * phi)` values on the grid.
    psi1_over_iphi: Vec<Complex<f64>>,
    /// Cached `psi_2(phi) / (i * phi)` values on the grid.
    psi2_over_iphi: Vec<Complex<f64>>,
    /// `true` when too many grid nodes had a non-finite or overflow-zeroed
    /// characteristic function, making the cached integral unreliable.
    pub(super) integrand_corrupted: bool,
}

/// No overflow-corrupted characteristic-function node is acceptable. Legitimate
/// tail underflow remains valid and does not count toward this threshold.
pub(super) const HESTON_STRIP_MAX_CORRUPT_FRACTION: f64 = 0.0;

impl HestonStripPricer {
    /// Build a strip pricer with characteristic-function values cached on the
    /// composite Gauss-Legendre integration grid.
    ///
    /// Returns `None` for invalid inputs, non-positive maturity, or a grid
    /// exceeding the node budget. Prices reject unresolved tails and corrupt nodes.
    ///
    /// # Arguments
    ///
    /// * `spot` - Positive finite underlying price in quote units.
    /// * `time` - Positive finite remaining maturity in years.
    /// * `params` - Finite rates and valid canonical Heston variance parameters.
    /// * `settings` - Validated integration extent, quadrature order, and panel count.
    #[must_use]
    pub fn new(
        spot: f64,
        time: f64,
        params: &HestonPricingParams,
        settings: &HestonFourierSettings,
    ) -> Option<Self> {
        validate_heston_inputs(spot, time, params).ok()?;
        settings.validate().ok()?;
        if time <= 0.0 {
            return None;
        }
        let grid =
            composite_gauss_legendre_grid(0.0, settings.u_max, settings.gl_order, settings.panels)?;
        let i = Complex::new(0.0, 1.0);
        let log_spot = spot.ln();
        let mut psi1_over_iphi = Vec::with_capacity(grid.len());
        let mut psi2_over_iphi = Vec::with_capacity(grid.len());

        // Count interior nodes (φ away from the singularity) and how many of
        // them returned an **overflow**-corrupted characteristic function.
        // Legitimate underflow (well-formed inputs, |ψ| → 0 in the decayed
        // tail) contributes exactly zero to the integral and must not count
        // toward corruption — long-dated / high-κθ surfaces underflow over
        // much of the grid without any loss of pricing accuracy.
        let mut interior_nodes = 0_usize;
        let mut corrupted_nodes = 0_usize;
        let mut ok_nodes = 0_usize;
        let mut tail_cf_magnitude = 0.0_f64;
        let tail_window_start = settings.u_max * (1.0 - HESTON_TAIL_WINDOW_FRACTION);

        for (phi, _) in &grid {
            if phi.abs() < settings.phi_eps {
                psi1_over_iphi.push(Complex::new(0.0, 0.0));
                psi2_over_iphi.push(Complex::new(0.0, 0.0));
                continue;
            }

            interior_nodes += 1;
            let denom = i * *phi;
            let (psi1, status1) =
                heston_pj_characteristic_function(1, *phi, time, log_spot, params);
            let (psi2, status2) =
                heston_pj_characteristic_function(2, *phi, time, log_spot, params);

            if status1 == HestonCfStatus::Overflow || status2 == HestonCfStatus::Overflow {
                corrupted_nodes += 1;
            }
            if status1 == HestonCfStatus::Ok && status2 == HestonCfStatus::Ok {
                ok_nodes += 1;
            }

            if *phi >= tail_window_start {
                tail_cf_magnitude = tail_cf_magnitude.max(psi1.norm()).max(psi2.norm());
            }
            psi1_over_iphi.push(psi1 / denom);
            psi2_over_iphi.push(psi2 / denom);
        }

        // Corrupted when too many nodes overflowed, or when *no* node carried
        // information at all (every interior node zeroed): the Gil-Pelaez
        // integral then degenerates to the 0.5 baseline and the resulting
        // price is plausible-but-wrong. Partial underflow (decayed tail) is
        // legitimate and does not count.
        let integrand_corrupted = interior_nodes == 0
            || ((corrupted_nodes as f64) / (interior_nodes as f64)
                > HESTON_STRIP_MAX_CORRUPT_FRACTION
                || ok_nodes == 0);

        Some(Self {
            spot,
            time,
            params: *params,
            grid,
            tail_window_start,
            tail_cf_magnitude,
            psi1_over_iphi,
            psi2_over_iphi,
            integrand_corrupted,
        })
    }

    /// Evaluate one Gil-Pelaez probability on the cached grid.
    ///
    /// Returns `(clamped_probability, raw_probability, tail_estimate)`. The raw
    /// (pre-clamp) probability and the truncated-tail estimate let the caller
    /// detect `u_max` truncation error that the `[0, 1]` clamp would otherwise
    /// hide (audit item 4). The tail estimate is the absolute integrand mass in
    /// the last [`HESTON_TAIL_WINDOW_FRACTION`] of the integration range,
    /// divided by π.
    fn probability(&self, log_strike: f64, cached_values: &[Complex<f64>]) -> (f64, f64, f64) {
        let i = Complex::new(0.0, 1.0);
        let mut integral = 0.0;
        let mut tail_abs_mass = 0.0;

        // `u_max` (largest grid abscissa) and the tail-window start were
        // precomputed at construction.
        let tail_window_start = self.tail_window_start;

        for ((phi, weight), cached) in self.grid.iter().zip(cached_values.iter()) {
            let exp_term = (-i * *phi * log_strike).exp();
            let value = (exp_term * *cached).re;
            if !value.is_finite() {
                return (f64::NAN, f64::NAN, f64::INFINITY);
            }
            integral += *weight * value;
            if *phi >= tail_window_start {
                tail_abs_mass += weight.abs() * value.abs();
            }
        }

        let raw = 0.5 + integral / PI;
        (raw.clamp(0.0, 1.0), raw, tail_abs_mass / PI)
    }

    /// Price a single European call using the cached strip pricer.
    ///
    /// Returns a structured convergence error when characteristic-function
    /// corruption, an unresolved tail, or invalid bounds make the result unreliable.
    ///
    /// # Arguments
    ///
    /// * `strike` - Positive finite exercise price in the same quote units as spot.
    pub fn price_call(&self, strike: f64) -> Result<f64> {
        if !strike.is_finite() || strike <= 0.0 {
            return Err(Error::Validation(
                "Heston strike must be finite and positive".to_string(),
            ));
        }
        if self.integrand_corrupted {
            return Err(Error::Calibration {
                category: "heston_fourier".to_string(),
                message: format!(
                    "Heston strip integration is corrupted for spot={}, strike={}, time={}",
                    self.spot, strike, self.time
                ),
            });
        }

        let log_strike = strike.ln();
        let (p1, raw_p1, tail_p1) = self.probability(log_strike, &self.psi1_over_iphi);
        let (p2, raw_p2, tail_p2) = self.probability(log_strike, &self.psi2_over_iphi);

        let discounted_spot = self.spot * (-self.params.q * self.time).exp();
        let discounted_strike = strike * (-self.params.r * self.time).exp();
        let price_tolerance = HESTON_TAIL_DIAGNOSTIC_THRESHOLD * discounted_spot;
        let tail_price = discounted_spot * tail_p1 + discounted_strike * tail_p2;
        let probability_excursion =
            discounted_spot * (raw_p1 - p1).abs() + discounted_strike * (raw_p2 - p2).abs();
        // The CF magnitude catches unresolved short-time tails even when
        // cancellation makes the final-window probability integral tiny.
        if self.tail_cf_magnitude > 1e-8
            || tail_price > price_tolerance
            || probability_excursion > price_tolerance
            || !tail_price.is_finite()
        {
            return Err(Error::Calibration {
                category: "heston_fourier".to_string(),
                message: format!("Heston integration tail is unresolved: price-tail estimate={tail_price}, CF magnitude={}", self.tail_cf_magnitude),
            });
        }

        let call_price = self.spot * (-self.params.q * self.time).exp() * p1
            - strike * (-self.params.r * self.time).exp() * p2;

        let lower_bound = (discounted_spot - discounted_strike).max(0.0);
        if !call_price.is_finite()
            || call_price < lower_bound - price_tolerance
            || call_price > discounted_spot + price_tolerance
        {
            return Err(Error::Calibration {
                category: "heston_fourier".to_string(),
                message: format!(
                    "Heston strip integration produced a non-finite price for \
                     spot={}, strike={}, time={}",
                    self.spot, strike, self.time
                ),
            });
        }

        Ok(call_price.clamp(lower_bound, discounted_spot))
    }

    /// Price a strip of European calls using the cached strip pricer.
    ///
    /// # Arguments
    ///
    /// * `strikes` - Positive finite exercise prices in spot units and result order.
    pub fn price_calls(&self, strikes: &[f64]) -> Result<Vec<f64>> {
        strikes
            .iter()
            .map(|&strike| self.price_call(strike))
            .collect()
    }
}
