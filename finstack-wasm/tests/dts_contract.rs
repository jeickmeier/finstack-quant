use std::fs;
use std::path::PathBuf;

fn index_dts() -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest_dir.join("index.d.ts")).expect("read finstack-wasm/index.d.ts")
}

fn benchmark_script() -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest_dir.join("benchmarks/bench.mjs"))
        .expect("read finstack-wasm/benchmarks/bench.mjs")
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
        "rollingGreeks(tickerIdx: number, window?: number): RollingGreeksResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "lookbackReturns(refDate: string, fiscalYearStartMonth?: number): LookbackReturns;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "rollingReturns(tickerIdx: number, window: number): RollingReturns;",
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
fn cashflows_dts_matches_json_bridge_surface() {
    let dts = index_dts();

    assert!(dts.contains("export interface CashflowsNamespace"));
    assert!(dts
        .contains("buildCashflowSchedule(specJson: string, marketJson?: string | null): string;"));
    assert!(dts.contains("validateCashflowSchedule(scheduleJson: string): string;"));
    assert!(dts.contains("datedFlows(scheduleJson: string): string;"));
    assert!(dts.contains("accruedInterest("));
    assert!(dts.contains("bondFromCashflows("));
    assert!(dts.contains("export declare const cashflows: CashflowsNamespace;"));
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
    assert!(dts.contains("FxBarrierOption: FxInstrumentConstructor<FxOptionInstrument>;"));
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

    assert!(dts.contains("export declare class WasmMarket {"));
    assert!(contains_ignoring_ws(
        &dts,
        "priceInstrumentWithMarket(instrumentJson: string, market: WasmMarket, asOf: string, model: string): string;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "priceInstrumentWithMetricsAndMarket(instrumentJson: string, market: WasmMarket, asOf: string, model: string, metrics: string[], pricingOptions?: string | null, marketHistory?: string | null): string;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "instrumentCashflowsWithMarket(instrumentJson: string, market: WasmMarket, asOf: string, model: string): string;",
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
}

#[test]
fn dts_documents_wasm_owned_handles_and_free_contract() {
    let dts = index_dts();

    assert!(
        dts.contains("WASM ownership: classes with a `free(): void` method own wasm heap memory.")
    );

    for class_name in [
        "Performance",
        "CreditFactorModel",
        "CreditCalibrator",
        "LevelsAtDate",
        "PeriodDecomposition",
        "FactorCovarianceForecast",
        "Portfolio",
    ] {
        let class_start = dts
            .find(&format!("export declare class {class_name} {{"))
            .unwrap_or_else(|| panic!("{class_name} class declaration missing"));
        let class_body = &dts[class_start..];
        let free_pos = class_body
            .find("free(): void;")
            .unwrap_or_else(|| panic!("{class_name}.free() declaration missing"));
        let before_free = &class_body[..free_pos];

        assert!(
            before_free.contains("Release the underlying wasm heap allocation"),
            "{class_name}.free() must document disposal semantics"
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
    assert!(dts.contains("runChecks(modelJson: string, suiteSpecJson: string): string;"));
    assert!(
        dts.contains("runThreeStatementChecks(modelJson: string, mappingJson: string): string;")
    );
    assert!(dts
        .contains("runCreditUnderwritingChecks(modelJson: string, mappingJson: string): string;"));
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
