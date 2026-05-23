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
use finstack_core::market_data::context::MarketContext;

/// Resolve the volatility σ to use at `(t, strike)` for a surface-driven pricer.
///
/// Precedence:
///
/// 1. `overrides.implied_volatility` — when set, interpreted as a flat σ across
///    tenor and strike (standard revaluation convention).
/// 2. Surface lookup via `curves.get_surface(surface_id).value_clamped(t, strike)`.
///
/// Use this in every pricer that previously wrote the inline
/// `if let Some(iv) = overrides.implied_volatility { iv } else { surface.value_clamped(t, K) }`
/// pattern.
#[inline]
pub(crate) fn resolve_sigma_at(
    overrides: &MarketQuoteOverrides,
    curves: &MarketContext,
    surface_id: &str,
    t: f64,
    strike: f64,
) -> finstack_core::Result<f64> {
    if let Some(iv) = overrides.implied_volatility {
        return Ok(iv);
    }
    Ok(curves.get_surface(surface_id)?.value_clamped(t, strike))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::surfaces::VolSurface;

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
