//! Checked-in JSON schemas owned by the calibration crate.

use finstack_quant_core::schema::{example, example_from_json, SchemaArtifact, SchemaKind};
use serde_json::Value;
use std::sync::OnceLock;

fn calibration_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example(&calibration_envelope_example())
}

fn market_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example(&crate::quotes::market_quote::MarketQuote::Rates(
        rate_quote_example()?,
    ))
}

fn calibration_envelope_example() -> crate::api::schema::CalibrationEnvelope {
    let plan = crate::api::schema::CalibrationPlan {
        id: "usd_curves".to_string(),
        description: Some("Bootstrap the USD OIS discount curve.".to_string()),
        quote_sets: Default::default(),
        steps: Vec::new(),
        settings: Default::default(),
    };
    crate::api::schema::CalibrationEnvelope::new(plan, Vec::new(), Vec::new())
}

/// The result of executing [`calibration_envelope_example`].
///
/// Execution stamps a wall-clock timestamp, which would make the checked-in
/// artifact differ on every regeneration; the example carries none.
fn calibration_result_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let mut envelope = crate::api::engine::calibrate(&calibration_envelope_example())?;
    envelope.result.results_meta.timestamp = None;
    envelope.result.report.results_meta.timestamp = None;
    example(&envelope)
}

/// The dry-run report of [`calibration_envelope_example`]: no findings.
fn calibration_validation_report_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example(&crate::api::validate::validate(
        &calibration_envelope_example(),
    ))
}

/// A USD OIS discount-curve step with the default method and interpolation.
fn step_params_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example_from_json::<crate::api::schema::StepParams>(serde_json::json!({
        "kind": "discount",
        "curve_id": "USD-OIS",
        "currency": "USD",
        "base_date": "2025-01-15"
    }))
}

/// A 5Y-into-5Y ATM swaption at 88 bp normal volatility.
fn hull_white_swaption_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example(&crate::hull_white::SwaptionQuote::try_new(
        5.0, 5.0, 0.0088, true,
    )?)
}

/// A 5Y cap struck at 3% and quoted at 88 bp normal volatility.
fn hull_white_cap_floor_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example(&crate::hull_white::CapFloorQuote::try_new(
        5.0, 0.03, 0.0088, true, true,
    )?)
}

fn hull_white_cap_floor_config_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example_from_json::<crate::hull_white::CapFloorCalibrationConfig>(serde_json::json!({
        "fit_tolerance": 1e-6,
        "frequency": "quarterly"
    }))
}

fn hull_white_piecewise_sigma_config_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example_from_json::<crate::hull_white::PiecewiseSigmaCalibrationConfig>(serde_json::json!({
        "fit_tolerance": 1e-6,
        "fixed_kappa": 0.03,
        "sigma_min": 1e-4,
        "sigma_max": 0.05,
        "frequency": "quarterly"
    }))
}

/// A 5Y North American single-name par spread.
fn cds_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example_from_json::<crate::quotes::cds::CdsQuote>(serde_json::json!({
        "type": "cds_par_spread",
        "id": "ACME-CDS-5Y",
        "entity": "ACME",
        "convention": {"currency": "USD", "doc_clause": "isda_na"},
        "pillar": {"tenor": {"count": 5, "unit": "years"}},
        "spread_bp": 120.0,
        "recovery_rate": 0.4
    }))
}

/// The 3-7% mezzanine tranche of a CDX IG series.
fn cds_tranche_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example_from_json::<crate::quotes::cds_tranche::CdsTrancheQuote>(serde_json::json!({
        "id": "CDX-IG-42-3-7-5Y",
        "index": "CDX.NA.IG",
        "series": 42,
        "attachment": 0.03,
        "detachment": 0.07,
        "maturity": "2029-06-20",
        "upfront_pct": 4.25,
        "coupon_bp": 100.0,
        "convention": {"currency": "USD", "doc_clause": "isda_na"}
    }))
}

/// A 5Y zero-coupon US CPI swap.
fn inflation_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example_from_json::<crate::quotes::inflation::InflationQuote>(serde_json::json!({
        "inflation_swap": {
            "id": "USD-CPI-ZC-5Y",
            "maturity": "2030-01-15",
            "rate": 0.0245,
            "index": "US-CPI-U",
            "convention": "USD"
        }
    }))
}

fn rate_quote_example() -> finstack_quant_core::Result<crate::quotes::rates::RateQuote> {
    Ok(crate::quotes::rates::RateQuote::Deposit {
        id: crate::quotes::ids::QuoteId::new("USD-SOFR-DEP-1M"),
        index: finstack_quant_core::types::IndexId::new("USD-SOFR"),
        pillar: crate::quotes::ids::Pillar::Tenor("1M".parse()?),
        rate: 0.0525,
    })
}

/// A one-month USD SOFR deposit.
fn rate_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example(&rate_quote_example()?)
}

/// A one-year at-the-money equity call volatility.
fn vol_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example_from_json::<crate::quotes::vol::VolQuote>(serde_json::json!({
        "option_vol": {
            "id": "SPX-1Y-5000-C",
            "underlying": "SPX",
            "expiry": "2026-01-16",
            "strike": 5000.0,
            "vol": 0.18,
            "option_type": "call"
        }
    }))
}

/// A 5Y EUR/USD cross-currency basis swap.
fn xccy_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    example_from_json::<crate::quotes::xccy::XccyQuote>(serde_json::json!({
        "id": "EURUSD-XCCY-5Y",
        "convention": "EUR/USD-XCCY",
        "far_pillar": {"tenor": {"count": 5, "unit": "years"}},
        "basis_spread_bp": -12.5
    }))
}

/// Return the calibration schema registry as a shared, lazily built slice.
#[must_use]
pub fn artifacts() -> &'static [SchemaArtifact] {
    static CACHE: OnceLock<Vec<SchemaArtifact>> = OnceLock::new();
    CACHE.get_or_init(build_artifacts)
}

fn build_artifacts() -> Vec<SchemaArtifact> {
    vec![
        SchemaArtifact::new::<crate::api::schema::CalibrationEnvelope>(
            "schemas/calibration/1/calibration.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/calibration.schema.json",
            "Calibration",
            "Canonical typed calibration request and result envelope.",
        )
        .with_packager(finstack_quant_valuations::schema::package_valuations_schema)
        .with_kind(SchemaKind::Input)
        .with_summary(
            "Build a market from quotes: a calibration plan, flat market data, and any pre-built curves or surfaces.",
        )
        .with_examples(calibration_examples),
        SchemaArtifact::new::<crate::api::schema::CalibrationResultEnvelope>(
            "schemas/calibration/1/calibration_result.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/calibration_result.schema.json",
            "Calibration Result",
            "Canonical typed calibration result envelope.",
        )
        .with_packager(finstack_quant_valuations::schema::package_valuations_schema)
        .with_kind(SchemaKind::Output)
        .with_summary(
            "The calibrated market snapshot with the plan-level and per-step fit reports.",
        )
        .with_examples(calibration_result_examples),
        SchemaArtifact::new::<crate::api::validate::CalibrationValidationReport>(
            "schemas/calibration/1/calibration_validation_report.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/calibration_validation_report.schema.json",
            "Calibration Validation Report",
            "Structural validation findings and the static step dependency graph.",
        )
        .with_packager(finstack_quant_valuations::schema::package_valuations_schema)
        .with_kind(SchemaKind::Output)
        .with_summary("Dry-run findings for a calibration envelope; nothing is solved.")
        .with_examples(calibration_validation_report_examples),
        SchemaArtifact::new::<crate::quotes::market_quote::MarketQuote>(
            "schemas/market/1/market_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/market_quote.schema.json",
            "Market Quote",
            "Canonical tagged market quote.",
        )
        .with_packager(finstack_quant_valuations::schema::package_valuations_schema)
        .with_summary("One market observation, tagged by asset class.")
        .with_examples(market_quote_examples),
        component::<crate::api::schema::StepParams>(
            "schemas/calibration/1/step_params.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/step_params.schema.json",
            "Step Params",
            "Kind-tagged parameters of one calibration step, flattened into `CalibrationStep`.",
        )
        .with_examples(step_params_examples),
        component::<crate::hull_white::SwaptionQuote>(
            "schemas/calibration/1/hull_white_swaption_quote.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/hull_white_swaption_quote.schema.json",
            "Hull-White Swaption Quote",
            "ATM swaption volatility quote in year fractions for the direct Hull-White calibrator.",
        )
        .with_examples(hull_white_swaption_quote_examples),
        component::<crate::hull_white::CapFloorQuote>(
            "schemas/calibration/1/hull_white_cap_floor_quote.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/hull_white_cap_floor_quote.schema.json",
            "Hull-White Cap/Floor Quote",
            "Cap or floor volatility quote in year fractions for the direct Hull-White calibrators.",
        )
        .with_examples(hull_white_cap_floor_quote_examples),
        component::<crate::hull_white::CapFloorCalibrationConfig>(
            "schemas/calibration/1/hull_white_cap_floor_calibration_config.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/hull_white_cap_floor_calibration_config.schema.json",
            "Hull-White Cap/Floor Calibration Config",
            "Settings of the scalar Hull-White fit to cap/floor quotes.",
        )
        .with_examples(hull_white_cap_floor_config_examples),
        component::<crate::hull_white::PiecewiseSigmaCalibrationConfig>(
            "schemas/calibration/1/hull_white_piecewise_sigma_calibration_config.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/hull_white_piecewise_sigma_calibration_config.schema.json",
            "Hull-White Piecewise Sigma Calibration Config",
            "Settings of the piecewise-constant Hull-White sigma bootstrap to cap/floor quotes.",
        )
        .with_examples(hull_white_piecewise_sigma_config_examples),
        component::<crate::quotes::cds::CdsQuote>(
            "schemas/market/1/cds_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/cds_quote.schema.json",
            "CDS Quote",
            "Single-name CDS par-spread or upfront quote.",
        )
        .with_examples(cds_quote_examples),
        component::<crate::quotes::cds_tranche::CdsTrancheQuote>(
            "schemas/market/1/cds_tranche_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/cds_tranche_quote.schema.json",
            "CDS Tranche Quote",
            "Index tranche upfront quote.",
        )
        .with_examples(cds_tranche_quote_examples),
        component::<crate::quotes::inflation::InflationQuote>(
            "schemas/market/1/inflation_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/inflation_quote.schema.json",
            "Inflation Quote",
            "Zero-coupon or year-on-year inflation swap quote.",
        )
        .with_examples(inflation_quote_examples),
        component::<crate::quotes::rates::RateQuote>(
            "schemas/market/1/rate_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/rate_quote.schema.json",
            "Rate Quote",
            "Deposit, FRA, futures or swap rate quote.",
        )
        .with_examples(rate_quote_examples),
        component::<crate::quotes::vol::VolQuote>(
            "schemas/market/1/vol_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/vol_quote.schema.json",
            "Vol Quote",
            "Option, swaption or cap/floor volatility quote.",
        )
        .with_examples(vol_quote_examples),
        component::<crate::quotes::xccy::XccyQuote>(
            "schemas/market/1/xccy_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/xccy_quote.schema.json",
            "Cross-Currency Quote",
            "Cross-currency basis swap quote.",
        )
        .with_examples(xccy_quote_examples),
    ]
}

/// Register a reusable definition that other calibration contracts embed.
///
/// Each one is published on its own so its Rust type name survives into the
/// generated host declarations, even where serde flattens it into a parent.
fn component<T: finstack_quant_core::schema::SerdeSchema>(
    relative_path: &'static str,
    id: &'static str,
    title: &'static str,
    description: &'static str,
) -> SchemaArtifact {
    SchemaArtifact::new::<T>(relative_path, id, title, description)
        .with_packager(finstack_quant_valuations::schema::package_valuations_schema)
}
