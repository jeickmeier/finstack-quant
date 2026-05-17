//! Local Volatility Model (Dupire).
//!
//! Implements Dupire's formula to construct a local volatility surface $\sigma_{loc}(K, T)$
//! from an implied volatility surface $\sigma_{imp}(K, T)$.
//!
//! $$ \sigma_{loc}^2(K, T) = \frac{\frac{\partial C}{\partial T} + (r-q)K\frac{\partial C}{\partial K} + qC}{\frac{1}{2}K^2 \frac{\partial^2 C}{\partial K^2}} $$
//!
//! where $C(K, T)$ is the call price surface, $r$ is the risk-free rate, and $q$ is the
//! dividend yield.

use crate::instruments::common_impl::parameters::VolatilityModel;
use finstack_core::dates::Date;
use finstack_core::Result;
use std::sync::Arc;

/// Bilinear Interpolation in 2D.
#[derive(Debug, Clone)]
pub struct BilinearInterp {
    xs: Vec<f64>,
    ys: Vec<f64>,
    z_flat: Vec<f64>,
}

impl BilinearInterp {
    /// Create a new Bilinear Interpolator.
    ///
    /// # Arguments
    /// * `xs`: Grid points for the X (slow) dimension, sorted ascending.
    /// * `ys`: Grid points for the Y (fast) dimension, sorted ascending.
    /// * `z_flat`: Values at all grid points in **row-major order**: X varies
    ///   slowest, Y varies fastest.  Concretely, `z_flat[i * ys.len() + j]`
    ///   is the value at `(xs[i], ys[j])`.
    ///
    /// # Axis-order contract
    ///
    /// The caller must fill `z_flat` in the same `for x { for y { ... } }` order that
    /// this struct expects.  `LocalVolBuilder::from_implied_vol` fills its grid as
    /// `for t { for k { ... } }` (times = X, strikes = Y), matching this contract.
    pub fn new(xs: Vec<f64>, ys: Vec<f64>, z_flat: Vec<f64>) -> Result<Self> {
        if xs.len() < 2 || ys.len() < 2 {
            return Err(finstack_core::Error::Validation(
                "Bilinear interpolation requires at least a 2x2 grid".into(),
            ));
        }
        if xs.len() * ys.len() != z_flat.len() {
            return Err(finstack_core::Error::Validation(
                "Grid dimensions do not match values length".into(),
            ));
        }
        debug_assert!(
            xs.windows(2).all(|w| w[0] <= w[1]),
            "BilinearInterp: xs must be sorted"
        );
        debug_assert!(
            ys.windows(2).all(|w| w[0] <= w[1]),
            "BilinearInterp: ys must be sorted"
        );
        Ok(Self { xs, ys, z_flat })
    }

    /// Interpolate at coordinate (x, y).
    pub fn interpolate(&self, x: f64, y: f64) -> Result<f64> {
        // Find indices
        let i = match self.xs.binary_search_by(|v| v.total_cmp(&x)) {
            Ok(idx) => idx,
            Err(idx) => {
                if idx == 0 {
                    0
                } else if idx >= self.xs.len() {
                    self.xs.len() - 2
                } else {
                    idx - 1
                }
            }
        };
        let j = match self.ys.binary_search_by(|v| v.total_cmp(&y)) {
            Ok(idx) => idx,
            Err(idx) => {
                if idx == 0 {
                    0
                } else if idx >= self.ys.len() {
                    self.ys.len() - 2
                } else {
                    idx - 1
                }
            }
        };

        // Ensure bounds (clamping)
        let i = i.min(self.xs.len().saturating_sub(2));
        let j = j.min(self.ys.len().saturating_sub(2));

        let x1 = self.xs[i];
        let x2 = self.xs[i + 1];
        let y1 = self.ys[j];
        let y2 = self.ys[j + 1];

        let z11 = self.z_flat[i * self.ys.len() + j];
        let z12 = self.z_flat[i * self.ys.len() + j + 1];
        let z21 = self.z_flat[(i + 1) * self.ys.len() + j];
        let z22 = self.z_flat[(i + 1) * self.ys.len() + j + 1];

        // Bilinear interpolation formula
        let denom = (x2 - x1) * (y2 - y1);
        if denom.abs() < 1e-12 {
            return Ok(z11); // Points coincide
        }

        let w11 = (x2 - x) * (y2 - y);
        let w12 = (x2 - x) * (y - y1);
        let w21 = (x - x1) * (y2 - y);
        let w22 = (x - x1) * (y - y1);

        let z = (z11 * w11 + z12 * w12 + z21 * w21 + z22 * w22) / denom;
        Ok(z)
    }
}

/// Local Volatility Surface.
///
/// Represents the instantaneous volatility $\sigma(S, t)$ as a function of spot price and time.
#[derive(Debug, Clone)]
pub struct LocalVolSurface {
    /// Base date of the surface
    pub base_date: Date,
    /// Interpolator for local volatility $\sigma(S, t)$
    /// X-axis: Time (years)
    /// Y-axis: Spot/Strike
    /// Z-axis: Local Volatility
    pub surface: Arc<BilinearInterp>,
}

impl LocalVolSurface {
    /// Create a new Local Volatility Surface.
    pub fn new(base_date: Date, surface: Arc<BilinearInterp>) -> Self {
        Self { base_date, surface }
    }

    /// Get local volatility at a given time and spot.
    ///
    /// # Arguments
    /// * `t`: Time to maturity (years)
    /// * `spot`: Spot price level
    pub fn get_vol(&self, t: f64, spot: f64) -> Result<f64> {
        // Ensure t is non-negative
        let t = t.max(0.0);
        // Ensure spot is positive
        let spot = spot.max(1e-6);

        self.surface.interpolate(t, spot)
    }
}

/// Parameters for constructing a local volatility surface via Dupire's formula.
pub struct DupireParams<'a> {
    /// As-of date for the surface.
    pub base_date: Date,
    /// Current spot price (or forward rate for rates).
    pub spot: f64,
    /// Risk-free rate (continuous).
    pub rate: f64,
    /// Dividend yield (continuous).
    pub div_yield: f64,
    /// Grid of strikes for the local vol surface.
    pub strikes: &'a [f64],
    /// Grid of times (years) for the local vol surface.
    pub times: &'a [f64],
    /// Lognormal (`VolatilityModel::Black`) or normal (`VolatilityModel::Normal`).
    pub vol_model: VolatilityModel,
}

/// Builder for Local Volatility Surface from Implied Volatility.
pub struct LocalVolBuilder;

impl LocalVolBuilder {
    /// Construct Local Volatility from Implied Volatility using Dupire's formula.
    ///
    /// Supports both lognormal (Black) and normal (Bachelier) volatility models.
    /// For rates-scale data (forward ~ 0.01-0.05), use `VolatilityModel::Normal`
    /// with normal implied vols to avoid the numerical instability of the lognormal
    /// model at small absolute levels.
    ///
    /// # Dupire Formulas
    ///
    /// **Lognormal** (Black) — spot measure with discounted call prices:
    /// $$ \sigma_{loc}^2 = \frac{\partial C/\partial T + (r-q)K\,\partial C/\partial K + qC}
    ///                          {\tfrac{1}{2}K^2\,\partial^2 C/\partial K^2} $$
    ///
    /// **Normal** (Bachelier) — forward measure with undiscounted call prices at fixed forward:
    /// $$ \sigma_{N,loc}^2 = \frac{\partial C_{und}/\partial T}
    ///                            {\tfrac{1}{2}\,\partial^2 C_{und}/\partial K^2} $$
    #[allow(non_snake_case)]
    pub fn from_implied_vol<F>(implied_vol: F, params: DupireParams<'_>) -> Result<LocalVolSurface>
    where
        F: Fn(f64, f64) -> Result<f64>,
    {
        let DupireParams {
            base_date,
            spot: S0,
            rate: r,
            div_yield: q,
            strikes,
            times,
            vol_model,
        } = params;
        let mut local_vols = Vec::with_capacity(times.len() * strikes.len());

        for &t in times {
            for &k in strikes {
                if t <= 1e-6 {
                    let vol = implied_vol(k, 1e-6)?;
                    local_vols.push(vol);
                    continue;
                }

                let dk = match vol_model {
                    VolatilityModel::Normal => (0.01 * k.abs()).max(1e-4),
                    VolatilityModel::Black => (0.01 * k.abs()).max(1e-8),
                };
                let dt = (0.01 * t).max(1e-6);

                let var_loc = match vol_model {
                    VolatilityModel::Normal => {
                        dupire_normal_point(&implied_vol, S0, r, q, k, t, dk, dt)?
                    }
                    VolatilityModel::Black => {
                        dupire_lognormal_point(&implied_vol, S0, r, q, k, t, dk, dt)?
                    }
                };

                local_vols.push(var_loc.sqrt());
            }
        }

        // Axis order: times = X (slow), strikes = Y (fast).
        // `local_vols` was filled in `for t { for k { ... } }` order above,
        // so index = i_time * strikes.len() + i_strike — matching BilinearInterp's contract.
        let surface = BilinearInterp::new(times.to_vec(), strikes.to_vec(), local_vols)?;

        Ok(LocalVolSurface::new(base_date, Arc::new(surface)))
    }
}

/// Lognormal (Black-Scholes) Dupire local variance at a single grid point.
///
/// Uses spot-measure discounted call prices and the standard Dupire formula:
/// `sigma_loc^2 = (dC/dT + (r-q)K dC/dK + qC) / (0.5 K^2 d2C/dK2)`
#[allow(non_snake_case, clippy::too_many_arguments)]
fn dupire_lognormal_point(
    implied_vol: &dyn Fn(f64, f64) -> Result<f64>,
    S0: f64,
    r: f64,
    q: f64,
    k: f64,
    t: f64,
    dk: f64,
    dt: f64,
) -> Result<f64> {
    use crate::instruments::common_impl::models::volatility::black::d1_d2;
    use finstack_core::math::norm_cdf;

    let bs_call = |strike: f64, time: f64| -> Result<f64> {
        let sigma = implied_vol(strike, time)?;
        if time <= 0.0 {
            return Ok((S0 - strike).max(0.0));
        }
        let (d1v, d2v) = d1_d2(S0, strike, r, sigma, time, q);
        Ok(S0 * (-q * time).exp() * norm_cdf(d1v) - strike * (-r * time).exp() * norm_cdf(d2v))
    };

    let c_k = bs_call(k, t)?;
    let c_k_plus = bs_call(k + dk, t)?;
    let c_k_minus = bs_call(k - dk, t)?;

    let dC_dK = (c_k_plus - c_k_minus) / (2.0 * dk);
    let d2C_dK2 = (c_k_plus - 2.0 * c_k + c_k_minus) / (dk * dk);

    if d2C_dK2 <= 0.0 {
        tracing::warn!(
            strike = k,
            time = t,
            "d²C/dK² <= 0 (butterfly arbitrage violation): falling back to implied vol"
        );
        let iv = implied_vol(k, t)?;
        return Ok(iv * iv);
    }

    let c_t_plus = bs_call(k, t + dt)?;
    let c_t_minus = if t > dt { bs_call(k, t - dt)? } else { c_k };
    let dC_dT = if t > dt {
        (c_t_plus - c_t_minus) / (2.0 * dt)
    } else {
        (c_t_plus - c_k) / dt
    };

    let numerator = dC_dT + (r - q) * k * dC_dK + q * c_k;
    let denominator = 0.5 * k * k * d2C_dK2;

    if denominator.abs() < 1e-12 {
        let iv = implied_vol(k, t)?;
        return Ok(iv * iv);
    }
    Ok((numerator / denominator).max(0.0))
}

/// Normal (Bachelier) Dupire local variance at a single grid point.
///
/// Works with **undiscounted** Bachelier call prices in the forward measure,
/// holding the forward fixed when perturbing T. This avoids the drift/discounting
/// terms that make the lognormal formula unstable at rates scale.
///
/// Forward-measure normal Dupire:
/// `sigma_N_loc^2 = dC_und/dT / (0.5 * d2C_und/dK2)`
#[allow(non_snake_case, clippy::too_many_arguments)]
fn dupire_normal_point(
    implied_vol: &dyn Fn(f64, f64) -> Result<f64>,
    s0: f64,
    r: f64,
    q: f64,
    k: f64,
    t: f64,
    dk: f64,
    dt: f64,
) -> Result<f64> {
    let forward = s0 * ((r - q) * t).exp();

    let bach_call = |strike: f64, time: f64, fwd: f64| -> Result<f64> {
        let sigma_n = implied_vol(strike, time)?;
        if time <= 0.0 {
            return Ok((fwd - strike).max(0.0));
        }
        Ok(finstack_core::math::volatility::bachelier_call(
            fwd, strike, sigma_n, time,
        ))
    };

    // K derivatives at fixed T and fixed forward
    let c_k = bach_call(k, t, forward)?;
    let c_k_plus = bach_call(k + dk, t, forward)?;
    let c_k_minus = bach_call(k - dk, t, forward)?;

    let d2C_dK2 = (c_k_plus - 2.0 * c_k + c_k_minus) / (dk * dk);

    if d2C_dK2 <= 0.0 {
        tracing::warn!(
            strike = k,
            time = t,
            "d²C/dK² <= 0 (butterfly arbitrage violation): falling back to implied vol"
        );
        let iv = implied_vol(k, t)?;
        return Ok(iv * iv);
    }

    // T derivative at FIXED forward — pure time decay, no drift contamination.
    let c_t_plus = bach_call(k, t + dt, forward)?;
    let c_t_minus = if t > dt {
        bach_call(k, t - dt, forward)?
    } else {
        c_k
    };
    let dC_dT = if t > dt {
        (c_t_plus - c_t_minus) / (2.0 * dt)
    } else {
        (c_t_plus - c_k) / dt
    };

    let denominator = 0.5 * d2C_dK2;

    if denominator.abs() < 1e-12 {
        let iv = implied_vol(k, t)?;
        return Ok(iv * iv);
    }
    Ok((dC_dT / denominator).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_vol_flat_smile() -> Result<()> {
        let const_vol = 0.20;
        let implied_vol_fn = |_: f64, _: f64| Ok(const_vol);

        let base_date =
            Date::from_ordinal_date(2024, 1).expect("Invalid date: 2024-01-01 should be valid");

        let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
        let times = vec![0.5, 1.0, 2.0];

        let lv_surface = LocalVolBuilder::from_implied_vol(
            implied_vol_fn,
            DupireParams {
                base_date,
                spot: 100.0,
                rate: 0.05,
                div_yield: 0.0,
                strikes: &strikes,
                times: &times,
                vol_model: VolatilityModel::Black,
            },
        )?;

        let lv = lv_surface.get_vol(1.0, 100.0)?;

        assert!(
            (lv - const_vol).abs() < 0.01,
            "Local vol {} should match flat implied vol {}",
            lv,
            const_vol
        );

        Ok(())
    }

    #[test]
    fn test_local_vol_rates_scale_normal() -> Result<()> {
        // Bachelier (normal) Dupire for rates-scale data avoids the numerical
        // instability of the lognormal model at small absolute levels.
        let const_vol = 0.005; // 50bp normal vol typical for rates
        let implied_vol_fn = |_: f64, _: f64| Ok(const_vol);

        let base_date =
            Date::from_ordinal_date(2024, 1).expect("Invalid date: 2024-01-01 should be valid");

        let strikes = vec![0.01, 0.02, 0.03, 0.04, 0.05];
        let times = vec![0.5, 1.0, 2.0];

        let lv_surface = LocalVolBuilder::from_implied_vol(
            implied_vol_fn,
            DupireParams {
                base_date,
                spot: 0.03,
                rate: 0.03,
                div_yield: 0.0,
                strikes: &strikes,
                times: &times,
                vol_model: VolatilityModel::Normal,
            },
        )?;

        let lv = lv_surface.get_vol(1.0, 0.03)?;

        let rel_error = (lv / const_vol - 1.0).abs();
        assert!(
            rel_error < 0.05,
            "Normal Dupire local vol {lv:.6} should be within 5% of flat implied vol \
             {const_vol:.6} (relative error: {:.2}%)",
            rel_error * 100.0
        );

        Ok(())
    }
}
