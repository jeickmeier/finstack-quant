//! Flat-list market-data inputs for `CalibrationEnvelope` v3.
//!
//! `MarketDatum` is the single id-addressable datum type consumed by the v3
//! calibration envelope. Each variant wraps an existing market-data primitive
//! (quote, scalar, surface, etc.) and is serialized with a `kind` tag so the
//! envelope can carry a heterogeneous list in JSON/YAML.

use crate::market::quotes::bond::BondQuote;
use crate::market::quotes::cds::CdsQuote;
use crate::market::quotes::cds_tranche::CDSTrancheQuote;
use crate::market::quotes::fx::FxQuote;
use crate::market::quotes::inflation::InflationQuote;
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::rates::RateQuote;
use crate::market::quotes::vol::VolQuote;
use crate::market::quotes::xccy::XccyQuote;
use f64;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::CreditIndexState;
use finstack_quant_core::market_data::dividends::DividendSchedule;
use finstack_quant_core::market_data::scalars::{InflationIndex, MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::surfaces::{FxDeltaVolSurface, VolCube};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// A single id-addressable input to the calibrator.
///
/// Each variant is tagged via serde as `{"kind": "<snake_case_variant>", ...}`
/// so callers can author flat heterogeneous lists in JSON/YAML.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MarketDatum {
    /// Interest-rate quote (deposit, FRA, future, swap, ...).
    RateQuote(RateQuote),
    /// Single-name credit-default-swap quote.
    CdsQuote(CdsQuote),
    /// CDS index tranche quote.
    CdsTrancheQuote(CDSTrancheQuote),
    /// FX quote (forward, swap, ...).
    FxQuote(FxQuote),
    /// Inflation quote (zero-coupon swap, year-on-year, ...).
    InflationQuote(InflationQuote),
    /// Volatility quote (cap/floor, swaption, ...).
    VolQuote(VolQuote),
    /// Cross-currency basis-swap quote.
    XccyQuote(XccyQuote),
    /// Bond quote (price or yield).
    BondQuote(BondQuote),
    /// FX spot quote.
    FxSpot(FxSpotDatum),
    /// Spot price for a single asset.
    Price(PriceDatum),
    /// Dividend schedule for an underlier.
    DividendSchedule(DividendScheduleDatum),
    /// Generic scalar time series (CPI, historical fixings, ...).
    FixingSeries(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        ScalarTimeSeries,
    ),
    /// Inflation index fixings.
    InflationFixings(
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))] //
        InflationIndex,
    ),
    /// Credit-index reference state.
    CreditIndex(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        CreditIndexState,
    ),
    /// FX delta-vol surface.
    FxVolSurface(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        FxDeltaVolSurface,
    ),
    /// Generic vol cube.
    VolCube(
        #[schemars(with = "serde_json::Value")] //
        #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
        VolCube,
    ),
    /// Collateral / CSA mapping entry.
    Collateral(CollateralEntry),
}

/// FX-spot quote payload for [`MarketDatum::FxSpot`].
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FxSpotDatum {
    /// Stable identifier for this datum.
    pub id: String,
    /// Base currency (e.g. `EUR` in `EUR/USD`).
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub from: Currency,
    /// Quote currency (e.g. `USD` in `EUR/USD`).
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub to: Currency,
    /// Rate such that `1 from = rate to`.
    pub rate: f64,
}

/// Single-name spot-price payload for [`MarketDatum::Price`].
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PriceDatum {
    /// Stable identifier (e.g., asset ticker).
    pub id: String,
    /// Scalar value (unitless or monetary).
    #[schemars(with = "serde_json::Value")]
    #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
    pub scalar: MarketScalar,
}

/// Dividend-schedule payload for [`MarketDatum::DividendSchedule`].
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DividendScheduleDatum {
    /// The dividend schedule itself.
    #[schemars(with = "serde_json::Value")]
    #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
    pub schedule: DividendSchedule,
}

/// Collateral / CSA mapping payload for [`MarketDatum::Collateral`].
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollateralEntry {
    /// Trade-leg currency this CSA mapping applies to.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub id: Currency,
    /// Collateral / CSA currency.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub csa_currency: Currency,
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
            MarketDatum::FxQuote(q) => q.id().as_str(),
            MarketDatum::InflationQuote(q) => q.id().as_str(),
            MarketDatum::VolQuote(q) => q.id().as_str(),
            MarketDatum::XccyQuote(q) => q.id().as_str(),
            MarketDatum::BondQuote(q) => q.id().as_str(),
            MarketDatum::FxSpot(d) => &d.id,
            MarketDatum::Price(d) => &d.id,
            MarketDatum::DividendSchedule(d) => d.schedule.id.as_str(),
            MarketDatum::FixingSeries(s) => s.id().as_str(),
            MarketDatum::InflationFixings(i) => i.id.as_str(),
            MarketDatum::CreditIndex(c) => c.id.as_str(),
            MarketDatum::FxVolSurface(s) => s.id().as_str(),
            MarketDatum::VolCube(c) => c.id().as_str(),
            MarketDatum::Collateral(c) => c.id.as_ref(),
        }
    }

    /// Serde discriminator tag for this variant (matches the `kind` field).
    pub fn kind_name(&self) -> &'static str {
        match self {
            MarketDatum::RateQuote(_) => "rate_quote",
            MarketDatum::CdsQuote(_) => "cds_quote",
            MarketDatum::CdsTrancheQuote(_) => "cds_tranche_quote",
            MarketDatum::FxQuote(_) => "fx_quote",
            MarketDatum::InflationQuote(_) => "inflation_quote",
            MarketDatum::VolQuote(_) => "vol_quote",
            MarketDatum::XccyQuote(_) => "xccy_quote",
            MarketDatum::BondQuote(_) => "bond_quote",
            MarketDatum::FxSpot(_) => "fx_spot",
            MarketDatum::Price(_) => "price",
            MarketDatum::DividendSchedule(_) => "dividend_schedule",
            MarketDatum::FixingSeries(_) => "fixing_series",
            MarketDatum::InflationFixings(_) => "inflation_fixings",
            MarketDatum::CreditIndex(_) => "credit_index",
            MarketDatum::FxVolSurface(_) => "fx_vol_surface",
            MarketDatum::VolCube(_) => "vol_cube",
            MarketDatum::Collateral(_) => "collateral",
        }
    }

    /// If this datum is a quote variant, wrap it in a `MarketQuote`.
    /// Returns `None` for snapshot variants (prices, surfaces, etc.).
    pub fn as_quote(&self) -> Option<MarketQuote> {
        match self {
            MarketDatum::RateQuote(q) => Some(MarketQuote::Rates(q.clone())),
            MarketDatum::CdsQuote(q) => Some(MarketQuote::Cds(q.clone())),
            MarketDatum::CdsTrancheQuote(q) => Some(MarketQuote::CDSTranche(q.clone())),
            MarketDatum::FxQuote(q) => Some(MarketQuote::Fx(q.clone())),
            MarketDatum::InflationQuote(q) => Some(MarketQuote::Inflation(q.clone())),
            MarketDatum::VolQuote(q) => Some(MarketQuote::Vol(q.clone())),
            MarketDatum::XccyQuote(q) => Some(MarketQuote::Xccy(q.clone())),
            MarketDatum::BondQuote(q) => Some(MarketQuote::Bond(q.clone())),
            _ => None,
        }
    }

    /// Convenience: returns `true` if this datum is a quote variant.
    pub fn is_quote(&self) -> bool {
        self.as_quote().is_some()
    }
}

/// Convert any [`MarketQuote`] into the matching `MarketDatum::*Quote` variant.
///
/// Inverse of [`MarketDatum::as_quote`] for the eight quote-bearing variants.
impl From<MarketQuote> for MarketDatum {
    fn from(q: MarketQuote) -> Self {
        match q {
            MarketQuote::Rates(q) => MarketDatum::RateQuote(q),
            MarketQuote::Cds(q) => MarketDatum::CdsQuote(q),
            MarketQuote::CDSTranche(q) => MarketDatum::CdsTrancheQuote(q),
            MarketQuote::Fx(q) => MarketDatum::FxQuote(q),
            MarketQuote::Inflation(q) => MarketDatum::InflationQuote(q),
            MarketQuote::Vol(q) => MarketDatum::VolQuote(q),
            MarketQuote::Xccy(q) => MarketDatum::XccyQuote(q),
            MarketQuote::Bond(q) => MarketDatum::BondQuote(q),
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
        let json = serde_json::to_string(&datum).unwrap();
        assert!(json.contains(r#""kind":"price""#));
        let back: MarketDatum = serde_json::from_str(&json).unwrap();
        let MarketDatum::Price(p) = back else {
            panic!("expected Price variant");
        };
        assert_eq!(p.id, "AAPL");
        assert!(matches!(p.scalar, MarketScalar::Unitless(v) if (v - 175.42).abs() < 1e-12));
    }

    #[test]
    fn rate_quote_id_kind_and_as_quote() {
        use crate::market::quotes::ids::{Pillar, QuoteId};
        use crate::market::quotes::rates::RateQuote;
        use finstack_quant_core::dates::{Tenor, TenorUnit};
        use finstack_quant_core::types::IndexId;

        let rq = RateQuote::Deposit {
            id: QuoteId::new("USD-DEP-1M"),
            index: IndexId::new("USD-SOFR"),
            pillar: Pillar::Tenor(Tenor::new(1, TenorUnit::Months)),
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
    fn collateral_id_returns_currency_string() {
        let datum = MarketDatum::Collateral(CollateralEntry {
            id: Currency::USD,
            csa_currency: Currency::USD,
        });
        assert_eq!(datum.id(), "USD");
        assert_eq!(datum.kind_name(), "collateral");
    }
}
