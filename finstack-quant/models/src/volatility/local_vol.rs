//! Dupire local volatility extraction from an implied volatility surface.
//!
//! The Dupire formula (1994) extracts a local (instantaneous) volatility surface
//! from the implied volatility surface of European options. The local
//! volatility model `dS = (r − q) S dt + σ_loc(t, S) S dW` prices every
//! European option consistently with that smile while staying Markovian in the
//! underlying.
//!
//! # Mathematical Foundation
//!
//! With total implied variance `w(k, T) = σ²(k, T)·T` and log-moneyness
//! `k = ln(K / F_T)` (Gatheral 2006, eq. 1.10):
//!
//! ```text
//! σ²_loc(K, T) = (∂w/∂T) / g
//! g = 1 − (k/w)·∂w/∂k + ¼·(−¼ − 1/w + k²/w²)·(∂w/∂k)² + ½·∂²w/∂k²
//! ```
//!
//! The time derivative holds `k` fixed, so rates and dividends enter only
//! through the forwards `F_T`: one forward per expiry carries the whole carry
//! term structure, and no discount curve is needed. `σ_loc(K, T)` is the
//! volatility of the underlying when it trades at level `K` at time `T`.
//!
//! # Implementation Notes
//!
//! - The formula and its finite differences are the ones the arbitrage check
//!   [`LocalVolDensityCheck`](crate::volatility::arbitrage::LocalVolDensityCheck)
//!   evaluates: central differences inside the grid, one-sided at its edges.
//! - The local volatility is stored on the grid of the implied surface and
//!   interpolated bilinearly, with flat extrapolation outside it.
//! - An implied surface with butterfly (`g ≤ 0`) or calendar (`∂w/∂T < 0`)
//!   arbitrage has no local volatility; extraction returns an error naming the
//!   first offending node. Smooth the quotes first
//!   ([`LocalVolSurface::from_implied_vol_smoothed`]) or repair the surface.
//!
//! # Reference
//!
//! - Dupire, B. (1994). "Pricing with a Smile." *Risk*, 7(1), 18-20. `docs/REFERENCES.md#dupire-1994`
//! - Gatheral, J. (2006). *The Volatility Surface: A Practitioner's Guide*.
//!   John Wiley & Sons. Chapters 1-2. `docs/REFERENCES.md#gatheral-volatility-surface`

use crate::volatility::arbitrage::DEFAULT_ARBITRAGE_TOLERANCE;
use crate::volatility::dupire::{dupire_node, MoneynessLookup};
use finstack_quant_core::error::InputError;
use finstack_quant_core::market_data::surfaces::{
    VolGridOpts, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::{Error, Result};

fn validate_implied_surface(surface: &VolSurface, forwards: &[f64]) -> Result<()> {
    surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
    surface.require_quote_type(VolQuoteType::BlackLognormal)?;
    let expiries = surface.expiries();
    let strikes = surface.strikes();
    if expiries.len() < 2 || strikes.len() < 3 {
        return Err(InputError::TooFewPoints.into());
    }
    if forwards.len() != expiries.len() {
        return Err(Error::Validation(format!(
            "local volatility needs one forward per expiry: got {} forwards for {} expiries",
            forwards.len(),
            expiries.len()
        )));
    }
    if forwards.iter().any(|f| !f.is_finite() || *f <= 0.0) {
        return Err(Error::Validation(
            "local volatility forwards must be finite and positive".to_string(),
        ));
    }
    if strikes.iter().any(|k| !k.is_finite() || *k <= 0.0) {
        return Err(Error::Validation(
            "local volatility needs finite positive strikes (log-moneyness is ln(K/F))".to_string(),
        ));
    }
    Ok(())
}

/// Local volatility surface `σ_loc(T, K)` on a rectangular expiry-by-strike
/// grid, extracted from an implied volatility surface via the Dupire formula.
///
/// Off-grid queries are interpolated bilinearly; outside the grid the nearest
/// boundary value applies (flat extrapolation).
///
/// # Invariants
///
/// - Expiries and strikes are finite and strictly increasing; expiries are
///   non-negative and strikes positive.
/// - There is one finite, non-negative local volatility per grid node, stored
///   row-major with the expiry as the slow axis.
///
/// Deserialization enforces the same invariants.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::surfaces::VolSurface;
/// use finstack_quant_models::volatility::local_vol::LocalVolSurface;
///
/// // Implied vol skewed in strike and flat in expiry.
/// let expiries = [0.25, 0.5, 1.0, 2.0];
/// let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
/// let smile = [0.24, 0.22, 0.20, 0.19, 0.185];
/// let mut builder = VolSurface::builder("SKEW").expiries(&expiries).strikes(&strikes);
/// for _ in &expiries {
///     builder = builder.row(&smile);
/// }
/// let surface = builder.build()?;
///
/// // One forward per expiry: F(T) = S·exp((r − q)·T) with S = 100, r − q = 2%.
/// let forwards: Vec<f64> = expiries.iter().map(|t| 100.0 * (0.02 * t).exp()).collect();
/// let local_vol = LocalVolSurface::from_implied_vol(&surface, &forwards)?;
///
/// assert_eq!(local_vol.grid_shape(), (4, 5));
/// // A negative skew is steeper in local than in implied volatility.
/// assert!(local_vol.value(1.0, 90.0) > local_vol.value(1.0, 110.0));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawLocalVolSurface")]
pub struct LocalVolSurface {
    /// Expiry axis in years, strictly increasing.
    expiries: Vec<f64>,
    /// Strike axis in price units of the underlying, strictly increasing.
    strikes: Vec<f64>,
    /// Local volatilities as annualized decimals, row-major:
    /// `local_vols[expiry_index * strikes.len() + strike_index]`.
    local_vols: Vec<f64>,
}

/// Unvalidated wire form of [`LocalVolSurface`].
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawLocalVolSurface {
    /// Expiry axis in years, strictly increasing and non-negative.
    expiries: Vec<f64>,
    /// Strike axis in price units of the underlying, strictly increasing and
    /// positive.
    strikes: Vec<f64>,
    /// Local volatilities as annualized decimals, finite and non-negative,
    /// row-major: `local_vols[expiry_index * strikes.len() + strike_index]`.
    local_vols: Vec<f64>,
}

impl TryFrom<RawLocalVolSurface> for LocalVolSurface {
    type Error = Error;

    fn try_from(raw: RawLocalVolSurface) -> Result<Self> {
        Self::new(raw.expiries, raw.strikes, raw.local_vols)
    }
}

impl LocalVolSurface {
    /// Build a local volatility surface from an explicit grid.
    ///
    /// # Arguments
    ///
    /// * `expiries` - Expiry axis in years: at least one finite, non-negative,
    ///   strictly increasing value.
    /// * `strikes` - Strike axis in price units of the underlying: at least
    ///   one finite, positive, strictly increasing value.
    /// * `local_vols` - Local volatilities as annualized decimals, finite and
    ///   non-negative, one per node in row-major order with the expiry as the
    ///   slow axis (`expiries.len() * strikes.len()` values).
    ///
    /// # Errors
    ///
    /// Returns a validation error when an axis is empty, non-finite or not
    /// strictly increasing, an expiry is negative, a strike is not positive,
    /// the value count does not match the grid, or a volatility is negative
    /// or non-finite.
    pub fn new(expiries: Vec<f64>, strikes: Vec<f64>, local_vols: Vec<f64>) -> Result<Self> {
        let check_axis = |name: &str, axis: &[f64], lower_ok: fn(f64) -> bool| {
            if axis.is_empty()
                || axis.iter().any(|x| !x.is_finite() || !lower_ok(*x))
                || axis.windows(2).any(|pair| pair[1] <= pair[0])
            {
                return Err(Error::Validation(format!(
                    "LocalVolSurface {name} must be non-empty, finite, in range and strictly \
                     increasing"
                )));
            }
            Ok(())
        };
        check_axis("expiries", &expiries, |t| t >= 0.0)?;
        check_axis("strikes", &strikes, |k| k > 0.0)?;
        if expiries.len().checked_mul(strikes.len()) != Some(local_vols.len()) {
            return Err(Error::Validation(format!(
                "LocalVolSurface needs {} x {} local volatilities, got {}",
                expiries.len(),
                strikes.len(),
                local_vols.len()
            )));
        }
        if local_vols.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(Error::Validation(
                "LocalVolSurface local volatilities must be finite and non-negative".to_string(),
            ));
        }
        Ok(Self {
            expiries,
            strikes,
            local_vols,
        })
    }

    /// Extract local volatility from an implied volatility surface using the
    /// Dupire formula in total-variance form.
    ///
    /// At every node of the implied grid the local variance is
    /// `(∂w/∂T) / g` (see the [module documentation](self)), with `∂w/∂T`
    /// taken at fixed log-moneyness `k = ln(K / F_T)`. Where the remapped
    /// strike `F_j·e^k` of a neighbouring expiry falls outside the strike
    /// grid, that expiry is read at the nearest grid strike (a flat implied
    /// wing).
    ///
    /// # Arguments
    ///
    /// * `surface` - Unshifted Black (lognormal) implied volatilities as
    ///   annualized decimals, on expiries in years and cash strikes in price
    ///   units of the underlying; at least two expiries and three positive
    ///   strikes.
    /// * `forwards` - Forward price of the underlying for each surface
    ///   expiry, in the units of the strikes and in the order of
    ///   `surface.expiries()`; finite and positive. For a flat carry,
    ///   `F_T = S·exp((r − q)·T)`.
    ///
    /// # Returns
    ///
    /// The local volatility on the grid of `surface`.
    ///
    /// # Errors
    ///
    /// Returns an input error for fewer than two expiries or three strikes,
    /// and a validation error when the surface does not use a strike axis or
    /// holds normal or displaced quotes, `forwards` has the wrong length or a
    /// non-finite or non-positive entry, a strike is not positive, or the
    /// surface is not arbitrage-free at a node: non-positive implied
    /// variance, non-positive Dupire density `g` (butterfly arbitrage), or
    /// total variance decreasing in expiry at fixed log-moneyness (calendar
    /// arbitrage). The message names the node.
    pub fn from_implied_vol(surface: &VolSurface, forwards: &[f64]) -> Result<Self> {
        validate_implied_surface(surface, forwards)?;
        let expiries = surface.expiries().to_vec();
        let strikes = surface.strikes().to_vec();

        let mut local_vols = Vec::with_capacity(expiries.len() * strikes.len());
        for (ei, &t) in expiries.iter().enumerate() {
            for (si, &strike) in strikes.iter().enumerate() {
                let arbitrage = |what: String| {
                    Error::Validation(format!(
                        "cannot extract local volatility at expiry {t}, strike {strike}: {what}"
                    ))
                };
                let node = dupire_node(surface, forwards, ei, si, MoneynessLookup::Clamped)
                    .ok_or_else(|| arbitrage("implied total variance is not positive".into()))?;
                let dw_dt = node
                    .dw_dt
                    .ok_or_else(|| arbitrage("neighbouring expiries coincide".into()))?;
                if !(node.density.is_finite() && node.density > 0.0) {
                    return Err(arbitrage(format!(
                        "the Dupire density is {:e}, not positive (butterfly arbitrage)",
                        node.density
                    )));
                }
                if dw_dt.is_nan() || dw_dt < -DEFAULT_ARBITRAGE_TOLERANCE {
                    return Err(arbitrage(format!(
                        "total variance falls with expiry at fixed log-moneyness, slope {dw_dt:e} \
                         (calendar arbitrage)"
                    )));
                }
                // A slope within tolerance of zero is a flat total variance.
                let local_vol = (dw_dt.max(0.0) / node.density).sqrt();
                if !local_vol.is_finite() {
                    return Err(arbitrage("the local variance is not finite".into()));
                }
                local_vols.push(local_vol);
            }
        }

        Self::new(expiries, strikes, local_vols)
    }

    /// Extract local volatility after Gaussian smoothing of the implied
    /// volatilities along the strike axis.
    ///
    /// Each implied volatility is replaced by the Gaussian-kernel weighted
    /// average of its expiry row before [`Self::from_implied_vol`] runs. This
    /// regularises the second strike derivative, which is noisy on
    /// market-calibrated grids and is the usual source of a non-positive
    /// Dupire density.
    ///
    /// # Arguments
    ///
    /// * `surface` - Unshifted Black implied volatilities, as for
    ///   [`Self::from_implied_vol`]; other quote conventions and axes are
    ///   rejected before smoothing.
    /// * `forwards` - Forward price of the underlying for each surface expiry,
    ///   as for [`Self::from_implied_vol`].
    /// * `sigma_strikes` - Standard deviation of the Gaussian kernel in strike
    ///   (price) units, non-negative. Zero disables smoothing and equals
    ///   [`Self::from_implied_vol`].
    ///
    /// # Errors
    ///
    /// Returns a validation error if `sigma_strikes` is negative or not
    /// finite, and every error of [`Self::from_implied_vol`], evaluated on
    /// the smoothed surface.
    pub fn from_implied_vol_smoothed(
        surface: &VolSurface,
        forwards: &[f64],
        sigma_strikes: f64,
    ) -> Result<Self> {
        validate_implied_surface(surface, forwards)?;
        if !(sigma_strikes.is_finite() && sigma_strikes >= 0.0) {
            return Err(Error::Validation(
                "sigma_strikes must be finite and non-negative".to_string(),
            ));
        }
        if sigma_strikes < 1e-14 {
            return Self::from_implied_vol(surface, forwards);
        }

        let expiries = surface.expiries();
        let strikes = surface.strikes();
        let mut smoothed_vols = Vec::with_capacity(expiries.len() * strikes.len());
        for &t in expiries {
            let raw: Vec<f64> = strikes
                .iter()
                .map(|&k| crate::volatility::get_surface_vol_clamped(surface, t, k))
                .collect();

            for &k_center in strikes {
                let mut weight_sum = 0.0;
                let mut value_sum = 0.0;
                for (&k_j, &vol) in strikes.iter().zip(&raw) {
                    let d = (k_j - k_center) / sigma_strikes;
                    let w = (-0.5 * d * d).exp();
                    weight_sum += w;
                    value_sum += w * vol;
                }
                smoothed_vols.push(value_sum / weight_sum);
            }
        }

        // Smoothing changes values only, preserving the full artifact contract
        // before the canonical extractor validates it again.
        let smoothed_surface = VolSurface::from_grid_opts(
            surface.id().as_str(),
            expiries,
            strikes,
            &smoothed_vols,
            VolGridOpts {
                secondary_axis: surface.secondary_axis(),
                quote_type: surface.quote_type(),
                interpolation_mode: surface.interpolation_mode(),
            },
            surface.get_displacements(),
        )?;

        Self::from_implied_vol(&smoothed_surface, forwards)
    }

    /// Evaluate the local volatility at a given (expiry, strike) point.
    ///
    /// Uses bilinear interpolation on the local vol grid. For coordinates outside
    /// the grid, clamps to the nearest boundary value (flat extrapolation).
    ///
    /// # Arguments
    ///
    /// * `expiry` - Time in years from the surface's valuation date.
    /// * `strike` - Level of the underlying in price units; in a simulation,
    ///   the spot at that time.
    ///
    /// # Returns
    ///
    /// Local volatility `σ_loc(T, K)` as an annualized decimal; `NaN` if
    /// either coordinate is `NaN`.
    pub fn value(&self, expiry: f64, strike: f64) -> f64 {
        let n_exp = self.expiries.len();
        let n_str = self.strikes.len();

        // Both axes are non-empty by construction.
        let t = expiry.clamp(self.expiries[0], self.expiries[n_exp - 1]);
        let k = strike.clamp(self.strikes[0], self.strikes[n_str - 1]);

        let ei = find_segment(&self.expiries, t);
        let si = find_segment(&self.strikes, k);

        // Handle exact hits — intentional exact comparison against grid values.
        #[allow(clippy::float_cmp)]
        let exact_e = self.expiries[ei] == t;
        #[allow(clippy::float_cmp)]
        let exact_s = self.strikes[si] == k;

        if exact_e && exact_s {
            return self.local_vols[ei * n_str + si];
        }

        let ei1 = if exact_e { ei } else { (ei + 1).min(n_exp - 1) };
        let si1 = if exact_s { si } else { (si + 1).min(n_str - 1) };

        let e0 = self.expiries[ei];
        let e1 = self.expiries[ei1];
        let s0 = self.strikes[si];
        let s1 = self.strikes[si1];

        let q11 = self.local_vols[ei * n_str + si];
        let q21 = self.local_vols[ei1 * n_str + si];
        let q12 = self.local_vols[ei * n_str + si1];
        let q22 = self.local_vols[ei1 * n_str + si1];

        let u = if exact_e || (e1 - e0).abs() < 1e-14 {
            0.0
        } else {
            (t - e0) / (e1 - e0)
        };
        let v = if exact_s || (s1 - s0).abs() < 1e-14 {
            0.0
        } else {
            (k - s0) / (s1 - s0)
        };

        (1.0 - u) * (1.0 - v) * q11 + u * (1.0 - v) * q21 + (1.0 - u) * v * q12 + u * v * q22
    }

    /// Returns the expiry axis in years.
    pub fn expiries(&self) -> &[f64] {
        &self.expiries
    }

    /// Returns the strike axis in price units of the underlying.
    pub fn strikes(&self) -> &[f64] {
        &self.strikes
    }

    /// Returns the local volatilities as annualized decimals in row-major
    /// order: node (`expiry_index`, `strike_index`) is at
    /// `expiry_index * strikes().len() + strike_index`.
    pub fn local_vols(&self) -> &[f64] {
        &self.local_vols
    }

    /// Grid shape as (n_expiries, n_strikes).
    pub fn grid_shape(&self) -> (usize, usize) {
        (self.expiries.len(), self.strikes.len())
    }
}

/// Find the segment index for a value in a sorted array.
/// Returns i such that arr[i] <= x < arr[i+1], or 0 / len-2 at boundaries.
fn find_segment(arr: &[f64], x: f64) -> usize {
    if arr.len() <= 1 {
        return 0;
    }
    if x <= arr[0] {
        return 0;
    }
    if x >= arr[arr.len() - 1] {
        return arr.len() - 2;
    }
    // Binary search: find largest i such that arr[i] <= x
    let pos = arr.partition_point(|&v| v <= x);
    pos.saturating_sub(1).min(arr.len() - 2)
}

/// An arbitrage-free implied surface with a term structure of skew, shared by
/// the local-volatility tests of the extractor, the Monte Carlo process and
/// the PDE.
#[cfg(test)]
pub(crate) mod test_support {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    /// SSVI surface `w(k, θ) = θ/2·(1 + ρφk + √((φk + ρ)² + 1 − ρ²))` with
    /// `θ = σ₀²·T` and `φ = η/√θ` (Gatheral & Jacquier 2014, free of static
    /// arbitrage for `η(1 + |ρ|) ≤ 2`).
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct Ssvi {
        /// At-the-money volatility level.
        pub(crate) sigma0: f64,
        /// Spot-volatility correlation; negative gives an equity skew.
        pub(crate) rho: f64,
        /// Curvature scale.
        pub(crate) eta: f64,
    }

    /// The surface every local-volatility repricing test uses: 20% ATM
    /// volatility with an at-the-money skew of about `-0.15/√T` per unit of
    /// log-moneyness.
    pub(crate) const SKEWED: Ssvi = Ssvi {
        sigma0: 0.2,
        rho: -0.5,
        eta: 0.6,
    };

    impl Ssvi {
        /// Total variance and its exact derivatives
        /// `(w, ∂w/∂T, ∂w/∂k, ∂²w/∂k²)` at log-moneyness `k` and expiry `t`.
        pub(crate) fn total_variance(&self, k: f64, t: f64) -> (f64, f64, f64, f64) {
            let theta = self.sigma0 * self.sigma0 * t;
            let phi = self.eta / theta.sqrt();
            let root = ((phi * k + self.rho).powi(2) + 1.0 - self.rho * self.rho).sqrt();
            let w = 0.5 * theta * (1.0 + self.rho * phi * k + root);
            let dw_dk = 0.5 * theta * phi * (self.rho + (phi * k + self.rho) / root);
            let d2w_dk2 = 0.5 * theta * phi * phi * (1.0 - self.rho * self.rho) / root.powi(3);
            // φ'(θ) = −φ/(2θ).
            let dw_dtheta = 0.5 * (1.0 + self.rho * phi * k + root)
                - 0.25 * phi * k * (self.rho + (phi * k + self.rho) / root);
            (w, self.sigma0 * self.sigma0 * dw_dtheta, dw_dk, d2w_dk2)
        }

        /// Black implied volatility at log-moneyness `k` and expiry `t`.
        pub(crate) fn implied_vol(&self, k: f64, t: f64) -> f64 {
            (self.total_variance(k, t).0 / t).sqrt()
        }

        /// Exact Dupire local volatility at log-moneyness `k` and expiry `t`.
        pub(crate) fn local_vol(&self, k: f64, t: f64) -> f64 {
            let (w, dw_dt, dw_dk, d2w_dk2) = self.total_variance(k, t);
            let density = 1.0 - k / w * dw_dk
                + 0.25 * (-0.25 - 1.0 / w + k * k / (w * w)) * dw_dk * dw_dk
                + 0.5 * d2w_dk2;
            (dw_dt / density).sqrt()
        }

        /// Implied surface sampled on `expiries` x `strikes`, with
        /// `forwards[i]` the forward of `expiries[i]`.
        pub(crate) fn surface(
            &self,
            expiries: &[f64],
            strikes: &[f64],
            forwards: &[f64],
        ) -> VolSurface {
            let mut vols = Vec::with_capacity(expiries.len() * strikes.len());
            for (&t, &forward) in expiries.iter().zip(forwards) {
                for &strike in strikes {
                    vols.push(self.implied_vol((strike / forward).ln(), t));
                }
            }
            VolSurface::from_grid("SSVI", expiries, strikes, &vols).expect("SSVI surface builds")
        }
    }

    /// `count` equally spaced values from `start` to `end` inclusive.
    pub(crate) fn linspace(start: f64, end: f64, count: usize) -> Vec<f64> {
        (0..count)
            .map(|i| start + (end - start) * i as f64 / (count - 1) as f64)
            .collect()
    }

    /// Forwards `spot·exp(carry·T)` for each expiry.
    pub(crate) fn forwards(spot: f64, carry: f64, expiries: &[f64]) -> Vec<f64> {
        expiries.iter().map(|t| spot * (carry * t).exp()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{forwards, linspace, SKEWED};
    use super::*;
    use crate::volatility::arbitrage::{check_surface, ArbitrageCheckConfig};

    const EXPIRIES: [f64; 4] = [0.25, 0.5, 1.0, 2.0];
    const STRIKES: [f64; 7] = [70.0, 80.0, 90.0, 100.0, 110.0, 120.0, 130.0];

    /// Coarse arbitrage-free skewed surface under a 2% carry.
    fn test_surface() -> (VolSurface, Vec<f64>) {
        let forwards = forwards(100.0, 0.02, &EXPIRIES);
        (SKEWED.surface(&EXPIRIES, &STRIKES, &forwards), forwards)
    }

    fn flat_surface(expiries: &[f64], strikes: &[f64], vol: f64) -> VolSurface {
        VolSurface::from_grid(
            "FLAT",
            expiries,
            strikes,
            &vec![vol; expiries.len() * strikes.len()],
        )
        .expect("flat surface should build")
    }

    #[test]
    fn test_surface_passes_the_arbitrage_checks() {
        let (surface, forwards) = test_surface();
        let report = check_surface(
            &surface,
            &ArbitrageCheckConfig {
                forward_prices: Some(forwards),
                ..ArbitrageCheckConfig::default()
            },
        )
        .expect("arbitrage check runs");
        assert!(report.passed, "{:?}", report.violations);
    }

    #[test]
    fn local_vol_rejects_unsupported_conventions_before_smoothing() {
        let (base, forwards) = test_surface();
        let shifted = base.clone().with_displacements(&[20.0; 4]).unwrap();
        let normal = base.clone().with_quote_type(VolQuoteType::Normal).unwrap();
        let tenor_grid = base.with_secondary_axis(VolSurfaceAxis::Tenor);
        for surface in [shifted, normal, tenor_grid] {
            let expected_error = LocalVolSurface::from_implied_vol(&surface, &forwards)
                .expect_err("unshifted Black extraction must reject incompatible metadata")
                .to_string();
            for width in [0.0, 7.5] {
                let smoothed_error =
                    LocalVolSurface::from_implied_vol_smoothed(&surface, &forwards, width)
                        .expect_err("smoothing must preserve and enforce input convention")
                        .to_string();
                assert_eq!(smoothed_error, expected_error);
            }
        }
    }

    #[test]
    fn local_vol_rejects_bad_forwards_and_thin_grids() {
        let (surface, forwards) = test_surface();
        for bad in [
            vec![100.0],
            vec![100.0, 100.0, 100.0, f64::NAN],
            vec![100.0, 100.0, 0.0, 100.0],
            vec![100.0, -1.0, 100.0, 100.0],
        ] {
            let error = LocalVolSurface::from_implied_vol(&surface, &bad)
                .expect_err("invalid forwards must be rejected");
            assert!(error.to_string().contains("forward"), "{error}");
        }

        let one_expiry = flat_surface(&[1.0], &[80.0, 90.0, 100.0, 110.0, 120.0], 0.2);
        assert!(LocalVolSurface::from_implied_vol(&one_expiry, &[100.0]).is_err());
        let two_strikes = flat_surface(&[1.0, 2.0], &[90.0, 110.0], 0.2);
        assert!(LocalVolSurface::from_implied_vol(&two_strikes, &[100.0, 100.0]).is_err());
        assert!(LocalVolSurface::from_implied_vol(&surface, &forwards).is_ok());
    }

    /// Pins the row-major (expiry = slow axis, strike = fast axis) storage
    /// contract on an asymmetric grid, so a transposed index would be caught
    /// rather than silently returning a neighbouring cell's volatility.
    #[test]
    fn local_vol_grid_is_row_major_expiry_slow_strike_fast() {
        let (surface, forwards) = test_surface();
        let lv = LocalVolSurface::from_implied_vol(&surface, &forwards)
            .expect("extraction should succeed");

        // 4 expiries x 7 strikes: a transposition would report (7, 4).
        assert_eq!(lv.grid_shape(), (4, 7));
        assert_eq!(lv.expiries(), EXPIRIES);
        assert_eq!(lv.strikes(), STRIKES);
        assert_eq!(lv.local_vols().len(), 28);
        assert_eq!(lv.value(EXPIRIES[1], STRIKES[5]), lv.local_vols()[7 + 5]);

        // Query with the axes swapped: 0.5 is a valid expiry and 100.0 a valid
        // strike, but 100.0 is far outside the expiry axis and 0.5 far below
        // the strike axis, so both clamp to the (max expiry, min strike)
        // corner. If the axes were transposed internally these would agree.
        let in_order = lv.value(0.5, 100.0);
        let swapped = lv.value(100.0, 0.5);
        assert!(in_order.is_finite() && swapped.is_finite());
        assert!(
            (in_order - swapped).abs() > 1e-9,
            "expiry/strike axes appear interchangeable: in_order={in_order}, swapped={swapped}"
        );
    }

    #[test]
    fn local_vol_smoothed_rejects_negative_sigma() {
        let (surface, forwards) = test_surface();
        for width in [-1.0, f64::NAN] {
            let err = LocalVolSurface::from_implied_vol_smoothed(&surface, &forwards, width)
                .expect_err("invalid smoothing width should be rejected");
            assert!(
                err.to_string().contains("sigma_strikes"),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn local_vol_smoothed_zero_sigma_matches_unsmoothed() {
        let (surface, forwards) = test_surface();
        let unsmoothed = LocalVolSurface::from_implied_vol(&surface, &forwards)
            .expect("extraction should succeed");
        let smoothed = LocalVolSurface::from_implied_vol_smoothed(&surface, &forwards, 0.0)
            .expect("zero smoothing should delegate to unsmoothed extractor");
        assert_eq!(smoothed, unsmoothed);
    }

    #[test]
    fn local_vol_smoothed_positive_sigma_flattens_the_skew() {
        let (surface, forwards) = test_surface();
        let raw = LocalVolSurface::from_implied_vol(&surface, &forwards).expect("raw");
        let lv = LocalVolSurface::from_implied_vol_smoothed(&surface, &forwards, 15.0)
            .expect("positive smoothing should succeed");

        assert!(lv.local_vols().iter().all(|v| *v > 0.0 && v.is_finite()));
        let skew = |surface: &LocalVolSurface| surface.value(1.0, 80.0) - surface.value(1.0, 120.0);
        assert!(skew(&raw) > 0.0);
        assert!(
            skew(&lv) < skew(&raw),
            "smoothing should flatten the skew: {} vs {}",
            skew(&lv),
            skew(&raw)
        );
    }

    #[test]
    fn local_vol_interpolates_bilinearly_and_extrapolates_flat() {
        let lv = LocalVolSurface::new(
            vec![1.0, 2.0],
            vec![100.0, 200.0],
            vec![0.10, 0.20, 0.30, 0.40],
        )
        .expect("valid grid");

        assert!((lv.value(1.5, 150.0) - 0.25).abs() < 1e-15);
        assert!((lv.value(1.0, 125.0) - 0.125).abs() < 1e-15);
        assert!((lv.value(1.25, 200.0) - 0.25).abs() < 1e-15);
        // Flat outside the grid, on each side of each axis.
        assert_eq!(lv.value(0.0, 50.0), 0.10);
        assert_eq!(lv.value(5.0, 50.0), 0.30);
        assert_eq!(lv.value(0.5, 1e6), 0.20);
        assert_eq!(lv.value(9.0, 1e6), 0.40);
        assert!(lv.value(f64::NAN, 100.0).is_nan());

        let single = LocalVolSurface::new(vec![1.0], vec![100.0], vec![0.3]).expect("one node");
        assert_eq!(single.value(0.2, 80.0), 0.3);
        assert_eq!(single.value(3.0, 120.0), 0.3);
    }

    #[test]
    fn local_vol_new_and_serde_enforce_the_grid_invariants() {
        for (expiries, strikes, vols) in [
            (vec![], vec![100.0], vec![]),
            (vec![1.0], vec![], vec![]),
            (vec![1.0, 1.0], vec![100.0], vec![0.2, 0.2]),
            (vec![2.0, 1.0], vec![100.0], vec![0.2, 0.2]),
            (vec![-1.0, 1.0], vec![100.0], vec![0.2, 0.2]),
            (vec![1.0], vec![0.0, 100.0], vec![0.2, 0.2]),
            (vec![1.0], vec![100.0, f64::INFINITY], vec![0.2, 0.2]),
            (vec![1.0], vec![100.0, 110.0], vec![0.2]),
            (vec![1.0], vec![100.0, 110.0], vec![0.2, -0.1]),
            (vec![1.0], vec![100.0, 110.0], vec![0.2, f64::NAN]),
        ] {
            assert!(
                LocalVolSurface::new(expiries.clone(), strikes.clone(), vols.clone()).is_err(),
                "({expiries:?}, {strikes:?}, {vols:?}) must be rejected"
            );
        }

        let (surface, forwards) = test_surface();
        let lv = LocalVolSurface::from_implied_vol(&surface, &forwards).expect("extraction");
        let json = serde_json::to_string(&lv).expect("serialize");
        let restored: LocalVolSurface = serde_json::from_str(&json).expect("round trip");
        assert_eq!(restored, lv);

        for bad in [
            r#"{"expiries":[1.0],"strikes":[100.0],"local_vols":[0.2],"extra":1}"#,
            r#"{"expiries":[1.0],"strikes":[100.0],"local_vols":[-0.2]}"#,
            r#"{"expiries":[1.0],"strikes":[100.0],"local_vols":[]}"#,
            r#"{"expiries":[],"strikes":[],"local_vols":[]}"#,
        ] {
            assert!(
                serde_json::from_str::<LocalVolSurface>(bad).is_err(),
                "{bad}"
            );
        }
    }

    /// A flat implied volatility is its own local volatility: `w = σ²T` has
    /// `∂w/∂T = σ²`, no strike dependence and density one, whatever the
    /// forwards are. Forward differences of a linear function are exact up to
    /// rounding, so the tolerance is tight.
    #[test]
    fn local_vol_of_a_flat_surface_is_the_implied_vol() {
        let expiries = linspace(0.25, 2.0, 8);
        let strikes = linspace(60.0, 160.0, 21);
        let flat = flat_surface(&expiries, &strikes, 0.20);
        for carry in [0.0, 0.03, -0.02] {
            let lv = LocalVolSurface::from_implied_vol(&flat, &forwards(100.0, carry, &expiries))
                .expect("extraction should succeed");
            for (index, vol) in lv.local_vols().iter().enumerate() {
                assert!(
                    (vol - 0.20).abs() < 1e-12,
                    "carry {carry}, node {index}: local vol {vol}"
                );
            }
            assert!((lv.value(0.77, 93.0) - 0.20).abs() < 1e-12);
        }
    }

    /// Against the exact SSVI local volatility on a fine grid with sloped
    /// forwards. Interior nodes use second-order central differences.
    #[test]
    fn local_vol_matches_the_exact_ssvi_local_vol() {
        let expiries = linspace(0.1, 2.0, 77);
        let strikes = linspace(50.0, 170.0, 241);
        let forwards = forwards(100.0, 0.02, &expiries);
        let surface = SKEWED.surface(&expiries, &strikes, &forwards);
        let lv = LocalVolSurface::from_implied_vol(&surface, &forwards).expect("extraction");

        let mut worst: f64 = 0.0;
        for (ei, &t) in expiries.iter().enumerate() {
            // Skip the edge rows and columns, which use one-sided differences.
            if ei == 0 || ei == expiries.len() - 1 {
                continue;
            }
            for &strike in &strikes[1..strikes.len() - 1] {
                // Within about three standard deviations of the forward.
                let k = (strike / forwards[ei]).ln();
                if k.abs() > 3.0 * 0.2 * t.sqrt() {
                    continue;
                }
                let exact = SKEWED.local_vol(k, t);
                worst = worst.max((lv.value(t, strike) - exact).abs());
            }
        }
        // Measured 3.7e-4 (under four basis points of volatility), at the short end.
        assert!(worst < 5e-4, "worst local-vol error {worst}");
    }

    /// Per-expiry forwards matter: holding the same cash-strike quotes but
    /// declaring a carry changes the fixed-moneyness time derivative.
    #[test]
    fn local_vol_depends_on_the_forward_term_structure() {
        let (surface, sloped) = test_surface();
        let with_carry = LocalVolSurface::from_implied_vol(&surface, &sloped).expect("carry");
        let no_carry = LocalVolSurface::from_implied_vol(&surface, &[100.0; 4]).expect("flat");
        assert!((with_carry.value(1.0, 100.0) - no_carry.value(1.0, 100.0)).abs() > 1e-4);
    }

    /// The surface of the type-level doc example.
    #[test]
    fn local_vol_of_a_cash_strike_skew_is_steeper_than_the_implied_skew() {
        let smile = [0.24, 0.22, 0.20, 0.19, 0.185];
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
        let mut builder = VolSurface::builder("SKEW")
            .expiries(&EXPIRIES)
            .strikes(&strikes);
        for _ in &EXPIRIES {
            builder = builder.row(&smile);
        }
        let surface = builder.build().expect("surface should build");
        let lv = LocalVolSurface::from_implied_vol(&surface, &forwards(100.0, 0.02, &EXPIRIES))
            .expect("extraction should succeed");
        assert_eq!(lv.grid_shape(), (4, 5));
        assert!(lv.value(1.0, 90.0) - lv.value(1.0, 110.0) > 0.22 - 0.19);

        // The fixture of the Python and WASM tests and of their parity case.
        let host_forwards = [100.5, 101.0, 102.0, 104.0];
        let host = LocalVolSurface::from_implied_vol(&surface, &host_forwards)
            .expect("host fixture extracts");
        assert!(host.value(1.0, 90.0) - host.value(1.0, 110.0) > 0.22 - 0.19);
        let smoothed = LocalVolSurface::from_implied_vol_smoothed(&surface, &host_forwards, 10.0)
            .expect("smoothed host fixture extracts");
        assert!(
            smoothed.value(1.0, 80.0) - smoothed.value(1.0, 120.0)
                < host.value(1.0, 80.0) - host.value(1.0, 120.0)
        );
    }

    #[test]
    fn local_vol_rejects_calendar_arbitrage_naming_the_node() {
        let surface = VolSurface::builder("CALENDAR-BAD")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[0.25, 0.25, 0.25, 0.25, 0.25])
            .row(&[0.25, 0.25, 0.25, 0.25, 0.25])
            .row(&[0.15, 0.15, 0.15, 0.15, 0.15])
            .build()
            .expect("surface should build");
        let error = LocalVolSurface::from_implied_vol(&surface, &[100.0; 3])
            .expect_err(
                "total variance falls from T=1 to T=2, seen by the backward difference at T=2",
            )
            .to_string();
        assert!(
            error.contains("calendar arbitrage") && error.contains("expiry 2, strike 80"),
            "{error}"
        );
    }

    #[test]
    fn local_vol_rejects_butterfly_arbitrage_naming_the_node() {
        let surface = VolSurface::builder("BUTTERFLY-BAD")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[70.0, 85.0, 100.0, 115.0, 130.0])
            .row(&[0.20, 0.45, 0.20, 0.45, 0.20])
            .row(&[0.21, 0.46, 0.21, 0.46, 0.21])
            .row(&[0.22, 0.47, 0.22, 0.47, 0.22])
            .build()
            .expect("surface should build");
        let error = LocalVolSurface::from_implied_vol(&surface, &[100.0; 3])
            .expect_err("a concave smile has negative density")
            .to_string();
        assert!(
            error.contains("butterfly arbitrage") && error.contains("expiry 0.5"),
            "{error}"
        );
        // Heavy smoothing removes the oscillation and the surface extracts.
        LocalVolSurface::from_implied_vol_smoothed(&surface, &[100.0; 3], 60.0)
            .expect("smoothed surface is arbitrage-free");
    }
}
