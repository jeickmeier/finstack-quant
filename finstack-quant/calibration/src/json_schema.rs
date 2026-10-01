//! Checked-in JSON schemas owned by the calibration crate.

use finstack_quant_core::schema::{SchemaArtifact, SchemaKind};
use serde_json::Value;
use std::sync::OnceLock;

fn calibration_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let plan = crate::api::schema::CalibrationPlan {
        id: "usd_curves".to_string(),
        description: Some("Bootstrap the USD OIS discount curve.".to_string()),
        quote_sets: Default::default(),
        steps: Vec::new(),
        settings: Default::default(),
    };
    let envelope = crate::api::schema::CalibrationEnvelope::new(plan, Vec::new(), Vec::new());
    serde_json::to_value(envelope)
        .map(|value| vec![value])
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!(
                "serialize calibration schema example: {error}"
            ))
        })
}

fn market_quote_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let quote =
        crate::quotes::market_quote::MarketQuote::Rates(crate::quotes::rates::RateQuote::Deposit {
            id: crate::quotes::ids::QuoteId::new("USD-SOFR-DEP-1M"),
            index: finstack_quant_core::types::IndexId::new("USD-SOFR"),
            pillar: crate::quotes::ids::Pillar::Tenor("1M".parse()?),
            rate: 0.0525,
        });
    serde_json::to_value(quote)
        .map(|value| vec![value])
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!(
                "serialize market quote schema example: {error}"
            ))
        })
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
        ),
        SchemaArtifact::new::<crate::api::validate::CalibrationValidationReport>(
            "schemas/calibration/1/calibration_validation_report.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/calibration_validation_report.schema.json",
            "Calibration Validation Report",
            "Structural validation findings and the static step dependency graph.",
        )
        .with_packager(finstack_quant_valuations::schema::package_valuations_schema)
        .with_kind(SchemaKind::Output)
        .with_summary("Dry-run findings for a calibration envelope; nothing is solved."),
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
        ),
        component::<crate::hull_white::SwaptionQuote>(
            "schemas/calibration/1/hull_white_swaption_quote.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/hull_white_swaption_quote.schema.json",
            "Hull-White Swaption Quote",
            "ATM swaption volatility quote in year fractions for the direct Hull-White calibrator.",
        ),
        component::<crate::hull_white::CapFloorQuote>(
            "schemas/calibration/1/hull_white_cap_floor_quote.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/hull_white_cap_floor_quote.schema.json",
            "Hull-White Cap/Floor Quote",
            "Cap or floor volatility quote in year fractions for the direct Hull-White calibrators.",
        ),
        component::<crate::hull_white::CapFloorCalibrationConfig>(
            "schemas/calibration/1/hull_white_cap_floor_calibration_config.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/hull_white_cap_floor_calibration_config.schema.json",
            "Hull-White Cap/Floor Calibration Config",
            "Settings of the scalar Hull-White fit to cap/floor quotes.",
        ),
        component::<crate::hull_white::PiecewiseSigmaCalibrationConfig>(
            "schemas/calibration/1/hull_white_piecewise_sigma_calibration_config.schema.json",
            "https://finstack_quant.dev/schemas/calibration/1/hull_white_piecewise_sigma_calibration_config.schema.json",
            "Hull-White Piecewise Sigma Calibration Config",
            "Settings of the piecewise-constant Hull-White sigma bootstrap to cap/floor quotes.",
        ),
        component::<crate::quotes::cds::CdsQuote>(
            "schemas/market/1/cds_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/cds_quote.schema.json",
            "CDS Quote",
            "Single-name CDS par-spread or upfront quote.",
        ),
        component::<crate::quotes::cds_tranche::CdsTrancheQuote>(
            "schemas/market/1/cds_tranche_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/cds_tranche_quote.schema.json",
            "CDS Tranche Quote",
            "Index tranche upfront quote.",
        ),
        component::<crate::quotes::inflation::InflationQuote>(
            "schemas/market/1/inflation_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/inflation_quote.schema.json",
            "Inflation Quote",
            "Zero-coupon or year-on-year inflation swap quote.",
        ),
        component::<crate::quotes::rates::RateQuote>(
            "schemas/market/1/rate_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/rate_quote.schema.json",
            "Rate Quote",
            "Deposit, FRA, futures or swap rate quote.",
        ),
        component::<crate::quotes::vol::VolQuote>(
            "schemas/market/1/vol_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/vol_quote.schema.json",
            "Vol Quote",
            "Option, swaption or cap/floor volatility quote.",
        ),
        component::<crate::quotes::xccy::XccyQuote>(
            "schemas/market/1/xccy_quote.schema.json",
            "https://finstack_quant.dev/schemas/market/1/xccy_quote.schema.json",
            "Cross-Currency Quote",
            "Cross-currency basis swap quote.",
        ),
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
