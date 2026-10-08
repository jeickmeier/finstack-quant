//! Dupire local variance in total-variance coordinates.
//!
//! One evaluation of the Dupire formula on an implied-volatility grid, shared
//! by the arbitrage check
//! ([`LocalVolDensityCheck`](crate::volatility::arbitrage::LocalVolDensityCheck))
//! and the local-volatility extractor
//! ([`LocalVolSurface`](crate::volatility::local_vol::LocalVolSurface)).
//!
//! # Formula
//!
//! With total implied variance `w(k, T) = σ²(k, T)·T` and log-moneyness
//! `k = ln(K / F_T)`:
//!
//! ```text
//! σ²_loc(K, T) = (∂w/∂T) / g
//! g = 1 − (k/w)·∂w/∂k + ¼·(−¼ − 1/w + k²/w²)·(∂w/∂k)² + ½·∂²w/∂k²
//! ```
//!
//! `∂w/∂T` is taken at fixed log-moneyness, so the cash strike is remapped to
//! `F_j·e^k` at each neighbouring expiry `T_j`. `g` is proportional to the
//! risk-neutral density of the underlying: `g ≥ 0` is the absence of butterfly
//! arbitrage and `∂w/∂T ≥ 0` the absence of calendar arbitrage.
//!
//! # References
//!
//! - Dupire, B. (1994). "Pricing with a Smile." *Risk*, 7(1), 18-20.
//!   `docs/REFERENCES.md#dupire-1994`
//! - Gatheral, J. (2006). *The Volatility Surface: A Practitioner's Guide*.
//!   Wiley. Chapter 1, eq. (1.10). `docs/REFERENCES.md#gatheral-volatility-surface`

use crate::volatility::{get_surface_vol, get_surface_vol_clamped};
use finstack_quant_core::market_data::surfaces::VolSurface;

/// Total variance below which the formula's `1/w` terms are not evaluated.
pub(crate) const MIN_TOTAL_VARIANCE: f64 = 1e-14;

/// How the time derivative reads a neighbouring expiry at a remapped strike
/// `F_j·e^k` that falls outside the strike grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoneynessLookup {
    /// No value: the time derivative is unavailable at that node. Used to
    /// assess arbitrage on observed moneyness only, because a clamped strike
    /// changes `k` and can fabricate a calendar violation.
    Strict,
    /// The implied volatility of the nearest grid strike (a flat wing), so
    /// every node has a time derivative.
    Clamped,
}

/// Dupire inputs at one grid node.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DupireNode {
    /// Slope of total variance in expiry at fixed log-moneyness, `∂w/∂T`;
    /// `None` when a neighbouring expiry cannot be read (see
    /// [`MoneynessLookup::Strict`]) or the expiries coincide.
    pub(crate) dw_dt: Option<f64>,
    /// Denominator `g` of the Dupire formula.
    pub(crate) density: f64,
}

/// Evaluate the Dupire numerator and denominator at grid node
/// (`expiry_index`, `strike_index`) of `surface`.
///
/// `forwards` holds one finite positive forward per surface expiry. Returns
/// `None` when the node's total variance is below [`MIN_TOTAL_VARIANCE`].
/// The caller guarantees at least two expiries and three strikes.
pub(crate) fn dupire_node(
    surface: &VolSurface,
    forwards: &[f64],
    expiry_index: usize,
    strike_index: usize,
    lookup: MoneynessLookup,
) -> Option<DupireNode> {
    let expiries = surface.expiries();
    let strikes = surface.strikes();
    let t = expiries[expiry_index];
    let big_k = strikes[strike_index];
    let k = (big_k / forwards[expiry_index]).ln(); // log-moneyness
    let v = get_surface_vol_clamped(surface, t, big_k);
    let w = v * v * t;

    if w < MIN_TOTAL_VARIANCE {
        return None;
    }

    let dw_dt = finite_diff_time(surface, expiries, forwards, expiry_index, k, lookup);

    let (dw_dstrike, d2w_dstrike2) = finite_diff_strike(surface, strikes, strike_index, t);
    // k = ln(K/F): d/dk = K d/dK. The density condition is
    // dimensionless and must be invariant to price-unit changes.
    let dw_dk = big_k * dw_dstrike;
    let d2w_dk2 = big_k * big_k * d2w_dstrike2 + dw_dk;

    let term1 = 1.0 - k / w * dw_dk;
    let term2 = 0.25 * (-0.25 - 1.0 / w + k * k / (w * w)) * dw_dk * dw_dk;
    let term3 = 0.5 * d2w_dk2;
    Some(DupireNode {
        dw_dt,
        density: term1 + term2 + term3,
    })
}

/// Compute dw/dT at fixed log-moneyness, remapping the cash strike at each expiry.
///
/// Uses central differences for interior points and one-sided differences at
/// boundaries.
pub(crate) fn finite_diff_time(
    surface: &VolSurface,
    expiries: &[f64],
    forwards: &[f64],
    ei: usize,
    k: f64,
    lookup: MoneynessLookup,
) -> Option<f64> {
    let total_var = |idx: usize| -> Option<f64> {
        let t = expiries[idx];
        let strike = forwards[idx] * k.exp();
        let v = match lookup {
            // Assess only overlapping observed moneyness. Clamping a remapped
            // strike would change k and can fabricate calendar arbitrage.
            MoneynessLookup::Strict => get_surface_vol(surface, t, strike).ok()?,
            MoneynessLookup::Clamped => get_surface_vol_clamped(surface, t, strike),
        };
        Some(v * v * t)
    };

    let n = expiries.len();
    if n < 2 {
        return None;
    }

    if ei == 0 {
        // Forward difference
        let dt = expiries[1] - expiries[0];
        if dt.abs() < 1e-14 {
            return None;
        }
        Some((total_var(1)? - total_var(0)?) / dt)
    } else if ei == n - 1 {
        // Backward difference
        let dt = expiries[n - 1] - expiries[n - 2];
        if dt.abs() < 1e-14 {
            return None;
        }
        Some((total_var(n - 1)? - total_var(n - 2)?) / dt)
    } else {
        // Central difference
        let dt = expiries[ei + 1] - expiries[ei - 1];
        if dt.abs() < 1e-14 {
            return None;
        }
        Some((total_var(ei + 1)? - total_var(ei - 1)?) / dt)
    }
}

/// Compute dw/dK and d2w/dK2 at a grid point using finite differences along
/// the strike axis.
///
/// Returns (first_derivative, second_derivative) of total variance with
/// respect to the raw strike K.
fn finite_diff_strike(surface: &VolSurface, strikes: &[f64], si: usize, expiry: f64) -> (f64, f64) {
    let total_var = |idx: usize| -> f64 {
        let v = get_surface_vol_clamped(surface, expiry, strikes[idx]);
        v * v * expiry
    };

    let n = strikes.len();
    if n < 3 {
        return (0.0, 0.0);
    }

    if si == 0 {
        // Forward differences
        let dk1 = strikes[1] - strikes[0];
        let dk2 = strikes[2] - strikes[0];
        if dk1.abs() < 1e-14 || dk2.abs() < 1e-14 {
            return (0.0, 0.0);
        }
        let w0 = total_var(0);
        let w1 = total_var(1);
        let w2 = total_var(2);
        let dw_dk = (w1 - w0) / dk1;
        let dk12 = strikes[2] - strikes[1];
        let dk_avg = 0.5 * (dk1 + dk12);
        let d2w_dk2 = (w2 / dk12 - w1 * (1.0 / dk12 + 1.0 / dk1) + w0 / dk1) / dk_avg;
        (dw_dk, d2w_dk2)
    } else if si == n - 1 {
        // Backward differences
        let dk1 = strikes[n - 1] - strikes[n - 2];
        let dk2 = strikes[n - 1] - strikes[n - 3];
        if dk1.abs() < 1e-14 || dk2.abs() < 1e-14 {
            return (0.0, 0.0);
        }
        let w0 = total_var(n - 3);
        let w1 = total_var(n - 2);
        let w2 = total_var(n - 1);
        let dw_dk = (w2 - w1) / dk1;
        let dk01 = strikes[n - 2] - strikes[n - 3];
        let dk_avg = 0.5 * (dk01 + dk1);
        let d2w_dk2 = (w2 / dk1 - w1 * (1.0 / dk1 + 1.0 / dk01) + w0 / dk01) / dk_avg;
        (dw_dk, d2w_dk2)
    } else {
        // Central differences
        let dk_minus = strikes[si] - strikes[si - 1];
        let dk_plus = strikes[si + 1] - strikes[si];
        if dk_minus.abs() < 1e-14 || dk_plus.abs() < 1e-14 {
            return (0.0, 0.0);
        }
        let w_minus = total_var(si - 1);
        let w_center = total_var(si);
        let w_plus = total_var(si + 1);

        let dw_dk = (w_plus - w_minus) / (dk_minus + dk_plus);
        let dk_avg = 0.5 * (dk_minus + dk_plus);
        let d2w_dk2 = (w_plus / dk_plus - w_center * (1.0 / dk_plus + 1.0 / dk_minus)
            + w_minus / dk_minus)
            / dk_avg;

        (dw_dk, d2w_dk2)
    }
}
