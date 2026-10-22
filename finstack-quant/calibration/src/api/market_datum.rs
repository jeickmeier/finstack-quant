//! Flat-list market-data inputs for `CalibrationEnvelope` canonical.
//!
//! `MarketDatum` is the single id-addressable datum type consumed by the canonical
//! calibration envelope. Each variant wraps an existing market-data primitive
//! (quote, scalar, surface, etc.) and is serialized with a `kind` tag so the
//! envelope can carry a heterogeneous list in JSON/YAML.

use crate::quotes::CdsQuote;
use crate::quotes::CdsTrancheQuote;
use crate::quotes::InflationQuote;
use crate::quotes::MarketQuote;
use crate::quotes::RateQuote;
use crate::quotes::VolQuote;
use crate::quotes::XccyQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::CreditIndexState;
use finstack_quant_core::market_data::dividends::DividendSchedule;
use finstack_quant_core::market_data::scalars::{InflationIndex, MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::surfaces::{FxDeltaVolSurface, VolCube};
use serde::{Deserialize, Serialize};

/// A single id-addressable input to the calibrator.
///
/// Each variant is tagged via serde as `{"kind": "<snake_case_variant>", ...}`
/// so callers can author flat heterogeneous lists in JSON/YAML.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MarketDatum {
    /// Interest-rate quote (deposit, FRA, future, swap, ...).
    RateQuote(RateQuote),
    /// Single-name credit-default-swap quote.
    CdsQuote(CdsQuote),
    /// CDS index tranche quote.
    CdsTrancheQuote(CdsTrancheQuote),
    /// Inflation quote (zero-coupon swap, year-on-year, ...).
    InflationQuote(InflationQuote),
    /// Volatility quote (cap/floor, swaption, ...).
    VolQuote(VolQuote),
    /// Cross-currency basis-swap quote.
    XccyQuote(XccyQuote),
    /// FX spot quote.
    FxSpot(FxSpotDatum),
    /// Spot price for a single asset.
    Price(PriceDatum),
    /// Dividend schedule for an underlier.
    DividendSchedule(DividendSchedule),
    /// Generic scalar time series (CPI, historical fixings, ...).
    FixingSeries(ScalarTimeSeries),
    /// Inflation index fixings.
    InflationFixings(
        //
        InflationIndex,
    ),
    /// Credit-index reference state.
    CreditIndex(CreditIndexState),
    /// FX delta-vol surface.
    FxVolSurface(FxDeltaVolSurface),
    /// Generic vol cube.
    VolCube(VolCube),
}

/// FX-spot quote payload for [`MarketDatum::FxSpot`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FxSpotDatum {
    /// Stable identifier for this datum.
    pub id: String,
    /// Base currency (e.g. `EUR` in `EUR/USD`).
    pub from: Currency,
    /// Quote currency (e.g. `USD` in `EUR/USD`).
    pub to: Currency,
    /// Rate such that `1 from = rate to`.
    pub rate: f64,
}

/// Single-name spot-price payload for [`MarketDatum::Price`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PriceDatum {
    /// Stable identifier (e.g., asset ticker).
    pub id: String,
    /// Scalar value (unitless or monetary).
    pub scalar: MarketScalar,
}

impl MarketDatum {
    /// Stable identifier for this datum, borrowed as a string slice.
    ///
    /// For quote variants this delegates to the inner `QuoteId`. For snapshot
    /// variants it returns the wrapped struct's `id` field.
    pub fn id(&self) -> &str {
        match self {
            MarketDatum::RateQuote(q) => q.id().as_str(),
            MarketDatum::CdsQuote(q) => q.id().as_str(),
            MarketDatum::CdsTrancheQuote(q) => q.id().as_str(),
            MarketDatum::InflationQuote(q) => q.id().as_str(),
            MarketDatum::VolQuote(q) => q.id().as_str(),
            MarketDatum::XccyQuote(q) => q.id().as_str(),
            MarketDatum::FxSpot(d) => &d.id,
            MarketDatum::Price(d) => &d.id,
            MarketDatum::DividendSchedule(d) => d.get_id().as_str(),
            MarketDatum::FixingSeries(s) => s.id().as_str(),
            MarketDatum::InflationFixings(i) => i.id(),
            MarketDatum::CreditIndex(c) => c.id.as_str(),
            MarketDatum::FxVolSurface(s) => s.id().as_str(),
            MarketDatum::VolCube(c) => c.id().as_str(),
        }
    }

    /// Serde discriminator tag for this variant (matches the `kind` field).
    pub fn kind_name(&self) -> &'static str {
        match self {
            MarketDatum::RateQuote(_) => "rate_quote",
            MarketDatum::CdsQuote(_) => "cds_quote",
            MarketDatum::CdsTrancheQuote(_) => "cds_tranche_quote",
            MarketDatum::InflationQuote(_) => "inflation_quote",
            MarketDatum::VolQuote(_) => "vol_quote",
            MarketDatum::XccyQuote(_) => "xccy_quote",
            MarketDatum::FxSpot(_) => "fx_spot",
            MarketDatum::Price(_) => "price",
            MarketDatum::DividendSchedule(_) => "dividend_schedule",
            MarketDatum::FixingSeries(_) => "fixing_series",
            MarketDatum::InflationFixings(_) => "inflation_fixings",
            MarketDatum::CreditIndex(_) => "credit_index",
            MarketDatum::FxVolSurface(_) => "fx_vol_surface",
            MarketDatum::VolCube(_) => "vol_cube",
        }
    }

    /// If this datum is a quote variant, wrap it in a `MarketQuote`.
    /// Returns `None` for snapshot variants (prices, surfaces, etc.).
    pub fn as_quote(&self) -> Option<MarketQuote> {
        match self {
            MarketDatum::RateQuote(q) => Some(MarketQuote::Rates(q.clone())),
            MarketDatum::CdsQuote(q) => Some(MarketQuote::Cds(q.clone())),
            MarketDatum::CdsTrancheQuote(q) => Some(MarketQuote::CdsTranche(q.clone())),
            MarketDatum::InflationQuote(q) => Some(MarketQuote::Inflation(q.clone())),
            MarketDatum::VolQuote(q) => Some(MarketQuote::Vol(q.clone())),
            MarketDatum::XccyQuote(q) => Some(MarketQuote::Xccy(q.clone())),
            _ => None,
        }
    }

    /// Convenience: returns `true` if this datum is a quote variant.
    pub fn is_quote(&self) -> bool {
        matches!(
            self,
            Self::RateQuote(_)
                | Self::CdsQuote(_)
                | Self::CdsTrancheQuote(_)
                | Self::InflationQuote(_)
                | Self::VolQuote(_)
                | Self::XccyQuote(_)
        )
    }
}

/// Convert any [`MarketQuote`] into the matching `MarketDatum::*Quote` variant.
///
/// Inverse of [`MarketDatum::as_quote`] for the six quote-bearing variants.
impl From<MarketQuote> for MarketDatum {
    fn from(q: MarketQuote) -> Self {
        match q {
            MarketQuote::Rates(q) => MarketDatum::RateQuote(q),
            MarketQuote::Cds(q) => MarketDatum::CdsQuote(q),
            MarketQuote::CdsTranche(q) => MarketDatum::CdsTrancheQuote(q),
            MarketQuote::Inflation(q) => MarketDatum::InflationQuote(q),
            MarketQuote::Vol(q) => MarketDatum::VolQuote(q),
            MarketQuote::Xccy(q) => MarketDatum::XccyQuote(q),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_price_datum() {
        let datum = MarketDatum::Price(PriceDatum {
            id: "AAPL".into(),
            scalar: MarketScalar::Unitless(175.42),
        });
        let json = serde_json::to_string(&datum).expect("price datum should serialize");
        assert!(json.contains(r#""kind":"price""#));
        let back: MarketDatum =
            serde_json::from_str(&json).expect("serialized price datum should deserialize");
        let MarketDatum::Price(p) = back else {
            unreachable!("round-tripped price datum should retain the Price variant");
        };
        assert_eq!(p.id, "AAPL");
        assert!(matches!(p.scalar, MarketScalar::Unitless(v) if (v - 175.42).abs() < 1e-12));
    }

    #[test]
    fn rate_quote_id_kind_and_as_quote() {
        use crate::quotes::RateQuote;
        use crate::quotes::{Pillar, QuoteId};
        use finstack_quant_core::dates::{Tenor, TenorUnit};
        use finstack_quant_core::types::IndexId;

        let rq = RateQuote::Deposit {
            id: QuoteId::new("USD-DEP-1M"),
            index: IndexId::new("USD-SOFR"),
            pillar: Pillar::Tenor(Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture")),
            rate: 0.0525,
        };
        let datum = MarketDatum::RateQuote(rq);
        assert_eq!(datum.id(), "USD-DEP-1M");
        assert_eq!(datum.kind_name(), "rate_quote");
        assert!(datum.as_quote().is_some());
        assert!(datum.is_quote());
    }

    #[test]
    fn price_is_not_a_quote() {
        let datum = MarketDatum::Price(PriceDatum {
            id: "AAPL".into(),
            scalar: MarketScalar::Unitless(175.42),
        });
        assert_eq!(datum.id(), "AAPL");
        assert_eq!(datum.kind_name(), "price");
        assert!(datum.as_quote().is_none());
        assert!(!datum.is_quote());
    }

    #[test]
    fn fx_spot_id_and_kind() {
        let datum = MarketDatum::FxSpot(FxSpotDatum {
            id: "EUR/USD".into(),
            from: Currency::EUR,
            to: Currency::USD,
            rate: 1.085,
        });
        assert_eq!(datum.id(), "EUR/USD");
        assert_eq!(datum.kind_name(), "fx_spot");
        assert!(datum.as_quote().is_none());
    }
    #[test]
    fn dividend_schedule_uses_direct_fields_and_materializes_unchanged() {
        let wire = serde_json::json!({
            "kind": "dividend_schedule",
            "id": "AAPL-DIVS",
            "underlying": "AAPL",
            "currency": "USD",
            "events": [{"date": "2026-08-15", "kind": {"cash": {"amount": "0.25", "currency": "USD"}}}]
        });
        let datum: MarketDatum =
            serde_json::from_value(wire.clone()).expect("valid direct schedule");
        assert_eq!(datum.id(), "AAPL-DIVS");
        assert_eq!(
            serde_json::to_value(&datum).expect("serialize schedule"),
            wire
        );
        let context = crate::api::context_builder::build_initial_context(
            &[],
            std::slice::from_ref(&datum),
            &crate::CalibrationConfig::default(),
        )
        .expect("materialize schedule");
        let (stored_id, stored) = context.dividends_iter().next().expect("stored schedule");
        assert_eq!(stored_id.as_str(), "AAPL-DIVS");
        let mut old_payload = wire.as_object().expect("object").clone();
        old_payload.remove("kind");
        assert_eq!(
            serde_json::to_value(stored).expect("stored wire"),
            serde_json::Value::Object(old_payload.clone())
        );
        let error = serde_json::from_value::<MarketDatum>(serde_json::json!({
            "kind": "dividend_schedule", "schedule": old_payload
        }))
        .expect_err("removed nested shape");
        assert!(error.to_string().contains("schedule"));
    }
}
