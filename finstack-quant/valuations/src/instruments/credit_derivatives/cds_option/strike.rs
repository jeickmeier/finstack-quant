//! Typed strike representation for [`CdsOption`](super::CdsOption).
//!
//! CDS options are struck either on a forward par spread (single-name,
//! CDX IG, iTraxx) or on a clean index price (CDX HY market convention).
//! The two conventions carry different units and different payoff algebra,
//! so the strike is a typed enum rather than a bare decimal:
//!
//! - [`CdsOptionStrike::Spread`] is a decimal annual rate: `0.0325` means
//!   325 bp.
//! - [`CdsOptionStrike::CleanPricePct`] is quoted in percentage-price
//!   points: `107.0` means a clean-price fraction `K = 1.07`.
//!
//! # Wire format
//!
//! Canonical JSON uses the externally tagged form:
//!
//! ```json
//! { "spread": "0.0325" }
//! ```
//!
//! ```json
//! { "clean_price_pct": "107.0" }
//! ```
//!
//! The pre-enum bare decimal strike (`"strike": "0.0325"`) is rejected;
//! there is no compatibility fallback.
//!
//! # Volatility-surface coordinate
//!
//! Surface lookup uses the native displayed coordinate returned by
//! [`CdsOptionStrike::native_surface_coordinate`]: the decimal spread
//! (`0.0325`) for a spread strike and the percentage price (`107.0`) for a
//! clean-price strike. Stored surface values remain lognormal
//! forward-spread model volatilities in both cases — a clean-price strike
//! axis does not change the model's state variable.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Typed CDS-option strike: forward-spread or clean-price convention.
///
/// Wire format: externally tagged JSON — `{"spread": "0.0325"}` or
/// `{"clean_price_pct": "107.0"}` — with no bare-decimal fallback.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOptionStrike;
/// use rust_decimal::Decimal;
///
/// let spread = CdsOptionStrike::Spread(Decimal::new(325, 4)); // 0.0325 = 325 bp
/// assert!(spread.spread_decimal().is_some());
/// assert_eq!(spread.native_surface_coordinate().unwrap(), 0.0325);
///
/// let price = CdsOptionStrike::CleanPricePct(Decimal::new(1070, 1)); // 107.0
/// assert_eq!(price.native_surface_coordinate().unwrap(), 107.0);
/// assert!((price.clean_price_fraction().unwrap() - 1.07).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CdsOptionStrike {
    /// Strike forward spread as a decimal annual rate (`0.0325` = 325 bp).
    Spread(
        #[serde(with = "finstack_quant_core::wire::decimal")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DecimalWire")
        )]
        Decimal,
    ),
    /// Strike clean price in percentage-price points (`107.0` = fraction
    /// `1.07`). CDX HY index options are quoted this way.
    CleanPricePct(
        #[serde(with = "finstack_quant_core::wire::decimal")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DecimalWire")
        )]
        Decimal,
    ),
}

/// Discriminant of a [`CdsOptionStrike`], for branching metric and pricing
/// paths without repeated pattern matches on the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdsOptionStrikeKind {
    /// Forward-spread strike.
    Spread,
    /// Clean-price strike.
    CleanPricePct,
}

impl std::fmt::Display for CdsOptionStrikeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spread => write!(f, "spread"),
            Self::CleanPricePct => write!(f, "clean_price_pct"),
        }
    }
}

impl CdsOptionStrike {
    /// Strike kind discriminant.
    pub fn kind(&self) -> CdsOptionStrikeKind {
        match self {
            Self::Spread(_) => CdsOptionStrikeKind::Spread,
            Self::CleanPricePct(_) => CdsOptionStrikeKind::CleanPricePct,
        }
    }

    /// The strike spread as a decimal rate, when this is a spread strike.
    pub fn spread_decimal(&self) -> Option<Decimal> {
        match self {
            Self::Spread(s) => Some(*s),
            Self::CleanPricePct(_) => None,
        }
    }

    /// The strike clean price in percentage points, when this is a
    /// clean-price strike.
    pub fn clean_price_pct_decimal(&self) -> Option<Decimal> {
        match self {
            Self::Spread(_) => None,
            Self::CleanPricePct(p) => Some(*p),
        }
    }

    /// The strike spread as `f64`, failing for clean-price strikes.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the strike is a clean-price strike
    /// or the decimal cannot be represented as `f64`.
    pub fn spread_f64(&self) -> finstack_quant_core::Result<f64> {
        match self {
            Self::Spread(s) => decimal_to_f64(*s, "strike spread"),
            Self::CleanPricePct(p) => Err(finstack_quant_core::Error::Validation(format!(
                "expected a spread strike but got clean-price strike {p}; \
                 clean-price strikes have no spread representation"
            ))),
        }
    }

    /// The strike clean price converted to a fraction (`107.0` → `1.07`),
    /// failing for spread strikes.
    ///
    /// This is the single place the percentage-points-to-fraction
    /// conversion happens; pricing must consume this accessor rather than
    /// re-dividing by 100.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the strike is a spread strike or
    /// the decimal cannot be represented as `f64`.
    pub fn clean_price_fraction(&self) -> finstack_quant_core::Result<f64> {
        match self {
            Self::Spread(s) => Err(finstack_quant_core::Error::Validation(format!(
                "expected a clean-price strike but got spread strike {s}; \
                 spread strikes have no clean-price representation"
            ))),
            Self::CleanPricePct(p) => Ok(decimal_to_f64(*p, "strike clean price")? / 100.0),
        }
    }

    /// Native volatility-surface strike coordinate: the decimal spread for
    /// spread strikes (`0.0325`) and the percentage clean price for
    /// clean-price strikes (`107.0`).
    ///
    /// # Errors
    ///
    /// Returns a validation error when the decimal cannot be represented
    /// as `f64`.
    pub fn native_surface_coordinate(&self) -> finstack_quant_core::Result<f64> {
        match self {
            Self::Spread(s) => decimal_to_f64(*s, "strike spread"),
            Self::CleanPricePct(p) => decimal_to_f64(*p, "strike clean price"),
        }
    }
}

fn decimal_to_f64(value: Decimal, label: &str) -> finstack_quant_core::Result<f64> {
    value.to_f64().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!("cannot represent {label} {value} as f64"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spread_variant_round_trips_canonical_json() {
        let strike = CdsOptionStrike::Spread(Decimal::new(325, 4));
        let json = serde_json::to_value(strike).expect("serialize");
        assert_eq!(json, serde_json::json!({ "spread": "0.0325" }));
        let back: CdsOptionStrike = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, strike);
    }

    #[test]
    fn clean_price_variant_round_trips_canonical_json() {
        let strike = CdsOptionStrike::CleanPricePct(Decimal::new(1070, 1));
        let json = serde_json::to_value(strike).expect("serialize");
        assert_eq!(json, serde_json::json!({ "clean_price_pct": "107.0" }));
        let back: CdsOptionStrike = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, strike);
    }

    #[test]
    fn old_bare_decimal_strike_is_rejected() {
        assert!(serde_json::from_value::<CdsOptionStrike>(serde_json::json!("0.0325")).is_err());
        assert!(serde_json::from_value::<CdsOptionStrike>(serde_json::json!(0.0325)).is_err());
    }

    #[test]
    fn unknown_variant_is_rejected() {
        let err = serde_json::from_value::<CdsOptionStrike>(serde_json::json!({
            "price": "107.0"
        }))
        .expect_err("unknown variant must fail");
        assert!(err.to_string().contains("price"));
    }

    #[test]
    fn native_surface_coordinates_use_displayed_units() {
        let spread = CdsOptionStrike::Spread(Decimal::new(325, 4));
        assert!((spread.native_surface_coordinate().unwrap() - 0.0325).abs() < 1e-15);

        let price = CdsOptionStrike::CleanPricePct(Decimal::new(1070, 1));
        assert!((price.native_surface_coordinate().unwrap() - 107.0).abs() < 1e-12);
    }

    #[test]
    fn clean_price_fraction_converts_percent_points_once() {
        let price = CdsOptionStrike::CleanPricePct(Decimal::new(1070, 1));
        assert!((price.clean_price_fraction().unwrap() - 1.07).abs() < 1e-15);

        let spread = CdsOptionStrike::Spread(Decimal::new(325, 4));
        assert!(spread.clean_price_fraction().is_err());
        assert!(price.spread_f64().is_err());
    }

    #[test]
    fn kind_and_accessors_are_consistent() {
        let spread = CdsOptionStrike::Spread(Decimal::new(1, 2));
        assert_eq!(spread.kind(), CdsOptionStrikeKind::Spread);
        assert_eq!(spread.spread_decimal(), Some(Decimal::new(1, 2)));
        assert_eq!(spread.clean_price_pct_decimal(), None);

        let price = CdsOptionStrike::CleanPricePct(Decimal::new(1070, 1));
        assert_eq!(price.kind(), CdsOptionStrikeKind::CleanPricePct);
        assert_eq!(price.spread_decimal(), None);
        assert_eq!(price.clean_price_pct_decimal(), Some(Decimal::new(1070, 1)));
    }
}
