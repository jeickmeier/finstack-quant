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

/// Whether `name` is a declared type of the facade: a hand-written
/// `interface`/`type`, or a schema-generated type re-exported through an
/// `export type { ... }` list.
fn declares_type(dts: &str, name: &str) -> bool {
    if dts.contains(&format!("export interface {name} "))
        || dts.contains(&format!("export type {name} ="))
        || dts.contains(&format!("export type {name}<"))
    {
        return true;
    }
    dts.split("export type {").skip(1).any(|rest| {
        rest.split('}')
            .next()
            .is_some_and(|list| list.split(',').any(|item| item.trim() == name))
    })
}

/// The schema-generated TypeScript module of one crate.
fn generated_types(crate_module: &str) -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(
        manifest_dir.join(format!("types/generated/{crate_module}/{crate_module}.ts")),
    )
    .unwrap_or_else(|error| panic!("read generated {crate_module} types: {error}"))
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
    assert!(dts.contains("declare class CreditFactorModel {"));
    assert!(contains_signature(
        &dts,
        "static fromJson(json: JsonInput): CreditFactorModel;"
    ));
    assert!(contains_signature(&dts, "toJson(): string;"));

    assert!(dts.contains("declare class CreditCalibrator {"));
    // Python's `CreditCalibrator(config=None)` twin: the config defaults to
    // the Rust `CreditCalibrationConfig::default()` in both hosts.
    assert!(contains_signature(
        &dts,
        "constructor(configJson?: JsonInput | null);"
    ));
    assert!(contains_signature(
        &dts,
        "calibrate(inputsJson: JsonInput): CreditFactorModel;"
    ));

    assert!(dts.contains("declare class LevelsAtDate {"));
    assert!(dts.contains("declare class PeriodDecomposition {"));

    assert!(dts.contains("declare class FactorCovarianceForecast {"));
    assert!(declares_type(&dts, "FactorCovarianceMatrix"));
    assert!(declares_type(&dts, "FactorModelConfig"));
    assert!(contains_signature(
        &dts,
        "constructor(model: CreditFactorModel);"
    ));
    assert!(contains_signature(
        &dts,
        "covarianceAt(horizonJson: JsonInput): FactorCovarianceMatrix;"
    ));
    assert!(contains_signature(
        &dts,
        "idiosyncraticVol(issuerId: string, horizonJson: JsonInput): number;"
    ));
    assert!(contains_signature(
        &dts,
        "factorModelAt(horizonJson: JsonInput, riskMeasureJson?: JsonInput | null): FactorModelConfig;"
    ));

    // The decomposition functions live on `models.factor.credit`; the package
    // root exports no functions besides the `init` default.
    assert!(!dts.contains("export declare function decomposeLevels("));
    assert!(!dts.contains("export declare function decomposePeriod("));

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
        "  periodicReturns(frequency?: string): PeriodicReturn[][];",
    );

    assert!(dts.contains("declare class Performance {"));
    assert!(dts.contains("Performance: typeof Performance;"));
    assert!(contains_ignoring_ws(
        &dts,
        "static fromReturns(dates: string[], returns: NumericMatrix, tickerNames: string[], benchmarkTicker?: string | null, frequency?: string): Performance;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "rollingGreeks(tickerIdx: number, window?: number, riskFreeRate?: number): RollingGreeks;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "activeDatesForTicker(tickerIdx: number): string[];",
    ));
    assert!(declares_type(&dts, "PeriodicReturn"));
    assert!(contains_ignoring_ws(
        &dts,
        "periodicReturns(frequency?: string): PeriodicReturn[][];",
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
    assert!(declares_type(&dts, "LookbackReturns"));
    assert!(
        interface_block(&generated_types("analytics"), "LookbackReturns")
            .contains("fytd: number[];")
    );
    assert!(!dts.contains("fytd: number[] | null;"));
    assert!(contains_ignoring_ws(
        &dts,
        "rollingReturns(tickerIdx: number, window: number): DatedSeries;",
    ));
    // Rolling series keep the Rust `DatedSeries` field names.
    assert!(contains_ignoring_ws(&dts, "values: Float64Array;"));
    assert!(contains_ignoring_ws(
        &dts,
        "value_column: 'volatility' | 'sortino' | 'sharpe' | 'return';",
    ));
    assert!(!dts.contains("return?: Float64Array;"));
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
        "  periodicReturns(frequency?: string): PeriodicReturn[][];",
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
        "choleskySolve(chol: NumericArray, b: NumericArray): Float64Array;",
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
    // One named-options constructor (no positional optional arguments).
    assert!(contains_signature(
        constructor,
        "new (options: DiscountCurveOptions | string): DiscountCurve;"
    ));
    let options = interface_block(&dts, "DiscountCurveOptions");
    assert!(options.contains("validationMode?: DiscountCurveValidationMode"));
    assert!(options.contains("forwardFloor?: number | null"));
    assert!(contains_ignoring_ws(options, "knots: NumericArray"));
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
    // Tranche statistics hang off the `PortfolioLossResult` handle, as in
    // Rust and Python; there is no flat WASM-only composition.
    assert!(contains_ignoring_ws(
        &dts,
        "PortfolioLossResult: PortfolioLossResultConstructor;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "fromLosses(losses: NumericArray, confidence: number): PortfolioLossResult;",
    ));
    assert!(contains_ignoring_ws(
        interface_block(&dts, "PortfolioLossResult "),
        "trancheLossStatistics(attachment: number, detachment: number, poolNotional: number): TrancheLossStatistics;",
    ));
    assert!(!interface_block(&dts, "CorrelationNamespace").contains("trancheLossStatistics("));
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
        "buildCashflowScheduleJson(specJson: JsonInput, marketJson?: JsonInput | null): string;"
    ));
    assert!(dts.contains("validateCashflowScheduleJson(scheduleJson: JsonInput): string;"));
    assert!(!dts.contains("CashflowScheduleEnvelope"));
    assert!(!dts.contains("buildCashflowScheduleEnvelopeJson"));
    assert!(!dts.contains("validateCashflowScheduleEnvelopeJson"));
    assert!(dts.contains("datedFlowsJson(scheduleJson: JsonInput): string;"));
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
    assert!(dts.contains("FxForward: FxForwardConstructor;"));
    assert!(dts.contains("FxSwap: FxInstrumentConstructor<FxInstrument>;"));
    assert!(dts.contains("Ndf: FxInstrumentConstructor<FxInstrument>;"));
    assert!(dts.contains("FxOption: FxOptionConstructor;"));
    assert!(dts.contains("FxBarrierOption: FxInstrumentConstructor<FxBarrierOptionInstrument>;"));
    assert!(dts.contains("FxDigitalOption: FxInstrumentConstructor<FxDigitalOptionInstrument>;"));
    assert!(dts.contains("FxTouchOption: FxInstrumentConstructor<FxTouchOptionInstrument>;"));
    assert!(dts.contains("QuantoOption: FxInstrumentConstructor<FxOptionInstrument>;"));
    assert!(dts.contains("fx: FxNamespace;"));
    assert!(dts.contains(
        "foreignRho(marketJson: JsonInput, asOf: string, model?: string | null): number;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "greeks(marketJson: JsonInput, asOf: string, model?: string | null): Record<string, number>;",
    ));
    // Every FX class carries an `id` getter, mirroring the Python typed
    // wrappers' `id` property.
    // Touch (and, by inheritance, barrier) options expose theta: the Rust
    // metric registry computes it for both.
    let bond = interface_block(&dts, "Bond ");
    assert!(contains_signature(
        bond,
        "withStub(stub: 'none' | 'short_front' | 'short_back' | 'long_front' | 'long_back'): Bond;"
    ));
    let bond_ctor = interface_block(&dts, "BondConstructor");
    assert!(contains_ignoring_ws(bond_ctor, "withConvention(id: string, notional: Money, couponRate: Rate, issueDate: string, maturity: string, convention:"));
    let touch = interface_block(&dts, "FxTouchOptionInstrument");
    assert!(contains_signature(
        touch,
        "theta(marketJson: JsonInput, asOf: string, model?: string | null): number;"
    ));
    let fx_instrument = interface_block(&dts, "FxInstrument");
    assert!(fx_instrument.contains("readonly id: string;"));
    assert!(contains_ignoring_ws(
        fx_instrument,
        "price(marketJson: JsonInput, asOf: string, model?: string | null, metrics?: string[] | null, metricPricingOverrides?: JsonInput | null, marketHistory?: JsonInput | null): ValuationResult;",
    ));
}

#[test]
fn valuations_dts_exposes_reusable_market_handle_pricing() {
    let dts = index_dts();

    let market = interface_block(&dts, "MarketContextConstructor");
    assert!(contains_signature(
        market,
        "fromJson(json: JsonInput): MarketContext;"
    ));
    let core_ns = interface_block(&dts, "CoreNamespace");
    assert!(contains_signature(
        core_ns,
        "MarketContext: MarketContextConstructor;"
    ));
    assert!(!interface_block(&dts, "ValuationsNamespace").contains("\n  Market:"));
    assert!(contains_ignoring_ws(
        &dts,
        "priceInstrumentWithMarket(instrumentJson: JsonInput, market: MarketContext, asOf: string, model?: string | null, metrics?: string[] | null, metricPricingOverrides?: JsonInput | null, marketHistory?: JsonInput | null): ValuationResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "instrumentCashflowsWithMarketJson(instrumentJson: JsonInput, market: MarketContext, asOf: string, model: string): string;",
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
        "dryRunJson(",
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

    assert!(declares_type(&dts, "ValuationResult"));
    // Emitted-field presence is stricter than the host schema's input view.
    assert!(contains_ignoring_ws(
        &dts,
        "covenants: NonNullable<HostValuationResult['covenants']> | null;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "details?: NonNullable<HostValuationResult['details']>;"
    ));
    assert!(dts.contains("import('./types/valuation-result.js').MonteCarloValuationDetails"));
    assert!(dts.contains("import('./types/valuation-result.js').ValuationDetails"));
    let host = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("types/valuation-result.d.ts"),
    )
    .expect("read generated facade host declarations");
    let monte_carlo = interface_block(&host, "MonteCarloValuationDetails");
    for field in [
        "model_key:",
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
    for variant in [
        "composite",
        "credit_derivative",
        "fx",
        "structured_credit_stochastic",
        "monte_carlo",
    ] {
        assert!(
            host.contains(&format!("type: \"{variant}\";")),
            "missing host detail variant {variant}"
        );
    }
    assert!(contains_ignoring_ws(
        interface_block(&host, "StochasticPricingResult"),
        "num_paths: bigint;"
    ));
    assert!(contains_ignoring_ws(
        interface_block(&host, "FxValuationDetails"),
        "fx_triangulated?: boolean | null;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "priceInstrument(instrumentJson: JsonInput, marketJson: JsonInput, asOf: string, model?: string | null, metrics?: string[] | null, metricPricingOverrides?: JsonInput | null, marketHistory?: JsonInput | null): ValuationResult;",
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
        "validateValuationResultJson(json: JsonInput): string;",
    ));
}

#[test]
fn structured_credit_tranche_analytics_declare_typed_results() {
    // OAS / metrics / scenario-table are computation results: they return
    // typed plain objects matching the Python `OasResult` / `TrancheMetrics`
    // / `ScenarioTable` wrappers' snake_case shape, not JSON strings. The
    // scalar entry points (discount margin, break-even CDR) stay numbers.
    let dts = index_dts();

    assert!(declares_type(&dts, "OasResult"));
    assert!(declares_type(&dts, "TrancheMetrics"));
    assert!(declares_type(&dts, "ScenarioTable"));
    assert!(declares_type(&dts, "ScenarioCell"));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheOas(instrumentJson: JsonInput, trancheId: string, marketJson: JsonInput, asOf: string, marketPricePct: number, config?: string | null): OasResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheMetrics(instrumentJson: JsonInput, trancheId: string, marketJson: JsonInput, asOf: string, marketPricePct?: number | null): TrancheMetrics;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheScenarioTable(instrumentJson: JsonInput, trancheId: string, marketJson: JsonInput, asOf: string, grid: string): ScenarioTable;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheDiscountMargin(instrumentJson: JsonInput, trancheId: string, marketJson: JsonInput, asOf: string, marketPricePct: number): number;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "structuredCreditTrancheBreakevenCdr(instrumentJson: JsonInput, trancheId: string, marketJson: JsonInput, asOf: string): number;",
    ));
}

#[test]
fn portfolio_cashflow_api_uses_full_cashflow_name_everywhere() {
    let dts = index_dts();
    let bench = benchmark_script();

    assert!(contains_signature(
        &dts,
        "aggregateFullCashflows(specJson: JsonInput, marketJson: JsonInput, allowPartial?: boolean): PortfolioCashflows;",
    ));
    assert!(contains_signature(
        &dts,
        "aggregateFullCashflowsBuilt(portfolio: Portfolio, marketJson: JsonInput, allowPartial?: boolean): PortfolioCashflows;",
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
    assert!(dts.contains("schema-generated `types/generated/<crate>/` modules"));
}

#[test]
fn scenarios_dts_matches_structured_surface() {
    let dts = index_dts();

    assert!(dts.contains("export interface ScenariosNamespace"));
    assert!(declares_type(&dts, "ScenarioSpec"));
    assert!(declares_type(&dts, "TemplateMetadata"));
    assert!(declares_type(&dts, "Warning"));
    assert!(declares_type(&dts, "ApplicationEnvelope"));
    assert!(
        interface_block(&generated_types("scenarios"), "ApplicationEnvelope")
            .contains("warnings: Warning[];")
    );
    assert!(contains_ignoring_ws(
        &dts,
        "computeHorizonReturn(instrumentJson: JsonInput, marketJson: JsonInput, asOf: string, scenarioJson: JsonInput, method?: 'parallel' | 'waterfall' | 'metrics_based' | 'taylor', configJson?: JsonInput, calendarId?: string): HorizonReport;",
    ));
    assert!(declares_type(&dts, "HorizonResult"));
    assert!(contains_ignoring_ws(
        &dts,
        "export type HorizonReport = HorizonResult & { summary: HorizonSummary };"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "applyScenario(scenarioJson: JsonInput, marketJson: JsonInput, modelJson: JsonInput, asOf: string, instrumentsJson?: JsonInput, configJson?: JsonInput): ApplicationEnvelope;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "applyScenarioToMarket(scenarioJson: JsonInput, marketJson: JsonInput, asOf: string, instrumentsJson?: JsonInput, configJson?: JsonInput): ApplicationEnvelope;",
    ));
    // `priority` mirrors the Rust serde default (0) and the Python keyword
    // default, so it must stay optional.
    assert!(contains_ignoring_ws(
        &dts,
        "buildScenarioSpec(id: string, operations: OperationSpec[], name?: string, description?: string, priority?: number, resolutionMode?: 'most_specific_wins' | 'cumulative', hazardBumpMode?: 'solve_to_par' | 'first_order_shift'): ScenarioSpec;",
    ));
    // Free-function twins of the Python OperationSpec / HorizonResult members:
    // the omitted trailing arguments resolve to the Rust defaults.
    for signature in [
        "operationSpecCurveParallelBp(curveKind: CurveKind, curveId: string | string[], bp: number, discountCurveId?: string): OperationSpec | OperationSpec[];",
        "operationSpecTimeRollForward(period: string, applyShocks?: boolean, rollMode?: TimeRollMode): OperationSpec;",
        "scenarioSpecWithHazardBumpMode(spec: ScenarioSpec | string, mode: HazardBumpMode): ScenarioSpec;",
        "horizonResultExplainText(result: HorizonReport | HorizonResult | string): string;",
        "horizonResultFactorContribution(result: HorizonReport | HorizonResult | string, factor: AttributionFactor): number;",
        "compoundingSemiAnnual(): Compounding;",
    ] {
        assert!(contains_ignoring_ws(&dts, signature), "{signature}");
    }
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
        "valuePortfolio(specJson: JsonInput, marketJson: JsonInput, strictRisk?: boolean, metrics?: string[]): PortfolioValuation;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "valuePortfolioBuilt(portfolio: Portfolio, marketJson: JsonInput, strictRisk?: boolean, metrics?: string[]): PortfolioValuation;",
    ));
    // Python defaults `confidence` from the Rust 95% presets; WASM does too.
    assert!(contains_ignoring_ws(
        &dts,
        "parametricVarDecomposition(positionIds: string[], weights: NumericArray, covariance: NumericArray[], confidence?: number | null, computeIncremental?: boolean | null): PositionRiskDecomposition;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "parametricEsDecomposition(positionIds: string[], weights: NumericArray, covariance: NumericArray[], confidence?: number | null): ParametricEsDecompositionView;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "historicalVarDecomposition(positionIds: string[], positionPnls: NumericArray[], confidence?: number | null): PositionRiskDecomposition;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateRiskBudget(positionIds: string[], actualVar: NumericArray, targetVarPct: NumericArray, portfolioVar: number, utilizationThreshold?: number | null): RiskBudgetResult;",
    ));
    // The WASM-only reporting views are gone: both hosts return the canonical
    // Rust result types.
    assert!(!dts.contains("VarDecompositionResult"));
    assert!(!dts.contains("EsDecompositionResult"));
    assert!(declares_type(&dts, "ImpactEstimate"));
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
        "campisiAttribution(portfolioJson: JsonInput, benchmarkJson: JsonInput, configJson: JsonInput): FiAttributionResult;",
    ));
    assert!(contains_signature(
        &dts,
        "campisiCarinoLink(periodsJson: JsonInput): FiCarinoLinkedResult;",
    ));
    assert!(contains_signature(
        &dts,
        "campisiCarinoLinkFromSnapshots(periodsJson: JsonInput, configJson: JsonInput): FiCarinoLinkedResult;",
    ));
    assert!(contains_signature(
        &dts,
        "campisiReconciliationCheck(resultJson: JsonInput, tolerance: number): FiReconciliationReport;",
    ));

    // The results-based linker must not grow a config argument: it links
    // periods that already carry their own `period_years`.
    assert!(!contains_ignoring_ws(
        &dts,
        "campisiCarinoLink(periodsJson: JsonInput, configJson: JsonInput)",
    ));
}

#[test]
fn models_liquidity_dts_exposes_reference_price_for_almgren_chriss() {
    let dts = index_dts();
    let liquidity = interface_block(&dts, "LiquidityNamespace");
    let models = generated_types("models");
    let impact = interface_block(&models, "ImpactEstimate");

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
        "kyleLambda(returns: NumericArray, volumes: NumericArray, referencePrice: number): number | undefined;",
    ));
    assert!(contains_signature(
        liquidity,
        "liquidityTier(daysToLiquidate: number, thresholds?: NumericArray | null): string;",
    ));
    assert!(!liquidity.contains("Json: JsonInput"));
    assert!(!interface_block(&dts, "PortfolioNamespace").contains("kyleLambda("));
}

#[test]
fn core_daycount_dts_exposes_context_for_context_dependent_conventions() {
    let dts = index_dts();

    assert!(dts.contains("export interface DayCountContext"));
    // One method per Rust fn: the context is an optional third argument.
    let day_count = interface_block(&dts, "DayCount ");
    assert!(contains_signature(
        day_count,
        "yearFraction(startEpochDays: number, endEpochDays: number, ctx?: DayCountContext): number;",
    ));
    assert!(!dts.contains("yearFractionWithContext"));
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
    // `calendarDays` is static, like Rust `DayCount::calendar_days`.
    assert!(contains_signature(
        day_count_ctor,
        "calendarDays(startEpochDays: number, endEpochDays: number): bigint;"
    ));
    assert!(!day_count.contains("calendarDays("));
    assert!(contains_ignoring_ws(
        &dts,
        "Act/Act ISMA needs a context frequency (or coupon period) and Bus/252 a"
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
        "MarketContext ",
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
        "Portfolio",
    ] {
        assert!(
            dts.contains(&format!("interface {class_name} extends WasmOwned {{}}")),
            "{class_name} must merge the wasm ownership contract"
        );
    }
}

#[test]
fn statements_dts_matches_runtime_exports() {
    let dts = index_dts();

    assert!(dts.contains("export interface StatementsNamespace"));
    assert!(dts.contains("validateFinancialModelJson(json: JsonInput): string;"));
    assert!(dts.contains("modelNodeIds(json: JsonInput): string[];"));
    assert!(dts.contains("validateCheckSuiteSpecJson(json: JsonInput): string;"));
    // Computation results are structured objects, not JSON strings: a string
    // return here would put JS out of step with the typed Python result.
    assert!(dts.contains("export interface Evaluator extends WasmOwned {"));
    assert!(dts.contains("Evaluator: EvaluatorConstructor;"));
    assert!(dts.contains("evaluate(model: JsonInput): StatementResult;"));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateWithMarket(model: JsonInput, market: JsonInput, asOf: string): StatementResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateMonteCarlo(model: JsonInput, config: generated.statements.MonteCarloConfig | string): MonteCarloResults;",
    ));
    // The free-function evaluators were replaced by the `Evaluator` class.
    assert!(!dts.contains("evaluateModel("));
    assert!(!dts.contains("evaluateModelWithMarket("));
    assert!(declares_type(&dts, "StatementResult"));
    assert!(dts.contains("export declare const statements: StatementsNamespace;"));
}

/// Covenant evaluation returns typed reports; the JSON bridge helpers keep
/// their `Json`-suffixed names and string returns.
#[test]
fn covenants_dts_separates_typed_results_from_json_bridges() {
    let dts = index_dts();

    assert!(dts.contains("export interface CovenantsNamespace"));
    assert!(declares_type(&dts, "CovenantReport"));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateEngine(engineJson: JsonInput, metricsJson: JsonInput, asOf: string): Record<string, CovenantReport>;",
    ));
    for wire in [
        "validateCovenantSpecJson(specJson: JsonInput): string;",
        "validateCovenantReportJson(reportJson: JsonInput): string;",
        "validateCovenantEngineJson(engineJson: JsonInput): string;",
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
    // goalSeek returns the Rust `GoalSeekResult`: the updated model is an
    // object under `model` (the Python field name), never a nested JSON string.
    let generated = generated_types("statements_analytics");
    let goal_seek = interface_block(&generated, "GoalSeekResult");
    assert!(goal_seek.contains("solved_value: number;"));
    assert!(goal_seek.contains("model?: statements.FinancialModelSpec | null;"));
    assert!(!dts.contains("updated_model_json"));
    assert!(contains_ignoring_ws(
        &dts,
        "goalSeek(modelJson: JsonInput, targetNode: string, targetPeriod: string, targetValue: number, driverNode: string, driverPeriod: string, updateModel: boolean, bounds?: [number, number] | null): GoalSeekResult;",
    ));
    // The dependency tree is the Rust `DependencyTree`; the ASCII rendering
    // says so in its name.
    assert!(declares_type(&dts, "DependencyTree"));
    assert!(dts.contains("DependencyTracer: DependencyTracerConstructor;"));
    assert!(contains_ignoring_ws(
        &dts,
        "dependencyTree(nodeId: string): DependencyTree;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "dependencyTreeText(nodeId: string): string;"
    ));
    assert!(!dts.contains("traceDependencies"));
    assert!(declares_type(&dts, "Explanation"));
    assert!(contains_ignoring_ws(
        &dts,
        "explainFormula(modelJson: JsonInput, resultsJson: JsonInput, nodeId: string, period: string): Explanation;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "explainFormulaText(modelJson: JsonInput, resultsJson: JsonInput, nodeId: string, period: string): string;"
    ));
    // Converted computation results: object returns, matching the typed
    // Python results for the same Rust calls.
    assert!(contains_ignoring_ws(
        &dts,
        "runSensitivity(modelJson: JsonInput, configJson: JsonInput): SensitivityResult;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runVariance(baseJson: JsonInput, comparisonJson: JsonInput, configJson: JsonInput): VarianceReport;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateScenarioSet(modelJson: JsonInput, scenarioSetJson: JsonInput): ScenarioResults;",
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "creditAssessment(resultsJson: JsonInput, period: string): CreditAssessment;",
    ));
    assert!(declares_type(&dts, "DcfSensitivityResult"));
    // dcfSensitivity mirrors the Rust signature: the DCF knobs arrive as one
    // Rust `DcfOptions` document, not a hand-picked subset of scalars.
    assert!(contains_ignoring_ws(
        &dts,
        "dcfSensitivity(modelJson: JsonInput, wacc: number, terminalValueJson: JsonInput, ufcfNode?: string | null, netDebtOverride?: number | null, optionsJson?: JsonInput | null, marketJson?: JsonInput | null): DcfSensitivityResult;",
    ));
    // evaluateLbo takes the Rust `LboConfig` (check mappings included) and
    // returns its check report.
    assert!(declares_type(&dts, "LboResult"));
    assert!(
        interface_block(&generated_types("statements_analytics"), "LboResult")
            .contains("checks?: CheckReport | null;")
    );
    assert!(contains_ignoring_ws(
        &dts,
        "evaluateLbo(modelJson: JsonInput, configJson: JsonInput): LboResult;"
    ));
    assert!(declares_type(&dts, "TornadoEntry"));
    assert!(contains_ignoring_ws(
        &dts,
        "generateTornadoEntries(resultJson: JsonInput, metricNode: string, period?: string | null): TornadoEntry[];"
    ));
    assert!(declares_type(&dts, "CheckReport"));
    assert!(declares_type(&dts, "CheckResult"));
    assert!(declares_type(&dts, "CheckFinding"));
    assert!(declares_type(&dts, "CheckSummary"));
    assert!(contains_ignoring_ws(
        &dts,
        "runChecks(modelJson: JsonInput, suiteSpecJson: JsonInput, resultsJson?: JsonInput | null): CheckReport;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runThreeStatementChecks(modelJson: JsonInput, mappingJson: JsonInput, resultsJson?: JsonInput | null): CheckReport;"
    ));
    assert!(contains_ignoring_ws(
        &dts,
        "runCreditUnderwritingChecks(modelJson: JsonInput, mappingJson: JsonInput, resultsJson?: JsonInput | null): CheckReport;"
    ));
    assert!(dts.contains("renderCheckReportText(reportJson: JsonInput): string;"));
    assert!(dts.contains("renderCheckReportHtml(reportJson: JsonInput): string;"));
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
    let merton = interface_block(&dts, "MertonModel");
    let merton_statics = interface_block(&dts, "MertonModelConstructor");
    assert!(merton
        .contains("defaultProbabilityWithDrift(assetDrift: number, horizon: number): number;"));
    assert!(
        merton.contains("distanceToDefaultWithDrift(assetDrift: number, horizon: number): number;")
    );
    assert!(merton.contains("debtSpread(horizon: number): number;"));
    assert!(merton.contains("cdsParSpread(maturity: number, recovery: number): number;"));
    assert!(merton.contains("): HazardCurve;"));
    assert!(merton_statics
        .contains("kmvDefaultPoint(shortTermDebt: number, longTermDebt: number): number;"));
    assert!(merton_statics.contains("creditGrades("));
    assert!(model_credit.contains("MertonModel: MertonModelConstructor;"));
    assert!(model_credit.contains("ToggleExerciseModel: ToggleExerciseModelConstructor;"));
    assert!(!dts.contains("mertonModelJson("));
    assert!(!dts.contains("creditStateJson("));
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
    // Path count and seed are optional: the Rust registry owns their defaults.
    assert_eq!(monte_carlo.matches("numPaths?: number | null,").count(), 2);
    assert_eq!(
        monte_carlo
            .matches("seed?: bigint | number | null,")
            .count(),
        2
    );
    assert_eq!(monte_carlo.matches("): MoneyEstimate;").count(), 2);
    assert!(declares_type(&dts, "MoneyEstimate"));
    assert!(!dts.contains("MonteCarloEstimateJson"));
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
        "riskScaledWeights(values: FeatureValue[], timeKey: string[], volatility: FeatureValue[]): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "rankToWeights(values: FeatureValue[], timeKey: string[]): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "neutralizeAndZscore(values: FeatureValue[], timeKey: string[], exposures: FeatureValue[][], params?: FeatureParams | null): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "transformPanelJson(specJson: JsonInput): string;"
    ));
    assert!(contains_signature(
        features,
        "transformPanel(spec: PanelTransformSpec | string): PanelTransformResult;"
    ));
    assert!(contains_signature(
        features,
        "panelTransformResultGetColumn(result: PanelTransformResult | string, name: string): FeatureValue[];"
    ));
    assert!(contains_signature(
        features,
        "timeSeriesOpParamKeys(op: TimeSeriesOp): string[];"
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
    assert!(contains_signature(fx_result, "toJson(): string;"));
    assert!(contains_signature(
        interface_block(&dts, "FxRateResultConstructor "),
        "fromJson(json: JsonInput): FxRateResult;"
    ));

    // Money exposes the lossless decimal-string getter and the Rust-named arithmetic.
    let money = interface_block(&dts, "Money ");
    assert!(contains_signature(money, "readonly amountDecimal: string;"));
    assert!(contains_signature(
        money,
        "convertAtRate(target: Currency, rate: number): Money;"
    ));
    for sig in [
        "checkedAdd(other: Money): Money;",
        "checkedSub(other: Money): Money;",
        "checkedMulF64(factor: number): Money;",
        "checkedDivF64(divisor: number): Money;",
        "checkedNeg(): Money;",
    ] {
        assert!(contains_signature(money, sig), "Money: {sig}");
    }
    for legacy in ["add(", "sub(", "mulScalar(", "divScalar(", "negate("] {
        assert!(!money.contains(&format!("\n  {legacy}")), "Money: {legacy}");
    }
    assert!(contains_signature(
        interface_block(&dts, "MoneyConstructor "),
        "fromDecimalStr(amount: string, currency: Currency): Money;"
    ));

    // Rate / Bps / Percentage accessors are getters, as in Rust-shaped Python.
    for (name, getters) in [
        (
            "Rate ",
            &[
                "asDecimal: number",
                "asPercent: number",
                "asBp: number",
                "asBasisPoints: Bps",
                "asPercentage: Percentage",
            ][..],
        ),
        (
            "Bps ",
            &[
                "asDecimal: number",
                "asBp: number",
                "asPercent: number",
                "asRate: Rate",
                "asPercentage: Percentage",
            ][..],
        ),
        (
            "Percentage ",
            &[
                "asDecimal: number",
                "asPercent: number",
                "asBp: number",
                "asRate: Rate",
                "asBasisPoints: Bps",
            ][..],
        ),
    ] {
        let block = interface_block(&dts, name);
        for getter in getters {
            assert!(
                contains_signature(block, &format!("readonly {getter};")),
                "{name}{getter}"
            );
        }
        assert!(
            contains_signature(block, "toJson(): string;"),
            "{name}toJson"
        );
        assert!(
            contains_signature(block, "isZero(): boolean;"),
            "{name}isZero"
        );
    }
    assert!(contains_signature(
        interface_block(&dts, "RateConstructor "),
        "parse(text: string): Rate;"
    ));

    // DayCountContext mirrors DayCountContextState: constructor, getters and JSON.
    let ctx = interface_block(&dts, "DayCountContext ");
    for sig in [
        "readonly calendarId: string | undefined;",
        "readonly frequency: Tenor | undefined;",
        "readonly busBasis: number | undefined;",
        "readonly couponPeriod: Int32Array | undefined;",
        "readonly endIsTerminationDate: boolean;",
        "toJson(): string;",
    ] {
        assert!(contains_signature(ctx, sig), "DayCountContext: {sig}");
    }
    assert!(!ctx.contains("withCouponPeriod"));
    assert!(!ctx.contains("withCalendar"));
    let ctx_constructor = interface_block(&dts, "DayCountContextConstructor ");
    assert!(contains_signature(
        ctx_constructor,
        "fromJson(json: JsonInput): DayCountContext;"
    ));
    assert!(contains_signature(
        ctx_constructor,
        "couponPeriod?: readonly [number, number] | null,"
    ));

    // Tenor binds the Rust Tenor API.
    let tenor = interface_block(&dts, "Tenor ");
    for sig in [
        "readonly unit: string;",
        "readonly months: number | undefined;",
        "readonly days: number | undefined;",
        "paymentsPerYear(): number;",
        "toDaysApprox(): number;",
        "addToDate(epochDays: number, calendarCode?: string | null, convention?: string | null): number;",
    ] {
        assert!(contains_signature(tenor, sig), "Tenor: {sig}");
    }
    let tenor_constructor = interface_block(&dts, "TenorConstructor ");
    for sig in [
        "parse(s: string): Tenor;",
        "fromYears(years: number, dayCount: DayCount): Tenor;",
        "fromPaymentsPerYear(payments: number): Tenor;",
        "biweekly(): Tenor;",
        "bimonthly(): Tenor;",
    ] {
        assert!(
            contains_signature(tenor_constructor, sig),
            "TenorConstructor: {sig}"
        );
    }

    // Curve and surface handles round-trip their canonical JSON.
    for name in [
        "HazardCurve ",
        "ForwardCurve ",
        "VolCube ",
        "FxDeltaVolSurface ",
    ] {
        assert!(
            contains_signature(interface_block(&dts, name), "toJson(): string;"),
            "{name}toJson"
        );
    }
    let hazard_constructor = interface_block(&dts, "HazardCurveConstructor ");
    for sig in [
        "new (options: HazardCurveOptions | string): HazardCurve;",
        "flat(id: string, baseDate: string, hazardRate: number, recoveryRate: number): HazardCurve;",
        "fromJson(json: JsonInput): HazardCurve;",
    ] {
        assert!(contains_signature(hazard_constructor, sig), "HazardCurveConstructor: {sig}");
    }
    assert!(contains_signature(
        interface_block(&dts, "ForwardCurveConstructor "),
        "fromJson(json: JsonInput): ForwardCurve;"
    ));
    assert!(contains_signature(
        interface_block(&dts, "VolCube "),
        "forwardAt(expIdx: number, tenorIdx: number): number;"
    ));
    assert!(contains_signature(
        interface_block(&dts, "FxDeltaVolSurface "),
        "readonly rr10d: Float64Array | undefined;"
    ));

    let fx = interface_block(&dts, "FxMatrix ");
    assert!(contains_signature(
        fx,
        "setQuoteOn(base: string, quote: string, date: string, policy: FxConversionPolicy, rate: number): void;"
    ));
    assert!(contains_signature(
        fx,
        "rate(base: string, quote: string, date: string, policy?: FxConversionPolicy): FxRateResult;"
    ));
    assert!(!dts.contains("rateDefault"));

    let day_count = interface_block(&dts, "DayCount ");
    assert!(contains_signature(
        day_count,
        "signedYearFraction(startEpochDays: number, endEpochDays: number, ctx?: DayCountContext): number;"
    ));
    let day_count_constructor = interface_block(&dts, "DayCountConstructor ");
    assert!(contains_signature(
        day_count_constructor,
        "act365l(): DayCount;"
    ));
    for sig in [
        "fromName(name: string): DayCount;",
        "parse(s: string): DayCount;",
        "nl365(): DayCount;",
    ] {
        assert!(
            contains_signature(day_count_constructor, sig),
            "DayCountConstructor: {sig}"
        );
    }
    assert!(!day_count_constructor.contains("new (name: string)"));
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
    assert!(contains_signature(pair, "readonly settlementDays: number;"));
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
    assert!(declares_type(&dts, "PnlAttribution"));
    assert!(dts.contains("attributePnl(params: AttributionJsonInputs): PnlAttribution;"));
    assert!(dts.contains("attributePnlJson(params: AttributionJsonInputs): string;"));
    assert!(dts.contains("AttributionJsonInputs: new ("));
    assert!(dts.contains("attributePnlEnvelope(specJson: JsonInput): AttributionResultEnvelope;"));
    assert!(declares_type(&dts, "AttributionResultEnvelope"));
    assert!(dts.contains("attributePnlEnvelopeJson(specJson: JsonInput): string;"));
    assert!(dts.contains("validateAttributionJson(json: JsonInput): string;"));
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
        "cellReturnsFromReference(referenceJson: JsonInput, baseLabel: string, configJson: JsonInput): DurationCellTable;",
    ));
    assert!(contains_signature(
        &dts,
        "cellReturnsFromCurves(\
           start: DiscountCurve, \
           end: DiscountCurve, \
           horizonYears: number, \
           maxDuration: number, \
           baseLabel: string, \
           configJson: JsonInput\
         ): DurationCellTable;",
    ));
    assert!(contains_signature(
        &dts,
        "excessReturns(positionsJson: JsonInput, tableJson: JsonInput): ExcessReturnResult;",
    ));
    assert!(contains_signature(
        &dts,
        "gridAttribution(portfolioJson: JsonInput, benchmarkJson: JsonInput): GridAttributionResult;",
    ));
    assert!(contains_signature(
        &dts,
        "gridCarinoLink(periodsJson: JsonInput): GridCarinoLinkedResult;",
    ));
    assert!(contains_signature(
        &dts,
        "factorBrinsonAttribution(inputJson: JsonInput, factorReturns: NumericArray): FactorBrinsonResult;",
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

/// Plain-object results carry the Rust type name of what they hold, and every
/// Rust boundary type with a JSON Schema is the schema-generated declaration
/// (re-exported from `types/generated`), never a hand-written copy.
#[test]
fn result_types_use_rust_names_from_the_generated_contract() {
    let dts = index_dts();

    for name in [
        "ForecastMetrics",
        "PeerStats",
        "RegressionResult",
        "DimensionScore",
        "RelativeValueResult",
        "TrancheLossStatistics",
        "LvarBangiaScalar",
        "LeverageImpact",
        "XvaResult",
        "MoneyEstimate",
        "RiskDecomposition",
        "PositionRiskDecomposition",
        "ParametricEsDecompositionView",
        "RiskBudgetResult",
        "StatementResult",
        "Explanation",
        "ScenarioCell",
        "CreditAssessment",
        "BrinsonPeriodResult",
        "LinkedReturn",
        "PortfolioValuation",
        "HorizonReport",
        "RollingGreeks",
    ] {
        assert!(declares_type(&dts, name), "missing type {name}");
    }
    for stale in [
        "BacktestForecastMetricsJson",
        "PeerStatsJson",
        "RegressionResultJson",
        "DimensionScoreJson",
        "RelativeValueResultJson",
        "TrancheLossStatisticsJson",
        "XvaResultJson",
        "MonteCarloEstimateJson",
        "FormulaExplanationJson",
        "StatementResultJson",
        "LvarBangiaResult",
        "LmeLeverageImpact",
        "RollingGreeksResult",
        "FactorRiskDecomposition",
        "TrancheScenarioCell",
        "ScenarioApplyResult",
        "VarDecompositionResult",
    ] {
        assert!(!dts.contains(stale), "stale hand-written name {stale}");
    }
    // Generated types are re-exported, not redeclared by hand.
    for generated in ["PeerStats", "CovenantReport", "ScenarioSpec", "XvaResult"] {
        assert!(
            !dts.contains(&format!("export interface {generated} ")),
            "{generated} is hand-written instead of generated"
        );
    }
    assert!(dts.contains("import type * as generated from './types/generated/index.js';"));
    // The documented host-only survivors keep their JavaScript shapes.
    assert!(interface_block(&dts, "RollingGreeks").contains("alphas: Float64Array;"));
    assert!(contains_ignoring_ws(
        &dts,
        "export type FxInstrumentSpec = Record<string, unknown>;"
    ));
}

/// The thrown error objects are declared with the Rust-owned kind vocabulary.
#[test]
fn error_types_are_declared() {
    let dts = index_dts();

    assert!(contains_ignoring_ws(
        &dts,
        "export type FinstackErrorKind = 'not_found' | 'validation' | 'computation' | 'invalid_type';"
    ));
    let finstack = interface_block(&dts, "FinstackError");
    assert!(contains_ignoring_ws(
        finstack,
        "name: 'FinstackError' | 'TypeError';"
    ));
    assert!(contains_ignoring_ws(finstack, "kind: FinstackErrorKind;"));
    assert!(contains_ignoring_ws(finstack, "code?: string;"));
    let calibration = interface_block(&dts, "CalibrationEnvelopeError");
    for field in [
        "name: 'CalibrationEnvelopeError';",
        "kind: string;",
        "stage: 'ingestion' | 'configuration' | 'context' | 'preflight' | 'target' | 'solver';",
        "step_id?: string;",
        "diagnostics: StrictLoadDiagnostic[];",
        "details: string;",
        "cause: unknown;",
    ] {
        assert!(contains_ignoring_ws(calibration, field), "missing {field}");
    }
    assert!(declares_type(&dts, "ContractValidationError"));
    assert!(!dts.contains("AttributionError"));
}

/// S21: Rust computations that were reachable only from Python are declared
/// on the facade with their Rust-canonical names and structured returns.
#[test]
fn rust_computation_twins_are_declared() {
    let dts = index_dts();
    for sig in [
        "absToSmm(speed: number, month: number): number;",
        "scheduleWal(scheduleJson: JsonInput, asOf: string): number;",
        "scheduleOutstandingByDate(scheduleJson: JsonInput): DatedFlowJson[];",
        "scheduleCalendarYearLadder(scheduleJson: JsonInput, pvs: number[] | Float64Array): CalendarYearLadderRow[];",
        "instrumentCashflows(instrumentJson: JsonInput, marketJson: JsonInput, asOf: string, model: string): generated.valuations.InstrumentCashflowEnvelope;",
        "valuationResultMetricSeries(result: ValuationResult | string, base: string): [string[], number][];",
        "calibrationEnvelopeContentHash(envelopeJson: CalibrationEnvelope | string): string;",
        "calibrationResultContentHash(resultJson: CalibrationResultEnvelope | string): string;",
        "attributePnlMany(params: AttributionJsonInputs, instruments: JsonInput[]): PnlAttribution[];",
        "attributeReturnContribution(spec: JsonInput): ReturnContributionResult;",
        "pnlAttributionExplainText(pnl: PnlAttribution | string): string;",
        "financialModelContentHash(modelJson: JsonInput): string;",
        "nodeToDatedSchedule(modelJson: JsonInput, resultJson: StatementResult | string, nodeId: string, convention?: 'end' | 'start' | null): [string, number][];",
        "scenarioComparisonTable(scenarioResultsJson: ScenarioResults | string, metrics: string[]): TableEnvelope;",
        "monteCarloPercentileByPeriod(resultsJson: MonteCarloResults | string, metric: string, percentile: number): Record<string, number> | undefined;",
        "statementResultToTableLong(resultJson: StatementResult | string): TableEnvelope;",
        "statementResultToTableWide(resultJson: StatementResult | string): TableEnvelope;",
        "synthesizedDealJson(): string;",
        "rebalanceFromSpec(specJson: JsonInput, resultJson: PortfolioOptimizationResult | string): Portfolio;",
        "netInCurrencyByDate(cashflowsJson: PortfolioCashflows | string, currency: string): [string, number][];",
        "portfolioMetricsSeries(metricsJson: PortfolioMetrics | string, base: string): PortfolioMetricSeriesEntry[];",
        "readonly entityIds: string[];",
        "readonly positionIds: string[];",
        "zeroCoupon(id: string, notional: Money, issueDate: string, maturity: string, discountCurveId: string): Bond;",
        "exampleFloatingWithDdtl(): TermLoan;",
        "facilityIrr(marketJson: JsonInput, asOf: string): number;",
        "expectedCashflows(marketJson: JsonInput, asOf: string): generated.valuations.CashFlowSchedule;",
        "marketForwardRate(marketJson: JsonInput, asOf: string): number;",
        "impliedVol(marketJson: JsonInput, asOf: string, targetPrice: number): number;",
        "FxForward: FxForwardConstructor;",
        "FxOption: FxOptionConstructor;",
    ] {
        assert!(contains_signature(&dts, sig), "index.d.ts is missing `{sig}`");
    }
}

/// P2: the typed cashflows and covenants surface is declared with handles for
/// the stateful types, wire types for data, and optional arguments where Rust
/// takes an `Option`.
#[test]
fn typed_cashflows_and_covenants_surface_is_declared() {
    let dts = index_dts();
    for sig in [
        "CashFlowSchedule: CashFlowScheduleConstructor;",
        "CashFlowBuilder: CashFlowBuilderConstructor;",
        "AccrualIndex: AccrualIndexConstructor;",
        "CovenantEngine: CovenantEngineConstructor;",
        "builder(): CashFlowBuilder;",
        "build(market?: MarketContext | null): CashFlowSchedule;",
        "buildCashflowSchedule(spec: generated.cashflows.CashflowScheduleBuildSpec | string, market?: MarketContext | null): CashFlowSchedule;",
        "scaleAmounts(scale: number): CashFlowSchedule;",
        "pvByPeriod(periods: readonly generated.statements.Period[], market: MarketContext, discCurveId: string, base: string, dayCount?: generated.core.DayCount | null, creditCurveId?: string | null): generated.cashflows.PeriodAggregation;",
        "fixedToFloat(switchDate: string, fixed: generated.cashflows.FixedCouponSpec, floating: generated.cashflows.FloatingCouponSpec): CashFlowBuilder;",
        "accruedInterestAmount(schedule: CashFlowSchedule, asOf: string, config?: generated.cashflows.AccrualConfig | null): MoneyValue;",
        "accruedAt(asOf: string): MoneyValue;",
        "periodAggregationGetAmount(aggregation: generated.cashflows.PeriodAggregation, period: string, currency: generated.core.Currency): MoneyValue | undefined;",
        "mergeCashflowSchedules(schedules: readonly (CashFlowSchedule | JsonInput)[], notional: generated.cashflows.Notional, dayCount: generated.core.DayCount): CashFlowSchedule;",
        "materializeFixings(market: MarketContext, schedules: readonly (CashFlowSchedule | JsonInput)[], oldDate: string, newDate: string): MarketContext;",
        "cashFlowGetBalanceDate(flow: generated.cashflows.CashFlow): generated.core.DateWire;",
        "scheduleParamsUsdSofrSwap(): generated.cashflows.ScheduleParams;",
        "covLite(maxLeverage: number, maxSeniorLeverage: number): generated.covenants.CovenantSpec[];",
        "evaluate(metrics: Record<string, number> | string, asOf: string): Record<string, CovenantReport>;",
        "evaluateSeries(metrics: readonly generated.covenants.DatedMetrics[]): generated.covenants.DatedCovenantReports[];",
        "readonly breachHistory: generated.covenants.CovenantBreach[];",
        "forecastCovenant(spec: generated.covenants.CovenantSpec, metrics: readonly generated.covenants.DatedMetrics[], config?: generated.covenants.CovenantForecastConfig | null): generated.covenants.CovenantForecast;",
        "forecastBreaches(engine: CovenantEngine, metrics: readonly generated.covenants.DatedMetrics[], config?: generated.covenants.CovenantForecastConfig | null): generated.covenants.FutureBreach[];",
        "covenantTypeCustom(metric: string, test: 'maximum' | 'minimum', value: number): generated.covenants.CovenantType;",
        "thresholdScheduleThresholdFor(schedule: generated.covenants.ThresholdSchedule, testDate: string): number | undefined;",
    ] {
        assert!(contains_signature(&dts, sig), "index.d.ts is missing `{sig}`");
    }
    // The facade joins the market / no-market twins; the raw twins are not published.
    assert!(!dts.contains("buildWithMarket("));
    assert!(!dts.contains("buildCashflowScheduleWithMarket("));
}
