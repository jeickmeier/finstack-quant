//! Shared helpers for resolving volatility from overrides or a vol surface.
//!
//! The canonical pattern across surface-driven pricers is to first check whether
//! `MarketQuoteOverrides::implied_volatility` is set and, if so, use it as a flat
//! σ across tenor and strike. Otherwise, look up the surface at `(t, strike)`
//! with clamping at the grid edges.
//!
//! This module centralises that pattern so new pricers can opt in with a single
//! call and existing pricers can be migrated away from ad-hoc `if let Some(iv)`
//! blocks.

use crate::instruments::pricing_overrides::MarketQuoteOverrides;
use crate::market::resolve_vol_source;
use finstack_quant_core::market_data::context::MarketContext;

use finstack_quant_core::{Error, Result};
use finstack_quant_models::volatility::VolatilityConvention;

/// Coordinates and pricing convention for one volatility resolution.
pub(crate) struct VolatilityRequest {
    pub expiry: f64,
    pub tenor: f64,
    pub strike: f64,
    /// None selects source metadata. Explicit models retain their quote units.
    pub convention: Option<VolatilityConvention>,
    pub clamp: bool,
}

/// One validated quote together with the convention needed by prices and Greeks.
#[derive(Clone, Copy)]
pub(crate) struct ResolvedVolatility {
    pub sigma: f64,
    pub convention: VolatilityConvention,
}

impl ResolvedVolatility {
    /// Validate and transform both model coordinates using the resolved shift.
    pub(crate) fn model_rates(self, forward: f64, strike: f64) -> Result<(f64, f64)> {
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

pub(crate) fn validate_sigma(sigma: f64) -> Result<f64> {
    if sigma.is_finite() && sigma >= 0.0 {
        Ok(sigma)
    } else {
        Err(Error::Validation(format!(
            "volatility must be finite and non-negative, got {sigma}"
        )))
    }
}

/// Resolve the active quote, retaining units and displacement throughout pricing.
pub(crate) fn resolve_volatility(
    overrides: &MarketQuoteOverrides,
    curves: &MarketContext,
    vol_surface_id: &str,
    request: VolatilityRequest,
) -> Result<ResolvedVolatility> {
    let source = if overrides.implied_volatility.is_some() && request.convention.is_some() {
        // An explicit scalar/model pair needs no inactive surface. If a cube is
        // present, its displacement remains part of the model definition.
        resolve_vol_source(curves, vol_surface_id).ok()
    } else {
        Some(resolve_vol_source(curves, vol_surface_id)?)
    };
    let source_convention = source
        .as_ref()
        .map(|source| source.get_convention(request.expiry, request.tenor))
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
    {
        if (shift - source_shift).abs() > 1e-12 {
            return Err(Error::Validation(format!("configured volatility displacement {shift} differs from source displacement {source_shift}")));
        }
    }
    let sigma = if let Some(sigma) = overrides.implied_volatility {
        sigma
    } else {
        let source = source
            .as_ref()
            .ok_or_else(|| Error::internal("volatility source missing"))?;
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

/// Resolve unshifted Black volatility for equity/FX-style surface consumers.
#[inline]
pub(crate) fn resolve_sigma_at(
    overrides: &MarketQuoteOverrides,
    curves: &MarketContext,
    vol_surface_id: &str,
    t: f64,
    strike: f64,
) -> Result<f64> {
    let resolved = resolve_volatility(
        overrides,
        curves,
        vol_surface_id,
        VolatilityRequest {
            expiry: t,
            tenor: 0.0,
            strike,
            convention: Some(VolatilityConvention::Lognormal),
            clamp: true,
        },
    )?;
    if resolved.convention != VolatilityConvention::Lognormal {
        return Err(Error::Validation(
            "unshifted Black consumer cannot use a displaced volatility source".to_owned(),
        ));
    }
    Ok(resolved.sigma)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::surfaces::VolSurface;

    #[test]
    fn explicit_implied_volatility_wins_without_surface_lookup() {
        let overrides = MarketQuoteOverrides {
            implied_volatility: Some(0.31),
            ..Default::default()
        };

        let sigma = resolve_sigma_at(&overrides, &MarketContext::new(), "MISSING-VOL", 1.0, 100.0)
            .expect("explicit vol should not require a surface");

        assert_eq!(sigma, 0.31);
    }

    #[test]
    fn missing_implied_volatility_interpolates_from_surface() {
        let surface = VolSurface::builder("EQ-VOL")
            .expiries(&[1.0])
            .strikes(&[90.0, 110.0])
            .row(&[0.20, 0.30])
            .build()
            .expect("surface");
        let market = MarketContext::new().insert_surface(surface);

        let sigma = resolve_sigma_at(
            &MarketQuoteOverrides::default(),
            &market,
            "EQ-VOL",
            1.0,
            100.0,
        )
        .expect("surface vol should resolve");

        assert!(
            (sigma - 0.25).abs() < 1e-12,
            "expected interpolated surface vol, got {sigma}"
        );
    }
}
