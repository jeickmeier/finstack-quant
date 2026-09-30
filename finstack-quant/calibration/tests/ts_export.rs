//! Tests for the surrounding crate component and its documented behavior.
#![cfg(feature = "ts_export")]
//!
use finstack_quant_calibration::api::errors::{EnvelopeError, StrictLoadDiagnostic};
use finstack_quant_calibration::api::market_datum::{
    CollateralEntry, DividendScheduleDatum, FxSpotDatum, MarketDatum, PriceDatum,
};
use finstack_quant_calibration::api::prior_market::PriorMarketObject;
use finstack_quant_calibration::api::schema::{
    BaseCorrelationParams, CalibrationEnvelope, CalibrationPlan, CalibrationResult,
    CalibrationResultEnvelope, CalibrationSchema, CalibrationStep, CapFloorHullWhiteStepParams,
    DiscountCurveParams, ForwardCurveParams, HazardCurveParams, HullWhiteStepParams,
    HullWhiteVolatilityMode, InflationCurveParams, ParametricCurveParams, SabrInterpolationMethod,
    SeasonalFactors, StepParams, StudentTParams, SurfaceExtrapolationPolicy, SviSurfaceParams,
    SwaptionVolConvention, SwaptionVolParams, VolSurfaceModel, VolSurfaceParams, XccyBasisParams,
};
use finstack_quant_calibration::api::validate::{
    CalibrationValidationReport, DependencyGraph, DependencyNode,
};
use finstack_quant_calibration::quotes::cds::CdsQuote;
use finstack_quant_calibration::quotes::cds_tranche::CdsTrancheQuote;
use finstack_quant_calibration::quotes::inflation::InflationQuote;
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::quotes::rates::RateQuote as RatesQuote;
use finstack_quant_calibration::quotes::vol::VolQuote;
use finstack_quant_calibration::quotes::xccy::XccyQuote;
use finstack_quant_calibration::{
    CalibrationConfig, CalibrationDiagnostics, CalibrationMethod, CalibrationReport,
    DiscountCurveSolveConfig, ForwardCurveSolveConfig, HazardCurveSolveConfig,
    InflationCurveSolveConfig, MarketFreshnessPolicy, MarketQuoteSide, QuoteQuality, RateBounds,
    RateBoundsPolicy, RatesStepConventions, ResidualWeightingScheme, SolverConfig, ValidationMode,
    VolSurfaceSolveConfig,
};
use finstack_quant_valuations::instruments::OptionType;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use ts_rs::TS;

static CONFIG: OnceLock<ts_rs::Config> = OnceLock::new();
static OUTPUT_DIR: OnceLock<PathBuf> = OnceLock::new();

fn output_dir() -> &'static Path {
    OUTPUT_DIR
        .get_or_init(|| {
            std::env::var_os("FINSTACK_TS_OUTPUT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../finstack-quant-wasm/types/generated")
                })
        })
        .as_path()
}

fn config() -> &'static ts_rs::Config {
    CONFIG.get_or_init(|| {
        std::fs::create_dir_all(output_dir()).expect("create generated types dir");
        ts_rs::Config::new().with_out_dir(output_dir())
    })
}

fn normalize_generated_types() {
    for entry in fs::read_dir(output_dir()).expect("read generated types dir") {
        let path = entry.expect("read generated type entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("ts") {
            continue;
        }

        let contents = fs::read_to_string(&path).expect("read generated type");
        let normalized = contents
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        if normalized != contents {
            fs::write(&path, normalized).expect("write normalized generated type");
        }
    }
}

#[test]
fn export_calibration_types() {
    let cfg = config();

    CalibrationConfig::export(cfg).expect("export CalibrationConfig");
    MarketFreshnessPolicy::export(cfg).expect("export MarketFreshnessPolicy");
    MarketQuoteSide::export(cfg).expect("export MarketQuoteSide");
    CalibrationMethod::export(cfg).expect("export CalibrationMethod");
    DiscountCurveSolveConfig::export(cfg).expect("export DiscountCurveSolveConfig");
    ForwardCurveSolveConfig::export(cfg).expect("export ForwardCurveSolveConfig");
    HazardCurveSolveConfig::export(cfg).expect("export HazardCurveSolveConfig");
    InflationCurveSolveConfig::export(cfg).expect("export InflationCurveSolveConfig");
    VolSurfaceSolveConfig::export(cfg).expect("export VolSurfaceSolveConfig");
    RateBoundsPolicy::export(cfg).expect("export RateBoundsPolicy");
    RatesStepConventions::export(cfg).expect("export RatesStepConventions");
    ResidualWeightingScheme::export(cfg).expect("export ResidualWeightingScheme");
    SolverConfig::export(cfg).expect("export SolverConfig");
    RateBounds::export(cfg).expect("export RateBounds");
    ValidationMode::export(cfg).expect("export ValidationMode");

    RatesQuote::export(cfg).expect("export RatesQuote");
    CdsQuote::export(cfg).expect("export CdsQuote");
    CdsTrancheQuote::export(cfg).expect("export CdsTrancheQuote");
    VolQuote::export(cfg).expect("export VolQuote");
    OptionType::export(cfg).expect("export OptionType");
    InflationQuote::export(cfg).expect("export InflationQuote");
    XccyQuote::export(cfg).expect("export XccyQuote");
    MarketQuote::export(cfg).expect("export MarketQuote");
    normalize_generated_types();
}

#[test]
fn export_calibration_envelope_types() {
    let cfg = config();

    // Envelope and plan types
    CalibrationEnvelope::export(cfg).expect("export CalibrationEnvelope");
    CalibrationSchema::export(cfg).expect("export CalibrationSchema");
    CalibrationPlan::export(cfg).expect("export CalibrationPlan");
    CalibrationStep::export(cfg).expect("export CalibrationStep");

    // Envelope payload types (market_data / prior_market lists).
    MarketDatum::export(cfg).expect("export MarketDatum");
    FxSpotDatum::export(cfg).expect("export FxSpotDatum");
    PriceDatum::export(cfg).expect("export PriceDatum");
    DividendScheduleDatum::export(cfg).expect("export DividendScheduleDatum");
    CollateralEntry::export(cfg).expect("export CollateralEntry");
    PriorMarketObject::export(cfg).expect("export PriorMarketObject");

    // StepParams tagged union and all variant Params
    StepParams::export(cfg).expect("export StepParams");
    DiscountCurveParams::export(cfg).expect("export DiscountCurveParams");
    ForwardCurveParams::export(cfg).expect("export ForwardCurveParams");
    HazardCurveParams::export(cfg).expect("export HazardCurveParams");
    InflationCurveParams::export(cfg).expect("export InflationCurveParams");
    SeasonalFactors::export(cfg).expect("export SeasonalFactors");
    VolSurfaceParams::export(cfg).expect("export VolSurfaceParams");
    SwaptionVolParams::export(cfg).expect("export SwaptionVolParams");
    BaseCorrelationParams::export(cfg).expect("export BaseCorrelationParams");
    StudentTParams::export(cfg).expect("export StudentTParams");
    HullWhiteStepParams::export(cfg).expect("export HullWhiteStepParams");
    HullWhiteVolatilityMode::export(cfg).expect("export HullWhiteVolatilityMode");
    CapFloorHullWhiteStepParams::export(cfg).expect("export CapFloorHullWhiteStepParams");
    SviSurfaceParams::export(cfg).expect("export SviSurfaceParams");
    XccyBasisParams::export(cfg).expect("export XccyBasisParams");
    ParametricCurveParams::export(cfg).expect("export ParametricCurveParams");

    // Supporting enums
    SurfaceExtrapolationPolicy::export(cfg).expect("export SurfaceExtrapolationPolicy");
    VolSurfaceModel::export(cfg).expect("export VolSurfaceModel");
    SwaptionVolConvention::export(cfg).expect("export SwaptionVolConvention");
    SabrInterpolationMethod::export(cfg).expect("export SabrInterpolationMethod");

    // Result envelope types
    CalibrationResultEnvelope::export(cfg).expect("export CalibrationResultEnvelope");
    CalibrationResult::export(cfg).expect("export CalibrationResult");

    // Report types
    CalibrationReport::export(cfg).expect("export CalibrationReport");
    CalibrationDiagnostics::export(cfg).expect("export CalibrationDiagnostics");
    QuoteQuality::export(cfg).expect("export QuoteQuality");

    // Static validation report (`dryRun`) and its error payloads
    CalibrationValidationReport::export(cfg).expect("export CalibrationValidationReport");
    DependencyGraph::export(cfg).expect("export DependencyGraph");
    DependencyNode::export(cfg).expect("export DependencyNode");
    EnvelopeError::export(cfg).expect("export EnvelopeError");
    StrictLoadDiagnostic::export(cfg).expect("export StrictLoadDiagnostic");
    normalize_generated_types();
}
