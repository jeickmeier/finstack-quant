//! Shared builders for test market contexts.
//!
//! These builders reduce boilerplate across instrument and metric tests by
//! providing consistent default configurations for common test scenarios.

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;

// Market Context Builders

/// Builder for creating test market contexts with common configurations.
pub struct TestMarketBuilder {
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: Option<f64>,
}

/// Discount curve id of every [`TestMarketBuilder`] market.
const DISCOUNT_CURVE_ID: &str = "USD-OIS";
/// Volatility surface id of every [`TestMarketBuilder`] market.
const VOL_SURFACE_ID: &str = "SPOT_VOL";
/// Spot price scalar id of every [`TestMarketBuilder`] market.
const SPOT_ID: &str = "SPOT";
/// Dividend yield scalar id, set only when a dividend yield is given.
const DIV_YIELD_ID: &str = "SPOT_DIV";
/// Far knot of the discount curve, in years.
const TENOR_YEARS: f64 = 2.0;

impl TestMarketBuilder {
    /// Create a new test market builder.
    pub fn new(as_of: Date) -> Self {
        Self {
            as_of,
            spot: 100.0,
            vol: 0.25,
            rate: 0.05,
            div_yield: None,
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
        self
    }

    /// Build the market context.
    pub fn build(self) -> MarketContext {
        let disc_curve = DiscountCurve::builder(DISCOUNT_CURVE_ID)
            .base_date(self.as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0_f64, 1.0_f64),
                (1.0_f64, (-self.rate).exp()),
                (TENOR_YEARS, (-self.rate * TENOR_YEARS).exp()),
            ])
            .build()
            .expect("discount curve should build in tests");

        let vol_surface = crate::volatility::flat_vol_surface(
            VOL_SURFACE_ID,
            &[0.5, 1.0, 2.0],
            &[80.0, 100.0, 120.0],
            self.vol,
        );

        let mut market = MarketContext::new()
            .insert(disc_curve)
            .insert_surface(vol_surface)
            .insert_price(SPOT_ID, MarketScalar::Unitless(self.spot));

        if let Some(div) = self.div_yield {
            market = market.insert_price(DIV_YIELD_ID, MarketScalar::Unitless(div));
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
