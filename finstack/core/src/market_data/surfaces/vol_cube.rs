//! SABR volatility cube for swaption pricing.
//!
//! Stores SABR parameters on a two-dimensional grid indexed by option expiry and
//! underlying swap tenor. The cube interpolates SABR parameters bilinearly across
//! the grid and evaluates implied volatilities via the Hagan (2002) approximation.
//!
//! # Financial Context
//!
//! Swaption volatility is naturally three-dimensional: the implied vol depends on
//! the option expiry, the underlying swap tenor, and the strike. Rather than
//! storing pre-computed vols on a full 3D grid, the cube stores calibrated SABR
//! parameters at each (expiry, tenor) node and evaluates the smile on the fly.
//! This reduces memory footprint and ensures arbitrage-free strike interpolation
//! within each smile.
//!
//! # Grid Layout
//!
//! Parameters and forwards are stored in **row-major** order:
//! `index = expiry_idx * n_tenors + tenor_idx`.
//!
//! # Interpolation
//!
//! Each SABR parameter (alpha, rho, nu) and the forward rate are interpolated
//! bilinearly between grid nodes. Beta is taken from the nearest node (it is
//! typically fixed across the grid). Shift is bilinear when all four surrounding
//! nodes carry a shift, otherwise nearest-node.
//!
//! After interpolation a post-clamp ensures parameter validity:
//! - alpha > 1e-8
//! - nu > 1e-8
//! - rho in (-0.9999, 0.9999)
//! - beta in [0, 1]

use crate::{
    error::InputError,
    math::interp::utils::{locate_segment, locate_segment_unchecked},
    math::volatility::sabr::SabrParams,
    types::CurveId,
};

use super::vol_surface::{VolInterpolationMode, VolSurface};

// ---------------------------------------------------------------------------
// Core struct
// ---------------------------------------------------------------------------

/// SABR volatility cube on an expiry x tenor grid.
///
/// Each grid node stores a [`SabrParams`] and a forward rate. Interpolation
/// between nodes is bilinear in parameter space; the implied vol at an
/// arbitrary (expiry, tenor, strike) is obtained by interpolating parameters
/// and then evaluating the Hagan approximation.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawVolCube", into = "RawVolCube")]
pub struct VolCube {
    id: CurveId,
    expiries: Box<[f64]>,
    tenors: Box<[f64]>,
    /// Row-major: params[expiry_idx * n_tenors + tenor_idx]
    params: Vec<SabrParams>,
    /// Row-major: forwards[expiry_idx * n_tenors + tenor_idx]
    forwards: Vec<f64>,
    interpolation_mode: VolInterpolationMode,
}

// ---------------------------------------------------------------------------
// Serde intermediate
// ---------------------------------------------------------------------------

/// Raw serializable state of a VolCube.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVolCube {
    pub id: String,
    pub expiries: Vec<f64>,
    pub tenors: Vec<f64>,
    pub params: Vec<SabrParams>,
    pub forwards: Vec<f64>,
    #[serde(default)]
    pub interpolation_mode: VolInterpolationMode,
}

impl From<VolCube> for RawVolCube {
    fn from(cube: VolCube) -> Self {
        RawVolCube {
            id: cube.id.to_string(),
            expiries: cube.expiries.to_vec(),
            tenors: cube.tenors.to_vec(),
            params: cube.params,
            forwards: cube.forwards,
            interpolation_mode: cube.interpolation_mode,
        }
    }
}

impl TryFrom<RawVolCube> for VolCube {
    type Error = crate::Error;

    fn try_from(raw: RawVolCube) -> crate::Result<Self> {
        Ok(VolCube::from_grid(
            &raw.id,
            &raw.expiries,
            &raw.tenors,
            &raw.params,
            &raw.forwards,
        )?
        .with_interpolation_mode(raw.interpolation_mode))
    }
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

/// Validate an axis: non-empty, finite, and strictly increasing if len > 1.
fn validate_axis(axis: &[f64]) -> crate::Result<()> {
    if axis.is_empty() {
        return Err(InputError::TooFewPoints.into());
    }
    if axis.iter().any(|x| !x.is_finite()) {
        return Err(InputError::Invalid.into());
    }
    if axis.len() > 1 {
        crate::math::interp::utils::validate_knots(axis)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// VolCube impl — construction and accessors
// ---------------------------------------------------------------------------

impl VolCube {
    /// Start building a new vol cube with identifier `id`.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> VolCubeBuilder {
        VolCubeBuilder {
            id: id.into(),
            expiries: Vec::new(),
            tenors: Vec::new(),
            params: Vec::new(),
            forwards: Vec::new(),
        }
    }

    /// Construct directly from axes and row-major parameter/forward arrays.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Either axis is empty, non-finite, or not strictly increasing
    /// - `params.len()` or `forwards.len()` does not equal `expiries.len() * tenors.len()`
    /// - Any forward is non-finite
    pub fn from_grid(
        id: impl AsRef<str>,
        expiries: &[f64],
        tenors: &[f64],
        params: &[SabrParams],
        forwards: &[f64],
    ) -> crate::Result<Self> {
        validate_axis(expiries)?;
        validate_axis(tenors)?;
        let n = expiries.len() * tenors.len();
        if params.len() != n || forwards.len() != n {
            return Err(InputError::DimensionMismatch.into());
        }
        if forwards.iter().any(|f| !f.is_finite()) {
            return Err(InputError::Invalid.into());
        }
        Ok(Self {
            id: CurveId::new(id.as_ref()),
            expiries: expiries.to_vec().into_boxed_slice(),
            tenors: tenors.to_vec().into_boxed_slice(),
            params: params.to_vec(),
            forwards: forwards.to_vec(),
            interpolation_mode: VolInterpolationMode::Vol,
        })
    }

    /// Unique identifier.
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Option expiry axis (years).
    pub fn expiries(&self) -> &[f64] {
        &self.expiries
    }

    /// Underlying swap tenor axis (years).
    pub fn tenors(&self) -> &[f64] {
        &self.tenors
    }

    /// Grid shape as `(n_expiries, n_tenors)`.
    pub fn grid_shape(&self) -> (usize, usize) {
        (self.expiries.len(), self.tenors.len())
    }

    /// SABR parameters at grid indices `(exp_idx, tenor_idx)`.
    ///
    /// # Panics
    ///
    /// Panics if indices are out of bounds.
    pub fn params_at(&self, exp_idx: usize, tenor_idx: usize) -> &SabrParams {
        let n_tenors = self.tenors.len();
        &self.params[exp_idx * n_tenors + tenor_idx]
    }

    /// Forward rate at grid indices `(exp_idx, tenor_idx)`.
    ///
    /// # Panics
    ///
    /// Panics if indices are out of bounds.
    pub fn forward_at(&self, exp_idx: usize, tenor_idx: usize) -> f64 {
        let n_tenors = self.tenors.len();
        self.forwards[exp_idx * n_tenors + tenor_idx]
    }

    /// Return a copy with the given interpolation mode.
    #[must_use]
    pub fn with_interpolation_mode(mut self, mode: VolInterpolationMode) -> Self {
        self.interpolation_mode = mode;
        self
    }
}

// ---------------------------------------------------------------------------
// VolCube impl — interpolation and vol evaluation (Task 3)
// ---------------------------------------------------------------------------

impl VolCube {
    /// Standard bilinear interpolation of four corner values.
    #[inline]
    fn bilinear(q00: f64, q10: f64, q01: f64, q11: f64, t: f64, u: f64) -> f64 {
        (1.0 - t) * (1.0 - u) * q00 + t * (1.0 - u) * q10 + (1.0 - t) * u * q01 + t * u * q11
    }

    /// Bilinear interpolation of SABR parameters with clamped extrapolation.
    ///
    /// Returns `(interpolated_params, interpolated_forward, clamped_expiry)`.
    /// Coordinates are clamped to the grid edges when out of bounds.
    fn interpolate_params_clamped(&self, expiry: f64, tenor: f64) -> (SabrParams, f64, f64) {
        let n_tenors = self.tenors.len();

        // Clamp coordinates to grid bounds
        let exp_min = self.expiries[0];
        let exp_max = self.expiries[self.expiries.len() - 1];
        let ten_min = self.tenors[0];
        let ten_max = self.tenors[n_tenors - 1];

        let exp_c = expiry.clamp(exp_min, exp_max);
        let ten_c = tenor.clamp(ten_min, ten_max);

        // Locate segments
        let ie = locate_segment_unchecked(&self.expiries, exp_c);
        let it = locate_segment_unchecked(&self.tenors, ten_c);

        // Handle exact hits and edge cases
        let ie1 = (ie + 1).min(self.expiries.len() - 1);
        let it1 = (it + 1).min(self.tenors.len() - 1);

        let e0 = self.expiries[ie];
        let e1 = self.expiries[ie1];
        let t0 = self.tenors[it];
        let t1 = self.tenors[it1];

        // Interpolation weights
        #[allow(clippy::float_cmp)]
        let t_weight = if e0 == e1 {
            0.0
        } else {
            (exp_c - e0) / (e1 - e0)
        };
        #[allow(clippy::float_cmp)]
        let u_weight = if t0 == t1 {
            0.0
        } else {
            (ten_c - t0) / (t1 - t0)
        };

        // Corner indices
        let idx00 = ie * n_tenors + it;
        let idx10 = ie1 * n_tenors + it;
        let idx01 = ie * n_tenors + it1;
        let idx11 = ie1 * n_tenors + it1;

        let p00 = &self.params[idx00];
        let p10 = &self.params[idx10];
        let p01 = &self.params[idx01];
        let p11 = &self.params[idx11];

        // Bilinear interpolation of each parameter
        let alpha = Self::bilinear(
            p00.alpha, p10.alpha, p01.alpha, p11.alpha, t_weight, u_weight,
        );
        let rho = Self::bilinear(p00.rho, p10.rho, p01.rho, p11.rho, t_weight, u_weight);
        let nu = Self::bilinear(p00.nu, p10.nu, p01.nu, p11.nu, t_weight, u_weight);

        // Beta: constant from nearest node
        let nearest_idx = if t_weight <= 0.5 {
            if u_weight <= 0.5 {
                idx00
            } else {
                idx01
            }
        } else if u_weight <= 0.5 {
            idx10
        } else {
            idx11
        };
        let beta = self.params[nearest_idx].beta;

        // Shift: bilinear if all 4 corners have it, else nearest
        let shift = match (p00.shift, p10.shift, p01.shift, p11.shift) {
            (Some(s00), Some(s10), Some(s01), Some(s11)) => {
                Some(Self::bilinear(s00, s10, s01, s11, t_weight, u_weight))
            }
            _ => self.params[nearest_idx].shift,
        };

        // Forward bilinear
        let fwd = Self::bilinear(
            self.forwards[idx00],
            self.forwards[idx10],
            self.forwards[idx01],
            self.forwards[idx11],
            t_weight,
            u_weight,
        );

        // Post-interpolation clamp
        let alpha = alpha.max(1e-8);
        let nu = nu.max(1e-8);
        let rho = rho.clamp(-0.9999, 0.9999);
        let beta = beta.clamp(0.0, 1.0);

        let mut params = SabrParams {
            alpha,
            beta,
            rho,
            nu,
            shift: None,
        };
        if let Some(s) = shift {
            params = params.with_shift(s);
        }

        (params, fwd, exp_c)
    }

    /// Implied volatility with bounds checking.
    ///
    /// Returns `Err` if `expiry` or `tenor` falls outside the grid, or if the
    /// SABR expansion yields a non-finite volatility (e.g. a χ(z) breakdown).
    /// Guarding here stops a silent `NaN` from poisoning Black-76 pricing or a
    /// compensated aggregation downstream.
    pub fn vol(&self, expiry: f64, tenor: f64, strike: f64) -> crate::Result<f64> {
        // Validate coordinates are within grid bounds
        locate_segment(&self.expiries, expiry)?;
        locate_segment(&self.tenors, tenor)?;

        let (params, fwd, exp_c) = self.interpolate_params_clamped(expiry, tenor);
        params.try_implied_vol_lognormal(fwd, strike, exp_c)
    }

    /// Implied volatility with clamped extrapolation.
    ///
    /// Clamps expiry and tenor to the grid edges before interpolation. Never
    /// panics and never returns a non-finite or non-positive value: a
    /// degenerate expansion is floored to a small positive vol, matching the
    /// `materialize_*` slice helpers.
    pub fn vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> f64 {
        let (params, fwd, exp_c) = self.interpolate_params_clamped(expiry, tenor);
        let v = params.implied_vol_lognormal(fwd, strike, exp_c);
        if v.is_finite() && v > 0.0 {
            v
        } else {
            0.001
        }
    }
}

// ---------------------------------------------------------------------------
// VolCube impl — grid materialization (Task 4)
// ---------------------------------------------------------------------------

impl VolCube {
    /// Materialize a tenor slice as a [`VolSurface`].
    ///
    /// The resulting surface has the cube's expiry axis on one dimension and the
    /// supplied strikes on the other. Each vol is computed by interpolating the
    /// SABR parameters at `(expiry_i, tenor)` and evaluating the smile.
    pub fn materialize_tenor_slice(
        &self,
        tenor: f64,
        strikes: &[f64],
    ) -> crate::Result<VolSurface> {
        if strikes.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let mut vols = Vec::with_capacity(self.expiries.len() * strikes.len());
        for &exp in self.expiries.iter() {
            let (params, fwd, exp_c) = self.interpolate_params_clamped(exp, tenor);
            for &k in strikes {
                let v = params.implied_vol_lognormal(fwd, k, exp_c);
                vols.push(if v.is_finite() && v > 0.0 { v } else { 0.001 });
            }
        }

        VolSurface::from_grid(self.id.as_str(), &self.expiries, strikes, &vols)
    }

    /// Materialize an expiry slice as a [`VolSurface`].
    ///
    /// The resulting surface uses the cube's tenor axis as its "expiry" axis
    /// and the supplied strikes as its strike axis. Each vol is computed by
    /// interpolating the SABR parameters at `(expiry, tenor_j)`.
    pub fn materialize_expiry_slice(
        &self,
        expiry: f64,
        strikes: &[f64],
    ) -> crate::Result<VolSurface> {
        if strikes.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let mut vols = Vec::with_capacity(self.tenors.len() * strikes.len());
        for &tnr in self.tenors.iter() {
            let (params, fwd, exp_c) = self.interpolate_params_clamped(expiry, tnr);
            for &k in strikes {
                let v = params.implied_vol_lognormal(fwd, k, exp_c);
                vols.push(if v.is_finite() && v > 0.0 { v } else { 0.001 });
            }
        }

        VolSurface::from_grid(self.id.as_str(), &self.tenors, strikes, &vols)
    }

    /// Materialize the full grid as a flat vector in `(expiry, tenor, strike)` order.
    ///
    /// The returned vector has length `n_expiries * n_tenors * n_strikes`.
    pub fn materialize_grid(&self, strikes: &[f64]) -> crate::Result<Vec<f64>> {
        if strikes.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let n_exp = self.expiries.len();
        let n_ten = self.tenors.len();
        let n_str = strikes.len();
        let mut out = Vec::with_capacity(n_exp * n_ten * n_str);

        for &exp in self.expiries.iter() {
            for &tnr in self.tenors.iter() {
                let (params, fwd, exp_c) = self.interpolate_params_clamped(exp, tnr);
                for &k in strikes {
                    let v = params.implied_vol_lognormal(fwd, k, exp_c);
                    out.push(if v.is_finite() && v > 0.0 { v } else { 0.001 });
                }
            }
        }

        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// VolProvider impl
// ---------------------------------------------------------------------------

impl crate::market_data::traits::VolProvider for VolCube {
    fn vol(&self, expiry: f64, tenor: f64, strike: f64) -> crate::Result<f64> {
        self.vol(expiry, tenor, strike)
    }
    fn vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> f64 {
        self.vol_clamped(expiry, tenor, strike)
    }
    fn vol_id(&self) -> &CurveId {
        self.id()
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Incremental builder for [`VolCube`].
///
/// Nodes must be added in row-major order: for each expiry, add one node per
/// tenor (in tenor order) before proceeding to the next expiry.
pub struct VolCubeBuilder {
    id: CurveId,
    expiries: Vec<f64>,
    tenors: Vec<f64>,
    params: Vec<SabrParams>,
    forwards: Vec<f64>,
}

impl VolCubeBuilder {
    /// Set the option expiry axis (years).
    pub fn expiries(mut self, exps: &[f64]) -> Self {
        self.expiries.extend_from_slice(exps);
        self
    }

    /// Set the underlying swap tenor axis (years).
    pub fn tenors(mut self, tnrs: &[f64]) -> Self {
        self.tenors.extend_from_slice(tnrs);
        self
    }

    /// Append a grid node (SABR params + forward) in row-major order.
    pub fn node(mut self, params: SabrParams, forward: f64) -> Self {
        self.params.push(params);
        self.forwards.push(forward);
        self
    }

    /// Finalise and validate the cube.
    pub fn build(self) -> crate::Result<VolCube> {
        VolCube::from_grid(
            self.id.as_str(),
            &self.expiries,
            &self.tenors,
            &self.params,
            &self.forwards,
        )
    }
}
