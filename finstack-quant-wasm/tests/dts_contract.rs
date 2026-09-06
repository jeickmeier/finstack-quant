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

fn preceding_jsdoc<'a>(dts: &'a str, declaration: &str) -> &'a str {
    let prefix = dts
        .split_once(declaration)
        .map(|(prefix, _)| prefix.trim_end())
        .unwrap_or_else(|| panic!("declaration missing: {declaration}"));
    assert!(
        prefix.ends_with("*/"),
        "JSDoc is not immediately before: {declaration}"
    );
    let doc_start = prefix
        .rfind("/**")
        .unwrap_or_else(|| panic!("JSDoc start missing before: {declaration}"));
    &prefix[doc_start..]
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
    assert!(dts.contains("export interface FactorCovarianceMatrix"));
    assert!(dts.contains("export interface FactorModelConfig"));
    assert!(contains_signature(
        &dts,
        "constructor(model: CreditFactorModel);"
    ));
    assert!(contains_signature(
        &dts,
        "covarianceAt(horizonJson: string): FactorCovarianceMatrix;"
    ));
    assert!(contains_signature(
        &dts,
        "idiosyncraticVol(issuerId: string, horizonJson: string): number;"
    ));
    assert!(contains_signature(
        &dts,
        "factorModelAt(horizonJson: string, riskMeasureJson: string): FactorModelConfig;"
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
    let numeric_matrix_docs = preceding_jsdoc(&dts, "export type NumericMatrix = NumericArray[];");
    let constructor_docs = preceding_jsdoc(
        &dts,
        "  constructor(
    dates: string[],
    prices: NumericMatrix,",
    );
    let from_returns_docs = preceding_jsdoc(
        &dts,
        "  static fromReturns(
    dates: string[],
    returns: NumericMatrix,",
    );
    let ticker_names_docs = preceding_jsdoc(&dts, "  tickerNames(): string[];");
    let drawdown_duration_docs = preceding_jsdoc(&dts, "  maxDrawdownDuration(): number[];");
    let periodic_returns_docs = preceding_jsdoc(
        &dts,
        "  periodicReturns(frequency?: string): PeriodicReturnPoint[][];",
    );

    assert!(dts.contains("export declare class Performance {"));
    assert!(dts.contains("Performance: typeof Performance;"));
    assert!(contains_ignoring_ws(
        &dts,
        "static fromReturns(dates: string[], returns: NumericMatrix, tickerNames: string[], benchmarkTicker?: string | null, frequency?: string): Performance;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "rollingGreeks(tickerIdx: number, window?: number, riskFreeRate?: number): RollingGreeksResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "activeDatesForTicker(tickerIdx: number): string[];",
    ));
    assert!(dts.contains("export interface PeriodicReturnPoint {"));
    assert!(contains_ignoring_ws(
        &dts,
        "periodicReturns(frequency?: string): PeriodicReturnPoint[][];",
    ));
    assert!(periodic_returns_docs.contains("The outer array is ticker-major"));
    for token in [
        "daily",
        "weekly",
        "monthly",
        "quarterly",
        "semi_annual",
        "annual",
    ] {
        assert!(periodic_returns_docs.contains(token));
    }
    assert!(periodic_returns_docs.contains("decimal `value` fields"));
    assert!(periodic_returns_docs.contains("@throws Error"));
    assert!(periodic_returns_docs.contains("@example"));
    assert!(dts.contains("export type NumericMatrix = NumericArray[];"));
    assert!(contains_signature(
        &dts,
        "constructor(dates: string[], prices: NumericMatrix, tickerNames: string[], benchmarkTicker?: string | null, frequency?: string);",
    ));
    assert!(
        numeric_matrix_docs.contains(
            "Nested numeric arrays represented as an outer collection of numeric vectors."
        ),
        "NumericMatrix docs must describe a neutral outer vector collection"
    );
    assert!(numeric_matrix_docs.contains(
        "Each API specifies the semantic meaning and required length of the outer and inner dimensions."
    ));
    assert!(!numeric_matrix_docs.contains("one inner array per row"));
    assert!(constructor_docs.contains(
        "@param prices - Ticker-major, column-oriented matrix where `prices[tickerIdx][dateIdx]` is the price for `tickerIdx` at `dates[dateIdx]`."
    ));
    assert!(!constructor_docs.contains("Row-major"));
    assert!(!constructor_docs.contains("one row per"));
    assert!(from_returns_docs.contains(
        "@param returns - Ticker-major, column-oriented simple decimal return matrix where `returns[tickerIdx][dateIdx]` is the return for `tickerIdx` at `dates[dateIdx]`."
    ));
    assert!(!from_returns_docs.contains("Row-major"));
    assert!(!from_returns_docs.contains("one row per"));
    assert!(contains_signature(&dts, "tickerNames(): string[];"));
    assert!(ticker_names_docs.contains("Ticker names in column order."));
    assert!(
        ticker_names_docs.contains("Ticker labels in column order as a JavaScript string array.")
    );
    assert!(ticker_names_docs
        .contains("Rejects if the ticker-name vector cannot be serialized to JavaScript."));
    assert!(contains_signature(&dts, "maxDrawdownDuration(): number[];"));
    assert!(
        drawdown_duration_docs.contains("Longest drawdown duration in calendar days per asset.")
    );
    assert!(drawdown_duration_docs.contains(
        "Per-ticker longest drawdown duration in calendar days, as a JavaScript number array."
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "lookbackReturns(refDate: string, fiscalYearStartMonth?: number, fiscalYearStartDay?: number): LookbackReturns;",
    ));
    assert!(contains_ignoring_ws(&dts, "fytd: number[];"));
    assert!(!dts.contains("fytd: number[] | null;"));
    assert!(contains_ignoring_ws(
        &dts,
        "rollingReturns(tickerIdx: number, window: number): DatedSeries;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "cagr(dayCount?: string, calendarId?: string): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "parametricVar(confidence?: number, horizonPeriods?: number): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "cornishFisherVar(confidence?: number, horizonPeriods?: number): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "multiFactorGreeks(tickerIdx: number, factorReturns: NumericMatrix, returnKind?: string, riskFreeRate?: number): MultiFactorResult;",
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
fn periodic_returns_has_one_exact_frequency_param_description() {
    let dts = index_dts();
    let docs = preceding_jsdoc(
        &dts,
        "  periodicReturns(frequency?: string): PeriodicReturnPoint[][];",
    );

    assert_eq!(docs.matches("@param frequency").count(), 1);
    assert!(docs.contains(
        "@param frequency - Optional calendar frequency token: `\"daily\"`, `\"weekly\"`, `\"monthly\"`, `\"quarterly\"`, `\"semi_annual\"`, or `\"annual\"` (pandas offset aliases `D`/`B`, `W`, `M`, `Q`, `A`/`Y` are accepted too); defaults to `\"monthly\"`."
    ));
}

#[test]
fn core_dts_exposes_typed_array_math() {
    let dts = index_dts();

    // One entry point per Rust function: flat row-major matrices and
    // typed-array-or-number[] inputs. The former `*Array` / `*Flat` twins and
    // their `number[]`-only counterparts are gone.
    assert!(contains_ignoring_ws(
        &dts,
        "choleskyDecomposition(matrix: NumericArray, n: number): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "choleskySolve(chol: NumericArray, b: NumericArray, n: number): Float64Array;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "mean(data: NumericArray): number;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "correlation(x: NumericArray, y: NumericArray): number;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "kahanSum(values: NumericArray): number;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "longestPositiveRun(values: NumericArray): number;",
    ));
    for gone in [
        "meanArray(",
        "kahanSumArray(",
        "countConsecutive(",
        "choleskyDecompositionFlat(",
        "validateCorrelationMatrixFlat(",
    ] {
        assert!(!dts.contains(gone), "{gone} should no longer be exported");
    }
}

#[test]
fn forward_curve_dts_exposes_projection_grid_and_rate_between() {
    let dts = index_dts();
    let curve = interface_block(&dts, "ForwardCurve");
    let constructor = interface_block(&dts, "ForwardCurveConstructor");
    let options = interface_block(&dts, "ForwardCurveOptions");

    assert!(contains_ignoring_ws(
        curve,
        "readonly projectionGrid: Float64Array | undefined;"
    ));
    assert!(contains_ignoring_ws(
        curve,
        "rateBetween(t1: number, t2: number): number;"
    ));
    assert!(contains_ignoring_ws(curve, "readonly resetLag: number;"));
    assert!(contains_ignoring_ws(
        options,
        "projectionGrid?: NumericArray"
    ));
    assert!(contains_ignoring_ws(options, "knots: NumericArray"));
    assert!(contains_ignoring_ws(options, "resetLag?: number | null"));
    assert!(contains_ignoring_ws(
        constructor,
        "new (options: ForwardCurveOptions): ForwardCurve;"
    ));
    assert!(!constructor.contains("fromOptions"));
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
    assert!(contains_signature(
        constructor,
        "flat(id: string, baseDate: string, continuousRate: number): DiscountCurve;"
    ));
    assert!(dts.contains(
        "export type DiscountCurveValidationMode = 'market_standard' | 'negative_rate_friendly';"
    ));
}

/// M2.21 — the correlation namespace's `Vec<f64>` returns cross the WASM
/// boundary as `Float64Array`, and the hand-written d.ts must say so.
#[test]
fn models_correlation_dts_uses_float64array_returns() {
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
    assert!(dts.contains("accruedInterest("));
    let cashflows_start = dts.find("export interface CashflowsNamespace").unwrap();
    let cashflows_end = dts[cashflows_start..]
        .find("export declare const cashflows")
        .unwrap()
        + cashflows_start;
    assert!(!dts[cashflows_start..cashflows_end].contains("bondFromCashflowsJson("));
    assert!(dts.contains("export interface ValuationInstrumentsNamespace"));
    assert!(dts.contains("export interface ValuationMarketNamespace"));
    assert!(dts.contains("bondFromCashflowsJson("));
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
    // Every FX class carries an `id` getter, mirroring the Python typed
    // wrappers' `id` property.
    let fx_instrument = interface_block(&dts, "FxInstrument");
    assert!(fx_instrument.contains("readonly id: string;"));
    assert!(contains_ignoring_ws(
        fx_instrument,
        "price(marketJson: string, asOf: string, model?: string | null, metrics?: string[] | null, pricingOptions?: string | null, marketHistory?: string | null): ValuationResult;",
    ));
}

#[test]
fn valuations_dts_exposes_reusable_market_handle_pricing() {
    let dts = index_dts();

    assert!(dts.contains("export declare class Market {"));
    assert!(contains_ignoring_ws(
        &dts,
        "priceInstrumentWithMarket(instrumentJson: string, market: Market, asOf: string, model: string, metrics?: string[] | null, pricingOptions?: string | null, marketHistory?: string | null): ValuationResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "instrumentCashflowsWithMarketJson(instrumentJson: string, market: Market, asOf: string, model: string): string;",
    ));
}

#[test]
fn calibration_dts_owns_calibration_surface() {
    let dts = index_dts();
    let calibration = interface_block(&dts, "CalibrationNamespace");
    let valuations = interface_block(&dts, "ValuationsNamespace");

    for name in [
        "calibrate(",
        "validateCalibrationJson(",
        "dryRun(",
        "calibrateBermudanLmmBaseVol(",
    ] {
        assert!(
            calibration.contains(name),
            "calibration interface missing {name}"
        );
        assert!(
            !valuations.contains(name),
            "valuations interface still exposes {name}"
        );
    }
    assert!(dts.contains("export declare const calibration: CalibrationNamespace;"));
}

#[test]
fn pricing_entry_points_declare_structured_valuation_results() {
    // Pricing returns are computation results, not wire documents: the
    // bindings hand back a plain JS object (parity with Python's
    // `ValuationResult`), so no `priceInstrument*` may be typed as `string`.
    let dts = index_dts();

    assert!(dts.contains("export interface ValuationResult {"));
    assert!(contains_ignoring_ws(&dts, "value: MoneyValue;"));
    assert!(contains_ignoring_ws(
        &dts,
        "measures: Record<string, number>;"
    ));
    assert!(contains_ignoring_ws(&dts, "details?: ValuationDetails;"));
    let monte_carlo = interface_block(&dts, "MonteCarloValuationDetails");
    for field in [
        "model_key: string;",
        "standard_error: number;",
        "training_paths: number;",
        "training_simulated_paths: number;",
        "make_whole_training_paths: number;",
        "make_whole_training_simulated_paths: number;",
        "estimator_paths: number;",
        "simulated_paths: number;",
        "seed: bigint;",
        "time_grid: number[];",
        "antithetic: boolean;",
        "sobol: boolean;",
        "brownian_bridge: boolean;",
    ] {
        assert!(
            contains_ignoring_ws(monte_carlo, field),
            "MonteCarloValuationDetails is missing `{field}`"
        );
    }
    assert!(dts.contains("type: 'monte_carlo'; data: MonteCarloValuationDetails"));
    assert!(contains_ignoring_ws(
        &dts,
        "priceInstrument(instrumentJson: string, marketJson: string, asOf: string, model?: string | null, metrics?: string[] | null, pricingOptions?: string | null, marketHistory?: string | null): ValuationResult;",
    ));
    let pricing_doc = preceding_jsdoc(&dts, "  priceInstrument(");
    for model in ["discounting", "hazard_rate", "tree", "rates_credit"] {
        assert!(
            pricing_doc.contains(model),
            "priceInstrument JSDoc omits explicit bond model `{model}`"
        );
    }
    assert!(pricing_doc.contains("Stochastic `\"rates_credit\"` runs"));
    // The valuation-result *validator* still takes and returns a wire string.
    assert!(contains_ignoring_ws(
        &dts,
        "validateValuationResultJson(json: string): string;",
    ));
}

#[test]
fn structured_credit_tranche_analytics_declare_typed_results() {
    // OAS / metrics / scenario-table are computation results: they return
    // typed plain objects matching the Python `OasResult` / `TrancheMetrics`
    // / `ScenarioTable` wrappers' snake_case shape, not JSON strings. The
    // scalar entry points (discount margin, break-even CDR) stay numbers.
    let dts = index_dts();

    assert!(dts.contains("export interface OasResult {"));
    assert!(dts.contains("export interface TrancheMetrics {"));
    assert!(dts.contains("export interface ScenarioTable {"));
    assert!(dts.contains("export interface TrancheScenarioCell {"));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheOas(instrumentJson: string, trancheId: string, marketPricePct: number, marketJson: string, asOf: string, config?: string | null): OasResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheMetrics(instrumentJson: string, trancheId: string, marketJson: string, asOf: string, marketPricePct?: number | null): TrancheMetrics;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheScenarioTable(instrumentJson: string, trancheId: string, marketJson: string, asOf: string, grid: string): ScenarioTable;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheDiscountMargin(instrumentJson: string, trancheId: string, marketJson: string, asOf: string, targetPv: number): number;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheBreakevenCdr(instrumentJson: string, trancheId: string, marketJson: string, asOf: string): number;",
    ));
}

#[test]
fn portfolio_cashflow_api_uses_full_cashflow_name_everywhere() {
    let dts = index_dts();
    let bench = benchmark_script();

    assert!(contains_signature(
        &dts,
        "aggregateFullCashflows(specJson: string, marketJson: string, allowPartial?: boolean): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "aggregateFullCashflowsBuilt(portfolio: Portfolio, marketJson: string, allowPartial?: boolean): Record<string, unknown>;",
    ));
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
    assert!(!dts.contains("export declare const factor_model:"));
    assert!(dts.contains("export declare const features: FeaturesNamespace;"));
    assert!(dts.contains("export declare const valuations: ValuationsNamespace;"));
    assert!(dts.contains("export declare const portfolio: PortfolioNamespace;"));
    assert!(dts.contains("generated `types/generated/*` files"));
}

#[test]
fn scenarios_dts_matches_structured_surface() {
    let dts = index_dts();

    assert!(dts.contains("export interface ScenariosNamespace"));
    assert!(dts.contains("export interface ScenarioSpec"));
    assert!(dts.contains("export interface TemplateMetadata"));
    assert!(dts.contains("export interface ScenarioWarning"));
    assert!(contains_ignoring_ws(&dts, "warnings: ScenarioWarning[];"));
    assert!(contains_ignoring_ws(
        &dts,
        "computeHorizonReturn(instrumentJson: string, marketJson: string, asOf: string, scenarioJson: string, method?: string, configJson?: string, calendarId?: string): Record<string, unknown>;",
    ));
    // `priority` mirrors the Rust serde default (0) and the Python keyword
    // default, so it must stay optional.
    assert!(contains_ignoring_ws(
        &dts,
        "buildScenarioSpec(id: string, operations: ScenarioOperation[], name?: string, description?: string, priority?: number, resolutionMode?: 'most_specific_wins' | 'cumulative', hazardBumpMode?: 'solve_to_par' | 'first_order_shift'): ScenarioSpec;",
    ));
    assert!(dts.contains("export declare const scenarios: ScenariosNamespace;"));
}

/// The `Portfolio` handle getter is `baseCurrency` (full-word camelCase,
/// matching Python `base_currency`); the historical `baseCcy` spelling was
/// intentionally removed and must not resurface in the declarations.
#[test]
fn portfolio_dts_uses_full_word_base_currency() {
    let dts = index_dts();

    assert!(contains_ignoring_ws(&dts, "readonly baseCurrency: string;"));
    assert!(!dts.contains("baseCcy"));
}

/// Python-parity optional parameters on the portfolio risk entry points:
/// `strictRisk` (default `true`) and `metrics` on the valuation pair,
/// `computeIncremental` on the parametric VaR decomposition, and the
/// Rust-defaulted `utilizationThreshold`.
#[test]
fn portfolio_dts_pins_python_parity_optional_parameters() {
    let dts = index_dts();

    assert!(contains_ignoring_ws(
        &dts,
        "valuePortfolio(specJson: string, marketJson: string, strictRisk?: boolean, metrics?: string[]): Record<string, unknown>;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "valuePortfolioBuilt(portfolio: Portfolio, marketJson: string, strictRisk?: boolean, metrics?: string[]): Record<string, unknown>;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "parametricVarDecomposition(positionIdsJson: string, weightsJson: string, covarianceJson: string, confidence: number, computeIncremental?: boolean): VarDecompositionResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateRiskBudget(positionIdsJson: string, actualVarJson: string, targetVarPctJson: string, portfolioVar: number, utilizationThreshold?: number): RiskBudgetResult;",
    ));
    assert!(contains_ignoring_ws(&dts, "execution_risk: number;"));
}

/// `index.d.ts` is hand-maintained, so nothing else stops a declaration from
/// drifting to the wrong argument count. A 2-argument `campisiCarinoLink`
/// declaration would compile clean for a TypeScript caller writing
/// `campisiCarinoLink(periods, config)` while the second argument is silently
/// discarded at the JS boundary — the exact "shared config" mistake the
/// results-based entry point exists to avoid. Pin the declared signatures
/// here; `tests/facade/portfolio.test.mjs` pins the runtime `Function.length`
/// of the real exports against the same arities.
#[test]
fn campisi_dts_declarations_pin_their_argument_lists() {
    let dts = index_dts();

    assert!(
        dts.contains("quote-reproducing `z_spread` basis"),
        "Campisi TypeScript docs must state the required spread-risk basis"
    );
    assert!(
        dts.contains("OAS, G-spread") && dts.contains("discount-margin values are incompatible"),
        "Campisi TypeScript docs must name the rejected mismatched bases"
    );
    assert!(contains_signature(
        &dts,
        "campisiAttribution(portfolioJson: string, benchmarkJson: string, configJson: string): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "campisiCarinoLink(periodsJson: string): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "campisiCarinoLinkFromSnapshots(periodsJson: string, configJson: string): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "campisiReconciliationCheck(resultJson: string, tolerance: number): Record<string, unknown>;",
    ));

    // The results-based linker must not grow a config argument: it links
    // periods that already carry their own `period_years`.
    assert!(!contains_ignoring_ws(
        &dts,
        "campisiCarinoLink(periodsJson: string, configJson: string)",
    ));
}

#[test]
fn models_liquidity_dts_exposes_reference_price_for_almgren_chriss() {
    let dts = index_dts();
    let liquidity = interface_block(&dts, "LiquidityNamespace");
    let impact = interface_block(&dts, "ImpactEstimate");

    assert!(liquidity.contains("referencePrice?: number | null"));
    assert!(liquidity.contains("): ImpactEstimate;"));
    for field in [
        "permanent_impact: number;",
        "temporary_impact: number;",
        "total_cost: number;",
        "cost_bp: number;",
        "execution_risk: number;",
    ] {
        assert!(
            impact.contains(field),
            "missing ImpactEstimate field {field}"
        );
    }
    assert!(!dts.contains("AlmgrenChrissImpactResult"));
    assert!(!dts.contains("total_impact: number;"));
    assert!(!dts.contains("expected_cost_bp: number;"));
    assert!(!interface_block(&dts, "PortfolioNamespace").contains("almgrenChrissImpact("));
}

#[test]
fn models_liquidity_dts_requires_reference_price_for_kyle_lambda() {
    let dts = index_dts();
    let liquidity = interface_block(&dts, "LiquidityNamespace");

    assert!(contains_signature(
        liquidity,
        "kyleLambda(returnsJson: string, volumesJson: string, referencePrice: number): number | undefined;",
    ));
    assert!(!interface_block(&dts, "PortfolioNamespace").contains("kyleLambda("));
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
    assert!(contains_signature(day_count_ctor, "actActAfb(): DayCount;"));
    assert!(contains_signature(
        day_count_ctor,
        "thirty360It(): DayCount;"
    ));
    let day_count = interface_block(&dts, "DayCount ");
    assert!(contains_signature(
        day_count,
        "calendarDays(startEpochDays: number, endEpochDays: number): bigint;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "Act/Act ISMA and Bus/252 require explicit frequency/calendar context"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "This method throws for those conventions"
    ));
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
        "HazardCurve ",
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
    assert!(dts.contains("validateCheckSuiteSpecJson(json: string): string;"));
    // Computation results are structured objects, not JSON strings: a string
    // return here would put JS out of step with the typed Python result.
    assert!(dts.contains("evaluateModel(modelJson: string): StatementResultJson;"));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateModelWithMarket(modelJson: string, marketJson: string, asOf: string): StatementResultJson;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runMonteCarlo(modelJson: string, configJson: string): Record<string, unknown>;",
    ));
    assert!(dts.contains("export interface StatementResultJson"));
    assert!(dts.contains("export declare const statements: StatementsNamespace;"));
}

/// Covenant evaluation returns typed reports; the JSON bridge helpers keep
/// their `Json`-suffixed names and string returns.
#[test]
fn covenants_dts_separates_typed_results_from_json_bridges() {
    let dts = index_dts();

    assert!(dts.contains("export interface CovenantsNamespace"));
    assert!(dts.contains("export interface CovenantReport"));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateEngine(engineJson: string, metricsJson: string, asOf: string): Record<string, CovenantReport>;",
    ));
    for wire in [
        "validateCovenantSpecJson(specJson: string): string;",
        "validateCovenantReportJson(reportJson: string): string;",
        "validateCovenantEngineJson(engineJson: string): string;",
        "covLiteJson(maxLeverage: number, maxSeniorLeverage: number): string;",
        "realEstateJson(minDscr: number, minDebtYield: number, maxLtv: number): string;",
    ] {
        assert!(contains_ignoring_ws(&dts, wire), "missing: {wire}");
    }
    assert!(dts.contains("export declare const covenants: CovenantsNamespace;"));
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
    // Converted computation results: object returns, matching the typed
    // Python results for the same Rust calls.
    assert!(contains_ignoring_ws(
        &dts,
        "runSensitivity(modelJson: string, configJson: string): Record<string, unknown>;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runVariance(baseJson: string, comparisonJson: string, configJson: string): Record<string, unknown>;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateScenarioSet(modelJson: string, scenarioSetJson: string): Record<string, StatementResultJson>;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "creditAssessment(resultsJson: string, period: string): Record<string, unknown>;",
    ));
    assert!(dts.contains("export interface DcfSensitivityResult"));
    // dcfSensitivity carries the same optional mid-year-convention and market
    // parameters as the Python twin (contracted 1:1 in parity_contract.toml).
    assert!(contains_ignoring_ws(
        &dts,
        "dcfSensitivity(modelJson: string, wacc: number, terminalValueJson: string, ufcfNode: string, netDebtOverride?: number | null, waccSensitivityBump?: number | null, waccDenominatorEpsilon?: number | null, maxStableGrowthRate?: number | null, exitMultipleBump?: number | null, midYearConvention?: boolean | null, marketJson?: string | null): DcfSensitivityResult;",
    ));
    assert!(dts.contains("export interface LboResult"));
    assert!(dts.contains("export interface TornadoEntry"));
    assert!(contains_ignoring_ws(
        &dts,
        "generateTornadoEntries(resultJson: string, metricNode: string, period?: string): TornadoEntry[];"
    ));
    assert!(dts.contains("export interface CheckReport"));
    assert!(dts.contains("export interface CheckResult"));
    assert!(dts.contains("export interface CheckFinding"));
    assert!(dts.contains("export interface CheckSummary"));
    assert!(contains_ignoring_ws(
        &dts,
        "runChecks(modelJson: string, suiteSpecJson: string, resultsJson?: string | null): CheckReport;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runThreeStatementChecks(modelJson: string, mappingJson: string, resultsJson?: string | null): CheckReport;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runCreditUnderwritingChecks(modelJson: string, mappingJson: string, resultsJson?: string | null): CheckReport;"
    ));
    assert!(dts.contains("renderCheckReportText(reportJson: string): string;"));
    assert!(dts.contains("renderCheckReportHtml(reportJson: string): string;"));
    assert!(
        dts.contains("export declare const statements_analytics: StatementsAnalyticsNamespace;")
    );
}

#[test]
fn models_and_valuations_dts_expose_owned_credit_namespaces() {
    let dts = index_dts();
    let core = interface_block(&dts, "CoreNamespace");
    let model_credit = interface_block(&dts, "ModelCreditNamespace");
    let models = interface_block(&dts, "ModelsNamespace");
    let valuations = interface_block(&dts, "ValuationsNamespace");

    assert!(dts.contains("export interface ModelCreditNamespace"));
    assert!(dts.contains("mertonModelJson("));
    assert!(dts.contains("mertonDefaultProbabilityWithDrift("));
    assert!(dts.contains("mertonDistanceToDefaultWithDrift("));
    assert!(
        dts.contains("mertonKmvDefaultPoint(shortTermDebt: number, longTermDebt: number): number;")
    );
    assert!(dts.contains("mertonDebtSpread(modelJson: string, horizon: number): number;"));
    assert!(dts.contains(
        "mertonCdsParSpread(modelJson: string, maturity: number, recovery: number): number;"
    ));
    assert!(dts.contains("creditGradesModelJson("));
    assert!(dts.contains("toggleExerciseOptimalJson("));
    assert!(model_credit.contains("analyzeExchangeOffer("));
    assert!(model_credit.contains("analyzeLme("));
    assert!(!core.contains("analyzeExchangeOffer("));
    assert!(!core.contains("analyzeLme("));
    assert!(dts.contains("export interface CreditDerivativesNamespace"));
    assert!(dts.contains("creditDefaultSwapExampleJson(): string;"));
    assert!(dts.contains("cdsOptionExampleJson(): string;"));
    assert!(models.contains("credit: ModelCreditNamespace;"));
    assert!(models.contains("correlation: CorrelationNamespace;"));
    assert!(dts.contains("creditDerivatives: CreditDerivativesNamespace;"));
    assert!(!valuations.contains("credit: ModelCreditNamespace;"));
    assert!(!valuations.contains("correlation: CorrelationNamespace;"));
    assert!(!valuations.contains("CreditFactorModel"));
    assert!(!valuations.contains("CreditCalibrator"));
    assert!(!valuations.contains("decomposeLevels"));
}

#[test]
fn models_factor_dts_exposes_credit_namespace() {
    let dts = index_dts();
    let factor = interface_block(&dts, "FactorNamespace");
    let models = interface_block(&dts, "ModelsNamespace");

    assert!(dts.contains("export interface FactorNamespace"));
    assert!(dts.contains("export interface FactorModelCreditNamespace"));
    assert!(dts.contains("credit: FactorModelCreditNamespace;"));
    assert!(models.contains("factor: FactorNamespace;"));
    assert!(!factor.contains("CreditFactorModel"));
    assert!(!factor.contains("decomposeLevels"));
    assert!(!dts.contains("export declare const factor_model:"));
}

#[test]
fn models_monte_carlo_dts_matches_pricing_surface() {
    let dts = index_dts();
    let monte_carlo = interface_block(&dts, "MonteCarloNamespace");

    assert!(dts.contains("export interface MonteCarloNamespace"));
    // The facade exports pinned under [wasm_models_subset]; the closed-form
    // Black-Scholes references live at `models.bsPrice`, not here.
    assert!(!monte_carlo.contains("blackScholesCall("));
    assert!(!monte_carlo.contains("blackScholesPut("));
    for name in ["priceHestonCall(", "priceHestonPut("] {
        assert!(
            monte_carlo.contains(name),
            "MonteCarloNamespace is missing `{name}`"
        );
    }
    let models = interface_block(&dts, "ModelsNamespace");
    assert!(models.contains("monteCarlo: MonteCarloNamespace;"));
    assert!(dts.contains("export declare const models: ModelsNamespace;"));
    assert!(!dts.contains("export declare const monte_carlo: MonteCarloNamespace;"));
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
        "rankToWeights(values: FeatureValue[], timeKey: string[], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "neutralizeAndZscore(values: FeatureValue[], timeKey: string[], exposures: FeatureValue[][], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "transformPanelJson(specJson: string): string;"
    ));
    assert!(dts.contains("export declare const features: FeaturesNamespace;"));
}

#[test]
fn core_market_data_dts_exposes_data_only_vol_cube() {
    let dts = index_dts();

    let cube = interface_block(&dts, "VolCube ");
    assert!(contains_signature(cube, "readonly id: string;"));
    assert!(contains_signature(
        cube,
        "readonly interpolationMode: string;"
    ));
    for removed_method in ["vol(", "volClamped(", "volNormal(", "volNormalClamped("] {
        assert!(
            !cube.contains(removed_method),
            "core VolCube must remain a data artifact without {removed_method}"
        );
    }
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
fn core_market_data_dts_exposes_data_only_fx_surface_and_rate_result() {
    let dts = index_dts();

    // FxDeltaVolSurface instance + constructor interfaces.
    let surface = interface_block(&dts, "FxDeltaVolSurface ");
    assert!(contains_signature(surface, "readonly id: string;"));
    assert!(contains_signature(
        surface,
        "readonly expiries: Float64Array;"
    ));
    assert!(contains_signature(surface, "readonly numExpiries: number;"));
    for removed_method in ["pillarVols(", "impliedVol("] {
        assert!(
            !surface.contains(removed_method),
            "core FxDeltaVolSurface must remain a data artifact without {removed_method}"
        );
    }

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
    assert!(!ctor.contains("deltaToStrike("));
    assert!(!ctor.contains("strikeToDelta("));

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
    assert!(contains_signature(
        day_count_constructor,
        "actActAfb(): DayCount;"
    ));
    assert!(contains_signature(
        day_count_constructor,
        "thirty360It(): DayCount;"
    ));

    assert!(dts.contains("export interface FxQuoteConvention"));
    assert!(dts.contains("export interface FxPairConvention"));
    let quote_ctor = interface_block(&dts, "FxQuoteConventionConstructor");
    assert!(contains_signature(
        quote_ctor,
        "direct(): FxQuoteConvention;"
    ));
    assert!(contains_signature(
        quote_ctor,
        "indirect(): FxQuoteConvention;"
    ));
    assert!(contains_signature(
        quote_ctor,
        "fromName(name: string): FxQuoteConvention;"
    ));
    let pair = interface_block(&dts, "FxPairConvention ");
    assert!(contains_signature(pair, "readonly base: Currency;"));
    assert!(contains_signature(pair, "readonly quote: Currency;"));
    assert!(contains_signature(
        pair,
        "readonly usdQuotation: FxQuoteConvention;"
    ));
    assert!(contains_signature(pair, "readonly pipSize: number;"));
    assert!(contains_signature(pair, "readonly spotLagDays: number;"));
    assert!(contains_signature(
        core_ns,
        "FxQuoteConvention: FxQuoteConventionConstructor;"
    ));
    assert!(contains_signature(
        core_ns,
        "FxPairConvention: FxPairConventionConstructor;"
    ));
    assert!(contains_signature(
        core_ns,
        "fxMarketPair(a: string, b: string): Currency[];"
    ));
    assert!(contains_signature(
        core_ns,
        "fxPairConvention(base: string, quote: string): FxPairConvention;"
    ));
    assert!(contains_signature(
        core_ns,
        "fxPipSize(base: string, quote: string): number;"
    ));
    assert!(contains_signature(
        core_ns,
        "invertFxRate(rate: number): number;"
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
    let dts = index_dts();

    assert!(dts.contains("export interface AttributionNamespace"));
    assert!(dts.contains("export interface PnlAttribution"));
    assert!(dts.contains("attributePnl(params: AttributionParams): PnlAttribution;"));
    assert!(dts.contains("attributePnlJson(params: AttributionParams): string;"));
    assert!(dts.contains("AttributionParams: new ("));
    assert!(dts.contains("attributePnlEnvelopeJson(specJson: string): string;"));
    assert!(dts.contains("validateAttributionJson(json: string): string;"));
    assert!(dts.contains("defaultWaterfallOrder(): string[];"));
    assert!(dts.contains("defaultAttributionMetrics(): string[];"));
    assert!(dts.contains("export declare const attribution: AttributionNamespace;"));
}

/// Mirrors `campisi_dts_declarations_pin_their_argument_lists` for the
/// credit excess-return / grid-attribution / factor-Brinson surfaces: the
/// `.d.ts` is hand-maintained and otherwise ungated, so a declaration with
/// the wrong argument count would compile clean for a TypeScript caller
/// while an extra or missing argument is silently mishandled at the JS
/// boundary. `tests/facade/portfolio.test.mjs` pins the runtime
/// `Function.length` of the real exports against the same arities.
#[test]
fn credit_excess_grid_factor_brinson_dts_declarations_pin_their_argument_lists() {
    let dts = index_dts();

    assert!(contains_signature(
        &dts,
        "cellReturnsFromReference(referenceJson: string, baseLabel: string, configJson: string): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "cellReturnsFromCurves(\
           start: DiscountCurve, \
           end: DiscountCurve, \
           horizonYears: number, \
           maxDuration: number, \
           baseLabel: string, \
           configJson: string\
         ): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "excessReturns(positionsJson: string, tableJson: string): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "gridAttribution(portfolioJson: string, benchmarkJson: string): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "gridCarinoLink(periodsJson: string): Record<string, unknown>;",
    ));
    assert!(contains_signature(
        &dts,
        "factorBrinsonAttribution(inputJson: string, factorReturns: NumericArray): Record<string, unknown>;",
    ));

    let analytics = interface_block(&dts, "AnalyticsNamespace");
    assert!(contains_signature(
        analytics,
        "constrainedLeastSquares(\
           exposures: NumericArray, \
           nFactors: number, \
           returns: NumericArray, \
           weights: NumericArray\
         ): Float64Array;",
    ));
}
