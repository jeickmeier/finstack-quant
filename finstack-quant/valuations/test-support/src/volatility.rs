use finstack_quant_core::market_data::surfaces::VolSurface;

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
