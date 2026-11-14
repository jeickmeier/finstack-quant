//! Valuations-owned resolution of computational volatility sources.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_models::volatility::VolSource;

/// Resolve the unique computational volatility source registered under `id`.
///
/// # Arguments
///
/// * `market` - Market context containing observed volatility artifacts.
/// * `id` - Identifier in one of `vol_cubes`, `surfaces`, or `fx_delta_vol_surfaces`.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if `id` is registered in
/// more than one volatility registry, or the market-context missing-data error
/// when no volatility artifact exists under `id`.
pub fn resolve_vol_source(
    market: &MarketContext,
    id: impl AsRef<str>,
) -> finstack_quant_core::Result<VolSource> {
    let id = id.as_ref();
    let cube = market.get_vol_cube(id).ok();
    let surface = market.get_surface(id).ok();
    let fx_delta = market.get_fx_delta_vol_surface(id).ok();
    let count = usize::from(cube.is_some())
        + usize::from(surface.is_some())
        + usize::from(fx_delta.is_some());
    if count > 1 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Ambiguous volatility source id '{id}': registered in multiple volatility registries"
        )));
    }
    if let Some(cube) = cube {
        return Ok(VolSource::Cube(cube));
    }
    if let Some(surface) = surface {
        return Ok(VolSource::Surface(surface));
    }
    if let Some(surface) = fx_delta {
        return Ok(VolSource::FxDelta(surface));
    }
    market.get_surface(id).map(VolSource::Surface)
}
