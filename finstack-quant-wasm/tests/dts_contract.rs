//! Contract test: generated TypeScript declarations match the facade surface.

use std::fs;
use std::path::PathBuf;

fn index_dts() -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest_dir.join("index.d.ts"))
        .expect("read finstack-quant-wasm/index.d.ts")
}

fn benchmark_script() -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest_dir.join("benchmarks/bench.mjs"))
        .expect("read finstack-quant-wasm/benchmarks/bench.mjs")
}

fn contains_signature(dts: &str, sig: &str) -> bool {
    contains_ignoring_ws(dts, sig)
}

fn contains_ignoring_ws(haystack: &str, needle: &str) -> bool {
    let compact_haystack: String = haystack.chars().filter(|c| !c.is_whitespace()).collect();
    let compact_needle: String = needle.chars().filter(|c| !c.is_whitespace()).collect();
    compact_haystack.contains(&compact_needle)
}

fn interface_block<'a>(dts: &'a str, interface_name: &str) -> &'a str {
    let start = dts
        .find(&format!("export interface {interface_name}"))
        .unwrap_or_else(|| panic!("{interface_name} interface declaration missing"));
    let rest = &dts[start..];
    let end = rest
        .find("\n}\n")
        .unwrap_or_else(|| panic!("{interface_name} interface declaration is unterminated"));
    &rest[..end]
}

#[test]
fn credit_factor_hierarchy_dts_exposes_public_surface() {
    let dts = index_dts();

    // Classes
    assert!(dts.contains("export declare class CreditFactorModel {"));
    assert!(contains_signature(
        &dts,
        "static fromJson(s: string): CreditFactorModel;"
    ));
    assert!(contains_signature(&dts, "toJson(): string;"));

    assert!(dts.contains("export declare class CreditCalibrator {"));
    assert!(contains_signature(&dts, "constructor(configJson: string);"));
    assert!(contains_signature(
        &dts,
        "calibrate(inputsJson: string): CreditFactorModel;"
    ));

    assert!(dts.contains("export declare class LevelsAtDate {"));
    assert!(dts.contains("export declare class PeriodDecomposition {"));

    assert!(dts.contains("export declare class FactorCovarianceForecast {"));
    assert!(contains_signature(
        &dts,
        "constructor(model: CreditFactorModel);"
    ));
    assert!(contains_signature(
        &dts,
        "covarianceAt(horizonJson: string): string;"
    ));
    assert!(contains_signature(
        &dts,
        "idiosyncraticVol(issuerId: string, horizonJson: string): number;"
    ));
    assert!(contains_signature(
        &dts,
        "factorModelAt(horizonJson: string, riskMeasureJson: string): string;"
    ));

    // Free functions
    assert!(contains_signature(
        &dts,
        "export declare function decomposeLevels(",
    ));
    assert!(contains_signature(
        &dts,
        "export declare function decomposePeriod(",
    ));

    // FactorModelCreditNamespace entries
    assert!(dts.contains("CreditFactorModel: typeof CreditFactorModel;"));
    assert!(dts.contains("CreditCalibrator: typeof CreditCalibrator;"));
    assert!(dts.contains("FactorCovarianceForecast: typeof FactorCovarianceForecast;"));
    assert!(dts.contains("decomposeLevels("));
    assert!(dts.contains(
        "decomposePeriod(fromLevels: LevelsAtDate, toLevels: LevelsAtDate): PeriodDecomposition;"
    ));
}

#[test]
fn analytics_dts_matches_runtime_hotspots() {
    let dts = index_dts();

    assert!(dts.contains("export declare class Performance {"));
    assert!(dts.contains("Performance: typeof Performance;"));
    assert!(contains_ignoring_ws(
        &dts,
        "static fromReturns(dates: string[], returns: NumericMatrix, tickerNames: string[], benchmarkTicker?: string | null, freq?: string): Performance;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "rollingGreeks(tickerIdx: number, window?: number, riskFreeRate?: number): RollingGreeksResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "activeDatesForTicker(tickerIdx: number): string[];",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "lookbackReturns(refDate: string, fiscalYearStartMonth?: number, fiscalYearStartDay?: number, calendar?: string): LookbackReturns;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "rollingReturns(tickerIdx: number, window: number): DatedSeries;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "multiFactorGreeks(tickerIdx: number, factorReturns: NumericMatrix): MultiFactorResult;",
    ));
    assert!(contains_ignoring_ws(&dts, "maxDrawdown(): Float64Array;"));
    assert!(contains_ignoring_ws(&dts, "meanDrawdown(): Float64Array;"));
    // GARCH / VaR-backtesting / ruin types and free functions must be gone.
    assert!(!dts.contains("fitGarch11"));
    assert!(!dts.contains("rollingVarForecasts"));
    assert!(!dts.contains("rollingVarBatch"));
    assert!(!dts.contains("RuinModel"));
    assert!(!dts.contains("BacktestResultJson"));
}

#[test]
fn core_dts_exposes_typed_array_math_fast_paths() {
    let dts = index_dts();

    assert!(contains_ignoring_ws(
        &dts,
        "choleskyDecompositionFlat(matrix: NumericArray, n: number): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "choleskySolveFlat(chol: NumericArray, b: NumericArray, n: number): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "validateCorrelationMatrixFlat(matrix: NumericArray, n: number): void;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "meanArray(data: NumericArray): number;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "correlationArray(x: NumericArray, y: NumericArray): number;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "kahanSumArray(values: NumericArray): number;",
    ));
}

#[test]
fn forward_curve_dts_exposes_projection_grid_and_rate_between() {
    let dts = index_dts();
    let curve = interface_block(&dts, "ForwardCurve");
    let constructor = interface_block(&dts, "ForwardCurveConstructor");

    assert!(contains_ignoring_ws(
        curve,
        "readonly projectionGrid: Float64Array | null;"
    ));
    assert!(contains_ignoring_ws(
        curve,
        "rateBetween(t1: number, t2: number): number;"
    ));
    assert!(contains_ignoring_ws(curve, "readonly resetLag: number;"));
    assert!(contains_ignoring_ws(
        constructor,
        "projectionGrid?: NumericArray | null"
    ));
    assert!(contains_ignoring_ws(constructor, "knots: NumericArray"));
    assert!(contains_ignoring_ws(
        constructor,
        "resetLag?: number | null"
    ));
}

#[test]
fn discount_curve_dts_exposes_canonical_validation_and_forward_names() {
    let dts = index_dts();
    let curve = interface_block(&dts, "DiscountCurve ");
    let constructor = interface_block(&dts, "DiscountCurveConstructor");

    assert!(contains_signature(
        curve,
        "forward(t1: number, t2: number): number;"
    ));
    assert!(!curve.contains("forwardRate"));
    assert!(constructor.contains("validationMode?: DiscountCurveValidationMode"));
    assert!(constructor.contains("forwardFloor?: number | null"));
    assert!(contains_ignoring_ws(constructor, "knots: NumericArray"));
    assert!(dts.contains(
        "export type DiscountCurveValidationMode = 'market_standard' | 'negative_rate_friendly';"
    ));
}

/// M2.21 — the correlation namespace's `Vec<f64>` returns cross the WASM
/// boundary as `Float64Array`, and the hand-written d.ts must say so.
#[test]
fn valuations_correlation_dts_uses_float64array_returns() {
    let dts = index_dts();

    assert!(dts.contains("export interface CorrelationNamespace"));
    assert!(contains_ignoring_ws(
        &dts,
        "correlationBounds(p1: number, p2: number): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "jointProbabilities(p1: number, p2: number, correlation: number): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "validateCorrelationMatrix(matrix: NumericArray, n: number): void;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "nearestCorrelation(matrix: NumericArray, n: number, maxIter?: number, tol?: number): Float64Array;",
    ));
    // The stale `number[]` declarations must be gone from this namespace.
    assert!(!dts.contains("correlationBounds(p1: number, p2: number): number[];"));
    assert!(
        !dts.contains("jointProbabilities(p1: number, p2: number, correlation: number): number[];")
    );
}

#[test]
fn cashflows_dts_matches_json_bridge_surface() {
    let dts = index_dts();

    assert!(dts.contains("export interface CashflowsNamespace"));
    assert!(dts.contains(
        "buildCashflowScheduleJson(specJson: string, marketJson?: string | null): string;"
    ));
    assert!(dts.contains("validateCashflowScheduleJson(scheduleJson: string): string;"));
    assert!(!dts.contains("CashflowScheduleEnvelope"));
    assert!(!dts.contains("buildCashflowScheduleEnvelopeJson"));
    assert!(!dts.contains("validateCashflowScheduleEnvelopeJson"));
    assert!(dts.contains("datedFlowsJson(scheduleJson: string): string;"));
    assert!(dts.contains("accruedInterestJson("));
    assert!(dts.contains("bondFromCashflowsJson("));
    assert!(dts.contains("export declare const cashflows: CashflowsNamespace;"));
    let generated = include_str!("../types/generated/CashflowSchedule.ts");
    assert!(generated.contains("method: \"Linear\" | \"Compounded\";"));
}

#[test]
fn valuations_dts_exposes_direct_fx_instruments() {
    let dts = index_dts();

    assert!(dts.contains("export interface FxNamespace"));
    assert!(dts.contains("FxSpot: FxInstrumentConstructor<FxInstrument>;"));
    assert!(dts.contains("FxForward: FxInstrumentConstructor<FxInstrument>;"));
    assert!(dts.contains("FxSwap: FxInstrumentConstructor<FxInstrument>;"));
    assert!(dts.contains("Ndf: FxInstrumentConstructor<FxInstrument>;"));
    assert!(dts.contains("FxOption: FxInstrumentConstructor<FxOptionInstrument>;"));
    assert!(dts.contains("FxBarrierOption: FxInstrumentConstructor<FxBarrierOptionInstrument>;"));
    assert!(dts.contains("FxDigitalOption: FxInstrumentConstructor<FxDigitalOptionInstrument>;"));
    assert!(dts.contains("FxTouchOption: FxInstrumentConstructor<FxTouchOptionInstrument>;"));
    assert!(dts.contains("QuantoOption: FxInstrumentConstructor<FxOptionInstrument>;"));
    assert!(dts.contains("fx: FxNamespace;"));
    assert!(dts
        .contains("foreignRho(marketJson: string, asOf: string, model?: string | null): number;"));
    assert!(contains_ignoring_ws(
        &dts,
        "greeks(marketJson: string, asOf: string, model?: string | null): Record<string, number>;",
    ));
}

#[test]
fn valuations_dts_exposes_reusable_market_handle_pricing() {
    let dts = index_dts();

    assert!(dts.contains("export declare class Market {"));
    assert!(contains_ignoring_ws(
        &dts,
        "priceInstrumentWithMarket(instrumentJson: string, market: Market, asOf: string, model: string): string;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "priceInstrumentWithMetricsAndMarket(instrumentJson: string, market: Market, asOf: string, model: string, metrics: string[], pricingOptions?: string | null, marketHistory?: string | null): string;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "instrumentCashflowsWithMarket(instrumentJson: string, market: Market, asOf: string, model: string): string;",
    ));
}

#[test]
fn portfolio_cashflow_api_uses_full_cashflow_name_everywhere() {
    let dts = index_dts();
    let bench = benchmark_script();

    assert!(dts.contains("aggregateFullCashflows(specJson: string, marketJson: string): string;"));
    assert!(!dts.contains("aggregateCashflows("));
    assert!(bench.contains("aggregateFullCashflows"));
    assert!(!bench.contains("aggregateCashflows"));
}

#[test]
fn package_dts_documents_hand_facade_over_raw_wasm_bindgen_types() {
    let dts = index_dts();

    assert!(dts.contains("not the package root contract"));
    assert!(dts.contains("export declare const core: CoreNamespace;"));
    assert!(dts.contains("export declare const analytics: AnalyticsNamespace;"));
    assert!(dts.contains("export declare const factor_model: FactorModelNamespace;"));
    assert!(dts.contains("export declare const features: FeaturesNamespace;"));
    assert!(dts.contains("export declare const valuations: ValuationsNamespace;"));
    assert!(dts.contains("export declare const portfolio: PortfolioNamespace;"));
    assert!(dts.contains("generated `types/generated/*` files"));
}

#[test]
fn scenarios_dts_matches_json_bridge_surface() {
    let dts = index_dts();

    assert!(dts.contains("export interface ScenariosNamespace"));
    assert!(dts.contains("export interface ScenarioWarning"));
    assert!(contains_ignoring_ws(&dts, "warnings: ScenarioWarning[];"));
    assert!(contains_ignoring_ws(
        &dts,
        "computeHorizonReturn(instrumentJson: string, marketJson: string, asOf: string, scenarioJson: string, method?: string, configJson?: string): string;",
    ));
    assert!(dts.contains("export declare const scenarios: ScenariosNamespace;"));
}

#[test]
fn portfolio_dts_exposes_reference_price_for_almgren_chriss() {
    let dts = index_dts();

    assert!(dts.contains("referencePrice?: number | null"));
}

#[test]
fn core_daycount_dts_exposes_context_for_context_dependent_conventions() {
    let dts = index_dts();

    assert!(dts.contains("export interface DayCountContext"));
    assert!(contains_ignoring_ws(
        &dts,
        "yearFractionWithContext(startEpochDays: number, endEpochDays: number, ctx: DayCountContext): number;",
    ));
    assert!(dts.contains("DayCountContext: DayCountContextConstructor;"));
    let day_count_ctor = interface_block(&dts, "DayCountConstructor");
    assert!(contains_signature(
        day_count_ctor,
        "thirtyE360Isda(): DayCount;"
    ));
    let day_count = interface_block(&dts, "DayCount ");
    assert!(contains_signature(
        day_count,
        "calendarDays(startEpochDays: number, endEpochDays: number): bigint;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "ActActIsma and Bus252 require explicit context"
    ));
    assert!(contains_ignoring_ws(&dts, "throws when called without it"));
}

#[test]
fn dts_documents_wasm_owned_handles_and_dispose_contract() {
    let dts = index_dts();

    assert!(dts.contains("export interface WasmOwned"));
    let owned = interface_block(&dts, "WasmOwned");
    assert!(contains_signature(owned, "free(): void;"));
    assert!(!owned.contains("Symbol.dispose"));
    assert!(dts.contains("installs `[Symbol.dispose]` as an alias of `free`"));
    assert!(!dts.contains("export { default } from './pkg/finstack_quant_wasm';"));
    assert!(dts.contains("export default function init("));

    for interface_name in [
        "Currency ",
        "Money ",
        "DayCount ",
        "DiscountCurve ",
        "ForwardCurve ",
        "VolCube ",
        "FxDeltaVolSurface ",
        "FxMatrix ",
    ] {
        let block = interface_block(&dts, interface_name);
        assert!(
            block
                .lines()
                .next()
                .is_some_and(|line| line.contains("extends WasmOwned")),
            "{interface_name} must expose wasm-bindgen ownership methods"
        );
    }

    for class_name in [
        "Performance",
        "CreditFactorModel",
        "CreditCalibrator",
        "LevelsAtDate",
        "PeriodDecomposition",
        "FactorCovarianceForecast",
        "Market",
        "Portfolio",
    ] {
        assert!(
            dts.contains(&format!(
                "export interface {class_name} extends WasmOwned {{}}"
            )),
            "{class_name} must merge the wasm ownership contract"
        );
    }
}

#[test]
fn statements_dts_matches_runtime_exports() {
    let dts = index_dts();

    assert!(dts.contains("export interface StatementsNamespace"));
    assert!(dts.contains("validateFinancialModelJson(json: string): string;"));
    assert!(dts.contains("modelNodeIds(json: string): string[];"));
    assert!(dts.contains("validateCheckSuiteSpec(json: string): string;"));
    assert!(dts.contains("export declare const statements: StatementsNamespace;"));
}

#[test]
fn statements_analytics_dts_matches_runtime_exports() {
    let dts = index_dts();

    assert!(dts.contains("export interface StatementsAnalyticsNamespace"));
    assert!(dts.contains("solved_value: number;"));
    assert!(dts.contains("updated_model_json?: string;"));
    assert!(contains_ignoring_ws(
        &dts,
        "goalSeek(modelJson: string, targetNode: string, targetPeriod: string, targetValue: number, driverNode: string, driverPeriod: string, updateModel: boolean, boundsLo?: number | null, boundsHi?: number | null): GoalSeekResult;",
    ));
    assert!(dts.contains("export interface FormulaExplanationJson"));
    assert!(contains_ignoring_ws(
        &dts,
        "explainFormula(modelJson: string, resultsJson: string, nodeId: string, period: string): FormulaExplanationJson;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "explainFormulaText(modelJson: string, resultsJson: string, nodeId: string, period: string): string;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runChecks(modelJson: string, suiteSpecJson: string, resultsJson?: string | null): string;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runThreeStatementChecks(modelJson: string, mappingJson: string, resultsJson?: string | null): string;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runCreditUnderwritingChecks(modelJson: string, mappingJson: string, resultsJson?: string | null): string;"
    ));
    assert!(dts.contains("renderCheckReportText(reportJson: string): string;"));
    assert!(dts.contains("renderCheckReportHtml(reportJson: string): string;"));
    assert!(
        dts.contains("export declare const statements_analytics: StatementsAnalyticsNamespace;")
    );
}

#[test]
fn valuations_dts_exposes_credit_namespaces() {
    let dts = index_dts();
    let valuations = interface_block(&dts, "ValuationsNamespace");

    assert!(dts.contains("export interface ValuationCreditNamespace"));
    assert!(dts.contains("mertonModelJson("));
    assert!(dts.contains("creditGradesModelJson("));
    assert!(dts.contains("toggleExerciseOptimalJson("));
    assert!(dts.contains("export interface CreditDerivativesNamespace"));
    assert!(dts.contains("creditDefaultSwapExampleJson(): string;"));
    assert!(dts.contains("cdsOptionExampleJson(): string;"));
    assert!(dts.contains("credit: ValuationCreditNamespace;"));
    assert!(dts.contains("creditDerivatives: CreditDerivativesNamespace;"));
    assert!(!valuations.contains("CreditFactorModel"));
    assert!(!valuations.contains("CreditCalibrator"));
    assert!(!valuations.contains("decomposeLevels"));
}

#[test]
fn factor_model_dts_exposes_credit_namespace() {
    let dts = index_dts();
    let factor_model = interface_block(&dts, "FactorModelNamespace");

    assert!(dts.contains("export interface FactorModelNamespace"));
    assert!(dts.contains("export interface FactorModelCreditNamespace"));
    assert!(dts.contains("credit: FactorModelCreditNamespace;"));
    assert!(!factor_model.contains("CreditFactorModel"));
    assert!(!factor_model.contains("decomposeLevels"));
    assert!(dts.contains("export declare const factor_model: FactorModelNamespace;"));
}

#[test]
fn features_dts_matches_transform_surface() {
    let dts = index_dts();
    let features = interface_block(&dts, "FeaturesNamespace");

    assert!(dts.contains("export type FeatureValue = number | null;"));
    assert!(contains_signature(
        features,
        "transformTimeseries(values: FeatureValue[], entity: string[], order: string[], op: string, params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "transformCrossSectional(values: FeatureValue[], timeKey: string[], op: string, params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "transformCrossSectionalGrouped(values: FeatureValue[], timeKey: string[], groups: string[], op: string, params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "neutralize(values: FeatureValue[], timeKey: string[], exposures: FeatureValue[][], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "transformTimeseriesPairwise(values: FeatureValue[], other: FeatureValue[], entity: string[], order: string[], op: string, params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "rollingRegressionResidual(values: FeatureValue[], exposures: FeatureValue[][], entity: string[], order: string[], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "riskScaledWeights(values: FeatureValue[], timeKey: string[], volatility: FeatureValue[], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "cleanSignal(values: FeatureValue[], timeKey: string[], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "normalizeSignal(values: FeatureValue[], timeKey: string[], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "rankToWeights(values: FeatureValue[], timeKey: string[], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "neutralizeAndZscore(values: FeatureValue[], timeKey: string[], exposures: FeatureValue[][], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "transformPanel(specJson: string): string;"
    ));
    assert!(dts.contains("export declare const features: FeaturesNamespace;"));
}

#[test]
fn core_market_data_dts_exposes_vol_cube_normal_vol_queries() {
    let dts = index_dts();

    let cube = interface_block(&dts, "VolCube ");
    assert!(contains_signature(
        cube,
        "vol(expiry: number, tenor: number, strike: number): number;"
    ));
    assert!(contains_signature(
        cube,
        "volClamped(expiry: number, tenor: number, strike: number): number;"
    ));
    assert!(contains_signature(
        cube,
        "volNormal(expiry: number, tenor: number, strike: number): number;"
    ));
    assert!(contains_signature(
        cube,
        "volNormalClamped(expiry: number, tenor: number, strike: number): number;"
    ));
    assert!(contains_signature(
        cube,
        "readonly interpolationMode: string;"
    ));
    let constructor = interface_block(&dts, "VolCubeConstructor");
    assert!(constructor.contains("interpolationMode?: string"));
    for input in ["expiries", "tenors", "paramsFlat", "forwards"] {
        assert!(
            contains_ignoring_ws(constructor, &format!("{input}: NumericArray")),
            "VolCube constructor must accept NumericArray for {input}"
        );
    }
}

#[test]
fn core_market_data_dts_exposes_fx_surface_and_rate_result() {
    let dts = index_dts();

    // FxDeltaVolSurface instance + constructor interfaces.
    let surface = interface_block(&dts, "FxDeltaVolSurface ");
    assert!(contains_signature(surface, "readonly id: string;"));
    assert!(contains_signature(
        surface,
        "readonly expiries: Float64Array;"
    ));
    assert!(contains_signature(surface, "readonly numExpiries: number;"));
    assert!(contains_signature(
        surface,
        "pillarVols(expiryIdx: number): Float64Array;"
    ));
    assert!(contains_signature(
        surface,
        "impliedVol(expiry: number, strike: number, forward: number): number;"
    ));

    let ctor = interface_block(&dts, "FxDeltaVolSurfaceConstructor");
    for input in ["expiries", "atmVols", "rr25d", "bf25d"] {
        assert!(
            contains_ignoring_ws(ctor, &format!("{input}: NumericArray")),
            "FxDeltaVolSurface constructor must accept NumericArray for {input}"
        );
    }
    for input in ["rr10d", "bf10d"] {
        assert!(
            contains_ignoring_ws(ctor, &format!("{input}?: NumericArray")),
            "FxDeltaVolSurface optional constructor input must accept NumericArray for {input}"
        );
    }
    assert!(contains_signature(
        ctor,
        "deltaToStrike(delta: number, forward: number, vol: number, expiry: number): number;"
    ));
    assert!(contains_signature(
        ctor,
        "strikeToDelta(strike: number, forward: number, vol: number, expiry: number): number;"
    ));

    // Registered on the core namespace.
    let core_ns = interface_block(&dts, "CoreNamespace");
    assert!(contains_signature(
        core_ns,
        "FxDeltaVolSurface: FxDeltaVolSurfaceConstructor;"
    ));

    // FxRateResult exposes getter-style properties matching Python, and no
    // invented binding-side policy state.
    let fx_result = interface_block(&dts, "FxRateResult");
    assert!(contains_signature(fx_result, "readonly rate: number;"));
    assert!(contains_signature(
        fx_result,
        "readonly triangulated: boolean;"
    ));
    assert!(!fx_result.contains("getPolicy"));
    assert!(!fx_result.contains("getRate"));

    // Money exposes the lossless decimal-string accessor.
    let money = interface_block(&dts, "Money ");
    assert!(contains_signature(money, "amountDecimal(): string;"));
    assert!(contains_signature(
        money,
        "convertAtRate(target: Currency, rate: number): Money;"
    ));

    // DayCountContext exposes the coupon-period builder.
    let ctx = interface_block(&dts, "DayCountContext ");
    assert!(contains_signature(
        ctx,
        "withCouponPeriod(startEpochDays: number, endEpochDays: number): DayCountContext;"
    ));
    assert!(contains_signature(
        ctx,
        "withEndIsTerminationDate(value: boolean): DayCountContext;"
    ));

    let fx = interface_block(&dts, "FxMatrix ");
    assert!(contains_signature(
        fx,
        "setQuoteOn(base: string, quote: string, date: string, policy: FxConversionPolicy, rate: number): void;"
    ));
    assert!(contains_signature(
        fx,
        "rate(base: string, quote: string, date: string, policy: FxConversionPolicy): FxRateResult;"
    ));
    assert!(contains_signature(
        fx,
        "rateDefault(base: string, quote: string, date: string): FxRateResult;"
    ));

    let day_count = interface_block(&dts, "DayCount ");
    assert!(contains_signature(
        day_count,
        "signedYearFraction(startEpochDays: number, endEpochDays: number): number;"
    ));
    let day_count_constructor = interface_block(&dts, "DayCountConstructor ");
    assert!(contains_signature(
        day_count_constructor,
        "act365l(): DayCount;"
    ));
}

#[test]
fn core_date_array_outputs_are_exact_typed_arrays() {
    let dts = index_dts();
    let core = interface_block(&dts, "CoreNamespace");
    assert!(contains_signature(
        core,
        "dateFromEpochDays(days: number): Int32Array;"
    ));
}

#[test]
fn attribution_dts_matches_json_pipeline_surface() {
    // The attribution namespace previously had zero dts assertions.
    let dts = index_dts();

    assert!(dts.contains("export interface AttributionNamespace"));
    assert!(dts.contains("attributePnl(params: AttributionParams): string;"));
    assert!(dts.contains("AttributionParams: new ("));
    assert!(dts.contains("attributePnlFromSpec(specJson: string): string;"));
    assert!(dts.contains("validateAttributionJson(json: string): string;"));
    assert!(dts.contains("defaultWaterfallOrder(): string[];"));
    assert!(dts.contains("defaultAttributionMetrics(): string[];"));
    assert!(dts.contains("export declare const attribution: AttributionNamespace;"));
}
