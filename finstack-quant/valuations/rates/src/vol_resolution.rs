//! Interest-rate option volatility models and quote resolution.

use finstack_quant_core::dates::VolatilityClock;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::volatility::{VolSource, VolatilityConvention};
use finstack_quant_valuations_core::instruments::pricing::volatility::validate_sigma;
use finstack_quant_valuations_core::instruments::pricing_overrides::MarketQuoteOverrides;
use finstack_quant_valuations_core::market::resolve_vol_source;

/// Coordinates and pricing convention for one volatility resolution.
pub struct VolatilityRequest {
    /// Positive option time in years.
    pub expiry: f64,
    /// Underlying contract tenor in years when required by the selected source.
    pub tenor: f64,
    /// Strike in the source's quoted units.
    pub strike: f64,
    /// `None` selects source metadata. Explicit models retain their quote units.
    pub convention: Option<VolatilityConvention>,
    /// Whether to use the source's clamped lookup.
    pub clamp: bool,
    /// Required expiry clock for a dated, source-backed SABR cube lookup.
    /// The cube must declare the same reference date, day count and context.
    /// Numeric lookups omit this requirement.
    pub required_cube_clock: Option<VolatilityClock>,
}

/// One validated quote together with the convention needed by prices and Greeks.
#[derive(Clone, Copy)]
pub struct ResolvedVolatility {
    /// Validated implied volatility magnitude in the quote convention.
    pub sigma: f64,
    /// Resolved quote convention (lognormal, shifted lognormal, or normal).
    pub convention: VolatilityConvention,
}

impl ResolvedVolatility {
    /// Validate and transform both model coordinates using the resolved shift.
    ///
    /// # Arguments
    ///
    /// * `forward` - Forward swap or caplet rate in decimal units.
    /// * `strike` - Contractual strike rate in decimal units.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if either input is non-finite or if the displaced
    /// forward or strike is non-positive under a lognormal convention.
    pub fn model_rates(self, forward: f64, strike: f64) -> Result<(f64, f64)> {
        if !forward.is_finite() || !strike.is_finite() {
            return Err(Error::Validation(
                "option forward and strike must be finite".to_owned(),
            ));
        }
        let shift = match self.convention {
            VolatilityConvention::Normal => return Ok((forward, strike)),
            VolatilityConvention::Lognormal => 0.0,
            VolatilityConvention::ShiftedLognormal { shift } => shift,
        };
        let (forward, strike) = (forward + shift, strike + shift);
        if !shift.is_finite() || forward <= 0.0 || strike <= 0.0 {
            return Err(Error::Validation(format!(
                "Black pricing requires positive forward and strike after displacement {shift}: forward={forward}, strike={strike}"
            )));
        }
        Ok((forward, strike))
    }
}

/// Resolve the active quote, retaining units and displacement throughout pricing.
///
/// # Arguments
///
/// * `overrides` - Optional explicit implied volatility in the selected quote convention.
/// * `curves` - Market context containing computational volatility sources.
/// * `vol_surface_id` - Identifier used to resolve the active volatility source.
/// * `request` - Model coordinates, convention and lookup policy. Dated SABR
///   consumers also specify their required source expiry clock.
///
/// # Errors
///
/// Returns an error when the source cannot be resolved, the quote conventions
/// or required cube clock disagree, or the selected quote is invalid.
pub fn resolve_volatility(
    overrides: &MarketQuoteOverrides,
    curves: &MarketContext,
    vol_surface_id: &str,
    request: VolatilityRequest,
) -> Result<ResolvedVolatility> {
    let has_cube_tenor = request.tenor.is_finite() && request.tenor > 0.0;
    let source = if overrides.implied_volatility.is_some() && request.convention.is_some() {
        // A cube can supply displacement only when the product has a tenor.
        resolve_vol_source(curves, vol_surface_id)
            .ok()
            .filter(|source| has_cube_tenor || !matches!(source, VolSource::Cube(_)))
    } else {
        Some(resolve_vol_source(curves, vol_surface_id)?)
    };
    if let (Some(VolSource::Cube(cube)), Some(required_clock)) =
        (&source, &request.required_cube_clock)
        && overrides.implied_volatility.is_none()
    {
        let clock = cube.expiry_clock().ok_or_else(|| {
            Error::Validation(format!(
                "SABR cube '{}' requires an explicit expiry_clock for dated valuation",
                cube.id()
            ))
        })?;
        if clock != required_clock {
            return Err(Error::Validation(format!(
                "SABR cube '{}' expiry_clock {clock:?} does not match the required model clock {required_clock:?}",
                cube.id()
            )));
        }
    }
    let source_convention = source
        .as_ref()
        .map(|source| {
            if matches!(source, VolSource::Cube(_)) {
                if !has_cube_tenor {
                    return Err(Error::Validation(format!(
                        "SABR cube '{}' requires a finite positive underlying tenor",
                        source.get_id()
                    )));
                }
                let requested = request.convention.ok_or_else(|| {
                    Error::Validation("SABR cube requires explicit output quote convention".into())
                })?;
                source.get_cube_convention(request.expiry, request.tenor, requested, request.clamp)
            } else {
                source.get_convention(request.expiry, request.tenor)
            }
        })
        .transpose()?;
    let mut convention = request.convention.or(source_convention).ok_or_else(|| {
        Error::Validation("Auto volatility requires source convention metadata".to_owned())
    })?;
    if let (
        VolatilityConvention::Lognormal,
        Some(VolatilityConvention::ShiftedLognormal { shift }),
    ) = (convention, source_convention)
    {
        convention = VolatilityConvention::ShiftedLognormal { shift };
    }
    if let (
        VolatilityConvention::ShiftedLognormal { shift },
        Some(VolatilityConvention::ShiftedLognormal {
            shift: source_shift,
        }),
    ) = (convention, source_convention)
        && (shift - source_shift).abs() > 1e-12
    {
        return Err(Error::Validation(format!(
            "configured volatility displacement {shift} differs from source displacement {source_shift}"
        )));
    }
    let sigma = if let Some(sigma) = overrides.implied_volatility {
        sigma
    } else {
        let source = source
            .as_ref()
            .ok_or_else(|| Error::Internal("volatility source missing".into()))?;
        match (convention, request.clamp) {
            (VolatilityConvention::Normal, true) => {
                source.get_normal_vol_clamped(request.expiry, request.tenor, request.strike)?
            }
            (VolatilityConvention::Normal, false) => {
                source.get_normal_vol(request.expiry, request.tenor, request.strike)?
            }
            (_, true) => source.get_vol_clamped(request.expiry, request.tenor, request.strike)?,
            (_, false) => source.get_vol(request.expiry, request.tenor, request.strike)?,
        }
    };
    Ok(ResolvedVolatility {
        sigma: validate_sigma(sigma)?,
        convention,
    })
}
