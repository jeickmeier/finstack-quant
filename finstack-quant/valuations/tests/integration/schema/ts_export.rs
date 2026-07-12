use finstack_quant_valuations::calibration::api::market_datum::{
    CollateralEntry, DividendScheduleDatum, FxSpotDatum, MarketDatum, PriceDatum,
};
use finstack_quant_valuations::calibration::api::prior_market::PriorMarketObject;
use finstack_quant_valuations::calibration::api::schema::{
    AtmStrikeConvention, BaseCorrelationParams, CalibrationEnvelope, CalibrationPlan,
    CalibrationResult, CalibrationResultEnvelope, CalibrationStep, CapFloorHullWhiteStepParams,
    DiscountCurveParams, ForwardCurveParams, HazardCurveParams, HullWhiteStepParams,
    InflationCurveParams, ParametricCurveParams, SabrInterpolationMethod, SeasonalFactors,
    StepParams, StudentTParams, SurfaceExtrapolationPolicy, SviSurfaceParams,
    SwaptionVolConvention, SwaptionVolParams, VolSurfaceParams, XccyBasisParams,
};
use finstack_quant_valuations::calibration::{
    CalibrationConfig, CalibrationDiagnostics, CalibrationMethod, CalibrationReport,
    DiscountCurveSolveConfig, HazardCurveSolveConfig, InflationCurveSolveConfig, QuoteQuality,
    RateBounds, RateBoundsPolicy, RatesStepConventions, ResidualWeightingScheme, SolverConfig,
    ValidationMode,
};
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::market::quotes::bond::BondQuote;
use finstack_quant_valuations::market::quotes::cds::CdsQuote;
use finstack_quant_valuations::market::quotes::cds_tranche::CDSTrancheQuote;
use finstack_quant_valuations::market::quotes::fx::FxQuote;
use finstack_quant_valuations::market::quotes::inflation::InflationQuote;
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::rates::RateQuote as RatesQuote;
use finstack_quant_valuations::market::quotes::vol::VolQuote;
use finstack_quant_valuations::market::quotes::xccy::XccyQuote;
use std::fs;
use std::sync::OnceLock;
use ts_rs::TS;

const OUT_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../finstack-quant-wasm/types/generated"
);

static CONFIG: OnceLock<ts_rs::Config> = OnceLock::new();

fn config() -> &'static ts_rs::Config {
    CONFIG.get_or_init(|| {
        std::fs::create_dir_all(OUT_DIR).expect("create generated types dir");
        ts_rs::Config::new().with_out_dir(OUT_DIR)
    })
}

fn normalize_generated_types() {
    for entry in fs::read_dir(OUT_DIR).expect("read generated types dir") {
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
    CalibrationMethod::export(cfg).expect("export CalibrationMethod");
    DiscountCurveSolveConfig::export(cfg).expect("export DiscountCurveSolveConfig");
    HazardCurveSolveConfig::export(cfg).expect("export HazardCurveSolveConfig");
    InflationCurveSolveConfig::export(cfg).expect("export InflationCurveSolveConfig");
    RateBoundsPolicy::export(cfg).expect("export RateBoundsPolicy");
    RatesStepConventions::export(cfg).expect("export RatesStepConventions");
    ResidualWeightingScheme::export(cfg).expect("export ResidualWeightingScheme");
    SolverConfig::export(cfg).expect("export SolverConfig");
    RateBounds::export(cfg).expect("export RateBounds");
    ValidationMode::export(cfg).expect("export ValidationMode");

    BondQuote::export(cfg).expect("export BondQuote");
    RatesQuote::export(cfg).expect("export RatesQuote");
    CdsQuote::export(cfg).expect("export CdsQuote");
    CDSTrancheQuote::export(cfg).expect("export CDSTrancheQuote");
    FxQuote::export(cfg).expect("export FxQuote");
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
    CalibrationPlan::export(cfg).expect("export CalibrationPlan");
    CalibrationStep::export(cfg).expect("export CalibrationStep");

    // v3 envelope payload types (market_data / prior_market lists).
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
    CapFloorHullWhiteStepParams::export(cfg).expect("export CapFloorHullWhiteStepParams");
    SviSurfaceParams::export(cfg).expect("export SviSurfaceParams");
    XccyBasisParams::export(cfg).expect("export XccyBasisParams");
    ParametricCurveParams::export(cfg).expect("export ParametricCurveParams");

    // Supporting enums
    SurfaceExtrapolationPolicy::export(cfg).expect("export SurfaceExtrapolationPolicy");
    SwaptionVolConvention::export(cfg).expect("export SwaptionVolConvention");
    AtmStrikeConvention::export(cfg).expect("export AtmStrikeConvention");
    SabrInterpolationMethod::export(cfg).expect("export SabrInterpolationMethod");

    // Result envelope types
    CalibrationResultEnvelope::export(cfg).expect("export CalibrationResultEnvelope");
    CalibrationResult::export(cfg).expect("export CalibrationResult");

    // Report types
    CalibrationReport::export(cfg).expect("export CalibrationReport");
    CalibrationDiagnostics::export(cfg).expect("export CalibrationDiagnostics");
    QuoteQuality::export(cfg).expect("export QuoteQuality");
    normalize_generated_types();
}
