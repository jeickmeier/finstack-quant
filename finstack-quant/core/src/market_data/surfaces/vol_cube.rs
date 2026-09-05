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
//!
//! # Quoting Conventions
//!
//! Core stores parameter nodes, forward nodes, axes, and interpolation
//! metadata. SABR evaluation and slice materialization live in
//! `finstack-quant-models`.

use crate::{error::InputError, types::CurveId};

use super::vol_surface::VolInterpolationMode;
use super::SabrParameterData;

/// SABR volatility cube on an expiry x tenor grid.
///
/// Each grid node stores a [`SabrParameterData`] and a forward rate. The
/// interpolation mode records how a models-layer evaluator should combine
/// nodes; core performs structural validation only.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "VolCubeWire", into = "VolCubeWire")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "VolCubeWire"))]
pub struct VolCube {
    id: CurveId,
    expiries: Box<[f64]>,
    tenors: Box<[f64]>,
    /// Row-major: params[expiry_idx * n_tenors + tenor_idx]
    params: Vec<SabrParameterData>,
    /// Row-major: forwards[expiry_idx * n_tenors + tenor_idx]
    forwards: Vec<f64>,
    interpolation_mode: VolInterpolationMode,
}

/// Raw serializable state of a VolCube.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct VolCubeWire {
    /// Identifier the cube is registered and looked up under.
    pub id: String,
    /// Option expiries in years, strictly increasing. Indexes the first axis.
    pub expiries: Vec<f64>,
    /// Underlying swap tenors in years, strictly increasing. Indexes the
    /// second axis.
    pub tenors: Vec<f64>,
    /// Row-major: params[expiry_idx * n_tenors + tenor_idx]
    pub params: Vec<SabrParameterData>,
    /// Row-major: forwards[expiry_idx * n_tenors + tenor_idx]
    pub forwards: Vec<f64>,
    /// How volatilities are interpolated between the cube's grid points.
    pub interpolation_mode: VolInterpolationMode,
}

impl From<VolCube> for VolCubeWire {
    fn from(cube: VolCube) -> Self {
        VolCubeWire {
            id: cube.id.to_string(),
            expiries: cube.expiries.to_vec(),
            tenors: cube.tenors.to_vec(),
            params: cube.params,
            forwards: cube.forwards,
            interpolation_mode: cube.interpolation_mode,
        }
    }
}

impl TryFrom<VolCubeWire> for VolCube {
    type Error = crate::Error;

    fn try_from(raw: VolCubeWire) -> crate::Result<Self> {
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

// VolCube impl — construction and accessors

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
    ///
    /// # Arguments
    ///
    /// * `id` - Market-data identifier stored on the cube and used for context lookup.
    /// * `expiries` - Strictly increasing option expiries in year-fraction units.
    /// * `tenors` - Strictly ordered tenor coordinates in year-fraction units.
    /// * `params` - SABR nodes in expiry-major, tenor-minor row order, one per grid point.
    /// * `forwards` - Forward levels aligned one-for-one with the corresponding grid coordinates.
    pub fn from_grid(
        id: impl AsRef<str>,
        expiries: &[f64],
        tenors: &[f64],
        params: &[SabrParameterData],
        forwards: &[f64],
    ) -> crate::Result<Self> {
        validate_axis(expiries)?;
        validate_axis(tenors)?;
        if expiries.iter().any(|&value| value <= 0.0) || tenors.iter().any(|&value| value <= 0.0) {
            return Err(InputError::NonPositiveValue.into());
        }
        let n = expiries.len() * tenors.len();
        if params.len() != n || forwards.len() != n {
            return Err(InputError::DimensionMismatch.into());
        }
        if forwards.iter().any(|f| !f.is_finite()) {
            return Err(InputError::Invalid.into());
        }
        for params in params {
            SabrParameterData::new_with_shift(
                params.alpha,
                params.beta,
                params.rho,
                params.nu,
                params.shift,
            )?;
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
    pub fn params_at(&self, exp_idx: usize, tenor_idx: usize) -> &SabrParameterData {
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

    /// Return the row-major SABR parameter nodes.
    pub fn params(&self) -> &[SabrParameterData] {
        &self.params
    }

    /// Return the row-major forward nodes.
    pub fn forwards(&self) -> &[f64] {
        &self.forwards
    }

    /// Interpolation contract used between expiry pillars.
    #[must_use]
    pub fn interpolation_mode(&self) -> VolInterpolationMode {
        self.interpolation_mode
    }

    /// Return a copy with the given interpolation mode.
    #[must_use]
    pub fn with_interpolation_mode(mut self, mode: VolInterpolationMode) -> Self {
        self.interpolation_mode = mode;
        self
    }
}

/// Incremental builder for [`VolCube`].
///
/// Nodes must be added in row-major order: for each expiry, add one node per
/// tenor (in tenor order) before proceeding to the next expiry.
pub struct VolCubeBuilder {
    id: CurveId,
    expiries: Vec<f64>,
    tenors: Vec<f64>,
    params: Vec<SabrParameterData>,
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
    pub fn node(mut self, params: SabrParameterData, forward: f64) -> Self {
        self.params.push(params);
        self.forwards.push(forward);
        self
    }

    /// Finalise and validate the cube.
    ///
    /// Nodes must supply exactly one `(SabrParameterData, forward)` pair for every
    /// expiry-tenor combination, in the row-major order documented on
    /// [`VolCubeBuilder`]. The default interpolation mode is pointwise SABR
    /// volatility interpolation; change it on the returned cube when
    /// total-variance interpolation across expiries is required.
    ///
    /// # Errors
    ///
    /// Returns the same construction errors as [`VolCube::from_grid`]: empty,
    /// non-finite, non-positive, or unsorted axes; incorrect node count;
    /// non-finite forwards; or invalid SABR parameters and shifts.
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
