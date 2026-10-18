//! Valuations-owned resolution of computational volatility sources.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_models::volatility::VolSource;

/// Resolve a concrete volatility source with cube-first precedence.
///
/// # Arguments
///
/// * `market` - Market context containing observed volatility artifacts.
/// * `id` - Identifier shared by the cube, surface, or FX-delta registries.
///
/// # Errors
///
/// Returns the market-context missing-data error when no volatility artifact
/// exists under `id`.
pub fn resolve_vol_source(
    market: &MarketContext,
    id: impl AsRef<str>,
) -> finstack_quant_core::Result<VolSource> {
    let id = id.as_ref();
    if let Ok(cube) = market.get_vol_cube(id) {
        return Ok(VolSource::Cube(cube));
    }
    if let Ok(surface) = market.get_surface(id) {
        return Ok(VolSource::Surface(surface));
    }
    if let Ok(surface) = market.get_fx_delta_vol_surface(id) {
        return Ok(VolSource::FxDelta(surface));
    }
    market.get_surface(id).map(VolSource::Surface)
}
