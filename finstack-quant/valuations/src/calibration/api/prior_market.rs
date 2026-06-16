//! Pre-built calibrated objects layered into the initial context before steps run.
//!
//! `PriorMarketObject` is a tagged sum type listing pre-built curves and surfaces
//! that can be loaded into the initial `MarketContext` before any calibration
//! step executes. Each variant wraps an existing curve / surface primitive and
//! serializes with a `kind` tag so the envelope can carry a heterogeneous list
//! in JSON/YAML.

use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, BasisSpreadCurve, DiscountCurve, ForwardCurve, HazardCurve,
    InflationCurve, ParametricCurve, PriceCurve, VolatilityIndexCurve,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// A pre-built calibrated market object.
///
/// Variants are tagged via serde as `{"kind": "<snake_case_variant>", ...}` so
/// callers can author flat heterogeneous lists in JSON/YAML. None of the
/// underlying curve / surface types currently derive [`JsonSchema`] directly,
/// so we apply the established `#[schemars(with = "serde_json::Value")]`
/// workaround (matching the pattern in `market_datum.rs`). The same applies
/// on the TypeScript side via `#[ts(type = "unknown")]`.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PriorMarketObject {
    /// Pre-built discount-factor curve.
    DiscountCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        DiscountCurve,
    ),
    /// Pre-built forward-rate curve.
    ForwardCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        ForwardCurve,
    ),
    /// Pre-built default hazard-rate curve.
    HazardCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        HazardCurve,
    ),
    /// Pre-built inflation (breakeven / index) curve.
    InflationCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        InflationCurve,
    ),
    /// Pre-built CDS-index base-correlation curve.
    BaseCorrelationCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        BaseCorrelationCurve,
    ),
    /// Pre-built tenor-basis spread curve.
    BasisSpreadCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        BasisSpreadCurve,
    ),
    /// Pre-built parametric (e.g. Nelson-Siegel) curve.
    ParametricCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        ParametricCurve,
    ),
    /// Pre-built spot / forward price curve.
    PriceCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        PriceCurve,
    ),
    /// Pre-built volatility-index forward curve.
    VolatilityIndexCurve(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        VolatilityIndexCurve,
    ),
    /// Pre-built volatility surface (expiry x strike).
    VolSurface(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        VolSurface,
    ),
}

impl PriorMarketObject {
    /// Stable identifier for this object, borrowed as a string slice.
    pub fn id(&self) -> &str {
        match self {
            PriorMarketObject::DiscountCurve(c) => c.id().as_str(),
            PriorMarketObject::ForwardCurve(c) => c.id().as_str(),
            PriorMarketObject::HazardCurve(c) => c.id().as_str(),
            PriorMarketObject::InflationCurve(c) => c.id().as_str(),
            PriorMarketObject::BaseCorrelationCurve(c) => c.id().as_str(),
            PriorMarketObject::BasisSpreadCurve(c) => c.id().as_str(),
            PriorMarketObject::ParametricCurve(c) => c.id().as_str(),
            PriorMarketObject::PriceCurve(c) => c.id().as_str(),
            PriorMarketObject::VolatilityIndexCurve(c) => c.id().as_str(),
            PriorMarketObject::VolSurface(s) => s.id().as_str(),
        }
    }

    /// Serde discriminator tag for this variant (matches the `kind` field).
    pub fn kind_name(&self) -> &'static str {
        match self {
            PriorMarketObject::DiscountCurve(_) => "discount_curve",
            PriorMarketObject::ForwardCurve(_) => "forward_curve",
            PriorMarketObject::HazardCurve(_) => "hazard_curve",
            PriorMarketObject::InflationCurve(_) => "inflation_curve",
            PriorMarketObject::BaseCorrelationCurve(_) => "base_correlation_curve",
            PriorMarketObject::BasisSpreadCurve(_) => "basis_spread_curve",
            PriorMarketObject::ParametricCurve(_) => "parametric_curve",
            PriorMarketObject::PriceCurve(_) => "price_curve",
            PriorMarketObject::VolatilityIndexCurve(_) => "volatility_index_curve",
            PriorMarketObject::VolSurface(_) => "vol_surface",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::math::interp::InterpStyle;
    use time::Month;

    fn make_simple_discount_curve() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("valid date"))
            .knots([(0.0, 1.0), (5.0, 0.9)])
            .interp(InterpStyle::MonotoneConvex)
            .build()
            .expect("DiscountCurve builder should succeed")
    }

    #[test]
    fn discount_curve_round_trips_with_kind_tag() {
        let curve = make_simple_discount_curve();
        let obj = PriorMarketObject::DiscountCurve(curve);
        let json = serde_json::to_string(&obj).expect("serialize");
        assert!(
            json.contains(r#""kind":"discount_curve""#),
            "missing kind tag: {json}"
        );
        let back: PriorMarketObject = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.kind_name(), "discount_curve");
        assert_eq!(back.id(), "USD-OIS");
        assert!(matches!(back, PriorMarketObject::DiscountCurve(_)));
    }
}
