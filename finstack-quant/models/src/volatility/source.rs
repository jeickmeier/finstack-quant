//! Volatility evaluation over core market-data artifacts.

use std::sync::Arc;

use finstack_quant_core::{
    error::InputError,
    market_data::context::MarketContext,
    market_data::surfaces::{
        FxDeltaVolSurface, VolCube, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    },
    types::CurveId,
    Error, Result,
};

use super::conventions::VolatilityConvention;
use super::fx::get_fx_delta_vol;
use super::sabr::{SabrModel, SabrParameters, SabrVolType};

/// Concrete computational view over the core volatility artifacts.
#[derive(Clone, Debug)]
pub enum VolSource {
    /// Strike- or tenor-grid implied-volatility observations.
    Surface(Arc<VolSurface>),
    /// SABR parameter and forward nodes on an expiry-tenor grid.
    Cube(Arc<VolCube>),
    /// FX ATM/risk-reversal/butterfly quotes in forward-delta space.
    FxDelta(Arc<FxDeltaVolSurface>),
}

impl VolSource {
    /// Resolve the source's quoting convention and interpolated displacement.
    ///
    /// Surface metadata determines normal versus Black quotes. Cubes select
    /// normal for beta zero and otherwise preserve the interpolated SABR shift.
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
    /// Returns a validation error when either coordinate is non-finite.
    pub fn get_convention(&self, expiry: f64, tenor: f64) -> Result<VolatilityConvention> {
        if !expiry.is_finite() || !tenor.is_finite() {
            return Err(Error::Validation(
                "volatility coordinates must be finite".to_owned(),
            ));
        }
        Ok(match self {
            Self::Surface(surface) => match surface.quote_type() {
                VolQuoteType::Normal => VolatilityConvention::Normal,
                VolQuoteType::BlackLognormal => VolatilityConvention::Lognormal,
            },
            Self::Cube(cube) => {
                let (params, _, _) = cube_params(cube, expiry, tenor);
                if SabrModel::new(params.clone()).vol_type() == SabrVolType::Normal {
                    VolatilityConvention::Normal
                } else if let Some(shift) = params.shift {
                    VolatilityConvention::ShiftedLognormal { shift }
                } else {
                    VolatilityConvention::Lognormal
                }
            }
            Self::FxDelta(_) => VolatilityConvention::Lognormal,
        })
    }

    /// Evaluate Black/lognormal volatility.
    ///
    /// For an FX-delta source, `tenor` carries the positive FX forward because
    /// that artifact has no tenor axis. Surface sources select `tenor` or
    /// `strike` according to their stored secondary-axis metadata.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Positive option expiry in years.
    /// * `tenor` - Underlying tenor in years, or the positive FX forward for
    ///   an FX-delta source.
    /// * `strike` - Strike in the same units as the source's forward or grid.
    ///
    /// # Errors
    ///
    /// Returns an input or model validation error for out-of-grid checked
    /// coordinates, invalid FX inputs, or a failed SABR expansion.
    pub fn get_vol(&self, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
        match self {
            Self::Surface(surface) => {
                surface.require_quote_type(VolQuoteType::BlackLognormal)?;
                let secondary = match surface.secondary_axis() {
                    VolSurfaceAxis::Strike => strike,
                    VolSurfaceAxis::Tenor => tenor,
                };
                get_surface_vol(surface, expiry, secondary)
            }
            Self::Cube(cube) => {
                if self.get_convention(expiry, tenor)? == VolatilityConvention::Normal {
                    return Err(Error::Validation(
                        "normal SABR cube cannot supply a Black volatility quote".into(),
                    ));
                }
                get_cube_vol(cube, expiry, tenor, strike)
            }
            Self::FxDelta(surface) => get_fx_delta_vol(surface, expiry, strike, tenor),
        }
    }

    /// Evaluate Black/lognormal volatility with flat coordinate extrapolation.
    ///
    /// # Arguments
    ///
    /// * `expiry` - Option expiry in years; finite values are clamped.
    /// * `tenor` - Underlying tenor, or FX forward for an FX-delta source.
    /// * `strike` - Strike in source units; finite grid coordinates are clamped.
    ///
    /// # Errors
    ///
    /// Returns an error for incompatible quote metadata, non-finite coordinates,
    /// or an invalid SABR/FX evaluation.
    pub fn get_vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
        let (expiry, tenor, strike) = self.clamped_coordinates(expiry, tenor, strike)?;
        self.get_vol(expiry, tenor, strike)
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
            Self::Cube(cube) => get_cube_normal_vol(cube, expiry, tenor, strike),
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
        let clamp = |axis: &[f64], x: f64| x.clamp(axis[0], axis[axis.len() - 1]);
        Ok(match self {
            Self::Surface(surface) => {
                let expiry = clamp(surface.expiries(), expiry);
                match surface.secondary_axis() {
                    VolSurfaceAxis::Strike => (expiry, tenor, clamp(surface.strikes(), strike)),
                    VolSurfaceAxis::Tenor => (expiry, clamp(surface.strikes(), tenor), strike),
                }
            }
            Self::Cube(cube) => (
                clamp(cube.expiries(), expiry),
                clamp(cube.tenors(), tenor),
                strike,
            ),
            Self::FxDelta(_) => (expiry, tenor, strike),
        })
    }

    /// Return the core artifact identifier.
    pub fn get_id(&self) -> &CurveId {
        match self {
            Self::Surface(surface) => surface.id(),
            Self::Cube(cube) => cube.id(),
            Self::FxDelta(surface) => surface.id(),
        }
    }
}

fn segment(axis: &[f64], value: f64) -> Result<(usize, usize, f64)> {
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
fn bilinear(q00: f64, q10: f64, q01: f64, q11: f64, t: f64, u: f64) -> f64 {
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
    let (s0, s1, u) = segment(surface.strikes(), strike)?;
    interpolate_surface(surface, expiry, e0, e1, s0, s1, t, u)
}

/// Evaluate a core surface after clamping finite coordinates to its axes.
///
/// # Arguments
///
/// * `surface` - Structurally validated observed volatility grid.
/// * `expiry` - Option expiry in years; finite values are clamped.
/// * `strike` - Secondary-axis coordinate; finite values are clamped.
pub fn get_surface_vol_clamped(surface: &VolSurface, expiry: f64, strike: f64) -> f64 {
    if !expiry.is_finite() || !strike.is_finite() {
        return f64::NAN;
    }
    let Some((&emin, &emax)) = surface.expiries().first().zip(surface.expiries().last()) else {
        return f64::NAN;
    };
    let Some((&smin, &smax)) = surface.strikes().first().zip(surface.strikes().last()) else {
        return f64::NAN;
    };
    get_surface_vol(surface, expiry.clamp(emin, emax), strike.clamp(smin, smax)).unwrap_or(f64::NAN)
}

/// Evaluate a surface with SVI wing extrapolation.
///
/// Expiry is clamped to the nearest stored pillar. Out-of-grid Black strikes
/// are evaluated from an SVI fit to the nearest expiry row. Normal-volatility
/// strikes use flat-wing extrapolation because SVI is a lognormal convention.
/// SVI fitting failures are returned and never replaced by a flat Black wing.
///
/// # Arguments
///
/// * `surface` - Structurally validated observed volatility grid.
/// * `expiry` - Option expiry in years.
/// * `strike` - Strike in the same units as `forward`.
/// * `forward` - Positive forward used to form log-moneyness.
///
/// # Errors
///
/// Returns an error for invalid inputs, fewer than five Black-volatility strike
/// nodes, or failed SVI calibration/evaluation.
pub fn get_surface_vol_extrapolated(
    surface: &VolSurface,
    expiry: f64,
    strike: f64,
    forward: f64,
) -> Result<f64> {
    if !expiry.is_finite() || !strike.is_finite() || !forward.is_finite() || forward <= 0.0 {
        return Err(InputError::Invalid.into());
    }
    if let Ok(vol) = get_surface_vol(surface, expiry, strike) {
        return Ok(vol);
    }
    let expiry = expiry.clamp(
        surface.expiries()[0],
        surface.expiries()[surface.expiries().len() - 1],
    );
    if strike >= surface.strikes()[0] && strike <= surface.strikes()[surface.strikes().len() - 1] {
        return get_surface_vol(surface, expiry, strike);
    }
    if surface.quote_type() == VolQuoteType::Normal {
        return Ok(get_surface_vol_clamped(surface, expiry, strike));
    }
    if surface.strikes().len() < 5 {
        return Err(Error::Validation(format!(
            "SVI wing extrapolation for '{}' requires at least five strikes",
            surface.id()
        )));
    }
    let row = surface
        .expiries()
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| (*left - expiry).abs().total_cmp(&(*right - expiry).abs()))
        .map_or(0, |(index, _)| index);
    let columns = surface.strikes().len();
    let start = row * columns;
    let vols = &surface.vols()[start..start + columns];
    let model_expiry = surface.expiries()[row];
    let params = super::svi::calibrate_svi(surface.strikes(), vols, forward, model_expiry)?;
    params.implied_vol((strike / forward).ln(), model_expiry)
}

/// Measure an implied-volatility surface move in percentage points.
///
/// When both reference coordinates are supplied, the two surfaces are
/// evaluated at that point. Otherwise the function averages changes at the
/// first surface's expiry nodes and middle secondary-axis node.
///
/// # Arguments
///
/// * `vol_surface_id` - Surface identifier present in both market contexts.
/// * `market_t0` - Earlier market context.
/// * `market_t1` - Later market context.
/// * `reference_expiry` - Optional expiry in years; must be supplied together
///   with `reference_strike` to select a single point.
/// * `reference_strike` - Optional secondary-axis coordinate; must be supplied
///   together with `reference_expiry`.
///
/// # Errors
///
/// Returns an error when a surface is missing or a requested coordinate is
/// outside either surface.
pub fn measure_vol_surface_shift(
    vol_surface_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    reference_expiry: Option<f64>,
    reference_strike: Option<f64>,
) -> Result<f64> {
    let surface_t0 = market_t0.get_surface(&vol_surface_id)?;
    let surface_t1 = market_t1.get_surface(&vol_surface_id)?;
    if let (Some(expiry), Some(strike)) = (reference_expiry, reference_strike) {
        return Ok((get_surface_vol(&surface_t1, expiry, strike)?
            - get_surface_vol(&surface_t0, expiry, strike)?)
            * 100.0);
    }
    let Some(&strike) = surface_t0.strikes().get(surface_t0.strikes().len() / 2) else {
        return Ok(0.0);
    };
    let mut total = 0.0;
    let mut count = 0usize;
    for &expiry in surface_t0.expiries().iter().filter(|expiry| **expiry > 0.0) {
        total += (get_surface_vol(&surface_t1, expiry, strike)?
            - get_surface_vol(&surface_t0, expiry, strike)?)
            * 100.0;
        count += 1;
    }
    if count == 0 {
        Ok(0.0)
    } else {
        Ok(total / count as f64)
    }
}

#[allow(clippy::too_many_arguments)]
fn interpolate_surface(
    surface: &VolSurface,
    expiry: f64,
    e0: usize,
    e1: usize,
    s0: usize,
    s1: usize,
    t: f64,
    u: f64,
) -> Result<f64> {
    let columns = surface.strikes().len();
    let values = surface.vols();
    let q00 = values[e0 * columns + s0];
    let q10 = values[e1 * columns + s0];
    let q01 = values[e0 * columns + s1];
    let q11 = values[e1 * columns + s1];
    match surface.interpolation_mode() {
        VolInterpolationMode::Vol => Ok(bilinear(q00, q10, q01, q11, t, u)),
        VolInterpolationMode::TotalVariance => {
            let x0 = surface.expiries()[e0];
            let x1 = surface.expiries()[e1];
            let variance = bilinear(
                x0 * q00 * q00,
                x1 * q10 * q10,
                x0 * q01 * q01,
                x1 * q11 * q11,
                t,
                u,
            ) / expiry.max(f64::EPSILON);
            if !variance.is_finite() || variance < 0.0 {
                return Err(Error::Validation(format!(
                    "vol surface '{}' produced invalid total variance at expiry {expiry}",
                    surface.id()
                )));
            }
            Ok(variance.sqrt())
        }
    }
}

fn cube_params(cube: &VolCube, expiry: f64, tenor: f64) -> (SabrParameters, f64, f64) {
    let expiry = expiry.clamp(
        cube.expiries()[0],
        cube.expiries()[cube.expiries().len() - 1],
    );
    let tenor = tenor.clamp(cube.tenors()[0], cube.tenors()[cube.tenors().len() - 1]);
    let (e0, e1, t) = segment(cube.expiries(), expiry).unwrap_or((0, 0, 0.0));
    let (n0, n1, u) = segment(cube.tenors(), tenor).unwrap_or((0, 0, 0.0));
    let columns = cube.tenors().len();
    let index = |e: usize, n: usize| e * columns + n;
    let p00 = cube.params_at(e0, n0);
    let p10 = cube.params_at(e1, n0);
    let p01 = cube.params_at(e0, n1);
    let p11 = cube.params_at(e1, n1);
    let nearest = index(
        if t <= 0.5 { e0 } else { e1 },
        if u <= 0.5 { n0 } else { n1 },
    );
    let nearest_params = &cube.params()[nearest];
    let shift = match (p00.shift, p10.shift, p01.shift, p11.shift) {
        (Some(q00), Some(q10), Some(q01), Some(q11)) => Some(bilinear(q00, q10, q01, q11, t, u)),
        _ => nearest_params.shift,
    };
    let params = SabrParameters {
        alpha: bilinear(p00.alpha, p10.alpha, p01.alpha, p11.alpha, t, u).max(1e-8),
        beta: nearest_params.beta.clamp(0.0, 1.0),
        nu: bilinear(p00.nu, p10.nu, p01.nu, p11.nu, t, u).max(1e-8),
        rho: bilinear(p00.rho, p10.rho, p01.rho, p11.rho, t, u).clamp(-0.9999, 0.9999),
        shift,
    };
    let forward = bilinear(
        cube.forwards()[index(e0, n0)],
        cube.forwards()[index(e1, n0)],
        cube.forwards()[index(e0, n1)],
        cube.forwards()[index(e1, n1)],
        t,
        u,
    );
    (params, forward, expiry)
}

fn cube_vol(cube: &VolCube, expiry: f64, tenor: f64, strike: f64, normal: bool) -> Result<f64> {
    if !strike.is_finite() {
        return Err(InputError::Invalid.into());
    }
    let (params, forward, model_expiry) = cube_params(cube, expiry, tenor);
    if normal {
        params.implied_vol_normal(forward, strike, model_expiry)
    } else {
        params.implied_vol_lognormal(forward, strike, model_expiry)
    }
}

/// Evaluate checked lognormal SABR cube volatility.
///
/// # Arguments
///
/// * `cube` - Structurally validated SABR parameter and forward grid.
/// * `expiry` - Option expiry in years within the stored expiry axis.
/// * `tenor` - Underlying tenor in years within the stored tenor axis.
/// * `strike` - Strike in the same decimal-rate units as the cube forwards.
///
/// # Errors
///
/// Returns an input error for non-finite or out-of-grid coordinates and a
/// validation error when the interpolated SABR expansion is undefined.
pub fn get_cube_vol(cube: &VolCube, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
    segment(cube.expiries(), expiry)?;
    segment(cube.tenors(), tenor)?;
    if cube.interpolation_mode() == VolInterpolationMode::TotalVariance {
        return cube_total_variance(cube, expiry, tenor, strike, false);
    }
    cube_vol(cube, expiry, tenor, strike, false)
}

/// Evaluate clamped lognormal SABR cube volatility.
///
/// # Arguments
///
/// * `cube` - Structurally validated SABR parameter and forward grid.
/// * `expiry` - Option expiry in years; finite values are clamped to the grid.
/// * `tenor` - Underlying tenor in years; finite values are clamped to the grid.
/// * `strike` - Strike in the same decimal-rate units as the cube forwards.
///
/// Non-finite inputs or an invalid SABR expansion return `NaN`.
pub fn get_cube_vol_clamped(cube: &VolCube, expiry: f64, tenor: f64, strike: f64) -> f64 {
    if !expiry.is_finite() || !tenor.is_finite() || !strike.is_finite() {
        return f64::NAN;
    }
    let expiry = expiry.clamp(
        cube.expiries()[0],
        cube.expiries()[cube.expiries().len() - 1],
    );
    let tenor = tenor.clamp(cube.tenors()[0], cube.tenors()[cube.tenors().len() - 1]);
    let result = if cube.interpolation_mode() == VolInterpolationMode::TotalVariance {
        cube_total_variance(cube, expiry, tenor, strike, false)
    } else {
        cube_vol(cube, expiry, tenor, strike, false)
    };
    result.unwrap_or(f64::NAN)
}

/// Evaluate checked normal SABR cube volatility.
///
/// # Arguments
///
/// * `cube` - Structurally validated SABR parameter and forward grid.
/// * `expiry` - Option expiry in years within the stored expiry axis.
/// * `tenor` - Underlying tenor in years within the stored tenor axis.
/// * `strike` - Strike in the same decimal-rate units as the cube forwards.
///
/// # Errors
///
/// Returns an input error for non-finite or out-of-grid coordinates and a
/// validation error when the interpolated normal SABR expansion is undefined.
pub fn get_cube_normal_vol(cube: &VolCube, expiry: f64, tenor: f64, strike: f64) -> Result<f64> {
    segment(cube.expiries(), expiry)?;
    segment(cube.tenors(), tenor)?;
    if cube.interpolation_mode() == VolInterpolationMode::TotalVariance {
        return cube_total_variance(cube, expiry, tenor, strike, true);
    }
    cube_vol(cube, expiry, tenor, strike, true)
}

/// Evaluate clamped normal SABR cube volatility.
///
/// # Arguments
///
/// * `cube` - Structurally validated SABR parameter and forward grid.
/// * `expiry` - Option expiry in years; finite values are clamped to the grid.
/// * `tenor` - Underlying tenor in years; finite values are clamped to the grid.
/// * `strike` - Strike in the same decimal-rate units as the cube forwards.
///
/// Non-finite inputs or an invalid SABR expansion return `NaN`.
pub fn get_cube_normal_vol_clamped(cube: &VolCube, expiry: f64, tenor: f64, strike: f64) -> f64 {
    if !expiry.is_finite() || !tenor.is_finite() || !strike.is_finite() {
        return f64::NAN;
    }
    let expiry = expiry.clamp(
        cube.expiries()[0],
        cube.expiries()[cube.expiries().len() - 1],
    );
    let tenor = tenor.clamp(cube.tenors()[0], cube.tenors()[cube.tenors().len() - 1]);
    let (params, forward, _) = cube_params(cube, expiry, tenor);
    let shift = params.shift.unwrap_or(0.0);
    if SabrModel::new(params.clone()).vol_type() != SabrVolType::Normal
        && (forward + shift <= 0.0 || strike + shift <= 0.0)
    {
        return f64::NAN;
    }
    let result = if cube.interpolation_mode() == VolInterpolationMode::TotalVariance {
        cube_total_variance(cube, expiry, tenor, strike, true)
    } else {
        params.implied_vol_normal(forward, strike, expiry)
    };
    result.unwrap_or(f64::NAN)
}

/// Materialize a lognormal cube tenor slice as a core surface artifact.
///
/// # Arguments
///
/// * `cube` - Source SABR cube.
/// * `tenor` - Underlying tenor in years; evaluation clamps it to the cube axis.
/// * `strikes` - Non-empty finite strike grid in the cube forward's units.
///
/// # Errors
///
/// Returns an input or structural-validation error when the requested grid
/// cannot form a valid surface artifact.
pub fn materialize_cube_tenor_slice(
    cube: &VolCube,
    tenor: f64,
    strikes: &[f64],
) -> Result<VolSurface> {
    materialize_cube_tenor_slice_with_convention(cube, tenor, strikes, false)
}

/// Materialize a normal cube tenor slice as a core surface artifact.
///
/// # Arguments
///
/// * `cube` - Source SABR cube.
/// * `tenor` - Underlying tenor in years; evaluation clamps it to the cube axis.
/// * `strikes` - Non-empty finite strike grid in the cube forward's units.
///
/// # Errors
///
/// Returns an input or structural-validation error when the requested grid
/// cannot form a valid normal-volatility surface artifact.
pub fn materialize_cube_tenor_slice_normal(
    cube: &VolCube,
    tenor: f64,
    strikes: &[f64],
) -> Result<VolSurface> {
    materialize_cube_tenor_slice_with_convention(cube, tenor, strikes, true)
}

fn materialize_cube_tenor_slice_with_convention(
    cube: &VolCube,
    tenor: f64,
    strikes: &[f64],
    normal: bool,
) -> Result<VolSurface> {
    if strikes.is_empty() || !tenor.is_finite() || strikes.iter().any(|value| !value.is_finite()) {
        return Err(InputError::Invalid.into());
    }
    let mut values = Vec::with_capacity(cube.expiries().len() * strikes.len());
    for &expiry in cube.expiries() {
        for &strike in strikes {
            values.push(if normal {
                get_cube_normal_vol_clamped(cube, expiry, tenor, strike)
            } else {
                get_cube_vol_clamped(cube, expiry, tenor, strike)
            });
        }
    }
    let surface = VolSurface::from_grid(cube.id().as_str(), cube.expiries(), strikes, &values)?
        .with_interpolation_mode(cube.interpolation_mode());
    Ok(if normal {
        surface.with_quote_type(VolQuoteType::Normal)
    } else {
        surface
    })
}

/// Materialize a lognormal cube expiry slice as a tenor-axis core surface.
///
/// # Arguments
///
/// * `cube` - Source SABR cube.
/// * `expiry` - Option expiry in years; evaluation clamps it to the cube axis.
/// * `strikes` - Non-empty finite strike grid in the cube forward's units.
///
/// # Errors
///
/// Returns an input or structural-validation error when the requested grid
/// cannot form a valid tenor-axis surface artifact.
pub fn materialize_cube_expiry_slice(
    cube: &VolCube,
    expiry: f64,
    strikes: &[f64],
) -> Result<VolSurface> {
    materialize_cube_expiry_slice_with_convention(cube, expiry, strikes, false)
}

/// Materialize a normal cube expiry slice as a tenor-axis core surface.
///
/// # Arguments
///
/// * `cube` - Source SABR cube.
/// * `expiry` - Option expiry in years; evaluation clamps it to the cube axis.
/// * `strikes` - Non-empty finite strike grid in the cube forward's units.
///
/// # Errors
///
/// Returns an input or structural-validation error when the requested grid
/// cannot form a valid normal-volatility tenor-axis surface artifact.
pub fn materialize_cube_expiry_slice_normal(
    cube: &VolCube,
    expiry: f64,
    strikes: &[f64],
) -> Result<VolSurface> {
    materialize_cube_expiry_slice_with_convention(cube, expiry, strikes, true)
}

fn materialize_cube_expiry_slice_with_convention(
    cube: &VolCube,
    expiry: f64,
    strikes: &[f64],
    normal: bool,
) -> Result<VolSurface> {
    if strikes.is_empty() || !expiry.is_finite() || strikes.iter().any(|value| !value.is_finite()) {
        return Err(InputError::Invalid.into());
    }
    let mut values = Vec::with_capacity(cube.tenors().len() * strikes.len());
    for &tenor in cube.tenors() {
        for &strike in strikes {
            values.push(if normal {
                get_cube_normal_vol_clamped(cube, expiry, tenor, strike)
            } else {
                get_cube_vol_clamped(cube, expiry, tenor, strike)
            });
        }
    }
    let surface = VolSurface::from_grid(cube.id().as_str(), cube.tenors(), strikes, &values)?
        .with_secondary_axis(VolSurfaceAxis::Tenor);
    Ok(if normal {
        surface.with_quote_type(VolQuoteType::Normal)
    } else {
        surface
    })
}

/// Materialize the full lognormal cube in expiry-tenor-strike order.
///
/// # Arguments
///
/// * `cube` - Source SABR cube whose stored axes define the first two dimensions.
/// * `strikes` - Non-empty finite strike grid in the cube forward's units.
///
/// # Errors
///
/// Returns an input error when `strikes` is empty or contains a non-finite value.
pub fn materialize_cube_grid(cube: &VolCube, strikes: &[f64]) -> Result<Vec<f64>> {
    if strikes.is_empty() || strikes.iter().any(|value| !value.is_finite()) {
        return Err(InputError::Invalid.into());
    }
    let mut values =
        Vec::with_capacity(cube.expiries().len() * cube.tenors().len() * strikes.len());
    for &expiry in cube.expiries() {
        for &tenor in cube.tenors() {
            for &strike in strikes {
                values.push(get_cube_vol_clamped(cube, expiry, tenor, strike));
            }
        }
    }
    Ok(values)
}

fn cube_total_variance(
    cube: &VolCube,
    expiry: f64,
    tenor: f64,
    strike: f64,
    normal: bool,
) -> Result<f64> {
    let (e0, e1, weight) = segment(cube.expiries(), expiry)?;
    let t0 = cube.expiries()[e0];
    let t1 = cube.expiries()[e1];
    let v0 = cube_vol(cube, t0, tenor, strike, normal)?;
    if e0 == e1 {
        return Ok(v0);
    }
    let v1 = cube_vol(cube, t1, tenor, strike, normal)?;
    let total = (1.0 - weight) * t0 * v0 * v0 + weight * t1 * v1 * v1;
    if !total.is_finite() || total <= 0.0 {
        return Err(Error::Validation(format!(
            "vol cube '{}' produced invalid total variance at expiry {expiry}",
            cube.id()
        )));
    }
    Ok((total / expiry).sqrt())
}
