//! Valuations-owned resolution of computational volatility sources.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{ErrorKind, InputError};
use finstack_quant_models::volatility::VolSource;

/// Map a registry lookup miss (`ErrorKind::NotFound`) to `Ok(None)` while
/// immediately propagating every other error.
fn probe_not_found<T>(
    result: finstack_quant_core::Result<T>,
) -> finstack_quant_core::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

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
/// more than one volatility registry, or [`InputError::NotFound`] when no
/// volatility artifact exists under `id`. Any non-`NotFound` lookup error is
/// propagated immediately.
pub fn resolve_vol_source(
    market: &MarketContext,
    id: impl AsRef<str>,
) -> finstack_quant_core::Result<VolSource> {
    let id = id.as_ref();
    let cube = probe_not_found(market.get_vol_cube(id))?;
    let surface = probe_not_found(market.get_surface(id))?;
    let fx_delta = probe_not_found(market.get_fx_delta_vol_surface(id))?;
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
    Err(InputError::NotFound { id: id.to_string() }.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::surfaces::{
        FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
        SabrParameterData, VolCube, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_core::money::fx::CurrencyPair;

    fn test_surface(id: &str) -> VolSurface {
        VolSurface::builder(id)
            .expiries(&[1.0, 2.0])
            .secondary_values(&[90.0, 100.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.2, 0.2])
            .row(&[0.2, 0.2])
            .build()
            .expect("valid surface")
    }

    fn test_fx_delta_surface(id: &str) -> FxDeltaVolSurface {
        let pair = CurrencyPair::try_new(Currency::EUR, Currency::USD).expect("valid pair");
        FxDeltaVolSurface::builder(id, pair)
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.25, 1.0])
            .atm_vols(&[0.08, 0.09])
            .rr_25d(&[0.01, 0.012])
            .bf_25d(&[0.005, 0.006])
            .build()
            .expect("valid fx delta surface")
    }

    fn test_cube(id: &str) -> VolCube {
        let node = SabrParameterData::new(0.2, 0.5, 0.3, -0.2, None).expect("valid sabr node");
        VolCube::from_grid(id, &[1.0], &[5.0], &[node], &[0.03]).expect("valid cube")
    }

    #[test]
    fn probe_not_found_maps_only_not_found_to_none() {
        let missing: finstack_quant_core::Result<i32> =
            Err(InputError::NotFound { id: "X".into() }.into());
        assert_eq!(probe_not_found(missing).expect("not found -> None"), None);

        let validation: finstack_quant_core::Result<i32> =
            Err(finstack_quant_core::Error::Validation("corrupt".into()));
        let err = probe_not_found(validation).expect_err("non-NotFound must propagate");
        assert_eq!(err.kind(), ErrorKind::Validation);
    }

    #[test]
    fn resolve_vol_source_handles_unique_missing_and_ambiguous() {
        let mut market = MarketContext::new().insert_surface(test_surface("SURF-1"));
        market.insert_fx_delta_vol_surface_mut(test_fx_delta_surface("FX-1"));
        let market = market.insert_vol_cube(test_cube("CUBE-1"));

        assert!(matches!(
            resolve_vol_source(&market, "SURF-1").expect("surface"),
            VolSource::Surface(_)
        ));
        assert!(matches!(
            resolve_vol_source(&market, "FX-1").expect("fx delta"),
            VolSource::FxDelta(_)
        ));
        assert!(matches!(
            resolve_vol_source(&market, "CUBE-1").expect("cube"),
            VolSource::Cube(_)
        ));

        let missing = resolve_vol_source(&market, "MISSING").expect_err("missing id");
        assert_eq!(missing.kind(), ErrorKind::NotFound);

        let ambiguous = market.insert_surface(test_surface("CUBE-1"));
        let err = resolve_vol_source(&ambiguous, "CUBE-1").expect_err("ambiguous id");
        assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
    }
}
