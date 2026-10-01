#[cfg(test)]
use super::characteristic_fn::{heston_pj_characteristic_function, HestonCfStatus};
#[cfg(test)]
use super::strip_pricer::HESTON_STRIP_MAX_CORRUPT_FRACTION;
#[cfg(test)]
use super::{HestonFourierSettings, HestonPricingParams};
use finstack_quant_core::math::gauss_legendre_grid;
#[cfg(test)]
use num_complex::Complex;
#[cfg(test)]
use std::f64::consts::PI;

pub(super) fn composite_gauss_legendre_grid(
    a: f64,
    b: f64,
    order: usize,
    panels: usize,
) -> Option<Vec<(f64, f64)>> {
    gauss_legendre_grid(a, b, order, panels).ok()
}

/// Fraction of the upper integration range whose absolute integrand mass is
/// used to estimate the truncated Gil-Pelaez tail (audit item 4).
pub(super) const HESTON_TAIL_WINDOW_FRACTION: f64 = 0.1;

/// Diagnostics from a single Gil-Pelaez probability inversion.
///
/// Carries the information needed to detect the two silent failure modes the
/// audit flagged: characteristic-function corruption (item 5) and truncation
/// of the Fourier integral at a fixed `u_max` (item 4).
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(super) struct HestonPjDiagnostics {
    /// Probability clamped to `[0, 1]` — the value used for pricing.
    pub(super) probability: f64,
    /// Probability *before* the `[0, 1]` clamp. A value materially outside
    /// `[0, 1]` is direct evidence that the truncated integral lost or gained
    /// mass; the clamp would otherwise hide it.
    pub(super) raw_probability: f64,
    /// Estimated magnitude of the truncated tail beyond `u_max`, expressed on
    /// the probability scale. Computed from the absolute integrand mass in the
    /// last [`HESTON_TAIL_WINDOW_FRACTION`] of the integration range — if the
    /// integrand has genuinely decayed this is tiny; if `u_max` is too small
    /// for the maturity it stays large.
    pub(super) tail_estimate: f64,
    /// `true` when too many interior integration nodes had a non-finite /
    /// overflow-zeroed characteristic function (see
    /// [`HESTON_STRIP_MAX_CORRUPT_FRACTION`]); the integral is then unreliable
    /// and must be rejected by the pricing driver.
    pub(super) corrupted: bool,
}

/// Compute the Pj probability for Heston call pricing via Fourier inversion,
/// returning full diagnostics alongside the value.
///
/// P_j = 0.5 + (1/π) ∫_0^∞ Re[exp(-i*φ*ln(K)) * ψ_j(φ) / (i*φ)] dφ
///
/// The integral is evaluated on the explicit composite Gauss-Legendre grid so
/// that, in a single pass, the routine can also:
/// - count overflow-zeroed characteristic-function nodes (audit item 5), and
/// - estimate the truncated tail mass beyond `u_max` (audit item 4).
///
/// # Arguments
///
/// * `j` - Probability index (1 or 2)
/// * `spot` - Current spot price
/// * `strike` - Strike price
/// * `time` - Time to maturity
/// * `params` - Heston model parameters
/// * `settings` - Integration settings
#[cfg(test)]
pub(super) fn heston_pj_with_diagnostics(
    j: u8,
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonPricingParams,
    settings: &HestonFourierSettings,
) -> HestonPjDiagnostics {
    let Some(grid) =
        composite_gauss_legendre_grid(0.0, settings.u_max, settings.gl_order, settings.panels)
    else {
        return HestonPjDiagnostics {
            probability: f64::NAN,
            raw_probability: f64::NAN,
            tail_estimate: f64::INFINITY,
            corrupted: true,
        };
    };

    heston_pj_on_grid(j, spot, strike, time, params, settings, &grid)
}

/// Gil-Pelaez `Pj` probability and diagnostics evaluated on a *prebuilt*
/// composite Gauss-Legendre grid.
///
/// The grid depends only on `settings` (not on `j`, `spot`, `strike`, or
/// `time`), so `heston_call_price_fourier` builds it once and
/// shares it across the `j = 1` and `j = 2` evaluations instead of rebuilding
/// the `gl_order * panels`-node grid twice per scalar price.
#[cfg(test)]
pub(super) fn heston_pj_on_grid(
    j: u8,
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonPricingParams,
    settings: &HestonFourierSettings,
    grid: &[(f64, f64)],
) -> HestonPjDiagnostics {
    let log_spot = spot.ln();
    let log_strike = strike.ln();
    let i = Complex::new(0.0, 1.0);

    // The tail window starts at this φ; absolute integrand mass beyond it
    // estimates the error from truncating the integral at `u_max`.
    let tail_window_start = settings.u_max * (1.0 - HESTON_TAIL_WINDOW_FRACTION);

    let mut integral = 0.0;
    let mut tail_abs_mass = 0.0;
    let mut interior_nodes = 0_usize;
    let mut corrupted_nodes = 0_usize;
    let mut ok_nodes = 0_usize;

    for (phi, weight) in grid {
        // Handle singularity at φ=0.
        if phi.abs() < settings.phi_eps {
            continue;
        }
        interior_nodes += 1;

        let (psi, status) = heston_pj_characteristic_function(j, *phi, time, log_spot, params);
        // Only **overflow**-corrupted nodes count toward the corruption
        // fraction; legitimate underflow (|ψ| → 0 with well-formed inputs)
        // contributes a correct zero to the integral.
        if status == HestonCfStatus::Overflow {
            corrupted_nodes += 1;
        }
        if status == HestonCfStatus::Ok {
            ok_nodes += 1;
        }

        let exp_term = (-i * *phi * log_strike).exp();
        let value = (exp_term * psi / (i * *phi)).re;
        if value.is_finite() {
            integral += *weight * value;
            if *phi >= tail_window_start {
                tail_abs_mass += weight.abs() * value.abs();
            }
        }
    }

    // Corrupted when too many nodes overflowed, or when *no* node carried
    // information at all (every interior node zeroed): the integral then
    // degenerates to the 0.5 baseline. Partial underflow is legitimate.
    let corrupted = interior_nodes > 0
        && ((corrupted_nodes as f64) / (interior_nodes as f64) > HESTON_STRIP_MAX_CORRUPT_FRACTION
            || ok_nodes == 0);

    let raw_probability = 0.5 + integral / PI;
    HestonPjDiagnostics {
        probability: raw_probability.clamp(0.0, 1.0),
        raw_probability,
        tail_estimate: tail_abs_mass / PI,
        corrupted,
    }
}
