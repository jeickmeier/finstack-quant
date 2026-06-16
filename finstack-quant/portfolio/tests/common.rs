//! Common test utilities and fixtures for portfolio integration tests.
//!
//! This module provides shared testing infrastructure including helper functions
//! for creating market contexts, discount curves, and FX providers used across
//! multiple test files.
//!
//! Note: Functions are marked `#[allow(dead_code)]` because each integration test
//! file compiles `common.rs` separately, and not all tests use all helpers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use std::sync::Arc;
use time::macros::date;

/// Standard base date used across portfolio integration tests.
pub fn base_date() -> Date {
    date!(2024 - 01 - 01)
}

// =============================================================================
// Discount Curves
// =============================================================================

/// Build a flat USD discount curve (DF=1.0 at all tenors).
fn usd_curve() -> DiscountCurve {
    DiscountCurve::builder("USD")
        .base_date(base_date())
        .knots(vec![(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("flat USD curve should build")
}

/// Build a flat EUR discount curve (DF=1.0 at all tenors).
fn eur_curve() -> DiscountCurve {
    DiscountCurve::builder("EUR")
        .base_date(base_date())
        .knots(vec![(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("flat EUR curve should build")
}

/// Create a USD discount curve with a flat rate in basis points.
///
/// DF(t) = exp(-rate * t) where rate = bp / 10000
#[allow(dead_code)]
pub fn usd_curve_at_rate(rate_bp: f64) -> DiscountCurve {
    let rate = rate_bp / 10000.0;
    let mut builder = DiscountCurve::builder("USD")
        .base_date(base_date())
        .knots(vec![
            (0.0, 1.0),
            (1.0, (-rate * 1.0_f64).exp()),
            (5.0, (-rate * 5.0_f64).exp()),
        ])
        .interp(InterpStyle::Linear);

    // For flat or near-zero rates, discount factors may be non-monotonic
    if rate_bp.abs() < 1.0 {
        builder = builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().expect("USD curve at rate should build")
}

// =============================================================================
// Market Contexts
// =============================================================================

/// Create a market context with a flat USD discount curve.
#[allow(dead_code)]
pub fn market_with_usd() -> MarketContext {
    MarketContext::new().insert(usd_curve())
}

/// Create a market context with USD curve at a specific rate level (in basis points).
#[allow(dead_code)]
pub fn market_with_usd_at_rate(rate_bp: f64) -> MarketContext {
    MarketContext::new().insert(usd_curve_at_rate(rate_bp))
}

/// Create a market context with a flat EUR discount curve.
#[allow(dead_code)]
pub fn market_with_eur() -> MarketContext {
    MarketContext::new().insert(eur_curve())
}

/// Create a market context with EUR curve and an FX matrix at the given rate.
#[allow(dead_code)]
pub fn market_with_eur_and_fx(rate: f64) -> MarketContext {
    market_with_eur().insert_fx(fx_matrix(rate))
}

// =============================================================================
// FX Infrastructure
// =============================================================================

/// Simple FX provider that returns a static rate for any currency pair.
pub struct StaticFx {
    /// Static exchange rate returned for every requested currency pair.
    pub rate: f64,
}

impl FxProvider for StaticFx {
    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        Ok(self.rate)
    }
}

/// Build an FX matrix with a static rate provider.
fn fx_matrix(rate: f64) -> FxMatrix {
    FxMatrix::new(Arc::new(StaticFx { rate }))
}
