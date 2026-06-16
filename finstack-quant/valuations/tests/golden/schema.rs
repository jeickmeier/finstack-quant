//! Serde structs for the `finstack_quant.golden/2` fixture schema.
//!
//! A fixture is a strict envelope with five sections, in order:
//! `metadata` (identity, provenance, valuation date), a `kind`-tagged body
//! (`pricing` or `sabr_smile`), `expected` (raw source values), and
//! `tolerances` (one entry per expected metric).
//!
//! Unknown-field rejection at the top level and inside each body is enforced
//! by [`crate::golden::walk`] (serde cannot `deny_unknown_fields` together with
//! `#[serde(flatten)]` or internally tagged enums).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Current golden fixture schema version.
pub const SCHEMA_VERSION: &str = "finstack_quant.golden/2";

/// Top-level fixture envelope. One per JSON file under `tests/golden/data/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenFixture {
    /// Schema version. Must equal [`SCHEMA_VERSION`].
    pub schema_version: String,
    /// Identity, provenance, and review metadata.
    pub metadata: Metadata,
    /// Domain-specific body, discriminated by the `kind` field.
    #[serde(flatten)]
    pub body: Body,
    /// Map of metric name to reference value, taken directly from the source.
    pub expected: BTreeMap<String, f64>,
    /// Map of metric name to tolerance entry. Must cover every expected metric.
    pub tolerances: BTreeMap<String, ToleranceEntry>,
}

impl GoldenFixture {
    /// Pricing body, when this fixture has `kind = "pricing"`.
    pub fn pricing(&self) -> Option<&PricingBody> {
        match &self.body {
            Body::Pricing(body) => Some(body),
            Body::SabrSmile(_) => None,
        }
    }

    /// SABR-smile body, when this fixture has `kind = "sabr_smile"`.
    pub fn sabr(&self) -> Option<&SabrBody> {
        match &self.body {
            Body::SabrSmile(body) => Some(body),
            Body::Pricing(_) => None,
        }
    }
}

/// Fixture identity, provenance, and review metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    /// Snake-case unique name within the domain.
    pub name: String,
    /// Dotted domain path: `rates.irs`, `fixed_income.bond`, `volatility.sabr`.
    pub domain: String,
    /// One-sentence description.
    pub description: String,
    /// YYYY-MM-DD valuation date the reference values are priced as of.
    pub valuation_date: String,
    /// Source mode: quantlib | bloomberg-api | bloomberg-screen | intex | formula | textbook.
    pub source: String,
    /// Free-form source details such as QuantLib version, Bloomberg screen, or textbook page.
    pub source_detail: String,
    /// Username at capture time.
    pub captured_by: String,
    /// YYYY-MM-DD when fixture was first written.
    pub captured_on: String,
    /// Username at last review.
    pub last_reviewed_by: String,
    /// YYYY-MM-DD when fixture was last reviewed.
    pub last_reviewed_on: String,
    /// Review interval in months. Defaults to 6 by convention.
    pub review_interval_months: u32,
    /// Exact command to regenerate. Empty allowed for formula or textbook sources.
    pub regen_command: String,
    /// Image evidence for manual sources. Required for bloomberg-screen and intex fixtures.
    #[serde(default)]
    pub screenshots: Vec<Screenshot>,
}

/// Screenshot evidence for manually captured external references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screenshot {
    /// Path relative to the fixture JSON.
    pub path: String,
    /// Bloomberg or Intex screen name.
    pub screen: String,
    /// YYYY-MM-DD capture date.
    pub captured_on: String,
    /// Free-form description.
    pub description: String,
}

/// Domain-specific fixture body, discriminated by the `kind` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    /// Instrument pricing fixture: model, market context, and instrument.
    Pricing(PricingBody),
    /// SABR smile fixture: parameters, forward, expiry, and strikes.
    SabrSmile(SabrBody),
}

/// Body of a `kind = "pricing"` fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingBody {
    /// Pricing model selector (e.g. `discounting`, `tree`, `hull_white_1f`).
    pub model: String,
    /// Market context: a materialized snapshot or a calibration envelope.
    pub market: Market,
    /// Instrument definition JSON (`finstack_quant.instrument/1` envelope).
    pub instrument: serde_json::Value,
}

/// Market context for a pricing fixture. Exactly one variant is present.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Market {
    /// Materialized `MarketContext` snapshot.
    Snapshot {
        /// `MarketContext` JSON.
        data: serde_json::Value,
    },
    /// Quote-driven `CalibrationEnvelope`.
    Envelope {
        /// `CalibrationEnvelope` JSON.
        envelope: serde_json::Value,
    },
}

/// Body of a `kind = "sabr_smile"` fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabrBody {
    /// SABR alpha (initial volatility).
    pub alpha: f64,
    /// SABR beta (CEV exponent).
    pub beta: f64,
    /// SABR nu (vol-of-vol).
    pub nu: f64,
    /// SABR rho (forward/vol correlation).
    pub rho: f64,
    /// Optional shift for the shifted-SABR (negative-rates) branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift: Option<f64>,
    /// Forward rate.
    pub forward: f64,
    /// Time to expiry in years.
    pub time_to_expiry: f64,
    /// Strikes with per-strike expected-output keys.
    pub strikes: Vec<StrikeEntry>,
}

/// A single strike entry in a SABR-smile fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrikeEntry {
    /// Expected-output key this strike's implied vol is recorded under.
    pub key: String,
    /// Strike value.
    pub strike: f64,
}

/// Per-metric tolerance. A comparison passes if either `abs` or `rel` is satisfied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToleranceEntry {
    /// Absolute tolerance: `|actual - expected| <= abs`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abs: Option<f64>,
    /// Relative tolerance: `|actual - expected| / max(|expected|, 1e-12) <= rel`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel: Option<f64>,
    /// Explanation for any fixture-specific tolerance override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRICING_JSON: &str = r#"{
      "schema_version": "finstack_quant.golden/2",
      "metadata": {
        "name": "test_fixture",
        "domain": "rates.irs",
        "description": "Minimal smoke fixture",
        "valuation_date": "2026-04-30",
        "source": "quantlib",
        "source_detail": "QL 1.34",
        "captured_by": "test",
        "captured_on": "2026-04-30",
        "last_reviewed_by": "test",
        "last_reviewed_on": "2026-04-30",
        "review_interval_months": 6,
        "regen_command": "uv run scripts/goldens/regen.py --kind irs-par"
      },
      "kind": "pricing",
      "model": "discounting",
      "market": {"kind": "envelope", "envelope": {"schema": "finstack_quant.calibration"}},
      "instrument": {"foo": 1},
      "expected": {"npv": 100.0},
      "tolerances": {"npv": {"abs": 0.01}}
    }"#;

    const SABR_JSON: &str = r#"{
      "schema_version": "finstack_quant.golden/2",
      "metadata": {
        "name": "smile",
        "domain": "volatility.sabr",
        "description": "Minimal SABR smile",
        "valuation_date": "2026-04-30",
        "source": "formula",
        "source_detail": "Hagan 2002",
        "captured_by": "test",
        "captured_on": "2026-04-30",
        "last_reviewed_by": "test",
        "last_reviewed_on": "2026-04-30",
        "review_interval_months": 6,
        "regen_command": ""
      },
      "kind": "sabr_smile",
      "alpha": 0.05,
      "beta": 0.5,
      "nu": 0.4,
      "rho": -0.1,
      "forward": 0.05,
      "time_to_expiry": 2.0,
      "strikes": [{"key": "vol_k0050", "strike": 0.05}],
      "expected": {"vol_k0050": 0.2292},
      "tolerances": {"vol_k0050": {"abs": 1e-9}}
    }"#;

    #[test]
    fn deserialize_pricing_fixture() {
        let fixture: GoldenFixture = serde_json::from_str(PRICING_JSON).expect("fixture parses");

        assert_eq!(fixture.schema_version, SCHEMA_VERSION);
        assert_eq!(fixture.metadata.name, "test_fixture");
        assert_eq!(fixture.metadata.valuation_date, "2026-04-30");
        assert_eq!(fixture.expected.get("npv"), Some(&100.0));
        assert!(fixture.metadata.screenshots.is_empty());

        let pricing = fixture.pricing().expect("pricing body");
        assert_eq!(pricing.model, "discounting");
        assert!(matches!(pricing.market, Market::Envelope { .. }));
        assert!(fixture.sabr().is_none());
    }

    #[test]
    fn deserialize_sabr_fixture() {
        let fixture: GoldenFixture = serde_json::from_str(SABR_JSON).expect("fixture parses");

        let sabr = fixture.sabr().expect("sabr body");
        assert_eq!(sabr.strikes.len(), 1);
        assert_eq!(sabr.strikes[0].key, "vol_k0050");
        assert!(sabr.shift.is_none());
        assert!(fixture.pricing().is_none());
    }

    #[test]
    fn round_trips_pricing_fixture() {
        let fixture: GoldenFixture = serde_json::from_str(PRICING_JSON).expect("parse");
        let serialized = serde_json::to_string(&fixture).expect("serialize");
        let reparsed: GoldenFixture = serde_json::from_str(&serialized).expect("reparse");
        assert_eq!(reparsed.metadata.name, "test_fixture");
        assert!(reparsed.pricing().is_some());
    }
}
