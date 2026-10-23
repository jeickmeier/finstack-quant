use finstack_quant_core::market_data::surfaces::{VolGridOpts, VolQuoteType, VolSurface};

/// Copy of `surface` under a different quote convention.
///
/// Rebuilds the surface through [`VolSurface::from_grid`], keeping its axes,
/// grid, secondary axis and interpolation mode. Displacements carry over only
/// for shifted Black quotes.
///
/// # Arguments
///
/// * `surface` - Surface to relabel; its grid and metadata are copied.
/// * `quote_type` - Quote convention of the copy.
///
/// # Errors
///
/// Returns the validation error `VolSurface::from_grid` reports for an
/// incompatible combination, such as shifted Black without displacements.
pub fn relabel_quote_type(
    surface: &VolSurface,
    quote_type: VolQuoteType,
) -> finstack_quant_core::Result<VolSurface> {
    let displacements = if quote_type == VolQuoteType::ShiftedBlackLognormal {
        surface.displacements()
    } else {
        None
    };
    VolSurface::from_grid(
        surface.id().as_str(),
        surface.expiries(),
        surface.strikes(),
        surface.vols(),
        VolGridOpts {
            secondary_axis: surface.secondary_axis(),
            quote_type,
            interpolation_mode: surface.interpolation_mode(),
        },
        displacements,
    )
}

/// Build a constant vol surface using provided expiries/strikes grid.
///
/// # Arguments
///
/// * `id` - Surface identifier the market context is keyed by.
/// * `expiries` - Expiry grid in years from the valuation date, ascending.
/// * `strikes` - Strike grid in the surface's quote units, ascending.
/// * `vol` - Annualised volatility, as a decimal, written at every grid node.
pub fn flat_vol_surface(id: &str, expiries: &[f64], strikes: &[f64], vol: f64) -> VolSurface {
    let mut builder = VolSurface::builder(id).expiries(expiries).strikes(strikes);
    for _ in expiries {
        builder = builder.row(&vec![vol; strikes.len()]);
    }
    builder.build().expect("vol surface should build in tests")
}
