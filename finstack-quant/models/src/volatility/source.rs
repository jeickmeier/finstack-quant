//! Volatility evaluation over core market-data artifacts.

use std::sync::Arc;

use finstack_quant_core::{
    Error, InputError, Result,
    market_data::context::MarketContext,
    market_data::surfaces::{
        FxDeltaVolSurface, VolCube, VolCubeExpirySlice, VolGridOpts, VolInterpolationMode,
        VolQuoteType, VolSurface, VolSurfaceAxis,
    },
    types::CurveId,
};

use super::convert::VolatilityConvention;
use super::fx::get_fx_delta_vol;
use super::sabr::SabrParameters;

/// Concrete computational view over the core volatility artifacts.
#[derive(Clone, Debug)]
pub enum VolSource {
    /// Strike- or tenor-grid implied-volatility observations.
    Surface(Arc<VolSurface>),
    /// SABR parameter and forward nodes on an expiry-tenor grid.
    Cube(Arc<VolCube>),
    /// Tenor-by-strike quotes at one fixed option expiry.
    CubeExpirySlice(Arc<VolCubeExpirySlice>),
    /// FX ATM/risk-reversal/butterfly quotes in forward-delta space.
    FxDelta(Arc<FxDeltaVolSurface>),
}

impl VolSource {
    /// Resolve the source's quoting convention and interpolated displacement.
    ///
    /// Surface metadata determines normal versus Black quotes. SABR cubes
    /// require an explicit output selection through `get_cube_convention`.
    /// FX delta sources contain unshifted Black quotes. Coordinates are clamped
    /// to the source grid exactly as in clamped volatility evaluation.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Finite option expiry in years, clamped to stored pillars.
    /// * `tenor` - Finite underlying tenor in years, clamped to cube pillars.
    ///
    /// # Errors
    ///
    /// Returns a validation error for a SABR cube or a non-finite coordinate.
    pub fn get_convention(&self, expiry: f64, tenor: f64) -> Result<VolatilityConvention> {
        if !expiry.is_finite() || !tenor.is_finite() {
            return Err(Error::Validation(
                "volatility coordinates must be finite".to_owned(),
            ));
        }
        Ok(match self {
            Self::Surface(surface) => grid_convention(
                surface.quote_type(),
                surface.displacements(),
                surface.expiries(),
                expiry,
            )?,
            Self::CubeExpirySlice(slice) => grid_convention(
                slice.quote_type(),
                slice.displacements(),
                slice.tenors(),
                tenor,
            )?,
            Self::Cube(_) => {
                return Err(Error::Validation(
                    "SABR cube requires an explicit output quote convention".into(),
                ));
            }
            Self::FxDelta(_) => VolatilityConvention::Lognormal,
        })
    }

    /// Resolve the actual convention for an explicitly selected SABR cube output.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Positive option expiry in years.
    /// * `tenor` - Positive underlying tenor in years.
    /// * `requested` - Output quote convention; an explicit shift must match the cube shift.
    /// * `clamp` - Whether to allow flat extrapolation to the nearest cube coordinates.
    ///
    /// # Errors
    ///
    /// Returns a validation error for incompatible displacement, non-cube sources,
    /// out-of-grid checked coordinates, or unsupported parameter combinations.
    pub fn get_cube_convention(
        &self,
        expiry: f64,
        tenor: f64,
        requested: VolatilityConvention,
        clamp: bool,
    ) -> Result<VolatilityConvention> {
        let Self::Cube(cube) = self else {
            return Err(Error::Validation("expected a SABR cube".into()));
        };
        let (expiry, tenor, _) = if clamp {
            self.clamped_coordinates(expiry, tenor, 0.0)?
        } else {
            (expiry, tenor, 0.0)
        };
        segment(cube.expiries(), expiry)?;
        segment(cube.tenors(), tenor)?;
        if requested == VolatilityConvention::Normal {
            return Ok(requested);
        }
        let shift = cube_black_shift(cube, expiry, tenor)?;
        if let VolatilityConvention::ShiftedLognormal {
            shift: requested_shift,
        } = requested
            && shift != Some(requested_shift)
        {
            return Err(Error::Validation(format!(
                "requested SABR displacement {requested_shift} differs from the cube displacement {shift:?}"
            )));
        }
        Ok(shift.map_or(VolatilityConvention::Lognormal, |shift| {
            VolatilityConvention::ShiftedLognormal { shift }
        }))
    }

    /// Evaluate Black/lognormal volatility.
    ///
    /// Surface sources select `tenor` or `strike` according to their stored
    /// secondary-axis metadata. FX-delta sources require an explicit FX forward
    /// rate and must be queried via [`Self::get_fx_vol`] or [`Self::get_fx_vol_clamped`].
    /// Shifted-Black sources retain their displaced quote; callers must use
    /// [`Self::get_convention`] (or [`Self::get_cube_convention`] for a cube)
    /// and apply its displacement to both forward and strike when pricing. The returned number is not converted to unshifted Black.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Positive option expiry in years.
    /// * `tenor` - Underlying tenor in years (used by tenor-indexed surfaces and SABR cubes).
    /// * `strike` - Strike in the same units as the source's forward or grid.
    ///
    /// # Errors
    ///
    /// Returns an input or model validation error for out-of-grid checked
    /// coordinates, an FX-delta source without an explicit forward, or a failed SABR expansion.
    pub fn get_vol(&self, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
        match self {
            Self::Surface(surface) => {
                require_black_quotes(surface.quote_type())?;
                let secondary = match surface.secondary_axis() {
                    VolSurfaceAxis::Strike => strike,
                    VolSurfaceAxis::Tenor => tenor,
                };
                get_surface_vol(surface, expiry, secondary)
            }
            Self::CubeExpirySlice(slice) => {
                segment(&[slice.expiry()], expiry)?;
                require_black_quotes(slice.quote_type())?;
                get_cube_expiry_slice_vol(slice, tenor, strike)
            }
            Self::Cube(cube) => {
                get_cube_vol(cube, expiry, tenor, strike, VolatilityConvention::Lognormal)
            }
            Self::FxDelta(_) => Err(Error::Validation(
                "FX delta volatility sources require an explicit FX forward; use VolSource::get_fx_vol".into(),
            )),
        }
    }

    /// Evaluate Black/lognormal volatility with flat coordinate extrapolation.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Option expiry in years; finite values are clamped.
    /// * `tenor` - Underlying tenor in years (used by tenor-indexed surfaces and SABR cubes).
    /// * `strike` - Strike in source units; finite grid coordinates are clamped.
    ///
    /// # Errors
    ///
    /// Returns an error for incompatible quote metadata, an FX-delta source without
    /// an explicit forward, non-finite coordinates, or an invalid SABR evaluation.
    pub fn get_vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
        let (expiry, tenor, strike) = self.clamped_coordinates(expiry, tenor, strike)?;
        self.get_vol(expiry, tenor, strike)
    }

    /// Evaluate FX Black/lognormal volatility with an explicit FX forward rate.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Positive option expiry in years within the stored expiry grid.
    /// * `strike` - Positive strike in domestic-per-foreign FX units.
    /// * `forward` - Positive FX forward rate in the same units as `strike`.
    ///
    /// # Errors
    ///
    /// Returns an error for non-positive or out-of-grid coordinates, non-Black or
    /// tenor-indexed surfaces, SABR cubes (which require an underlying tenor), or
    /// invalid FX smile wing volatilities.
    pub fn get_fx_vol(&self, expiry: f64, strike: f64, forward: f64) -> Result<f64> {
        if !forward.is_finite() || forward <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        match self {
            Self::FxDelta(surface) => get_fx_delta_vol(surface, expiry, strike, forward),
            Self::Surface(surface) => {
                surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
                require_black_quotes(surface.quote_type())?;
                get_surface_vol(surface, expiry, strike)
            }
            Self::Cube(_) | Self::CubeExpirySlice(_) => Err(Error::Validation(
                "SABR cube volatility sources require an underlying tenor; use VolSource::get_vol"
                    .into(),
            )),
        }
    }

    /// Evaluate FX Black/lognormal volatility with flat strike extrapolation and an explicit FX forward rate.
    ///
    /// For [`VolSource::Surface`], both expiry and strike are clamped to the stored grid bounds.
    /// For [`VolSource::FxDelta`], strikes extrapolate flat beyond the materialized wing range while
    /// `expiry` remains strictly bounded by `[expiries.first(), expiries.last()]`.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Positive option expiry in years; clamped to stored expiry bounds on `Surface`, or strictly bounded to the surface expiry range on `FxDelta`.
    /// * `strike` - Positive strike in domestic-per-foreign FX units; clamped to stored strike bounds.
    /// * `forward` - Positive FX forward rate in the same units as `strike`.
    ///
    /// # Errors
    ///
    /// Returns an error for non-positive or non-finite coordinates, out-of-range `FxDelta` expiry,
    /// non-Black or tenor-indexed surfaces, SABR cubes, or invalid FX smile wing volatilities.
    pub fn get_fx_vol_clamped(&self, expiry: f64, strike: f64, forward: f64) -> Result<f64> {
        if !forward.is_finite() || forward <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        match self {
            Self::FxDelta(surface) => get_fx_delta_vol(surface, expiry, strike, forward),
            Self::Surface(surface) => {
                surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
                require_black_quotes(surface.quote_type())?;
                let (clamped_expiry, _, clamped_strike) =
                    self.clamped_coordinates(expiry, 0.0, strike)?;
                get_surface_vol(surface, clamped_expiry, clamped_strike)
            }
            Self::Cube(_) | Self::CubeExpirySlice(_) => Err(Error::Validation(
                "SABR cube volatility sources require an underlying tenor; use VolSource::get_vol_clamped"
                    .into(),
            )),
        }
    }

    /// Evaluate normal/Bachelier volatility.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Positive option expiry in years.
    /// * `tenor` - Underlying tenor in years.
    /// * `strike` - Strike in the same units as the source forward.
    ///
    /// # Errors
    ///
    /// Returns an error for Black-only FX sources, a surface not tagged as
    /// normal, invalid coordinates, or a failed SABR expansion.
    pub fn get_normal_vol(&self, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
        match self {
            Self::Surface(surface) => {
                surface.require_quote_type(VolQuoteType::Normal)?;
                let secondary = match surface.secondary_axis() {
                    VolSurfaceAxis::Strike => strike,
                    VolSurfaceAxis::Tenor => tenor,
                };
                get_surface_vol(surface, expiry, secondary)
            }
            Self::Cube(cube) => {
                get_cube_vol(cube, expiry, tenor, strike, VolatilityConvention::Normal)
            }
            Self::CubeExpirySlice(slice) => {
                segment(&[slice.expiry()], expiry)?;
                if slice.quote_type() != VolQuoteType::Normal {
                    return Err(Error::Validation(
                        "slice does not contain normal volatility quotes".into(),
                    ));
                }
                get_cube_expiry_slice_vol(slice, tenor, strike)
            }
            Self::FxDelta(_) => Err(Error::Validation(
                "FX delta volatility sources store Black/lognormal quotes".to_owned(),
            )),
        }
    }

    /// Evaluate normal/Bachelier volatility with flat coordinate extrapolation.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Option expiry in years; finite values are clamped.
    /// * `tenor` - Underlying tenor in years.
    /// * `strike` - Strike in the same units as the source forward.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-normal surface, FX source, non-finite inputs,
    /// or a failed normal SABR expansion.
    pub fn get_normal_vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
        let (expiry, tenor, strike) = self.clamped_coordinates(expiry, tenor, strike)?;
        self.get_normal_vol(expiry, tenor, strike)
    }

    fn clamped_coordinates(&self, expiry: f64, tenor: f64, strike: f64) -> Result<(f64, f64, f64)> {
        if !expiry.is_finite() || !tenor.is_finite() || !strike.is_finite() {
            return Err(Error::Validation(
                "volatility coordinates must be finite".to_owned(),
            ));
        }
        if expiry <= 0.0
            || (matches!(self, Self::Cube(_) | Self::CubeExpirySlice(_)) && tenor <= 0.0)
        {
            return Err(InputError::NonPositiveValue.into());
        }
        let clamp = |axis: &[f64], x: f64| x.clamp(axis[0], axis[axis.len() - 1]);
        Ok(match self {
            Self::Surface(surface) => {
                let expiry = clamp(surface.expiries(), expiry);
                match surface.secondary_axis() {
                    VolSurfaceAxis::Strike => {
                        (expiry, tenor, clamp(surface.secondary_values(), strike))
                    }
                    VolSurfaceAxis::Tenor => {
                        (expiry, clamp(surface.secondary_values(), tenor), strike)
                    }
                }
            }
            Self::Cube(cube) => (
                clamp(cube.expiries(), expiry),
                clamp(cube.tenors(), tenor),
                strike,
            ),
            Self::CubeExpirySlice(slice) => (
                slice.expiry(),
                clamp(slice.tenors(), tenor),
                clamp(slice.strikes(), strike),
            ),
            Self::FxDelta(_) => (expiry, tenor, strike),
        })
    }

    /// Return the core artifact identifier.
    pub fn get_id(&self) -> &CurveId {
        match self {
            Self::Surface(surface) => surface.id(),
            Self::Cube(cube) => cube.id(),
            Self::CubeExpirySlice(slice) => slice.id(),
            Self::FxDelta(surface) => surface.id(),
        }
    }
}

fn require_black_quotes(quote_type: VolQuoteType) -> Result<()> {
    if quote_type == VolQuoteType::Normal {
        Err(Error::Validation(
            "normal volatility quotes cannot supply a Black volatility".into(),
        ))
    } else {
        Ok(())
    }
}

fn grid_convention(
    quote_type: VolQuoteType,
    shifts: Option<&[f64]>,
    axis: &[f64],
    coordinate: f64,
) -> Result<VolatilityConvention> {
    Ok(match quote_type {
        VolQuoteType::Normal => VolatilityConvention::Normal,
        VolQuoteType::BlackLognormal => VolatilityConvention::Lognormal,
        VolQuoteType::ShiftedBlackLognormal => {
            let shifts = shifts.ok_or_else(|| {
                Error::Validation("shifted Black quotes are missing displacement metadata".into())
            })?;
            let (lo, hi, weight) = segment_clamped(axis, coordinate);
            VolatilityConvention::ShiftedLognormal {
                shift: (1.0 - weight) * shifts[lo] + weight * shifts[hi],
            }
        }
    })
}

pub(super) fn segment(axis: &[f64], value: f64) -> Result<(usize, usize, f64)> {
    if axis.is_empty() {
        return Err(InputError::TooFewPoints.into());
    }
    if !value.is_finite() {
        return Err(Error::Validation(format!(
            "volatility coordinate must be finite, got {value}"
        )));
    }
    if value < axis[0] || value > axis[axis.len() - 1] {
        return Err(Error::Validation(format!(
            "volatility coordinate {value} is out of bounds [{}, {}]",
            axis[0],
            axis[axis.len() - 1]
        )));
    }
    if axis.len() == 1 {
        return Ok((0, 0, 0.0));
    }
    let upper = axis.partition_point(|node| *node <= value);
    if upper == axis.len() {
        let last = axis.len() - 1;
        return Ok((last, last, 0.0));
    }
    #[allow(clippy::float_cmp)]
    if upper > 0 && axis[upper - 1] == value {
        return Ok((upper - 1, upper - 1, 0.0));
    }
    let lower = upper - 1;
    let weight = (value - axis[lower]) / (axis[upper] - axis[lower]);
    Ok((lower, upper, weight))
}

#[inline]
pub(super) fn segment_clamped(axis: &[f64], value: f64) -> (usize, usize, f64) {
    let clamped = value.clamp(axis[0], axis[axis.len() - 1]);
    segment(axis, clamped).unwrap_or((0, 0, 0.0))
}

#[inline]
pub(super) fn linear_clamped(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let (lo, hi, weight) = segment_clamped(xs, x);
    ys[lo] + weight * (ys[hi] - ys[lo])
}

#[inline]
pub(super) fn bilinear(q00: f64, q10: f64, q01: f64, q11: f64, t: f64, u: f64) -> f64 {
    (1.0 - t) * (1.0 - u) * q00 + t * (1.0 - u) * q10 + (1.0 - t) * u * q01 + t * u * q11
}

/// Evaluate a core surface with checked coordinates.
///
/// # Arguments
///
/// * `surface` - Structurally validated observed volatility grid.
/// * `expiry` - Option expiry in years within the stored expiry axis.
/// * `strike` - Secondary-axis coordinate within the stored grid.
///
/// # Errors
///
/// Returns an input error for non-finite or out-of-grid coordinates and a
/// validation error if total-variance interpolation is invalid.
pub fn get_surface_vol(surface: &VolSurface, expiry: f64, strike: f64) -> Result<f64> {
    let (e0, e1, t) = segment(surface.expiries(), expiry)?;
    let (s0, s1, u) = segment(surface.secondary_values(), strike)?;
    interpolate_surface(surface, expiry, (e0, e1, t), (s0, s1, u))
}

/// Evaluate a core surface after clamping finite coordinates to its axes.
///
/// # Arguments
///
/// * `surface` - Structurally validated observed volatility grid.
/// * `expiry` - Option expiry in years; finite values are clamped.
/// * `strike` - Secondary-axis coordinate; finite values are clamped.
pub fn get_surface_vol_clamped(surface: &VolSurface, expiry: f64, strike: f64) -> f64 {
    if !expiry.is_finite() || expiry <= 0.0 || !strike.is_finite() {
        return f64::NAN;
    }
    let Some((&emin, &emax)) = surface.expiries().first().zip(surface.expiries().last()) else {
        return f64::NAN;
    };
    let Some((&smin, &smax)) = surface
        .secondary_values()
        .first()
        .zip(surface.secondary_values().last())
    else {
        return f64::NAN;
    };
    get_surface_vol(surface, expiry.clamp(emin, emax), strike.clamp(smin, smax)).unwrap_or(f64::NAN)
}

/// Measure closing minus opening implied volatility at a fixed strike or tenor.
///
/// Both surfaces must have the same secondary-axis meaning, annual quote basis,
/// and effective displacement at the requested points. A Black move from 0.20
/// to 0.21 returns 0.01. A normal move from 0.008 to 0.009 returns 0.001 in
/// underlying units per square-root year.
///
/// # Arguments
///
/// * `vol_surface_id` - Identifier of the same underlying volatility surface in both market contexts.
/// * `market_t0` - Opening market containing the observed surface.
/// * `market_t1` - Closing market containing the observed surface.
/// * `expiry_t0` - Opening expiry in source years, strictly inside or on its observed grid.
/// * `expiry_t1` - Closing expiry in source years for the comparable contract or tenor.
/// * `secondary` - Absolute strike in underlying units or underlying tenor in years, as declared on both grids.
///
/// # Errors
///
/// Returns an error for missing surfaces, incompatible quote conventions or
/// annual clock bases, an out-of-grid coordinate, or inconsistent shifts.
pub fn measure_vol_surface_shift(
    vol_surface_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    expiry_t0: f64,
    expiry_t1: f64,
    secondary: f64,
) -> Result<f64> {
    let surface_t0 = market_t0.get_surface(&vol_surface_id)?;
    let surface_t1 = market_t1.get_surface(&vol_surface_id)?;
    let compatible_clocks = match (surface_t0.expiry_clock(), surface_t1.expiry_clock()) {
        (None, None) => true,
        (Some(opening), Some(closing)) => {
            opening.get_day_count() == closing.get_day_count()
                && opening.get_context() == closing.get_context()
        }
        _ => false,
    };
    if surface_t0.secondary_axis() != surface_t1.secondary_axis()
        || surface_t0.quote_type() != surface_t1.quote_type()
        || !compatible_clocks
    {
        return Err(Error::Validation(format!(
            "incompatible quote type, secondary axis or annual volatility clock basis for '{}'",
            vol_surface_id.as_ref()
        )));
    }
    let displacement = |surface: &VolSurface, expiry: f64| -> Result<Option<f64>> {
        let Some(shifts) = surface.displacements() else {
            return Ok(None);
        };
        let (lo, hi, _) = segment(surface.expiries(), expiry)?;
        if shifts[lo].partial_cmp(&shifts[hi]) != Some(std::cmp::Ordering::Equal) {
            return Err(Error::Validation(
                "cannot blend volatility quotes with different displacements".into(),
            ));
        }
        Ok(Some(shifts[lo]))
    };
    if displacement(&surface_t0, expiry_t0)? != displacement(&surface_t1, expiry_t1)? {
        return Err(Error::Validation(format!(
            "incompatible volatility displacements for '{}'",
            vol_surface_id.as_ref()
        )));
    }
    Ok(get_surface_vol(&surface_t1, expiry_t1, secondary)?
        - get_surface_vol(&surface_t0, expiry_t0, secondary)?)
}

/// Bilinear (vol or total-variance) interpolation inside one grid cell.
///
/// `(e0, e1, t)` and `(s0, s1, u)` are the bracketing expiry and strike
/// indices with their interpolation weights, as returned by `segment`.
fn interpolate_surface(
    surface: &VolSurface,
    expiry: f64,
    (e0, e1, t): (usize, usize, f64),
    (s0, s1, u): (usize, usize, f64),
) -> Result<f64> {
    let columns = surface.secondary_values().len();
    let values = surface.vols();
    let q00 = values[e0 * columns + s0];
    let q10 = values[e1 * columns + s0];
    let q01 = values[e0 * columns + s1];
    let q11 = values[e1 * columns + s1];
    match surface.interpolation_mode() {
        VolInterpolationMode::Vol => Ok(bilinear(q00, q10, q01, q11, t, u)),
        VolInterpolationMode::TotalVariance => {
            let at_t0 = e0 == e1 || t.total_cmp(&0.0).is_eq();
            let at_t1 = t.total_cmp(&1.0).is_eq();
            let at_u0 = s0 == s1 || u.total_cmp(&0.0).is_eq();
            let at_u1 = u.total_cmp(&1.0).is_eq();
            if at_t0 && at_u0 {
                return Ok(q00);
            }
            if at_t1 && at_u0 {
                return Ok(q10);
            }
            if at_t0 && at_u1 {
                return Ok(q01);
            }
            if at_t1 && at_u1 {
                return Ok(q11);
            }
            let scale = q00.max(q10).max(q01).max(q11);
            if scale == 0.0 {
                return Ok(0.0);
            }
            let r00 = q00 / scale;
            let r10 = q10 / scale;
            let r01 = q01 / scale;
            let r11 = q11 / scale;
            let w0_hat = (1.0 - u) * r00 * r00 + u * r01 * r01;
            let w1_hat = (1.0 - u) * r10 * r10 + u * r11 * r11;
            let mean_hat = if e0 == e1 {
                w0_hat
            } else {
                let x0 = surface.expiries()[e0];
                let x1 = surface.expiries()[e1];
                let a0 = (1.0 - t) * (x0 / expiry);
                let ratio1 = x1 / expiry;
                let a1 = if ratio1.is_finite() {
                    t * ratio1
                } else {
                    (t * x1) / expiry
                };
                a0 * w0_hat + a1 * w1_hat
            };
            let sigma = scale * mean_hat.sqrt();
            if !sigma.is_finite() || (mean_hat > 0.0 && sigma == 0.0) {
                return Err(Error::Validation(format!(
                    "vol surface '{}' produced invalid total variance at expiry {expiry}",
                    surface.id()
                )));
            }
            Ok(sigma)
        }
    }
}

fn cube_params(cube: &VolCube, expiry: f64, tenor: f64) -> Result<(SabrParameters, f64, f64)> {
    let (e0, e1, t) = segment(cube.expiries(), expiry)?;
    let (n0, n1, u) = segment(cube.tenors(), tenor)?;
    let columns = cube.tenors().len();
    let index = |e: usize, n: usize| e * columns + n;
    let p00 = &cube.params()[index(e0, n0)];
    if e0 == e1 && n0 == n1 {
        return Ok((
            SabrParameters::from(*p00),
            cube.forwards()[index(e0, n0)],
            expiry,
        ));
    }
    let p10 = &cube.params()[index(e1, n0)];
    let p01 = &cube.params()[index(e0, n1)];
    let p11 = &cube.params()[index(e1, n1)];
    // segment collapses exact coordinates to one index, so these corners
    // reference only the nodes that contribute at this expiry and tenor.
    let beta = p00.beta();
    if [p10, p01, p11]
        .iter()
        .any(|node| node.beta().partial_cmp(&beta) != Some(std::cmp::Ordering::Equal))
    {
        return Err(Error::Validation(format!(
            "SABR cube '{}' cannot interpolate different beta values at expiry {expiry}, tenor {tenor}",
            cube.id()
        )));
    }
    let shift = match [p00.shift(), p10.shift(), p01.shift(), p11.shift()] {
        [None, None, None, None] => None,
        [Some(s00), Some(s10), Some(s01), Some(s11)] => {
            if [s10, s01, s11]
                .iter()
                .all(|shift| shift.partial_cmp(&s00) == Some(std::cmp::Ordering::Equal))
            {
                Some(s00)
            } else {
                Some(bilinear(s00, s10, s01, s11, t, u))
            }
        }
        _ => {
            return Err(Error::Validation(format!(
                "SABR cube '{}' mixes absent and explicit displacements in one parameter interpolation",
                cube.id()
            )));
        }
    };
    let params = SabrParameters {
        alpha: bilinear(p00.alpha(), p10.alpha(), p01.alpha(), p11.alpha(), t, u),
        beta,
        nu: bilinear(p00.nu(), p10.nu(), p01.nu(), p11.nu(), t, u),
        rho: bilinear(p00.rho(), p10.rho(), p01.rho(), p11.rho(), t, u),
        shift,
    };
    params.validate()?;
    let forward = bilinear(
        cube.forwards()[index(e0, n0)],
        cube.forwards()[index(e1, n0)],
        cube.forwards()[index(e0, n1)],
        cube.forwards()[index(e1, n1)],
        t,
        u,
    );
    Ok((params, forward, expiry))
}

fn cube_vol(cube: &VolCube, expiry: f64, tenor: f64, strike: f64, normal: bool) -> Result<f64> {
    if !strike.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            "strike must be finite".to_string(),
        ));
    }
    let (params, forward, model_expiry) = cube_params(cube, expiry, tenor)?;
    if normal {
        params.implied_vol_normal(forward, strike, model_expiry)
    } else {
        params.implied_vol_lognormal(forward, strike, model_expiry)
    }
}

/// Whether a cube output convention is normal; a shifted-lognormal output is
/// not a cube quote (the cube's own shifts apply to its lognormal output).
fn normal_output(convention: VolatilityConvention) -> Result<bool> {
    match convention {
        VolatilityConvention::Normal => Ok(true),
        VolatilityConvention::Lognormal => Ok(false),
        VolatilityConvention::ShiftedLognormal { .. } => {
            Err(finstack_quant_core::Error::Validation(
                "SABR cube volatilities are quoted normal or lognormal (the cube's own shifts \
                 apply to lognormal); a shifted-lognormal output convention is not supported"
                    .to_string(),
            ))
        }
    }
}

/// Evaluate checked SABR cube volatility in one output convention.
///
/// # Arguments
///
/// * `cube` - Structurally validated SABR parameter and forward grid.
/// * `expiry` - Option expiry in years within the stored expiry axis.
/// * `tenor` - Underlying tenor in years within the stored tenor axis.
/// * `strike` - Strike in the same decimal-rate units as the cube forwards.
/// * `convention` - Output quote: [`VolatilityConvention::Lognormal`] (Black,
///   shifted by the cube's own shifts where it stores them) or
///   [`VolatilityConvention::Normal`] (absolute, decimal rate units).
///
/// # Errors
///
/// Returns an input error for non-finite or out-of-grid coordinates, a
/// validation error for a shifted-lognormal `convention`, and a validation
/// error when the interpolated SABR expansion is undefined.
pub fn get_cube_vol(
    cube: &VolCube,
    expiry: f64,
    tenor: f64,
    strike: f64,
    convention: VolatilityConvention,
) -> Result<f64> {
    let normal = normal_output(convention)?;
    segment(cube.expiries(), expiry)?;
    segment(cube.tenors(), tenor)?;
    if cube.interpolation_mode() == VolInterpolationMode::TotalVariance {
        return cube_total_variance(cube, expiry, tenor, strike, normal);
    }
    cube_vol(cube, expiry, tenor, strike, normal)
}

/// Evaluate clamped SABR cube volatility in one output convention.
///
/// # Arguments
///
/// * `cube` - Structurally validated SABR parameter and forward grid.
/// * `expiry` - Option expiry in years; finite values are clamped to the grid.
/// * `tenor` - Underlying tenor in years; finite values are clamped to the grid.
/// * `strike` - Strike in the same decimal-rate units as the cube forwards.
/// * `convention` - Output quote: [`VolatilityConvention::Lognormal`] or
///   [`VolatilityConvention::Normal`].
///
/// # Errors
///
/// Returns a validation error for invalid coordinates, conventions or expansions.
pub fn get_cube_vol_clamped(
    cube: &VolCube,
    expiry: f64,
    tenor: f64,
    strike: f64,
    convention: VolatilityConvention,
) -> Result<f64> {
    if !expiry.is_finite() || !tenor.is_finite() || !strike.is_finite() {
        return Err(Error::Validation("SABR coordinates must be finite".into()));
    }
    if expiry <= 0.0 || tenor <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    let expiry = expiry.clamp(
        cube.expiries()[0],
        cube.expiries()[cube.expiries().len() - 1],
    );
    let tenor = tenor.clamp(cube.tenors()[0], cube.tenors()[cube.tenors().len() - 1]);
    get_cube_vol(cube, expiry, tenor, strike, convention)
}

/// Materialize a cube tenor slice as a core surface artifact.
///
/// # Arguments
///
/// * `cube` - Source SABR cube.
/// * `tenor` - Underlying tenor in years within the stored cube axis.
/// * `strikes` - Non-empty finite strike grid in the cube forward's units.
/// * `convention` - Output quote: [`VolatilityConvention::Lognormal`] (a Black
///   or shifted-Black surface, by the cube's shifts) or
///   [`VolatilityConvention::Normal`] (a normal surface).
///
/// * `interpolation_mode` - Output sigma-grid expiry interpolation, chosen independently of the SABR cube evaluator.
///
/// # Errors
///
/// Returns a validation error for a shifted-lognormal `convention`, and an
/// input or structural-validation error when the requested grid cannot form a
/// valid surface artifact.
pub fn materialize_cube_tenor_slice(
    cube: &VolCube,
    tenor: f64,
    strikes: &[f64],
    convention: VolatilityConvention,
    interpolation_mode: VolInterpolationMode,
) -> Result<VolSurface> {
    materialize_cube_tenor_slice_with_convention(
        cube,
        tenor,
        strikes,
        normal_output(convention)?,
        interpolation_mode,
    )
}

enum CubeSliceAxis {
    Tenor(f64),
    Expiry(f64),
}

fn evaluate_cube_slice_grid(
    cube: &VolCube,
    axis: CubeSliceAxis,
    strikes: &[f64],
    normal: bool,
) -> Result<(Vec<f64>, Option<Vec<f64>>, VolQuoteType)> {
    let (label, fixed_axis, fixed_coord, sampled_axis) = match axis {
        CubeSliceAxis::Tenor(tenor) => ("tenor", cube.tenors(), tenor, cube.expiries()),
        CubeSliceAxis::Expiry(expiry) => ("expiry", cube.expiries(), expiry, cube.tenors()),
    };
    if strikes.is_empty()
        || !fixed_coord.is_finite()
        || strikes.iter().any(|value| !value.is_finite())
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{label} slice needs a finite {label} and a non-empty, finite strike grid"
        )));
    }
    segment(fixed_axis, fixed_coord)?;
    let convention = if normal {
        VolatilityConvention::Normal
    } else {
        VolatilityConvention::Lognormal
    };
    let coords = |sample: f64| match axis {
        CubeSliceAxis::Tenor(tenor) => (sample, tenor),
        CubeSliceAxis::Expiry(expiry) => (expiry, sample),
    };
    let mut values = Vec::with_capacity(sampled_axis.len() * strikes.len());
    for &sample in sampled_axis {
        let (expiry, tenor) = coords(sample);
        for &strike in strikes {
            values.push(get_cube_vol(cube, expiry, tenor, strike, convention)?);
        }
    }
    let displacements = if normal {
        None
    } else {
        let shifts = sampled_axis
            .iter()
            .map(|&sample| {
                let (expiry, tenor) = coords(sample);
                cube_black_shift(cube, expiry, tenor)
            })
            .collect::<Result<Vec<_>>>()?;
        materialized_displacements(shifts)?
    };
    let quote_type = if normal {
        VolQuoteType::Normal
    } else if displacements.is_some() {
        VolQuoteType::ShiftedBlackLognormal
    } else {
        VolQuoteType::BlackLognormal
    };
    Ok((values, displacements, quote_type))
}

fn materialize_cube_tenor_slice_with_convention(
    cube: &VolCube,
    tenor: f64,
    strikes: &[f64],
    normal: bool,
    interpolation_mode: VolInterpolationMode,
) -> Result<VolSurface> {
    let (values, displacements, quote_type) =
        evaluate_cube_slice_grid(cube, CubeSliceAxis::Tenor(tenor), strikes, normal)?;
    if interpolation_mode == VolInterpolationMode::TotalVariance
        && displacements.as_ref().is_some_and(|shifts| {
            shifts
                .windows(2)
                .any(|pair| pair[0].partial_cmp(&pair[1]) != Some(std::cmp::Ordering::Equal))
        })
    {
        return Err(Error::Validation(
            "tenor slice cannot interpolate Black total variance across different displacements"
                .into(),
        ));
    }
    let mut surface = VolSurface::from_grid(
        cube.id().as_str(),
        cube.expiries(),
        strikes,
        &values,
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type,
            interpolation_mode,
        },
        displacements.as_deref(),
    )?;
    if let Some(clock) = cube.expiry_clock() {
        surface = surface.with_expiry_clock(clock.clone());
    }
    Ok(surface)
}

/// Materialize quotes on tenor and strike axes at one fixed expiry.
///
/// # Arguments
///
/// * `cube` - Source SABR cube.
/// * `expiry` - Option expiry in years within the stored cube axis.
/// * `strikes` - Non-empty finite strike grid in the cube forward's units.
/// * `convention` - Output quote: [`VolatilityConvention::Lognormal`] or
///   [`VolatilityConvention::Normal`].
///
/// # Errors
///
/// Returns a validation error for a shifted-lognormal `convention`, and an
/// input or structural-validation error when the requested grid cannot form a
/// valid tenor-axis surface artifact.
pub fn materialize_cube_expiry_slice(
    cube: &VolCube,
    expiry: f64,
    strikes: &[f64],
    convention: VolatilityConvention,
) -> Result<VolCubeExpirySlice> {
    materialize_cube_expiry_slice_with_convention(cube, expiry, strikes, normal_output(convention)?)
}

fn materialize_cube_expiry_slice_with_convention(
    cube: &VolCube,
    expiry: f64,
    strikes: &[f64],
    normal: bool,
) -> Result<VolCubeExpirySlice> {
    let (values, displacements, quote_type) =
        evaluate_cube_slice_grid(cube, CubeSliceAxis::Expiry(expiry), strikes, normal)?;
    let mut slice = VolCubeExpirySlice::from_grid(
        cube.id().as_str(),
        expiry,
        cube.tenors(),
        strikes,
        &values,
        quote_type,
        displacements.as_deref(),
    )?;
    if let Some(clock) = cube.expiry_clock() {
        slice = slice.with_expiry_clock(clock.clone());
    }
    Ok(slice)
}

/// Interpolate fixed-expiry volatility over the slice's tenor and strike axes.
///
/// # Arguments
///
/// * `slice` - Structurally validated fixed-expiry quotes with explicit tenor and strike coordinates.
/// * `tenor` - Finite underlying tenor in years within the stored axis.
/// * `strike` - Finite strike in forward-price or rate units within the stored axis.
///
/// # Errors
///
/// Returns a validation error for non-finite or out-of-grid coordinates.
pub fn get_cube_expiry_slice_vol(
    slice: &VolCubeExpirySlice,
    tenor: f64,
    strike: f64,
) -> Result<f64> {
    let (t0, t1, t) = segment(slice.tenors(), tenor)?;
    let (s0, s1, u) = segment(slice.strikes(), strike)?;
    let cols = slice.strikes().len();
    let values = slice.vols();
    Ok(bilinear(
        values[t0 * cols + s0],
        values[t1 * cols + s0],
        values[t0 * cols + s1],
        values[t1 * cols + s1],
        t,
        u,
    ))
}

/// Interpolate fixed-expiry quotes with flat tenor and strike extrapolation.
///
/// # Arguments
///
/// * `slice` - Validated fixed-expiry tenor-by-strike quote grid.
/// * `tenor` - Underlying tenor in years; finite values are clamped to the stored axis.
/// * `strike` - Strike in forward-price or rate units; finite values are clamped to the stored axis.
///
/// Non-finite coordinates return NaN.
pub fn get_cube_expiry_slice_vol_clamped(
    slice: &VolCubeExpirySlice,
    tenor: f64,
    strike: f64,
) -> f64 {
    if !tenor.is_finite() || !strike.is_finite() {
        return f64::NAN;
    }
    get_cube_expiry_slice_vol(
        slice,
        tenor.clamp(slice.tenors()[0], slice.tenors()[slice.tenors().len() - 1]),
        strike.clamp(
            slice.strikes()[0],
            slice.strikes()[slice.strikes().len() - 1],
        ),
    )
    .unwrap_or(f64::NAN)
}

fn materialized_displacements(shifts: Vec<Option<f64>>) -> Result<Option<Vec<f64>>> {
    if shifts.iter().all(Option::is_none) {
        return Ok(None);
    }
    shifts
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .map(Some)
        .ok_or_else(|| {
            Error::Validation(
                "one materialized Black grid cannot mix shifted and unshifted quotes".into(),
            )
        })
}

fn cube_black_shift(cube: &VolCube, expiry: f64, tenor: f64) -> Result<Option<f64>> {
    if cube.interpolation_mode() != VolInterpolationMode::TotalVariance {
        return Ok(cube_params(cube, expiry, tenor)?.0.shift);
    }
    let (lo, hi, _) = segment(cube.expiries(), expiry)?;
    let left = cube_params(cube, cube.expiries()[lo], tenor)?.0.shift;
    if lo != hi && left != cube_params(cube, cube.expiries()[hi], tenor)?.0.shift {
        return Err(Error::Validation(format!(
            "SABR cube '{}' cannot blend Black total variances across different displacements",
            cube.id()
        )));
    }
    Ok(left)
}

fn cube_total_variance(
    cube: &VolCube,
    expiry: f64,
    tenor: f64,
    strike: f64,
    normal: bool,
) -> Result<f64> {
    let (e0, e1, weight) = segment(cube.expiries(), expiry)?;
    if !normal && e0 != e1 {
        cube_black_shift(cube, expiry, tenor)?;
    }
    let t0 = cube.expiries()[e0];
    let t1 = cube.expiries()[e1];
    let v0 = cube_vol(cube, t0, tenor, strike, normal)?;
    if e0 == e1 {
        return Ok(v0);
    }
    let v1 = cube_vol(cube, t1, tenor, strike, normal)?;
    let scale = v0.max(v1);
    let relative_variance = (1.0 - weight) * (t0 / expiry) * (v0 / scale).powi(2)
        + ((weight * t1) / expiry) * (v1 / scale).powi(2);
    let volatility = scale * relative_variance.sqrt();
    if !volatility.is_finite() || volatility <= 0.0 {
        return Err(Error::Validation(format!(
            "vol cube '{}' produced invalid total variance at expiry {expiry}",
            cube.id()
        )));
    }
    Ok(volatility)
}
