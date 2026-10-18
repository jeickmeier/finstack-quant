//! Shared builders for test market contexts.
//!
//! These builders reduce boilerplate across instrument and metric tests by
//! providing consistent default configurations for common test scenarios.

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;

// Market Context Builders

/// Builder for creating test market contexts with common configurations.
pub struct TestMarketBuilder {
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: Option<f64>,
    discount_curve_id: String,
    vol_surface_id: String,
    spot_id: String,
    div_yield_id: Option<String>,
    tenor_years: f64,
}

impl TestMarketBuilder {
    /// Create a new test market builder.
    pub fn new(as_of: Date) -> Self {
        Self {
            as_of,
            spot: 100.0,
            vol: 0.25,
            rate: 0.05,
            div_yield: None,
            discount_curve_id: "USD-OIS".to_string(),
            vol_surface_id: "SPOT_VOL".to_string(),
            spot_id: "SPOT".to_string(),
            div_yield_id: None,
            tenor_years: 2.0,
        }
    }

    /// Set the spot price.
    pub fn spot(mut self, spot: f64) -> Self {
        self.spot = spot;
        self
    }

    /// Set the volatility level (flat across surface).
    pub fn vol(mut self, vol: f64) -> Self {
        self.vol = vol;
        self
    }

    /// Set the risk-free rate.
    pub fn rate(mut self, rate: f64) -> Self {
        self.rate = rate;
        self
    }

    /// Set the dividend yield.
    pub fn div_yield(mut self, div_yield: f64) -> Self {
        self.div_yield = Some(div_yield);
        self.div_yield_id = Some(format!("{}_DIV", self.spot_id));
        self
    }

    /// Set the discount curve ID.
    pub fn discount_curve_id(mut self, id: &str) -> Self {
        self.discount_curve_id = id.to_string();
        self
    }

    /// Set the volatility surface ID.
    pub fn vol_surface_id(mut self, id: &str) -> Self {
        self.vol_surface_id = id.to_string();
        self
    }

    /// Set the spot price scalar ID.
    pub fn spot_id(mut self, id: &str) -> Self {
        self.spot_id = id.to_string();
        if self.div_yield.is_some() {
            self.div_yield_id = Some(format!("{}_DIV", id));
        }
        self
    }

    /// Set the far tenor for the discount curve (default: 2.0 years).
    pub fn tenor_years(mut self, years: f64) -> Self {
        self.tenor_years = years;
        self
    }

    /// Build the market context.
    pub fn build(self) -> MarketContext {
        let disc_curve = DiscountCurve::builder(self.discount_curve_id.as_str())
            .base_date(self.as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0_f64, 1.0_f64),
                (1.0_f64, (-self.rate).exp()),
                (self.tenor_years, (-self.rate * self.tenor_years).exp()),
            ])
            .build()
            .expect("discount curve should build in tests");

        let vol_surface = VolSurface::builder(self.vol_surface_id.as_str())
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0])
            .row(&[self.vol, self.vol, self.vol])
            .row(&[self.vol, self.vol, self.vol])
            .row(&[self.vol, self.vol, self.vol])
            .build()
            .expect("vol surface should build in tests");

        let mut market = MarketContext::new()
            .insert(disc_curve)
            .insert_surface(vol_surface)
            .insert_price(&self.spot_id, MarketScalar::Unitless(self.spot));

        if let (Some(div), Some(div_id)) = (self.div_yield, &self.div_yield_id) {
            market = market.insert_price(div_id, MarketScalar::Unitless(div));
        }

        market
    }
}

/// Create a simple test market with the given parameters.
///
/// This is a convenience function for simple test cases that don't need
/// the full flexibility of `TestMarketBuilder`.
///
/// # Arguments
///
/// * `as_of` - Valuation date; base date of the `USD-OIS` curve and `SPOT_VOL` surface.
/// * `spot` - Spot price stored under the `SPOT` scalar, in the option's quote units.
/// * `vol` - Flat annualised volatility, as a decimal, written across the surface.
/// * `rate` - Continuously compounded flat discount rate, as a decimal.
pub fn simple_option_market(as_of: Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
    TestMarketBuilder::new(as_of)
        .spot(spot)
        .vol(vol)
        .rate(rate)
        .build()
}

/// Create a test market with dividend yield.
///
/// # Arguments
///
/// * `as_of` - Valuation date; base date of the `USD-OIS` curve and `SPOT_VOL` surface.
/// * `spot` - Spot price stored under the `SPOT` scalar, in the option's quote units.
/// * `vol` - Flat annualised volatility, as a decimal, written across the surface.
/// * `rate` - Continuously compounded flat discount rate, as a decimal.
/// * `div_yield` - Continuous dividend yield, as a decimal, stored under the `SPOT_DIV` scalar.
pub fn option_market_with_divs(
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: f64,
) -> MarketContext {
    TestMarketBuilder::new(as_of)
        .spot(spot)
        .vol(vol)
        .rate(rate)
        .div_yield(div_yield)
        .build()
}

/// Create a test market context via builder.
///
/// # Arguments
///
/// * `as_of` - Valuation date every curve and surface the builder emits is based on.
pub fn test_market(as_of: Date) -> TestMarketBuilder {
    TestMarketBuilder::new(as_of)
}
