//! Shared forward-price resolution for commodity forwards and options.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::scalar_numeric_value;

/// Inputs for [`resolve_forward_price`].
pub(crate) struct ForwardPriceRequest<'a> {
    /// Market context supplying the price curve, spot scalar and discount curve.
    pub market: &'a MarketContext,
    /// Valuation date.
    pub as_of: Date,
    /// Delivery / expiry date the forward is resolved for.
    pub delivery: Date,
    /// Explicit forward override; wins over every market lookup when set.
    pub quoted: Option<f64>,
    /// `PriceCurve` identifier holding the commodity forward curve.
    pub forward_curve_id: &'a CurveId,
    /// Optional spot scalar identifier for the cost-of-carry fallback.
    pub spot_id: Option<&'a str>,
    /// Discount curve used by the cost-of-carry fallback `F = S / DF(as_of, delivery)`.
    pub discount_curve_id: &'a CurveId,
}

/// Resolve a commodity forward price with the shared waterfall.
///
/// 1. `quoted` override.
/// 2. `PriceCurve` by `forward_curve_id`: the curve spot at or past `delivery`,
///    otherwise `price_on_date(delivery)` (respects the curve's own day count).
/// 3. Cost of carry from `spot_id`: spot at or past `delivery`, otherwise
///    `spot / DF(as_of, delivery)`; missing configured spot or discount data
///    is an error, not permission to substitute spot.
///
/// # Arguments
///
/// * `request` - Market handles, dates and override for the resolution.
///
/// # Errors
///
/// Returns `InputError::NotFound` when neither an override, a price curve nor
/// a spot identifier is available, and propagates curve lookup failures.
pub(crate) fn resolve_forward_price(request: ForwardPriceRequest<'_>) -> Result<f64> {
    let ForwardPriceRequest {
        market,
        as_of,
        delivery,
        quoted,
        forward_curve_id,
        spot_id,
        discount_curve_id,
    } = request;
    if let Some(price) = quoted {
        return Ok(price);
    }
    let delivered = delivery <= as_of;
    if let Ok(price_curve) = market.get_price_curve(forward_curve_id.as_str()) {
        if delivered {
            return Ok(price_curve.spot_price());
        }
        return price_curve.price_on_date(delivery);
    }
    if let Some(spot_id) = spot_id {
        let spot = scalar_numeric_value(market.get_price(spot_id)?);
        if delivered {
            return Ok(spot);
        }
        let disc = market.get_discount(discount_curve_id.as_str())?;
        let df = disc.df_between_dates(as_of, delivery)?;
        return Ok(spot / df);
    }
    Err(finstack_quant_core::Error::Input(
        finstack_quant_core::InputError::NotFound {
            id: format!(
                "PriceCurve '{forward_curve_id}' not found. \
                 Use MarketContext::insert_price_curve() to add a commodity forward price curve."
            ),
        },
    ))
}
