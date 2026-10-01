// Type declarations for the finstack-quant-wasm namespaced facade.
// Shapes follow `wasm-bindgen` JS names in `src/api/**` (see Rust `js_name`).
// The raw `pkg/finstack_quant_wasm.d.ts` emitted by wasm-bindgen is intentionally
// not the package root contract: it exposes a flat module, while `index.js`
// publishes a namespaced facade. Keep this file as the facade declaration and
// use the schema-generated `types/generated/<crate>/` modules for JSON contract
// shapes (also published as `finstack-quant-wasm/types`, one namespace per crate).
//
// Building a MarketContext from quotes (canonical path):
//
//   import { calibration } from 'finstack-quant-wasm/exports/calibration.js';
//   import type { CalibrationEnvelope } from 'finstack-quant-wasm';
//   const envelope: CalibrationEnvelope = {
//     schema: 'finstack_quant.calibration/1',
//     plan: { id: 'usd_curves', quote_sets: {...}, steps: [...], settings: {} },
//     market_data: [],   // flat id-addressable quotes/snapshots
//     prior_market: [],  // optional pre-built curves/surfaces
//   };
//   const result = calibration.calibrate(envelope);  // CalibrationResultEnvelope
//   const marketJson = JSON.stringify(result.result.final_market);
//
// `result.result.final_market` is the materialized MarketContextState ready
// for any downstream pricing / scenario / attribution call that takes a
// market_json argument. Always check the per-step report
// (`result.result.step_reports`) and the plan summary
// (`result.result.report`) to confirm the curves actually fit before using
// the market downstream.
//
// `validateCalibrationJson` is a fast pre-flight check that canonicalizes
// the envelope without solving — use it to surface schema errors early.
//
// Errors: every thrown error is a `FinstackError` (`kind` mirrors the Rust
// `ErrorKind`; a wrong-type argument is a `TypeError` with kind
// 'invalid_type'). Calibration execution failures are
// `CalibrationEnvelopeError`s and persisted-contract failures are
// `ContractValidationError`s; both are declared below.

// WASM ownership: every wasm-bindgen class exposed below owns a wasm heap
// allocation. Call `free()` when a handle is no longer needed. On runtimes
// that define `Symbol.dispose`, wasm-bindgen also installs
// `instance[Symbol.dispose] === instance.free`. Plain JSON results, arrays,
// and namespace functions do not need manual disposal.

/**
 * Inputs accepted by the wasm-bindgen web initializer.
 */
export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

/**
 * Initialized WebAssembly exports.
 */
export type InitOutput = WebAssembly.Exports;

/**
 * Initialize the package's WebAssembly module.
 * @example
 * ```typescript
 * import init from "finstack-quant-wasm";
 * const wasm = await init();
 * void wasm;
 * ```
 * @param moduleOrPath - Optional module source: a URL, Response, WebAssembly.Module, or Promise accepted by wasm-bindgen initialization.
 * @returns Returns a Promise that resolves to `InitOutput`.
 */
export default function init(
  moduleOrPath?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>
): Promise<InitOutput>;

// --- JSON contract types (generated from the Rust JSON Schemas) ---
//
// Every Rust data type that crosses the boundary (spec, config, result) is
// typed by the TypeScript generated from its JSON Schema under
// `types/generated/<crate>/`, and re-exported here under its Rust type name.
// Hand-written declarations below are limited to host-only shapes: wasm-bindgen
// handle classes and namespaces, error objects, argument aliases, and the few
// results whose JavaScript form differs from the JSON wire (typed arrays,
// numeric non-finite values, lossless `bigint` valuation integers).
//
// Map key order: results keyed by an identifier (position, entity, node or
// period ids) are plain objects. JavaScript enumerates integer-like keys
// ("0".."4294967294") in ascending numeric order before all other keys, so a
// map keyed by ids such as "10" and "2" iterates differently from the Rust
// `IndexMap` insertion order that Python dicts preserve. Keyed lookup is
// unaffected; take ordering from the corresponding input (for example
// `spec.positions`) when it matters.
import type * as generated from './types/generated/index.js';
import type {
  CalibrationEnvelope,
  CalibrationResultEnvelope,
  CalibrationValidationReport,
  StrictLoadDiagnostic,
} from './types/generated/calibration/index.js';
import type {
  SabrParameterData,
  TableColumn,
  TableColumnData,
  TableEnvelope,
} from './types/generated/core/index.js';
import type {
  DrawdownEpisode,
  LookbackReturns,
  PeriodicReturn,
} from './types/generated/analytics/index.js';
import type { CovenantReport } from './types/generated/covenants/index.js';
import type { VmResult, XvaResult } from './types/generated/margin/index.js';
import type {
  ArbitrageValidationResult,
  BsGreeks,
  BumpSizeConfig,
  ExchangeOfferAnalysis,
  FactorContribution,
  FactorCovarianceMatrix,
  FactorDefinition,
  FactorModelConfig,
  FactorType,
  ForwardGreeks,
  ImpactEstimate,
  LeverageImpact,
  LmeAnalysis,
  LvarBangiaScalar,
  MoneyEstimate,
  ParametricEsDecompositionView,
  PositionEsContribution,
  PositionEsContributionView,
  PositionFactorContribution,
  PositionRiskDecomposition,
  PositionVarContribution,
  RiskDecomposition,
  RiskMeasure,
  SviParams,
  TrancheLossStatistics,
  VolatilityConvention,
} from './types/generated/models/index.js';
import type {
  BorrowingBaseReport,
  CompositeExposureReport,
  CompositeHistoryRow,
  CompositeRebalanceResult,
  CompositeTrade,
  EnhancedMonteCarloResult,
  ListedProductCoverage,
  MetricMetadata,
  OasResult,
  PrimitiveAggregate,
  PrimitiveExposure,
  ResultsMeta,
  ScenarioCell,
  ScenarioTable,
  TrancheMetrics,
  ValidationReport,
} from './types/generated/valuations/index.js';
import type {
  AttributionResultEnvelope,
  PnlAttribution,
} from './types/generated/attribution/index.js';
import type {
  CheckCategory,
  CheckFinding,
  CheckReport,
  CheckResult,
  CheckSeverity,
  CheckSummary,
  Materiality,
  MonteCarloResults,
  StatementResult,
} from './types/generated/statements/index.js';
import type {
  CreditAssessment,
  DcfSensitivityResult,
  DependencyTree,
  DimensionScore,
  GoalSeekResult,
  LboResult,
  PeerStats,
  RegressionResult,
  RelativeValueResult,
  ScenarioResults,
  SensitivityResult,
  TornadoEntry,
  VarianceReport,
} from './types/generated/statements_analytics/index.js';
import type {
  AggregatedMetric,
  BrinsonPeriodResult,
  CarinoLinkedAttribution,
  CreditVolReport,
  DurationCellTable,
  Entity,
  ExcessReturnResult,
  FactorBrinsonResult,
  FactorPnlProfile,
  FiAttributionResult,
  FiCarinoLinkedResult,
  FiReconciliationReport,
  GridAttributionResult,
  GridCarinoLinkedResult,
  LinkedReturn,
  MaterializationReport,
  PortfolioAttribution,
  PortfolioCashflows,
  PortfolioMetrics,
  PositionChange,
  PositionMetrics,
  PositionSpec,
  ReconciliationReport,
  ScenarioPnlBatchItem,
  ScenarioPnlView,
  SensitivityMatrixJson,
  StressResult,
  TradeSpec,
  WeightAllocationResult,
  WeightAllocationSpec,
  WhatIfResult,
} from './types/generated/portfolio/index.js';
import type {
  ApplicationEnvelope,
  ApplicationReport,
  HorizonResult,
  HorizonSummary,
  OperationSpec,
  RollForwardReport,
  ScenarioChangeManifest,
  ScenarioSpec,
  TemplateMetadata,
  Warning,
} from './types/generated/scenarios/index.js';

export type {
  CalibrationEnvelope,
  CalibrationResultEnvelope,
  CalibrationValidationReport,
  StrictLoadDiagnostic,
};
export type {
  CalibrationPlan,
  CalibrationReport,
  CalibrationResult,
  CalibrationStep,
  MarketDatum,
  PriorMarketObject,
  StepParams,
} from './types/generated/calibration/index.js';
export type { TableColumn, TableColumnData, TableEnvelope };
export type { DrawdownEpisode, LookbackReturns, PeriodicReturn };
export type { CovenantReport };
export type { VmResult, XvaResult };
export type {
  ArbitrageValidationResult,
  BsGreeks,
  BumpSizeConfig,
  ExchangeOfferAnalysis,
  FactorContribution,
  FactorCovarianceMatrix,
  FactorDefinition,
  FactorModelConfig,
  FactorType,
  ForwardGreeks,
  ImpactEstimate,
  LeverageImpact,
  LmeAnalysis,
  LvarBangiaScalar,
  MoneyEstimate,
  ParametricEsDecompositionView,
  PositionEsContribution,
  PositionEsContributionView,
  PositionFactorContribution,
  PositionRiskDecomposition,
  PositionVarContribution,
  RiskDecomposition,
  RiskMeasure,
  SviParams,
  TrancheLossStatistics,
  VolatilityConvention,
};
export type {
  BorrowingBaseReport,
  CompositeExposureReport,
  CompositeHistoryRow,
  CompositeRebalanceResult,
  CompositeTrade,
  EnhancedMonteCarloResult,
  ListedProductCoverage,
  MetricMetadata,
  OasResult,
  PrimitiveAggregate,
  PrimitiveExposure,
  ResultsMeta,
  ScenarioCell,
  ScenarioTable,
  TrancheMetrics,
  ValidationReport,
};
export type { Diagnostic } from './types/generated/valuations/index.js';
export type { AttributionResultEnvelope, PnlAttribution };
export type {
  CheckCategory,
  CheckFinding,
  CheckReport,
  CheckResult,
  CheckSeverity,
  CheckSummary,
  Materiality,
  MonteCarloResults,
  StatementResult,
};
export type {
  CreditAssessment,
  DcfSensitivityResult,
  DependencyTree,
  DimensionScore,
  GoalSeekResult,
  LboResult,
  PeerStats,
  RegressionResult,
  RelativeValueResult,
  ScenarioResults,
  SensitivityResult,
  TornadoEntry,
  VarianceReport,
};
export type {
  AggregatedMetric,
  BrinsonPeriodResult,
  CarinoLinkedAttribution,
  CreditVolReport,
  DurationCellTable,
  Entity,
  ExcessReturnResult,
  FactorBrinsonResult,
  FactorPnlProfile,
  FiAttributionResult,
  FiCarinoLinkedResult,
  FiReconciliationReport,
  GridAttributionResult,
  GridCarinoLinkedResult,
  LinkedReturn,
  MaterializationReport,
  PortfolioAttribution,
  PortfolioCashflows,
  PortfolioMetrics,
  PositionChange,
  PositionMetrics,
  PositionSpec,
  ReconciliationReport,
  ScenarioPnlBatchItem,
  ScenarioPnlView,
  SensitivityMatrixJson,
  StressResult,
  TradeSpec,
  WeightAllocationResult,
  WeightAllocationSpec,
  WhatIfResult,
};
export type { MaterializationPhases } from './types/generated/portfolio/index.js';
export type {
  ApplicationEnvelope,
  ApplicationReport,
  HorizonResult,
  HorizonSummary,
  OperationSpec,
  RollForwardReport,
  ScenarioChangeManifest,
  ScenarioSpec,
  TemplateMetadata,
  Warning,
};

/**
 * JSON sentinel strings (`"nan"`, `"inf"`, `"-inf"`) that the Rust wire uses for non-finite floats.
 */
export type NonFiniteSentinel = generated.analytics.NonFiniteSentinel;

/**
 * A generated result type whose top-level non-finite fields arrive as JavaScript numbers.
 *
 * The Rust type names those fields (`NonFiniteFields`); the binding turns their
 * JSON sentinel strings back into `NaN` / `±Infinity`, so the sentinel strings
 * never appear at runtime.
 */
export type NonFiniteNumbers<T> = { [K in keyof T]: Exclude<T[K], NonFiniteSentinel> };

/**
 * A generated type with some fields retyped for their JavaScript form; every other field, and
 * whether each field is optional, is unchanged.
 */
export type WithFields<T, R> = { [K in keyof T]: K extends keyof R ? R[K] : T[K] };

/**
 * Exact-decimal money `{amount, currency}` (Rust `Money` serde form): `amount` is a decimal string.
 *
 * Named `MoneyValue` in JavaScript because `Money` is the wasm-bindgen class.
 */
export type MoneyValue = generated.core.Money;

/**
 * Aggregate statistics for grouped periodic returns (Rust `PeriodStats`); non-finite ratios are numbers.
 */
export type PeriodStats = NonFiniteNumbers<generated.analytics.PeriodStats>;

/**
 * OLS beta with standard error and confidence interval (Rust `BetaResult`); non-finite values are numbers.
 */
export type BetaResult = NonFiniteNumbers<generated.analytics.BetaResult>;

/**
 * Benchmark regression alpha, beta and R² (Rust `GreeksResult`); non-finite values are numbers.
 */
export type GreeksResult = NonFiniteNumbers<generated.analytics.GreeksResult>;

/**
 * Multi-factor regression result (Rust `MultiFactorResult`); non-finite fit statistics are numbers.
 */
export type MultiFactorResult = NonFiniteNumbers<generated.analytics.MultiFactorResult>;

/**
 * Forecast accuracy metrics (Rust `ForecastMetrics`); a non-finite metric is `NaN` or `±Infinity`.
 */
export type ForecastMetrics = NonFiniteNumbers<generated.statements_analytics.ForecastMetrics>;

/**
 * One component of a formula explanation (Rust `ExplanationStep`); a non-finite `value` is a number.
 */
export type ExplanationStep = NonFiniteNumbers<generated.statements_analytics.ExplanationStep>;

/**
 * Structured formula explanation (Rust `Explanation`); non-finite values are numbers.
 */
export type Explanation = WithFields<
  NonFiniteNumbers<generated.statements_analytics.Explanation>,
  { breakdown: ExplanationStep[] }
>;

/**
 * One position's risk-budget row (Rust `PositionBudgetEntry`); a non-finite `utilization` is a number.
 */
export type PositionBudgetEntry = NonFiniteNumbers<generated.models.PositionBudgetEntry>;

/**
 * Per-position risk-budget evaluation (Rust `RiskBudgetResult`).
 */
export type RiskBudgetResult = WithFields<
  generated.models.RiskBudgetResult,
  { positions: PositionBudgetEntry[] }
>;

type HostValuationResult = import('./types/valuation-result.js').ValuationResult;

/**
 * Valuation envelope returned by the `priceInstrument*` entry points (Rust `ValuationResult`).
 *
 * Generated from the host valuation-result schema: 64-bit integers (Monte Carlo
 * seeds, path counts) are lossless `bigint`s, so serialize stochastic results
 * with `valuations.valuationResultToJson` rather than `JSON.stringify`. The
 * emitted-field presence is stricter than the schema's input view: `covenants`
 * is always present (`null` when the instrument has none), while `details` and
 * `explanation` are omitted rather than `null` when absent.
 */
export type ValuationResult = Omit<HostValuationResult, 'covenants' | 'details' | 'explanation'> & {
  covenants: NonNullable<HostValuationResult['covenants']> | null;
  details?: NonNullable<HostValuationResult['details']>;
  explanation?: NonNullable<HostValuationResult['explanation']>;
};

/**
 * One position's valuation (Rust `PositionValue`), with its embedded `ValuationResult` in host form.
 */
export type PositionValue = WithFields<
  generated.portfolio.PositionValue,
  { valuation_result?: ValuationResult | null }
>;

/**
 * Portfolio valuation (Rust `PortfolioValuation`), with each position's valuation in host form.
 */
export type PortfolioValuation = WithFields<
  generated.portfolio.PortfolioValuation,
  { position_values: { [positionId: string]: PositionValue } }
>;

/**
 * Stressed portfolio valuation plus the scenario report (Rust `ScenarioRevalueView`).
 */
export type ScenarioRevalueView = WithFields<
  generated.portfolio.ScenarioRevalueView,
  { valuation: PortfolioValuation }
>;

/**
 * One historical replay step (Rust `ReplayStep`), with its valuation in host form.
 */
export type ReplayStep = WithFields<
  generated.portfolio.ReplayStep,
  { valuation: PortfolioValuation }
>;

/**
 * Historical portfolio replay (Rust `ReplayResult`), with each step's valuation in host form.
 */
export type ReplayResult = WithFields<generated.portfolio.ReplayResult, { steps: ReplayStep[] }>;

/**
 * Solution of a portfolio optimization problem (Rust `PortfolioOptimizationResult`, serialized
 * through its `PortfolioOptimizationResultWire` form).
 */
export type PortfolioOptimizationResult = generated.portfolio.PortfolioOptimizationResultWire;

/**
 * A dated amount (Rust `DatedFlowJson`); in `cashflows.scheduleOutstandingByDate`
 * the amount is the outstanding balance after that date's flows.
 */
export interface DatedFlowJson {
  /**
   * ISO-8601 date at which the requested value or market quote applies.
   */
  date: string;
  /**
   * Amount with its currency.
   */
  amount: MoneyValue;
}

/**
 * Calendar-year cashflow totals (Rust `CalendarYearLadderRow`).
 */
export interface CalendarYearLadderRow {
  /**
   * Calendar year of the grouped flows.
   */
  year: number;
  /**
   * Sum of non-principal amounts (interest, fees, recovery, ...) in that year.
   */
  non_principal: number;
  /**
   * Sum of principal-like amounts in that year.
   */
  principal: number;
  /**
   * Sum of present values in that year.
   */
  pv: number;
}

/**
 * Projected facility and residual cashflows of an asset-backed facility
 * (Rust `FacilityProjection`).
 */
export interface FacilityProjection {
  /**
   * Interest and principal paid to the facility note.
   */
  facility: Record<string, unknown>;
  /**
   * Cash paid to the residual class.
   */
  residual: Record<string, unknown>;
  /**
   * Commitment fees as `[isoDate, Money]` pairs.
   */
  commitment_fees: [string, MoneyValue][];
  /**
   * Lender draws (scheduled draws and re-advances) as `[isoDate, Money]` pairs.
   */
  draws: [string, MoneyValue][];
  /**
   * Per-period record of the synthetic structured-credit deal.
   */
  diagnostics: Record<string, unknown>;
}

/**
 * Return-contribution attribution result (Rust `ReturnContributionResult`).
 */
export interface ReturnContributionResult {
  /**
   * Total portfolio return, equal to the summed instrument contributions.
   */
  portfolio_return: number;
  /**
   * Per-instrument contribution rows.
   */
  instrument_contribution: Record<string, unknown>[];
  /**
   * Contribution rows by group dimension.
   */
  group_contribution: Record<string, Record<string, unknown>[]>;
  /**
   * Factor contribution rows.
   */
  factor_contribution: Record<string, unknown>[];
  /**
   * Idiosyncratic residual when factors are supplied.
   */
  specific_return?: number;
  /**
   * Benchmark-relative (Brinson) attribution when a benchmark is supplied.
   */
  benchmark_relative?: Record<string, unknown>;
  /**
   * Diagnostic warnings.
   */
  warnings?: string[];
}

/**
 * One decoded series entry of `portfolio.portfolioMetricsSeries`.
 */
export interface PortfolioMetricSeriesEntry {
  /**
   * Decoded key components after the base metric (e.g. `["USD-OIS", "10y"]`).
   */
  components: string[];
  /**
   * Portfolio total for this entry.
   */
  total: number;
  /**
   * Totals by entity id.
   */
  by_entity: Record<string, number>;
}

/**
 * Horizon total return (Rust `HorizonReport`): the `HorizonResult` fields plus its derived `summary`.
 */
export type HorizonReport = HorizonResult & { summary: HorizonSummary };

// wasm-bindgen handle classes are reached through their namespace (for example
// `valuations.instruments.Bond`); the package root exports their types only.
export type {
  CreditCalibrator,
  CreditFactorModel,
  FactorCovarianceForecast,
  InstrumentArtifactCache,
  LevelsAtDate,
  Performance,
  PeriodDecomposition,
  Portfolio,
  PortfolioBuilder,
};

// --- core -----------------------------------------------------------------

/**
 * Lifecycle contract for a WebAssembly-backed value that owns a wasm heap allocation.
 */
export interface WasmOwned {
  /**
   * Release the underlying wasm heap allocation. Do not use this handle afterward.
   */
  free(): void;
}

// wasm-bindgen emits these as classes. Interface merging adds their generated
// `free()` contract without duplicating methods. At runtime, wasm-bindgen also
// installs `[Symbol.dispose]` as an alias of `free` when the host defines that
// symbol; it is intentionally omitted here so ES2020 consumers do not require
// the `esnext.disposable` TypeScript library.
/**
 * Stateful performance analytics engine over a panel of ticker price (or return) series.
 *
 * Dates are ISO-8601 values in ascending order. The `prices` and `returns`
 * supplied to the panel constructors are ticker-major and column-oriented;
 * matrix inputs to other methods follow their parameter documentation. Scalar
 * rates and returns use decimal fractions; numeric outputs are Float64Array
 * values in ticker order unless the method documents an object or matrix shape.
 *
 * Invalid dates, shapes, frequencies, tickers, and confidence levels are
 * returned as rejected JsValue errors.
 */
interface Performance extends WasmOwned {}
/**
 * Calibrated credit factor hierarchy artifact.
 *
 * Produced by [`JsCreditCalibrator`] or loaded from JSON via
 * [`JsCreditFactorModel::from_json`]. Immutable once constructed.
 */
interface CreditFactorModel extends WasmOwned {}
/**
 * Deterministic calibrator that produces a [`JsCreditFactorModel`].
 *
 * Configuration and inputs are passed as JSON strings.
 */
interface CreditCalibrator extends WasmOwned {}
/**
 * Snapshot of all hierarchy-level factor values at a single date.
 *
 * Produced by [`decompose_levels`]. Pass to [`decompose_period`] to compute
 * period-over-period changes.  The full data is available via `toJson`.
 */
interface LevelsAtDate extends WasmOwned {}
/**
 * Component-wise difference between two [`JsLevelsAtDate`] snapshots.
 *
 * Produced by [`decompose_period`].
 */
interface PeriodDecomposition extends WasmOwned {}
/**
 * Vol-forecast view over a calibrated `CreditFactorModel`.
 *
 * `VolHorizon::Custom` is intentionally **not** exposed.
 */
interface FactorCovarianceForecast extends WasmOwned {}
/**
 * Handle to a built [`finstack_quant_portfolio::Portfolio`] that can be reused
 * across WASM calls without re-parsing and rebuilding from the spec.
 *
 * `Portfolio::from_spec` parses positions, builds indices, and validates
 * invariants; for pipelines that call both `valuePortfolio` and
 * `aggregateFullCashflows` on the same portfolio, holding this handle
 * avoids paying that cost twice.
 */
interface Portfolio extends WasmOwned {}

/**
 * ISO-4217 currency code wrapper for JavaScript.
 *
 * Currencies parse from three-letter alphabetic codes (case-insensitive).
 * They expose the alphabetic code, the ISO numeric code, and the number of
 * decimal places (minor units) for the currency.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * usd.code;     // "USD"
 * usd.numeric;  // 840
 * usd.decimals; // 2
 * ```
 */
export interface Currency extends WasmOwned {
  /**
   * Three-letter ISO-4217 alphabetic code.
   *
   * @returns The uppercase alphabetic code (e.g. `"USD"`).
   */
  readonly code: string;
  /**
   * ISO-4217 numeric code.
   *
   * @returns Numeric code (e.g. `840` for USD, `978` for EUR).
   */
  readonly numeric: number;
  /**
   * Number of decimal places (minor units) for this currency.
   *
   * @returns Decimal-place count (e.g. `2` for USD, `0` for JPY).
   */
  readonly decimals: number;
  /**
   * Human-readable code (same as `code`).
   *
   * @returns The uppercase alphabetic ISO-4217 code.
   */
  toString(): string;
  /**
   * Serialize to a JSON string.
   *
   * @returns A JSON string (the ISO-4217 alphabetic code in quotes).
   * @throws If serialization fails (should not happen for valid `Currency`).
   */
  toJson(): string;
}

/**
 * ISO-4217 currency code wrapper for JavaScript.
 *
 * Currencies parse from three-letter alphabetic codes (case-insensitive).
 * They expose the alphabetic code, the ISO numeric code, and the number of
 * decimal places (minor units) for the currency.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * usd.code;     // "USD"
 * usd.numeric;  // 840
 * usd.decimals; // 2
 * ```
 */
export interface CurrencyConstructor {
  /**
   * Parse a case-insensitive ISO-4217 alphabetic currency code.
   *
   * @example
   * ```javascript
   * const eur = new core.Currency("eur"); // case-insensitive
   * eur.code; // "EUR"
   * ```
   * @param code - Three-letter ISO-4217 code (e.g. `"USD"`, `"eur"`, `"GBP"`). Case-insensitive; surrounding whitespace is not trimmed.
   * @returns Constructed `Currency`.
   * @throws `TypeError` (kind `invalid_type`) if `code` is not a string; `FinstackError` (kind `validation`) naming the rejected text if it is not a supported ISO-4217 alphabetic code (e.g. `" USD "`).
   */
  new (code: string): Currency;
  /**
   * Deserialize from a JSON string produced by `Currency.toJson`.
   *
   * @param json - A JSON string containing a quoted ISO-4217 code.
   * @returns The parsed `Currency`.
   * @throws If `json` is malformed or contains an unknown code.
   */
  fromJson(json: JsonInput): Currency;
}

/**
 * Currency-tagged monetary amount.
 *
 * Money values pin a numeric amount to a [`JsCurrency`]. The arithmetic
 * methods carry the Rust names: `checkedAdd` / `checkedSub` refuse to mix
 * currencies; `checkedMulF64` / `checkedDivF64` scale by a number and keep
 * the currency; `checkedNeg` negates exactly.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * const total = new core.Money(1_000_000, usd);
 * const fee   = new core.Money(50, usd);
 * const net   = total.checkedSub(fee);          // Money { amount: 999950, currency: USD }
 * const tax   = net.checkedMulF64(0.07);        // 7% of net
 * console.log(net.toString(), tax.toString());  // "USD 999950.00", "USD 69996.50"
 * ```
 */
export interface Money extends WasmOwned {
  /**
   * Numeric amount in major units as `f64`.
   *
   * The Rust core stores money as `Decimal`; this getter exposes the finite
   * JavaScript number view for interop.
   *
   * @returns Amount in major units (e.g. dollars, not cents).
   */
  readonly amount: number;
  /**
   * Currency of this amount.
   *
   * @returns The [`JsCurrency`] this amount is tagged with.
   */
  readonly currency: Currency;
  /**
   * Lossless amount as a decimal string (e.g. `"1234.56"`).
   *
   * Renders the internal Rust `Decimal` directly, so no `f64` round-trip
   * occurs. Parse with a JavaScript decimal library for exact arithmetic.
   *
   * @returns The exact decimal amount as a string.
   */
  readonly amountDecimal: string;
  /**
   * Convert using an already-resolved positive FX rate.
   * @returns Converted `Money` amount in the target currency.
   * @param target - Target Currency for the converted monetary amount.
   * @param rate - FX conversion rate expressed as target-currency units per source-currency unit.
   * @throws Error - For a different target currency, throws a JavaScript exception if `rate` is non-finite or not strictly positive, or if the converted amount cannot be represented as a decimal.
   */
  convertAtRate(target: Currency, rate: number): Money;
  /**
   * Add two amounts (Rust `Money::checked_add`).
   *
   * @example
   * ```javascript
   * const usd = new core.Currency("USD");
   * const a = new core.Money(10, usd);
   * const b = new core.Money(5, usd);
   * a.checkedAdd(b).amount;  // 15
   * ```
   * @param other - Another `Money` value.
   * @returns Sum, in the same currency.
   * @throws If `other.currency` differs from `this.currency`, or the operation is not representable as a `Decimal`.
   */
  checkedAdd(other: Money): Money;
  /**
   * Subtract two amounts (Rust `Money::checked_sub`).
   *
   * @param other - Another `Money` value.
   * @returns Difference, in the same currency.
   * @throws If `other.currency` differs from `this.currency`, or the operation is not representable as a `Decimal`.
   */
  checkedSub(other: Money): Money;
  /**
   * Multiply by a number (Rust `Money::checked_mul_f64`).
   *
   * @param factor - Dimensionless multiplier (must be finite).
   * @returns Scaled amount, in the same currency.
   * @throws If `factor` is non-finite or the result is not representable.
   */
  checkedMulF64(factor: number): Money;
  /**
   * Divide by a number (Rust `Money::checked_div_f64`).
   *
   * @param divisor - Dimensionless divisor (must be finite and non-zero).
   * @returns Scaled amount, in the same currency.
   * @throws If `divisor` is zero, non-finite, or the result is not representable.
   */
  checkedDivF64(divisor: number): Money;
  /**
   * Negate the monetary amount (Rust `Money::checked_neg`).
   *
   * Negation is exact on the stored `Decimal`, keeps its scale and never
   * passes through `f64`, so it cannot fail.
   *
   * @returns Negated amount in the same currency.
   */
  checkedNeg(): Money;
  /**
   * Format the amount with explicit display options.
   *
   * `decimals` accepts `null`/`undefined` (currency ISO minor units) or a
   * non-negative integer up to 1,000,000. `group` is an optional
   * single-character thousands separator (e.g. `","`); `null`/`undefined`
   * disables grouping. `rounding` accepts the canonical mode names
   * (`"bankers"`, `"away_from_zero"`, `"toward_zero"`, `"floor"`,
   * `"ceil"`); `null`/`undefined` selects bankers rounding. Formatting
   * never mutates the stored amount.
   *
   * @example
   * ```javascript
   * const usd = new core.Currency("USD");
   * const m = core.Money.fromDecimalStr("1234.567", usd);
   * try {
   *   m.formatWith(2, true, ",", "bankers");  // "USD 1,234.57"
   * } finally {
   *   m.free();
   *   usd.free();
   * }
   * ```
   * @param decimals - JavaScript number of fractional digits, an integer in `0..=1_000_000`; null or undefined selects ISO minor units.
   * @param showCurrency - Whether to prepend the ISO code; omitted means true.
   * @param group - Optional single-character thousands separator; omitted means no grouping.
   * @param rounding - Canonical lowercase rounding mode; omitted selects the Rust default (bankers).
   * @returns Formatted amount such as `"USD 1,234.57"`.
   * @throws If `decimals` is not a non-negative integer or exceeds 1,000,000, if `group` is not a single character, or if `rounding` is not a recognised mode name.
   */
  formatWith(
    decimals?: number | null,
    showCurrency?: boolean | null,
    group?: string | null,
    rounding?: string | null
  ): string;
  /**
   * Default string representation (e.g. `"USD 10.00"`).
   *
   * @returns Formatted amount with currency code.
   */
  toString(): string;
  /**
   * Serialize to a JSON string using the canonical Rust serde schema.
   *
   * @returns A JSON string carrying the exact decimal amount and the ISO-4217 currency code.
   * @throws If serialization fails (should not happen for valid `Money`).
   */
  toJson(): string;
}

/**
 * Currency-tagged monetary amount.
 *
 * Money values pin a numeric amount to a [`JsCurrency`]. The arithmetic
 * methods carry the Rust names: `checkedAdd` / `checkedSub` refuse to mix
 * currencies; `checkedMulF64` / `checkedDivF64` scale by a number and keep
 * the currency; `checkedNeg` negates exactly.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * const total = new core.Money(1_000_000, usd);
 * const fee   = new core.Money(50, usd);
 * const net   = total.checkedSub(fee);          // Money { amount: 999950, currency: USD }
 * const tax   = net.checkedMulF64(0.07);        // 7% of net
 * console.log(net.toString(), tax.toString());  // "USD 999950.00", "USD 69996.50"
 * ```
 */
export interface MoneyConstructor {
  /**
   * Creates a new money value without implicit currency-minor-unit rounding.
   *
   * WASM accepts a JavaScript `number` only. Its finite numeric value is
   * converted to Rust `Decimal` and stored without currency-minor-unit
   * rounding; precision already absent from the input `number` cannot be
   * recovered. Formatting does not mutate the stored amount.
   *
   * @example
   * ```javascript
   * const usd = new core.Currency("USD");
   * const m = new core.Money(1234.56, usd);
   * m.amount;          // 1234.56
   * m.currency.code;   // "USD"
   * ```
   * @param amount - Numeric amount in major units (must be finite).
   * @param currency - ISO-4217 Currency object that tags the amount and controls arithmetic compatibility.
   * @returns The constructed `Money`.
   * @throws If `amount` is non-finite (NaN, ±∞) or cannot be represented as a `Decimal`.
   */
  new (amount: number, currency: Currency): Money;
  /**
   * Deserialize from a JSON string produced by `Money.toJson`.
   *
   * @param json - A JSON string in the canonical Rust `Money` schema.
   * @returns The parsed `Money`.
   * @throws If `json` is malformed or fails strict schema validation.
   */
  fromJson(json: JsonInput): Money;
  /**
   * Construct from exact decimal text, rejecting inexact amounts.
   *
   * `amount` accepts fixed-point (`"1234.56"`) or scientific (`"1.2345e3"`)
   * text and must be exactly representable as a Rust `Decimal` (96-bit
   * mantissa, up to 28 fractional digits; the scientific mantissa must
   * itself fit exactly). Inexact or underflowing amounts throw rather than
   * being silently rounded, and the text never passes through `f64`. The
   * currency is a tag only — no FX conversion is performed.
   *
   * @example
   * ```javascript
   * const usd = new core.Currency("USD");
   * const m = core.Money.fromDecimalStr("1.245", usd);
   * try {
   *   m.amountDecimal;  // "1.245"
   * } finally {
   *   m.free();
   *   usd.free();
   * }
   * ```
   * @param amount - Exact fixed-point or scientific decimal text in major currency units.
   * @param currency - ISO-4217 `Currency` object that tags the amount, as for the `Money` constructor (build one with `new Currency(code)`).
   * @returns The constructed `Money`.
   * @throws If `amount` is malformed, non-finite, needs more precision than `Decimal` can hold, or underflows its supported scale; or if `currency` is not a `Currency` object.
   */
  fromDecimalStr(amount: string, currency: Currency): Money;
}

/**
 * Interest or discount rate stored as a decimal (e.g. `0.05` is 5%).
 *
 * Conventions:
 * - **Decimal**: `0.05` represents 5%.
 * - **Percent**: `5.0` represents 5%.
 * - **Basis points**: `500` represents 5% (1 bp = 0.01%).
 *
 * Use the `fromPercent` or `fromBp` factories to avoid scaling errors
 * when working with quoted rates.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const r = core.Rate.fromBp(250);     // 2.5% as 250 bp
 * r.asDecimal;  // 0.025
 * r.asPercent;  // 2.5
 * r.asBp;      // 250
 * ```
 */
export interface Rate extends WasmOwned {
  /**
   * Rate as a decimal (e.g. `0.05` for 5%).
   *
   * @returns Decimal rate.
   */
  readonly asDecimal: number;
  /**
   * Rate as a percent (e.g. `5.0` for 5%).
   *
   * @returns Percent rate.
   */
  readonly asPercent: number;
  /**
   * Rate in basis points, rounded to the nearest integer (e.g. `500` for 5%).
   *
   * @returns Rate in bp.
   */
  readonly asBp: number;
  /**
   * The rate as `Bps`, rounded to the nearest whole basis point.
   */
  readonly asBasisPoints: Bps;
  /**
   * The rate as a `Percentage`.
   */
  readonly asPercentage: Percentage;
  /**
   * Absolute value (Rust `Rate::abs`).
   *
   * @returns A new `Rate` with the sign removed.
   */
  abs(): Rate;
  /**
   * Whether the value is exactly zero.
   *
   * @returns `true` when the rate is zero.
   */
  isZero(): boolean;
  /**
   * Whether the value is strictly positive.
   *
   * @returns `true` when the rate is above zero.
   */
  isPositive(): boolean;
  /**
   * Whether the value is strictly negative.
   *
   * @returns `true` when the rate is below zero.
   */
  isNegative(): boolean;
  /**
   * Serialize to the canonical JSON wire form shared with Python `Rate.to_json`.
   *
   * @returns Compact JSON text.
   * @throws If serialization fails (not expected for a valid value).
   */
  toJson(): string;
}

/**
 * Interest or discount rate stored as a decimal (e.g. `0.05` is 5%).
 *
 * Conventions:
 * - **Decimal**: `0.05` represents 5%.
 * - **Percent**: `5.0` represents 5%.
 * - **Basis points**: `500` represents 5% (1 bp = 0.01%).
 *
 * Use the `fromPercent` or `fromBp` factories to avoid scaling errors
 * when working with quoted rates.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const r = core.Rate.fromBp(250);     // 2.5% as 250 bp
 * r.asDecimal;  // 0.025
 * r.asPercent;  // 2.5
 * r.asBp;      // 250
 * ```
 */
export interface RateConstructor {
  /**
   * Create a rate from a decimal value.
   *
   * @example
   * ```javascript
   * const r = new core.Rate(0.05);  // 5%
   * r.asPercent;  // 5
   * ```
   * @param decimal - Rate as a decimal (e.g. `0.05` for 5%).
   * @returns The constructed `Rate`.
   * @throws If `decimal` is non-finite (NaN, ±∞).
   */
  new (decimal: number): Rate;
  /**
   * Create a rate from a percent figure.
   *
   * @example
   * ```javascript
   * const r = core.Rate.fromPercent(5.0);
   * r.asDecimal;  // 0.05
   * ```
   * @param percent - Percent value (e.g. `5.0` for 5%).
   * @returns The constructed `Rate`.
   * @throws If `percent` is non-finite.
   */
  fromPercent(percent: number): Rate;
  /**
   * Create a rate from a whole number of basis points.
   *
   * The canonical Rust `Rate::from_bp` takes an integer (`i32`) number
   * of basis points. Because JavaScript numbers are `f64`, this binding
   * accepts a float but **rejects fractional input** rather than
   * silently rounding it: a sub-bp rate quietly rounded to whole bp is a
   * pricing bug, not a convenience. Use `new Rate(decimal)` or
   * `Rate.fromPercent` for sub-bp rates.
   *
   * @example
   * ```javascript
   * const r = core.Rate.fromBp(250);  // 2.5%
   * r.asDecimal;  // 0.025
   * ```
   * @param bp - Rate in whole basis points (e.g. `500` for 5%).
   * @returns The constructed `Rate`.
   * @throws If `bp` is non-finite or not a whole number of basis points.
   */
  fromBp(bp: number): Rate;
  /**
   * Parse a rate quote through Rust `Rate::from_str` (the twin of Python `Rate(text)`).
   *
   * @example
   * ```javascript
   * core.Rate.parse("12.5bp").asDecimal;  // 0.00125
   * ```
   * @param text - Quote text: a decimal (`"0.05"`), a percent (`"5%"`) or basis points (`"25bp"`, fractional bp allowed).
   * @returns The parsed `Rate`.
   * @throws `TypeError` if `text` is not a string; `FinstackError` (kind `validation`) if it is not a recognised rate quote.
   */
  parse(text: string): Rate;
  /**
   * Deserialize from the canonical JSON wire form produced by `toJson`.
   *
   * @param json - Canonical `Rate` JSON text (or the equivalent plain value).
   * @returns The parsed `Rate`.
   * @throws If `json` is malformed or holds an invalid value.
   */
  fromJson(json: JsonInput): Rate;
}

/**
 * Basis points (1 bp = 0.01%, 10_000 bp = 100%).
 *
 * Stored as integer bp internally; constructors reject fractional input.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const spread = new core.Bps(125);
 * spread.asDecimal;  // 0.0125
 * spread.asBp;       // 125
 * spread.asRate.asPercent;  // 1.25
 * ```
 */
export interface Bps extends WasmOwned {
  /**
   * Value as a decimal (e.g. 25 bp → 0.0025).
   *
   * @returns Decimal equivalent.
   */
  readonly asDecimal: number;
  /**
   * Value in whole basis points.
   *
   * @returns Integer bp.
   */
  readonly asBp: number;
  /**
   * Value in percent (e.g. 25 bp → 0.25).
   */
  readonly asPercent: number;
  /**
   * Value as a decimal `Rate`.
   */
  readonly asRate: Rate;
  /**
   * Value as a `Percentage`.
   */
  readonly asPercentage: Percentage;
  /**
   * Absolute value (Rust `Bps::abs`).
   *
   * @returns A new `Bps` with the sign removed.
   */
  abs(): Bps;
  /**
   * Whether the value is exactly zero.
   *
   * @returns `true` when the basis-point value is zero.
   */
  isZero(): boolean;
  /**
   * Whether the value is strictly positive.
   *
   * @returns `true` when the basis-point value is above zero.
   */
  isPositive(): boolean;
  /**
   * Whether the value is strictly negative.
   *
   * @returns `true` when the basis-point value is below zero.
   */
  isNegative(): boolean;
  /**
   * Serialize to the canonical JSON wire form shared with Python `Bps.to_json`.
   *
   * @returns Compact JSON text.
   * @throws If serialization fails (not expected for a valid value).
   */
  toJson(): string;
}

/**
 * Basis points (1 bp = 0.01%, 10_000 bp = 100%).
 *
 * Stored as integer bp internally; constructors reject fractional input.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const spread = new core.Bps(125);
 * spread.asDecimal;  // 0.0125
 * spread.asBp;       // 125
 * spread.asRate.asPercent;  // 1.25
 * ```
 */
export interface BpsConstructor {
  /**
   * Create basis points from a whole-number value.
   *
   * @param bp - Value in whole basis points (e.g. `25` for 25 bp).
   * @returns The constructed `Bps`.
   * @throws If `bp` is non-finite or not a whole number of basis points. Sub-bp spreads must use the JSON instrument path (which preserves fractional values) or a decimal `Rate`.
   */
  new (bp: number): Bps;
  /**
   * Deserialize from the canonical JSON wire form produced by `toJson`.
   *
   * @param json - Canonical `Bps` JSON text (or the equivalent plain value).
   * @returns The parsed `Bps`.
   * @throws If `json` is malformed or holds an invalid value.
   */
  fromJson(json: JsonInput): Bps;
}

/**
 * Percentage stored in percent points (`5.0` means 5%).
 *
 * Use this when you want the API to be explicit that the value is in
 * percent (rather than decimal). Equivalent to `Rate` for arithmetic.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const p = new core.Percentage(5.0);
 * p.asDecimal;  // 0.05
 * p.asPercent;  // 5
 * p.asBp;       // 500
 * ```
 */
export interface Percentage extends WasmOwned {
  /**
   * Value as a decimal (5% → 0.05).
   *
   * @returns Decimal equivalent.
   */
  readonly asDecimal: number;
  /**
   * Value in percent points.
   *
   * @returns Percent value.
   */
  readonly asPercent: number;
  /**
   * Value in basis points, rounded to the nearest integer (e.g. 17.5% → 1750).
   */
  readonly asBp: number;
  /**
   * Value as a decimal `Rate`.
   */
  readonly asRate: Rate;
  /**
   * Value as `Bps`, rounded to the nearest whole basis point.
   */
  readonly asBasisPoints: Bps;
  /**
   * Absolute value (Rust `Percentage::abs`).
   *
   * @returns A new `Percentage` with the sign removed.
   */
  abs(): Percentage;
  /**
   * Whether the value is exactly zero.
   *
   * @returns `true` when the percentage is zero.
   */
  isZero(): boolean;
  /**
   * Whether the value is strictly positive.
   *
   * @returns `true` when the percentage is above zero.
   */
  isPositive(): boolean;
  /**
   * Whether the value is strictly negative.
   *
   * @returns `true` when the percentage is below zero.
   */
  isNegative(): boolean;
  /**
   * Serialize to the canonical JSON wire form shared with Python `Percentage.to_json`.
   *
   * @returns Compact JSON text.
   * @throws If serialization fails (not expected for a valid value).
   */
  toJson(): string;
}

/**
 * Percentage stored in percent points (`5.0` means 5%).
 *
 * Use this when you want the API to be explicit that the value is in
 * percent (rather than decimal). Equivalent to `Rate` for arithmetic.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const p = new core.Percentage(5.0);
 * p.asDecimal;  // 0.05
 * p.asPercent;  // 5
 * p.asBp;       // 500
 * ```
 */
export interface PercentageConstructor {
  /**
   * Create a percentage.
   *
   * @param percent - Value in percent (e.g. `5.0` for 5%).
   * @returns The constructed `Percentage`.
   * @throws If `percent` is non-finite.
   */
  new (percent: number): Percentage;
  /**
   * Deserialize from the canonical JSON wire form produced by `toJson`.
   *
   * @param json - Canonical `Percentage` JSON text (or the equivalent plain value).
   * @returns The parsed `Percentage`.
   * @throws If `json` is malformed or holds an invalid value.
   */
  fromJson(json: JsonInput): Percentage;
}

/**
 * Day-count convention for computing year fractions and day counts.
 *
 * Dates are represented as **epoch days** (`i32`, days since 1970-01-01).
 * Use `createDate` to convert from a `(year, month, day)` triple.
 *
 * Available conventions (canonical names for `DayCount.fromName`) and their factories:
 * - `one_one` → `DayCount.oneOne`
 * - `act_360` → `DayCount.act360`
 * - `act_365f` → `DayCount.act365f`
 * - `act_365l` → `DayCount.act365l`
 * - `nl_365` (No-Leap/365) → `DayCount.nl365`
 * - `30_360` → `DayCount.thirty360`
 * - `30e_360` → `DayCount.thirtyE360`
 * - `30e_360_isda` → `DayCount.thirtyE360Isda`
 * - `act_act` (ISDA) → `DayCount.actAct`
 * - `act_act_isma` (ICMA) → `DayCount.actActIsma`
 * - `act_act_afb` (AFB / Actual/Actual Euro) → `DayCount.actActAfb`
 * - `30_360_it` (Italian) → `DayCount.thirty360It`
 * - `bus_252` → `DayCount.bus252`
 *
 * Term-sheet spellings such as `"ACT/360"` or `"30/360 ISDA"` go through the
 * lenient `DayCount.parse`.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const day_count = core.DayCount.act365f();
 * const start = core.createDate(2025, 1, 15);
 * const end   = core.createDate(2025, 7, 15);
 * const yf    = day_count.yearFraction(start, end);
 * // yf ≈ 0.4959 (181 / 365)
 * ```
 */
export interface DayCount extends WasmOwned {
  /**
   * Compute the year fraction between two dates given as epoch days
   * (Rust `DayCount::year_fraction(start, end, ctx)`).
   *
   * An omitted `ctx` is the empty Rust default context (`new DayCountContext()`).
   * Act/Act ISMA needs a context frequency (or coupon period) and Bus/252 a
   * context calendar; both throw without them.
   *
   * @example
   * ```javascript
   * const dayCount = core.DayCount.act360();
   * const start = core.createDate(2025, 1, 15);
   * const end   = core.createDate(2025, 4, 15);
   * dayCount.yearFraction(start, end); // 90 / 360 = 0.25
   * const bus = new core.DayCountContext("nyse");
   * core.DayCount.bus252().yearFraction(start, end, bus);
   * ```
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01; must not be before the start.
   * @param ctx - DayCountContext supplying calendar, frequency, coupon-period and termination metadata; omitted means the empty default context.
   * @returns Non-negative year fraction in years under the convention and context.
   * @throws `TypeError` (kind `invalid_type`) if a date is not an integer epoch-day number; `FinstackError` (kind `validation`) if a date is out of range, the start is after the end, or the convention's required context is missing or invalid; kind `not_found` if the context names an unknown calendar.
   */
  yearFraction(startEpochDays: number, endEpochDays: number, ctx?: DayCountContext): number;
  /**
   * Compute a signed year fraction, preserving the start/end orientation
   * (Rust `DayCount::signed_year_fraction(start, end, ctx)`).
   *
   * An omitted `ctx` is the empty Rust default context, as for `yearFraction`.
   *
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01; may precede the start.
   * @param ctx - DayCountContext supplying calendar, frequency, coupon-period and termination metadata; omitted means the empty default context.
   * @returns Signed year fraction in years; negative when `end` is before `start`.
   * @throws Error - Throws `TypeError` (kind `invalid_type`) if a date is not an integer epoch-day number; `FinstackError` (kind `validation`) if a date is out of range or the convention's required context is missing or invalid; kind `not_found` if the context names an unknown calendar.
   */
  signedYearFraction(startEpochDays: number, endEpochDays: number, ctx?: DayCountContext): number;
  /**
   * Convention name.
   * @returns Human-readable string form of this value.
   */
  toString(): string;
}

/**
 * Day-count convention for computing year fractions and day counts.
 *
 * Dates are represented as **epoch days** (`i32`, days since 1970-01-01).
 * Use `createDate` to convert from a `(year, month, day)` triple.
 *
 * Available conventions (canonical names for `DayCount.fromName`) and their factories:
 * - `one_one` → `DayCount.oneOne`
 * - `act_360` → `DayCount.act360`
 * - `act_365f` → `DayCount.act365f`
 * - `act_365l` → `DayCount.act365l`
 * - `nl_365` (No-Leap/365) → `DayCount.nl365`
 * - `30_360` → `DayCount.thirty360`
 * - `30e_360` → `DayCount.thirtyE360`
 * - `30e_360_isda` → `DayCount.thirtyE360Isda`
 * - `act_act` (ISDA) → `DayCount.actAct`
 * - `act_act_isma` (ICMA) → `DayCount.actActIsma`
 * - `act_act_afb` (AFB / Actual/Actual Euro) → `DayCount.actActAfb`
 * - `30_360_it` (Italian) → `DayCount.thirty360It`
 * - `bus_252` → `DayCount.bus252`
 *
 * Term-sheet spellings such as `"ACT/360"` or `"30/360 ISDA"` go through the
 * lenient `DayCount.parse`.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const day_count = core.DayCount.act365f();
 * const start = core.createDate(2025, 1, 15);
 * const end   = core.createDate(2025, 7, 15);
 * const yf    = day_count.yearFraction(start, end);
 * // yf ≈ 0.4959 (181 / 365)
 * ```
 */
export interface DayCountConstructor {
  /**
   * JavaScript prototype of `DayCount`; instances come from `fromName`, `parse` and the named factories, not `new`.
   */
  readonly prototype: DayCount;
  /**
   * Look up a convention by its canonical name (Rust `DayCount::from_str`,
   * the twin of Python `DayCount.from_name`).
   *
   * @param name - Canonical snake_case convention name, for example `"act_360"`, `"30_360"` or `"act_act"` (see the class list).
   * @returns The matching `DayCount`.
   * @throws `TypeError` if `name` is not a string; `FinstackError` (kind `validation`) if it is not a canonical convention name. Use `DayCount.parse` for term-sheet spellings.
   */
  fromName(name: string): DayCount;
  /**
   * Parse a convention leniently (Rust `DayCount::parse`): case, spaces,
   * `/` and `-` are normalised, so term-sheet spellings such as
   * `"ACT/360"`, `"Act/Act ICMA"` or `"30E/360 ISDA"` are accepted.
   *
   * @example
   * ```javascript
   * core.DayCount.parse("Act/Act ICMA").toString();  // "act_act_isma"
   * ```
   * @param s - Convention text in canonical or term-sheet spelling.
   * @returns The matching `DayCount`.
   * @throws `TypeError` if `s` is not a string; `FinstackError` (kind `validation`) if no convention matches.
   */
  parse(s: string): DayCount;
  /**
   * No-Leap/365: actual days excluding February 29, over 365.
   * @returns A `DayCount` handle for this convention.
   */
  nl365(): DayCount;
  /**
   * Count the calendar days between two dates (epoch days), independent of the
   * convention (Rust associated fn `DayCount::calendar_days`).
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01.
   * @returns Signed calendar-day count from start to end.
   * @throws Error - Throws a JavaScript exception if either epoch-day value is outside the representable date range.
   */
  calendarDays(startEpochDays: number, endEpochDays: number): bigint;
  /**
   * Act/360 day-count convention.
   * @returns A `DayCount` handle for this convention.
   */
  act360(): DayCount;
  /**
   * One unit per nonempty contractual accrual period; empty periods return zero.
   * @returns The 1/1 convention used for annual inflation accrual periods.
   * @throws This constructor does not throw.
   */
  oneOne(): DayCount;
  /**
   * Actual/365 Fixed.
   * @returns A `DayCount` handle for this convention.
   */
  act365f(): DayCount;
  /**
   * Actual/365L (ICMA Rule 251). Annual periods (or periods without
   * frequency context) use denominator 366 exactly when February 29 falls
   * in `(start, end]`; non-annual periods use 366 exactly when the end
   * date's year is a leap year. Otherwise the denominator is 365. This is
   * not ACT/ACT AFB.
   * @returns A `DayCount` handle for this convention.
   */
  act365l(): DayCount;
  /**
   * 30/360 US (Bond Basis).
   * @returns A `DayCount` handle for this convention.
   */
  thirty360(): DayCount;
  /**
   * 30E/360 (Eurobond Basis).
   * @returns A `DayCount` handle for this convention.
   */
  thirtyE360(): DayCount;
  /**
   * 30E/360 ISDA day-count convention.
   * @returns A `DayCount` handle for this convention.
   */
  thirtyE360Isda(): DayCount;
  /**
   * Actual/Actual (ISDA).
   * @returns A `DayCount` handle for this convention.
   */
  actAct(): DayCount;
  /**
   * Actual/Actual (ICMA/ISMA).
   * @returns A `DayCount` handle for this convention.
   */
  actActIsma(): DayCount;
  /**
   * Actual/Actual AFB (Actual/Actual Euro).
   *
   * Walks whole years backwards from the end date (QuantLib
   * `ActualActual::AFB`). A year-step landing on 28 February of a leap
   * year is bumped to 29 February. The residual uses denominator 366 if
   * 29 February lies in `[start, residual_end)`, else 365.
   * @returns A `DayCount` handle for this convention.
   */
  actActAfb(): DayCount;
  /**
   * Return a `DayCount` handle configured for thirty360 it.
   *
   * Day 31 becomes 30, and any February day after the 27th becomes 30
   * (QuantLib `Thirty360::Italian`). Distinct from US SIA and 30E/360.
   * @returns A `DayCount` handle for this convention.
   */
  thirty360It(): DayCount;
  /**
   * Business/252 day-count convention.
   * @returns A `DayCount` handle for this convention.
   */
  bus252(): DayCount;
}

/**
 * Optional context for day-count conventions that need market metadata.
 */
export interface DayCountContext extends WasmOwned {
  /**
   * Holiday-calendar identifier, or `undefined`.
   */
  readonly calendarId: string | undefined;
  /**
   * Coupon frequency, or `undefined`.
   */
  readonly frequency: Tenor | undefined;
  /**
   * Custom business-day denominator, or `undefined`.
   */
  readonly busBasis: number | undefined;
  /**
   * Reference coupon period as `[startEpochDays, endEpochDays]`, or `undefined`.
   */
  readonly couponPeriod: Int32Array | undefined;
  /**
   * Whether the accrual end is the instrument termination date.
   */
  readonly endIsTerminationDate: boolean;
  /**
   * Serialize to the canonical JSON wire form shared with Python `DayCountContext.to_json`.
   *
   * @returns Compact JSON text (dates in the coupon period are ISO-8601).
   * @throws If serialization fails (not expected for a valid context).
   */
  toJson(): string;
}

/**
 * Optional context for day-count conventions that need market metadata.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const context = new core.DayCountContext("nyse", "3M", 252);
 * console.log(context.frequency?.toString()); // "3M"
 * ```
 */
export interface DayCountContextConstructor {
  /**
   * Create a day-count context (Rust `DayCountContextState::try_new`, the
   * validation point shared with Python `DayCountContext(...)`).
   *
   * Every argument is optional; `new DayCountContext()` is the empty
   * context `DayCount.yearFraction` / `signedYearFraction` use when none is
   * passed.
   *
   * @example
   * ```javascript
   * const ctx = new core.DayCountContext("nyse", "3M", 252);
   * ctx.frequency.toString();  // "3M"
   * core.DayCountContext.fromJson(ctx.toJson()).busBasis;  // 252
   * ```
   * @param calendarId - Registered holiday-calendar identifier (for example `"nyse"`) used by Bus/252; resolved when the context is used.
   * @param frequency - Coupon frequency as tenor text (for example `"6M"`; use `tenor.toString()` for a `Tenor`), required by Act/Act ICMA and used by Act/365L.
   * @param busBasis - Business-day denominator for Bus/252 (an integer in `0..=65535`, normally `252`).
   * @param couponPeriod - Reference coupon period `[startEpochDays, endEpochDays]` (days since 1970-01-01) for Act/Act ICMA; the start must precede the end.
   * @param endIsTerminationDate - Whether the accrual end is the instrument's termination date (30E/360 ISDA February-end handling); omitted means `false`.
   * @returns A new `DayCountContext`.
   * @throws `TypeError` (kind `invalid_type`) for a mistyped argument or a `couponPeriod` that is not a two-element array of epoch days; `FinstackError` (kind `validation`) if `frequency` is not a tenor or the coupon period is inverted or out of range.
   */
  new (
    calendarId?: string | null,
    frequency?: string | null,
    busBasis?: number | null,
    couponPeriod?: readonly [number, number] | null,
    endIsTerminationDate?: boolean | null
  ): DayCountContext;
  /**
   * Deserialize from the canonical JSON wire form produced by `toJson`.
   *
   * @param json - Canonical DayCountContext JSON text or plain object; unknown fields are rejected. As in Rust and Python, the coupon period is validated when the context is used (`DayCountContextState::to_ctx`).
   * @returns The parsed `DayCountContext`.
   * @throws If `json` is malformed or has unknown or mistyped fields.
   */
  fromJson(json: JsonInput): DayCountContext;
}

/**
 * A financial tenor such as `3M`, `1Y`, or `2W`.
 *
 * Tenors carry a numeric count and a unit (days, weeks, months, years).
 * Parse from strings (`new Tenor(s)` or `Tenor.parse(s)`) or use the
 * named-period factories (`Tenor.daily`, `Tenor.weekly`, `Tenor.biweekly`,
 * `Tenor.monthly`, `Tenor.bimonthly`, `Tenor.quarterly`, `Tenor.semiAnnual`,
 * `Tenor.annual`), `Tenor.fromPaymentsPerYear` or `Tenor.fromYears`.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const t = new core.Tenor("3M");
 * t.toString();        // "3M"
 * t.toYears();   // 0.25
 *
 * const annual = core.Tenor.annual();
 * annual.toString();   // "1Y"
 * ```
 */
export interface Tenor extends WasmOwned {
  /**
   * Unit count of this tenor, such as `3` for `"3M"`.
   */
  readonly count: number;
  /**
   * Unit designator: `"D"`, `"W"`, `"M"` or `"Y"` (Rust `TenorUnit::designator`).
   */
  readonly unit: string;
  /**
   * Equivalent whole months, or `undefined` for day/week tenors.
   */
  readonly months: number | undefined;
  /**
   * Equivalent whole days, or `undefined` for month/year tenors.
   */
  readonly days: number | undefined;
  /**
   * Approximate length in years (simple estimate, no calendar).
   * @returns Approximate tenor length in years, such as `0.25` for `"3M"`.
   */
  toYears(): number;
  /**
   * Coupon payments per year implied by this tenor (`3M` gives `4`, `2Y` gives `0.5`).
   * @returns Payments per year.
   */
  paymentsPerYear(): number;
  /**
   * Approximate length in calendar days (no calendar).
   *
   * @returns Whole days as a number.
   * @throws Never for a valid tenor (the Rust tenor bounds keep it within range).
   */
  toDaysApprox(): number;
  /**
   * Add this tenor to a date (Rust `Tenor::add_to_date`).
   *
   * Month and year tenors clamp to the last valid day of the target month.
   *
   * @param epochDays - Anchor date as days since 1970-01-01.
   * @param calendarCode - Registered holiday-calendar identifier used to roll the result; omitted skips adjustment.
   * @param convention - Business-day convention name applied with the calendar; omitted uses the Rust default (`"modified_following"`).
   * @returns The (optionally adjusted) end date as epoch days.
   * @throws `TypeError` for a mistyped argument; `FinstackError` if the date is out of range, the convention is unknown, the calendar is unknown (kind `not_found`), or no business day is found.
   */
  addToDate(epochDays: number, calendarCode?: string | null, convention?: string | null): number;
  /**
   * Exact year fraction of this tenor from a date under a day count
   * (Rust `Tenor::to_years_with_context`).
   *
   * @param asOfEpochDays - Start date as days since 1970-01-01.
   * @param dayCount - Convention used to measure the span.
   * @param calendarCode - Registered holiday-calendar identifier used to roll the end date; omitted skips adjustment.
   * @param convention - Business-day convention name applied with the calendar; omitted uses the Rust default (`"modified_following"`).
   * @returns Year fraction between the start and the rolled end date.
   * @throws `TypeError` for a mistyped argument; `FinstackError` if the date is out of range, the convention or calendar is unknown, or the day count needs context it cannot get (e.g. Bus/252 without a calendar).
   */
  toYearsWithContext(
    asOfEpochDays: number,
    dayCount: DayCount,
    calendarCode?: string | null,
    convention?: string | null
  ): number;
  /**
   * Tenor string representation.
   * @returns Human-readable string form of this value.
   */
  toString(): string;
}

/**
 * A financial tenor such as `3M`, `1Y`, or `2W`.
 *
 * Tenors carry a numeric count and a unit (days, weeks, months, years).
 * Parse from strings (`new Tenor(s)` or `Tenor.parse(s)`) or use the
 * named-period factories (`Tenor.daily`, `Tenor.weekly`, `Tenor.biweekly`,
 * `Tenor.monthly`, `Tenor.bimonthly`, `Tenor.quarterly`, `Tenor.semiAnnual`,
 * `Tenor.annual`), `Tenor.fromPaymentsPerYear` or `Tenor.fromYears`.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const t = new core.Tenor("3M");
 * t.toString();        // "3M"
 * t.toYears();   // 0.25
 *
 * const annual = core.Tenor.annual();
 * annual.toString();   // "1Y"
 * ```
 */
export interface TenorConstructor {
  /**
   * Parse a tenor string.
   *
   * @param s - Tenor string. Accepted forms include `"3M"`, `"1Y"`, `"2W"`, `"7D"`, `"6M"`, `"10Y"`. Whitespace is permitted.
   * @returns The parsed `Tenor`.
   * @throws If `s` cannot be parsed (unknown unit, missing count).
   */
  new (s: string): Tenor;
  /**
   * Parse a tenor string (Rust `Tenor::parse`; same as `new Tenor(s)`).
   *
   * @param s - Tenor text such as `"3M"`, `"1Y"`, `"2W"` or `"7D"`.
   * @returns The parsed `Tenor`.
   * @throws If `s` is not a string or cannot be parsed.
   */
  parse(s: string): Tenor;
  /**
   * Tenor for a year fraction under a day count (Rust `Tenor::from_years`):
   * a whole number of months when the fraction is one, otherwise days.
   *
   * @param years - Positive, finite length in years.
   * @param dayCount - Convention used to interpret `years`.
   * @returns The matching `Tenor`.
   * @throws If `years` is not finite and positive or the tenor is out of range.
   */
  fromYears(years: number, dayCount: DayCount): Tenor;
  /**
   * Tenor for a coupon frequency (Rust `Tenor::from_payments_per_year`;
   * `4` gives `3M`).
   *
   * @param payments - Coupon payments per year; must be positive and divide 12 (`12` monthly, `4` quarterly, `2` semi-annual, `1` annual).
   * @returns The matching `Tenor`.
   * @throws `TypeError` if `payments` is not a non-negative integer; `FinstackError` (kind `validation`) if no tenor matches.
   */
  fromPaymentsPerYear(payments: number): Tenor;
  /**
   * Return a `Tenor` handle configured for biweekly.
   * @returns A `Tenor` handle for this named period.
   */
  biweekly(): Tenor;
  /**
   * Return a `Tenor` handle configured for bimonthly.
   * @returns A `Tenor` handle for this named period.
   */
  bimonthly(): Tenor;
  /**
   * One-day tenor (`"1D"`).
   * @returns A `Tenor` handle for this named period.
   */
  daily(): Tenor;
  /**
   * One-week tenor (`"1W"`).
   * @returns A `Tenor` handle for this named period.
   */
  weekly(): Tenor;
  /**
   * One-month tenor (`"1M"`).
   * @returns A `Tenor` handle for this named period.
   */
  monthly(): Tenor;
  /**
   * 3-month (quarterly) tenor.
   * @returns A `Tenor` handle for this named period.
   */
  quarterly(): Tenor;
  /**
   * 6-month (semi-annual) tenor.
   * @returns A `Tenor` handle for this named period.
   */
  semiAnnual(): Tenor;
  /**
   * 12-month (annual) tenor.
   * @returns A `Tenor` handle for this named period.
   */
  annual(): Tenor;
}

/**
 * Discount-curve validation policy: market-standard or negative-rate-friendly.
 */
export type DiscountCurveValidationMode = 'market_standard' | 'negative_rate_friendly';

/**
 * Named options for constructing a `DiscountCurve`; unknown keys are rejected.
 */
export interface DiscountCurveOptions {
  /**
   * Curve identifier; the lookup key inside a `MarketContext`.
   */
  id: string;
  /**
   * ISO-8601 base date; knot times are year fractions from it under `dayCount`.
   */
  baseDate: string;
  /**
   * Flat `[t0, df0, t1, df1, …]` pairs: `t` in years, `df` strictly positive; even length.
   */
  knots: NumericArray;
  /**
   * Interpolation style; the Rust builder default is `"monotone_convex"`.
   */
  interp?: string;
  /**
   * Extrapolation policy; the Rust builder default is `"flat_forward"`.
   */
  extrapolation?: string;
  /**
   * Day-count convention for the time axis; the Rust builder default is `"act_365f"`.
   */
  dayCount?: string;
  /**
   * Validation preset; omitted means the Rust default `"market_standard"`.
   */
  validationMode?: DiscountCurveValidationMode;
  /**
   * Decimal minimum implied forward; required with `"negative_rate_friendly"`, rejected otherwise.
   */
  forwardFloor?: number | null;
}

/**
 * Discount factor curve for present-value calculations.
 *
 * Built from `(time, discount_factor)` pillars where `time` is a year
 * fraction from `baseDate` and `df` is the price today of $1 paid at that
 * time. Defaults reflect the most common practitioner convention
 * (Hagan-West monotone-convex interpolation, flat-forward extrapolation,
 * Act/365 fixed day-count).
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * // OIS-style USD curve, base-date 2025-01-02, three pillars.
 * const curve = new core.DiscountCurve({
 *   id: "USD-OIS",
 *   baseDate: "2025-01-02",
 *   knots: [0.0, 1.0, 1.0, 0.95, 5.0, 0.78],
 *   interp: "monotone_convex",
 *   extrapolation: "flat_forward",
 *   dayCount: "act_365f",
 * });
 * curve.df(2.5);          // discount factor at 2.5y
 * curve.zero(2.5);        // continuously-compounded zero rate at 2.5y
 * ```
 */
export interface DiscountCurve extends WasmOwned {
  /**
   * Curve identifier.
   */
  readonly id: string;
  /**
   * Base date as ISO string.
   */
  readonly baseDate: string;
  /**
   * Discount factor at year fraction `t`.
   * @returns Discount factor for 1 unit paid at time `t`.
   * @param t - Time from the curve base date in years.
   */
  df(t: number): number;
  /**
   * Continuously-compounded zero rate at year fraction `t`.
   * @returns Continuously compounded zero rate as a decimal, such as `0.04` for 4%.
   * @param t - Time from the curve base date in years.
   */
  zero(t: number): number;
  /**
   * Continuously-compounded forward rate between `t1` and `t2`.
   * @returns Continuously compounded forward rate as a decimal over `(t1, t2)`.
   * @param t1 - Earlier curve time in years used as the start of the forward interval.
   * @param t2 - Later curve time in years used as the end of the forward interval.
   * @throws Error - Throws a JavaScript exception if either time is non-finite, `t2` is not later than `t1`, the interval is shorter than the curve's minimum forward tenor, or either endpoint discount factor is non-finite or non-positive.
   */
  forward(t1: number, t2: number): number;
}

/**
 * Discount factor curve for present-value calculations.
 *
 * Built from `(time, discount_factor)` pillars where `time` is a year
 * fraction from `baseDate` and `df` is the price today of $1 paid at that
 * time. Defaults reflect the most common practitioner convention
 * (Hagan-West monotone-convex interpolation, flat-forward extrapolation,
 * Act/365 fixed day-count).
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * // OIS-style USD curve, base-date 2025-01-02, three pillars.
 * const curve = new core.DiscountCurve({
 *   id: "USD-OIS",
 *   baseDate: "2025-01-02",
 *   knots: [0.0, 1.0, 1.0, 0.95, 5.0, 0.78],
 *   interp: "monotone_convex",
 *   extrapolation: "flat_forward",
 *   dayCount: "act_365f",
 * });
 * curve.df(2.5);          // discount factor at 2.5y
 * curve.zero(2.5);        // continuously-compounded zero rate at 2.5y
 * ```
 */
export interface DiscountCurveConstructor {
  /**
   * Construct a discount curve from named options.
   *
   * @param options - DiscountCurveOptions object (or its JSON text) with: `id` (curve identifier, the `MarketContext` lookup key); `baseDate` (ISO-8601 `"YYYY-MM-DD"`; knot times are year fractions from it under `dayCount`); `knots` (flat `[t0, df0, t1, df1, …]` array or typed array, `t` in years, `df` strictly positive, even length); and the optional `interp` (`"linear"`, `"log_linear"`, `"monotone_convex"`, `"cubic_hermite"`, `"piecewise_quadratic_forward"`), `extrapolation` (`"flat_zero"`, `"flat_forward"`, or `"none"`, which returns NaN outside the pillar range), `dayCount` (used to convert dates to curve time; not inferred from the ID), `validationMode` (`"market_standard"` or `"negative_rate_friendly"`) and `forwardFloor` (decimal minimum implied forward, required with `"negative_rate_friendly"` and rejected otherwise). Omitted options use the Rust builder defaults: `monotone_convex`, `flat_forward`, `act_365f` and `market_standard`. Unknown keys are rejected.
   * @returns The constructed `DiscountCurve`.
   * @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON string or plain object; `FinstackError` (kind `validation`) for an unknown or mistyped key, an odd `knots` length, a malformed date, an unknown interpolation/extrapolation/day-count/validation name, a misplaced or missing `forwardFloor`, or discount factors the curve validation rejects.
   */
  new (options: DiscountCurveOptions | string): DiscountCurve;
  /**
   * Construct a flat continuously-compounded discount curve.
   * @returns A `DiscountCurve` handle.
   * @param id - Curve identifier stored on the constructed discount curve.
   * @param baseDate - ISO-8601 curve base date from which time coordinates are measured.
   * @param continuousRate - Flat continuously compounded zero rate expressed as a decimal.
   * @throws Error - Throws a JavaScript exception if `baseDate` is not a valid ISO date, `continuousRate` is non-finite or `|continuousRate| > 1` (rates are decimals: `0.05` is 5%), or the implied discount factors are not finite and strictly positive.
   */
  flat(id: string, baseDate: string, continuousRate: number): DiscountCurve;
}

/**
 * Credit hazard-rate curve for default-probability modelling.
 *
 * Built from `(time, hazard_rate)` pillars where `time` is a year fraction
 * from `baseDate` and `hazard_rate` is the instantaneous default intensity
 * `λ(t)`. Survival is `S(t) = exp(-∫₀ᵗ λ(u) du)`.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * // Flat 200bp hazard rate, 40% recovery.
 * const hz = new core.HazardCurve({
 *   id: "ACME-HZD",
 *   baseDate: "2025-01-02",
 *   knots: [0.0, 0.02, 30.0, 0.02],
 *   recoveryRate: 0.4,
 * });
 * hz.sp(5.0);          // survival probability at 5y
 * hz.hazardRate(5.0);  // instantaneous hazard rate at 5y
 * const copy = core.HazardCurve.fromJson(hz.toJson());
 * ```
 */
export interface HazardCurve extends WasmOwned {
  /**
   * Curve identifier.
   */
  readonly id: string;
  /**
   * Base date as ISO string.
   */
  readonly baseDate: string;
  /**
   * Recovery rate assumed on default.
   */
  readonly recoveryRate: number;
  /**
   * Survival probability `S(t)` at year fraction `t`.
   * @param t - Time from the curve base date in years.
   * @returns The probability of surviving from the base date through `t`, in `[0, 1]`. This operation does not throw.
   */
  sp(t: number): number;
  /**
   * Instantaneous hazard rate `lambda(t)` at year fraction `t`.
   * @param t - Time from the curve base date in years.
   * @returns The annualized default intensity at `t`, expressed as a decimal rate. This operation does not throw.
   */
  hazardRate(t: number): number;
  /**
   * Knots as a flat `[t0, lambda0, t1, lambda1, …]` array (years, decimal intensities).
   */
  readonly knotPoints: Float64Array;
  /**
   * Par CDS quotes as a flat `[t0, bp0, …]` array in basis points (may be empty).
   */
  readonly parSpreadPoints: Float64Array;
  /**
   * Day-count convention label (e.g. `"act_365f"`).
   */
  readonly dayCount: string;
  /**
   * Currency of the protection leg, or `undefined`.
   */
  readonly currency: Currency | undefined;
  /**
   * Issuer name metadata, or `undefined`.
   */
  readonly issuer: string | undefined;
  /**
   * Debt seniority label (`"senior_secured"`, `"senior"`, `"subordinated"`,
   * `"junior"`), or `undefined`.
   */
  readonly seniority: string | undefined;
  /**
   * Par-spread readout interpolation label (`"linear"` or `"log_linear"`).
   */
  readonly parInterp: string;
  /**
   * Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
   *
   * @returns Compact JSON text.
   * @throws If serialization fails (not expected for a valid curve).
   */
  toJson(): string;
  /**
   * Survival probability on a date, measured with the curve day count.
   *
   * @param date - ISO-8601 target date on or after `baseDate`.
   * @returns Survival probability in `(0, 1]`.
   * @throws If `date` is not an ISO date or the year fraction cannot be computed.
   */
  spOnDate(date: string): number;
  /**
   * Hazard rate (decimal per year) on a date, measured with the curve day count.
   *
   * @param date - ISO-8601 target date on or after `baseDate`.
   * @returns Annual default intensity as a decimal.
   * @throws If `date` is not an ISO date or the year fraction cannot be computed.
   */
  hazardRateOnDate(date: string): number;
  /**
   * Survival probabilities on several dates.
   *
   * @param dates - ISO-8601 target dates on or after `baseDate`.
   * @returns One survival probability per input date, in order.
   * @throws If a date is not an ISO date or a year fraction cannot be computed.
   */
  survivalAtDates(dates: readonly string[]): Float64Array;
  /**
   * Probability of default in `[t1, t2]`: `sp(t1) - sp(t2)`.
   *
   * @param t1 - Start year fraction from `baseDate`.
   * @param t2 - End year fraction; must not precede `t1`.
   * @returns Default probability in `[0, 1]`.
   * @throws If `t2 < t1`.
   */
  defaultProb(t1: number, t2: number): number;
  /**
   * Interpolated par CDS spread in basis points at year fraction `t`.
   *
   * Uses the stored `parSpreads` quotes; with fewer than two quotes it
   * falls back to a hazard-based approximation.
   *
   * @param t - Year fraction from `baseDate`.
   * @param method - `"linear"` or `"log_linear"`; omitted uses the curve's `parInterp`.
   * @returns Par spread in basis points.
   * @throws If `method` is not a recognised label.
   */
  cdsQuoteBp(t: number, method?: string | null): number;
  /**
   * Copy of this curve with a different recovery rate (survival unchanged).
   *
   * @param recoveryRate - New recovery as a decimal fraction in `[0, 1]`.
   * @returns A new `HazardCurve`.
   * @throws If `recoveryRate` is outside `[0, 1]`.
   */
  withRecoveryRate(recoveryRate: number): HazardCurve;
}

/**
 * Credit hazard-rate curve for default-probability modelling.
 *
 * Built from `(time, hazard_rate)` pillars where `time` is a year fraction
 * from `baseDate` and `hazard_rate` is the instantaneous default intensity
 * `λ(t)`. Survival is `S(t) = exp(-∫₀ᵗ λ(u) du)`.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * // Flat 200bp hazard rate, 40% recovery.
 * const hz = new core.HazardCurve({
 *   id: "ACME-HZD",
 *   baseDate: "2025-01-02",
 *   knots: [0.0, 0.02, 30.0, 0.02],
 *   recoveryRate: 0.4,
 * });
 * hz.sp(5.0);          // survival probability at 5y
 * hz.hazardRate(5.0);  // instantaneous hazard rate at 5y
 * const copy = core.HazardCurve.fromJson(hz.toJson());
 * ```
 */
export interface HazardCurveConstructor {
  /**
   * Construct a hazard curve from named options.
   *
   * @param options - HazardCurveOptions object (or its JSON text) with: `id` (curve identifier, the `MarketContext` lookup key); `baseDate` (ISO-8601 `"YYYY-MM-DD"`; knot times are year fractions from it under `dayCount`); `knots` (flat `[t0, lambda0, t1, lambda1, …]` array or typed array, `t` in years, `lambda` a non-negative annual default intensity as a decimal); `recoveryRate` (required recovery on default, decimal in `[0, 1]`); and the optional `dayCount` (default `"act_365f"`), `parSpreads` (flat `[t0, bp0, …]` par CDS quotes in basis points, kept for reporting), `interp` (survival interpolation; only `"log_linear"` is accepted), `parInterp` (`"linear"` default or `"log_linear"`), `issuer`, `seniority` (`"senior_secured"`, `"senior"`, `"subordinated"`, `"junior"`), `currency` (ISO-4217 code of the protection leg) and `maxHazardRate` (sanity ceiling on any knot, default `10.0`). Omitted options use the Rust builder defaults. Unknown keys are rejected.
   * @returns The constructed `HazardCurve`.
   * @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON string or plain object, or holds a non-finite number; `FinstackError` (kind `validation`) for an unknown, missing or mistyped key, an odd-length `knots`/`parSpreads`, a malformed date, an unknown label, a knot the curve builder rejects, or `recoveryRate` outside `[0, 1]`.
   */
  new (options: HazardCurveOptions | string): HazardCurve;
  /**
   * Construct a flat (constant-intensity) hazard curve (Rust `HazardCurve::flat`).
   *
   * @param id - Curve identifier stored on the curve.
   * @param baseDate - ISO-8601 valuation date anchoring `t = 0`.
   * @param hazardRate - Constant annual default intensity as a decimal (`0.02` is 2%).
   * @param recoveryRate - Recovery on default as a decimal fraction in `[0, 1]`.
   * @returns Curve with `sp(t) === Math.exp(-hazardRate * t)`.
   * @throws If `baseDate` is not an ISO date, `hazardRate` is non-finite or negative, or `recoveryRate` is outside `[0, 1]`.
   */
  flat(id: string, baseDate: string, hazardRate: number, recoveryRate: number): HazardCurve;
  /**
   * Construct a hazard curve from survival-probability pillars (Rust
   * `HazardCurve::from_survival_probs`).
   *
   * @param id - Curve identifier stored on the curve.
   * @param baseDate - ISO-8601 valuation date anchoring `t = 0`.
   * @param points - Flat `[t0, s0, t1, s1, …]` array: times in years and survival probabilities in `(0, 1]`, non-increasing in time; a `t = 0` pillar must be `1.0`.
   * @param recoveryRate - Recovery on default as a decimal fraction in `[0, 1]`.
   * @returns Piecewise-constant hazard curve reproducing every pillar.
   * @throws If `points` is empty or odd-length, a probability is outside `(0, 1]` or increases with time, or `recoveryRate` is outside `[0, 1]`.
   */
  fromSurvivalProbs(
    id: string,
    baseDate: string,
    points: NumericArray,
    recoveryRate: number
  ): HazardCurve;
  /**
   * Deserialize a hazard curve from its canonical JSON wire form (the
   * Rust serde schema shared with Python `HazardCurve.to_json`).
   *
   * @param json - Canonical HazardCurve JSON text or plain object, such as `HazardCurve.toJson()` or `models.credit.mertonToHazardCurveJson` output. Unknown fields are rejected and the curve is re-validated.
   * @returns The validated `HazardCurve`.
   * @throws If `json` is malformed, has unknown fields, or fails curve validation.
   */
  fromJson(json: JsonInput): HazardCurve;
}

/**
 * Named options for constructing a `HazardCurve`; unknown keys are rejected.
 */
export interface HazardCurveOptions {
  /**
   * Curve identifier; the lookup key inside a `MarketContext`.
   */
  id: string;
  /**
   * ISO-8601 base date; knot times are year fractions from it under `dayCount`.
   */
  baseDate: string;
  /**
   * Flat `[t0, lambda0, t1, lambda1, …]` pairs: `t` in years, `lambda` a non-negative decimal intensity.
   */
  knots: NumericArray;
  /**
   * Recovery on default as a decimal fraction in `[0, 1]`.
   */
  recoveryRate: number;
  /**
   * Day-count convention for the time axis; the Rust builder default is `"act_365f"`.
   */
  dayCount?: string;
  /**
   * Flat `[t0, bp0, …]` par CDS quotes in basis points, kept for reporting and re-bootstrap risk.
   */
  parSpreads?: NumericArray;
  /**
   * Survival interpolation; only `"log_linear"` preserves piecewise-constant hazards.
   */
  interp?: string;
  /**
   * Par-spread readout interpolation: `"linear"` (Rust default) or `"log_linear"`.
   */
  parInterp?: string;
  /**
   * Issuer name metadata.
   */
  issuer?: string;
  /**
   * Debt seniority: `"senior_secured"`, `"senior"`, `"subordinated"` or `"junior"`.
   */
  seniority?: string;
  /**
   * ISO-4217 code of the protection-leg currency (metadata).
   */
  currency?: string;
  /**
   * Sanity ceiling on any knot hazard rate; the Rust builder default is `10.0`.
   */
  maxHazardRate?: number;
}

/**
 * Forward rate curve for a floating-rate index with a fixed tenor.
 */
export interface ForwardCurve extends WasmOwned {
  /**
   * Curve identifier.
   */
  readonly id: string;
  /**
   * Base date as ISO string.
   */
  readonly baseDate: string;
  /**
   * Contractual projection boundaries, or `undefined` for legacy tenor stepping.
   */
  readonly projectionGrid: Float64Array | undefined;
  /**
   * Business days from fixing to spot.
   */
  readonly resetLag: number;
  /**
   * Forward rate at year fraction `t`.
   * @returns Simply compounded forward rate as a decimal at time `t`.
   * @param t - Time from the curve base date in years.
   */
  rate(t: number): number;
  /**
   * Discount-factor-implied simple forward over `(t1, t2)`.
   * @returns Simple forward rate as a decimal implied by discount factors over `(t1, t2)`.
   * @param t1 - Earlier curve time in years used as the start of the forward interval.
   * @param t2 - Later curve time in years used as the end of the forward interval.
   * @throws Error - Throws a JavaScript exception if either time is non-finite, `t2` is not later than `t1`, a projection discount factor cannot be computed, or the implied rate is non-finite.
   */
  rateBetween(t1: number, t2: number): number;
  /**
   * Index tenor in years.
   */
  readonly tenor: number;
  /**
   * Knot times in years.
   */
  readonly knots: Float64Array;
  /**
   * Forward rates at the knots, as decimals.
   */
  readonly forwards: Float64Array;
  /**
   * Day-count convention label (e.g. `"act_360"`).
   */
  readonly dayCount: string;
  /**
   * Interpolation style label (e.g. `"linear"`).
   */
  readonly interpStyle: string;
  /**
   * Extrapolation policy label (e.g. `"flat_forward"`).
   */
  readonly extrapolation: string;
  /**
   * Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
   *
   * @returns Compact JSON text.
   * @throws If serialization fails (not expected for a valid curve).
   */
  toJson(): string;
  /**
   * Simple forward rate over `[t1, t2]` implied by the curve (Rust `rate_period`).
   *
   * @param t1 - Start of the accrual period in years from `baseDate`.
   * @param t2 - End of the accrual period in years from `baseDate`.
   * @returns The average forward over the period as a decimal.
   */
  ratePeriod(t1: number, t2: number): number;
  /**
   * Projection discount factor implied by the forwards at year fraction `t`.
   *
   * @param t - Time from `baseDate` in years.
   * @returns Projection discount factor.
   * @throws If the implied discount factor is non-finite or non-positive.
   */
  df(t: number): number;
  /**
   * Projection discount factor on a date, measured with the curve day count.
   *
   * @param date - ISO-8601 target date.
   * @returns Projection discount factor.
   * @throws If `date` is not an ISO date, the year fraction cannot be computed, or the implied discount factor is invalid.
   */
  dfOnDateCurve(date: string): number;
}

/**
 * Forward rate curve for a floating-rate index with a fixed tenor.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const curve = new core.ForwardCurve({
 *   id: "USD-SOFR-3M", tenor: 0.25, baseDate: "2026-01-02",
 *   knots: [0, 0.03, 1, 0.035]
 * });
 * console.log(curve.rate(0.5));
 * ```
 */
export interface ForwardCurveConstructor {
  /**
   * Construct a forward curve using named inputs and canonical Rust defaults.
   * @returns The validated forward curve.
   * @param options - ForwardCurveOptions object: curve id, tenor in years, ISO baseDate, flat time/decimal-rate knots, and optional dayCount, interp, extrapolation, projectionGrid and resetLag. Omitted policies use the Rust builder defaults; arrays and typed arrays are accepted.
   * @throws Error - Throws Error when options cannot be decoded or canonical curve validation rejects dates, conventions, knots, tenor, reset lag, or projection grid.
   */
  new (options: ForwardCurveOptions): ForwardCurve;
  /**
   * Construct a flat forward curve (Rust `ForwardCurve::flat`).
   *
   * @param id - Curve identifier stored on the curve.
   * @param tenor - Index tenor in years (e.g. `0.25` for a 3M index).
   * @param baseDate - ISO-8601 valuation date anchoring `t = 0`.
   * @param rate - Constant forward rate as a decimal.
   * @returns A `ForwardCurve` with the Rust builder defaults.
   * @throws If `baseDate` is not an ISO date, or `tenor` or `rate` is invalid.
   */
  flat(id: string, tenor: number, baseDate: string, rate: number): ForwardCurve;
  /**
   * Deserialize a forward curve from its canonical JSON wire form (the
   * Rust serde schema shared with Python `ForwardCurve.to_json`).
   *
   * @param json - Canonical ForwardCurve JSON text or plain object; unknown fields are rejected and the curve is re-validated.
   * @returns The validated `ForwardCurve`.
   * @throws If `json` is malformed, has unknown fields, or fails curve validation.
   */
  fromJson(json: JsonInput): ForwardCurve;
}

/**
 * Named options for constructing a `ForwardCurve`.
 */
export interface ForwardCurveOptions {
  /**
   * Curve identifier stored on the constructed forward curve.
   */
  id: string;
  /**
   * Index tenor in years, such as 0.25 for a 3-month forward.
   */
  tenor: number;
  /**
   * ISO-8601 base or valuation date that anchors the curve time axis.
   */
  baseDate: string;
  /**
   * Flat `[time, rate]` pairs in year-fraction / decimal-rate units.
   */
  knots: NumericArray;
  /**
   * Day-count convention used to convert dates into year fractions.
   */
  dayCount?: string;
  /**
   * Interpolation style between knots, such as `"monotone_convex"`.
   */
  interp?: string;
  /**
   * Extrapolation policy beyond the last knot, such as `"flat_forward"`.
   */
  extrapolation?: string;
  /**
   * Projection-grid specification that defines the curve's forward-rate intervals.
   */
  projectionGrid?: NumericArray;
  /**
   * Reset lag applied when projecting the index or forward rate.
   */
  resetLag?: number | null;
}

/**
 * Data-only SABR volatility cube for swaption pricing.
 *
 * Stores parameter nodes, forward nodes, axes, and interpolation metadata.
 * Use `models.volatility` for evaluation.
 */
export interface VolCube extends WasmOwned {
  /**
   * Cube identifier.
   */
  readonly id: string;
  /**
   * Interpolation contract used across the expiry axis.
   */
  readonly interpolationMode: string;
  /**
   * Option expiry axis in years.
   */
  readonly expiries: Float64Array;
  /**
   * Underlying swap tenor axis in years.
   */
  readonly tenors: Float64Array;
  /**
   * Grid shape as `[nExpiries, nTenors]`.
   */
  readonly gridShape: Uint32Array;
  /**
   * Row-major forward rates (decimals), one per grid node.
   */
  readonly forwards: Float64Array;
  /**
   * Row-major SABR nodes as plain `{alpha, beta, rho, nu, shift?}` objects.
   * @returns One object per grid node in row-major (expiry, tenor) order.
   * @throws If serialization fails (not expected for a valid cube).
   */
  readonly params: SabrParameterData[];
  /**
   * Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
   *
   * @returns Compact JSON text.
   * @throws If serialization fails (not expected for a valid cube).
   */
  toJson(): string;
  /**
   * SABR parameters at grid indices, as a plain `{alpha, beta, rho, nu, shift?}` object.
   *
   * @param expIdx - Zero-based index into `expiries`.
   * @param tenorIdx - Zero-based index into `tenors`.
   * @returns The node's SABR parameters.
   * @throws `TypeError` if an index is not a non-negative integer; `FinstackError` (kind `validation`) if it lies outside `gridShape`.
   */
  paramsAt(expIdx: number, tenorIdx: number): SabrParameterData;
  /**
   * Forward rate (decimal) at grid indices.
   *
   * @param expIdx - Zero-based index into `expiries`.
   * @param tenorIdx - Zero-based index into `tenors`.
   * @returns The node's forward rate.
   * @throws `TypeError` if an index is not a non-negative integer; `FinstackError` (kind `validation`) if it lies outside `gridShape`.
   */
  forwardAt(expIdx: number, tenorIdx: number): number;
}

/**
 * Data-only SABR volatility cube for swaption pricing.
 *
 * Stores calibrated parameter nodes and forwards on an expiry × tenor grid.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const cube = new core.VolCube(
 *   "USD-SWAPTION",
 *   [1],
 *   [5],
 *   [0.02, 0.5, -0.2, 0.4, Number.NaN],
 *   [0.03]
 * );
 * console.log(cube.id, cube.interpolationMode);
 * cube.free();
 * ```
 */
export interface VolCubeConstructor {
  /**
   * Construct a vol cube from a flat SABR parameter array.
   *
   * @returns A `VolCube` handle.
   * @param id - Curve identifier.
   * @param expiries - Option expiry axis in years (strictly increasing).
   * @param tenors - Swap tenor axis in years (strictly increasing).
   * @param paramsFlat - Row-major flat array of SABR parameters: `[alpha0, beta0, rho0, nu0, shift0, alpha1, …]`. Length must equal `expiries.len() * tenors.len() * 5`. Pass `NaN` for the shift element of a node to omit the shift.
   * @param forwards - Row-major forward rates, one per grid node.
   * @param interpolationMode - Interpolation across the expiry axis: `"vol"` or `"total_variance"`; omitted keeps the Rust `VolCube::from_grid` default (`"vol"`).
   * @throws Error - Throws a JavaScript exception if an axis is empty, non-finite, non-positive, or not strictly increasing; the parameter or forward array has the wrong length; a forward is non-finite; any SABR node has invalid alpha, beta, rho, nu, or shift; or `interpolationMode` is neither `vol` nor `total_variance`.
   */
  new (
    id: string,
    expiries: NumericArray,
    tenors: NumericArray,
    paramsFlat: NumericArray,
    forwards: NumericArray,
    interpolationMode?: string
  ): VolCube;
  /**
   * Deserialize a canonical SABR cube state without flattening parameter nodes.
   *
   * @example
   * ```typescript
   * import init, { core } from "finstack-quant-wasm";
   * await init();
   * const cube = core.VolCube.fromJson(
   *   JSON.stringify({
   *     id: "USD-SWAPTION",
   *     expiries: [1],
   *     tenors: [5],
   *     params: [{ alpha: 0.03, beta: 0.5, rho: -0.2, nu: 0.4, shift: null }],
   *     forwards: [0.03],
   *     interpolation_mode: "vol",
   *   })
   * );
   * console.log(cube.id);
   * cube.free();
   * ```
   * @param json - Canonical VolCube JSON containing id, expiry and tenor axes in years, row-major SABR nodes and decimal-rate forwards, and interpolation_mode. Missing or null node shifts remain absent; unknown fields are rejected.
   * @returns A validated VolCube handle owned by the caller; release it with free().
   * @throws Error - Throws when JSON is malformed, fields are unknown, or native axis, parameter, or forward validation fails.
   */
  fromJson(json: JsonInput): VolCube;
}

/**
 * Typed FX conversion policy wrapper for WASM callers.
 */
export interface FxConversionPolicy extends WasmOwned {
  /**
   * String form of the conversion policy.
   * @returns Human-readable string form of this value.
   */
  toString(): string;
}

/**
 * Typed FX conversion policy wrapper for WASM callers.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const policy = core.FxConversionPolicy.cashflowDate();
 * console.log(policy.toString());
 * ```
 */
export interface FxConversionPolicyConstructor {
  /**
   * JavaScript prototype of `FxConversionPolicy`; instances come from the static factories, not `new`.
   */
  readonly prototype: FxConversionPolicy;
  /**
   * Use spot/forward on the cashflow date.
   * @returns An `FxConversionPolicy` handle.
   */
  cashflowDate(): FxConversionPolicy;
  /**
   * Use period end date.
   * @returns An `FxConversionPolicy` handle.
   */
  periodEnd(): FxConversionPolicy;
  /**
   * Use an average over the period.
   * @returns An `FxConversionPolicy` handle.
   */
  periodAverage(): FxConversionPolicy;
  /**
   * Parse from a string label such as ``\"cashflow_date\"``.
   * @returns An `FxConversionPolicy` handle.
   * @param name - Policy label: `cashflow_date`, `period_end`, or `period_average`.
   * @throws Error - Throws a JavaScript exception unless `name` is `cashflow_date`, `period_end`, or `period_average`.
   */
  fromName(name: string): FxConversionPolicy;
}

/**
 * Structured FX lookup result for WASM callers.
 */
export interface FxRateResult extends WasmOwned {
  /**
   * The FX conversion rate.
   */
  readonly rate: number;
  /**
   * Whether the rate was obtained via triangulation.
   */
  readonly triangulated: boolean;
  /**
   * Serialize to the canonical JSON wire form shared with Python `FxRateResult.to_json`.
   *
   * @returns Compact JSON text with `rate` and `triangulated`.
   * @throws If serialization fails (not expected for a valid result).
   */
  toJson(): string;
}

/**
 * `FxRateResult` has no public constructor; instances come from `FxMatrix.rate`
 * or `FxRateResult.fromJson`.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const matrix = new core.FxMatrix();
 * matrix.setQuote("EUR", "USD", 1.1);
 * const result = matrix.rate(
 *   "EUR",
 *   "USD",
 *   "2026-01-02",
 *   core.FxConversionPolicy.cashflowDate()
 * );
 * console.log(result.rate, result.triangulated);
 * ```
 */
export interface FxRateResultConstructor {
  /**
   * JavaScript prototype of `FxRateResult`; instances come from `FxMatrix.rate`, not `new`.
   */
  readonly prototype: FxRateResult;
  /**
   * Deserialize from the canonical JSON wire form produced by `toJson`.
   *
   * @param json - Canonical FxRateResult JSON text or plain object; unknown fields are rejected.
   * @returns The parsed `FxRateResult`.
   * @throws If `json` is malformed or has unknown or missing fields.
   */
  fromJson(json: JsonInput): FxRateResult;
}

/**
 * Foreign-exchange rate matrix for currency conversion.
 */
export interface FxMatrix extends WasmOwned {
  /**
   * Set an explicit FX quote.
   *
   * @param base - Base (from) currency ISO code.
   * @param quote - Quote (to) currency ISO code.
   * @param rate - Conversion rate.
   * @throws Error - Throws a JavaScript exception if either currency code is invalid or `rate` is non-finite or not strictly positive.
   */
  setQuote(base: string, quote: string, rate: number): void;
  /**
   * Set an authoritative quote scoped to one date and conversion policy.
   * @param base - Base currency code of the FX quote, where the rate is quote per base.
   * @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
   * @param date - ISO-8601 date used by the calculation or market-data lookup.
   * @param policy - FX quote-selection policy for resolving direct, inverse, or triangulated rates.
   * @param rate - Finite positive quote-currency units per one base-currency unit.
   * @throws Error - Throws a JavaScript exception if either currency code is invalid, `date` is not a valid ISO date, or `rate` is non-finite or not strictly positive.
   */
  setQuoteOn(
    base: string,
    quote: string,
    date: string,
    policy: FxConversionPolicy,
    rate: number
  ): void;
  /**
   * Look up an FX rate.
   * Global quotes precede pinned fixings, then provider observations,
   * resolving source priority before taking a reciprocal. An omitted `policy`
   * runs the Rust default query (`FxQuery::new`, cashflow-date policy).
   *
   * @returns Resolved FX rate, including whether it was triangulated.
   * @param base - Base (from) currency ISO code.
   * @param quote - Quote (to) currency ISO code.
   * @param date - ISO date string.
   * @param policy - Reusable conversion policy handle; omitted means the Rust default (cashflow date).
   * @throws Error - Throws a JavaScript exception if either currency code or `date` is invalid, no direct, inverse, or triangulated quote is available, or a resolved quote is non-finite or non-positive.
   */
  rate(base: string, quote: string, date: string, policy?: FxConversionPolicy): FxRateResult;
}

/**
 * Foreign-exchange rate matrix for currency conversion.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const matrix = new core.FxMatrix();
 * matrix.setQuote("EUR", "USD", 1.1);
 * console.log(matrix.rate("EUR", "USD", "2026-01-02").rate);
 * ```
 */
export interface FxMatrixConstructor {
  /**
   * Create an empty FX matrix.
   * @returns An `FxMatrix` handle.
   */
  new (): FxMatrix;
}

/**
 * USD quotation style for a market FX pair (Direct or Indirect versus USD).
 *
 * **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
 * means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
 * quotation of market CCY1 versus USD.
 */
export interface FxQuoteConvention extends WasmOwned {
  /**
   * String form of the USD quotation style (`"direct"` or `"indirect"`).
   * @returns Human-readable string form of this value.
   */
  toString(): string;
}

/**
 * USD quotation style for a market FX pair (Direct or Indirect versus USD).
 *
 * **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
 * means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
 * quotation of market CCY1 versus USD.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const direct = core.FxQuoteConvention.direct();
 * direct.toString(); // "direct"
 * ```
 */
export interface FxQuoteConventionConstructor {
  /**
   * JavaScript prototype of `FxQuoteConvention`; instances come from the static factories, not `new`.
   */
  readonly prototype: FxQuoteConvention;
  /**
   * USD is the quote currency (units of USD per one unit of CCY1).
   * @returns An `FxQuoteConvention` handle.
   */
  direct(): FxQuoteConvention;
  /**
   * USD is the base currency (units of CCY2 per one USD).
   * @returns An `FxQuoteConvention` handle.
   */
  indirect(): FxQuoteConvention;
  /**
   * Parse from a string label such as `"direct"` or `"indirect"`.
   * @returns An `FxQuoteConvention` handle.
   * @param name - Convention label: `direct` or `indirect`.
   * @throws Error - Throws a JavaScript exception unless `name` is `direct` or `indirect`.
   */
  fromName(name: string): FxQuoteConvention;
}

/**
 * Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.
 *
 * Instances come from `fxPairConvention`. `base` / `quote` are always market
 * CCY1/CCY2, even when the lookup arguments were inverted.
 */
export interface FxPairConvention extends WasmOwned {
  /**
   * Market CCY1 (one unit of this currency in the screen pair).
   */
  readonly base: Currency;
  /**
   * Market CCY2 (units of this currency per one unit of CCY1).
   */
  readonly quote: Currency;
  /**
   * Direct if the USD leg quotes USD as CCY2; Indirect if USD is CCY1.
   */
  readonly usdQuotation: FxQuoteConvention;
  /**
   * Pip size in outright-rate units (`0.01` or `0.0001`).
   */
  readonly pipSize: number;
  /**
   * Standard spot lag in business days (T+1 or T+2).
   */
  readonly settlementDays: number;
}

/**
 * Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.
 *
 * Instances come from `fxPairConvention`. `base` / `quote` are always market
 * CCY1/CCY2, even when the lookup arguments were inverted.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const conv = core.fxPairConvention("USD", "EUR");
 * conv.base.code;          // "EUR"
 * conv.usdQuotation.toString(); // "direct"
 * conv.pipSize;            // 0.0001
 * conv.settlementDays;     // 2
 * ```
 */
export interface FxPairConventionConstructor {
  /**
   * JavaScript prototype of `FxPairConvention`; instances come from
   * `fxPairConvention`, not `new`.
   */
  readonly prototype: FxPairConvention;
}

/**
 * FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
 * 10-delta wings).
 *
 * Stores market-standard FX delta quotes (Wystup 2006, Clark 2011). Use
 * `models.volatility` for pillar recovery, conversion, and evaluation. The
 * delta convention is **forward delta (premium-unadjusted)**.
 */
export interface FxDeltaVolSurface extends WasmOwned {
  /**
   * Surface identifier.
   */
  readonly id: string;
  /**
   * Expiry axis in years.
   */
  readonly expiries: Float64Array;
  /**
   * Number of expiry pillars.
   */
  readonly numExpiries: number;
  /**
   * ATM delta-neutral straddle vols per expiry (decimals).
   */
  readonly atmVols: Float64Array;
  /**
   * 25-delta risk reversals per expiry (call vol minus put vol, decimals).
   */
  readonly rr25d: Float64Array;
  /**
   * 25-delta butterflies per expiry (wing average minus ATM, decimals).
   */
  readonly bf25d: Float64Array;
  /**
   * 10-delta risk reversals per expiry, or `undefined` without 10-delta wings.
   */
  readonly rr10d: Float64Array | undefined;
  /**
   * 10-delta butterflies per expiry, or `undefined` without 10-delta wings.
   */
  readonly bf10d: Float64Array | undefined;
  /**
   * Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
   *
   * @returns Compact JSON text.
   * @throws If serialization fails (not expected for a valid surface).
   */
  toJson(): string;
}

/**
 * FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
 * 10-delta wings).
 *
 * Stores market-standard FX delta quotes (Wystup 2006, Clark 2011). The delta
 * convention is **forward delta (premium-unadjusted)**.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const surface = new core.FxDeltaVolSurface(
 *   "EURUSD-VOL",
 *   [1],
 *   [0.12],
 *   [0.01],
 *   [0.002]
 * );
 * console.log(surface.id, surface.numExpiries);
 * surface.free();
 * ```
 */
export interface FxDeltaVolSurfaceConstructor {
  /**
   * Construct an FX delta-quoted vol surface with 25-delta wings.
   *
   * Optional `rr10d` / `bf10d` add 10-delta wings for richer wing
   * interpolation. Omit both (`undefined`/`null`) for a three-point smile;
   * the Rust constructor rejects one without the other.
   *
   * @returns An `FxDeltaVolSurface` handle.
   * @param id - Stable surface identifier.
   * @param expiries - Strictly increasing positive expiry times (years).
   * @param atmVols - ATM delta-neutral straddle vols per expiry.
   * @param rr25d - 25-delta risk reversal per expiry (call vol − put vol).
   * @param bf25d - 25-delta butterfly per expiry (wing avg − ATM).
   * @param rr10d - Optional 10-delta risk reversal per expiry.
   * @param bf10d - Optional 10-delta butterfly per expiry.
   * @throws Error - Throws a JavaScript exception if `rr10d` and `bf10d` are not both present or both absent; quote arrays are empty or have mismatched lengths; expiries are not finite, positive, and strictly increasing; ATM vols are not finite and positive; or any risk reversal or butterfly is non-finite.
   */
  new (
    id: string,
    expiries: NumericArray,
    atmVols: NumericArray,
    rr25d: NumericArray,
    bf25d: NumericArray,
    rr10d?: NumericArray,
    bf10d?: NumericArray
  ): FxDeltaVolSurface;
  /**
   * Deserialize canonical FX delta quotes without reconstructing positional arrays.
   *
   * @example
   * ```typescript
   * import init, { core } from "finstack-quant-wasm";
   * await init();
   * const surface = core.FxDeltaVolSurface.fromJson(
   *   JSON.stringify({
   *     id: "EURUSD",
   *     expiries: [1],
   *     atm_vols: [0.12],
   *     rr_25d: [0.01],
   *     bf_25d: [0.002],
   *     rr_10d: null,
   *     bf_10d: null,
   *   })
   * );
   * console.log(surface.id);
   * surface.free();
   * ```
   * @param json - Canonical FxDeltaVolSurface JSON with expiries in years and annualized decimal ATM, risk-reversal, and butterfly quotes. Optional 10-delta wings must occur together; unknown fields are rejected.
   * @returns A validated FxDeltaVolSurface handle owned by the caller; release it with free().
   * @throws Error - Throws when JSON is malformed, fields are unknown, or native expiry, quote, or wing validation fails.
   */
  fromJson(json: JsonInput): FxDeltaVolSurface;
}

/**
 * Parsed Rust `MarketContext` held across pricing calls.
 *
 * Build it with `MarketContext.fromJson`, then pass it to
 * `valuations.instruments.priceInstrumentWithMarket` and the other
 * `*WithMarket` entry points. This avoids re-parsing the market JSON in
 * bulk-pricing and Greeks-sweep loops. WASM binds only the JSON round trip;
 * the Python class also exposes the insertion and lookup methods.
 */
export interface MarketContext extends WasmOwned {
  /**
   * Serialize the wrapped MarketContext back to canonical JSON.
   * @returns Canonical MarketContext JSON accepted by `MarketContext.fromJson` and every `marketJson` argument.
   * @throws Error - Throws if the market context cannot be serialized to JSON.
   */
  toJson(): string;
}

/**
 * Static entry points of the `MarketContext` handle.
 * @example
 * ```typescript
 * import init, { core, valuations } from "finstack-quant-wasm";
 * await init();
 * const market = core.MarketContext.fromJson(marketJson);
 * for (const instr of instruments) {
 *   const result = valuations.instruments.priceInstrumentWithMarket(instr, market, "2025-06-15");
 * }
 * market.free();
 * ```
 */
export interface MarketContextConstructor {
  /**
   * JavaScript prototype of `MarketContext`; instances come from `fromJson`, not `new`.
   */
  readonly prototype: MarketContext;
  /**
   * Parse a market context from its canonical JSON representation.
   * @param json - Canonical MarketContext JSON (string or plain object), the same payload accepted by pricing `marketJson` arguments. Unknown fields are rejected.
   * @returns A `MarketContext` handle that can be reused across pricing calls; release it with free().
   * @throws Error - Throws with kind `validation` when the JSON is malformed or does not match the MarketContext schema, and a `TypeError` when `json` is neither a string nor a plain object.
   */
  fromJson(json: JsonInput): MarketContext;
}

/**
 * Namespaced TypeScript entry points for core calculations and types.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * console.log(core.mean([1, 2, 3]));
 * ```
 */
export interface CoreNamespace {
  /**
   * ISO-4217 currency constructor (`new core.Currency("USD")`).
   */
  Currency: CurrencyConstructor;
  /**
   * Currency-tagged decimal amount constructor.
   */
  Money: MoneyConstructor;
  /**
   * Decimal interest-rate constructor (0.05 is 5%).
   */
  Rate: RateConstructor;
  /**
   * Basis-point quantity constructor (1 is 0.01%).
   */
  Bps: BpsConstructor;
  /**
   * Percentage quantity constructor (5 is 5%).
   */
  Percentage: PercentageConstructor;
  /**
   * Day-count convention constructor and named factories.
   */
  DayCount: DayCountConstructor;
  /**
   * Optional market metadata for day-count calculations.
   */
  DayCountContext: DayCountContextConstructor;
  /**
   * Period-length constructor and named factories (`3M`, `1Y`).
   */
  Tenor: TenorConstructor;
  /**
   * Create a date and return it as epoch days (days since 1970-01-01).
   * @returns Days since 1970-01-01 (Unix epoch).
   * @param year - Four-digit calendar year component of the supplied date.
   * @param month - Calendar month number from 1 through 12.
   * @param day - Calendar day number within the selected month.
   * @throws Error - Throws a JavaScript exception if `month` is outside `1..=12` or the supplied year, month, and day do not form a representable calendar date.
   */
  createDate(year: number, month: number, day: number): number;
  /**
   * Convert epoch days back to `[year, month, day]` as a JS array-compatible triple.
   * @returns `[year, month, day]` as an `Int32Array`, with month in `1..=12`.
   * @param days - Number of days since 1970-01-01 to decompose into year, month, and day.
   * @throws Error - Throws a JavaScript exception if `days` is outside the representable date range.
   */
  dateFromEpochDays(days: number): Int32Array;
  /**
   * Adjust a date (epoch days) according to a business-day convention and calendar.
   *
   * Returns the adjusted date as epoch days.
   * @returns Adjusted date as days since 1970-01-01.
   * @param epochDays - Unadjusted date as days since 1970-01-01.
   * @param convention - Business-day adjustment convention string accepted by the date API.
   * @param calendarCode - Registered holiday-calendar identifier used to find business days.
   * @throws Error - Throws a JavaScript exception if `epochDays` is outside the representable date range, `convention` is unrecognized, `calendarCode` is unknown, or adjustment cannot produce a representable business date.
   */
  adjust(epochDays: number, convention: string, calendarCode: string): number;
  /**
   * Return the list of available calendar codes.
   * @returns Registered holiday-calendar identifiers, sorted alphabetically.
   */
  availableCalendars(): string[];
  /**
   * Discount-factor curve constructor.
   */
  DiscountCurve: DiscountCurveConstructor;
  /**
   * Credit hazard-rate curve constructor.
   */
  HazardCurve: HazardCurveConstructor;
  /**
   * Index forward-rate curve constructor.
   */
  ForwardCurve: ForwardCurveConstructor;
  /**
   * SABR swaption volatility cube constructor.
   */
  VolCube: VolCubeConstructor;
  /**
   * FX delta-quoted volatility surface constructor.
   */
  FxDeltaVolSurface: FxDeltaVolSurfaceConstructor;
  /**
   * Parsed market-context handle for reuse across `*WithMarket` calls.
   */
  MarketContext: MarketContextConstructor;
  /**
   * FX conversion timing-policy constructor.
   */
  FxConversionPolicy: FxConversionPolicyConstructor;
  /**
   * Resolved FX quote result constructor.
   */
  FxRateResult: FxRateResultConstructor;
  /**
   * Cross-currency FX matrix constructor.
   */
  FxMatrix: FxMatrixConstructor;
  /**
   * USD quotation-style constructor (`direct` / `indirect` versus USD).
   */
  FxQuoteConvention: FxQuoteConventionConstructor;
  /**
   * Market FX pair-convention prototype; instances come from `fxPairConvention`.
   */
  FxPairConvention: FxPairConventionConstructor;
  /**
   * Order two currencies into the market CCY1/CCY2 pair.
   *
   * Priority is EUR > GBP > AUD > NZD > USD > other, with a stable ISO-4217
   * alphabetic tie-break when both sides share the same rank.
   * @param a - First currency ISO code of the unordered pair. Need not be market CCY1.
   * @param b - Second currency ISO code of the unordered pair. Need not be market CCY2.
   * @returns A two-element array `[CCY1, CCY2]` of `Currency` handles in market order.
   * @throws Error - Throws a JavaScript exception if either code is not a recognized ISO-4217 alphabetic currency.
   */
  fxMarketPair(a: string, b: string): Currency[];
  /**
   * Market convention for an unordered currency pair.
   *
   * Returned `base` / `quote` are always the market CCY1/CCY2, even when the
   * arguments are inverted.
   * @param base - One currency ISO code of the pair. Orientation is ignored.
   * @param quote - The other currency ISO code of the pair. Orientation is ignored.
   * @returns Market CCY1/CCY2, USD quotation, pip size, and standard spot lag.
   * @throws Error - Throws a JavaScript exception if either code is not a recognized ISO-4217 alphabetic currency.
   */
  fxPairConvention(base: string, quote: string): FxPairConvention;
  /**
   * Pip size in outright-rate units for a currency pair.
   *
   * Returns `0.01` when either side is JPY, KRW, or HUF; otherwise `0.0001`.
   * Argument order does not matter.
   * @param base - One currency ISO code of the pair. Order is not significant.
   * @param quote - The other currency ISO code of the pair. Order is not significant.
   * @returns Pip size as a decimal increment of the outright FX rate.
   * @throws Error - Throws a JavaScript exception if either code is not a recognized ISO-4217 alphabetic currency.
   */
  fxPipSize(base: string, quote: string): number;
  /**
   * Reciprocal of a strictly positive finite FX rate.
   * @param rate - Outright FX rate to invert, in quote-per-base units. Must be finite and strictly positive; the reciprocal must also be a valid FX rate.
   * @returns `1 / rate` when that reciprocal is a valid FX rate.
   * @throws Error - Throws a `validation` error if `rate` is non-finite, zero or negative, or its reciprocal overflows.
   */
  invertFxRate(rate: number): number;
  /**
   * Apply a lower-triangular factor L to a vector z, returning `L z`.
   *
   * This is the Cholesky "apply" step that turns independent standard normals
   * into correlated normals: if `A = L L^T` and `z ~ N(0, I)`, then
   * `L z ~ N(0, A)`. Accepts L as `n * n` row-major entries; only the lower
   * triangle is read and the upper triangle is assumed zero.
   * @returns Transformed vector `L z` as a `Float64Array` of length `n`.
   * @param l - Lower-triangular Cholesky factor as a flat row-major array of n × n entries.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @param z - Vector of length n to transform, typically independent standard-normal draws.
   * @throws Error - Throws a JavaScript exception if `l` does not contain exactly `n * n` entries (including when `n * n` overflows) or `z` does not contain exactly `n` entries.
   */
  applyLowerTriangular(l: NumericArray, n: number, z: NumericArray): Float64Array;
  /**
   * Cholesky decomposition for a flat row-major matrix.
   *
   * Accepts a `Float64Array`/`number[]` containing `n * n` row-major entries
   * and returns a flat lower-triangular factor.
   * @param matrix - Flat row-major `n * n` entries of a symmetric positive-definite matrix.
   * @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
   * @returns Lower-triangular factor L as a flat row-major `Float64Array`.
   * @throws Error - Throws a JavaScript exception if `matrix` does not contain exactly `n * n` entries (including when `n * n` overflows), or the matrix contains a non-finite value, is singular, or is not positive definite.
   */
  choleskyDecomposition(matrix: NumericArray, n: number): Float64Array;
  /**
   * Solve a symmetric positive-definite linear system from a flat Cholesky factor.
   * @param chol - Lower-triangular Cholesky factor as a flat row-major array of `b.length * b.length` entries.
   * @param b - Right-hand-side vector of the linear system; its length is the system dimension.
   * @returns Solution vector `x` of `L Lᵀ x = b`, with the same length as `b`.
   * @throws Error - Throws a JavaScript exception if `chol` does not contain exactly `b.length * b.length` entries or a diagonal factor is singular.
   */
  choleskySolve(chol: NumericArray, b: NumericArray): Float64Array;
  /**
   * Arithmetic mean over a typed numeric array.
   * @param data - Numeric observations in input order; an empty series yields 0.0.
   * @returns Arithmetic mean of `data`, or 0.0 when `data` is empty.
   */
  mean(data: NumericArray): number;
  /**
   * Sample variance over a typed numeric array.
   * @param data - Sample observations in input order; fewer than two points yield 0.0.
   * @returns Unbiased sample variance, or 0.0 when `data` has fewer than two points.
   */
  variance(data: NumericArray): number;
  /**
   * Population variance over a typed numeric array.
   * @param data - Observations in input order; fewer than two points yield 0.0.
   * @returns Population variance, or 0.0 when `data` has fewer than two points.
   */
  populationVariance(data: NumericArray): number;
  /**
   * Pearson correlation over typed numeric arrays.
   * @param x - First numeric series; must have the same length as `y`.
   * @param y - Second numeric series, aligned one-for-one with `x`.
   * @returns Sample correlation in `[-1, 1]`, or NaN when a series has fewer than two points.
   */
  correlation(x: NumericArray, y: NumericArray): number;
  /**
   * Sample covariance over typed numeric arrays.
   * @param x - First numeric series; must have the same length as `y`.
   * @param y - Second numeric series, aligned one-for-one with `x`.
   * @returns Unbiased sample covariance, or 0.0 when a series has fewer than two points.
   */
  covariance(x: NumericArray, y: NumericArray): number;
  /**
   * Empirical quantile over a typed numeric array.
   * @param data - Sample observations in input order; empty or non-finite data yields NaN.
   * @param q - Quantile probability in `[0, 1]`; values outside that range yield NaN.
   * @returns R-7 interpolated quantile, or NaN when `data` is empty or non-finite.
   */
  quantile(data: NumericArray, q: number): number;
  /**
   * Annualized realized variance of a close price series (Rust
   * `stats::realized_variance`): the mean of squared log returns times the
   * annualization factor, with no mean subtraction.
   *
   * @example
   * ```javascript
   * core.realizedVariance([100, 101, 99.5, 100.2]);  // close-to-close, 252/yr
   * ```
   * @param prices - Close prices in time order; each must be finite and positive.
   * @param method - Estimator name; only `"close_to_close"` applies to closes (the OHLC estimators need `realizedVarianceOhlc`). Omitted selects the Rust default (`"close_to_close"`).
   * @param annualizationFactor - Observations per year (for example `252` for daily closes); omitted selects the Rust daily default (252).
   * @returns Annualized realized variance (decimal, not volatility).
   * @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) for an unknown or OHLC-only method, a non-positive or non-finite price, or a non-positive annualization factor.
   */
  realizedVariance(
    prices: NumericArray,
    method?: string | null,
    annualizationFactor?: number | null
  ): number;
  /**
   * Annualized realized variance from OHLC bars (Rust `stats::realized_variance_ohlc`).
   *
   * @param open - Opening prices, one per bar.
   * @param high - High prices, one per bar.
   * @param low - Low prices, one per bar.
   * @param close - Closing prices, one per bar.
   * @param method - Estimator name: `"close_to_close"`, `"parkinson"`, `"garman_klass"`, `"rogers_satchell"` or `"yang_zhang"`. Omitted selects the Rust OHLC default (`"yang_zhang"`).
   * @param annualizationFactor - Bars per year (for example `252` for daily bars); omitted selects the Rust daily default (252).
   * @returns Annualized realized variance (decimal, not volatility).
   * @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) for an unknown method, series of different lengths, an invalid bar, or a non-positive annualization factor.
   */
  realizedVarianceOhlc(
    open: NumericArray,
    high: NumericArray,
    low: NumericArray,
    close: NumericArray,
    method?: string | null,
    annualizationFactor?: number | null
  ): number;
  /**
   * Standard normal CDF Φ(x).
   * @param x - Real-valued point at which to evaluate Φ; any finite or infinite `x` is accepted.
   * @returns Probability in `(0, 1)` for finite `x`, with the usual ±∞ limits.
   */
  normCdf(x: number): number;
  /**
   * Standard normal PDF φ(x).
   * @param x - Real-valued point at which to evaluate φ.
   * @returns Density at `x`; φ(0) is `1/sqrt(2π)`.
   */
  normPdf(x: number): number;
  /**
   * Inverse standard normal CDF Φ⁻¹(p).
   * @param p - Probability input strictly between 0 and 1 for the inverse normal distribution.
   * @returns Standard-normal quantile for probability `p`.
   */
  standardNormalInvCdf(p: number): number;
  /**
   * Error function erf(x).
   * @param x - Real-valued argument to erf; the function is odd, so erf(-x) = -erf(x).
   * @returns erf(x) in `(-1, 1)` for finite `x`.
   */
  erf(x: number): number;
  /**
   * Natural logarithm of the Gamma function ln(Γ(x)).
   * @param x - Real argument; must be positive and away from the non-positive integers.
   * @returns ln(Γ(x)); ln(Γ(1)) is 0 and ln(Γ(n+1)) is ln(n!).
   */
  lnGamma(x: number): number;
  /**
   * Kahan compensated summation over a typed numeric array.
   * @param values - Finite numeric terms in summation or scan order.
   * @returns Compensated sum of `values` in input order.
   */
  kahanSum(values: NumericArray): number;
  /**
   * Neumaier compensated summation over a typed numeric array.
   * @param values - Finite numeric terms in summation or scan order.
   * @returns Compensated sum of `values`, robust to mixed-sign cancellation.
   */
  neumaierSum(values: NumericArray): number;
  /**
   * Count the longest consecutive run of strictly positive values in a typed array.
   * @param values - Finite numeric terms in summation or scan order.
   * @returns Length of the longest run of strictly positive observations.
   */
  longestPositiveRun(values: NumericArray): number;
}

/**
 * Namespaced TypeScript entry point for core APIs.
 */
export declare const core: CoreNamespace;

// --- analytics ------------------------------------------------------------

/**
 * JavaScript `number[]` or `Float64Array` accepted by numeric WASM entry points.
 */
export type NumericArray = number[] | Float64Array;
/**
 * Nested numeric arrays represented as an outer collection of numeric vectors.
 * Each API specifies the semantic meaning and required length of the outer and inner dimensions.
 */
export type NumericMatrix = NumericArray[];

/**
 * Dated rolling result returned by per-ticker rolling analytics: the serde
 * form of Rust `DatedSeries`, with `values` as a typed array.
 *
 * Hand-declared because the JavaScript form differs from the generated wire
 * type (`number[]`, with non-finite sentinels): `values` is a `Float64Array`
 * holding `NaN` for an undefined window.
 *
 * Identical keys to Python `DatedSeries.to_json()`.
 */
export interface DatedSeries {
  /**
   * Rolling metric values, one per completed window.
   */
  values: Float64Array;
  /**
   * ISO-8601 window-end dates aligned 1:1 with `values`, in chronological order.
   */
  dates: string[];
  /**
   * Rust `RollingMetric` name of the series: `"volatility"` (`rollingVolatility`),
   * `"sortino"` (`rollingSortino`), `"sharpe"` (`rollingSharpe`) or `"return"`
   * (`rollingReturns`).
   */
  value_column: 'volatility' | 'sortino' | 'sharpe' | 'return';
}

/**
 * Rolling greeks aligned with rolling-window end dates: the serde form of Rust
 * `RollingGreeks`, with `alphas` and `betas` as typed arrays.
 *
 * Hand-declared because the JavaScript form differs from the generated wire
 * type (`number[]`): the numeric columns are `Float64Array`s.
 */
export interface RollingGreeks {
  /**
   * ISO-8601 end dates of each rolling window, in chronological order.
   */
  dates: string[];
  /**
   * Annualized Jensen alpha at each window end, as decimal fractions.
   */
  alphas: Float64Array;
  /**
   * OLS beta at each window end.
   */
  betas: Float64Array;
}

/**
 * Stateful performance analytics engine over a panel of ticker series.
 *
 * `Performance` is the single entry point exposed to JS. Construct from
 * a price matrix (`new Performance(...)`) or a return matrix
 * (`Performance.fromReturns(...)`); every metric is then reachable as
 * an instance method.
 *
 * The `prices` and `returns` supplied to the panel constructors are ticker-major
 * and column-oriented; matrix inputs to other methods follow their parameter
 * documentation.
 *
 * All multi-ticker scalar outputs come back as `number[]` indexed by the
 * panel's ticker order; vector / per-ticker / structured outputs are
 * serialized to plain JS objects (e.g. `DatedSeries`, `BetaResult[]`).
 */
declare class Performance {
  /**
   * Construct from a ticker-major, column-oriented price matrix. The outer
   * element selects a ticker, and each inner series is aligned to `dates`.
   * @param dates - ISO-8601 observation dates in ascending order, with one entry per value in each inner price series.
   * @param prices - Ticker-major, column-oriented matrix where `prices[tickerIdx][dateIdx]` is the price for `tickerIdx` at `dates[dateIdx]`.
   * @param tickerNames - Ticker labels aligned with the outer elements of `prices`.
   * @param benchmarkTicker - Optional ticker label to use as the benchmark return series.
   * @param frequency - Optional observation frequency token; defaults to daily.
   * @throws Error - Rejects malformed dates or matrices, invalid prices, unsupported frequencies, and an unknown benchmark ticker.
   */
  constructor(
    dates: string[],
    prices: NumericMatrix,
    tickerNames: string[],
    benchmarkTicker?: string | null,
    frequency?: string
  );
  /**
   * Construct from a ticker-major, column-oriented return matrix. The outer
   * element selects a ticker, and each inner series is aligned to `dates`.
   * @param dates - ISO-8601 observation dates in ascending order, with one entry per value in each inner return series.
   * @param returns - Ticker-major, column-oriented simple decimal return matrix where `returns[tickerIdx][dateIdx]` is the return for `tickerIdx` at `dates[dateIdx]`.
   * @param tickerNames - Ticker labels aligned with the outer elements of `returns`.
   * @param benchmarkTicker - Optional ticker label to use as the benchmark return series.
   * @param frequency - Optional observation frequency token; defaults to daily.
   * @returns A `Performance` handle over the supplied return panel.
   * @throws Error - Rejects malformed dates or matrices and invalid benchmark or frequency inputs.
   */
  static fromReturns(
    dates: string[],
    returns: NumericMatrix,
    tickerNames: string[],
    benchmarkTicker?: string | null,
    frequency?: string
  ): Performance;
  /**
   * Restrict subsequent analytics to `[start, end]`.
   * @param start - Inclusive ISO-8601 start date for the active analysis window.
   * @param end - Inclusive ISO-8601 end date for the active analysis window.
   * @throws Error - Rejects `start` or `end` when it is not a valid ISO-8601 calendar date.
   */
  resetDateRange(start: string, end: string): void;
  /**
   * Change the benchmark ticker.
   * @param ticker - Existing ticker label to use as the benchmark return series.
   * @throws Error - Rejects `ticker` when it does not match a loaded ticker name.
   */
  resetBenchTicker(ticker: string): void;
  /**
   * Ticker names in column order.
   * @returns Ticker labels in column order as a JavaScript string array.
   * @throws Error - Rejects if the ticker-name vector cannot be serialized to JavaScript.
   */
  tickerNames(): string[];
  /**
   * Benchmark column index.
   * @returns Zero-based index of the benchmark ticker in `tickerNames()`.
   */
  benchmarkIdx(): number;
  /**
   * Observation frequency token.
   * @returns Frequency string such as `"daily"` or `"monthly"`.
   */
  frequency(): string;
  /**
   * Full return-aligned date grid as ISO date strings (independent of any active window).
   * @returns Full panel dates as ISO-8601 strings, ignoring any `resetDateRange` window.
   */
  dates(): string[];
  /**
   * Dates of the currently active analysis window as ISO date strings.
   * @returns ISO-8601 dates of the active analysis window, in chronological order.
   */
  activeDates(): string[];
  /**
   * Dates for one ticker's active return series as ISO date strings.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @returns ISO-8601 dates for that ticker's active return series, in chronological order.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns.
   */
  activeDatesForTicker(tickerIdx: number): string[];
  /**
   * Compound annual growth rate per asset.
   *
   * `dayCount` omitted or `"act365_25"` uses Act/365.25 (the Rust
   * `CagrDayCount` default). Other values are core DayCount names such as
   * `"act_365f"` or `"bus_252"`. `bus_252` requires `calendarId`. Labels
   * only: to reuse a `core.DayCount` handle, pass `dayCount.toString()`
   * (Python additionally accepts the `DayCount` object itself).
   * @param dayCount - Optional day-count: `"act365_25"` or a core name such as `"act_365f"`; defaults to Act/365.25.
   * @param calendarId - Optional holiday-calendar id, or `+`-joined ids for a union calendar (`"nyse+gblo"`); required for `bus_252`.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects an unknown day-count (kind `validation`), an unknown calendar id (kind `not_found`, with suggestions), a missing calendar when `bus_252` is requested, or a ticker whose active range has no positive holding period.
   */
  cagr(dayCount?: string, calendarId?: string): Float64Array;
  /**
   * Mean periodic return per asset (annualized by default).
   * @param annualize - Whether to annualize by the configured frequency; defaults to true.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  meanReturn(annualize?: boolean): Float64Array;
  /**
   * Return volatility per asset (annualized by default).
   * @param annualize - Whether to annualize by the configured frequency; defaults to true.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  volatility(annualize?: boolean): Float64Array;
  /**
   * Sharpe ratio per asset for the given risk-free rate.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  sharpe(riskFreeRate?: number): Float64Array;
  /**
   * Sortino ratio; mar is a per-period threshold.
   * @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  sortino(mar?: number): Float64Array;
  /**
   * Calmar ratio (CAGR / |max drawdown|) over the active window, not
   * Young's 36-month CTA definition.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  calmar(): Float64Array;
  /**
   * Maximum drawdown per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  maxDrawdown(): Float64Array;
  /**
   * Mean drawdown per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  meanDrawdown(): Float64Array;
  /**
   * Historical value-at-risk per asset at the given confidence level.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  valueAtRisk(confidence?: number): Float64Array;
  /**
   * Expected shortfall (CVaR) per asset at the given confidence level.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  expectedShortfall(confidence?: number): Float64Array;
  /**
   * Tracking error versus the benchmark per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  trackingError(): Float64Array;
  /**
   * Information ratio versus the benchmark per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  informationRatio(): Float64Array;
  /**
   * Return skewness per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  skewness(): Float64Array;
  /**
   * Excess kurtosis of returns per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  kurtosis(): Float64Array;
  /**
   * Geometric mean return per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  geometricMean(): Float64Array;
  /**
   * Downside deviation; mar is a per-period threshold.
   * @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  downsideDeviation(mar?: number): Float64Array;
  /**
   * Longest drawdown duration in calendar days per asset.
   * @returns Per-ticker longest drawdown duration in calendar days, as a JavaScript number array.
   * @throws Error - Rejects if the duration vector cannot be serialized to JavaScript.
   */
  maxDrawdownDuration(): number[];
  /**
   * Empyrical-style annualized geometric up-capture.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  upCapture(): Float64Array;
  /**
   * Empyrical-style annualized geometric down-capture.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  downCapture(): Float64Array;
  /**
   * Empyrical-style annualized geometric up/down capture ratio.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  captureRatio(): Float64Array;
  /**
   * Omega ratio per asset for the given threshold return.
   * @param threshold - Per-period threshold return as a decimal; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  omegaRatio(threshold?: number): Float64Array;
  /**
   * Treynor ratio per asset for the given risk-free rate.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  treynor(riskFreeRate?: number): Float64Array;
  /**
   * Gain-to-pain ratio per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  gainToPain(): Float64Array;
  /**
   * Ulcer index per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  ulcerIndex(): Float64Array;
  /**
   * Martin ratio (excess return over ulcer index) per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  martinRatio(): Float64Array;
  /**
   * Recovery factor (total return over max drawdown) per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  recoveryFactor(): Float64Array;
  /**
   * Pain index (mean drawdown magnitude) per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  painIndex(): Float64Array;
  /**
   * Pain ratio (excess return over pain index) per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  painRatio(riskFreeRate?: number): Float64Array;
  /**
   * Tail ratio of upper to lower return quantiles per asset.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  tailRatio(confidence?: number): Float64Array;
  /**
   * R-squared of returns against the benchmark per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  rSquared(): Float64Array;
  /**
   * Share of periods beating the benchmark per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  battingAverage(): Float64Array;
  /**
   * Equal-weight Gaussian value-at-risk per asset.
   *
   * `horizonPeriods` omitted is one-period VaR. A positive `h` scales
   * mean by `h` and volatility by `√h`.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @param horizonPeriods - Optional horizon in observation periods; omitted is one-period VaR.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  parametricVar(confidence?: number, horizonPeriods?: number): Float64Array;
  /**
   * Cornish-Fisher adjusted value-at-risk per asset.
   *
   * `horizonPeriods` omitted is one-period VaR. A positive `h` scales
   * Cornish–Fisher moments to that horizon.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @param horizonPeriods - Optional horizon in observation periods; omitted is one-period VaR.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  cornishFisherVar(confidence?: number, horizonPeriods?: number): Float64Array;
  /**
   * Conditional drawdown-at-risk per asset at the given confidence level.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  cdar(confidence?: number): Float64Array;
  /**
   * M-squared (Modigliani) risk-adjusted return per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  mSquared(riskFreeRate?: number): Float64Array;
  /**
   * Modified Sharpe ratio using annualized excess return and corresponding-annual-horizon Cornish-Fisher VaR per asset.
   *
   * The panel frequency supplies the periods-per-year scaling for both terms, including the horizon decay of skewness and excess kurtosis; the denominator is not one-period VaR.
   * @param riskFreeRate - Annualized decimal risk-free rate, decompounded to the panel frequency before constructing annualized excess return; defaults to 0.0.
   * @param confidence - Annual-horizon tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  modifiedSharpe(riskFreeRate?: number, confidence?: number): Float64Array;
  /**
   * Sterling ratio over the `n` largest drawdowns per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @param n - Finite non-negative integer count of largest drawdowns; defaults to 5. Invalid numeric values are rejected.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  sterlingRatio(riskFreeRate?: number, n?: number): Float64Array;
  /**
   * Burke ratio over the `n` largest drawdowns per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @param n - Finite non-negative integer count of largest drawdowns; defaults to 5. Invalid numeric values are rejected.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  burkeRatio(riskFreeRate?: number, n?: number): Float64Array;
  /**
   * Per-period simple returns per asset, as decimal fractions (0.01 = +1%).
   *
   * Canonical accessor for the raw return panel over the active window; prefer
   * it over `excessReturns` with an all-zero risk-free series or un-compounding
   * `cumulativeReturns`. Series are span-aware and therefore ragged across
   * assets on edge-ragged panels.
   * @returns One Float64Array of simple decimal returns per ticker in `tickerNames()` order.
   */
  returns(): Float64Array[];
  /**
   * Per-period simple returns for one asset, as decimal fractions (0.01 = +1%).
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @returns Simple decimal returns for the selected ticker, in date order.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns.
   */
  returnsForTicker(tickerIdx: number): Float64Array;
  /**
   * Cumulative return series per asset.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   */
  cumulativeReturns(): Float64Array[];
  /**
   * The standard per-ticker summary as a table envelope: one row per ticker
   * with 22 metric columns plus a leading `ticker` dimension column. Same
   * rows as the Python `Performance.to_summary_dataframe`.
   * @returns Column-oriented table envelope of the summary metrics.
   * @param riskFreeRate - Annualized risk-free rate as a decimal (0.02 = 2%); affects only `sharpe`. Defaults to 0.0.
   * @param confidence - Tail confidence as a decimal probability applied to VaR, ES and tail ratio; defaults to 0.95.
   * @throws Error - Throws a JavaScript exception if the panel is too short to annualize (`cagr` / `calmar`) or the table cannot be converted.
   */
  summary(riskFreeRate?: number | null, confidence?: number | null): TableEnvelope;
  /**
   * Calendar-bucketed compounded returns for every ticker.
   *
   * The outer array is ticker-major in `tickerNames()` order. Each inner
   * array contains chronological Rust `PeriodicReturn` points. Chaining one ticker's
   * decimal `value` fields reconciles with its final `cumulativeReturns()`
   * value.
   *
   * @example
   * ```ts
   * const perf = Performance.fromReturns(
   *   ["2024-01-01", "2024-01-02"],
   *   [[0.01, 0.02]],
   *   ["FUND"],
   * );
   * const [[point]] = perf.periodicReturns();
   * console.log(point.date, point.value); // "2024-01-02", 0.0302
   * ```
   * @param frequency - Optional calendar frequency token: `"daily"`, `"weekly"`, `"monthly"`, `"quarterly"`, `"semi_annual"`, or `"annual"` (pandas offset aliases `D`/`B`, `W`, `M`, `Q`, `A`/`Y` are accepted too); defaults to `"monthly"`.
   * @returns Ticker-major nested arrays of chronological Rust `PeriodicReturn` `{ date, value }` points with simple decimal returns.
   * @throws Error - Rejects an unsupported frequency or a panel that cannot be serialized to JavaScript.
   */
  periodicReturns(frequency?: string): PeriodicReturn[][];
  /**
   * Drawdown series per asset.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   */
  drawdownSeries(): Float64Array[];
  /**
   * Return correlation matrix across assets.
   *
   * Uses the complete-case common window when every ticker has at least
   * two overlapping points; otherwise pairwise intersecting spans, then
   * Higham repair.
   * @returns Square correlation matrix as nested Float64Array rows in `tickerNames()` order.
   * @throws Error - Rejects a degenerate pair or a matrix that cannot be repaired to a valid correlation matrix.
   */
  correlationMatrix(): Float64Array[];
  /**
   * `true` when `correlationMatrix()` had to be Higham-repaired to the nearest valid correlation matrix (ragged panels can yield a raw pairwise estimate that is not positive semi-definite).
   * @returns `true` when the estimate was projected to the nearest correlation matrix.
   * @throws Error - Rejects the same degenerate-pair conditions as `correlationMatrix`.
   * @throws Error - Rejects when a ticker pair is degenerate or Higham repair fails.
   */
  correlationMatrixRepaired(): boolean;
  /**
   * Cumulative outperformance versus the benchmark per asset.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   */
  cumulativeReturnsOutperformance(): Float64Array[];
  /**
   * Difference between asset and benchmark drawdown series.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   */
  drawdownDifference(): Float64Array[];
  /**
   * Excess returns over the supplied risk-free series per asset.
   *
   * `rf` must have one value per active panel date. `nperiods` omitted
   * geometrically decompounds an annual series using the engine frequency;
   * pass `1` when `rf` is already periodic.
   * @param rf - Risk-free return series as decimal values aligned with active panel dates.
   * @param nperiods - Optional periods per year used to decompound annual `rf`; omit to use the engine frequency, or pass `1` for already-periodic `rf`.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   * @throws Error - Rejects when `rf` is neither a numeric JavaScript array nor a `Float64Array`, or when its length differs from the active panel.
   */
  excessReturns(rf: NumericArray, nperiods?: number): Float64Array[];
  /**
   * OLS beta versus the benchmark per asset, with standard error and 95% CI.
   * @returns Per-ticker `{ beta, std_err, ci_lower, ci_upper }` objects in `tickerNames()` order.
   * @throws Error - Rejects if the beta results cannot be serialized to JavaScript.
   */
  beta(): BetaResult[];
  /**
   * Benchmark regression annualized Jensen alpha/beta statistics per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker `{ alpha, beta, r_squared, adjusted_r_squared }` objects in `tickerNames()` order.
   * @throws Error - Rejects if the regression results cannot be serialized to JavaScript.
   */
  greeks(riskFreeRate?: number): GreeksResult[];
  /**
   * Rolling benchmark annualized Jensen alpha/beta for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns `{ dates, alphas, betas }` series for the selected ticker.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created.
   */
  rollingGreeks(tickerIdx: number, window?: number, riskFreeRate?: number): RollingGreeks;
  /**
   * Rolling volatility series for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
   * @returns `{ values, dates, value_column }` series for the selected ticker; `value_column` is `"volatility"`.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created.
   */
  rollingVolatility(tickerIdx: number, window?: number): DatedSeries;
  /**
   * Rolling Sortino ratio series for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
   * @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
   * @returns `{ values, dates, value_column }` series for the selected ticker; `value_column` is `"sortino"`.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created.
   */
  rollingSortino(tickerIdx: number, window?: number, mar?: number): DatedSeries;
  /**
   * Rolling Sharpe ratio series for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns `{ values, dates, value_column }` series for the selected ticker; `value_column` is `"sharpe"`.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created.
   */
  rollingSharpe(tickerIdx: number, window?: number, riskFreeRate?: number): DatedSeries;
  /**
   * Rolling compounded return series for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count. Zero, fractional, non-finite, and out-of-range values are rejected.
   * @returns `{ values, dates, value_column }` series for the selected ticker; `value_column` is `"return"`.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created. An overlong `window` returns an empty series; a zero window is rejected.
   */
  rollingReturns(tickerIdx: number, window: number): DatedSeries;
  /**
   * Details of the `n` largest drawdown episodes for one asset.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param n - Finite non-negative integer count of episodes; defaults to 5. Invalid numeric values are rejected.
   * @returns Drawdown episode objects for the selected ticker, largest first.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the drawdown details cannot be serialized to JavaScript.
   */
  drawdownDetails(tickerIdx: number, n?: number): DrawdownEpisode[];
  /**
   * Multi-factor regression statistics for one asset.
   *
   * Factor series are already-excess. `returnKind` `"excess"` leaves the
   * ticker series unchanged; `"total"` subtracts the geometrically
   * decompounded period risk-free rate from the ticker series only.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param factorReturns - Matrix of aligned already-excess decimal factor-return series, one row per factor.
   * @param returnKind - `"excess"` or `"total"`; defaults to `"excess"`.
   * @param riskFreeRate - Annualized decimal risk-free rate used when `returnKind` is `"total"`; defaults to 0.0.
   * @returns `{ alpha, betas, r_squared, adjusted_r_squared, residual_vol }` for the selected ticker.
   * @throws Error - Rejects a non-numeric `factor_returns` matrix, an unknown `returnKind`, an out-of-range `ticker_idx`, no factors, too few observations, non-finite or length-mismatched inputs, a singular factor design, or a result that cannot be serialized to JavaScript.
   */
  multiFactorGreeks(
    tickerIdx: number,
    factorReturns: NumericMatrix,
    returnKind?: string,
    riskFreeRate?: number
  ): MultiFactorResult;
  /**
   * Period-to-date lookback returns.
   *
   * FYTD is the first observation on or after the fiscal calendar start
   * through `refDate`. Holidays are not skipped. The first included
   * simple return still spans the prior close.
   * @param refDate - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end.
   * @param fiscalYearStartMonth - Optional fiscal-year start month from 1 through 12; defaults to January. With both parts omitted the fiscal year is the calendar year.
   * @param fiscalYearStartDay - Optional fiscal-year start day; defaults to the first day of the month (Rust `FiscalConfig::from_parts`).
   * @param refDate - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end.
   * @param fiscalYearStartMonth - Optional fiscal-year start month from 1 through 12; defaults to January.
   * @param fiscalYearStartDay - Optional fiscal-year start day; defaults to the first day.
   * @returns Per-ticker `{ mtd, qtd, ytd, fytd }` numeric arrays of lookback returns as decimal fractions; `fytd` is never null.
   * @throws Error - Rejects an invalid ISO `ref_date`, a fiscal month outside `1..=12`, a fiscal day outside `1..=31`, or a result that cannot be serialized to JavaScript.
   */
  lookbackReturns(
    refDate: string,
    fiscalYearStartMonth?: number,
    fiscalYearStartDay?: number
  ): LookbackReturns;
  /**
   * Aggregated period statistics for one asset at the given frequency.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param aggregationFrequency - Optional aggregation frequency token; defaults to monthly.
   * @param fiscalYearStartMonth - Optional fiscal-year start month from 1 through 12; when only the day is given the month is January.
   * @param fiscalYearStartDay - Optional fiscal-year start day within the selected month; when only the month is given the day is the 1st.
   * @returns Period statistics object for the selected ticker at the requested frequency.
   * @throws Error - Rejects an unsupported `aggregation_frequency`, a fiscal month outside `1..=12`, a fiscal day outside `1..=31`, an out-of-range `ticker_idx`, or period statistics that cannot be serialized to JavaScript.
   */
  periodStats(
    tickerIdx: number,
    aggregationFrequency?: string,
    fiscalYearStartMonth?: number,
    fiscalYearStartDay?: number
  ): PeriodStats;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Namespaced TypeScript entry points for analytics calculations and types.
 * @example
 * ```typescript
 * import init, { analytics } from "finstack-quant-wasm";
 * await init();
 * const factorReturns = analytics.constrainedLeastSquares(
 *   [1, 2],
 *   1,
 *   [0.01, 0.02],
 *   [0.5, 0.5]
 * );
 * console.log(factorReturns[0]);
 * ```
 */
export interface AnalyticsNamespace {
  /**
   * `Performance` is the single entry point for analytics on a panel of
   * ticker series. Construct from prices (`new Performance(...)`) or from
   * returns (`Performance.fromReturns(...)`); every metric — return/risk
   * scalars, drawdown statistics, rolling windows, periodic returns
   * (MTD/QTD/YTD/FYTD), benchmark alpha/beta, basic factor models — is a
   * method on the resulting instance.
   */
  Performance: typeof Performance;
  /**
   * Fit factor returns satisfying the equality constraint `w'Xf = w'r`.
   *
   * Binds Rust `constrained_least_squares` (Jeet & Partani 2023, Appendix
   * A): adds the minimal Lagrangian correction to an unconstrained OLS fit
   * so the corrected factor returns exactly reproduce the weighted
   * realized return `w'r`. Typically used to fit the benchmark factor
   * returns consumed by `portfolio.factorBrinsonAttribution`, which
   * requires factor returns satisfying that same completeness condition.
   * @param exposures - Row-major factor exposure matrix, `n_assets x n_factors`: asset i's exposure to factor j is `exposures[i * n_factors + j]`.
   * @param nFactors - Number of factor columns in `exposures`; a non-negative whole number no greater than `4294967295` (Rust rejects `0` as invalid input).
   * @param returns - Realized asset returns, length `n_assets` (defines `n_assets`).
   * @param weights - Holding weights whose weighted return `w'r` must be fully reproduced by `w'Xf` (e.g. benchmark weights for a benchmark-return attribution).
   * @returns Constrained factor returns `f`, one per factor, satisfying `w'Xf = w'r` to numerical precision.
   * @throws Error - A `TypeError` (kind `invalid_type`) if `nFactors` is not a finite, non-negative whole number within the WebAssembly `usize` range. A validation error from Rust if `nFactors` is zero or `returns` is empty; if vector dimensions are inconsistent (including an overflowing `n_assets * n_factors`); if any vector value is non-finite; if the design matrix is rank-deficient; if coefficient rescaling or the constraint correction produces a non-finite value; or if the correction direction is degenerate and OLS does not already satisfy the constraint.
   */
  constrainedLeastSquares(
    exposures: NumericArray,
    nFactors: number,
    returns: NumericArray,
    weights: NumericArray
  ): Float64Array;
  /**
   * Sharpe ratio of one return series (annualized excess mean over
   * annualized sample volatility; the same kernel as `Performance.sharpe`).
   * @param returns - Per-period simple decimal returns in date order.
   * @param rf - Annualized risk-free rate as a decimal (`0.02` for 2%); defaults to `0`.
   * @param periodsPerYear - Observations per year used to annualize; defaults to `252`.
   * @returns The Sharpe ratio; `±Infinity` when volatility is zero with a non-zero excess return, `NaN` for fewer than two observations, non-finite inputs, or an invalid `periodsPerYear`.
   * @throws Error - Rejects a `returns` value that is not a numeric array.
   */
  sharpe(returns: NumericArray, rf?: number, periodsPerYear?: number): number;
  /**
   * Annualized Sortino ratio of one return series.
   * @param returns - Per-period simple decimal returns in date order.
   * @param mar - Minimum acceptable return per period as a decimal (not annualized); defaults to `0`.
   * @param periodsPerYear - Observations per year used to annualize; defaults to `252`.
   * @returns The Sortino ratio; `±Infinity` with no downside deviation but a non-zero excess mean, `NaN` for fewer than two observations, non-finite inputs, or an invalid `periodsPerYear`.
   * @throws Error - Rejects a `returns` value that is not a numeric array.
   */
  sortino(returns: NumericArray, mar?: number, periodsPerYear?: number): number;
  /**
   * Annualized sample volatility (n−1 denominator) of one return series.
   * @param returns - Per-period simple decimal returns in date order.
   * @param periodsPerYear - Observations per year; the per-period standard deviation is scaled by its square root. Defaults to `252`.
   * @returns Annualized volatility as a decimal; `0` for an empty array, `NaN` for non-finite inputs or an invalid `periodsPerYear`.
   * @throws Error - Rejects a `returns` value that is not a numeric array.
   */
  volatility(returns: NumericArray, periodsPerYear?: number): number;
  /**
   * Maximum peak-to-trough drawdown of one return series.
   * @param returns - Per-period simple decimal returns in date order; they are compounded into a wealth path before the running-peak decline is measured.
   * @returns Non-positive fraction (`-0.25` is a 25% loss); `0` when the series never falls below its running peak or is empty. Non-finite returns or reconstructed drawdowns yield `NaN`.
   * @throws Error - Rejects a `returns` value that is not a numeric array.
   */
  maxDrawdown(returns: NumericArray): number;
}

/**
 * Namespaced TypeScript entry point for analytics APIs.
 */
export declare const analytics: AnalyticsNamespace;

// --- models.factor.credit ----------------------------------------------------

/**
 * Calibrated credit factor hierarchy artifact.
 *
 * Produced by `CreditCalibrator` or deserialized from JSON via `fromJson`.
 * Immutable once constructed.
 */
declare class CreditFactorModel {
  private constructor();
  /**
   * Deserialize and validate a `CreditFactorModel` from JSON.
   * @returns A calibrated `CreditFactorModel` handle.
   * @param json - JSON-serialized CreditFactorModel to deserialize.
   * @throws Error - Throws a `validation` error if the JSON is malformed or fails validation.
   */
  static fromJson(json: JsonInput): CreditFactorModel;
  /**
   * Exact namespaced credit-factor-model schema marker.
   * @returns The string `"finstack_quant.credit_factor_model/1"`.
   */
  readonly schema: string;
  /**
   * Serialize to compact canonical JSON.
   * @returns Canonical JSON string.
   * @throws Error - Throws a JavaScript exception if the model cannot be serialized to JSON.
   */
  toJson(): string;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Deterministic calibrator that produces a `CreditFactorModel`.
 *
 * Configuration and inputs are passed as JSON strings or plain objects.
 */
declare class CreditCalibrator {
  /**
   * Construct a calibrator from a JSON-serialized `CreditCalibrationConfig`.
   * Omitting `configJson` uses the Rust `CreditCalibrationConfig::default()`,
   * as Python's `CreditCalibrator()` does.
   * @param configJson - Optional credit-factor calibration configuration JSON; omitted or `null` uses the Rust `CreditCalibrationConfig::default()`.
   * @throws Error - Throws if `config_json` is not a valid `CreditCalibrationConfig`.
   */
  constructor(configJson?: JsonInput | null);
  /**
   * Run the calibration pipeline and return a `CreditFactorModel`.
   * @returns A calibrated `CreditFactorModel` handle.
   * @param inputsJson - Credit-factor calibration input JSON containing issuers, spreads, and observations.
   * @throws Error - Throws if inputs are structurally invalid or calibration fails.
   */
  calibrate(inputsJson: JsonInput): CreditFactorModel;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Snapshot of all hierarchy-level factor values at a single date.
 *
 * Produced by `decomposeLevels`. Pass to `decomposePeriod` to compute
 * period-over-period changes.
 */
declare class LevelsAtDate {
  private constructor();
  /**
   * Deserialize a hierarchy-level snapshot from canonical JSON.
   * @param json - Canonical `LevelsAtDate` JSON containing `date`, `generic`, `by_level`, and `adder` fields.
   * @param json - Canonical `LevelsAtDate` JSON.
   * @returns A validated `LevelsAtDate` handle.
   * @throws Error - Throws when the JSON is malformed or a numeric field is non-finite.
   */
  static fromJson(json: JsonInput): LevelsAtDate;
  /**
   * Observation date as an ISO-8601 string.
   */
  readonly date: string;
  /**
   * Generic factor level in basis points.
   */
  readonly generic: number;
  /**
   * Number of hierarchy levels.
   */
  readonly nLevels: number;
  /**
   * Return bucket values for a zero-based hierarchy level.
   * @param levelIndex - Zero-based hierarchy level index, which must be less than `nLevels`.
   * @param levelIndex - Zero-based hierarchy level index.
   * @returns A bucket-name to factor-level mapping in basis points.
   * @throws Error - Throws when `level_index` is outside the available levels or the map cannot be converted to a JavaScript object.
   */
  levelValues(levelIndex: number): Record<string, number>;
  /**
   * Return per-issuer residual adders in basis points.
   * @returns An issuer-ID to residual-adder mapping.
   * @throws Error - Throws when the mapping cannot be converted to a JavaScript object.
   */
  adder(): Record<string, number>;
  /**
   * Serialize the snapshot to compact canonical JSON.
   * @returns Canonical JSON string.
   * @throws Error - Throws if any numeric output field is non-finite (NaN/Inf), naming the offending field instead of silently serializing `null`.
   */
  toJson(): string;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Component-wise difference between two `LevelsAtDate` snapshots.
 *
 * Produced by `decomposePeriod`.
 */
declare class PeriodDecomposition {
  private constructor();
  /**
   * Deserialize a period decomposition from canonical JSON.
   * @param json - Canonical `PeriodDecomposition` JSON containing `from`, `to`, `d_generic`, `by_level`, and `d_adder` fields.
   * @param json - Canonical `PeriodDecomposition` JSON.
   * @returns A validated `PeriodDecomposition` handle.
   * @throws Error - Throws when the JSON is malformed or a numeric field is non-finite.
   */
  static fromJson(json: JsonInput): PeriodDecomposition;
  /**
   * Earlier snapshot date as an ISO-8601 string.
   */
  readonly fromDate: string;
  /**
   * Later snapshot date as an ISO-8601 string.
   */
  readonly toDate: string;
  /**
   * Change in the generic factor in basis points.
   */
  readonly dGeneric: number;
  /**
   * Number of hierarchy levels.
   */
  readonly nLevels: number;
  /**
   * Return bucket deltas for a zero-based hierarchy level.
   * @param levelIndex - Zero-based hierarchy level index, which must be less than `nLevels`.
   * @param levelIndex - Zero-based hierarchy level index.
   * @returns A bucket-name to factor-change mapping in basis points.
   * @throws Error - Throws when `level_index` is outside the available levels or the map cannot be converted to a JavaScript object.
   */
  levelDeltas(levelIndex: number): Record<string, number>;
  /**
   * Return per-issuer residual-adder deltas in basis points.
   * @returns An issuer-ID to residual-adder-change mapping.
   * @throws Error - Throws when the mapping cannot be converted to a JavaScript object.
   */
  dAdder(): Record<string, number>;
  /**
   * Serialize the decomposition to compact canonical JSON.
   * @returns Canonical JSON string.
   * @throws Error - Throws if any numeric output field is non-finite (NaN/Inf), naming the offending field instead of silently serializing `null`.
   */
  toJson(): string;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Vol-forecast view over a calibrated `CreditFactorModel`.
 *
 * `VolHorizon::Custom` is intentionally **not** exposed.
 *
 * Horizon strings accepted by `covarianceAt`, `idiosyncraticVol`, and
 * `factorModelAt`:
 * - `"one_step"` — calibrated annualized variance unchanged.
 * - `"unconditional"` — long-run.
 * - `'{"n_steps": N}'` — variance scaled by `N`.
 */
declare class FactorCovarianceForecast {
  /**
   * Wrap a `CreditFactorModel` for vol forecasting.
   * @param model - Calibrated CreditFactorModel used to produce the covariance forecast.
   */
  constructor(model: CreditFactorModel);
  /**
   * Build the factor covariance matrix at the requested horizon.
   * @param horizonJson - JSON-serialized forecast horizon defining the future covariance date or period.
   * @returns Structured covariance matrix with ordered factor axes and row-major data.
   * @throws Error - Throws if the horizon string is invalid or the model data is inconsistent.
   */
  covarianceAt(horizonJson: JsonInput): FactorCovarianceMatrix;
  /**
   * Idiosyncratic vol (std dev) for a specific issuer at the requested horizon.
   * @returns Issuer idiosyncratic volatility as a decimal standard deviation at `horizonJson`.
   * @param issuerId - Stable issuer identifier used to select the required domain object.
   * @param horizonJson - JSON-serialized forecast horizon defining the future covariance date or period.
   * @throws Error - Throws if the issuer is not present in the model's vol state or the calibrated variance is negative.
   */
  idiosyncraticVol(issuerId: string, horizonJson: JsonInput): number;
  /**
   * Build a portfolio-level `FactorModelConfig` at the given horizon and risk
   * measure. Omitting the risk measure uses the Rust `RiskMeasure::default()`
   * (`"variance"`), as Python's `factor_model_at` does.
   * @param horizonJson - JSON-serialized forecast horizon defining the future covariance date or period.
   * @param riskMeasureJson - Optional risk-measure JSON for the horizon factor model; omitted or `null` uses the Rust `RiskMeasure::default()` (`"variance"`).
   * @returns Structured factor-model configuration ready for portfolio risk workflows.
   * @throws Error - Throws if the horizon or risk measure is invalid, or the model builder rejects the assembled configuration.
   */
  factorModelAt(horizonJson: JsonInput, riskMeasureJson?: JsonInput | null): FactorModelConfig;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Namespaced TypeScript entry points for factor model credit calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const config = JSON.stringify({
 *   policy: "globally_off",
 *   hierarchy: { levels: [] },
 *   min_bucket_size_per_level: { per_level: [] },
 *   vol_model: "sample",
 *   covariance_strategy: "diagonal",
 *   beta_shrinkage: "none",
 *   use_returns_or_levels: "returns",
 *   panel_frequency: "monthly",
 *   bucket_weighting: "equal"
 * });
 * const calibrator = new models.factor.credit.CreditCalibrator(config);
 * calibrator.free();
 * ```
 */
export interface FactorModelCreditNamespace {
  /**
   * Calibrated credit-factor hierarchy artifact.
   */
  CreditFactorModel: typeof CreditFactorModel;
  /**
   * Deterministic calibrator that produces a `CreditFactorModel`.
   */
  CreditCalibrator: typeof CreditCalibrator;
  /**
   * Hierarchy-level factor snapshot at a single date.
   */
  LevelsAtDate: typeof LevelsAtDate;
  /**
   * Component-wise difference between two level snapshots.
   */
  PeriodDecomposition: typeof PeriodDecomposition;
  /**
   * Horizon covariance view over a calibrated credit factor model.
   */
  FactorCovarianceForecast: typeof FactorCovarianceForecast;
  /**
   * Decompose observed issuer spreads at a point in time into per-level factor
   * values and per-issuer residual adders.
   *
   * - `model` — calibrated `CreditFactorModel`.
   * - `observed_spreads_json` — JSON `{issuer_id: spread}` map.
   * - `observed_generic` — generic (PC) factor value at `as_of`.
   * - `as_of` — ISO 8601 date string.
   * - `runtime_tags_json` — optional JSON `{issuer_id: {dim_key: tag}}` for
   *   issuers not present in the model artifact.
   *
   * Returns a `LevelsAtDate` handle.
   * @returns Per-level factor values and residual adders at the requested date, in bp.
   * @param model - Calibrated credit factor hierarchy used for the peel.
   * @param observedSpreadsJson - JSON `{issuer_id: spread}` map in decimal (`0.012` = 120 bp). Values that look like bp (e.g. `100.0`) are rejected.
   * @param observedGeneric - Generic (PC) factor value at `as_of`, same decimal convention as the spreads.
   * @param asOf - ISO-8601 valuation date for the snapshot.
   * @param runtimeTagsJson - Optional JSON `{issuer_id: {dim_key: tag}}` for issuers not present in the model artifact.
   * @param model - Calibrated CreditFactorModel used for the peel.
   * @param observedSpreadsJson - JSON `{issuer_id: spread}` map in decimal (`0.012` = 120 bp). Returned levels are bp.
   * @param observedGeneric - Observed generic-market spread in decimal, aligned with the model factors.
   * @param asOf - ISO-8601 valuation date used to stamp the snapshot.
   * @param runtimeTagsJson - Optional runtime-tag JSON for issuers missing from the artifact.
   * @throws Error - Throws a `not_found` error if an issuer has no model row and no `runtime_tags` entry, and a `validation` error if `as_of` cannot be parsed or a spread is outside the decimal band.
   */
  decomposeLevels(
    model: CreditFactorModel,
    observedSpreadsJson: JsonInput,
    observedGeneric: number,
    asOf: string,
    runtimeTagsJson?: JsonInput
  ): LevelsAtDate;
  /**
   * Difference two `LevelsAtDate` snapshots component-wise.
   *
   * Output buckets and issuers are restricted to those present in **both**
   * snapshots so the linear reconciliation invariant on `ΔS_i` holds.
   * @returns Component-wise change between two hierarchy-level snapshots.
   * @param fromLevels - Credit-factor levels at the start of the attribution period.
   * @param toLevels - Credit-factor levels at the end of the attribution period.
   * @throws Error - Throws if `from_levels.date > to_levels.date` or the snapshots disagree on hierarchy depth.
   */
  decomposePeriod(fromLevels: LevelsAtDate, toLevels: LevelsAtDate): PeriodDecomposition;
}

/**
 * Product-independent factor and position risk decomposition kernels.
 *
 * The VaR and risk-budget functions return the canonical Rust result types
 * (`PositionRiskDecomposition`, `RiskBudgetResult`), the same objects the
 * Python functions return; `parametricEsDecomposition` returns the
 * `ParametricEsDecompositionView` reporting view, as in Python.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const result = models.factor.risk.parametricVarDecomposition(
 *   ["A", "B"],
 *   [0.6, 0.4],
 *   [[0.04, 0.01], [0.01, 0.09]],
 * );
 * console.log(result.portfolio_var, result.var_contributions[0].relative_var);
 * ```
 */
export interface FactorRiskNamespace {
  /**
   * Decompose portfolio VaR and ES into position contributions via
   * parametric Euler allocation.
   *
   * Returns the canonical `PositionRiskDecomposition` (the object Python's
   * `parametric_var_decomposition` returns): portfolio VaR/ES (losses
   * negative), `method`, and per-position `var_contributions` and
   * `es_contributions` rows.
   * @returns The canonical `PositionRiskDecomposition` with VaR and ES contributions.
   * @param positionIds - Position identifiers, one per weight.
   * @param weights - Position weights or exposures in portfolio currency.
   * @param covariance - Square position-return covariance matrix as nested rows (`n x n`, row-major).
   * @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::parametric_95()` preset (0.95).
   * @param computeIncremental - Optional; when `true`, also computes incremental VaR (one full repricing per position). Defaults to `false`.
   * @throws Error - Throws a `TypeError` if an argument has the wrong JavaScript type, and a `validation` error if identifier, weight, or covariance dimensions disagree; the covariance matrix is not finite, symmetric, and positive semidefinite; or `confidence` is not finite and in `(0.5, 1)`.
   */
  parametricVarDecomposition(
    positionIds: string[],
    weights: NumericArray,
    covariance: NumericArray[],
    confidence?: number | null,
    computeIncremental?: boolean | null
  ): PositionRiskDecomposition;
  /**
   * Decompose portfolio Expected Shortfall into position contributions via
   * parametric Euler allocation.
   *
   * Returns the `ParametricEsDecompositionView` reporting view (the object
   * Python's `parametric_es_decomposition` returns).
   * @returns The `ParametricEsDecompositionView` with per-position ES rows.
   * @param positionIds - Position identifiers, one per weight.
   * @param weights - Position weights or exposures in portfolio currency.
   * @param covariance - Square position-return covariance matrix as nested rows (`n x n`, row-major).
   * @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::parametric_95()` preset (0.95).
   * @throws Error - Throws a `TypeError` if an argument has the wrong JavaScript type, and a `validation` error if identifier, weight, or covariance dimensions disagree; the covariance matrix is not finite, symmetric, and positive semidefinite; or `confidence` is not finite and in `(0.5, 1)`.
   */
  parametricEsDecomposition(
    positionIds: string[],
    weights: NumericArray,
    covariance: NumericArray[],
    confidence?: number | null
  ): ParametricEsDecompositionView;
  /**
   * Decompose portfolio VaR and Expected Shortfall from per-position scenario
   * profit-and-loss series using historical simulation.
   *
   * `positionPnls` is position-major (one row per position), the Rust
   * layout both hosts share.
   * @returns The canonical `PositionRiskDecomposition`, including the historical `es_contributions` rows.
   * @param positionIds - Position identifiers, one per P&L row.
   * @param positionPnls - Position-major P&L matrix: one row per position, one column per scenario (losses negative).
   * @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::historical_95()` preset (0.95).
   * @throws Error - Throws a `TypeError` if an argument has the wrong JavaScript type, and a `validation` error if the matrix does not have one row per position or its rows have different scenario counts, `confidence` is not finite and in `(0.5, 1)`, too few scenarios resolve the requested tail, or a P&L value is non-finite.
   */
  historicalVarDecomposition(
    positionIds: string[],
    positionPnls: NumericArray[],
    confidence?: number | null
  ): PositionRiskDecomposition;
  /**
   * Evaluate per-position component VaRs against target risk-budget shares.
   * @returns The canonical `RiskBudgetResult` with per-position utilization and excess.
   * @param positionIds - Position identifiers, one per budget row.
   * @param actualVar - Actual component VaR per position, in portfolio currency (loss-signed as the engine reports it).
   * @param targetVarPct - Target share of portfolio VaR per position; non-empty targets must sum to one.
   * @param portfolioVar - Total portfolio VaR used to convert risk-budget shares into absolute amounts.
   * @param utilizationThreshold - Optional actual-to-target risk ratio that flags a budget breach; omit for the Rust default of 1.2.
   * @throws Error - Throws a `TypeError` if an argument has the wrong JavaScript type, and a `validation` error if actual or target arrays do not match the identifier count, a position id is duplicated, non-empty target shares do not sum to one within tolerance, or nonzero component risk is paired with zero `portfolioVar`.
   */
  evaluateRiskBudget(
    positionIds: string[],
    actualVar: NumericArray,
    targetVarPct: NumericArray,
    portfolioVar: number,
    utilizationThreshold?: number | null
  ): RiskBudgetResult;
}

/**
 * Namespaced TypeScript entry points for factor model calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const credit = models.factor.credit;
 * const config = JSON.stringify({
 *   policy: "globally_off",
 *   hierarchy: { levels: [] },
 *   min_bucket_size_per_level: { per_level: [] },
 *   vol_model: "sample",
 *   covariance_strategy: "diagonal",
 *   beta_shrinkage: "none",
 *   use_returns_or_levels: "returns",
 *   panel_frequency: "monthly",
 *   bucket_weighting: "equal"
 * });
 * const calibrator = new credit.CreditCalibrator(config);
 * calibrator.free();
 * ```
 */
export interface FactorNamespace {
  /**
   * Credit factor hierarchy artifacts, calibration, and decomposition.
   */
  credit: FactorModelCreditNamespace;
  /**
   * Pure factor and position risk decomposition kernels.
   */
  risk: FactorRiskNamespace;
}

// --- features ---------------------------------------------------------------

/**
 * Feature observation: a finite number, or `null` for a missing value.
 */
export type FeatureValue = number | null;
/**
 * Operation-specific parameter object passed to feature transforms.
 */
export type FeatureParams = Record<string, unknown>;

/**
 * Vectorized panel feature transforms.
 *
 * `values` accepts finite numbers or `null`; non-finite values are treated as
 * missing by the Rust crate. Time-series transforms are grouped by `entity` and
 * sorted by `order`; cross-sectional transforms partition by `timeKey`.
 * @example
 * ```typescript
 * import init, { features } from "finstack-quant-wasm";
 * await init();
 * const changes = features.transformTimeseries(
 *   [1, 3, 6],
 *   ["ACME", "ACME", "ACME"],
 *   ["2026-01-01", "2026-01-02", "2026-01-03"],
 *   "diff"
 * );
 * console.log(changes);
 * ```
 */
export interface FeaturesNamespace {
  /**
   * Transform a time-series panel column per entity.
   * Rolling windows count rows including gaps; min_periods counts finite inputs.
   * Aggregates may emit at missing rows. EWMA span must be at least 1; mature
   * constant series have zero volatility. String keys need consistent UTC and precision.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Observation-order key used to sort each entity time series.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object. `rolling_sharpe` accepts optional `risk_free` (default `0.0`, same units as the return series).
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, an unsupported `op`, malformed operation parameters, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
   */
  transformTimeseries(
    values: FeatureValue[],
    entity: string[],
    order: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Transform a cross-section per timestamp.
   * cap_weights constrains final absolute weights with zero net and unit gross
   * exposure, preserving centered-signal signs; infeasible caps fail.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal `values` and `time_key` lengths, an unsupported `op`, malformed operation parameters, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
   */
  transformCrossSectional(
    values: FeatureValue[],
    timeKey: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Transform a cross-section within each time/group sub-partition.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param groups - Group labels aligned with values for within-group cross-sectional operations.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal `values`, `time_key`, and `groups` lengths, an unsupported `op`, malformed operation parameters, or a result that cannot be serialized to JavaScript.
   */
  transformCrossSectionalGrouped(
    values: FeatureValue[],
    timeKey: string[],
    groups: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Remove cross-sectional exposure effects by OLS residualization.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, exposure columns whose lengths differ from `values`, a non-boolean `fit_intercept`, a singular or underdetermined cross-section, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
   */
  neutralize(
    values: FeatureValue[],
    timeKey: string[],
    exposures: FeatureValue[][],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Transform two time-series panel columns per entity.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param other - Second value series aligned with the primary series for a pairwise transformation.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Lexicographic observation-order key; use ISO-8601 for calendar chronology.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object. `window` counts rows including gaps; `min_periods <= window` counts complete pairs.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, an unsupported `op`, non-positive or non-integer `window` or `min_periods` parameters, or a result that cannot be serialized to JavaScript.
   */
  transformTimeseriesPairwise(
    values: FeatureValue[],
    other: FeatureValue[],
    entity: string[],
    order: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Return rolling OLS residuals per entity.
   * window counts rows including gaps; min_periods counts complete rows.
   * Missing current responses or exposures and rank-deficient windows yield null.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Observation-order key used to sort each entity time series.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, exposure columns whose lengths differ from `values`, malformed `window`, `min_periods`, or `fit_intercept` parameters, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
   */
  rollingRegressionResidual(
    values: FeatureValue[],
    exposures: FeatureValue[][],
    entity: string[],
    order: string[],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Convert a signal to inverse-risk-scaled weights per timestamp.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric signal observations aligned with `timeKey` and `volatility`.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param volatility - Row-aligned nonnegative risk estimates in a common horizon and units, used as `signal / volatility`; negative estimates fail, while zero, missing, or non-finite values yield missing weights.
   * @throws Error - Rejects inputs that cannot be decoded into the declared arrays, unequal `values`, `time_key`, and `volatility` lengths, negative volatility, or a result that cannot be serialized to JavaScript.
   */
  riskScaledWeights(
    values: FeatureValue[],
    timeKey: string[],
    volatility: FeatureValue[]
  ): FeatureValue[];
  /**
   * Convert ranks into long/short weights.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @throws Error - Rejects inputs that cannot be decoded into the declared arrays, unequal `values` and `time_key` lengths, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
   */
  rankToWeights(values: FeatureValue[], timeKey: string[]): FeatureValue[];
  /**
   * Neutralize a signal and z-score residuals.
   * fit_intercept must be true (the default) to preserve exposure neutrality.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, exposure columns whose lengths differ from `values`, a false or non-boolean `fit_intercept`, or a result that cannot be serialized to JavaScript.
   */
  neutralizeAndZscore(
    values: FeatureValue[],
    timeKey: string[],
    exposures: FeatureValue[][],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Apply a JSON panel transform pipeline.
   * @returns JSON panel after applying the transform pipeline.
   * @param specJson - Canonical panel-transformation JSON. Each operation may set optional `input` (`undefined` default: previous column, or raw `values` for the first op).
   * @throws Error - Rejects malformed JSON or panel specifications, blank, reserved (`values`), or duplicate operation names, unknown `input` columns, missing partition columns, unequal row counts, malformed operation parameters, operations that cannot be evaluated, non-finite arithmetic, or a result that cannot be serialized to JSON.
   */
  transformPanelJson(specJson: JsonInput): string;
}

/**
 * Namespaced TypeScript entry point for features APIs.
 */
export declare const features: FeaturesNamespace;

// --- models.correlation -------------------------------------------------

/**
 * Concrete copula model for portfolio default correlation.
 */
export interface Copula extends WasmOwned {
  /**
   * Number of systematic factors in the model.
   */
  readonly numFactors: number;
  /**
   * Model name for diagnostics.
   */
  readonly modelName: string;
  /**
   * Conditional default probability given factor realization(s).
   * @returns Conditional default probability in `[0, 1]`.
   * @param defaultThreshold - Latent-variable default threshold corresponding to the marginal default probability.
   * @param factorRealization - Realized systematic-factor value conditioning the default probability.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @throws Error - Throws a JavaScript exception if the factor count does not match the copula, any input is non-finite, `correlation` is outside `[0, 1]`, or the model produces a probability outside `[0, 1]`.
   */
  conditionalDefaultProb(
    defaultThreshold: number,
    factorRealization: number[],
    correlation: number
  ): number;
  /**
   * Strict lower-tail dependence coefficient `λ_L` at the given
   * correlation.
   *
   * Returns `NaN` when the model has no closed-form `λ_L` (Random Factor
   * Loading); check `Number.isNaN()` before using the result. For the
   * RFL heuristic stress gauge use `stressCorrelationProxy` instead.
   * @returns Lower-tail dependence `λ_L` in `[0, 1]`, or `NaN` when the copula has no closed form.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   */
  tailDependence(correlation: number): number;
  /**
   * Heuristic stress-correlation proxy for the Random Factor Loading
   * copula.
   *
   * This is **not** the strict copula lower-tail-dependence coefficient
   * `λ_L` (which has no closed form for RFL — `tailDependence` returns
   * `NaN`). It gauges the extra correlation mass in the high-loading
   * tail and vanishes in the Gaussian (`loadingVol = 0`) limit.
   *
   * Throws for non-RFL copulas.
   * @returns Heuristic extra correlation mass in the high-loading tail; 0 in the Gaussian limit.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @throws Error - Throws a JavaScript exception if this copula is not a Random Factor Loading model.
   */
  stressCorrelationProxy(correlation: number): number;
}

/**
 * Copula model specification for configuration and deferred construction.
 */
export interface CopulaSpec extends WasmOwned {
  /**
   * True if this is a Gaussian spec.
   */
  readonly isGaussian: boolean;
  /**
   * True if this is a Student-t spec.
   */
  readonly isStudentT: boolean;
  /**
   * True if this is a Random Factor Loading spec.
   */
  readonly isRfl: boolean;
  /**
   * True if this is a Multi-factor spec.
   */
  readonly isMultiFactor: boolean;
  /**
   * Build a concrete copula from this specification.
   * @returns A concrete `Copula` handle.
   * @throws Error - Throws a JavaScript exception if a Student-t specification contains non-finite degrees of freedom or a value at most two.
   */
  build(): Copula;
}

/**
 * Copula model specification for configuration and deferred construction.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const copula = models.correlation.CopulaSpec.gaussian().build();
 * console.log(copula.modelName, copula.numFactors);
 * ```
 */
export interface CopulaSpecConstructor {
  /**
   * JavaScript prototype of `CopulaSpec`; instances come from the static factories, not `new`.
   */
  readonly prototype: CopulaSpec;
  /**
   * One-factor Gaussian copula (market standard).
   * @returns A `CopulaSpec` handle for deferred construction.
   */
  gaussian(): CopulaSpec;
  /**
   * Student-t copula with specified degrees of freedom (must be > 2).
   * @returns A `CopulaSpec` handle for deferred construction.
   * @param degreesOfFreedom - Student-t degrees of freedom controlling tail thickness; finite and strictly greater than two.
   * @throws Error - Throws a JavaScript exception if `degreesOfFreedom` is not finite and strictly greater than two.
   */
  studentT(degreesOfFreedom: number): CopulaSpec;
  /**
   * Random Factor Loading copula with stochastic correlation.
   * @returns A `CopulaSpec` handle for deferred construction.
   * @param loadingVol - Standard deviation used to randomize the factor loading.
   */
  randomFactorLoading(loadingVol: number): CopulaSpec;
  /**
   * Two-factor Gaussian copula with global and shared-sector factors.
   * @returns A `CopulaSpec` handle for deferred construction.
   */
  multiFactor(): CopulaSpec;
}

/**
 * Concrete recovery model for credit portfolio pricing.
 */
export interface RecoveryModel extends WasmOwned {
  /**
   * Expected (unconditional, Jensen-corrected) recovery rate.
   */
  readonly expectedRecovery: number;
  /**
   * Loss given default (1 − recovery).
   */
  readonly lgd: number;
  /**
   * Recovery-rate volatility scale (0 for constant models).
   */
  readonly recoveryVolatility: number;
  /**
   * Whether recovery varies with the market factor.
   */
  readonly isStochastic: boolean;
  /**
   * Model name for diagnostics.
   */
  readonly modelName: string;
  /**
   * Recovery conditional on the systematic market factor.
   * @returns Conditional recovery rate as a fraction of par in `[0, 1]`.
   * @param marketFactor - Realized standardized market factor used to condition recovery or loss given default.
   */
  conditionalRecovery(marketFactor: number): number;
  /**
   * Conditional LGD given market factor.
   * @returns Conditional loss-given-default as a fraction of par in `[0, 1]`.
   * @param marketFactor - Realized standardized market factor used to condition recovery or loss given default.
   */
  conditionalLgd(marketFactor: number): number;
}

/**
 * Recovery model specification for configuration and deferred construction.
 */
export interface RecoverySpec extends WasmOwned {
  /**
   * Location-parameter recovery rate of this spec.
   *
   * For a constant spec this is the constant rate. For a
   * market-correlated spec this returns the `mean` input — the target
   * recovery at factor `Z = 0` — which differs from the Jensen-corrected
   * unconditional mean `E_Z[R(Z)]` whenever the factor sensitivity is
   * non-zero. For the true unconditional mean call
   * `build().expectedRecovery`.
   */
  readonly expectedRecovery: number;
  /**
   * Build a concrete recovery model from this specification.
   * @returns A concrete `RecoveryModel` handle.
   */
  build(): RecoveryModel;
}

/**
 * Recovery model specification for configuration and deferred construction.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const recovery = models.correlation.RecoverySpec.constant(0.4).build();
 * console.log(recovery.expectedRecovery, recovery.lgd);
 * ```
 */
export interface RecoverySpecConstructor {
  /**
   * JavaScript prototype of `RecoverySpec`; instances come from the static factories, not `new`.
   */
  readonly prototype: RecoverySpec;
  /**
   * Constant recovery rate.
   *
   * Throws if `rate` is not finite or lies outside `[0, 1]`.
   * @returns A `RecoverySpec` handle for deferred construction.
   * @param rate - Constant recovery rate expressed as a fraction from 0 through 1.
   * @throws Error - Throws a JavaScript exception if `rate` is not finite or lies outside `[0, 1]`.
   */
  constant(rate: number): RecoverySpec;
  /**
   * Market-correlated (Andersen-Sidenius) stochastic recovery.
   *
   * Throws if `mean` is not finite or lies outside `[0, 1]`, or if `vol` /
   * `correlation` are not finite.
   * @returns A `RecoverySpec` handle for deferred construction.
   * @param mean - Mean recovery rate expressed as a fraction from 0 through 1.
   * @param vol - Recovery-rate volatility scale in the correlated recovery model.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @throws Error - Throws a JavaScript exception if `mean` is not finite or lies outside `[0, 1]`, or if `vol` or `correlation` is non-finite. Finite volatility and correlation inputs are clamped to their supported ranges.
   */
  marketCorrelated(mean: number, vol: number, correlation: number): RecoverySpec;
  /**
   * Market-standard stochastic recovery (40% mean, 25% vol, +40% corr —
   * recovery falls in stress under the canonical low-factor-stress
   * convention).
   * @returns A `RecoverySpec` handle for deferred construction.
   */
  marketStandardStochastic(): RecoverySpec;
}

/**
 * Exported class; construct instances via `CopulaSpec.build()` (no public `new`).
 */
export interface CopulaClass {
  /**
   * JavaScript prototype of `Copula`; construct instances via `CopulaSpec.build()`.
   */
  readonly prototype: Copula;
}

/**
 * Exported class; construct instances via `RecoverySpec.build()` (no public `new`).
 */
export interface RecoveryModelClass {
  /**
   * JavaScript prototype of `RecoveryModel`; construct instances via `RecoverySpec.build()`.
   */
  readonly prototype: RecoveryModel;
}

/**
 * Portfolio credit-loss distribution with loss-positive VaR and expected
 * shortfall (Rust and Python `PortfolioLossResult`).
 */
export interface PortfolioLossResult extends WasmOwned {
  /**
   * Loss per simulated path, in path order.
   */
  readonly losses: Float64Array;
  /**
   * Arithmetic mean path loss.
   */
  readonly expectedLoss: number;
  /**
   * Loss-positive nearest-rank VaR at `confidence` (larger is worse).
   */
  readonly var: number;
  /**
   * Probability-weighted mean loss in the worst `1 - confidence` tail.
   */
  readonly expectedShortfall: number;
  /**
   * Confidence used for `var` and `expectedShortfall`, in `(0, 1)`.
   */
  readonly confidence: number;
  /**
   * Tranche loss statistics for one attachment/detachment pair.
   *
   * `attachment` and `detachment` are fractions of pool notional in `[0, 1]` —
   * a 0-3% equity tranche is `(0.0, 0.03)`, not `(0.0, 3.0)`. Each path's pool
   * loss fraction `L = loss / poolNotional` maps through
   * `clamp(L - attachment, 0, width) / width`, and the resulting distribution
   * is aggregated at this result's own `confidence`.
   * @returns The tranche notional, expected loss, VaR, expected shortfall, and breach probabilities.
   * @param attachment - Lower tranche boundary as a fraction of pool notional from 0 through 1.
   * @param detachment - Upper tranche boundary as a fraction of pool notional, strictly above the attachment and at most 1.
   * @param poolNotional - Total pool notional, finite and strictly positive, in the same unit as the losses.
   * @throws Error - Throws a `validation` error if the tranche boundaries are invalid, `poolNotional` is not finite and positive, or a derived statistic is non-finite.
   */
  trancheLossStatistics(
    attachment: number,
    detachment: number,
    poolNotional: number
  ): TrancheLossStatistics;
  /**
   * Serialize to the canonical JSON wire format.
   * @returns Canonical `PortfolioLossResult` JSON.
   * @throws Error - Throws a `validation` error if serialization fails.
   */
  toJson(): string;
}

/**
 * Portfolio credit-loss distribution constructors.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const result = models.correlation.PortfolioLossResult.fromLosses([0, 1, 2, 5, 10], 0.75);
 * const equity = result.trancheLossStatistics(0, 0.03, 100);
 * console.log(result.var, equity.expected_loss_fraction);
 * ```
 */
export interface PortfolioLossResultConstructor {
  /**
   * JavaScript prototype of `PortfolioLossResult`; instances come from the static factories, not `new`.
   */
  readonly prototype: PortfolioLossResult;
  /**
   * Aggregate a finite loss distribution under loss-positive conventions.
   *
   * VaR is the nearest-rank loss quantile at `confidence`; expected shortfall
   * averages exactly the worst `1 - confidence` probability mass, with
   * fractional weight on the boundary observation.
   * @param losses - Loss-positive path losses in one caller-defined unit, one entry per simulated path, as a `number[]` or `Float64Array`.
   * @param confidence - Loss-positive VaR and expected-shortfall confidence strictly between 0 and 1.
   * @returns A `PortfolioLossResult` handle.
   * @throws Error - Throws a `TypeError` if `losses` is not an array of numbers, and a `validation` error if the distribution is empty, a loss is non-finite or negative, or `confidence` is outside `(0, 1)`.
   */
  fromLosses(losses: NumericArray, confidence: number): PortfolioLossResult;
  /**
   * Load a result from its canonical JSON form; the losses and confidence are
   * validated and the aggregates recomputed, so a payload whose aggregates
   * disagree with its losses is rejected.
   * @param json - `PortfolioLossResult` JSON (`losses`, `expected_loss`, `var`, `expected_shortfall`, `confidence`), as a string or plain object.
   * @returns A `PortfolioLossResult` handle.
   * @throws Error - Throws a `TypeError` if `json` is neither a string nor a plain object, and a `validation` error if it is malformed or fails the checks above.
   */
  fromJson(json: JsonInput): PortfolioLossResult;
}

/**
 * Namespaced TypeScript entry points for correlation calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const [lower, upper] = models.correlation.correlationBounds(0.1, 0.2);
 * console.log(lower, upper);
 * ```
 */
export interface CorrelationNamespace {
  /**
   * Copula specification constructor for correlation-sensitive pricing.
   */
  CopulaSpec: CopulaSpecConstructor;
  /**
   * Fitted copula handle used by correlation-sensitive pricing.
   */
  Copula: CopulaClass;
  /**
   * Recovery-model specification constructor.
   */
  RecoverySpec: RecoverySpecConstructor;
  /**
   * Recovery-model handle used with copula pricing.
   */
  RecoveryModel: RecoveryModelClass;
  /**
   * Portfolio credit-loss distribution handle with tranche loss statistics.
   */
  PortfolioLossResult: PortfolioLossResultConstructor;
  /**
   * Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.
   *
   * Returns `[rho_min, rho_max]`.
   * @returns `[rho_min, rho_max]` Fréchet-Hoeffding bounds for the two Bernoulli marginals.
   * @param p1 - First marginal default probability from 0 through 1.
   * @param p2 - Second marginal default probability from 0 through 1.
   * @throws Error - Throws a JavaScript exception if either marginal probability is non-finite or outside `[0, 1]`.
   */
  correlationBounds(p1: number, p2: number): Float64Array;
  /**
   * Joint probabilities for two correlated Bernoulli variables.
   *
   * Returns `[p11, p10, p01, p00]`.
   * @returns Joint probabilities `[p11, p10, p01, p00]` for the two correlated Bernoullis.
   * @param p1 - First marginal default probability from 0 through 1.
   * @param p2 - Second marginal default probability from 0 through 1.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @throws Error - Throws a JavaScript exception if either marginal probability is non-finite or outside `[0, 1]`, or `correlation` is non-finite or outside `[-1, 1]`.
   */
  jointProbabilities(p1: number, p2: number, correlation: number): Float64Array;
  /**
   * Validate a flat row-major correlation matrix.
   *
   * Accepts a `Float64Array`/`number[]` of `n * n` row-major entries and
   * checks unit diagonal, off-diagonal in `[-1, 1]`, symmetry, and positive
   * semi-definiteness. Returns nothing on success; raises a descriptive error
   * (including the failing dimension or constraint) otherwise.
   * @param matrix - Flat row-major `n * n` correlation coefficients; unit diagonal, off-diagonals in `[-1, 1]`.
   * @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
   * @throws Error - Throws a JavaScript exception if the flat length is not `n * n`, a diagonal entry is not one, an entry is outside the correlation bounds, the matrix is not symmetric, or the matrix is not positive semidefinite.
   */
  validateCorrelationMatrix(matrix: NumericArray, n: number): void;
  /**
   * Nearest correlation matrix (Higham 2002) for a near-PSD input.
   *
   * Projects a symmetric, near-unit-diagonal, near-PSD matrix onto the set of
   * valid correlation matrices in Frobenius norm. Gross input violations
   * (asymmetry > 1e-6 or diagonal far from 1) throw rather than being silently
   * reshaped. Returns the flat row-major result as a `Float64Array`.
   */
  /**
   * Nearest correlation matrix (Higham 2002).
   *
   * Given a flat row-major `n*n` matrix that is approximately a correlation
   * matrix but fails Cholesky by a small margin, returns the nearest valid
   * correlation matrix (symmetric, unit diagonal, PSD) in Frobenius norm.
   * Gross input violations raise rather than being silently reshaped.
   * @returns Nearest valid correlation matrix as a flat row-major `Float64Array` of `n * n` entries.
   * @param matrix - Flat row-major `n * n` near-correlation matrix to project onto the correlation set.
   * @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
   * @param maxIter - Maximum number of Higham nearest-correlation projection iterations.
   * @param tol - Positive convergence tolerance for the nearest-correlation projection.
   * @throws Error - Throws a `validation` error if the flat length is not `n * n` or the input has a gross diagonal or symmetry violation, and a `computation` error if the projection does not converge within `maxIter` iterations at `tol`.
   */
  nearestCorrelation(matrix: NumericArray, n: number, maxIter?: number, tol?: number): Float64Array;
}

// --- models.monteCarlo ----------------------------------------------------------
// Host-neutral subset shared with Python: Heston Monte Carlo pricing. The
// closed-form Black-Scholes references live at `models.bsPrice`.

/**
 * Namespaced TypeScript entry points for Monte Carlo calculations.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const est = models.monteCarlo.priceHestonCall(
 *   100, 100, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, 1.0, 5000, 42n,
 * );
 * console.log(est.mean.amount, est.mean.currency);
 * // Path count and seed default to the Rust registry values.
 * const byDefault = models.monteCarlo.priceHestonCall(
 *   100, 100, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, 1.0,
 * );
 * console.log(byDefault.num_paths); // 100000
 * ```
 */
export interface MonteCarloNamespace {
  /**
   * Price a European call under Heston stochastic volatility.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param kappa - Mean-reversion speed of variance in the Heston stochastic-volatility model.
   * @param theta - Long-run variance level in the Heston stochastic-volatility model.
   * @param volOfVol - Annualized volatility of variance in the Heston stochastic-volatility model.
   * @param rho - Instantaneous correlation between the asset and variance shocks.
   * @param v0 - Initial instantaneous variance in the Heston stochastic-volatility model.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; omitted or `null` uses the Rust registry European-pricer default (100 000).
   * @param seed - Deterministic random-number seed (number or BigInt); omitted or `null` uses the Rust registry default seed, so results stay reproducible.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @returns The Rust `MoneyEstimate` serde object: `mean` and `ci_95` as `{amount, currency}` money, plus `stderr`, `num_paths`, `num_simulated_paths` and the optional statistics (`null` when not captured).
   * @throws Error - Throws a JavaScript exception if `currency` is unknown; embedded defaults cannot be loaded when `num_steps` is omitted; `rate` or `div_yield` is non-finite; `kappa`, `theta`, `vol_of_vol`, or `v0` is non-finite or non-positive; `rho` is outside `[-1, 1]`; the expiry, step count, path count, or computed discount factor fails validation; a simulated discounted payoff is non-finite; or the result cannot be serialized.
   */
  priceHestonCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    kappa: number,
    theta: number,
    volOfVol: number,
    rho: number,
    v0: number,
    expiry: number,
    numPaths?: number | null,
    seed?: bigint | number | null,
    numSteps?: number | null,
    currency?: string | null
  ): MoneyEstimate;
  /**
   * Price a European put under Heston stochastic volatility.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param kappa - Mean-reversion speed of variance in the Heston stochastic-volatility model.
   * @param theta - Long-run variance level in the Heston stochastic-volatility model.
   * @param volOfVol - Annualized volatility of variance in the Heston stochastic-volatility model.
   * @param rho - Instantaneous correlation between the asset and variance shocks.
   * @param v0 - Initial instantaneous variance in the Heston stochastic-volatility model.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; omitted or `null` uses the Rust registry European-pricer default (100 000).
   * @param seed - Deterministic random-number seed (number or BigInt); omitted or `null` uses the Rust registry default seed, so results stay reproducible.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @returns The Rust `MoneyEstimate` serde object: `mean` and `ci_95` as `{amount, currency}` money, plus `stderr`, `num_paths`, `num_simulated_paths` and the optional statistics (`null` when not captured).
   * @throws Error - Throws a JavaScript exception if `currency` is unknown; embedded defaults cannot be loaded when `num_steps` is omitted; `rate` or `div_yield` is non-finite; `kappa`, `theta`, `vol_of_vol`, or `v0` is non-finite or non-positive; `rho` is outside `[-1, 1]`; the expiry, step count, path count, or computed discount factor fails validation; a simulated discounted payoff is non-finite; or the result cannot be serialized.
   */
  priceHestonPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    kappa: number,
    theta: number,
    volOfVol: number,
    rho: number,
    v0: number,
    expiry: number,
    numPaths?: number | null,
    seed?: bigint | number | null,
    numSteps?: number | null,
    currency?: string | null
  ): MoneyEstimate;
}

/**
 * Namespaced TypeScript entry point for monte carlo APIs.
 */

// --- margin ----------------------------------------------------------------

/**
 * Namespaced TypeScript entry points for margin calculations and types.
 * @example
 * ```typescript
 * import init, { margin } from "finstack-quant-wasm";
 * await init();
 * const csa = margin.csaUsdRegulatoryJson();
 * const vm = margin.calculateVm(csa, 1_000_000, 0, "USD", "2026-01-02");
 * console.log(vm.collect_amount.amount); // exact decimal string
 * ```
 */
export interface MarginNamespace {
  /**
   * Create a standard USD regulatory CSA specification as JSON.
   *
   * Returns the canonical ISDA-compliant CSA for USD OTC derivatives.
   * @returns Canonical ISDA USD regulatory CSA JSON.
   * @throws Error - Rejects if the embedded margin registry cannot be loaded or the resulting CSA cannot be serialized to JSON.
   */
  csaUsdRegulatoryJson(): string;
  /**
   * Create a standard EUR regulatory CSA specification as JSON.
   * @returns Canonical ISDA EUR regulatory CSA JSON.
   * @throws Error - Rejects if the embedded margin registry cannot be loaded or the resulting CSA cannot be serialized to JSON.
   */
  csaEurRegulatoryJson(): string;
  /**
   * Validate a CSA specification JSON string.
   *
   * Validates the JSON schema and CSA semantics, including amount currencies,
   * monetary bounds and calendar lookup. Returns canonical JSON on success.
   * @returns Canonical CSA JSON after schema validation.
   * @param json - CSA specification JSON to validate and normalize into canonical form.
   * @throws Error - Rejects malformed or schema-incompatible `json`, or failure to serialize the decoded CSA specification; also rejects invalid CSA terms or calendar identifiers.
   */
  validateCsaJson(json: JsonInput): string;
  /**
   * Calculate variation margin given exposure, posted collateral, and CSA JSON.
   *
   * Returns the Rust `VmResult` in its canonical serde form (the same wire
   * Python `VmResult.to_json()` emits): `date`, `gross_exposure`,
   * `net_exposure`, `post_amount`, `collect_amount` (each a Money object
   * `{amount, currency}` with a decimal-string amount) and `settlement_date`.
   *
   * @param csaJson - CSA specification JSON governing thresholds, minimum transfer, and timing.
   * @param exposure - Signed mark-to-market in the supplied currency: positive means the counterparty owes the desk.
   * @param postedCollateral - Signed collateral balance: positive held, negative posted, including pending agreed calls.
   * @param currency - ISO-4217 currency code shared by exposure and collateral amounts.
   * @param asOf - ISO-8601 VM calculation date.
   * @returns The canonical `VmResult` as a plain object.
   * @throws Error - Rejects malformed or schema-incompatible `csa_json`, an unknown `currency`, non-finite exposure or collateral amounts, an invalid calendar date, a currency mismatch with the CSA, invalid VM parameters, calendar lookup or settlement-date adjustment failures, or failure to serialize the result.
   */
  calculateVm(
    csaJson: JsonInput,
    exposure: number,
    postedCollateral: number,
    currency: string,
    asOf: string
  ): VmResult;
  /**
   * Compute bilateral XVA: CVA, DVA, FVA, MVA, and the all-in adjustment.
   *
   * All legs are weighted by joint (first-to-default) survival. MVA is computed
   * only when `fundingJson` carries an `im_profile`; that posted IM also reduces
   * ENE for bilateral DVA.
   *
   * The returned object reports the required all-in amount as
   * `total_xva = CVA - DVA + FVA + MVA`. Optional funding legs are absent from
   * the payload when they were not computed.
   *
   * @example
   * ```javascript
   * import init, { core, margin } from "finstack-quant-wasm";
   * await init();
   * const df = new core.DiscountCurve("USD-OIS", "2025-01-01", [0.0, 1.0, 5.0, 1.0], "log_linear");
   * const hz = core.HazardCurve.flat("CPTY", "2025-01-01", 0.02, 0.4);
   * const result = margin.computeBilateralXva(
   *   JSON.stringify({ times: [1, 2], mtm_values: [1e6, 1e6], epe: [1e6, 1e6], ene: [0, 0] }),
   *   hz, hz, df, 0.4, 0.4,
   *   JSON.stringify({ funding_spread_bp: 50.0 }),
   * );
   * result.total_xva; // CVA - DVA + FVA + MVA
   * ```
   * @param exposureProfileJson - Strict `ExposureProfile` JSON with `times`, `mtm_values`, `epe`, and `ene` arrays of equal length and an optional `diagnostics` object (`market_roll_failures`, `valuation_failures`, `total_time_points`); unknown fields are rejected.
   * @param counterpartyHazardCurve - Hazard curve for the counterparty's credit.
   * @param ownHazardCurve - Hazard curve for the institution's own credit.
   * @param discountCurve - Risk-free discount curve for present-valuing.
   * @param counterpartyRecoveryRate - Recovery on counterparty default, in `[0, 1]`.
   * @param ownRecoveryRate - Recovery on own default, in `[0, 1]`.
   * @param fundingJson - Optional strict `FundingConfig` JSON driving FVA and, when it carries `im_profile`, MVA; unknown fields are rejected. Omit for credit legs only.
   * @returns The `XvaResult` as a plain object.
   * @throws Error - If JSON is malformed or has unknown profile or funding fields, a recovery rate is outside `[0, 1]`, a profile is invalid or has a mismatched IM horizon, or a curve evaluation is non-finite.
   */
  computeBilateralXva(
    exposureProfileJson: JsonInput,
    counterpartyHazardCurve: HazardCurve,
    ownHazardCurve: HazardCurve,
    discountCurve: DiscountCurve,
    counterpartyRecoveryRate: number,
    ownRecoveryRate: number,
    fundingJson?: JsonInput | null
  ): XvaResult;
}

/**
 * Namespaced TypeScript entry point for margin APIs.
 */
export declare const margin: MarginNamespace;

// --- cashflows -------------------------------------------------------------

/**
 * Canonical cashflow schedule: classified dated flows with their notional,
 * day count and metadata.
 *
 * Obtain one from `CashFlowSchedule.builder()`, `buildCashflowSchedule`,
 * `CashFlowSchedule.fromJson` or `CashFlowSchedule.fromParts`. Methods that
 * return a schedule return a new handle and leave this one unchanged.
 */
export interface CashFlowSchedule extends WasmOwned {
  /**
   * Serialize the schedule to canonical JSON.
   *
   * @returns `CashFlowSchedule` JSON accepted by `fromJson` and by every `scheduleJson` argument.
   * @throws If the schedule cannot be serialized.
   */
  toJson(): string;
  /**
   * Copy of the schedule tagged with a different representation.
   *
   * @param representation - `"contractual"`, `"projected"`, `"placeholder"` or `"no_residual"`: what the flows mean to pricing and waterfall policy.
   * @returns A new `CashFlowSchedule` with `meta.representation` replaced.
   * @throws If `representation` is not one of the listed strings (kind `validation`).
   */
  withRepresentation(representation: generated.cashflows.CashflowRepresentation): CashFlowSchedule;
  /**
   * Copy of the schedule with a different notional.
   *
   * @param notional - `Notional` wire object replacing the schedule's initial balance and amortization rule.
   * @returns A new `CashFlowSchedule`; flows are unchanged.
   * @throws If `notional` is not a `Notional` (kind `validation`).
   */
  withNotional(notional: generated.cashflows.Notional): CashFlowSchedule;
  /**
   * All flows in canonical schedule order.
   *
   * @returns `CashFlow` wire objects: coupons, fees, principal and state rows.
   * @throws If the flows cannot be serialized.
   */
  getFlows(): generated.cashflows.CashFlow[];
  /**
   * Coupon flows only (fixed, floating, inflation and stub interest).
   *
   * @returns `CashFlow` wire objects of interest-like kinds, in schedule order.
   * @throws If the flows cannot be serialized.
   */
  coupons(): generated.cashflows.CashFlow[];
  /**
   * Unique flow dates in ascending order.
   *
   * @returns ISO-8601 dates on which at least one flow falls.
   * @throws If the dates cannot be serialized.
   */
  dates(): generated.core.DateWire[];
  /**
   * Notional of the schedule.
   *
   * @returns `Notional` wire object: initial balance and amortization rule.
   * @throws If the notional cannot be serialized.
   */
  getNotional(): generated.cashflows.Notional;
  /**
   * Day count attached to the schedule.
   *
   * @returns `DayCount` wire string such as `"act_360"`.
   * @throws If the day count cannot be serialized.
   */
  getDayCount(): generated.core.DayCount;
  /**
   * Schedule-level metadata.
   *
   * @returns `CashFlowMeta` wire object: representation, calendar ids, commitment, issue date, maturity and projected fixings.
   * @throws If the metadata cannot be serialized.
   */
  getMeta(): generated.cashflows.CashFlowMeta;
  /**
   * Check the schedule's invariants.
   *
   * @throws If the notional or a flow is invalid, flow dates are out of order, or funding, outstanding balances, currencies and the schedule metadata do not reconcile (kind `validation`).
   */
  validate(): void;
  /**
   * Copy of the schedule with every flow amount multiplied by a factor.
   *
   * Scales principal, interest, fee and recovery amounts; the
   * representative notional, flow kinds and dates are unchanged.
   *
   * @param scale - Finite multiplier applied to every flow amount (for example a position quantity; a negative value reverses the cashflow direction).
   * @returns A new, scaled `CashFlowSchedule`.
   * @throws If `scale` is not a number (kind `invalid_type`) or is NaN or infinite (kind `validation`).
   */
  scaleAmounts(scale: number): CashFlowSchedule;
  /**
   * Weighted average life in years from a date.
   *
   * WAL over the positive principal flows (amortization, notional and
   * prepayment) dated after `asOf`, with time measured as actual days / 365
   * (the SIFMA convention), not the schedule's accrual day count.
   *
   * @param asOf - ISO-8601 measurement date; only principal flows strictly after it count.
   * @returns WAL in years; `0` when no principal flow falls after `asOf`.
   * @throws If `asOf` is not an ISO-8601 string (kind `invalid_type` or `validation`) or a year fraction cannot be computed.
   */
  wal(asOf: string): number;
  /**
   * Outstanding principal balance after each unique balance date.
   *
   * Principal flows (amortization, PIK, draws and repayments) are replayed
   * from the initial notional.
   *
   * @returns `{ date, amount }` entries in date order; `amount` is the outstanding balance after that date's flows.
   * @throws If `meta.issue_date` is unset or principal flows mix currencies (kind `validation`).
   */
  outstandingByDate(): DatedFlowJson[];
  /**
   * Present value of the flows grouped by reporting period and currency.
   *
   * Each flow is discounted on the named curve from `base`; with a credit
   * curve the flows are also weighted by survival and recovery.
   *
   * @param periods - `Period` wire objects (`{ id, start, end, is_actual }`) defining the reporting buckets.
   * @param market - `MarketContext` handle holding the discount (and credit) curve.
   * @param discCurveId - Identifier of the discount curve in `market`, for example `"USD-OIS"`.
   * @param base - ISO-8601 valuation date that discounting starts from.
   * @param dayCount - Optional `DayCount` wire string for discounting year fractions; omitted means `"act_365f"`.
   * @param creditCurveId - Optional identifier of a hazard curve in `market`; omitted means no credit adjustment.
   * @returns `PeriodAggregation`: `{ periodId: { currency: Money } }` of PV per period and currency.
   * @throws If a curve is missing from `market` (kind `not_found`), or an argument is malformed or a discount factor cannot be computed (kind `invalid_type` or `validation`).
   */
  pvByPeriod(periods: readonly generated.statements.Period[], market: MarketContext, discCurveId: string, base: string, dayCount?: generated.core.DayCount | null, creditCurveId?: string | null): generated.cashflows.PeriodAggregation;
  /**
   * Calendar-year non-principal / principal / PV ladder.
   *
   * @param pvs - Present value of each flow, one per flow in schedule order, in flow-amount units.
   * @returns `{ year, non_principal, principal, pv }` rows in ascending year order. (Python returns the same rows as a DataFrame.)
   * @throws If `pvs` is not an array of numbers (kind `invalid_type`), does not have one entry per flow, or holds a non-finite value (kind `validation`).
   */
  calendarYearLadder(pvs: number[] | Float64Array): CalendarYearLadderRow[];
}

/**
 * Static entry points of the `CashFlowSchedule` handle.
 * @example
 * ```typescript
 * import init, { cashflows } from "finstack-quant-wasm";
 * await init();
 * const schedule = cashflows.CashFlowSchedule.builder()
 *   .principal({ amount: "1000000", currency: "USD" }, "2025-01-15", "2027-01-15")
 *   .fixedCf({ rate: "0.05", coupon_type: "cash", ...cashflows.scheduleParamsSemiannual30360() })
 *   .build();
 * console.log(schedule.wal("2025-01-15"), schedule.getFlows().length);
 * schedule.free();
 * ```
 */
export interface CashFlowScheduleConstructor {
  /**
   * JavaScript prototype of `CashFlowSchedule`; instances come from its static methods, not `new`.
   */
  readonly prototype: CashFlowSchedule;
  /**
   * Start an empty schedule builder.
   *
   * @returns A `CashFlowBuilder`; set the principal first, then add coupons, fees and principal events.
   */
  builder(): CashFlowBuilder;
  /**
   * Assemble a schedule from existing flows, notional, day count and metadata.
   *
   * Flows are put into the canonical schedule order; nothing is validated
   * until `validate()` or an analytic runs.
   *
   * @param flows - `CashFlow` wire objects.
   * @param notional - `Notional` wire object: initial balance and amortization rule.
   * @param dayCount - `DayCount` wire string (for example `"act_360"`) used for accrual and year fractions.
   * @param meta - `CashFlowMeta` wire object: representation, calendar ids, commitment, issue date and maturity.
   * @returns The assembled `CashFlowSchedule`.
   * @throws If an argument does not match its wire type (kind `validation`).
   */
  fromParts(flows: readonly generated.cashflows.CashFlow[], notional: generated.cashflows.Notional, dayCount: generated.core.DayCount, meta: generated.cashflows.CashFlowMeta): CashFlowSchedule;
  /**
   * Assemble a schedule from existing flows, with default metadata unless given.
   *
   * Same as `fromParts` with `meta` optional. (Python's `from_flows` also
   * reads a pandas DataFrame; WASM takes the flow list.)
   *
   * @param flows - `CashFlow` wire objects.
   * @param notional - `Notional` wire object: initial balance and amortization rule.
   * @param dayCount - `DayCount` wire string used for accrual and year fractions.
   * @param meta - Optional `CashFlowMeta` wire object; omitted means the default metadata (contractual representation, no issue date).
   * @returns The assembled `CashFlowSchedule`.
   * @throws If an argument does not match its wire type (kind `validation`).
   */
  fromFlows(flows: readonly generated.cashflows.CashFlow[], notional: generated.cashflows.Notional, dayCount: generated.core.DayCount, meta?: generated.cashflows.CashFlowMeta | null): CashFlowSchedule;
  /**
   * Parse a schedule from its canonical JSON.
   *
   * @param json - `CashFlowSchedule` JSON string or plain object; unknown fields are rejected.
   * @returns The parsed `CashFlowSchedule`; it is not validated until `validate()` or an analytic runs.
   * @throws If `json` is neither a string nor a plain object (kind `invalid_type`) or does not match the schedule schema (kind `validation`).
   */
  fromJson(json: JsonInput): CashFlowSchedule;
}

/**
 * Fluent builder for a `CashFlowSchedule`.
 *
 * Created by `CashFlowSchedule.builder()`. Every setter consumes the builder
 * and returns it, so calls chain; a call on a builder that was already
 * consumed throws. Setter errors are recorded and reported by `build`.
 */
export interface CashFlowBuilder extends WasmOwned {
  /**
   * Set the initial principal and the issue and maturity dates.
   *
   * @param initial - `Money` wire object: initial outstanding balance; its currency is the schedule currency.
   * @param issueDate - ISO-8601 issue date; accrual starts here.
   * @param maturity - ISO-8601 maturity date; the final principal is repaid here.
   * @returns The builder, for chaining.
   * @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
   */
  principal(initial: MoneyValue, issueDate: string, maturity: string): CashFlowBuilder;
  /**
   * Choose whether the initial funding and final redemption are emitted as flows.
   *
   * @param exchange - `"none"` or `"initial_and_final"` (the default when never called).
   * @returns The builder, for chaining.
   * @throws If `exchange` is not one of the listed strings (kind `validation`).
   */
  principalExchange(exchange: generated.cashflows.PrincipalExchange): CashFlowBuilder;
  /**
   * Set the amortization rule of the principal.
   *
   * @param spec - `AmortizationSpec` wire value, for example `"none"` or `{ linear_to: { final_notional } }`.
   * @returns The builder, for chaining.
   * @throws If `spec` is not an `AmortizationSpec` (kind `validation`).
   */
  amortization(spec: generated.cashflows.AmortizationSpec): CashFlowBuilder;
  /**
   * Add a dated principal event (draw, repayment or other balance change).
   *
   * @param date - ISO-8601 economic date on which the outstanding balance changes.
   * @param paymentDate - ISO-8601 cash settlement date of the event.
   * @param delta - `Money` wire object: change in outstanding balance (positive increases it, negative repays).
   * @param kind - `CFKind` wire string of the emitted flow, for example `"amortization"` or `"revolving_draw"`.
   * @param cash - Optional `Money` wire object: cash paid or received when it differs from `delta` (OID, fees); omitted means equal to `delta`.
   * @returns The builder, for chaining.
   * @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
   */
  addPrincipalEvent(date: string, paymentDate: string, delta: MoneyValue, kind: generated.cashflows.CFKind, cash?: MoneyValue | null): CashFlowBuilder;
  /**
   * Add a fixed-rate coupon leg over the whole life.
   *
   * @param spec - `FixedCouponSpec` wire object: decimal `rate`, coupon type and schedule parameters.
   * @returns The builder, for chaining.
   * @throws If `spec` is not a `FixedCouponSpec` (kind `validation`).
   */
  fixedCf(spec: generated.cashflows.FixedCouponSpec): CashFlowBuilder;
  /**
   * Add a floating-rate coupon leg over the whole life.
   *
   * @param spec - `FloatingCouponSpec` wire object: rate specification, coupon type and schedule parameters.
   * @returns The builder, for chaining.
   * @throws If `spec` is not a `FloatingCouponSpec` (kind `validation`).
   */
  floatingCf(spec: generated.cashflows.FloatingCouponSpec): CashFlowBuilder;
  /**
   * Add a fixed coupon leg whose rate steps up on listed dates.
   *
   * @param spec - `StepUpCouponSpec` wire object: initial decimal rate, dated rate steps, coupon type and schedule parameters.
   * @returns The builder, for chaining.
   * @throws If `spec` is not a `StepUpCouponSpec` (kind `validation`).
   */
  stepUpCf(spec: generated.cashflows.StepUpCouponSpec): CashFlowBuilder;
  /**
   * Add a one-off or recurring fee.
   *
   * @param spec - `FeeSpec` wire value: `{ fixed: { date, amount } }` or `{ periodic_bp: { ... } }`.
   * @returns The builder, for chaining.
   * @throws If `spec` is not a `FeeSpec` (kind `validation`).
   */
  fee(spec: generated.cashflows.FeeSpec): CashFlowBuilder;
  /**
   * Add a fixed coupon leg that applies only inside a date window.
   *
   * @param start - ISO-8601 start of the window (inclusive).
   * @param end - ISO-8601 end of the window (exclusive).
   * @param spec - `FixedCouponSpec` wire object for the window.
   * @returns The builder, for chaining.
   * @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
   */
  addFixedWindow(start: string, end: string, spec: generated.cashflows.FixedCouponSpec): CashFlowBuilder;
  /**
   * Add a floating coupon leg that applies only inside a date window.
   *
   * @param start - ISO-8601 start of the window (inclusive).
   * @param end - ISO-8601 end of the window (exclusive).
   * @param spec - `FloatingCouponSpec` wire object for the window.
   * @returns The builder, for chaining.
   * @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
   */
  addFloatingWindow(start: string, end: string, spec: generated.cashflows.FloatingCouponSpec): CashFlowBuilder;
  /**
   * Set the cash/PIK split of coupons paid inside a date window.
   *
   * @param start - ISO-8601 start of the window (inclusive).
   * @param end - ISO-8601 end of the window (exclusive).
   * @param split - `CouponType` wire value: `"cash"`, `"pik"` or `{ split: { cash_fraction, pik_fraction } }`.
   * @returns The builder, for chaining.
   * @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
   */
  addPaymentWindow(start: string, end: string, split: generated.cashflows.CouponType): CashFlowBuilder;
  /**
   * Set a sequence of cash/PIK splits, each applying until its end date.
   *
   * @param steps - `[isoDate, CouponType]` pairs ordered by date: each split applies up to (not including) its date; periods after the last step pay cash.
   * @returns The builder, for chaining.
   * @throws If `steps` is not an array of `[isoDate, CouponType]` pairs (kind `validation`).
   */
  paymentSplitProgram(steps: readonly (readonly [string, generated.cashflows.CouponType])[]): CashFlowBuilder;
  /**
   * Pay a fixed coupon until a switch date, then a floating coupon.
   *
   * @param switchDate - ISO-8601 date on which the leg switches from fixed to floating (Rust and Python name it `switch`, a reserved word in JavaScript).
   * @param fixed - `FixedCouponSpec` wire object used before the switch.
   * @param floating - `FloatingCouponSpec` wire object used from the switch.
   * @returns The builder, for chaining.
   * @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
   */
  fixedToFloat(switchDate: string, fixed: generated.cashflows.FixedCouponSpec, floating: generated.cashflows.FloatingCouponSpec): CashFlowBuilder;
  /**
   * Add a floating leg whose margin steps on listed dates.
   *
   * @param steps - `[isoDate, decimalString]` pairs with strictly increasing dates: the spread in basis points in force from each date.
   * @param baseSpec - `FloatingCouponSpec` wire object supplying index, schedule and conventions; its own spread applies before the first step.
   * @returns The builder, for chaining.
   * @throws If an argument does not match its wire type (kind `validation`).
   */
  floatMarginSteps(steps: readonly (readonly [string, string])[], baseSpec: generated.cashflows.FloatingCouponSpec): CashFlowBuilder;
  /**
   * Build the schedule, projecting floating coupons from market curves when a market is given.
   *
   * The builder is not consumed and can be built again.
   *
   * @param market - Optional `MarketContext` handle holding the forward curves and fixings the floating legs reference; omitted means floating coupons that need a forward curve use their fallback policy.
   * @returns The built `CashFlowSchedule` handle; release it with free().
   * @throws If a setter recorded an error, the principal is unset, a referenced curve or fixing is missing (kind `not_found`), or schedule generation fails (kind `validation`).
   */
  build(market?: MarketContext | null): CashFlowSchedule;
}

/**
 * Static entry points of the `CashFlowBuilder` handle.
 * @example
 * ```typescript
 * import init, { cashflows } from "finstack-quant-wasm";
 * await init();
 * const builder = cashflows.CashFlowSchedule.builder()
 *   .principal({ amount: "1000000", currency: "USD" }, "2025-01-15", "2027-01-15")
 *   .amortization(cashflows.amortizationSpecLinearTo({ amount: "500000", currency: "USD" }))
 *   .fixedCf({ rate: "0.05", coupon_type: "cash", ...cashflows.scheduleParamsSemiannual30360() });
 * const schedule = builder.build();
 * console.log(schedule.outstandingByDate());
 * schedule.free();
 * builder.free();
 * ```
 */
export interface CashFlowBuilderConstructor {
  /**
   * JavaScript prototype of `CashFlowBuilder`; instances come from `CashFlowSchedule.builder()`, not `new`.
   */
  readonly prototype: CashFlowBuilder;

}

/**
 * Precomputed accrual state for repeated accrued-interest queries.
 *
 * Builds the coupon periods and outstanding path once for a schedule and
 * accrual configuration; prefer it over `accruedInterestAmount` when accruing
 * the same schedule on many dates.
 */
export interface AccrualIndex extends WasmOwned {
  /**
   * Accrued interest as of a date.
   *
   * @param asOf - ISO-8601 accrual snapshot date.
   * @returns `Money` wire object in the schedule currency; negative inside an ex-coupon window.
   * @throws If `asOf` is not an ISO-8601 string (kind `invalid_type` or `validation`), a day-count calculation fails, or the ex-coupon calendar cannot be resolved (kind `validation`).
   */
  accruedAt(asOf: string): MoneyValue;
}

/**
 * Static entry points of the `AccrualIndex` handle.
 * @example
 * ```typescript
 * import init, { cashflows } from "finstack-quant-wasm";
 * await init();
 * const schedule = cashflows.CashFlowSchedule.builder()
 *   .principal({ amount: "1000000", currency: "USD" }, "2025-01-15", "2027-01-15")
 *   .fixedCf({ rate: "0.05", coupon_type: "cash", ...cashflows.scheduleParamsSemiannual30360() })
 *   .build();
 * const index = cashflows.AccrualIndex.build(schedule);
 * console.log(index.accruedAt("2025-04-15").amount);
 * index.free();
 * schedule.free();
 * ```
 */
export interface AccrualIndexConstructor {
  /**
   * JavaScript prototype of `AccrualIndex`; instances come from its static methods, not `new`.
   */
  readonly prototype: AccrualIndex;
  /**
   * Build reusable accrual state for a schedule.
   *
   * @param schedule - `CashFlowSchedule` handle with coupon, PIK and notional flows.
   * @param config - Optional `AccrualConfig` wire object bound into the index; omitted means linear accrual with the Rust defaults. Build another index to accrue under a different configuration.
   * @returns An `AccrualIndex` handle; release it with free().
   * @throws If the schedule fails validation or mixes coupon currencies, `config` is not an `AccrualConfig`, or the outstanding path or a day-count calculation fails (kind `validation`).
   */
  build(schedule: CashFlowSchedule, config?: generated.cashflows.AccrualConfig | null): AccrualIndex;
}

/**
 * Cashflow schedules from the Rust `finstack-quant-cashflows` crate.
 *
 * `CashFlowSchedule`, `CashFlowBuilder` and `AccrualIndex` are handles. Specs,
 * flows and aggregations cross as plain objects that mirror the canonical Rust
 * serde model (types under `./types`), with their Rust constructors and
 * methods as free functions. The `*Json` functions are the JSON-string bridge.
 * @example
 * ```typescript
 * import init, { cashflows } from "finstack-quant-wasm";
 * await init();
 * const spec = JSON.stringify({
 *   notional: { initial: { amount: "1000000", currency: "USD" }, amort: "none" },
 *   issue_date: "2026-01-02",
 *   maturity: "2027-01-02",
 *   coupon_program: [{
 *     kind: "fixed",
 *     spec: {
 *       coupon_type: "cash",
 *       rate: "0.05",
 *       frequency: { count: 12, unit: "months" },
 *       day_count: "30_360",
 *       business_day_convention: "following",
 *       calendar_id: "weekends_only",
 *       stub: "none",
 *       end_of_month: false,
 *       payment_lag_days: 0
 *     }
 *   }]
 * });
 * const schedule = cashflows.buildCashflowScheduleJson(spec);
 * console.log(JSON.parse(cashflows.datedFlowsJson(schedule)).length);
 * ```
 */
export interface CashflowsNamespace {
  /**
   * Build a cashflow schedule from a `CashflowScheduleBuildSpec` JSON string.
   *
   * @param specJson - JSON-encoded `CashflowScheduleBuildSpec`. Optional `principal_exchange` is `"none"` or `"initial_and_final"` (default). `principal_events` entries require both economic `date` and cash `payment_date`.
   * @param marketJson - Optional JSON-encoded market context for floating-rate lookups.
   * @returns JSON-encoded `CashFlowSchedule`.
   * @throws If the spec or market JSON is malformed, or schedule construction fails.
   */
  buildCashflowScheduleJson(specJson: JsonInput, marketJson?: JsonInput | null): string;

  /**
   * Validate a cashflow schedule JSON string and return it canonicalized.
   *
   * @param scheduleJson - JSON-encoded `CashFlowSchedule`.
   * @returns Canonicalized JSON-encoded `CashFlowSchedule`.
   * @throws If the schedule JSON is malformed or fails validation.
   */
  validateCashflowScheduleJson(scheduleJson: JsonInput): string;

  /**
   * Extract dated flows from a cashflow schedule JSON string.
   *
   * @param scheduleJson - JSON-encoded `CashFlowSchedule`.
   * @returns JSON array of settlement cash entries. PIK and `DefaultedNotional` state rows are omitted; parse the full schedule JSON when flow classification is required.
   * @throws If the schedule JSON is malformed or the schedule fails `CashFlowSchedule` validation (kind `"validation"`).
   */
  datedFlowsJson(scheduleJson: JsonInput): string;

  /**
   * Compute accrued interest from a cashflow schedule JSON string as of a given date.
   *
   * @param scheduleJson - JSON-encoded `CashFlowSchedule`.
   * @param asOf - ISO-8601 date (YYYY-MM-DD) for the accrual snapshot.
   * @param configJson - Optional JSON-encoded `AccrualConfig` overriding defaults.
   * @returns Accrued interest in the schedule's settlement currency as a JS number. The Rust engine computes from the canonical schedule and then crosses the WASM boundary as `f64`; for large notionals, compare with an absolute tolerance scaled to the schedule notional rather than expecting decimal-string equality.
   * @throws If any JSON input is malformed or the accrual computation fails.
   */
  accruedInterest(scheduleJson: JsonInput, asOf: string, configJson?: JsonInput | null): number;

  /**
   * Convert an annual CPR (constant prepayment rate) to a monthly SMM.
   *
   * Uses the standard relationship `SMM = 1 - (1 - CPR)^(1/12)`.
   *
   * @param cpr - Annualized CPR as a decimal in `[0, 1]` (0.06 means 6%).
   * @returns Monthly SMM as a decimal.
   * @throws If `cpr` is negative, non-finite, or above 1.0.
   */
  cprToSmm(cpr: number): number;

  /**
   * Convert a monthly SMM (single monthly mortality) to an annual CPR.
   *
   * Uses `CPR = 1 - (1 - SMM)^12`.
   *
   * @param smm - Monthly SMM as a decimal in `[0, 1]`.
   * @returns Annualized CPR as a decimal.
   * @throws If `smm` is negative, non-finite, or above 1.0.
   */
  smmToCpr(smm: number): number;

  /**
   * Convert an annual CDR (constant default rate) to a monthly MDR.
   *
   * Default and prepayment mortality rates share the same annual-to-monthly
   * conversion kernel: `MDR = 1 - (1 - CDR)^(1/12)`.
   *
   * @param cdr - Constant annual default rate as a decimal in `[0, 1]`.
   * @returns Monthly MDR as a decimal.
   * @throws If `cdr` is negative, non-finite, or above 1.0.
   */
  cdrToMdr(cdr: number): number;

  /**
   * Convert a monthly MDR (monthly default rate) to an annual CDR.
   *
   * Uses `CDR = 1 - (1 - MDR)^12`.
   *
   * @param mdr - Monthly default rate as a decimal in `[0, 1]`.
   * @returns Annualized CDR as a decimal.
   * @throws If `mdr` is negative, non-finite, or above 1.0.
   */
  mdrToCdr(mdr: number): number;

  /**
   * Convert an ABS speed to the single-month mortality for a seasoning month.
   *
   * @param speed - Monthly prepayment as a decimal fraction of the original balance (`0.015` = 1.5% ABS), in `[0, 1]`.
   * @param month - Seasoning month counted from origination (non-negative integer).
   * @returns Single-month mortality as a decimal in `[0, 1]`.
   * @throws If `speed` is non-finite or outside `[0, 1]` (kind `validation`), or `month` is not a non-negative integer (kind `invalid_type`).
   */
  absToSmm(speed: number, month: number): number;

  /**
   * Weighted average life of a schedule, in years from `asOf`.
   *
   * @param scheduleJson - `CashFlowSchedule` (object or JSON).
   * @param asOf - ISO-8601 measurement date; only principal flows strictly after it count.
   * @returns WAL in years; `0` when no principal flow falls after `asOf`.
   * @throws If the schedule or date is malformed or the schedule fails validation (kind `validation`).
   */
  scheduleWal(scheduleJson: JsonInput, asOf: string): number;

  /**
   * Outstanding principal balance after each unique date of a schedule.
   *
   * @param scheduleJson - `CashFlowSchedule` (object or JSON) with `meta.issue_date` set.
   * @returns `{ date, amount }` entries in date order; `amount` is the outstanding balance after that date's flows.
   * @throws If the schedule is malformed or fails validation, `meta.issue_date` is unset, or principal flows mix currencies (kind `validation`).
   */
  scheduleOutstandingByDate(scheduleJson: JsonInput): DatedFlowJson[];

  /**
   * Calendar-year non-principal / principal / PV ladder of a schedule.
   *
   * @param scheduleJson - `CashFlowSchedule` (object or JSON).
   * @param pvs - Present value of each schedule flow, one per flow in schedule order, in flow-amount units.
   * @returns `{ year, non_principal, principal, pv }` rows in ascending year order.
   * @throws If the schedule is malformed or fails validation, `pvs` does not have one entry per flow, or a value is non-finite (kind `validation`).
   */
  scheduleCalendarYearLadder(scheduleJson: JsonInput, pvs: number[] | Float64Array): CalendarYearLadderRow[];

  /**
   * `CashFlowSchedule` handle: canonical cashflow schedule: classified dated flows with their notional,
   */
  CashFlowSchedule: CashFlowScheduleConstructor;

  /**
   * `CashFlowBuilder` handle: fluent builder for a `CashFlowSchedule`.
   */
  CashFlowBuilder: CashFlowBuilderConstructor;

  /**
   * `AccrualIndex` handle: precomputed accrual state for repeated accrued-interest queries.
   */
  AccrualIndex: AccrualIndexConstructor;

  /**
   * Build a cashflow schedule from a build spec.
   *
   * Typed twin of `buildCashflowScheduleJson`: the same spec, returning a
   * `CashFlowSchedule` handle.
   *
   * @param spec - `CashflowScheduleBuildSpec` wire object or JSON: notional, issue and maturity dates, coupon program, fees and principal events.
   * @param market - Optional `MarketContext` handle holding the forward curves and fixings the floating legs reference; omitted means floating coupons that need a forward curve use their fallback policy.
   * @returns The built `CashFlowSchedule` handle; release it with free().
   * @throws If `spec` does not match the build-spec schema (kind `invalid_type` or `validation`), a referenced curve or fixing is missing (kind `not_found`), or schedule construction fails (kind `validation`).
   */
  buildCashflowSchedule(spec: generated.cashflows.CashflowScheduleBuildSpec | string, market?: MarketContext | null): CashFlowSchedule;

  /**
   * Settlement cash flows of a schedule as dated amounts.
   *
   * Validates the schedule and keeps only cash-settling rows (`pik` and
   * `defaulted_notional` state rows are excluded), so the result is safe to
   * sum per currency. Typed twin of `datedFlowsJson`.
   *
   * @param schedule - `CashFlowSchedule` handle.
   * @returns `{ date, amount }` entries in schedule order. (Python returns `(date, Money)` tuples.)
   * @throws If the schedule fails validation (kind `validation`).
   */
  datedFlows(schedule: CashFlowSchedule): DatedFlowJson[];

  /**
   * Build a schedule from dated amounts that all share one cashflow kind.
   *
   * @param flows - `{ date, amount }` entries: ISO-8601 date and `Money` amount of each flow.
   * @param kind - `CFKind` wire string stamped on every flow, for example `"fixed"`.
   * @param dayCount - `DayCount` wire string attached to the schedule for downstream accrual and yield calculations.
   * @param opts - Optional `ScheduleBuildOpts` wire object (`{ notional_hint?, meta? }`); omitted means a zero notional in the first flow's currency and default metadata.
   * @returns The `CashFlowSchedule` handle, flows in canonical order.
   * @throws If an argument does not match its wire type (kind `validation`).
   */
  scheduleFromDatedFlows(flows: readonly DatedFlowJson[], kind: generated.cashflows.CFKind, dayCount: generated.core.DayCount, opts?: generated.cashflows.ScheduleBuildOpts | null): CashFlowSchedule;

  /**
   * Build a schedule from flows that already carry their cashflow kind.
   *
   * @param flows - `CashFlow` wire objects; each flow's kind is kept as is.
   * @param dayCount - `DayCount` wire string attached to the schedule for downstream accrual and yield calculations.
   * @param opts - Optional `ScheduleBuildOpts` wire object (`{ notional_hint?, meta? }`); omitted means a zero notional in the first flow's currency and default metadata.
   * @returns The `CashFlowSchedule` handle, flows in canonical order.
   * @throws If an argument does not match its wire type (kind `validation`).
   */
  scheduleFromClassifiedFlows(flows: readonly generated.cashflows.CashFlow[], dayCount: generated.core.DayCount, opts?: generated.cashflows.ScheduleBuildOpts | null): CashFlowSchedule;

  /**
   * Merge several schedules into one under a new notional and day count.
   *
   * Flows are concatenated and put into canonical order; calendar ids and
   * projected fixings are merged.
   *
   * @param schedules - Schedules to merge: `CashFlowSchedule` handles (the facade passes them as JSON), canonical JSON strings or plain objects.
   * @param notional - `Notional` wire object of the merged schedule.
   * @param dayCount - `DayCount` wire string of the merged schedule.
   * @returns The merged `CashFlowSchedule` handle.
   * @throws If `schedules` is not an array (kind `invalid_type`) or an argument does not match its wire type (kind `validation`).
   */
  mergeCashflowSchedules(schedules: readonly (CashFlowSchedule | JsonInput)[], notional: generated.cashflows.Notional, dayCount: generated.core.DayCount): CashFlowSchedule;

  /**
   * Ex-coupon date for a coupon paid on a date.
   *
   * From the returned date (inclusive) until the payment date (exclusive) the
   * bond trades ex-coupon and accrued interest is negative.
   *
   * @param rule - `ExCouponRule` wire object: `days_before_coupon` and an optional `calendar_id` (business days when set, calendar days otherwise).
   * @param paymentDate - ISO-8601 coupon payment date the window is counted back from.
   * @returns ISO-8601 ex-coupon date.
   * @throws If `rule` is not an `ExCouponRule`, `days_before_coupon` exceeds 366, the calendar id cannot be resolved (kind `validation`), or `paymentDate` is not an ISO-8601 string (kind `invalid_type` or `validation`).
   */
  exCouponRuleExDate(rule: generated.cashflows.ExCouponRule, paymentDate: string): generated.core.DateWire;

  /**
   * Accrued interest of a schedule as of a date.
   *
   * Typed twin of `accruedInterest`, which takes schedule JSON and returns a
   * number.
   *
   * @param schedule - `CashFlowSchedule` handle.
   * @param asOf - ISO-8601 accrual snapshot date; interest accrues through accrual end and stays accrued until payment.
   * @param config - Optional `AccrualConfig` wire object (`{ method, ex_coupon, include_pik, frequency }`); omitted means linear accrual with the Rust defaults.
   * @returns `Money` wire object in the schedule currency; negative inside an ex-coupon window.
   * @throws If the schedule fails validation or mixes coupon currencies, `config` is not an `AccrualConfig`, or a day-count calculation fails (kind `validation`).
   */
  accruedInterestAmount(schedule: CashFlowSchedule, asOf: string, config?: generated.cashflows.AccrualConfig | null): MoneyValue;

  /**
   * Sum dated amounts into reporting periods, keeping currencies separate.
   *
   * @param flows - `{ date, amount }` entries: ISO-8601 date and `Money` amount of each flow. A flow falls in the period whose `[start, end)` contains its date; flows outside every period are dropped.
   * @param periods - `Period` wire objects (`{ id, start, end, is_actual }`), sorted by date and non-overlapping.
   * @returns `PeriodAggregation`: `{ periodId: { currency: Money } }`, with only periods that received a flow.
   * @throws If an argument does not match its wire type, the periods are unsorted, overlapping or repeat an id, or a period total is non-finite or out of range (kind `validation`).
   */
  aggregateByPeriod(flows: readonly DatedFlowJson[], periods: readonly generated.statements.Period[]): generated.cashflows.PeriodAggregation;

  /**
   * Sum dated amounts that must all be in one currency.
   *
   * @param flows - `{ date, amount }` entries: ISO-8601 date and `Money` amount of each flow.
   * @param target - ISO-4217 currency code every flow must be in; no FX conversion is applied.
   * @returns `Money` wire object: the compensated sum in `target`.
   * @throws If a flow is in another currency or the sum cannot be represented (kind `validation`), or an argument does not match its wire type.
   */
  aggregateCashflowsChecked(flows: readonly DatedFlowJson[], target: generated.core.Currency): MoneyValue;

  /**
   * Group dated cashflows into a calendar-year non-principal / principal / PV ladder.
   *
   * The four arrays are parallel: entry `i` of each describes flow `i`.
   *
   * @param dates - ISO-8601 flow dates; the calendar year of each date is its bucket.
   * @param kinds - Cashflow kind labels as Rust displays them (`"fixed"`, `"notional"`, `"prepayment"`; also `"coupon"` and `"principal"`), ASCII case ignored; principal-like kinds go to `principal`, all others to `non_principal`.
   * @param amounts - Flow amounts in one currency's units.
   * @param pvs - Present value of each flow, in the same units as `amounts`.
   * @returns `{ year, non_principal, principal, pv }` rows in ascending year order.
   * @throws If the arrays differ in length, a kind label is unknown, or an amount or PV is non-finite (kind `validation`), or an array has the wrong element type (kind `invalid_type`).
   */
  calendarYearLadder(dates: readonly string[], kinds: readonly string[], amounts: number[] | Float64Array, pvs: number[] | Float64Array): CalendarYearLadderRow[];

  /**
   * Amount aggregated for one period and currency.
   *
   * Mirrors Rust `PeriodAggregation::get_amount`; on a plain object this is
   * `aggregation[period]?.[currency]`.
   *
   * @param aggregation - `PeriodAggregation` wire object, as returned by `aggregateByPeriod` or `CashFlowSchedule.pvByPeriod`.
   * @param period - Period code exactly as the aggregation spells it, for example `"2025Q1"`.
   * @param currency - ISO-4217 currency code of the bucket to read.
   * @returns `Money` wire object, or `undefined` when the period has no flows in that currency.
   * @throws If `aggregation` is not a `PeriodAggregation` or `currency` is not an ISO-4217 code (kind `validation`), or `period` is not a string (kind `invalid_type`).
   */
  periodAggregationGetAmount(aggregation: generated.cashflows.PeriodAggregation, period: string, currency: generated.core.Currency): MoneyValue | undefined;

  /**
   * Materialize the reset observations crossed by a realized-forward market roll.
   *
   * Every projected fixing recorded in the schedules' metadata with a date in
   * `(oldDate, newDate]` is written into a copy of the market as an observed
   * fixing; existing observations take precedence and are kept unchanged.
   *
   * @param market - `MarketContext` handle of the pre-roll market; it is not modified.
   * @param schedules - Schedules projected on `market` at `oldDate`: `CashFlowSchedule` handles (the facade passes them as JSON), canonical JSON strings or plain objects.
   * @param oldDate - ISO-8601 exclusive start of the fixing window.
   * @param newDate - ISO-8601 inclusive end of the window; must be on or after `oldDate`.
   * @returns A new `MarketContext` handle with the crossed fixings materialized; release it with free().
   * @throws If the window runs backward, a fixing-series identifier is malformed, a crossed projection is unavailable or non-finite, or two schedules project the same index and date differently (kind `validation`).
   */
  materializeFixings(market: MarketContext, schedules: readonly (CashFlowSchedule | JsonInput)[], oldDate: string, newDate: string): MarketContext;

  /**
   * Parse a cashflow kind label into its `CFKind` wire string.
   *
   * The label is the kind as Rust displays it (Rust `CFKind::from_str`), which
   * is the wire string except for prepayments: the label is `"prepayment"`
   * and the wire string `"pre_payment"`.
   *
   * @param name - Kind label such as `"fixed"`, `"float_reset"` or `"prepayment"`.
   * @returns The `CFKind` wire string, as used by the `kind` field of a `CashFlow`.
   * @throws If `name` is not a string (kind `invalid_type`) or names no cashflow kind (kind `validation`).
   */
  cfKindParse(name: string): generated.cashflows.CFKind;

  /**
   * Whether a cashflow kind is interest-like (fixed, floating, inflation or stub coupon).
   *
   * @param kind - `CFKind` wire string.
   * @returns `true` for coupon-type kinds; fees, principal and margin kinds are `false`.
   * @throws If `kind` is not a `CFKind` wire string (kind `validation`).
   */
  cfKindIsInterestLike(kind: generated.cashflows.CFKind): boolean;

  /**
   * Whether a cashflow kind changes or returns principal.
   *
   * @param kind - `CFKind` wire string.
   * @returns `true` for notional, PIK, amortization, prepayment, revolving draw/repayment and defaulted-notional kinds.
   * @throws If `kind` is not a `CFKind` wire string (kind `validation`).
   */
  cfKindIsPrincipalLike(kind: generated.cashflows.CFKind): boolean;

  /**
   * Whether a cashflow kind settles in cash.
   *
   * Non-cash state rows (`pik`, `defaulted_notional`) are excluded from
   * settlement sums such as `datedFlows`.
   *
   * @param kind - `CFKind` wire string.
   * @returns `true` when flows of this kind are paid or received in cash.
   * @throws If `kind` is not a `CFKind` wire string (kind `validation`).
   */
  isCashSettlementKind(kind: generated.cashflows.CFKind): boolean;

  /**
   * Date on which a cashflow changes the outstanding balance.
   *
   * Mirrors Rust `CashFlow::get_balance_date`: the flow's `principal_date`
   * when set, otherwise its payment `date`.
   *
   * @param flow - `CashFlow` wire object.
   * @returns ISO-8601 balance date.
   * @throws If `flow` is not a `CashFlow` (kind `validation`).
   */
  cashFlowGetBalanceDate(flow: generated.cashflows.CashFlow): generated.core.DateWire;

  /**
   * Copy of a cashflow with an explicit economic principal date.
   *
   * @param flow - `CashFlow` wire object.
   * @param date - ISO-8601 date on which the outstanding balance changes, independent of the cash payment date.
   * @returns The updated `CashFlow`.
   * @throws If `flow` is not a `CashFlow` or `date` is not an ISO date (kind `validation`).
   */
  cashFlowWithPrincipalDate(flow: generated.cashflows.CashFlow, date: string): generated.cashflows.CashFlow;

  /**
   * Copy of a cashflow carrying its accrual-period metadata.
   *
   * @param flow - `CashFlow` wire object.
   * @param accrual - `CashFlowAccrual` wire object: accrual start and end dates, day count and optional projected index rate.
   * @returns The updated `CashFlow`.
   * @throws If either argument does not match its wire type (kind `validation`).
   */
  cashFlowWithAccrual(flow: generated.cashflows.CashFlow, accrual: generated.cashflows.CashFlowAccrual): generated.cashflows.CashFlow;

  /**
   * Copy of a cashflow with an explicit outstanding-balance change.
   *
   * @param flow - `CashFlow` wire object.
   * @param delta - `Money` wire object: change in outstanding principal (positive increases the balance), which may differ from the cash amount.
   * @returns The updated `CashFlow`.
   * @throws If either argument does not match its wire type (kind `validation`).
   */
  cashFlowWithPrincipalDelta(flow: generated.cashflows.CashFlow, delta: MoneyValue): generated.cashflows.CashFlow;

  /**
   * Check a cashflow's invariants.
   *
   * @param flow - `CashFlow` wire object.
   * @throws If `flow` is not a `CashFlow`, its amount, accrual factor or rate is non-finite, its accrual factor is negative, its reset date is after its payment date, or its principal delta is in another currency (kind `validation`).
   */
  cashFlowValidate(flow: generated.cashflows.CashFlow): void;

  /**
   * Straight-line amortization from the initial notional down to a final balance.
   *
   * @param finalNotional - `Money` wire object: outstanding balance left at maturity, in the notional currency.
   * @returns `AmortizationSpec` wire value `{ linear_to: { final_notional } }`.
   * @throws If `finalNotional` is not a `Money` wire object (kind `validation`).
   */
  amortizationSpecLinearTo(finalNotional: MoneyValue): generated.cashflows.AmortizationSpec;

  /**
   * Amortization to an explicit remaining balance on each listed date.
   *
   * @param schedule - `[isoDate, Money]` pairs: the outstanding balance after each date, in the notional currency.
   * @returns `AmortizationSpec` wire value `{ step_remaining: { schedule } }`.
   * @throws If `schedule` is not an array of `[isoDate, Money]` pairs (kind `validation`).
   */
  amortizationSpecStepRemaining(schedule: readonly (readonly [string, MoneyValue])[]): generated.cashflows.AmortizationSpec;

  /**
   * Amortization of a fixed share of the original notional each period.
   *
   * @param pct - Share of the original notional repaid per period as a decimal (`0.05` = 5%).
   * @returns `AmortizationSpec` wire value `{ percent_of_original_per_period: { pct } }`.
   * @throws If `pct` is not a number (kind `invalid_type`).
   */
  amortizationSpecPercentOfOriginalPerPeriod(pct: number): generated.cashflows.AmortizationSpec;

  /**
   * Amortization of a fixed share of the remaining balance each period.
   *
   * @param pct - Share of the then-outstanding balance repaid per period as a decimal (`0.05` = 5%).
   * @returns `AmortizationSpec` wire value `{ percent_of_remaining_per_period: { pct } }`.
   * @throws If `pct` is not a number (kind `invalid_type`).
   */
  amortizationSpecPercentOfRemainingPerPeriod(pct: number): generated.cashflows.AmortizationSpec;

  /**
   * Straight-line amortization to zero between two dates.
   *
   * @param start - ISO-8601 date on which amortization begins; the balance is flat before it.
   * @param end - ISO-8601 date on which the balance reaches zero.
   * @returns `AmortizationSpec` wire value `{ linear_between: { start, end } }`.
   * @throws If either date is not an ISO-8601 string (kind `invalid_type` or `validation`).
   */
  amortizationSpecLinearBetween(start: string, end: string): generated.cashflows.AmortizationSpec;

  /**
   * Amortization by explicit principal payments on listed dates.
   *
   * @param items - `[isoDate, Money]` pairs: the principal repaid on each date, in the notional currency.
   * @returns `AmortizationSpec` wire value `{ custom_principal: { items } }`.
   * @throws If `items` is not an array of `[isoDate, Money]` pairs (kind `validation`).
   */
  amortizationSpecCustomPrincipal(items: readonly (readonly [string, MoneyValue])[]): generated.cashflows.AmortizationSpec;

  /**
   * Coupon paid partly in cash and partly in kind.
   *
   * @param cashFraction - Share of each coupon paid in cash, as an exact decimal string (`"0.6"`).
   * @param pikFraction - Share of each coupon capitalized into principal, as an exact decimal string (`"0.4"`); the two shares must sum to one when the schedule is built.
   * @returns `CouponType` wire value `{ split: { cash_fraction, pik_fraction } }`.
   * @throws If a fraction is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
   */
  couponTypeSplit(cashFraction: string, pikFraction: string): generated.cashflows.CouponType;

  /**
   * Fee base equal to the undrawn part of a commitment.
   *
   * @param commitment - `Money` wire object: total facility commitment; the fee accrues on commitment minus drawn balance.
   * @returns `FeeBase` wire value `{ undrawn: { commitment } }`.
   * @throws If `commitment` is not a `Money` wire object (kind `validation`).
   */
  feeBaseUndrawn(commitment: MoneyValue): generated.cashflows.FeeBase;

  /**
   * One-off fee of a fixed amount on a date.
   *
   * @param date - ISO-8601 payment date of the fee.
   * @param amount - `Money` wire object: fee amount in its own currency.
   * @returns `FeeSpec` wire value `{ fixed: { date, amount } }`.
   * @throws If `date` is not an ISO-8601 string or `amount` is not a `Money` wire object (kind `invalid_type` or `validation`).
   */
  feeSpecFixed(date: string, amount: MoneyValue): generated.cashflows.FeeSpec;

  /**
   * Recurring fee quoted in basis points of a drawn or undrawn balance.
   *
   * The argument is the `periodic_bp` payload of `FeeSpec`, parsed by the Rust
   * serde contract: `stub` defaults to `"short_front"` and `accrual_basis` to
   * `"point_in_time"`; every other field is required. (Python's
   * `FeeSpec.periodic_bp` takes the same fields positionally.)
   *
   * @param fields - `{ base, bp, frequency, day_count, business_day_convention, calendar_id, stub?, accrual_basis? }`: `base` is a `FeeBase`, `bp` an exact decimal string in basis points per annum (`"25"` = 0.25%), `frequency` a `Tenor`.
   * @returns Canonical `FeeSpec` wire value `{ periodic_bp: { ... } }` with defaults filled in.
   * @throws If `fields` is missing a required field, has an unknown field or a field of the wrong shape (kind `validation`).
   */
  feeSpecPeriodicBp(fields: Record<string, unknown>): generated.cashflows.FeeSpec;

  /**
   * Fallback that uses a fixed index rate when a projection is unavailable.
   *
   * @param rate - Index rate used in place of the missing projection, as an exact decimal string (`"0.03"` = 3%), before spread and gearing.
   * @returns `FloatingRateFallback` wire value `{ fixed_rate: rate }`.
   * @throws If `rate` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
   */
  floatingRateFallbackFixedRate(rate: string): generated.cashflows.FloatingRateFallback;

  /**
   * Overnight compounding in arrears with a lookback.
   *
   * @param lookbackDays - Business days each daily observation is shifted back (non-negative integer; `0` for none).
   * @returns `FloatingLegCompounding` wire value `{ compounded_in_arrears: { lookback_days } }`.
   * @throws If `lookbackDays` is not a non-negative integer (kind `invalid_type`).
   */
  floatingLegCompoundingCompoundedInArrears(lookbackDays: number): generated.cashflows.FloatingLegCompounding;

  /**
   * Overnight compounding with an observation-period shift.
   *
   * @param shiftDays - Business days the whole observation period (rates and weights) is shifted back (non-negative integer).
   * @returns `FloatingLegCompounding` wire value `{ compounded_with_observation_shift: { shift_days } }`.
   * @throws If `shiftDays` is not a non-negative integer (kind `invalid_type`).
   */
  floatingLegCompoundingCompoundedWithObservationShift(shiftDays: number): generated.cashflows.FloatingLegCompounding;

  /**
   * Overnight compounding with a rate cutoff before period end.
   *
   * @param cutoffDays - Business days before period end from which the last observed rate is repeated (non-negative integer).
   * @returns `FloatingLegCompounding` wire value `{ compounded_with_rate_cutoff: { cutoff_days } }`.
   * @throws If `cutoffDays` is not a non-negative integer (kind `invalid_type`).
   */
  floatingLegCompoundingCompoundedWithRateCutoff(cutoffDays: number): generated.cashflows.FloatingLegCompounding;

  /**
   * Check a floating-rate specification's reset lag, caps and floors.
   *
   * @param spec - `FloatingRateSpec` wire object.
   * @throws If `spec` is not a `FloatingRateSpec`, its reset lag is negative, its index floor exceeds its index cap, or its all-in floor exceeds its all-in cap (kind `validation`).
   */
  floatingRateSpecValidate(spec: generated.cashflows.FloatingRateSpec): void;

  /**
   * USD SOFR floating-rate specification (curve `USD-SOFR`, compounded in arrears).
   *
   * @param spreadBp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
   * @returns `FloatingRateSpec` wire object with the Rust SOFR conventions.
   * @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
   */
  floatingRateSpecSofr(spreadBp: string): generated.cashflows.FloatingRateSpec;

  /**
   * GBP SONIA floating-rate specification (curve `GBP-SONIA`, compounded in arrears).
   *
   * @param spreadBp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
   * @returns `FloatingRateSpec` wire object with the Rust SONIA conventions.
   * @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
   */
  floatingRateSpecSonia(spreadBp: string): generated.cashflows.FloatingRateSpec;

  /**
   * EUR 3-month EURIBOR floating-rate specification (term rate set in advance).
   *
   * @param spreadBp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
   * @returns `FloatingRateSpec` wire object with the Rust EURIBOR 3M conventions.
   * @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
   */
  floatingRateSpecEuribor3m(spreadBp: string): generated.cashflows.FloatingRateSpec;

  /**
   * Bullet notional of a given amount: no amortization.
   *
   * @param amount - Finite initial outstanding balance in major currency units.
   * @param currency - ISO-4217 currency code such as `"USD"`.
   * @returns `Notional` wire object `{ initial, amort: "none" }`.
   * @throws If `amount` is not a finite number or `currency` is not an ISO-4217 code (kind `invalid_type` or `validation`).
   */
  notionalPar(amount: number, currency: generated.core.Currency): generated.cashflows.Notional;

  /**
   * Currency of a notional's initial balance.
   *
   * @param notional - `Notional` wire object.
   * @returns ISO-4217 currency code.
   * @throws If `notional` is not a `Notional` (kind `validation`).
   */
  notionalCurrency(notional: generated.cashflows.Notional): generated.core.Currency;

  /**
   * Check a notional and its amortization rule for consistency.
   *
   * @param notional - `Notional` wire object.
   * @throws If `notional` is not a `Notional`, its initial balance is negative or non-finite, or its amortization rule mixes currencies, exceeds the initial balance, has a percentage outside `[0, 1]` or has dates out of order (kind `validation`).
   */
  notionalValidate(notional: generated.cashflows.Notional): void;

  /**
   * Constant annual default rate.
   *
   * @param cdr - Annual constant default rate as a decimal (`0.02` = 2% CDR).
   * @returns `DefaultModelSpec` wire object with no seasoning curve.
   * @throws If `cdr` is not a number (kind `invalid_type`).
   */
  defaultModelSpecConstantCdr(cdr: number): generated.cashflows.DefaultModelSpec;

  /**
   * PSA/BMA Standard Default Assumption curve.
   *
   * Annual CDR ramps 0.02% per month to a 0.60% peak at month 30, is flat to
   * month 60, declines to 0.03% at month 120 and stays there.
   *
   * @param speedMultiplier - SDA speed, where `1.0` means 100% SDA and `2.0` means 200% SDA.
   * @returns `DefaultModelSpec` wire object using the SDA curve.
   * @throws If `speedMultiplier` is not a number (kind `invalid_type`).
   */
  defaultModelSpecSda(speedMultiplier: number): generated.cashflows.DefaultModelSpec;

  /**
   * Constant 2% CDR baseline.
   *
   * @returns `DefaultModelSpec` wire object equal to `defaultModelSpecConstantCdr(0.02)`.
   * @throws If the model cannot be serialized.
   */
  defaultModelSpecCdr2pct(): generated.cashflows.DefaultModelSpec;

  /**
   * Explicit annual CDR for each month of seasoning.
   *
   * @param monthlyCdr - Annual CDR per seasoning month as decimals in `[0, 1]`, month 1 first; the last value is held. Must be non-empty.
   * @returns `DefaultModelSpec` wire object using the vector curve.
   * @throws If `monthlyCdr` is not an array of numbers (kind `invalid_type`).
   */
  defaultModelSpecVector(monthlyCdr: number[] | Float64Array): generated.cashflows.DefaultModelSpec;

  /**
   * Cumulative net-loss curve with a constant severity.
   *
   * @param cumulativeNetLossPct - Cumulative net loss in percent of the original balance per seasoning month (`1.5` = 1.5%), non-decreasing, month 1 first; the last value is held.
   * @param severity - Loss severity as a decimal fraction of defaulted par in `(0, 1]`; defaults are loss divided by severity.
   * @returns `DefaultModelSpec` wire object using the cumulative-loss curve.
   * @throws If an argument is not a number or array of numbers (kind `invalid_type`).
   */
  defaultModelSpecCumulativeLoss(cumulativeNetLossPct: number[] | Float64Array, severity: number): generated.cashflows.DefaultModelSpec;

  /**
   * Rating-agency default timing over a lifetime cumulative default rate.
   *
   * @param cumulativeDefaultRate - Lifetime defaults as a decimal fraction of the original balance in `[0, 1]`.
   * @param annualPct - Share of lifetime defaults in each year of seasoning, in percent (for example `[15, 30, 30, 15, 10]`), summing to 100.
   * @returns `DefaultModelSpec` wire object using the timing curve.
   * @throws If an argument is not a number or array of numbers (kind `invalid_type`).
   */
  defaultModelSpecTiming(cumulativeDefaultRate: number, annualPct: number[] | Float64Array): generated.cashflows.DefaultModelSpec;

  /**
   * Check a default model's curve parameters.
   *
   * @param spec - `DefaultModelSpec` wire object.
   * @throws If `spec` is not a `DefaultModelSpec`, or its CDR, SDA multiplier, vector, cumulative-loss or timing parameters are out of range (kind `validation`).
   */
  defaultModelSpecValidate(spec: generated.cashflows.DefaultModelSpec): void;

  /**
   * Monthly default rate (MDR) of a default model at a seasoning month.
   *
   * @param spec - `DefaultModelSpec` wire object.
   * @param seasoningMonths - Months since origination or pool start (non-negative integer).
   * @returns Monthly default rate as a decimal; a constant CDR converts as `1 - (1 - CDR)^(1/12)`.
   * @throws If `spec` is not a `DefaultModelSpec` or its curve parameters are invalid (kind `validation`), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
   */
  defaultModelSpecMdr(spec: generated.cashflows.DefaultModelSpec, seasoningMonths: number): number;

  /**
   * Constant annual prepayment rate.
   *
   * @param cpr - Annual constant prepayment rate as a decimal (`0.06` = 6% CPR).
   * @returns `PrepaymentModelSpec` wire object with no seasoning curve.
   * @throws If `cpr` is not a number (kind `invalid_type`).
   */
  prepaymentModelSpecConstantCpr(cpr: number): generated.cashflows.PrepaymentModelSpec;

  /**
   * PSA prepayment curve: a 30-month ramp to a 6% annual CPR, then flat.
   *
   * @param speedMultiplier - PSA speed, where `1.0` means 100% PSA and `1.5` means 150% PSA.
   * @returns `PrepaymentModelSpec` wire object using the PSA curve.
   * @throws If `speedMultiplier` is not a number (kind `invalid_type`).
   */
  prepaymentModelSpecPsa(speedMultiplier: number): generated.cashflows.PrepaymentModelSpec;

  /**
   * 100% PSA, the standard prepayment assumption.
   *
   * @returns `PrepaymentModelSpec` wire object equal to `prepaymentModelSpecPsa(1.0)`.
   * @throws If the model cannot be serialized.
   */
  prepaymentModelSpecPsa100(): generated.cashflows.PrepaymentModelSpec;

  /**
   * CMBS-style lockout: no prepayment during the lockout, then a constant CPR.
   *
   * @param lockoutMonths - Months with zero prepayment (non-negative integer; for example `60` for five years).
   * @param postLockoutCpr - Annual CPR after the lockout as a decimal (`0.10` = 10%).
   * @returns `PrepaymentModelSpec` wire object using the lockout curve.
   * @throws If `lockoutMonths` is not a non-negative integer or `postLockoutCpr` is not a number (kind `invalid_type`).
   */
  prepaymentModelSpecCmbsWithLockout(lockoutMonths: number, postLockoutCpr: number): generated.cashflows.PrepaymentModelSpec;

  /**
   * ABS speed curve (auto-loan and consumer ABS convention).
   *
   * Each month the same share of the original balance prepays, so
   * `SMM_t = speed / (1 − speed·(t − 1))`.
   *
   * @param speed - Monthly prepayment as a decimal fraction of the original balance (`0.015` = 1.5% ABS), in `[0, 1]`.
   * @returns `PrepaymentModelSpec` wire object using the ABS curve.
   * @throws If `speed` is not a number (kind `invalid_type`).
   */
  prepaymentModelSpecAbs(speed: number): generated.cashflows.PrepaymentModelSpec;

  /**
   * Explicit annual CPR for each month of seasoning.
   *
   * @param monthlyCpr - Annual CPR per seasoning month as decimals in `[0, 1]`, month 1 first; the last value is held. Must be non-empty.
   * @returns `PrepaymentModelSpec` wire object using the vector curve.
   * @throws If `monthlyCpr` is not an array of numbers (kind `invalid_type`).
   */
  prepaymentModelSpecVector(monthlyCpr: number[] | Float64Array): generated.cashflows.PrepaymentModelSpec;

  /**
   * Check a prepayment model's curve parameters.
   *
   * @param spec - `PrepaymentModelSpec` wire object.
   * @throws If `spec` is not a `PrepaymentModelSpec`, or its CPR, PSA multiplier, ABS speed or vector is out of range (kind `validation`).
   */
  prepaymentModelSpecValidate(spec: generated.cashflows.PrepaymentModelSpec): void;

  /**
   * Single-month mortality (SMM) of a prepayment model at a seasoning month.
   *
   * @param spec - `PrepaymentModelSpec` wire object.
   * @param seasoningMonths - Months since origination or pool start (non-negative integer).
   * @returns Monthly prepayment rate as a decimal; a constant CPR converts as `1 - (1 - CPR)^(1/12)`.
   * @throws If `spec` is not a `PrepaymentModelSpec` or its curve parameters are invalid (kind `validation`), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
   */
  prepaymentModelSpecSmm(spec: generated.cashflows.PrepaymentModelSpec, seasoningMonths: number): number;

  /**
   * Copy of a recovery model with a loss-severity vector by month of default.
   *
   * @param spec - `RecoveryModelSpec` wire object.
   * @param severityVector - Loss severity (`1 − recovery`) per seasoning month of the default as decimals in `[0, 1]`, month 1 first; the last value is held. Must be non-empty.
   * @returns The `RecoveryModelSpec` with the vector attached; `rate` stays as the flat fallback but no longer drives recoveries.
   * @throws If `spec` is not a `RecoveryModelSpec` (kind `validation`) or `severityVector` is not an array of numbers (kind `invalid_type`).
   */
  recoveryModelSpecWithSeverityVector(spec: generated.cashflows.RecoveryModelSpec, severityVector: number[] | Float64Array): generated.cashflows.RecoveryModelSpec;

  /**
   * Recovery rate of a recovery model for a default in a seasoning month.
   *
   * @param spec - `RecoveryModelSpec` wire object.
   * @param seasoningMonths - Months since origination of the defaulting balance (non-negative integer).
   * @returns Recovery as a decimal fraction of defaulted par: `1 − severity` for the month when a severity vector is set, otherwise the flat `rate`.
   * @throws If `spec` is not a `RecoveryModelSpec` (kind `validation`) or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
   */
  recoveryModelSpecRecoveryRate(spec: generated.cashflows.RecoveryModelSpec, seasoningMonths: number): number;

  /**
   * Check a recovery model's rate and severity vector.
   *
   * @param spec - `RecoveryModelSpec` wire object.
   * @throws If `spec` is not a `RecoveryModelSpec`, its rate is outside `[0, 1]`, or its severity vector is empty or holds a value outside `[0, 1]` (kind `validation`).
   */
  recoveryModelSpecValidate(spec: generated.cashflows.RecoveryModelSpec): void;

  /**
   * Quarterly Act/360 schedule with Modified Following on a weekends-only calendar.
   *
   * @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsQuarterlyAct360(): generated.cashflows.ScheduleParams;

  /**
   * Semi-annual 30/360 schedule with Modified Following on a weekends-only calendar.
   *
   * @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsSemiannual30360(): generated.cashflows.ScheduleParams;

  /**
   * Annual ISDA Act/Act schedule with Following on a weekends-only calendar.
   *
   * This is ISDA Actual/Actual, not ICMA; government bonds use
   * `scheduleParamsEurGovBond` or `scheduleParamsUsdTreasury`.
   *
   * @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsAnnualActact(): generated.cashflows.ScheduleParams;

  /**
   * USD SOFR swap leg: quarterly, Act/360, Modified Following, USNY, T+2 payment lag.
   *
   * @returns `ScheduleParams` wire object following ARRC SOFR conventions.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsUsdSofrSwap(): generated.cashflows.ScheduleParams;

  /**
   * USD corporate bond: semi-annual, 30/360, Following, USNY.
   *
   * @returns `ScheduleParams` wire object for a plain USD corporate coupon schedule.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsUsdCorporateBond(): generated.cashflows.ScheduleParams;

  /**
   * USD Treasury bond: semi-annual, Act/Act, Following, USNY.
   *
   * @returns `ScheduleParams` wire object for a Treasury-style coupon schedule.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsUsdTreasury(): generated.cashflows.ScheduleParams;

  /**
   * EUR €STR swap leg: annual, Act/360, Modified Following, TARGET2, T+2 payment lag (LCH template).
   *
   * @returns `ScheduleParams` wire object for a €STR-style floating leg.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsEurEstrSwap(): generated.cashflows.ScheduleParams;

  /**
   * EUR government bond: annual, Act/Act, Following, TARGET2.
   *
   * @returns `ScheduleParams` wire object for an annual EUR government coupon schedule.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsEurGovBond(): generated.cashflows.ScheduleParams;

  /**
   * GBP SONIA swap leg: annual, Act/365F, Modified Following, GBLO, no payment lag.
   *
   * @returns `ScheduleParams` wire object for a SONIA-style floating leg.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsGbpSoniaSwap(): generated.cashflows.ScheduleParams;

  /**
   * JPY TONA swap leg: annual, Act/365F, Modified Following, JPTO, T+2 payment lag.
   *
   * @returns `ScheduleParams` wire object for a TONA-style floating leg.
   * @throws If the parameters cannot be serialized.
   */
  scheduleParamsJpyTonaSwap(): generated.cashflows.ScheduleParams;

  /**
   * Check schedule conventions before building a schedule.
   *
   * @param params - `ScheduleParams` wire object.
   * @throws If `params` is not a `ScheduleParams`, its `calendar_id` names no registered holiday calendar (other than `"weekends_only"`), or its payment lag is negative (kind `validation`).
   */
  scheduleParamsValidate(params: generated.cashflows.ScheduleParams): void;
}

/**
 * Namespaced TypeScript entry point for cashflows APIs.
 */
export declare const cashflows: CashflowsNamespace;

// --- covenants -------------------------------------------------------------

/**
 * Covenant package: specifications, waivers and the breach history.
 *
 * Add specifications and waivers, then evaluate metric values on a test
 * date. `evaluate` is read-only; `evaluateAndTrack` also records breaches and
 * cures.
 */
export interface CovenantEngine extends WasmOwned {
  /**
   * Serialize the engine, including its breach history, to canonical JSON.
   *
   * @returns `CovenantEngine` JSON accepted by `fromJson` and by `evaluateEngine`.
   * @throws If the engine cannot be serialized.
   */
  toJson(): string;
  /**
   * Add a covenant specification in place.
   *
   * @param spec - `CovenantSpec` wire object. Its covenant label is the report key and must be unique among specifications applicable on a test date.
   * @throws If `spec` is not a `CovenantSpec` (kind `validation`).
   */
  addSpec(spec: generated.covenants.CovenantSpec): void;
  /**
   * Add a waiver or threshold amendment in place.
   *
   * @param waiver - `CovenantWaiver` wire object: covenant id, effective and optional expiry dates, and an optional amended threshold (omitted means a full waiver).
   * @throws If `waiver` is not a `CovenantWaiver` (kind `validation`).
   */
  addWaiver(waiver: generated.covenants.CovenantWaiver): void;
  /**
   * Check the engine configuration without evaluating.
   *
   * @throws If a specification, window, waiver or breach record is invalid (kind `validation`).
   */
  validate(): void;
  /**
   * Evaluate every applicable covenant on a test date.
   *
   * Read-only: the breach history is not updated.
   *
   * @param metrics - Metric values keyed by covenant metric identifier (plain object or JSON), in the units the tests expect: ratios in turns (`4.5` means 4.5x), amounts in the reporting currency.
   * @param asOf - ISO-8601 test date; it selects the active window, waivers, threshold schedule and cure-period state.
   * @returns Covenant reports keyed by stable covenant instance key, in engine order.
   * @throws If the engine is invalid, two applicable specifications share an instance key, a metric value is not a number (kind `validation`), or a required metric is missing (kind `not_found`).
   */
  evaluate(metrics: Record<string, number> | string, asOf: string): Record<string, CovenantReport>;
  /**
   * Evaluate like `evaluate` and update the breach history.
   *
   * A failing covenant without an active breach gains a breach record with
   * its cure deadline; a later pass inside the cure period marks it cured.
   * On error the history is left untouched.
   *
   * @param metrics - Metric values keyed by covenant metric identifier (plain object or JSON), in the units the tests expect.
   * @param asOf - ISO-8601 test date; it also stamps new breach records.
   * @param scope - `"maintenance"` for scheduled testing, or `"incurrence"` only for a completed action assessed with its pro forma metrics.
   * @returns Covenant reports keyed by stable covenant instance key, in engine order.
   * @throws If `scope` is not one of the listed strings, the engine is invalid, a metric value is not a number (kind `validation`), or a required metric is missing (kind `not_found`).
   */
  evaluateAndTrack(metrics: Record<string, number> | string, asOf: string, scope: generated.covenants.CovenantScope): Record<string, CovenantReport>;
  /**
   * Evaluate the engine on each dated row of metric values.
   *
   * Each row is evaluated independently; the breach history is not updated.
   *
   * @param metrics - `DatedMetrics` rows (`{ date, metrics }`): metric values per ISO-8601 test date, evaluated in the order given. (Python takes a date-indexed DataFrame.)
   * @returns `DatedCovenantReports` rows (`{ as_of, reports }`), one per input row in input order. (Python returns the same reports as a long DataFrame.)
   * @throws If `metrics` is not an array of `DatedMetrics`, the engine is invalid (kind `validation`), or a required metric is missing on a date (kind `not_found`).
   */
  evaluateSeries(metrics: readonly generated.covenants.DatedMetrics[]): generated.covenants.DatedCovenantReports[];
  /**
   * Covenant specifications, in the order they were added.
   * @returns `CovenantSpec` wire objects.
   * @throws If the specifications cannot be serialized.
   */
  readonly specs: generated.covenants.CovenantSpec[];
  /**
   * Waivers and threshold amendments.
   * @returns `CovenantWaiver` wire objects.
   * @throws If the waivers cannot be serialized.
   */
  readonly waivers: generated.covenants.CovenantWaiver[];
  /**
   * Breach records written by `evaluateAndTrack`.
   * @returns `CovenantBreach` wire objects, oldest first; each keeps its original consequences and cure state.
   * @throws If the breach history cannot be serialized.
   */
  readonly breachHistory: generated.covenants.CovenantBreach[];
}

/**
 * Static entry points of the `CovenantEngine` handle.
 * @example
 * ```typescript
 * import init, { covenants } from "finstack-quant-wasm";
 * await init();
 * const engine = covenants.CovenantEngine.fromSpecs(covenants.covLite(7.0, 4.5));
 * const reports = engine.evaluate({ total_leverage: 5.0, senior_leverage: 3.0 }, "2026-03-31");
 * console.log(reports.max_total_leverage.passed);
 * engine.free();
 * ```
 */
export interface CovenantEngineConstructor {
  /**
   * JavaScript prototype of `CovenantEngine`; instances come from `new CovenantEngine()`.
   */
  readonly prototype: CovenantEngine;
  /**
   * Create an empty engine.
   *
   * @returns An engine with no specifications, waivers or breach history.
   */
  new (): CovenantEngine;
  /**
   * Build an engine from covenant specifications.
   *
   * @param specs - `CovenantSpec` wire objects, for example the result of `covLite` or `lboStandard`; they are added in order.
   * @returns A `CovenantEngine` holding the specifications; it is validated when evaluated.
   * @throws If `specs` is not an array of `CovenantSpec` (kind `validation`).
   */
  fromSpecs(specs: readonly generated.covenants.CovenantSpec[]): CovenantEngine;
  /**
   * Parse an engine from its canonical JSON.
   *
   * @param json - `CovenantEngine` JSON string or plain object; only `specs` is required and unknown fields are rejected.
   * @returns The parsed `CovenantEngine`; it is validated when evaluated.
   * @throws If `json` is neither a string nor a plain object (kind `invalid_type`) or does not match the engine schema (kind `validation`).
   */
  fromJson(json: JsonInput): CovenantEngine;
}

/**
 * Covenant definitions, evaluation and forecasting from the Rust
 * `finstack-quant-covenants` crate.
 *
 * `CovenantEngine` is a handle. Covenant definitions, reports and forecasts
 * cross as plain objects that mirror the canonical Rust serde model (types
 * under `./types`), with their Rust constructors and methods as free
 * functions. The `*Json` functions are the JSON-string bridge.
 * @example
 * ```typescript
 * import init, { covenants } from "finstack-quant-wasm";
 * await init();
 * const engine = JSON.parse(covenants.lboStandardJson(6, 2, 1.5, 50_000_000));
 * console.log(engine);
 * ```
 */
export interface CovenantsNamespace {
  /**
   * Validate and canonicalize a covenant spec JSON string.
   * @returns Canonical covenant-spec JSON after schema validation.
   * @param specJson - JSON-serialized covenant specification to validate.
   * @throws Error - Throws a JavaScript exception if `specJson` is malformed, does not match the covenant-spec schema, violates covenant threshold or frequency invariants, or cannot be serialized to canonical JSON.
   */
  validateCovenantSpecJson(specJson: JsonInput): string;
  /**
   * Validate and canonicalize a covenant report JSON string.
   * @returns Canonical covenant-report JSON after schema validation.
   * @param reportJson - JSON-serialized covenant evaluation report to validate.
   * @throws Error - Throws a JavaScript exception if `reportJson` is malformed, does not match the covenant-report schema, or cannot be serialized to canonical JSON.
   */
  validateCovenantReportJson(reportJson: JsonInput): string;
  /**
   * Validate and canonicalize a covenant engine JSON string.
   * @returns Canonical covenant-engine JSON after schema validation.
   * @param engineJson - JSON-serialized covenant engine and its covenant definitions.
   * @throws Error - Throws a JavaScript exception if `engineJson` is malformed, does not match the covenant-engine schema, contains an invalid covenant package, violates engine invariants, or cannot be serialized to canonical JSON.
   */
  validateCovenantEngineJson(engineJson: JsonInput): string;
  /**
   * Evaluate a covenant engine JSON string against a JSON metric map.
   * @returns A plain object keyed by covenant instance key, each value a `CovenantReport`.
   * @param engineJson - JSON-serialized covenant engine and its covenant definitions.
   * @param metricsJson - JSON object of financial metrics referenced by the covenant engine.
   * @param asOf - ISO-8601 date on which every covenant test is evaluated.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed or has the wrong schema, a metric is non-numeric, `asOf` is not a valid ISO date, the engine or required metrics fail validation, or the reports cannot be serialized to JavaScript.
   */
  evaluateEngine(
    engineJson: JsonInput,
    metricsJson: JsonInput,
    asOf: string
  ): Record<string, CovenantReport>;
  /**
   * Standard leveraged-buyout covenant package as JSON.
   * @returns Standard leveraged-buyout covenant package as canonical JSON.
   * @param initialLeverage - Maximum leverage ratio permitted at the initial test date.
   * @param interestCoverage - Minimum EBIT-to-interest coverage ratio in turns.
   * @param fixedChargeCoverage - Minimum EBITDA-to-fixed-charges coverage ratio.
   * @param maxCapex - Maximum annual capital expenditure amount in the caller's reporting currency.
   * @throws Error - Throws a JavaScript exception if any threshold is `NaN`, infinite or negative, or if the generated covenant package cannot be serialized to JSON.
   */
  lboStandardJson(
    initialLeverage: number,
    interestCoverage: number,
    fixedChargeCoverage: number,
    maxCapex: number
  ): string;
  /**
   * Covenant-lite package as JSON.
   * @returns Covenant-lite package as canonical JSON.
   * @param maxLeverage - Maximum total debt-to-EBITDA leverage ratio.
   * @param maxSeniorLeverage - Maximum senior-debt-to-EBITDA leverage ratio.
   * @throws Error - Throws a JavaScript exception if any threshold is `NaN`, infinite or negative, or if the generated covenant package cannot be serialized to JSON.
   */
  covLiteJson(maxLeverage: number, maxSeniorLeverage: number): string;
  /**
   * Real-estate covenant package as JSON.
   * @returns Real-estate covenant package as canonical JSON.
   * @param minDscr - Minimum debt-service coverage ratio.
   * @param minDebtYield - Minimum net-operating-income debt yield expressed as a decimal.
   * @param maxLtv - Maximum loan-to-value ratio expressed as a decimal.
   * @throws Error - Throws a JavaScript exception if any threshold is `NaN`, infinite or negative, or if the generated covenant package cannot be serialized to JSON.
   */
  realEstateJson(minDscr: number, minDebtYield: number, maxLtv: number): string;
  /**
   * Project-finance covenant package as JSON.
   * @returns Project-finance covenant package as canonical JSON.
   * @param minDscr - Minimum debt-service coverage ratio.
   * @param distributionLockupDscr - DSCR threshold below which borrower distributions are locked up.
   * @param minLiquidity - Minimum required liquidity reserve in the model's monetary units.
   * @param maxNetLeverage - Maximum net-debt-to-EBITDA leverage ratio.
   * @throws Error - Throws a JavaScript exception if any threshold is `NaN`, infinite or negative, or if the generated covenant package cannot be serialized to JSON.
   */
  projectFinanceJson(
    minDscr: number,
    distributionLockupDscr: number,
    minLiquidity: number,
    maxNetLeverage: number
  ): string;

  /**
   * `CovenantEngine` handle: covenant package: specifications, waivers and the breach history.
   */
  CovenantEngine: CovenantEngineConstructor;

  /**
   * Standard leveraged-buyout covenant package.
   *
   * Typed twin of `lboStandardJson`: quarterly maximum Debt / EBITDA, minimum
   * interest coverage and minimum fixed-charge coverage tests plus an annual
   * maximum-capex test.
   *
   * @param initialLeverage - Maximum debt-to-EBITDA threshold in turns (`5.0` means 5.0x).
   * @param interestCoverage - Minimum interest-coverage threshold in turns.
   * @param fixedChargeCoverage - Minimum fixed-charge-coverage threshold in turns.
   * @param maxCapex - Maximum annual capital expenditure in the reporting currency.
   * @returns `CovenantSpec` wire objects, ready for `CovenantEngine.fromSpecs`.
   * @throws If a threshold is not a number (kind `invalid_type`), or is NaN, infinite or negative (kind `validation`).
   */
  lboStandard(initialLeverage: number, interestCoverage: number, fixedChargeCoverage: number, maxCapex: number): generated.covenants.CovenantSpec[];

  /**
   * Covenant-lite package: incurrence-style total and senior leverage tests.
   *
   * Typed twin of `covLiteJson`.
   *
   * @param maxLeverage - Maximum total debt-to-EBITDA leverage in turns.
   * @param maxSeniorLeverage - Maximum senior-debt-to-EBITDA leverage in turns.
   * @returns `CovenantSpec` wire objects, ready for `CovenantEngine.fromSpecs`.
   * @throws If a threshold is not a number (kind `invalid_type`), or is NaN, infinite or negative (kind `validation`).
   */
  covLite(maxLeverage: number, maxSeniorLeverage: number): generated.covenants.CovenantSpec[];

  /**
   * Real-estate covenant package: DSCR, debt yield and loan-to-value tests.
   *
   * Typed twin of `realEstateJson`.
   *
   * @param minDscr - Minimum debt-service coverage ratio in turns.
   * @param minDebtYield - Minimum net-operating-income debt yield as a decimal (`0.08` = 8%).
   * @param maxLtv - Maximum loan-to-value ratio as a decimal (`0.65` = 65%).
   * @returns `CovenantSpec` wire objects, ready for `CovenantEngine.fromSpecs`.
   * @throws If a threshold is not a number (kind `invalid_type`), or is NaN, infinite or negative (kind `validation`).
   */
  realEstate(minDscr: number, minDebtYield: number, maxLtv: number): generated.covenants.CovenantSpec[];

  /**
   * Project-finance covenant package: DSCR, distribution lock-up, liquidity and net leverage tests.
   *
   * Typed twin of `projectFinanceJson`.
   *
   * @param minDscr - Minimum debt-service coverage ratio in turns.
   * @param distributionLockupDscr - DSCR below which distributions are locked up, in turns.
   * @param minLiquidity - Minimum liquidity reserve in the reporting currency.
   * @param maxNetLeverage - Maximum net-debt-to-EBITDA leverage in turns.
   * @returns `CovenantSpec` wire objects, ready for `CovenantEngine.fromSpecs`.
   * @throws If a threshold is not a number (kind `invalid_type`), or is NaN, infinite or negative (kind `validation`).
   */
  projectFinance(minDscr: number, distributionLockupDscr: number, minLiquidity: number, maxNetLeverage: number): generated.covenants.CovenantSpec[];

  /**
   * Forecast one numeric covenant over dated metric projections.
   *
   * Deterministic by default: breach probability is `0` for a pass and `1` for
   * a breach. With `config.stochastic` a lognormal overlay on the metric gives
   * probabilities, analytic when `num_paths` is `0` and Monte Carlo otherwise.
   *
   * @param spec - `CovenantSpec` wire object of a numeric covenant.
   * @param metrics - `DatedMetrics` rows (`{ date, metrics }`): projected metric values per ISO-8601 test date, in any order with unique dates. (Python takes a date-indexed DataFrame.)
   * @param config - Optional `CovenantForecastConfig` wire object; omitted means the deterministic Rust default.
   * @returns `CovenantForecast` wire object: per-date projected values, thresholds, headroom and breach probabilities, with the first breach date and minimum headroom.
   * @throws If `metrics` is empty or repeats a date, the covenant is not numeric, the configuration is invalid or an observation is non-finite (kind `validation`), or the covenant's metric is missing on a date (kind `not_found`).
   */
  forecastCovenant(spec: generated.covenants.CovenantSpec, metrics: readonly generated.covenants.DatedMetrics[], config?: generated.covenants.CovenantForecastConfig | null): generated.covenants.CovenantForecast;

  /**
   * Forecast every breach of an engine's covenants over dated metric projections.
   *
   * Effective windows, waivers and threshold schedules are honored;
   * non-numeric covenants are skipped. Deterministic breaches are always
   * listed; in stochastic mode dates whose breach probability reaches
   * `config.breach_probability_threshold` are listed too.
   *
   * @param engine - `CovenantEngine` handle.
   * @param metrics - `DatedMetrics` rows (`{ date, metrics }`): projected metric values per ISO-8601 test date, in any order with unique dates. (Python takes a date-indexed DataFrame.)
   * @param config - Optional `CovenantForecastConfig` wire object; omitted means deterministic maintenance-scope forecasting.
   * @returns `FutureBreach` wire objects sorted by breach date, then covenant id.
   * @throws If `metrics` is empty or repeats a date, the engine or configuration is invalid or an observation is non-finite (kind `validation`), or a required metric is missing on a date (kind `not_found`).
   */
  forecastBreaches(engine: CovenantEngine, metrics: readonly generated.covenants.DatedMetrics[], config?: generated.covenants.CovenantForecastConfig | null): generated.covenants.FutureBreach[];

  /**
   * Copy of a forecast configuration with a different covenant scope.
   *
   * @param config - `CovenantForecastConfig` wire object.
   * @param scope - `"maintenance"` (scheduled tests) or `"incurrence"` (hypothetical capacity for a contemplated action).
   * @returns The updated `CovenantForecastConfig`.
   * @throws If `config` is not a `CovenantForecastConfig` or `scope` is not one of the listed strings (kind `validation`).
   */
  covenantForecastConfigWithScope(config: generated.covenants.CovenantForecastConfig, scope: generated.covenants.CovenantScope): generated.covenants.CovenantForecastConfig;

  /**
   * Maximum total debt / EBITDA covenant.
   *
   * @param threshold - Maximum allowed ratio in turns (`4.5` means 4.5x).
   * @returns `CovenantType` wire value `{ max_debt_to_ebitda: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMaxDebtToEbitda(threshold: number): generated.covenants.CovenantType;

  /**
   * Minimum interest coverage (EBIT / interest) covenant.
   *
   * @param threshold - Minimum required ratio in turns.
   * @returns `CovenantType` wire value `{ min_interest_coverage: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMinInterestCoverage(threshold: number): generated.covenants.CovenantType;

  /**
   * Minimum fixed-charge coverage covenant.
   *
   * @param threshold - Minimum required ratio in turns.
   * @returns `CovenantType` wire value `{ min_fixed_charge_coverage: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMinFixedChargeCoverage(threshold: number): generated.covenants.CovenantType;

  /**
   * Maximum total leverage covenant.
   *
   * @param threshold - Maximum allowed total debt / EBITDA in turns.
   * @returns `CovenantType` wire value `{ max_total_leverage: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMaxTotalLeverage(threshold: number): generated.covenants.CovenantType;

  /**
   * Maximum senior leverage covenant.
   *
   * @param threshold - Maximum allowed senior debt / EBITDA in turns.
   * @returns `CovenantType` wire value `{ max_senior_leverage: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMaxSeniorLeverage(threshold: number): generated.covenants.CovenantType;

  /**
   * Minimum asset coverage covenant.
   *
   * @param threshold - Minimum required asset coverage ratio in turns.
   * @returns `CovenantType` wire value `{ min_asset_coverage: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMinAssetCoverage(threshold: number): generated.covenants.CovenantType;

  /**
   * Minimum debt-service coverage ratio covenant.
   *
   * @param threshold - Minimum required DSCR in turns.
   * @returns `CovenantType` wire value `{ min_dscr: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMinDscr(threshold: number): generated.covenants.CovenantType;

  /**
   * Maximum net debt / EBITDA covenant.
   *
   * @param threshold - Maximum allowed net debt / EBITDA in turns; the specification needs an earnings denominator metric.
   * @returns `CovenantType` wire value `{ max_net_debt_to_ebitda: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMaxNetDebtToEbitda(threshold: number): generated.covenants.CovenantType;

  /**
   * Maximum capital expenditure covenant.
   *
   * @param threshold - Maximum allowed capex amount in the reporting currency.
   * @returns `CovenantType` wire value `{ max_capex: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMaxCapex(threshold: number): generated.covenants.CovenantType;

  /**
   * Minimum liquidity covenant.
   *
   * @param threshold - Minimum required liquidity amount in the reporting currency.
   * @returns `CovenantType` wire value `{ min_liquidity: { threshold } }`.
   * @throws If `threshold` is not a number (kind `invalid_type`).
   */
  covenantTypeMinLiquidity(threshold: number): generated.covenants.CovenantType;

  /**
   * Negative (restrictive) covenant described in words.
   *
   * @param restriction - Description of the restricted action, for example `"no additional senior debt"`.
   * @returns `CovenantType` wire value `{ negative: { restriction } }`; it has no numeric test.
   * @throws If `restriction` is not a string (kind `invalid_type`).
   */
  covenantTypeNegative(restriction: string): generated.covenants.CovenantType;

  /**
   * Affirmative covenant described in words.
   *
   * @param requirement - Description of the required action, for example `"deliver audited financials within 90 days"`.
   * @returns `CovenantType` wire value `{ affirmative: { requirement } }`; it has no numeric test.
   * @throws If `requirement` is not a string (kind `invalid_type`).
   */
  covenantTypeAffirmative(requirement: string): generated.covenants.CovenantType;

  /**
   * Custom covenant on a caller-named metric.
   *
   * @param metric - Metric identifier the test reads, for example `"net_working_capital"`.
   * @param test - `"maximum"` (the metric must not exceed `value`) or `"minimum"` (the metric must reach `value`).
   * @param value - Finite threshold in the metric's own units.
   * @returns `CovenantType` wire value `{ custom: { metric, test } }`.
   * @throws If `metric` or `test` is not a string or `value` is not a number (kind `invalid_type`), or `test` is not one of the listed strings or `value` is non-finite (kind `validation`).
   */
  covenantTypeCustom(metric: string, test: 'maximum' | 'minimum', value: number): generated.covenants.CovenantType;

  /**
   * Basket covenant: a capped amount for a named category.
   *
   * @param name - Basket name, for example `"restricted_payments"`.
   * @param limit - Maximum basket usage in the reporting currency.
   * @returns `CovenantType` wire value `{ basket: { name, limit } }`.
   * @throws If `name` is not a string or `limit` is not a number (kind `invalid_type`).
   */
  covenantTypeBasket(name: string, limit: number): generated.covenants.CovenantType;

  /**
   * Breach consequence: event of default.
   *
   * @returns `CovenantConsequence` wire value `"default"`.
   * @throws If the value cannot be serialized.
   */
  covenantConsequenceDefault(): generated.covenants.CovenantConsequence;

  /**
   * Breach consequence: the borrowing rate steps up.
   *
   * @param bpIncrease - Rate increase in basis points per annum (`200` = +2.00%); must be finite and non-negative when the covenant is validated.
   * @returns `CovenantConsequence` wire value `{ rate_increase: { bp_increase } }`.
   * @throws If `bpIncrease` is not a number (kind `invalid_type`).
   */
  covenantConsequenceRateIncrease(bpIncrease: number): generated.covenants.CovenantConsequence;

  /**
   * Breach consequence: excess cash is swept to repay debt.
   *
   * @param sweepPercentage - Share of excess cash swept, as a decimal in `[0, 1]` (`0.5` = 50%).
   * @returns `CovenantConsequence` wire value `{ cash_sweep: { sweep_percentage } }`.
   * @throws If `sweepPercentage` is not a number (kind `invalid_type`).
   */
  covenantConsequenceCashSweep(sweepPercentage: number): generated.covenants.CovenantConsequence;

  /**
   * Breach consequence: distributions to equity are blocked.
   *
   * @returns `CovenantConsequence` wire value `"block_distributions"`.
   * @throws If the value cannot be serialized.
   */
  covenantConsequenceBlockDistributions(): generated.covenants.CovenantConsequence;

  /**
   * Breach consequence: additional collateral must be posted.
   *
   * @param description - Non-empty description of the required collateral.
   * @returns `CovenantConsequence` wire value `{ require_collateral: { description } }`.
   * @throws If `description` is not a string (kind `invalid_type`).
   */
  covenantConsequenceRequireCollateral(description: string): generated.covenants.CovenantConsequence;

  /**
   * Breach consequence: the maturity is accelerated to a new date.
   *
   * @param newMaturity - ISO-8601 accelerated maturity date.
   * @returns `CovenantConsequence` wire value `{ accelerate_maturity: { new_maturity } }`.
   * @throws If `newMaturity` is not an ISO-8601 string (kind `invalid_type` or `validation`).
   */
  covenantConsequenceAccelerateMaturity(newMaturity: string): generated.covenants.CovenantConsequence;

  /**
   * Create a covenant with the Rust defaults.
   *
   * Mirrors Rust `Covenant::new` (the Python `Covenant(...)` constructor): an
   * active maintenance covenant with no cure period, consequences or
   * springing condition.
   *
   * @param covenantType - `CovenantType` wire value, for example `{ max_debt_to_ebitda: { threshold: 4.5 } }`.
   * @param testFrequency - `Tenor` wire object (`{ count, unit }`) recording how often the covenant is tested; it is metadata, the caller chooses each test date.
   * @param label - Instance label; it is the report key and must be unique within an engine on a test date.
   * @returns `Covenant` wire object.
   * @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
   */
  covenantNew(covenantType: generated.covenants.CovenantType, testFrequency: generated.covenants.Tenor, label: string): generated.covenants.Covenant;

  /**
   * Copy of a covenant with a cure period.
   *
   * @param covenant - `Covenant` wire object.
   * @param days - Calendar days the borrower has to cure a breach (integer); omitted or `null` means no cure period.
   * @returns The updated `Covenant`.
   * @throws If `covenant` is not a `Covenant` (kind `validation`) or `days` is not an integer (kind `invalid_type`).
   */
  covenantWithCurePeriod(covenant: generated.covenants.Covenant, days?: number | null): generated.covenants.Covenant;

  /**
   * Copy of a covenant with one more breach consequence.
   *
   * @param covenant - `Covenant` wire object.
   * @param consequence - `CovenantConsequence` wire value appended to the covenant's consequences.
   * @returns The updated `Covenant`.
   * @throws If either argument does not match its wire type (kind `validation`).
   */
  covenantWithConsequence(covenant: generated.covenants.Covenant, consequence: generated.covenants.CovenantConsequence): generated.covenants.Covenant;

  /**
   * Copy of a covenant with a different scope.
   *
   * @param covenant - `Covenant` wire object.
   * @param scope - `"maintenance"` (tested on a schedule) or `"incurrence"` (tested only when an action is taken).
   * @returns The updated `Covenant`.
   * @throws If `covenant` is not a `Covenant` or `scope` is not one of the listed strings (kind `validation`).
   */
  covenantWithScope(covenant: generated.covenants.Covenant, scope: generated.covenants.CovenantScope): generated.covenants.Covenant;

  /**
   * Copy of a covenant that is tested only while a springing condition holds.
   *
   * @param covenant - `Covenant` wire object.
   * @param condition - `SpringingCondition` wire object (`{ metric_id, test }`), for example revolver utilization above a level; the covenant is inactive while the condition is unmet.
   * @returns The updated `Covenant`.
   * @throws If either argument does not match its wire type (kind `validation`).
   */
  covenantWithSpringingCondition(covenant: generated.covenants.Covenant, condition: generated.covenants.SpringingCondition): generated.covenants.Covenant;

  /**
   * Covenant specification that reads its test value from a named metric.
   *
   * Mirrors Rust `CovenantSpec::with_metric` (the Python `CovenantSpec(...)`
   * constructor with a metric id): a net debt / EBITDA covenant also gets the
   * default `"ebitda"` earnings denominator.
   *
   * @param covenant - `Covenant` wire object.
   * @param metricId - Identifier of the metric the covenant tests, as supplied in the metric map, for example `"debt_to_ebitda"`.
   * @returns `CovenantSpec` wire object.
   * @throws If `covenant` is not a `Covenant` (kind `validation`) or `metricId` is not a string (kind `invalid_type`).
   */
  covenantSpecWithMetric(covenant: generated.covenants.Covenant, metricId: string): generated.covenants.CovenantSpec;

  /**
   * Copy of a specification with an explicit earnings denominator metric.
   *
   * @param spec - `CovenantSpec` wire object of a built-in leverage covenant.
   * @param metricId - Identifier of the earnings metric the leverage ratio divides by, for example `"adjusted_ebitda"`.
   * @returns The updated `CovenantSpec`.
   * @throws If `spec` is not a `CovenantSpec` (kind `validation`) or `metricId` is not a string (kind `invalid_type`).
   */
  covenantSpecWithDenominatorMetric(spec: generated.covenants.CovenantSpec, metricId: string): generated.covenants.CovenantSpec;

  /**
   * Copy of a specification with step-down (or step-up) thresholds.
   *
   * @param spec - `CovenantSpec` wire object.
   * @param schedule - `ThresholdSchedule` wire value: `[isoDate, threshold]` pairs; from each date that threshold replaces the covenant's static one.
   * @returns The updated `CovenantSpec`.
   * @throws If `spec` is not a `CovenantSpec`, or `schedule` has a non-finite threshold or repeats a date (kind `validation`).
   */
  covenantSpecWithThresholdSchedule(spec: generated.covenants.CovenantSpec, schedule: generated.covenants.ThresholdSchedule): generated.covenants.CovenantSpec;

  /**
   * Threshold in force on a test date.
   *
   * @param schedule - `ThresholdSchedule` wire value: `[isoDate, threshold]` pairs.
   * @param testDate - ISO-8601 covenant test date.
   * @returns The threshold of the latest entry effective on or before `testDate`, or `undefined` when the schedule starts later.
   * @throws If `schedule` has a non-finite threshold or repeats a date (kind `validation`), or `testDate` is not an ISO-8601 string (kind `invalid_type` or `validation`).
   */
  thresholdScheduleThresholdFor(schedule: generated.covenants.ThresholdSchedule, testDate: string): number | undefined;
}

/**
 * Namespaced TypeScript entry point for covenants APIs.
 */
export declare const covenants: CovenantsNamespace;

// --- valuations ------------------------------------------------------------

/**
 * Typed bond instrument handle; serialize with `toJson()` for generic pricing entry points.
 *
 * Thin wrapper over the canonical Rust `Bond`. Serialize with `toJson()` and
 * pass the result to `valuations.instruments.priceInstrument` (or the other
 * generic pricing entry points) to price it.
 */
export interface Bond extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Stable instrument identifier.
   */
  readonly id: string;
  /**
   * Serialize to a canonical `finstack_quant.instrument/1` envelope.
   *
   * Pass the result to `valuations.instruments.priceInstrument` (or the
   * other generic pricing entry points) to price this bond.
   * @returns Canonical instrument envelope accepted by `priceInstrument` and `Bond.fromJson`.
   * @throws If serialization fails.
   */
  toJson(): string;
  /**
   * Return a copy of this bond with a different coupon-schedule stub rule.
   *
   * Mirrors Rust `Bond::with_stub`; the receiver is not modified.
   * @param stub - Stub policy: `none`, `short_front`, `short_back`, `long_front`, or `long_back`.
   * @returns A new bond whose coupon schedule uses `stub`.
   * @throws Error - Throws with kind `validation` if `stub` is not a known stub policy.
   */
  withStub(stub: 'none' | 'short_front' | 'short_back' | 'long_front' | 'long_back'): Bond;
  /**
   * Return a copy of this bond with a minimum-MOIC return floor.
   *
   * Mirrors Rust `Bond::min_moic`; the receiver is not modified.
   * @param multiple - Minimum multiple of invested capital the holder receives (e.g. `1.3` for 1.3x), validated by the bond on use.
   * @returns A new bond carrying the return floor.
   * @throws Error - Throws with kind `invalid_type` if `multiple` is not a number.
   */
  minMoic(multiple: number): Bond;
  /**
   * Return a copy of this bond with a minimum-IRR (XIRR) return floor.
   *
   * Mirrors Rust `Bond::min_xirr`; the receiver is not modified.
   * @param rate - Target annualized IRR (e.g. `Rate.fromPercent(12)`).
   * @returns A new bond carrying the return floor.
   */
  minXirr(rate: Rate): Bond;
}

/**
 * Constructor surface for the typed `Bond` WebAssembly instrument.
 * @example
 * ```typescript
 * import init, { core, valuations } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * const bond = valuations.instruments.Bond.fixed(
 *   "BOND-1",
 *   new core.Money(1_000_000, usd),
 *   new core.Rate(0.05),
 *   "2024-01-01",
 *   "2034-01-01",
 *   "none",
 *   "USD-OIS"
 * );
 * const result = valuations.instruments.priceInstrument(bond.toJson(), marketJson, "2024-06-30", "default");
 * ```
 */
export interface BondConstructor {
  /**
   * JavaScript prototype of `Bond`; instances come from the static factories, not `new`.
   */
  readonly prototype: Bond;
  /**
   * Create a US corporate fixed-rate bond (semi-annual, 30/360, T+1).
   * Mirrors Rust `Bond::fixed` and requires an explicit stub policy.
   * @param id - Unique instrument identifier.
   * @param notional - Principal amount of the bond.
   * @param couponRate - Annual coupon rate.
   * @param issueDate - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param stub - Stub policy: `none`, `short_front`, `short_back`, `long_front`, or `long_back`.
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns The validated fixed-rate bond.
   * @throws If validation fails (e.g. maturity not after issue_date).
   */
  fixed(
    id: string,
    notional: Money,
    couponRate: Rate,
    issueDate: string,
    maturity: string,
    stub: 'none' | 'short_front' | 'short_back' | 'long_front' | 'long_back',
    discountCurveId: string
  ): Bond;
  /**
   * Create a fixed-rate bond from a named market convention preset.
   *
   * Mirrors Rust `Bond::with_convention`: frequency, day count, calendar,
   * business-day convention, settlement lag and stub rule all come from the
   * preset. Chain `withStub` to override the preset's stub rule.
   * @param id - Unique instrument identifier.
   * @param notional - Principal amount of the bond.
   * @param couponRate - Annual coupon rate.
   * @param issueDate - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param convention - Bond convention preset: `us_treasury`, `us_agency`, `german_bund`, `uk_gilt`, `french_oat`, `jgb`, `us_corporate`, or `eur_corporate`.
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns The validated fixed-rate bond.
   * @throws Error - Throws with kind `validation` if `convention` is not a known preset, a date is malformed, or bond validation fails (e.g. maturity not after issue_date).
   */
  withConvention(
    id: string,
    notional: Money,
    couponRate: Rate,
    issueDate: string,
    maturity: string,
    convention:
      | 'us_treasury'
      | 'us_agency'
      | 'german_bund'
      | 'uk_gilt'
      | 'french_oat'
      | 'jgb'
      | 'us_corporate'
      | 'eur_corporate',
    discountCurveId: string
  ): Bond;
  /**
   * Create a floating-rate bond (FRN) linked to a forward index. Mirrors Rust `Bond::floating`. Settlement, calendar, and business-day convention come from the notional currency: USD UsCorporate (T+1, usny), EUR EurCorporate (T+2, target2), GBP UkGilt (T+1), JPY Jgb (T+2). Unmapped currencies throw.
   * @param id - Unique instrument identifier.
   * @param notional - Principal amount of the bond.
   * @param forwardCurveId - Forward curve identifier (e.g. `"USD-SOFR-3M"`).
   * @param spreadBp - Spread over the index in whole basis points (`Bps` rejects fractional values; use `Bond.fromJson` for sub-bp margins, which preserves the exact decimal spread).
   * @param issueDate - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param frequency - Payment frequency (e.g. `Tenor.quarterly()`).
   * @param dayCount - Day count convention (e.g. `DayCount.act360()`).
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns The validated floating-rate note.
   * @throws If the notional currency has no mapped settlement convention or validation fails.
   */
  floating(
    id: string,
    notional: Money,
    forwardCurveId: string,
    spreadBp: Bps,
    issueDate: string,
    maturity: string,
    frequency: Tenor,
    dayCount: DayCount,
    discountCurveId: string
  ): Bond;
  /**
   * Deserialize a bond from its canonical v1 instrument envelope.
   *
   * Bare payloads are rejected; the loader's validation runs on the result.
   * @param json - A `finstack_quant.instrument/1` envelope containing type `"bond"`.
   * @returns The validated bond.
   * @throws If the JSON is malformed, has a different instrument type, or fails validation.
   */
  fromJson(json: JsonInput): Bond;
  /**
   * Create a floating-rate bond (FRN) from a named market convention preset.
   *
   * Mirrors Rust `Bond::floating_with_convention`: calendar, business-day
   * convention, settlement lag and stub rule come from the preset.
   * @param id - Unique instrument identifier.
   * @param notional - Principal amount of the bond.
   * @param forwardCurveId - Forward curve identifier (e.g. `"USD-SOFR-3M"`).
   * @param spreadBp - Spread over the index in whole basis points.
   * @param issueDate - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param frequency - Payment frequency (e.g. `Tenor.quarterly()`).
   * @param dayCount - Day count convention (e.g. `DayCount.act360()`).
   * @param convention - Bond convention preset: `us_treasury`, `us_agency`, `german_bund`, `uk_gilt`, `french_oat`, `jgb`, `us_corporate`, or `eur_corporate`.
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns The validated floating-rate note.
   * @throws Error - Throws with kind `validation` if `convention` is not a known preset, a date is malformed, or bond validation fails.
   */
  floatingWithConvention(
    id: string,
    notional: Money,
    forwardCurveId: string,
    spreadBp: Bps,
    issueDate: string,
    maturity: string,
    frequency: Tenor,
    dayCount: DayCount,
    convention:
      | 'us_treasury'
      | 'us_agency'
      | 'german_bund'
      | 'uk_gilt'
      | 'french_oat'
      | 'jgb'
      | 'us_corporate'
      | 'eur_corporate',
    discountCurveId: string
  ): Bond;
  /**
   * Create a zero-coupon bond that pays `notional` at maturity (mirrors Rust `Bond::zero_coupon`).
   * @param id - Unique instrument identifier.
   * @param notional - Principal repaid at maturity.
   * @param issueDate - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns The validated zero-coupon bond.
   * @throws Error - Throws with kind `validation` if a date is malformed or bond validation fails (e.g. maturity not after issue_date).
   */
  zeroCoupon(id: string, notional: Money, issueDate: string, maturity: string, discountCurveId: string): Bond;
  /**
   * Canonical example bond (mirrors Rust `Bond::example`): a 10-year US Treasury, USD 1,000,000 at 4.25%.
   * @returns The example bond.
   * @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
   */
  example(): Bond;
  /**
   * Canonical example floating-rate note (mirrors Rust `Bond::example_floating`): USD SOFR 3M + 150bp.
   * @returns The example floating-rate note.
   * @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
   */
  exampleFloating(): Bond;
  /**
   * Canonical example callable bond (mirrors Rust `Bond::example_callable`).
   * @returns The example callable bond.
   * @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
   */
  exampleCallable(): Bond;
  /**
   * Canonical example amortizing bond (mirrors Rust `Bond::example_amortizing`).
   * @returns The example amortizing bond.
   * @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
   */
  exampleAmortizing(): Bond;
}

/**
 * Typed term-loan instrument handle; serialize with `toJson()` for generic pricing entry points.
 *
 * Thin wrapper over the canonical Rust `TermLoan`. Serialize with `toJson()`
 * and pass the result to `valuations.instruments.priceInstrument` (or the
 * other generic pricing entry points) to price it.
 */
export interface TermLoan extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Stable instrument identifier.
   */
  readonly id: string;
  /**
   * Serialize to a canonical `finstack_quant.instrument/1` envelope.
   *
   * Pass the result to `valuations.instruments.priceInstrument` (or the
   * other generic pricing entry points) to price this loan.
   * @returns Canonical instrument envelope accepted by `priceInstrument` and `TermLoan.fromJson`.
   * @throws If serialization fails.
   */
  toJson(): string;
}

/**
 * Constructor surface for the typed `TermLoan` WebAssembly instrument.
 *
 * Rust has no `fixed`/`floating` convenience constructors for term loans;
 * construct via `fromJson` with a canonical v1 instrument envelope or start
 * from `example()`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const loan = valuations.instruments.TermLoan.example();
 * const result = valuations.instruments.priceInstrument(loan.toJson(), marketJson, "2024-06-30", "default");
 * ```
 */
export interface TermLoanConstructor {
  /**
   * JavaScript prototype of `TermLoan`; instances come from the static factories, not `new`.
   */
  readonly prototype: TermLoan;
  /**
   * Deserialize a term loan from its canonical v1 instrument envelope.
   *
   * Bare payloads are rejected; the loader's validation runs on the result.
   * @param json - A `finstack_quant.instrument/1` envelope containing type `"term_loan"`.
   * @returns The validated term loan.
   * @throws If the JSON is malformed, has a different instrument type, or fails validation.
   */
  fromJson(json: JsonInput): TermLoan;
  /**
   * Canonical example term loan (mirrors Rust `TermLoan::example`).
   *
   * Returns a 5-year USD fixed-rate loan (6%, quarterly, Act/360, 2.5%
   * per-period amortization) useful as a starting point and in tests.
   * @returns The example loan.
   * @throws If construction fails (should not occur).
   */
  example(): TermLoan;
  /**
   * Canonical example floating-rate term loan with a delayed-draw tranche (mirrors Rust `TermLoan::example_floating_with_ddtl`).
   * @returns The example loan.
   * @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
   */
  exampleFloatingWithDdtl(): TermLoan;
  /**
   * Canonical example callable term loan (mirrors Rust `TermLoan::example_callable`).
   * @returns The example loan.
   * @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
   */
  exampleCallable(): TermLoan;
}

/**
 * Typed asset-backed facility handle (a warehouse line against a collateral pool); serialize with `toJson()` for generic pricing entry points.
 *
 * Thin wrapper over the canonical Rust `AssetBackedFacility`. Serialize with
 * `toJson()` and pass the result to `valuations.instruments.priceInstrument`
 * (or the other generic pricing entry points) to price the lender's flows;
 * the facility's borrowing-base metrics (`abf_borrowing_base`,
 * `abf_borrowing_base_cushion`, `abf_advance_rate_utilization`,
 * `abf_facility_irr`, `abf_residual_irr`) are available there too.
 */
export interface AssetBackedFacility extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Stable instrument identifier.
   */
  readonly id: string;
  /**
   * Serialize to a canonical `finstack_quant.instrument/1` envelope.
   * @returns Canonical instrument envelope accepted by `priceInstrument` and `AssetBackedFacility.fromJson`.
   * @throws If serialization fails.
   */
  toJson(): string;
  /**
   * Borrowing base on the closing collateral.
   * @returns Plain `BorrowingBaseReport` object with `eligible_collateral`, `concentration_excess` and `borrowing_base` Money values.
   * @throws If the borrowing-base rules are malformed.
   */
  borrowingBase(): BorrowingBaseReport;
  /**
   * The two-class structured-credit deal the engine runs for this facility (mirrors Rust `AssetBackedFacility::synthesized_deal`): facility note plus residual, borrowing-base test, reinvestment window, early-amortization rules and the term-out call.
   * @returns Canonical `finstack_quant.instrument/1` envelope JSON of type `structured_credit`, accepted by `priceInstrument`.
   * @throws Error - Throws with kind `validation` if the facility or the synthetic deal fails validation.
   */
  synthesizedDealJson(): string;
  /**
   * Undrawn commitment at closing: `commitment - drawn`.
   * @returns Undrawn amount in the commitment currency.
   * @throws Error - Throws with kind `validation` if commitment and drawn amounts differ in currency.
   */
  readonly undrawn: Money;
  /**
   * Effective end of revolving: the scheduled `revolving_end` or the earliest scheduled date amortization event, whichever is first.
   * @returns ISO-8601 date string (`"YYYY-MM-DD"`).
   */
  readonly effectiveRevolvingEnd: string;
  /**
   * Project the facility and residual cashflows through the synthetic two-class structured-credit deal (mirrors Rust `AssetBackedFacility::project`).
   * @param marketJson - Serialized `MarketContext` with the curves and fixings the collateral and facility coupon project from.
   * @param asOf - Valuation date as an ISO-8601 string; the projection starts here.
   * @returns Plain `FacilityProjection` object: `facility` and `residual` tranche cashflows, `commitment_fees` and `draws` as `[date, Money]` pairs, and the per-period `diagnostics`.
   * @throws Error - Throws with kind `validation` if the facility fails validation or an input is malformed, and kind `not_found` if market data is missing.
   */
  project(marketJson: JsonInput, asOf: string): FacilityProjection;
  /**
   * Lender IRR: XIRR of `-drawn` on `asOf` against every projected interest, principal and fee receipt (mirrors Rust `AssetBackedFacility::facility_irr`).
   * @param marketJson - Serialized `MarketContext` with the curves and fixings for the projection.
   * @param asOf - Valuation date as an ISO-8601 string; the investment is dated here.
   * @returns Annual IRR as a decimal (`0.08` = 8%).
   * @throws Error - Throws with kind `validation` if an input is malformed, kind `not_found` if market data is missing, and kind `computation` if the XIRR does not converge.
   */
  facilityIrr(marketJson: JsonInput, asOf: string): number;
}

/**
 * Constructor surface for the typed `AssetBackedFacility` WebAssembly instrument.
 *
 * Construct via `fromJson` with a canonical v1 instrument envelope or start
 * from `example()`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const facility = valuations.instruments.AssetBackedFacility.example();
 * const base = facility.borrowingBase();
 * const result = valuations.instruments.priceInstrument(facility.toJson(), marketJson, "2024-01-15", "default");
 * ```
 */
export interface AssetBackedFacilityConstructor {
  /**
   * JavaScript prototype of `AssetBackedFacility`; instances come from the static factories, not `new`.
   */
  readonly prototype: AssetBackedFacility;
  /**
   * Parse a canonical `finstack_quant.instrument/1` envelope whose instrument is an `asset_backed_facility`.
   * @param json - Canonical instrument envelope JSON.
   * @returns The typed facility.
   * @throws If the JSON is malformed, has a different instrument type, or fails validation.
   */
  fromJson(json: JsonInput): AssetBackedFacility;
  /**
   * The canonical example facility: the example CLO pool financed by a USD 80M commitment drawn USD 70M.
   * @returns The example facility.
   * @throws If construction fails (should not occur).
   */
  example(): AssetBackedFacility;
}

/**
 * Typed revolving-credit facility handle; serialize with `toJson()` for generic pricing entry points.
 *
 * Thin wrapper over the canonical Rust `RevolvingCredit`. Serialize with
 * `toJson()` and pass the result to `valuations.instruments.priceInstrument`
 * (or the other generic pricing entry points) to price it.
 */
export interface RevolvingCredit extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Stable instrument identifier.
   */
  readonly id: string;
  /**
   * Serialize to a canonical `finstack_quant.instrument/1` envelope.
   *
   * Pass the result to `valuations.instruments.priceInstrument` (or the
   * other generic pricing entry points) to price this facility.
   * @returns Canonical instrument envelope accepted by `priceInstrument` and `RevolvingCredit.fromJson`.
   * @throws If serialization fails.
   */
  toJson(): string;
  /**
   * Price a stochastic facility with the path-retaining Monte Carlo engine.
   *
   * Mirrors Rust `RevolvingCreditPricer::price_with_paths` and Python
   * `RevolvingCredit.price_with_paths`: every simulated path is kept with
   * its present value, utilization and credit-spread samples, cashflows and
   * draw option cost, next to the antithetic-aware estimates. The draw
   * option cost is negative when draws at the fixed margin are worth less
   * than at the path's fair spread.
   * @param marketJson - Serialized `MarketContext` holding the facility's discount curve, its floating index forward curve and fixings, and the credit curve of a market-anchored spread process.
   * @param asOf - Valuation date as an ISO 8601 `YYYY-MM-DD` string.
   * @returns Plain `EnhancedMonteCarloResult` object with `mc_result`, one `path_results` entry per simulated path and the `draw_option_cost` estimate; path counts are plain numbers and `mc_result.run` is `null`.
   * @throws If the market JSON or date is malformed, a required curve or fixing is missing, or the facility has a deterministic draw schedule (only stochastic facilities simulate paths).
   */
  priceWithPaths(marketJson: JsonInput, asOf: string): EnhancedMonteCarloResult;
  /**
   * Whether the draw/repay schedule is stochastic (Monte Carlo) rather than deterministic (mirrors Rust `RevolvingCredit::is_stochastic`).
   */
  readonly isStochastic: boolean;
  /**
   * Cashflow schedule of the facility: the contractual schedule for a deterministic draw/repay spec, the path-averaged schedule for a stochastic one.
   * @param marketJson - Serialized `MarketContext` with the curves the schedule projects from.
   * @param asOf - Valuation date as an ISO-8601 string; flows are projected from here.
   * @returns Plain `CashFlowSchedule` object with dated interest, fee and principal flows from the lender's perspective (draws negative, repayments positive).
   * @throws Error - Throws with kind `validation` if the facility fails validation or an input is malformed, and kind `not_found` if a required curve is missing.
   */
  expectedCashflows(marketJson: JsonInput, asOf: string): generated.valuations.CashFlowSchedule;
}

/**
 * Constructor surface for the typed `RevolvingCredit` WebAssembly instrument.
 *
 * Construct via `fromJson` with a canonical v1 instrument envelope or start
 * from `example()`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const facility = valuations.instruments.RevolvingCredit.example();
 * const result = valuations.instruments.priceInstrument(facility.toJson(), marketJson, "2024-06-30", "default");
 * ```
 */
export interface RevolvingCreditConstructor {
  /**
   * JavaScript prototype of `RevolvingCredit`; instances come from the static factories, not `new`.
   */
  readonly prototype: RevolvingCredit;
  /**
   * Deserialize a revolving credit facility from its canonical v1 instrument envelope.
   *
   * Bare payloads are rejected; the loader's validation runs on the result.
   * @param json - A `finstack_quant.instrument/1` envelope containing type `"revolving_credit"`.
   * @returns The validated facility.
   * @throws If the JSON is malformed, has a different instrument type, or fails validation.
   */
  fromJson(json: JsonInput): RevolvingCredit;
  /**
   * Canonical example facility (mirrors Rust `RevolvingCredit::example`).
   *
   * Returns a three-year USD 50M SOFR + 250bp facility with USD 10M drawn
   * and a scheduled draw and repayment, useful as a starting point and in tests.
   * @returns The example facility.
   * @throws If construction fails (should not occur).
   */
  example(): RevolvingCredit;
}

/**
 * Convergence and reproducibility diagnostics for a Monte Carlo valuation.
 */
export type MonteCarloValuationDetails =
  import('./types/valuation-result.js').MonteCarloValuationDetails;

/**
 * Model-specific native detail, generated from Rust with exact host integer representations.
 */
export type ValuationDetails = import('./types/valuation-result.js').ValuationDetails;

/**
 * Listed-market product coverage and exchange routing metadata.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const rows = valuations.market.listedProductCatalog("cme");
 * console.log(rows.every((row) => row.exchange === "cme"));
 * ```
 */
export interface ValuationMarketNamespace {
  /**
   * Return the maintained liquid listed-derivatives coverage catalog.
   * @param exchange - Optional exact filter: `"cme"`, `"eurex"`, `"montreal"`, or `"sgx"`.
   * @returns Product-family coverage rows with instrument routes and official source URLs.
   * @throws Error - Throws when `exchange` is unsupported, the embedded listed-product sidecar is invalid, or rows cannot be converted to JavaScript.
   */
  listedProductCatalog(
    exchange?: 'cme' | 'eurex' | 'montreal' | 'sgx' | null
  ): ListedProductCoverage[];
}

/**
 * Namespaced TypeScript entry points for valuation instruments calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const models = valuations.instruments.listModels();
 * console.log(models.includes("discounting"));
 * ```
 */
export interface ValuationInstrumentsNamespace {
  /**
   * Typed `Bond` instrument class (see `BondConstructor`).
   */
  Bond: BondConstructor;
  /**
   * Typed `TermLoan` instrument class (see `TermLoanConstructor`).
   */
  TermLoan: TermLoanConstructor;
  /**
   * Typed `RevolvingCredit` instrument class (see `RevolvingCreditConstructor`).
   */
  RevolvingCredit: RevolvingCreditConstructor;
  /**
   * Typed `AssetBackedFacility` instrument class (see `AssetBackedFacilityConstructor`).
   */
  AssetBackedFacility: AssetBackedFacilityConstructor;
  /**
   * Construct a canonical bond instrument envelope from a cashflow schedule.
   * @returns Canonical bond instrument envelope JSON.
   * @param instrumentId - Stable instrument identifier used for pricing and metric keys.
   * @param scheduleJson - Canonical cashflow-schedule JSON used to construct the fixed-income instrument.
   * @param discountCurveId - Market-context discount-curve identifier for the instrument currency.
   * @param quotedCleanPricePct - Optional observed clean bond price in percent of par (99.5 = 99.5%), stored as `instrument_pricing_overrides.market_quotes.quoted_clean_price_pct`.
   * @throws Error - Throws a JavaScript exception if `scheduleJson` is malformed or violates cash-flow invariants, bond construction fails, or the canonical bond envelope cannot be serialized.
   */
  bondFromCashflowsJson(
    instrumentId: string,
    scheduleJson: JsonInput,
    discountCurveId: string,
    quotedCleanPricePct?: number | null
  ): string;
  /**
   * Validate a canonical v1 instrument envelope after optional metric-pricing
   * overrides are merged by the canonical pricing path.
   *
   * Bare instrument payloads are rejected. Returns canonical re-serialized JSON.
   * @returns Canonical instrument envelope JSON after schema validation.
   * @param json - Required `finstack_quant.instrument/1` envelope.
   * @param metricPricingOverrides - Serialized metric-pricing override object merged before native instrument validation; `None` (omitted or null in JavaScript) retains the envelope configuration.
   * @throws Error - Throws a JavaScript exception if the instrument or override JSON is malformed, the merged payload is not a canonical v1 instrument envelope, instrument validation fails, or the envelope cannot be canonically serialized.
   */
  validateInstrumentJson(json: JsonInput, metricPricingOverrides?: JsonInput | null): string;
  /**
   * Price an instrument from its canonical envelope and return a `ValuationResult` object.
   *
   * Pass `model = "default"` to use the instrument-native default model.
   * Fields are readable directly (`result.value.amount`,
   * `result.measures.dv01`). Monte Carlo results carry a lossless `bigint`
   * seed; serialize them with `valuations.valuationResultToJson`.
   * For bonds, `"discounting"` is non-callable rates-only PV,
   * `"hazard_rate"` is non-callable fractional recovery of par, `"tree"`
   * values rates-only exercise rights, and `"rates_credit"` values joint
   * rates-credit bonds including call, put, and return floors. Stochastic `"rates_credit"` runs add
   * `type: "monte_carlo"` diagnostics to `result.details`.
   * @param instrumentJson - Required `finstack_quant.instrument/1` envelope.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit for the instrument-native model.
   * @param metrics - Optional canonical metric IDs such as `"ytm"`, `"dv01"`, `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a valuation-only result. Mortgage OAS and CMO Z-spread consume clean prices per 100 current face and include settlement accrued interest. MBS DV01, bucketed DV01 and duration share rate-dependent prepayment assumptions. FI TRS duration DV01 requires `duration_id` and a finite signed scalar in years. Roll specialness is in basis points versus `repo_curve_id` (a discount curve), or the discount curve when omitted; implied financing is an ACT/360 decimal.
   * @param metricPricingOverrides - Optional JSON metric-pricing overrides merged into the envelope before validation. Omit, `null`, or `undefined` to use the envelope as-is.
   * @param marketHistory - Optional serialized market-history JSON required by historical risk metrics such as historical VaR.
   * @returns Plain JavaScript `ValuationResult` (`instrument_id`, `as_of`, `value`, `measures`, `meta`, …).
   * @throws Error - Throws a JavaScript exception if an instrument, market, metric-pricing-override, or market-history payload is invalid; `metrics` is not a string array; `asOf`, `model`, or a metric identifier is invalid; required market data is missing; pricing or a metric calculation fails; or the valuation cannot be converted to a JavaScript value.
   */
  priceInstrument(
    instrumentJson: JsonInput,
    marketJson: JsonInput,
    asOf: string,
    model?: string | null,
    metrics?: string[] | null,
    metricPricingOverrides?: JsonInput | null,
    marketHistory?: JsonInput | null
  ): ValuationResult;
  /**
   * Price an instrument using a pre-parsed `core.MarketContext` handle.
   *
   * Avoids the per-call market-parse overhead of `priceInstrument`.
   * For bonds, `"discounting"` is non-callable rates-only PV,
   * `"hazard_rate"` is non-callable fractional recovery of par, `"tree"`
   * values rates-only exercise rights, and `"rates_credit"` values joint
   * rates-credit bonds including call, put, and return floors. Stochastic `"rates_credit"` runs add
   * `type: "monte_carlo"` diagnostics to `result.details`.
   * Their `seed` is a lossless `bigint`; serialize such results with
   * `valuations.valuationResultToJson`.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data for this call.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit, `null`, or `"default"` for the instrument-native model.
   * @param metrics - Optional canonical metric IDs such as `"ytm"`, `"dv01"`, `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a valuation-only result.
   * @param metricPricingOverrides - Optional JSON metric-pricing overrides merged into the envelope before validation. Omit, `null`, or `undefined` to use the envelope as-is.
   * @param marketHistory - Optional serialized market-history JSON required by historical risk metrics such as historical VaR.
   * @returns Plain JavaScript `ValuationResult` (`instrument_id`, `as_of`, `value`, `measures`, `meta`, …).
   * @throws Error - Throws a JavaScript exception if an instrument, metric-pricing-override, or market- history payload is invalid; `metrics` is not a string array; `asOf`, `model`, or a metric identifier is invalid; required market data is missing; pricing or a metric calculation fails; or the valuation cannot be converted to a JavaScript value.
   */
  priceInstrumentWithMarket(
    instrumentJson: JsonInput,
    market: MarketContext,
    asOf: string,
    model?: string | null,
    metrics?: string[] | null,
    metricPricingOverrides?: JsonInput | null,
    marketHistory?: JsonInput | null
  ): ValuationResult;
  /**
   * Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.
   *
   * `model` must be `"discounting"` or `"hazard_rate"`. Unsupported models or
   * incompatible instrument types throw. Hazard-rate export also rejects bonds
   * with call, put, or return-floor rights because static rows cannot represent
   * exercise-contingent value. For supported static-flow pairs, the envelope's
   * `total_pv` matches the instrument's `base_value` within rounding.
   * @returns Per-flow cashflow envelope JSON (discount factor, survival, PV).
   * @param instrumentJson - Required `finstack_quant.instrument/1` envelope.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON or `asOf` is invalid, `model` is unsupported or incompatible with the instrument, a bond with embedded exercise rights is requested under a static cashflow model, required curves are missing, the schedule mixes currencies, canonical pricing fails, or the cash-flow envelope cannot be serialized.
   */
  instrumentCashflowsJson(
    instrumentJson: JsonInput,
    marketJson: JsonInput,
    asOf: string,
    model: string
  ): string;
  /**
   * Per-flow cashflow envelope using a pre-parsed `core.MarketContext` handle. Hazard-rate export
   * rejects bonds with call, put, or return-floor rights because static rows
   * cannot represent exercise-contingent value.
   * @returns Per-flow cashflow envelope JSON using the pre-parsed market.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
   * @throws Error - Throws a JavaScript exception if `instrumentJson` or `asOf` is invalid, `model` is unsupported or incompatible with the instrument, a bond with embedded exercise rights is requested under a static cashflow model, required curves are missing, the schedule mixes currencies, canonical pricing fails, or the cash-flow envelope cannot be serialized.
   */
  instrumentCashflowsWithMarketJson(
    instrumentJson: JsonInput,
    market: MarketContext,
    asOf: string,
    model: string
  ): string;
  /**
   * Per-flow cashflow envelope for an instrument, as a plain object (typed twin of `instrumentCashflowsJson`; `JSON.stringify` of the result equals that string).
   * @param instrumentJson - Required `finstack_quant.instrument/1` envelope.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
   * @returns `InstrumentCashflowEnvelope` with one row per flow and the total PV.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON or `asOf` is invalid, `model` is unsupported or incompatible with the instrument, a bond with embedded exercise rights is requested under a static cashflow model, required curves are missing, the schedule mixes currencies, or canonical pricing fails.
   */
  instrumentCashflows(
    instrumentJson: JsonInput,
    marketJson: JsonInput,
    asOf: string,
    model: string
  ): generated.valuations.InstrumentCashflowEnvelope;
  /**
   * Per-flow cashflow envelope using a pre-parsed `core.MarketContext` handle, as a plain object (typed twin of `instrumentCashflowsWithMarketJson`).
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
   * @returns `InstrumentCashflowEnvelope` with one row per flow and the total PV.
   * @throws Error - Throws a JavaScript exception if `instrumentJson` or `asOf` is invalid, `model` is unsupported or incompatible with the instrument, a bond with embedded exercise rights is requested under a static cashflow model, required curves are missing, the schedule mixes currencies, or canonical pricing fails.
   */
  instrumentCashflowsWithMarket(
    instrumentJson: JsonInput,
    market: MarketContext,
    asOf: string,
    model: string
  ): generated.valuations.InstrumentCashflowEnvelope;
  /**
   * List every pricing model key registered in the standard pricer registry.
   *
   * The list is registry-derived rather than enum-derived, so it reflects real
   * dispatch coverage: a model with no registered pricer is omitted. Returns a
   * sorted array of canonical keys (`"discounting"`, `"rates_credit"`, …)
   * accepted by the `model` argument of `priceInstrument`.
   * @returns Returns the resulting `string[]` collection in ascending model-key order.
   * @throws Error - Throws a JavaScript exception if the model key list cannot be converted to a JavaScript value.
   */
  listModels(): string[];
  /**
   * List the standard registry's pricing models grouped by instrument type.
   *
   * Returns a JSON object `{ instrument_type: [model_key, ...], ... }`. Only
   * instrument types with at least one registered pricer appear, and each
   * entry lists only the models that can actually price that instrument. The
   * `"bond"` entry includes `"discounting"`, `"hazard_rate"`, `"tree"`, and
   * `"rates_credit"`.
   * @returns Returns the resulting `Record<string, string[]>` value keyed by instrument type.
   * @throws Error - Throws a JavaScript exception if the grouped model registry cannot be converted to a JavaScript value.
   */
  listModelsGrouped(): Record<string, string[]>;
  /**
   * List all metric IDs in the standard metric registry.
   * @returns Canonical metric identifiers, sorted alphabetically.
   * @throws Error - Throws a JavaScript exception if the metric identifier list cannot be converted to a JavaScript value.
   */
  listStandardMetrics(): string[];
  /**
   * List all standard metrics organized by group.
   *
   * Returns a JSON object `{ group_name: [metric_id, ...], ... }` where
   * each key is a human-readable group name (e.g. "Pricing", "Greeks",
   * "Sensitivity") and the value is a sorted array of metric ID strings.
   * @returns Metric identifiers grouped by human-readable group name.
   * @throws Error - Throws a JavaScript exception if the grouped metric registry cannot be converted to a JavaScript value.
   */
  listStandardMetricsGrouped(): Record<string, string[]>;
  /**
   * Describe canonical metric identifiers using Rust-owned units and coordinates.
   * @example
   * ```typescript
   * import init, { valuations } from "finstack-quant-wasm";
   * await init();
   * const [ytm, bucket] = valuations.instruments.metricMetadata([
   *   'ytm',
   *   'bucketed_dv01::USD-OIS::10y',
   * ]);
   * console.log(ytm.unit, bucket.components, bucket.bucketed);
   * ```
   * @param keys - Canonical scalar or qualified wire keys in output order. Custom keys retain unknown units; malformed or obsolete encodings are rejected.
   * @returns Ordered metadata records preserving original keys, decoded components, native unit families, display groups, and bucket eligibility.
   * @throws Error - Throws a validation error for malformed canonical metric keys or a serialization error when conversion fails.
   */
  metricMetadata(keys: string[]): MetricMetadata[];
  /**
   * Z-spread-equivalent discount margin for a floating-rate tranche, returned in
   * decimal units (`0.015` = 150 bp).
   *
   * Contractual cashflows are projected without changing coupon projection,
   * then a constant additive spread is applied to the discount curve. The result
   * is zero at the model price, negative for a richer (higher)
   * `marketPricePct`, and positive for a cheaper (lower) `marketPricePct`; it is
   * not the contractual quoted margin.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Identifier of the floating-rate tranche whose contractual cashflows are spread-discounted.
   * @param marketJson - Canonical market-context JSON supplying the discount curve and any forward curves or historical fixings required for cashflow projection.
   * @param asOf - ISO-8601 valuation date used for projection and discounting.
   * @param marketPricePct - Clean settlement price as a percentage of the tranche's CURRENT balance (100.0 = par); accrued interest is added once at the deal's quote_settlement_date or the valuation date.
   * @returns The z-spread-equivalent discount margin in decimal units.
   * @throws Error - Thrown if JSON or the date is malformed, the deal is invalid, the tranche is missing or fixed-rate, market_price_pct is not finite and positive, required market data is unavailable, or the spread solve fails or exceeds ±5000 bp.
   */
  structuredCreditTrancheDiscountMargin(
    instrumentJson: JsonInput,
    trancheId: string,
    marketJson: JsonInput,
    asOf: string,
    marketPricePct: number
  ): number;
  /**
   * Break-even constant default rate (CDR, decimal) for a tranche — the highest
   * CDR at which the tranche takes no principal writedown.
   * @returns Break-even constant default rate as a decimal, such as `0.02` for 2% CDR.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON is malformed; the instrument fails pricing validation or is not a structured-credit deal; `as_of` is invalid; the tranche or required market data is missing; or the break-even calculation fails.
   */
  structuredCreditTrancheBreakevenCdr(
    instrumentJson: JsonInput,
    trancheId: string,
    marketJson: JsonInput,
    asOf: string
  ): number;
  /**
   * Option-adjusted spread for a tranche; returns a typed `OasResult` object.
   *
   * The result is a plain object with snake_case fields — the same shape
   * Python exposes through its typed `OasResult` wrapper. Pass it to
   * `JSON.stringify` if a wire string is needed.
   *
   * `marketPricePct` is the clean settlement quote as a percentage of CURRENT balance.
   * `config`, when present, is a JSON `OasConfig`; the default is used otherwise.
   * @returns Typed `OasResult` object for the tranche.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param marketPricePct - Clean settlement quote as a percentage of CURRENT balance; accrued interest is added once.
   * @throws Error - Throws a JavaScript exception if the instrument, market, or optional configuration JSON is malformed; the instrument fails pricing validation; `as_of` is invalid; the tranche or discount curve is missing; the OAS solve fails or produces a non-finite result; or the result cannot be converted to a JavaScript value.
   * @param config - Config used by this call.
   */
  structuredCreditTrancheOas(
    instrumentJson: JsonInput,
    trancheId: string,
    marketJson: JsonInput,
    asOf: string,
    marketPricePct: number,
    config?: string | null
  ): OasResult;
  /**
   * Scenario (CPR x CDR x severity) table for a tranche; returns a typed
   * `ScenarioTable` object. `grid` is a JSON `ScenarioGrid` (`cprs`, `cdrs`,
   * `severities`).
   *
   * The result is a plain object with snake_case fields — the same shape
   * Python exposes through its typed `ScenarioTable` wrapper. Pass it to
   * `JSON.stringify` if a wire string is needed.
   * @returns Typed `ScenarioTable` object over the CPR/CDR/severity grid.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Throws a JavaScript exception if the instrument, market, or scenario-grid JSON is malformed; the instrument fails pricing validation; `as_of` is invalid; the tranche or required market data is missing; a scenario fails or produces a non-finite result; or the table cannot be converted to a JavaScript value.
   * @param grid - Grid as a string.
   */
  structuredCreditTrancheScenarioTable(
    instrumentJson: JsonInput,
    trancheId: string,
    marketJson: JsonInput,
    asOf: string,
    grid: string
  ): ScenarioTable;
  /**
   * Per-tranche risk/spread metrics (PV, price, WAL, z-spread, CS01, spread/
   * modified duration, convexity) computed from one tranche's own cashflows.
   *
   * `marketPricePct`, when provided, is the quoted clean price (% of CURRENT balance)
   * the z-spread and CS01 are solved against; otherwise the tranche's own model
   * price is used (zero z-spread). Returns a typed `TrancheMetrics` object —
   * a plain object with the same snake_case fields Python exposes through its
   * typed `TrancheMetrics` wrapper. Pass it to `JSON.stringify` if a wire
   * string is needed.
   * @returns Typed `TrancheMetrics` object (PV, price, WAL, z-spread, CS01, duration, convexity).
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param marketPricePct - Optional clean settlement quote as a percentage of CURRENT balance; omit to use the deal quote, or its model clean price when no quote is supplied.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON is malformed; the instrument fails pricing validation; `as_of` is invalid; the tranche or discount curve is missing; a metric fails or is non-finite; or the result cannot be converted to a JavaScript value.
   */
  structuredCreditTrancheMetrics(
    instrumentJson: JsonInput,
    trancheId: string,
    marketJson: JsonInput,
    asOf: string,
    marketPricePct?: number | null
  ): TrancheMetrics;
}

/**
 * Bare FX instrument spec object for one exact FX instrument type.
 *
 * Tagged payloads, envelopes and JSON strings are rejected; use `fromJson` for
 * a canonical envelope string.
 */
export type FxInstrumentSpec = Record<string, unknown>;

/**
 * FX instrument handle priced against a market context.
 */
export interface FxInstrument extends WasmOwned {
  /**
   * Instrument identifier (mirrors the Python typed wrappers' `id` property).
   */
  readonly id: string;
  /**
   * Serialize to the canonical `finstack_quant.instrument/1` envelope.
   * @returns Compact canonical instrument envelope, byte-identical to the Python `to_json()` of the same instrument and accepted by `fromJson` and `priceInstrument`.
   * @throws Error - Throws a JavaScript exception if the instrument cannot be serialized.
   */
  toJson(): string;
  /**
   * Price this FX instrument against the supplied market.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @param metrics - Optional canonical metric IDs such as `"delta"`, `"vega"`, `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a valuation-only result.
   * @param metricPricingOverrides - Optional JSON metric-pricing overrides merged into the envelope before validation. Omit, `null`, or `undefined` to use the envelope as-is.
   * @param marketHistory - Optional serialized market-history JSON required by historical risk metrics such as historical VaR.
   * @returns Structured `ValuationResult` for the selected model.
   * @throws Error - Throws a JavaScript exception if the instrument, market, metric-pricing-override, or market-history JSON is invalid; `metrics` is not a string array; `asOf`, `model`, or a metric identifier is invalid; required market data is missing; pricing or a metric calculation fails; or the valuation cannot be converted to JavaScript.
   */
  price(
    marketJson: JsonInput,
    asOf: string,
    model?: string | null,
    metrics?: string[] | null,
    metricPricingOverrides?: JsonInput | null,
    marketHistory?: JsonInput | null
  ): ValuationResult;
  /**
   * Compute one metric of this FX instrument against the supplied market.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param metricId - Fully qualified metric identifier, e.g. `"delta"` or `"dv01"`.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns The metric value in the metric's documented unit.
   * @throws Error - Throws if an input is invalid, the metric is not defined for this instrument, required market data is missing (kind `not_found`), or the computation fails.
   */
  metric(marketJson: JsonInput, asOf: string, metricId: string, model?: string | null): number;
}

/**
 * FX forward handle with the Rust forward-rate helpers.
 */
export interface FxForwardInstrument extends FxInstrument {
  /**
   * Return a copy whose contract rate is `spotRate + forwardPoints` (mirrors Rust `FxForward::with_forward_points`).
   * @param spotRate - Spot rate, quote currency per unit of base currency; must be positive.
   * @param forwardPoints - Forward points in rate units, added to `spotRate`.
   * @returns A new forward with the contract rate set.
   * @throws Error - Throws with kind `validation` if `spotRate` is not positive and finite or the resulting rate is not positive.
   */
  withForwardPoints(spotRate: number, forwardPoints: number): FxForwardInstrument;
  /**
   * Return a copy whose contract rate is `spotRate + pips * pipSize` (mirrors Rust `FxForward::with_forward_pips`).
   * @param spotRate - Spot rate, quote currency per unit of base currency; must be positive.
   * @param pips - Forward points quoted in pips.
   * @returns A new forward with the contract rate set.
   * @throws Error - Throws with kind `validation` if `pips` or `spotRate` is not finite or the resulting rate is not positive.
   */
  withForwardPips(spotRate: number, pips: number): FxForwardInstrument;
  /**
   * Covered-interest-parity forward rate implied by the market (mirrors Rust `FxForward::market_forward_rate`).
   * @param marketJson - Serialized `MarketContext` carrying both discount curves and the FX spot.
   * @param asOf - Valuation date as an ISO-8601 string.
   * @returns Forward rate, quote currency per unit of base currency.
   * @throws Error - Throws with kind `not_found` if a discount curve or the FX spot is missing, and kind `validation` if an input is malformed.
   */
  marketForwardRate(marketJson: JsonInput, asOf: string): number;
}

/**
 * Optional arguments of `FxForward.fromTradeDate`; omitted fields take the Rust defaults.
 */
export interface FxForwardFromTradeDateOptions {
  /**
   * Base-currency holiday calendar; omitted uses weekends only.
   */
  base_calendar_id?: string | null;
  /**
   * Quote-currency holiday calendar; omitted uses weekends only.
   */
  quote_calendar_id?: string | null;
  /**
   * T+N spot lag in business days; omitted uses the pair's market standard.
   */
  settlement_days?: number | null;
  /**
   * Maturity roll rule (serde name, e.g. `"modified_following"`); omitted uses Modified Following.
   */
  business_day_convention?: string | null;
  /**
   * Apply the FX end-of-month rule when spot falls on month end (default `false`).
   */
  end_of_month?: boolean;
}

/**
 * Vanilla FX option handle: the option Greeks plus implied volatility.
 */
export interface FxVanillaOptionInstrument extends FxOptionInstrument {
  /**
   * Implied volatility that reproduces `targetPrice` (mirrors Rust `FxOption::implied_vol`, Garman–Kohlhagen inversion).
   * @param marketJson - Serialized `MarketContext` carrying both discount curves and the FX spot.
   * @param asOf - Valuation date as an ISO-8601 string.
   * @param targetPrice - Observed option PV in quote currency, on the same scale as `price`.
   * @returns Annualized lognormal volatility as a decimal (`0.10` = 10%).
   * @throws Error - Throws with kind `not_found` if a curve or the spot is missing, kind `validation` if an input is malformed, and kind `computation` if the root search does not converge.
   */
  impliedVol(marketJson: JsonInput, asOf: string, targetPrice: number): number;
}

/**
 * FX vanilla option handle with first-order greeks.
 */
export interface FxOptionInstrument extends FxInstrument {
  /**
   * Spot delta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot delta: change in value per unit spot.
   */
  delta(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Spot gamma of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot gamma: change in delta per unit spot.
   */
  gamma(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Vega of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vega: change in value per 1.0 absolute move in implied volatility.
   */
  vega(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Theta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Theta: change in value per year of calendar time.
   */
  theta(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Domestic-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Domestic rho: change in value per 1.0 absolute move in the domestic rate.
   */
  rho(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Foreign-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Foreign rho: change in value per 1.0 absolute move in the foreign rate.
   */
  foreignRho(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Vanna of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vanna: cross sensitivity of delta to implied volatility.
   */
  vanna(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Volga of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Volga: change in vega per 1.0 absolute move in implied volatility.
   */
  volga(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Named first-order greeks produced by the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Map of greek name to value, keyed in the Rust `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`, …); only the Greeks that apply to the instrument are present.
   */
  greeks(marketJson: JsonInput, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * FX digital option handle with first-order greeks.
 */
export interface FxDigitalOptionInstrument extends FxInstrument {
  /**
   * Spot delta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot delta: change in value per unit spot.
   */
  delta(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Spot gamma of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot gamma: change in delta per unit spot.
   */
  gamma(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Vega of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vega: change in value per 1.0 absolute move in implied volatility.
   */
  vega(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Theta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Theta: change in value per year of calendar time.
   */
  theta(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Domestic-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Domestic rho: change in value per 1.0 absolute move in the domestic rate.
   */
  rho(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Named first-order greeks produced by the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Map of greek name to value, keyed in the Rust `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`, …); only the Greeks that apply to the instrument are present.
   */
  greeks(marketJson: JsonInput, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * FX touch option handle with first-order greeks.
 */
export interface FxTouchOptionInstrument extends FxInstrument {
  /**
   * Spot delta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot delta: change in value per unit spot.
   */
  delta(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Spot gamma of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot gamma: change in delta per unit spot.
   */
  gamma(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Vega of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vega: change in value per 1.0 absolute move in implied volatility.
   */
  vega(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Theta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Theta: P&L over the theta horizon (default one day, capped at expiry) with spot held at its `asOf` level, so a roll into the monitoring window observes that spot; not annualized.
   */
  theta(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Domestic-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Domestic rho: change in value per 1.0 absolute move in the domestic rate.
   */
  rho(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Named first-order greeks produced by the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Map of greek name to value, keyed in the Rust `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`, …); only the Greeks that apply to the instrument are present.
   */
  greeks(marketJson: JsonInput, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * FX barrier option handle with vanna and volga in addition to touch greeks.
 */
export interface FxBarrierOptionInstrument extends FxTouchOptionInstrument {
  /**
   * Vanna of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vanna: cross sensitivity of delta to implied volatility.
   */
  vanna(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Volga of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Volga: change in vega per 1.0 absolute move in implied volatility.
   */
  volga(marketJson: JsonInput, asOf: string, model?: string | null): number;
}

/**
 * Constructors and factories for typed FX instrument handles.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const spot = new valuations.fx.FxSpot({
 *   id: "EURUSD-SPOT",
 *   base_currency: "EUR",
 *   quote_currency: "USD",
 *   settlement: "2025-01-17",
 *   quoted_spot: 1.1,
 *   notional: { amount: "1000000", currency: "EUR" },
 *   attributes: {},
 * });
 * console.log(JSON.parse(spot.toJson()).instrument.type);
 * spot.free();
 * ```
 */
export interface FxInstrumentConstructor<T extends FxInstrument> {
  /**
   * Construct an FX instrument from a bare spec object.
   * @param spec - Bare FX instrument spec object for this exact instrument type (not a JSON string, tagged payload, or envelope; use `fromJson` for canonical envelope JSON).
   * @returns A typed FX instrument handle.
   * @throws Error - Throws a `FinstackError` with kind `validation` when `spec` is not a bare spec object or does not describe this instrument type.
   */
  new (spec: FxInstrumentSpec): T;
  /**
   * Parse a `FxInstrument` value from canonical JSON.
   * @param json - Canonical FX instrument JSON accepted by `fromJson`.
   * @returns A typed FX instrument handle.
   */
  fromJson(json: JsonInput): T;
}

/**
 * Constructor surface of `FxForward`: the FX constructors plus the Rust presets.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const forward = valuations.fx.FxForward.example().withForwardPoints(1.1, 0.002);
 * console.log(valuations.fx.FxForward.standardSettlementDays("EUR", "USD"), forward.id);
 * forward.free();
 * ```
 */
export interface FxForwardConstructor extends FxInstrumentConstructor<FxForwardInstrument> {
  /**
   * Canonical example forward (mirrors Rust `FxForward::example`): EUR/USD, EUR 1,000,000 at 1.12.
   * @returns The example forward.
   * @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
   */
  example(): FxForwardInstrument;
  /**
   * Market-standard spot lag in business days for a currency pair (mirrors Rust `FxForward::standard_settlement_days`).
   * @param base - Base (foreign) currency ISO code.
   * @param quote - Quote (domestic) currency ISO code.
   * @returns Spot lag in business days (1 or 2).
   * @throws Error - Throws with kind `validation` if a currency code is not ISO-4217.
   */
  standardSettlementDays(base: string, quote: string): number;
  /**
   * Build an at-market forward from a trade date and a standard FX tenor (mirrors Rust `FxForward::from_trade_date`).
   * @param id - Unique instrument identifier.
   * @param baseCurrency - Base (foreign) currency ISO code; the notional currency.
   * @param quoteCurrency - Quote (domestic) currency ISO code.
   * @param tradeDate - Trade date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param tenor - Standard FX tenor from spot (e.g. `Tenor.parse("3M")`).
   * @param notional - Notional in `baseCurrency`.
   * @param domesticDiscountCurveId - Quote-currency discount curve identifier.
   * @param foreignDiscountCurveId - Base-currency discount curve identifier.
   * @param options - Optional `{ base_calendar_id?, quote_calendar_id?, settlement_days?, business_day_convention?, end_of_month? }`; `business_day_convention` uses the serde names (e.g. `"modified_following"`).
   * @returns The validated forward without a contract rate; chain `withForwardPoints` / `withForwardPips` to fix it.
   * @throws Error - Throws with kind `validation` if a currency, date or option is malformed, the currencies coincide, or the notional currency differs from `baseCurrency`; kind `not_found` if a calendar identifier is unknown.
   */
  fromTradeDate(
    id: string,
    baseCurrency: string,
    quoteCurrency: string,
    tradeDate: string,
    tenor: Tenor,
    notional: Money,
    domesticDiscountCurveId: string,
    foreignDiscountCurveId: string,
    options?: FxForwardFromTradeDateOptions | null
  ): FxForwardInstrument;
}

/**
 * Constructor surface of `FxOption`: the FX constructors plus the Rust presets.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const option = valuations.fx.FxOption.example();
 * console.log(option.id, JSON.parse(option.toJson()).instrument.type);
 * option.free();
 * ```
 */
export interface FxOptionConstructor extends FxInstrumentConstructor<FxVanillaOptionInstrument> {
  /**
   * Canonical example option (mirrors Rust `FxOption::example`): EUR/USD call, strike 1.12, EUR 1,000,000.
   * @returns The example option.
   * @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
   */
  example(): FxVanillaOptionInstrument;
  /**
   * Build a European FX option with currency-derived OIS curves (mirrors Rust `FxOption::european`).
   * @param id - Unique instrument identifier.
   * @param baseCurrency - Base (foreign) currency ISO code; the notional currency.
   * @param quoteCurrency - Quote (domestic) currency ISO code.
   * @param strike - Strike, quote currency per unit of base currency.
   * @param expiry - Expiry date as an ISO-8601 string.
   * @param notional - Notional in `baseCurrency`.
   * @param volSurfaceId - FX volatility surface identifier.
   * @param optionType - `"call"` or `"put"` on the base currency.
   * @param deltaConventionKind - `"spot"`, `"forward"`, `"premium_adjusted_spot"` or `"premium_adjusted_forward"`.
   * @param premiumCurrency - ISO code of the premium currency (base or quote).
   * @param venue - Non-empty market venue or quoting-source identifier.
   * @returns The validated option.
   * @throws Error - Throws with kind `validation` if a code, date or enum is malformed, the currencies coincide, the premium currency is neither leg, the venue is blank, or the notional is not positive.
   */
  european(
    id: string,
    baseCurrency: string,
    quoteCurrency: string,
    strike: number,
    expiry: string,
    notional: Money,
    volSurfaceId: string,
    optionType: 'call' | 'put',
    deltaConventionKind: 'spot' | 'forward' | 'premium_adjusted_spot' | 'premium_adjusted_forward',
    premiumCurrency: string,
    venue: string
  ): FxVanillaOptionInstrument;
}

/**
 * Namespaced TypeScript entry points for fx calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const Spot = valuations.fx.FxSpot;
 * const spot = new Spot({
 *   id: "EURUSD-SPOT",
 *   base_currency: "EUR",
 *   quote_currency: "USD",
 *   settlement: "2025-01-17",
 *   quoted_spot: 1.1,
 *   notional: { amount: "1000000", currency: "EUR" },
 *   attributes: {},
 * });
 * console.log(spot.toJson());
 * spot.free();
 * ```
 */
export interface FxNamespace {
  /**
   * Spot FX instrument constructor.
   */
  FxSpot: FxInstrumentConstructor<FxInstrument>;
  /**
   * FX forward instrument constructor.
   */
  FxForward: FxForwardConstructor;
  /**
   * FX swap instrument constructor.
   */
  FxSwap: FxInstrumentConstructor<FxInstrument>;
  /**
   * Non-deliverable forward constructor.
   */
  Ndf: FxInstrumentConstructor<FxInstrument>;
  /**
   * Vanilla FX option constructor.
   */
  FxOption: FxOptionConstructor;
  /**
   * Digital FX option constructor.
   */
  FxDigitalOption: FxInstrumentConstructor<FxDigitalOptionInstrument>;
  /**
   * One-touch / no-touch FX option constructor.
   */
  FxTouchOption: FxInstrumentConstructor<FxTouchOptionInstrument>;
  /**
   * Barrier FX option constructor.
   */
  FxBarrierOption: FxInstrumentConstructor<FxBarrierOptionInstrument>;
  /**
   * FX variance-swap constructor.
   */
  FxVarianceSwap: FxInstrumentConstructor<FxInstrument>;
  /**
   * Quanto option constructor.
   */
  QuantoOption: FxInstrumentConstructor<FxOptionInstrument>;
}

// --- SABR (Stochastic Alpha Beta Rho) volatility -------------------------

/**
 * SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 */
export interface SabrParameters extends WasmOwned {
  /**
   * SABR `alpha` (ATM volatility level).
   */
  readonly alpha: number;
  /**
   * SABR `beta` (backbone exponent).
   */
  readonly beta: number;
  /**
   * SABR `nu` (vol-of-vol).
   */
  readonly nu: number;
  /**
   * SABR `rho` (spot/vol correlation).
   */
  readonly rho: number;
  /**
   * Displacement applied for shifted SABR, if any.
   */
  readonly shift: number | undefined;
  /**
   * Whether a displacement (shift) is configured.
   * @returns `true` when a SABR displacement shift is configured.
   */
  isShifted(): boolean;
}

/**
 * SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const params = new models.volatility.SabrParameters(0.2, 1.0, 0.3, -0.2);
 * console.log(params.alpha, params.rho);
 * params.free();
 * ```
 */
export interface SabrParametersConstructor {
  /**
   * Create SABR parameters from alpha, beta, nu, rho, and optional shift.
   * @returns A `SabrParameters` handle.
   * @param alpha - Positive SABR initial volatility scale parameter.
   * @param beta - SABR CEV elasticity parameter from 0 through 1.
   * @param nu - Positive SABR volatility-of-volatility parameter.
   * @param rho - Instantaneous correlation between the asset and variance shocks.
   * @param shift - Additive SABR rate shift applied to forward and strike before modelling.
   * @throws Error - Throws a JavaScript exception if `alpha` is not finite and positive, `beta` is outside `[0, 1]`, `nu` is negative or non-finite, `rho` is outside `[-1, 1]`, or a supplied `shift` is not finite and positive.
   */
  new (alpha: number, beta: number, nu: number, rho: number, shift?: number): SabrParameters;
  /**
   * Equity-standard defaults `(alpha=0.20, beta=1.0, nu=0.30, rho=-0.20)`.
   * @returns A `SabrParameters` handle.
   */
  equityDefault(): SabrParameters;
  /**
   * Rates-standard defaults `(alpha=0.02, beta=0.5, nu=0.30, rho=0.0)`.
   * @returns A `SabrParameters` handle.
   */
  ratesDefault(): SabrParameters;
}

/**
 * Hagan-2002 SABR volatility model.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 */
export interface SabrModel extends WasmOwned {
  /**
   * Implied volatility for the given strike: normal (Bachelier) vol in absolute rate units when beta < 1e-4, Black decimal vol otherwise.
   * @returns Hagan-2002 implied volatility: normal vol in absolute rate units when beta < 1e-4, Black decimal volatility otherwise.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param t - Time from the curve base date in years.
   * @throws Error - Throws a JavaScript exception if `t` is not positive, the forward or strike lies outside the selected shifted or unshifted SABR domain, or the Hagan expansion produces an undefined or non-finite volatility.
   */
  impliedVol(forward: number, strike: number, t: number): number;
  /**
   * Parameters used by this model.
   */
  readonly params: SabrParameters;
  /**
   * Whether the parameterization admits negative forwards.
   * @returns `true` when the SABR parameterization admits negative forwards.
   */
  supportsNegativeRates(): boolean;
}

/**
 * Hagan-2002 SABR volatility model.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const params = models.volatility.SabrParameters.equityDefault();
 * const model = new models.volatility.SabrModel(params);
 * console.log(model.impliedVol(100, 105, 1));
 * model.free();
 * params.free();
 * ```
 */
export interface SabrModelConstructor {
  /**
   * Create a Hagan-2002 SABR model from the supplied parameters.
   * @returns A `SabrModel` handle.
   * @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
   */
  new (params: SabrParameters): SabrModel;
}

/**
 * Volatility smile generator for a fixed `(forward, t)` pair.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 */
export interface SabrSmile extends WasmOwned {
  /**
   * At-the-money implied volatility.
   * @returns ATM implied volatility for this smile's `(forward, t)`: normal vol in absolute rate units when beta < 1e-4, Black decimal vol otherwise.
   * @throws Error - Throws a JavaScript exception if the smile's expiry or effective forward is outside the model domain, or the ATM calculation produces an invalid volatility.
   */
  atmVol(): number;
  /**
   * Implied volatility at `strike`: normal (Bachelier) vol in absolute rate units when beta < 1e-4, Black decimal vol otherwise.
   * @param strike - Option strike price in the same price units as the underlying.
   * @returns Normal (Bachelier) vol in absolute rate units when beta < 1e-4, Black decimal vol otherwise.
   * @throws Error - Throws a JavaScript exception if the smile's expiry, forward, or requested `strike` is outside the model domain or the Hagan expansion fails.
   */
  impliedVol(strike: number): number;
  /**
   * Implied volatilities for a strike grid.
   * @returns One implied vol per strike, in the same order as `strikes` (normal vol when beta < 1e-4, Black decimal vol otherwise).
   * @param strikes - Option strikes at which to evaluate the SABR volatility smile.
   * @throws Error - Throws a JavaScript exception if the smile's expiry or forward, or any supplied strike, is outside the model domain, or the Hagan expansion produces an invalid volatility.
   */
  generateSmile(strikes: number[]): Float64Array;
  /**
   * Butterfly + strike-monotonicity static-arbitrage check of the smile.
   *
   * Returns the Rust `ArbitrageValidationResult` serde object (`arbitrage_free`,
   * `butterfly_violations`, `monotonicity_violations`), the same shape Python
   * `SabrSmile.validate_no_arbitrage` returns.
   * @returns The Rust `ArbitrageValidationResult` for the supplied strike grid.
   * @param strikes - Ascending option strikes used to test the smile for static arbitrage.
   * @param r - Continuously compounded risk-free rate (decimal) that discounts the forward-based Black call prices compared against the 1e-6 tolerance.
   * @throws Error - Throws a JavaScript exception if volatility generation fails for the stored smile and supplied strikes, or the result cannot be converted to a JavaScript value.
   */
  validateNoArbitrage(strikes: NumericArray, r: number): ArbitrageValidationResult;
}

/**
 * Volatility smile generator for a fixed `(forward, t)` pair.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const params = models.volatility.SabrParameters.ratesDefault();
 * const smile = new models.volatility.SabrSmile(params, 0.03, 2);
 * console.log(smile.generateSmile([0.02, 0.03, 0.04]));
 * smile.free();
 * params.free();
 * ```
 */
export interface SabrSmileConstructor {
  /**
   * Create a SABR smile for a fixed forward and expiry.
   * @returns A `SabrSmile` handle for the supplied forward and expiry.
   * @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param t - Time from the curve base date in years.
   */
  new (params: SabrParameters, forward: number, t: number): SabrSmile;
}

/**
 * SABR calibrator (Levenberg-Marquardt with beta fixed).
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 */
export interface SabrCalibrator extends WasmOwned {
  /**
   * Return a copy of this calibrator with an overridden maximum relative
   * quote error (also used as the solver residual tolerance), preserving all
   * other settings (e.g. the iteration cap from `highPrecision`).
   * @returns A `SabrCalibrator` handle.
   * @param tolerance - Positive finite maximum relative error of any final volatility quote; 1e-4 permits 0.01% of each quote. Invalid settings or a fit outside this budget throw during calibration.
   */
  withTolerance(tolerance: number): SabrCalibrator;
  /**
   * Return a copy of this calibrator with an overridden iteration cap,
   * preserving all other settings.
   * @returns A `SabrCalibrator` handle.
   * @param maxIterations - Positive cap on solver iterations before a non-convergence error; pair a tight tolerance with a larger budget.
   */
  withMaxIterations(maxIterations: number): SabrCalibrator;
  /**
   * Calibrate `(alpha, nu, rho)` to market vols with `beta` fixed.
   * @returns A `SabrParameters` handle.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strikes - Option strikes aligned one-for-one with market_vols.
   * @param marketVols - Market-implied annualized volatilities aligned one-for-one with strikes.
   * @param t - Time from the curve base date in years.
   * @param beta - SABR CEV elasticity parameter held fixed during calibration.
   * @throws Error - Throws a JavaScript exception if the strike and volatility lengths differ, the quote arrays are empty, the SABR inputs or fitted parameters are invalid, or the calibration solver does not converge.
   */
  calibrate(
    forward: number,
    strikes: number[],
    marketVols: number[],
    t: number,
    beta: number
  ): SabrParameters;
  /**
   * Return a copy of this calibrator with an overridden displacement policy,
   * preserving all other settings.
   * @returns A `SabrCalibrator` handle.
   * @param shift - `null`/`undefined` fits the quotes as-is; a number is a fixed additive shift in the forward's units (decimal rate or price); `"auto"` picks the smallest standardized shift (1-4%) that leaves 10bp of headroom above the most negative forward or strike, or none when every input is non-negative. The shift used is stored on the fitted `SabrParameters`.
   * @throws Error - Throws a `FinstackError` (`kind: "validation"`) for a string other than `"auto"` (parsed by the Rust `SabrShift`), and a `TypeError` (`kind: "invalid_type"`) for any other non-null, non-number value such as a boolean.
   */
  withShift(shift: number | 'auto' | null | undefined): SabrCalibrator;
  /**
   * Return a copy of this calibrator with exact ATM pinning enabled or
   * disabled, preserving all other settings.
   * @returns A `SabrCalibrator` handle.
   * @param atmPinning - When `true`, alpha is solved analytically so the model reproduces the ATM volatility interpolated from the quotes exactly, and only nu and rho are fitted to the smile.
   */
  withAtmPinning(atmPinning: boolean): SabrCalibrator;
}

/**
 * SABR calibrator (Levenberg-Marquardt with beta fixed).
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const calibrator = models.volatility.SabrCalibrator.highPrecision();
 * const tighter = calibrator.withTolerance(1e-10);
 * tighter.free();
 * calibrator.free();
 * ```
 */
export interface SabrCalibratorConstructor {
  /**
   * Create a Levenberg-Marquardt SABR calibrator with default tolerances.
   * @returns A `SabrCalibrator` handle.
   */
  new (): SabrCalibrator;
  /**
   * Calibrator preset with tighter convergence tolerances.
   * @returns A `SabrCalibrator` handle.
   */
  highPrecision(): SabrCalibrator;
}

/**
 * Namespaced TypeScript entry points for structural-credit model calculations.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const model = models.credit.mertonModelJson(100, 0.25, 60, 0.03);
 * console.log(models.credit.mertonDefaultProbability(model, 1));
 * ```
 */
export interface ModelCreditNamespace {
  /**
   * Compare hold-versus-tender economics for a distressed exchange offer.
   * Tendering is recommended only when total consideration exceeds the
   * hold-out present value by more than 2%.
   * @returns Tender total, NPV pickup, breakeven recovery, and recommendation.
   * @param oldPv - Present value of the existing claim if it is not tendered, in the caller's monetary unit.
   * @param newPv - Present value of the new instrument received on tendering, in the same unit as oldPv.
   * @param consentFee - Cash consent or early-tender fee paid to participating holders, in the same unit as oldPv.
   * @param equitySweetenerValue - Estimated value of equity or warrants attached to the new instrument, in the same unit as oldPv.
   * @param exchangeType - Canonical offer structure: par_for_par, discount, uptier, or downtier.
   * @throws Error - Throws a JavaScript exception if `exchangeType` is unrecognized, any monetary input is negative or non-finite, or the result cannot be converted to a JavaScript object.
   */
  analyzeExchangeOffer(
    oldPv: number,
    newPv: number,
    consentFee: number,
    equitySweetenerValue: number,
    exchangeType: string
  ): ExchangeOfferAnalysis;
  /**
   * Compute discount capture and leverage impact for an LME transaction.
   * @returns Cash cost, par retired, discount captured, holder impact, and optional leverage analysis.
   * @param lmeType - Canonical structure: open_market_repurchase, tender_offer, amend_and_extend, or dropdown.
   * @param notional - Outstanding face amount of the target instrument, in the caller's monetary unit; must be positive.
   * @param repurchasePricePct - Price as a fraction of par for repurchases and tenders, the extension fee for amend-and-extend, or the transferred-asset fraction for a dropdown.
   * @param optAcceptancePct - Fraction of holders participating, in [0, 1].
   * @param ebitda - EBITDA in the same unit as notional; a positive value adds the leverage_impact block, null or non-positive omits it.
   * @throws Error - Throws a JavaScript exception if `lmeType` is unrecognized, `notional` is non-positive or non-finite, `optAcceptancePct` is outside `[0, 1]`, or `repurchasePricePct` is outside the range accepted for the selected LME type: `(0, 1.5]` for repurchases and tenders, `[0, 0.1]` for amend-and-extend, and `[0, 1]` for dropdowns. It also throws if the result cannot be converted to a JavaScript object.
   */
  analyzeLme(
    lmeType: string,
    notional: number,
    repurchasePricePct: number,
    optAcceptancePct: number,
    ebitda?: number | null
  ): LmeAnalysis;
  /**
   * Build a structural Merton model JSON payload.
   * @returns Canonical Merton structural-model JSON.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal.
   * @param debtBarrier - Positive debt face value defining the structural-model default barrier.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @throws Error - Throws a JavaScript exception if `asset_value`, `asset_vol`, or `debt_barrier` is non-positive, or if the model cannot be serialized to JSON.
   */
  mertonModelJson(
    assetValue: number,
    assetVol: number,
    debtBarrier: number,
    riskFreeRate: number
  ): string;
  /**
   * Build a CreditGrades structural model JSON payload.
   * @returns Canonical CreditGrades structural-model JSON.
   * @param equityValue - Current market value of equity in the firm's monetary units.
   * @param equityVol - Annualized equity-return volatility expressed as a decimal.
   * @param totalDebt - Total debt face value in the firm's monetary units.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param barrierUncertainty - Lognormal dispersion of the CreditGrades default barrier, not a generic uncertainty score.
   * @param meanRecovery - Mean recovery rate at default expressed as a fraction from 0 through 1.
   * @throws Error - Throws a JavaScript exception if CreditGrades or Merton model validation rejects the supplied equity, volatility, debt, barrier-uncertainty, or recovery inputs, or if the model cannot be serialized to JSON.
   */
  creditGradesModelJson(
    equityValue: number,
    equityVol: number,
    totalDebt: number,
    riskFreeRate: number,
    barrierUncertainty: number,
    meanRecovery: number
  ): string;
  /**
   * Compute structural default probability from model JSON.
   * @returns Risk-neutral default probability in `[0, 1]` over `horizon` years.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed or does not deserialize as a Merton model.
   */
  mertonDefaultProbability(modelJson: JsonInput, horizon: number): number;
  /**
   * Compute the physical-measure (Moody's KMV) default probability, the theoretical EDF, from a Merton model JSON payload.
   * @returns Physical-measure default probability in `[0, 1]` over `horizon` years.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param assetDrift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `asset_drift` is not finite, or if the model uses driftless CreditGrades dynamics.
   */
  mertonDefaultProbabilityWithDrift(
    modelJson: JsonInput,
    assetDrift: number,
    horizon: number
  ): number;
  /**
   * Compute distance-to-default from a Merton model JSON payload.
   *
   * Distance-to-default is `ln(V/B)/(sigma*sqrt(T))` plus drift adjustments.
   * Lower values indicate higher default risk. This is the risk-neutral `d2`,
   * not the Moody's KMV distance-to-default.
   * @returns Distance-to-default in standard-deviation units over `horizon` years.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed or does not deserialize as a Merton model.
   */
  mertonDistanceToDefault(modelJson: JsonInput, horizon: number): number;
  /**
   * Compute the physical-measure (Moody's KMV) distance-to-default from a Merton model JSON payload.
   * @returns Physical-measure distance-to-default in standard-deviation units over `horizon` years.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param assetDrift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `asset_drift` is not finite, or if the model uses driftless CreditGrades dynamics.
   */
  mertonDistanceToDefaultWithDrift(
    modelJson: JsonInput,
    assetDrift: number,
    horizon: number
  ): number;
  /**
   * Compute the Moody's KMV default point, short-term debt plus half of long-term debt, for use as a structural default barrier.
   * @returns Default point in the same monetary units as the debt inputs.
   * @param shortTermDebt - Liabilities due within one year, in the firm's monetary units.
   * @param longTermDebt - Liabilities maturing beyond one year, in the same units; half of it enters the default point.
   * @throws Error - Throws a JavaScript exception if either input is negative or non-finite, or if the resulting default point is zero.
   */
  mertonKmvDefaultPoint(shortTermDebt: number, longTermDebt: number): number;
  /**
   * Compute the zero-coupon bond credit spread (per year) from a Merton model
   * JSON payload, given an exogenous recovery rate paid at maturity.
   * @returns Zero-coupon credit spread per year as a decimal, such as `0.015` for 150 bp.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed or does not deserialize as a Merton model, `horizon` is non-finite or non-positive, or `recovery` is outside `[0, 1]`.
   */
  mertonImpliedSpread(modelJson: JsonInput, horizon: number, recovery: number): number;
  /**
   * Compute the Merton (1974) endogenous debt spread (per year) from a Merton
   * model JSON payload, where recovery is the firm's own terminal asset value.
   * @returns Endogenous debt spread per year as a decimal, such as `0.004` for 40 bp.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Maturity of the firm's debt measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `horizon` is non-positive, if the barrier type is not terminal, or if the implied debt value is non-positive.
   */
  mertonDebtSpread(modelJson: JsonInput, horizon: number): number;
  /**
   * Compute the ISDA-style CDS par spread (per year, as a decimal) implied by a Merton model's survival curve.
   * @returns CDS par spread per year as a decimal, such as `0.015` for 150 bp.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param maturity - CDS maturity in years; must be positive and finite.
   * @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `maturity` is non-positive, if `recovery` is outside `[0, 1]` or contradicts the model's CreditGrades `mean_recovery`, or if the implied survival curve cannot be bootstrapped.
   */
  mertonCdsParSpread(modelJson: JsonInput, maturity: number, recovery: number): number;
  /**
   * Build a Merton model JSON payload from observable equity inputs (KMV calibration).
   * @returns Canonical Merton model JSON calibrated from equity observables.
   * @param equityValue - Current market value of equity in the firm's monetary units.
   * @param equityVol - Annualized equity-return volatility expressed as a decimal.
   * @param totalDebt - Total debt face value used as the structural default barrier.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param payoutRate - Continuous dividend or payout yield on assets, expressed as a decimal.
   * @param maturity - Calibration horizon in years; must be positive and finite.
   * @throws Error - Throws a JavaScript exception if equity, volatility, debt, rate, or maturity inputs are invalid, or if the model cannot be serialized to JSON.
   */
  mertonFromEquityJson(
    equityValue: number,
    equityVol: number,
    totalDebt: number,
    riskFreeRate: number,
    payoutRate: number,
    maturity: number
  ): string;
  /**
   * Build a Merton model JSON payload from a target CDS par spread.
   *
   * The objective is a full ISDA-style par spread built from the model's
   * survival curve. A quote that no volatility in `[0.01, 2.0]` reproduces, or
   * one consistent with several volatilities, is rejected rather than resolved
   * arbitrarily.
   * @returns Canonical Merton model JSON calibrated to a CDS par spread.
   * @param cdsSpreadBp - Target CDS par spread in basis points.
   * @param recovery - Recovery rate at default expressed as a fraction from 0 through 1.
   * @param totalDebt - Total debt face value in the firm's monetary units.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param maturity - Calibration horizon in years; must be positive and finite.
   * @param assetValue - Assumed initial firm asset value in monetary units.
   * @param payoutRate - Continuous payout rate on assets, expressed as a decimal.
   * @throws Error - Throws a JavaScript exception if spread, recovery, debt, rate, maturity, asset value, or payout inputs are invalid, if the quote is unattainable or ambiguous, or if the model cannot be serialized to JSON.
   */
  mertonFromCdsSpreadJson(
    cdsSpreadBp: number,
    recovery: number,
    totalDebt: number,
    riskFreeRate: number,
    maturity: number,
    assetValue: number,
    payoutRate: number
  ): string;
  /**
   * Build a Merton model JSON payload calibrated to a target cumulative default probability.
   * @returns Canonical Merton model JSON calibrated to a target cumulative PD.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal; must be positive.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%. Pass the expected physical asset return to calibrate against a real-world default rate.
   * @param payoutRate - Continuous payout rate on assets, expressed as a decimal; it enters the calibration drift and is carried on the returned model.
   * @param targetPd - Target cumulative default probability in `(0, 1)`.
   * @param maturity - Calibration horizon in years; must be positive and finite.
   * @throws Error - Throws a JavaScript exception if asset value, volatility, rate, target PD, or maturity inputs are invalid, or if the model cannot be serialized to JSON.
   */
  mertonFromTargetPdJson(
    assetValue: number,
    assetVol: number,
    riskFreeRate: number,
    payoutRate: number,
    targetPd: number,
    maturity: number
  ): string;
  /**
   * Build a Merton model JSON payload with explicit barrier and asset-dynamics specifications.
   * @returns Canonical Merton model JSON with explicit barrier and asset dynamics.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal.
   * @param debtBarrier - Positive debt face value defining the structural-model default barrier.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param payoutRate - Continuous payout rate on assets, expressed as a decimal.
   * @param barrierTypeJson - Serialized `MertonBarrierType` JSON (terminal or first-passage).
   * @param dynamicsJson - Serialized `AssetDynamics` JSON (GBM, jump-diffusion, or CreditGrades).
   * @throws Error - Throws a JavaScript exception if model inputs are invalid, if `barrier_type_json` or `dynamics_json` does not deserialize, or if the model cannot be serialized to JSON.
   */
  mertonModelWithDynamicsJson(
    assetValue: number,
    assetVol: number,
    debtBarrier: number,
    riskFreeRate: number,
    payoutRate: number,
    barrierTypeJson: JsonInput,
    dynamicsJson: JsonInput
  ): string;
  /**
   * Compute implied equity value and equity volatility from a Merton model JSON payload.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @returns A `Float64Array` of length 2: `[equityValue, equityVolatility]`.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `horizon` is non-positive or non-finite, or if the inversion is numerically ill-conditioned.
   */
  mertonTryImpliedEquity(modelJson: JsonInput, horizon: number): Float64Array;
  /**
   * Bootstrap a hazard-curve JSON payload from structural default probabilities.
   * @returns Hazard-curve JSON bootstrapped from structural default probabilities.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param id - Hazard-curve identifier string.
   * @param baseDate - Valuation date in ISO-8601 form, such as `"2025-01-15"`.
   * @param tenors - Tenor grid in years as a `number[]` or `Float64Array`; entries must be positive and distinct.
   * @param recovery - Recovery rate at default expressed as a fraction from 0 through 1.
   * @param dayCount - Day-count convention the curve uses to turn dates into year fractions, such as `"act_365f"` or `"act_360"`.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `base_date` is not a valid ISO-8601 calendar date (`YYYY-MM-DD`), if `tenors` is empty or contains non-positive values, if `recovery` is out of range or contradicts the model's CreditGrades `mean_recovery`, if `day_count` is not a recognized convention, if the implied survival curve is non-monotonic, or if the hazard curve cannot be serialized to JSON.
   */
  mertonToHazardCurveJson(
    modelJson: JsonInput,
    id: string,
    baseDate: string,
    tenors: NumericArray,
    recovery: number,
    dayCount: string
  ): string;
  /**
   * Simulate firm-asset paths and return a JSON payload with the time grid and row-major asset values.
   * @returns JSON payload with the time grid and row-major simulated asset values.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param numPaths - Number of Monte Carlo paths to simulate.
   * @param numSteps - Number of time steps per path; must be at least 1.
   * @param horizon - Simulation horizon in years; must be positive and finite.
   * @param seed - Seed for reproducible draws; the Rust `MertonModel::simulate_paths_seeded` owns the generator (PCG64), so equal seeds give equal paths in every host.
   * @param antithetic - When `true`, use antithetic variates for variance reduction.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if path or step counts exceed the safe-integer range, if `num_steps` is zero, if `horizon` is non-positive or non-finite, or if the result cannot be serialized to JSON.
   */
  mertonSimulatePathsJson(
    modelJson: JsonInput,
    numPaths: number,
    numSteps: number,
    horizon: number,
    seed: bigint,
    antithetic: boolean
  ): string;
  /**
   * Evaluate a `DynamicRecoverySpec` JSON payload at a given accreted
   * notional, returning the implied recovery rate. Result is clamped to
   * `[0, base_recovery]`.
   * @returns Implied recovery rate as a fraction of par, clamped to `[0, base_recovery]`.
   * @param specJson - Serialized DynamicRecoverySpec JSON defining the notional-to-recovery mapping.
   * @param notional - Signed trade notional in the instrument's native currency units.
   * @throws Error - Throws a JavaScript exception if `spec_json` is malformed or does not deserialize as a dynamic-recovery specification.
   */
  dynamicRecoveryAtNotional(specJson: JsonInput, notional: number): number;
  /**
   * Evaluate an `EndogenousHazardSpec` JSON payload at a given leverage
   * level, returning the implied hazard rate. Floored at 0.
   * @returns Annualized hazard rate as a decimal, floored at 0.
   * @param specJson - Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping.
   * @param leverage - Debt-to-assets leverage ratio used by the structural credit model.
   * @throws Error - Throws a JavaScript exception if `spec_json` is malformed or does not deserialize as an endogenous-hazard specification.
   */
  endogenousHazardAtLeverage(specJson: JsonInput, leverage: number): number;
  /**
   * Convenience evaluator: hazard rate after a PIK accrual updates the
   * outstanding notional. Computes leverage = `accreted_notional / asset_value`
   * then evaluates the hazard mapping.
   * @returns Annualized hazard rate as a decimal after the PIK leverage update.
   * @param specJson - Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping.
   * @param accretedNotional - Outstanding notional after PIK accrual, in the debt's monetary units.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @throws Error - Throws a JavaScript exception if `spec_json` is malformed or does not deserialize as an endogenous-hazard specification.
   */
  endogenousHazardAfterPikAccrual(
    specJson: JsonInput,
    accretedNotional: number,
    assetValue: number
  ): number;
  /**
   * Build a constant dynamic-recovery spec JSON payload.
   * @returns Canonical constant dynamic-recovery specification JSON.
   * @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
   * @throws Error - Throws a JavaScript exception if `recovery` is outside `[0, 1]` or the specification cannot be serialized to JSON.
   */
  dynamicRecoveryConstantJson(recovery: number): string;
  /**
   * Build an endogenous hazard power-law spec JSON payload.
   * @returns Canonical endogenous-hazard power-law specification JSON.
   * @param baseHazard - Reference annual default intensity used by the leverage-to-hazard mapping.
   * @param baseLeverage - Positive reference debt-to-assets leverage ratio for the hazard mapping.
   * @param exponent - Power-law exponent in `lambda(L) = baseHazard * (L / baseLeverage)^exponent`.
   * @throws Error - Throws a JavaScript exception if `base_hazard` is negative, `base_leverage` is non-positive, or the specification cannot be serialized to JSON.
   */
  endogenousHazardPowerLawJson(baseHazard: number, baseLeverage: number, exponent: number): string;
  /**
   * Build a credit-state JSON payload for toggle-exercise decisions.
   *
   * Parameter order follows the canonical Rust `CreditState` field order
   * (and the Python binding): `hazardRate`, `distanceToDefault`, `leverage`,
   * `accretedNotional`, `couponDue`, `assetValue`.
   * @returns Canonical credit-state JSON for toggle-exercise decisions.
   * @param hazardRate - Annualized instantaneous default intensity, expressed as a decimal.
   * @param distanceToDefault - Optional distance to default, measured as standard deviations from the default point.
   * @param leverage - Debt-to-assets leverage ratio used by the structural credit model.
   * @param accretedNotional - Outstanding notional after PIK accrual, in the debt's monetary units.
   * @param couponDue - Cash coupon amount due at the toggle decision date, in debt monetary units.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @throws Error - Throws a `validation` error if any supplied value is non-finite (JSON cannot carry `NaN` or infinities).
   */
  creditStateJson(
    hazardRate: number,
    distanceToDefault: number | null | undefined,
    leverage: number,
    accretedNotional: number,
    couponDue: number,
    assetValue?: number | null
  ): string;
  /**
   * Build a threshold toggle-exercise model JSON payload.
   * @returns Canonical threshold toggle-exercise model JSON.
   * @param variable - Credit-state variable: `"hazard_rate"`, `"distance_to_default"`, or `"leverage"`.
   * @param threshold - Threshold value in the units of the selected credit-state variable.
   * @param direction - Threshold comparison: `"above"` selects PIK above the level and `"below"` below it.
   * @throws Error - Throws a `validation` error if `variable` or `direction` is not a supported value or `threshold` is non-finite.
   */
  toggleExerciseThresholdJson(
    variable: 'hazard_rate' | 'distance_to_default' | 'leverage',
    threshold: number,
    direction: 'above' | 'below'
  ): string;
  /**
   * Build an optimal toggle-exercise model JSON payload.
   *
   * `nested_paths` is the Monte-Carlo path count for the nested optimal-exercise
   * simulation. It is rejected if it exceeds `Number.MAX_SAFE_INTEGER` (`2^53-1`):
   * `usize` counts marshal across the wasm boundary as IEEE-754 doubles, so a
   * larger value would round silently rather than fail loudly.
   * @returns Canonical optimal toggle-exercise model JSON.
   * @param nestedPaths - Number of nested Monte Carlo paths for continuation-value estimation; must fit JavaScript's safe integer range.
   * @param equityDiscountRate - Annual equity-holder discount rate used in the nested toggle decision.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a `TypeError` if `nested_paths` is not a safe non-negative integer, and a `validation` error if it is zero, a rate is non-finite, `asset_vol` is negative or non-finite, or `horizon` is not finite and positive.
   */
  toggleExerciseOptimalJson(
    nestedPaths: number,
    equityDiscountRate: number,
    assetVol: number,
    riskFreeRate: number,
    horizon: number
  ): string;
}

/**
 * Namespaced TypeScript entry points for credit derivatives calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const cds = JSON.parse(valuations.creditDerivatives.creditDefaultSwapExampleJson());
 * console.log(cds.instrument.type);
 * ```
 */
export interface CreditDerivativesNamespace {
  /**
   * Example tagged `CreditDefaultSwap` instrument JSON.
   * @returns Example tagged `CreditDefaultSwap` instrument JSON.
   * @throws Error - Throws a JavaScript exception if the example instrument fails validation or the example envelope cannot be serialized to JSON.
   */
  creditDefaultSwapExampleJson(): string;
  /**
   * Example tagged `CdsIndex` instrument JSON.
   * @returns Example tagged `CdsIndex` instrument JSON.
   * @throws Error - Throws a JavaScript exception if the example instrument fails validation or the example envelope cannot be serialized to JSON.
   */
  cdsIndexExampleJson(): string;
  /**
   * Example tagged `CdsTranche` instrument JSON.
   * @returns Example tagged `CdsTranche` instrument JSON.
   * @throws Error - Throws a JavaScript exception if the example instrument fails validation or the example envelope cannot be serialized to JSON.
   */
  cdsTrancheExampleJson(): string;
  /**
   * Example tagged `CdsOption` instrument JSON.
   * @returns Example tagged `CdsOption` instrument JSON.
   * @throws Error - Throws a JavaScript exception if the example option cannot be constructed or its envelope cannot be serialized to JSON.
   */
  cdsOptionExampleJson(): string;
}

/**
 * A structured input: JSON text, or the equivalent plain object or array.
 *
 * Objects are written to JSON by the binding before Rust parses them, so the
 * Rust type's unknown-field checks apply either way. `NaN`/`Infinity`, array
 * holes, functions, `Map`/`Date`/class instances and WASM handles throw a
 * `TypeError` with `kind: "invalid_type"`; `bigint` values are written as
 * exact integers and typed arrays as arrays.
 */
export type JsonInput = string | Record<string, unknown> | readonly unknown[];

/**
 * Composite-instrument construction, decomposition, execution, and history.
 *
 * Pricing uses frozen quantities. Only `initialize` and `rebalance` calculate
 * a new state. There is no `initializeFixed` export; `initialize` also
 * resolves `fixed_quantity` without history. Period return is `pnl / capital`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const fixed = { kind: "fixed_quantity" };
 * console.log(fixed.kind === "fixed_quantity");
 * ```
 */
export interface CompositeNamespace {
  /**
   * Resolve a bare specification into an immutable priceable envelope.
   *
   * Fixed-quantity specs do not require `history`. Volatility weighting
   * requires strictly increasing observations that end on `asOf`.
   * @param spec - Bare canonical `CompositeSpec` object or JSON string.
   * @param market - Complete market-context object or JSON string at `asOf`.
   * @param asOf - ISO-8601 state effective date; no later history is permitted.
   * @param history - Optional chronological market-observation array or JSON for volatility or expression inputs.
   * @returns Canonical resolved envelope plus primitive establishment trades.
   * @throws Error - Throws when JSON, dates, specifications, market inputs, history, metrics, notionals, or resolved quantities are invalid.
   */
  initialize(
    spec: JsonInput,
    market: JsonInput,
    asOf: string,
    history?: Record<string, unknown>[] | string
  ): CompositeRebalanceResult;
  /**
   * Explicitly resolve a distinct state without mutating prior quantities.
   *
   * Trades are net primitive quantity deltas from `instrument` to the new state.
   * @param instrument - Canonical resolved composite envelope object or JSON.
   * @param market - Complete rebalance-date market-context object or JSON.
   * @param asOf - ISO-8601 effective date for the new state.
   * @param history - Optional chronological market-observation array or JSON; required for volatility weighting and must end on `asOf`.
   * @returns New canonical envelope plus net primitive quantity deltas.
   * @throws Error - Throws for malformed inputs, invalid history, missing market data, or quantity-resolution failures.
   */
  rebalance(
    instrument: JsonInput,
    market: JsonInput,
    asOf: string,
    history?: Record<string, unknown>[] | string
  ): CompositeRebalanceResult;
  /**
   * Price frozen primitive paths and aggregate net/gross value and risk.
   *
   * Only additive metrics are accepted. Amounts are converted to the
   * composite reporting currency on `asOf`.
   * @param instrument - Canonical resolved composite envelope object or JSON.
   * @param market - Complete valuation and FX context object or JSON.
   * @param asOf - ISO-8601 valuation date used for prices, metrics, and FX.
   * @param metrics - Optional additive metric identifiers; omit or pass `[]` to report value only.
   * @returns Path-level primitive exposures and net/gross aggregates.
   * @throws Error - Throws for non-additive metrics, invalid state, missing market data, FX failures, or primitive pricing failures.
   */
  primitiveExposures(
    instrument: JsonInput,
    market: JsonInput,
    asOf: string,
    metrics?: string[]
  ): CompositeExposureReport;
  /**
   * Flatten target holdings or a transition into executable primitive deltas.
   * @param instrument - Canonical target resolved composite envelope.
   * @param previous - Optional prior resolved envelope; omit for establishment trades.
   * @returns Net primitive quantity-delta array.
   * @throws Error - Throws for malformed envelopes, invalid frozen states, or conflicting primitive definitions.
   */
  executionTrades(instrument: JsonInput, previous?: JsonInput): CompositeTrade[];
  /**
   * Initialize on the first supplied snapshot and calculate dated history.
   *
   * Warmup observations feed weighting only. The first output row has
   * `return_index = 100` and zero P&L. Scheduled rebalances are close-effective.
   * @param spec - Bare canonical `CompositeSpec` object or JSON string.
   * @param observations - Non-empty strictly increasing complete observation array or JSON.
   * @param warmup - Optional complete observations strictly before the output period.
   * @param metrics - Optional additive primitive metrics reported on every row; omit or pass `[]` for value only.
   * @returns Chronological value, cashflow, P&L, return, index, exposure, state, and trade rows.
   * @throws Error - Throws for empty, duplicate, unordered, or overlapping observations and any initialization, pricing, FX, or rebalance failure.
   */
  historyFromSpec(
    spec: JsonInput,
    observations: Record<string, unknown>[] | string,
    warmup?: Record<string, unknown>[] | string,
    metrics?: string[]
  ): CompositeHistoryRow[];
  /**
   * Calculate dated history from an already-resolved initial state.
   *
   * The initial effective date must be on or before the first observation.
   * Period return is `pnl / capital`; `return_index` starts at 100.
   * @param instrument - Canonical resolved composite envelope object or JSON.
   * @param observations - Non-empty strictly increasing complete observation array or JSON.
   * @param metrics - Optional additive primitive metrics reported on every row; omit or pass `[]` for value only.
   * @returns Chronological composite history rows.
   * @throws Error - Throws for invalid states or observations, missing inputs, or valuation and rebalance failures.
   */
  history(
    instrument: JsonInput,
    observations: Record<string, unknown>[] | string,
    metrics?: string[]
  ): CompositeHistoryRow[];
}

/**
 * Dynamic Nelson-Siegel and statistical yield-curve models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const yields = models.rates.dtsm.nelsonSiegelYields(
 *   0.7308, [0.03, -0.01, 0.005], [1, 5, 10]
 * );
 * ```
 */
export interface DtsmNamespace {
  /**
   * Evaluate the static Nelson-Siegel (1987) yield curve for one factor triple.
   *
   * @example
   * ```typescript
   * import init, { models } from "finstack-quant-wasm";
   * await init();
   * const yields = models.rates.dtsm.nelsonSiegelYields(
   *   0.7308, [0.03, -0.01, 0.005], [1, 5, 10]
   * );
   * ```
   * @param lambda - Exponential decay parameter for tenors in years; must be finite and greater than zero (0.7308 is the years-equivalent of Diebold-Li's 0.0609 months value).
   * @param factors - Nelson-Siegel `[level, slope, curvature]` (beta1, beta2, beta3) in decimal yield units such as `[0.06, -0.02, 0.01]`; exactly three numbers.
   * @param tenors - Maturities in years, each finite and non-negative; output order matches this array.
   * @returns One decimal yield per tenor, in the same order as `tenors`.
   * @throws Error - Throws a `TypeError` (`kind: "invalid_type"`) if `factors` or `tenors` is not an array of numbers, and a `FinstackError` (`kind: "validation"`) if `factors` does not hold exactly three entries, `lambda` is non-finite or non-positive, any factor loading is non-finite, or any tenor is non-finite or negative.
   */
  nelsonSiegelYields(lambda: number, factors: NumericArray, tenors: NumericArray): Float64Array;
}

/**
 * Product-independent interest-rate models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const yields = models.rates.dtsm.nelsonSiegelYields(
 *   0.7308, [0.03, -0.01, 0.005], [1, 5, 10]
 * );
 * ```
 */
export interface RatesNamespace {
  /**
   * Dynamic term-structure models.
   */
  dtsm: DtsmNamespace;
}

/**
 * Product-independent volatility models and evaluators.
 *
 * Core owns the serializable surface, cube, and FX-delta artifacts. This
 * namespace owns SABR behavior and all currently bound artifact evaluation.
 * @example
 * ```typescript
 * import init, { core, models } from "finstack-quant-wasm";
 * await init();
 * const cube = new core.VolCube(
 *   "USD-SWAPTION", [1], [5], [0.03, 0.5, -0.2, 0.4, Number.NaN], [0.03]
 * );
 * console.log(models.volatility.getCubeVol(cube, 1, 5, 0.03));
 * cube.free();
 * ```
 */
export interface VolatilityNamespace {
  /**
   * Validated SABR parameters.
   */
  SabrParameters: SabrParametersConstructor;
  /**
   * Hagan-2002 SABR volatility model.
   */
  SabrModel: SabrModelConstructor;
  /**
   * SABR smile at a fixed forward and expiry.
   */
  SabrSmile: SabrSmileConstructor;
  /**
   * Levenberg-Marquardt SABR calibrator with fixed beta.
   */
  SabrCalibrator: SabrCalibratorConstructor;
  /**
   * Evaluate checked Black/lognormal volatility from a data-only SABR cube.
   * @returns Annualized Black volatility as a decimal.
   * @param cube - Structurally validated data-only volatility cube.
   * @param expiry - Positive option expiry in years within the cube grid.
   * @param tenor - Positive underlying tenor in years within the cube grid.
   * @param strike - Finite strike in the same rate units as the stored forwards.
   * @throws Error - Throws a JavaScript exception for out-of-grid coordinates or invalid SABR model inputs.
   */
  getCubeVol(cube: VolCube, expiry: number, tenor: number, strike: number): number;
  /**
   * Evaluate Black/lognormal cube volatility with flat coordinate clamping.
   * @returns Annualized Black volatility, or `NaN` when undefined.
   * @throws Never; invalid inputs are represented by `NaN`.
   * @param cube - Structurally validated data-only volatility cube.
   * @param expiry - Finite option expiry in years; clamped to the stored grid.
   * @param tenor - Finite underlying tenor in years; clamped to the stored grid.
   * @param strike - Finite strike in the same rate units as the stored forwards.
   */
  getCubeVolClamped(cube: VolCube, expiry: number, tenor: number, strike: number): number;
  /**
   * Evaluate checked normal/Bachelier volatility from a data-only SABR cube.
   * @returns Annualized normal volatility in absolute rate units.
   * @param cube - Structurally validated data-only volatility cube.
   * @param expiry - Positive option expiry in years within the cube grid.
   * @param tenor - Positive underlying tenor in years within the cube grid.
   * @param strike - Finite strike in the same rate units as the stored forwards.
   * @throws Error - Throws a JavaScript exception for out-of-grid coordinates, an invalid shifted-SABR domain, or a failed normal-volatility expansion.
   */
  getCubeNormalVol(cube: VolCube, expiry: number, tenor: number, strike: number): number;
  /**
   * Evaluate normal/Bachelier cube volatility with coordinate clamping.
   * @returns Annualized normal volatility, or `NaN` when undefined.
   * @throws Never; invalid inputs are represented by `NaN`.
   * @param cube - Structurally validated data-only volatility cube.
   * @param expiry - Finite option expiry in years; clamped to the stored grid.
   * @param tenor - Finite underlying tenor in years; clamped to the stored grid.
   * @param strike - Finite strike in the same rate units as the stored forwards.
   */
  getCubeNormalVolClamped(cube: VolCube, expiry: number, tenor: number, strike: number): number;
  /**
   * Return ATM, 25-delta put, and 25-delta call vols at a stored FX expiry.
   * @returns Three annualized decimal volatilities in ATM, put, call order.
   * @param surface - Structurally validated data-only FX delta surface.
   * @param expiryIndex - Zero-based stored expiry index.
   * @throws Error - Throws a JavaScript exception when `expiry_index` is outside the surface.
   */
  getFxDeltaPillarVols(surface: FxDeltaVolSurface, expiryIndex: number): Float64Array;
  /**
   * Evaluate an FX delta-quoted surface at an expiry, strike, and forward.
   * @returns Annualized Black volatility as a decimal.
   * @param surface - Structurally validated data-only FX delta surface.
   * @param expiry - Positive option expiry in years.
   * @param strike - Positive strike in the FX quote currency.
   * @param forward - Positive FX forward in quote currency per base currency.
   * @throws Error - Throws a JavaScript exception for invalid coordinates or a non-positive reconstructed wing volatility.
   */
  getFxDeltaVol(
    surface: FxDeltaVolSurface,
    expiry: number,
    strike: number,
    forward: number
  ): number;
  /**
   * Convert premium-unadjusted forward call delta to strike.
   * @returns Strike in the same units as `forward`.
   * @throws Never; invalid inputs propagate IEEE non-finite results.
   * @param delta - Forward call delta as a decimal probability in `(0, 1)`.
   * @param forward - Positive forward in the same units as the returned strike.
   * @param vol - Positive annualized Black volatility as a decimal.
   * @param expiry - Positive option expiry in years.
   */
  deltaToStrike(delta: number, forward: number, vol: number, expiry: number): number;
  /**
   * Convert strike to premium-unadjusted forward call delta.
   * @returns Forward call delta as a decimal probability.
   * @throws Never; invalid inputs propagate IEEE non-finite results.
   * @param strike - Positive strike in the same units as `forward`.
   * @param forward - Positive forward in the same units as `strike`.
   * @param vol - Positive annualized Black volatility as a decimal.
   * @param expiry - Positive option expiry in years.
   */
  strikeToDelta(strike: number, forward: number, vol: number, expiry: number): number;
  /**
   * Convert an ATM volatility quote between normal, lognormal and shifted-lognormal conventions.
   * @returns Volatility in the target convention (decimal Black vol, or absolute normal vol).
   * @param vol - Input volatility in the source convention: decimal Black vol for `"lognormal"` / shifted-lognormal, absolute vol in the forward's rate units for `"normal"`. Must be positive.
   * @param fromConvention - `"normal"`, `"lognormal"`, or `{"shifted_lognormal": {"shift": s}}` (serde form of `VolatilityConvention`).
   * @param toConvention - Target convention in the same encoding.
   * @param forwardRate - ATM forward rate or price; must satisfy the target convention's domain.
   * @param timeToExpiry - Time to expiry in years (non-negative).
   * @throws Error - Throws a JavaScript exception if a convention cannot be decoded, an input is outside its domain, or the price-matching solver fails to converge.
   */
  convertAtmVolatility(
    vol: number,
    fromConvention: VolatilityConvention,
    toConvention: VolatilityConvention,
    forwardRate: number,
    timeToExpiry: number
  ): number;
  /**
   * Calibrate Gatheral SVI parameters to a market smile.
   * @returns Validated SVI parameters `{ a, b, rho, m, sigma }`.
   * @param strikes - Positive strikes (at least five).
   * @param vols - Black implied vols (decimal) aligned one-for-one with `strikes`.
   * @param forward - Positive forward at `expiry`.
   * @param expiry - Positive time to expiry in years.
   * @throws Error - Throws a JavaScript exception if lengths differ, fewer than five quotes are supplied, an input is outside its domain, the optimizer fails to converge, or the fit violates the SVI no-arbitrage conditions.
   */
  calibrateSvi(strikes: number[], vols: number[], forward: number, expiry: number): SviParams;
  /**
   * Black implied volatility from SVI parameters at log-moneyness `k = ln(K / F)`.
   * @returns Annualized Black volatility as a decimal.
   * @param params - SVI parameter object `{a, b, rho, m, sigma}` (validated on decode).
   * @param k - Log-moneyness `ln(K / F)`.
   * @param t - Positive time to expiry in years.
   * @throws Error - Throws a JavaScript exception if `params` fails validation, `t` is not positive, or the total variance at `k` is negative.
   */
  sviImpliedVol(params: SviParams, k: number, t: number): number;
}

/**
 * Product-independent liquidity risk and market-impact models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * console.log(models.liquidity.daysToLiquidate(1_000_000, 250_000, 0.20));
 * ```
 */
export interface LiquidityNamespace {
  /**
   * Estimate Roll effective spread from an ordered return series.
   * @param returns - Decimal returns in time order, as a `number[]` or `Float64Array`.
   * @returns Effective spread in return units, or `undefined` when it cannot be estimated.
   * @throws Error - Throws a `TypeError` if `returns` is not an array of numbers. Invalid estimator samples return `undefined`.
   */
  rollEffectiveSpread(returns: NumericArray): number | undefined;
  /**
   * Compute Amihud illiquidity from aligned returns and volumes.
   * @param returns - Decimal returns in time order, as a `number[]` or `Float64Array`.
   * @param volumes - Positive traded volumes aligned with `returns`, as a `number[]` or `Float64Array`.
   * @returns Mean absolute return per unit volume, or `undefined` for an invalid sample.
   * @throws Error - Throws a `TypeError` if either argument is not an array of numbers. Invalid estimator samples return `undefined`.
   */
  amihudIlliquidity(returns: NumericArray, volumes: NumericArray): number | undefined;
  /**
   * Calculate the trading days required to liquidate a position.
   * @param positionQuantity - Shares or contracts to liquidate; the absolute value is used.
   * @param adv - Average daily volume in the same quantity units.
   * @param participationRate - Fraction of ADV available for execution each trading day.
   * @returns Liquidation horizon in trading days, or infinity for non-positive capacity.
   */
  daysToLiquidate(positionQuantity: number, adv: number, participationRate: number): number;
  /**
   * Classify a liquidation horizon into a liquidity tier, using the Rust
   * `LiquidityConfig` default thresholds unless custom ones are supplied.
   * @param daysToLiquidate - Estimated unwind horizon in trading days.
   * @param thresholds - Optional upper bounds of Tiers 1-4 in trading days, `[tier1Max, tier2Max, tier3Max, tier4Max]`; omitted or `null` uses the Rust `LiquidityConfig` default `[1, 5, 20, 60]`.
   * @returns One of `tier1` through `tier5`, with Tier 1 the most liquid.
   * @throws Error - Throws a `TypeError` if `thresholds` is not an array of four numbers, and a `validation` error if a threshold is non-finite or not positive, or the thresholds are not strictly ascending.
   */
  liquidityTier(daysToLiquidate: number, thresholds?: NumericArray | null): string;
  /**
   * Compute Bangia liquidity-adjusted VaR under the loss-sign convention.
   * @param spreadMean - Finite non-negative mean relative bid-ask spread as a decimal.
   * @param spreadVol - Finite non-negative volatility of the relative spread.
   * @param confidence - Confidence level strictly between 0.5 and 1.
   * @param positionValue - Finite current market value; only its magnitude is used.
   * @returns An object containing `var`, `spread_cost`, `lvar`, and `lvar_ratio`.
   * @throws Error - Throws a JavaScript exception if an input violates the stated finiteness, sign, or range contract, or if the result cannot be converted.
   * @param varValue - Loss-convention VaR in the same units as `positionValue`; must be non-positive.
   */
  lvarBangia(
    varValue: number,
    spreadMean: number,
    spreadVol: number,
    confidence: number,
    positionValue: number
  ): LvarBangiaScalar;
  /**
   * Estimate uniform Almgren-Chriss execution-impact components.
   * @param positionSize - Finite signed quantity in shares or contracts.
   * @param avgDailyVolume - Positive finite ADV in matching quantity units.
   * @param volatility - Positive finite daily volatility as a decimal.
   * @param executionHorizonDays - Positive finite execution horizon in trading days.
   * @param permanentImpactCoef - Non-negative finite multiplier on permanent impact.
   * @param temporaryImpactCoef - Positive finite multiplier on temporary impact.
   * @param referencePrice - Optional positive finite price for notional and basis-point scaling.
   * @returns Permanent, temporary, total, basis-point, and execution-risk impact fields.
   * @throws Error - Throws a JavaScript exception if an input violates the stated finiteness, sign, or range contract, calculation fails, or conversion fails.
   */
  almgrenChrissImpact(
    positionSize: number,
    avgDailyVolume: number,
    volatility: number,
    executionHorizonDays: number,
    permanentImpactCoef: number,
    temporaryImpactCoef: number,
    referencePrice?: number | null
  ): ImpactEstimate;
  /**
   * Estimate price-space Kyle lambda using an Amihud-ratio proxy.
   *
   * Argument order matches `amihudIlliquidity`: returns first, then volumes.
   * @param returns - Decimal returns in time order, as a `number[]` or `Float64Array`.
   * @param volumes - Positive volume observations aligned with `returns`, as a `number[]` or `Float64Array`.
   * @param referencePrice - Positive price per share or contract.
   * @returns Estimated price-space impact coefficient, or `undefined` for invalid inputs.
   * @throws Error - Throws a `TypeError` if `returns` or `volumes` is not an array of numbers. Invalid estimator samples return `undefined`.
   */
  kyleLambda(
    returns: NumericArray,
    volumes: NumericArray,
    referencePrice: number
  ): number | undefined;
}

/**
 * Namespaced TypeScript entry points for reusable quantitative models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * console.log(models.bsPrice(100, 100, 0.03, 0, 0.2, 1, true));
 * ```
 */
export interface ModelsNamespace {
  /**
   * Factor definitions, covariance, matching, and credit-factor calibration.
   */
  factor: FactorNamespace;
  /**
   * Product-independent liquidity risk and market-impact models.
   */
  liquidity: LiquidityNamespace;
  /**
   * Monte Carlo pricing engines.
   */
  monteCarlo: MonteCarloNamespace;
  /**
   * Structural-credit models and toggle-exercise helpers.
   */
  credit: ModelCreditNamespace;
  /**
   * Copula, recovery, and credit-correlation model infrastructure.
   */
  correlation: CorrelationNamespace;
  /**
   * Product-independent interest-rate models.
   */
  rates: RatesNamespace;
  /**
   * Product-independent volatility models and evaluators.
   */
  volatility: VolatilityNamespace;
  /**
   * Per-unit Black-Scholes / Garman-Kohlhagen price of a European option.
   *
   * Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
   * Merton (1973): see docs/REFERENCES.md#merton-1973.
   * Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
   *
   * @example
   * ```javascript
   * import init, { models } from "finstack-quant-wasm";
   * await init();
   * const price = models.bsPrice(
   *   100,    // spot
   *   100,    // strike (ATM)
   *   0.05,   // rate = 5%
   *   0.0,    // divYield = 0
   *   0.20,   // vol = 20%
   *   1.0,    // expiry = 1 year
   *   true,   // call
   * );
   * // price ≈ 10.45
   * ```
   *
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param rate - Risk-free rate, **decimal** continuously compounded (e.g. `0.05` for 5%).
   * @param divYield - Continuous dividend yield (or foreign rate for FX), **decimal** continuously compounded.
   * @param vol - Annualized volatility, **decimal** (e.g. `0.20` for 20%).
   * @param expiry - Time to expiry in **years**.
   * @param isCall - `true` for a call, `false` for a put.
   * @returns Per-unit option price.
   * @throws If spot or strike is non-positive, volatility or expiry is negative, any numerical input is non-finite, or discounted legs or price overflow.
   */
  bsPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    isCall: boolean
  ): number;
  /**
   * Vanilla option payoff at expiry: `max(±(spot - strike), 0)`.
   *
   * @example
   * ```javascript
   * import init, { models } from "finstack-quant-wasm";
   * await init();
   * const payoff = models.vanillaExpiryPayoff(110, 100, true);
   * // payoff === 10
   * ```
   *
   * @param spot - Underlying level at expiry, in the same price units as `strike`. Must be finite and non-negative; zero spot is allowed.
   * @param strike - Exercise price; must be finite and strictly positive.
   * @param isCall - `true` for a call (`max(spot - strike, 0)`), `false` for a put (`max(strike - spot, 0)`).
   * @returns Undiscounted expiry payoff in the same units as `spot` and `strike`.
   * @throws If `spot` is non-finite or negative, or `strike` is non-finite or not strictly positive.
   */
  vanillaExpiryPayoff(spot: number, strike: number, isCall: boolean): number;
  /**
   * Black-Scholes / Garman-Kohlhagen Greeks as a `{delta, gamma, vega, theta, rho_r, rho_q}` object.
   *
   * Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
   * Merton (1973): see docs/REFERENCES.md#merton-1973.
   * Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
   *
   * @example
   * ```javascript
   * const g = models.bsGreeks(100, 100, 0.05, 0.0, 0.20, 1.0, true);
   * // g.delta ≈ 0.64, g.gamma ≈ 0.019, g.vega ≈ 0.38 (per 1% vol)
   * ```
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param rate - Risk-free rate, **decimal** continuously compounded.
   * @param divYield - Dividend yield (or foreign rate for FX), **decimal** continuously compounded.
   * @param vol - Annualized volatility, **decimal**; must be positive.
   * @param expiry - Time to expiry in **years**; must be positive.
   * @param isCall - `true` for a call, `false` for a put.
   * @param thetaDaysPerYear - Day-count denominator for theta. Default `365`. Pass `252` for trading-day theta.
   * @returns Object `{ delta, gamma, vega, theta, rho_r, rho_q }` (snake_case keys matching the Rust/Python canonical `BsGreeks` fields). `vega` and both rho values are **per 1% move**; `theta` is **per day** under `thetaDaysPerYear`.
   * @throws If serialization to JS fails (should not happen on valid inputs).
   */
  bsGreeks(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    isCall: boolean,
    thetaDaysPerYear?: number
  ): BsGreeks;
  /**
   * Solve for Black-Scholes / Garman-Kohlhagen implied volatility.
   *
   * Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
   * Merton (1973): see docs/REFERENCES.md#merton-1973.
   * Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
   *
   * @example
   * ```javascript
   * const iv = models.bsImpliedVol(100, 100, 0.05, 0.0, 1.0, 10.45, true);
   * // iv ≈ 0.20
   * ```
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param rate - Risk-free rate, **decimal** continuously compounded.
   * @param divYield - Dividend yield, **decimal** continuously compounded.
   * @param expiry - Time to expiry in **years**; must be positive.
   * @param price - Observed option price (per unit).
   * @param isCall - `true` for a call, `false` for a put.
   * @returns Annualized implied volatility, **decimal** (e.g. `0.20`).
   * @throws If `expiry` is not positive, `price` is below intrinsic value, above the no-arbitrage upper bound, or the solver fails to converge.
   */
  bsImpliedVol(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    expiry: number,
    price: number,
    isCall: boolean
  ): number;
  /**
   * Solve for Black-76 (forward-based) implied volatility.
   *
   * Black (1976): see docs/REFERENCES.md#black-1976.
   * @returns Annualized Black-76 implied volatility as a decimal.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param df - Discount factor from valuation to expiry, expressed as a positive decimal.
   * @param expiry - Time to expiry in years; must be positive.
   * @param price - Observed option price in the same units as the forward.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if an input is non-finite; `expiry`, `forward`, `strike`, `df`, or `price` is not positive; the price is not above intrinsic value or cannot be bracketed; or the implied-volatility solver does not converge.
   */
  black76ImpliedVol(
    forward: number,
    strike: number,
    df: number,
    expiry: number,
    price: number,
    isCall: boolean
  ): number;
  /**
   * Black-76 per-unit price of a European option on a forward: `df * Black(F, K, vol, expiry)`.
   *
   * The Rust `closed_form::black76_price` owns the call/put dispatch, the
   * discounting and the input validation (`df` must be positive, as for
   * `black76ImpliedVol`). Black (1976): see docs/REFERENCES.md#black-1976.
   * @returns Discounted per-unit option price in the units of `forward`.
   * @param forward - Forward price or rate at expiry.
   * @param strike - Strike in the same units as `forward`.
   * @param df - Discount factor from valuation to expiry; a finite decimal strictly greater than zero (the same domain `black76ImpliedVol` accepts).
   * @param expiry - Time to expiry in years; non-negative.
   * @param vol - Annualized lognormal (Black) volatility, decimal; non-negative.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite, `forward`, `strike` or `df` is not positive, `vol` or `expiry` is negative, or the price is non-finite.
   */
  black76Price(
    forward: number,
    strike: number,
    df: number,
    expiry: number,
    vol: number,
    isCall: boolean
  ): number;
  /**
   * Black-76 undiscounted forward Greeks `{ delta, gamma, vega }`.
   *
   * `delta` / `gamma` are with respect to the forward; `vega` is per unit (1.0) change in `vol`.
   * Black (1976): see docs/REFERENCES.md#black-1976.
   * @param forward - Forward price or rate at expiry.
   * @param strike - Strike in the same units as `forward`.
   * @param expiry - Time to expiry in years; non-negative.
   * @param vol - Annualized lognormal (Black) volatility, decimal; non-negative.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @returns The Rust `ForwardGreeks` object `{ delta, gamma, vega }`.
   * @throws Error - Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite, `forward` or `strike` is not positive, `vol` or `expiry` is negative, or a Greek is non-finite.
   */
  black76Greeks(
    forward: number,
    strike: number,
    expiry: number,
    vol: number,
    isCall: boolean
  ): ForwardGreeks;
  /**
   * Bachelier (normal-model) undiscounted per-unit option price.
   *
   * Bachelier (1900): see docs/REFERENCES.md#bachelier-1900.
   * @returns Undiscounted per-unit option value in the units of `forward`.
   * @param forward - Forward price or rate at expiry (may be negative).
   * @param strike - Strike in the same units as `forward`.
   * @param normalVol - Annualized **absolute** (normal) volatility in the units of `forward` (e.g. `0.0075` for 75 bp on decimal rates).
   * @param expiry - Time to expiry in years; non-negative.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite, `normalVol` or `expiry` is negative, or the price is non-finite.
   */
  bachelierPrice(
    forward: number,
    strike: number,
    normalVol: number,
    expiry: number,
    isCall: boolean
  ): number;
  /**
   * Bachelier (normal-model) undiscounted forward Greeks `{ delta, gamma, vega }`.
   *
   * `vega` is per unit (1.0) change in `normalVol` (absolute units).
   * Bachelier (1900): see docs/REFERENCES.md#bachelier-1900.
   * @param forward - Forward price or rate at expiry (may be negative).
   * @param strike - Strike in the same units as `forward`.
   * @param normalVol - Annualized absolute (normal) volatility in the units of `forward`; non-negative.
   * @param expiry - Time to expiry in years; non-negative.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @returns The Rust `ForwardGreeks` object `{ delta, gamma, vega }`.
   * @throws Error - Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite, `normalVol` or `expiry` is negative, or a Greek is non-finite.
   */
  bachelierGreeks(
    forward: number,
    strike: number,
    normalVol: number,
    expiry: number,
    isCall: boolean
  ): ForwardGreeks;
  /**
   * Shifted (displaced) Black undiscounted per-unit price for negative-rate markets.
   *
   * Prices `Black(forward + shift, strike + shift, vol, expiry)`.
   * @returns Undiscounted per-unit option value in the units of `forward`.
   * @param forward - Forward rate at expiry (decimal; may be negative).
   * @param strike - Strike (decimal, same units as `forward`).
   * @param vol - Annualized shifted-lognormal volatility, decimal.
   * @param expiry - Time to expiry in years.
   * @param shift - Displacement added to forward and strike, in rate units (e.g. `0.03` for a 3% shift); both shifted values must be positive.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite, `forward + shift` or `strike + shift` is not positive, `vol` or `expiry` is negative, or the price is non-finite.
   */
  blackShiftedPrice(
    forward: number,
    strike: number,
    vol: number,
    expiry: number,
    shift: number,
    isCall: boolean
  ): number;
  /**
   * Shifted (displaced) Black vega per unit (1.0) change in `vol`, undiscounted.
   * @returns Undiscounted vega in the units of `forward` per unit vol.
   * @param forward - Forward rate at expiry (decimal; may be negative).
   * @param strike - Strike (decimal, same units as `forward`).
   * @param vol - Annualized shifted-lognormal volatility, decimal.
   * @param expiry - Time to expiry in years.
   * @param shift - Displacement added to forward and strike, in rate units; both shifted values must be positive.
   * @throws Error - Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite, `forward + shift` or `strike + shift` is not positive, `vol` or `expiry` is negative, or the vega is non-finite.
   */
  blackShiftedVega(
    forward: number,
    strike: number,
    vol: number,
    expiry: number,
    shift: number
  ): number;
  /**
   * Reiner-Rubinstein continuous-monitoring barrier call price.
   *
   * `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
   * Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991.
   * @returns Discounted barrier-call price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param barrier - Continuously monitored barrier level in the same price units as spot.
   * @param rate - Continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to expiry in years.
   * @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
   * @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
   * @throws Error - Throws a JavaScript exception if `direction` or `knock` is unsupported, or the supplied model inputs produce a non-finite barrier price.
   */
  barrierCall(
    spot: number,
    strike: number,
    barrier: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    direction: 'up' | 'down',
    knock: 'in' | 'out'
  ): number;
  /**
   * Reiner-Rubinstein continuous-monitoring barrier put price.
   *
   * `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
   * Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991.
   * @returns Discounted barrier-put price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param barrier - Continuously monitored barrier level in the same price units as spot.
   * @param rate - Continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to expiry in years.
   * @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
   * @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
   * @throws Error - Throws a JavaScript exception if `direction` or `knock` is unsupported, or the supplied model inputs produce a non-finite barrier price.
   */
  barrierPut(
    spot: number,
    strike: number,
    barrier: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    direction: 'up' | 'down',
    knock: 'in' | 'out'
  ): number;
  /**
   * Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option.
   *
   * Kemna-Vorst (1990): see docs/REFERENCES.md#kemna-vorst-1990.
   * Turnbull-Wakeman (1991): see docs/REFERENCES.md#turnbull-wakeman-1991.
   * @returns Discounted Asian option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to expiry in years.
   * @param numFixings - Positive number of equally spaced averaging observations before expiry.
   * @param averaging - Asian averaging convention: `"arithmetic"` (default) or `"geometric"`.
   * @param isCall - Whether to value a call (`true`, default) or put (`false`).
   * @throws Error - Throws a JavaScript exception if `numFixings` is not a positive whole number, `averaging` is not `"arithmetic"` or `"geometric"`, or the supplied model inputs produce a non-finite option price.
   */
  asianOptionPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numFixings: number,
    averaging?: 'arithmetic' | 'geometric',
    isCall?: boolean
  ): number;
  /**
   * Conze-Viswanathan lookback option.
   *
   * `strike_type` is `"fixed"` (default) or `"floating"`. For `"floating"`,
   * `strike` is ignored and `extremum` is the observed min/max to date.
   * Conze-Viswanathan (1991): see docs/REFERENCES.md#conze-viswanathan-1991.
   * @returns Discounted lookback option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to expiry in years.
   * @param extremum - Observed maximum for fixed calls or floating puts, minimum for fixed puts or floating calls, including current spot; in spot-price units.
   * @param strikeType - Lookback payoff convention: `"fixed"` (default) or `"floating"`.
   * @param isCall - Whether to value a call (`true`, default) or put (`false`).
   * @throws Error - Throws a JavaScript exception if `strikeType` is not `"fixed"` or `"floating"`, or the supplied model inputs produce a non-finite option price.
   */
  lookbackOptionPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    extremum: number,
    strikeType?: 'fixed' | 'floating',
    isCall?: boolean
  ): number;
  /**
   * Quanto option (FX-adjusted cross-currency) price in domestic currency.
   *
   * Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
   * Brigo-Mercurio (2006): see docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models.
   *
   * @returns Discounted quanto option price in domestic currency units.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param expiry - Time to expiry in years.
   * @param rateDomestic - Domestic continuously compounded risk-free rate, expressed as a decimal.
   * @param rateForeign - Foreign continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param volAsset - Annualized asset-price volatility expressed as a decimal.
   * @param volFx - Annualized FX-rate volatility expressed as a decimal.
   * @param correlation - Instantaneous correlation between the asset and FX-rate shocks, from -1 to 1.
   * @param isCall - Whether to value a call (`true`, default) or put (`false`).
   * @throws If the inputs produce a non-finite price.
   */
  quantoOptionPrice(
    spot: number,
    strike: number,
    expiry: number,
    rateDomestic: number,
    rateForeign: number,
    divYield: number,
    volAsset: number,
    volFx: number,
    correlation: number,
    isCall?: boolean
  ): number;
  /**
   * Closed-form (Fourier) Heston price of a European option.
   *
   * Heston (1993): see docs/REFERENCES.md#heston-1993.
   * @returns Present-value per-unit option price.
   * @param spot - Current spot price in the same units as the strike.
   * @param strike - Option strike price.
   * @param expiry - Time to expiry in years; a non-positive value returns intrinsic.
   * @param rate - Continuously compounded risk-free rate, decimal.
   * @param divYield - Continuous dividend yield or foreign rate, decimal.
   * @param kappa - Mean-reversion speed of the variance process (per year).
   * @param theta - Long-run variance level (variance units).
   * @param sigmaV - Volatility of variance (vol-of-vol).
   * @param rho - Spot/variance correlation in `(-1, 1)`.
   * @param v0 - Initial instantaneous variance (variance, not volatility).
   * @param isCall - Whether to value a call (`true`, default) or put (`false`).
   * @throws Error - Throws a JavaScript exception if a parameter is non-finite or outside its domain, or the Fourier integration fails to produce a finite price.
   */
  hestonPrice(
    spot: number,
    strike: number,
    expiry: number,
    rate: number,
    divYield: number,
    kappa: number,
    theta: number,
    sigmaV: number,
    rho: number,
    v0: number,
    isCall?: boolean
  ): number;
  /**
   * Price a European option under the Black-Scholes model using the COS method.
   *
   * Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
   * Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
   * @returns Discounted European option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%; must be positive.
   * @param expiry - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @param nTerms - Optional number of COS expansion terms in `1..=65536`; omit to use the pricer default (128).
   * @throws Error - Throws a `validation` error if `nTerms` is outside `1..=65536` or `vol` is not positive, and a `computation` error if the model produces a degenerate or invalid COS truncation range, a non-finite characteristic-function value or forward moment, or a non-finite option price.
   */
  bsCosPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /**
   * Price a European option under the Variance Gamma model using the COS method.
   *
   * Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
   * Madan-Carr-Chang (1998): see docs/REFERENCES.md#madan-carr-chang-1998.
   * @returns Discounted European option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param theta - Variance-Gamma drift parameter controlling skew in log returns.
   * @param nu - Variance-Gamma variance-rate parameter; larger values increase tail thickness.
   * @param expiry - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @param nTerms - Optional number of COS expansion terms in `1..=65536`; omit to use the pricer default (128).
   * @throws Error - Throws a `validation` error if `nTerms` is outside `1..=65536`, and a `computation` error if the model produces a degenerate or invalid COS truncation range, a non-finite characteristic-function value or forward moment, or a non-finite option price.
   */
  vgCosPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    sigma: number,
    theta: number,
    nu: number,
    expiry: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /**
   * Price a European option under Merton (1976) jump-diffusion using the COS method.
   *
   * Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
   * Merton jump-diffusion (1976): see docs/REFERENCES.md#merton-1976-jump.
   * @returns Discounted European option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param muJump - Mean log jump size in the Merton jump-diffusion model.
   * @param sigmaJump - Standard deviation of log jump sizes in the Merton jump-diffusion model.
   * @param lambda - Annual jump-arrival intensity in the Merton jump-diffusion model.
   * @param expiry - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @param nTerms - Optional number of COS expansion terms in `1..=65536`; omit to use the pricer default (128).
   * @throws Error - Throws a `validation` error if `nTerms` is outside `1..=65536`, and a `computation` error if the model produces a degenerate or invalid COS truncation range, a non-finite characteristic-function value or forward moment, or a non-finite option price.
   */
  mertonJumpCosPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    sigma: number,
    muJump: number,
    sigmaJump: number,
    lambda: number,
    expiry: number,
    isCall: boolean,
    nTerms?: number
  ): number;
}

/**
 * Namespaced TypeScript entry point for models APIs.
 */
export declare const models: ModelsNamespace;

/**
 * Quote ingestion, market construction, and explicit model calibration.
 * @example
 * ```typescript
 * import init, { calibration } from "finstack-quant-wasm";
 * await init();
 * const report = calibration.dryRun({
 *   schema: "finstack_quant.calibration/1",
 *   plan: { id: "smoke", description: null, quote_sets: {}, steps: [], settings: {} }
 * });
 * console.log(report.errors);
 * ```
 */
export interface CalibrationNamespace {
  /**
   * Execute a calibration envelope and return its fitted market and reports.
   * @param envelope - Typed calibration envelope or its serialized JSON form.
   * @returns Calibration result including the materialized market and per-step reports.
   * @throws Error - Throws a JavaScript exception if `envelopeJson` is malformed or violates the calibration schema or static plan contract (fail-fast: first static error; `dryRun` lists every static error), market context construction or a calibration step fails, a solver does not converge, or the result envelope cannot be converted to a JavaScript value.
   */
  calibrate(envelope: CalibrationEnvelope | string): CalibrationResultEnvelope;
  /**
   * Validate and canonicalize a calibration envelope without solving it.
   * @param envelope - Typed calibration envelope or its serialized JSON form.
   * @returns Canonical pretty-printed calibration-envelope JSON.
   * @throws Error - Throws a JavaScript exception if `json` is malformed, its calibration schema marker is missing, malformed, or unsupported, static envelope validation fails (fail-fast: first error; `dryRun` lists every static error), or the canonical envelope cannot be serialized.
   */
  validateCalibrationJson(envelope: CalibrationEnvelope | string): string;
  /**
   * Return all static plan errors and dependencies without running solvers.
   * @param envelope - Typed calibration envelope or its serialized JSON form.
   * @returns `CalibrationValidationReport` object containing every static error and the dependency graph.
   * @throws Error - Throws a JavaScript exception if `envelopeJson` is malformed, its schema marker is missing, malformed, or unsupported, the envelope structure is invalid, or the validation report cannot be converted to a JavaScript value. Semantic findings are returned in the report rather than thrown.
   */
  dryRun(envelope: CalibrationEnvelope | string): CalibrationValidationReport;
  /**
   * JSON wire twin of `dryRun`: the validation report as pretty-printed JSON.
   * @param envelope - Typed calibration envelope or its serialized JSON form.
   * @returns Pretty-printed `CalibrationValidationReport` JSON.
   * @throws Error - Throws a JavaScript exception if `envelopeJson` is malformed, its schema marker is missing, malformed, or unsupported, the envelope structure is invalid, or the validation report cannot be serialized. Semantic findings are returned in the report rather than thrown.
   */
  dryRunJson(envelope: CalibrationEnvelope | string): string;
  /**
   * Fit the Bermudan LMM loading scale from the market swaption surface.
   * @param market - Reusable market handle containing discount and swaption-volatility inputs.
   * @param asOf - ISO-8601 valuation date.
   * @returns Positive finite LMM base volatility.
   * @throws Error - Throws if the envelope is not a Bermudan swaption, the date or market inputs are invalid, or the Rebonato calibration cannot be completed.
   * @param instrument - Instrument used by this call.
   */
  calibrateBermudanLmmBaseVol(
    instrument: Record<string, unknown> | string,
    market: MarketContext,
    asOf: string
  ): number;
  /**
   * Canonical content hash of a calibration envelope (twin of Python `CalibrationEnvelope.content_hash`).
   * @param envelopeJson - `CalibrationEnvelope` (object or JSON).
   * @returns `"sha256:<hex>"` content hash.
   * @throws Error - Throws a `CalibrationEnvelopeError` if the envelope is malformed, its schema marker is missing or unsupported, or it contains a non-finite number.
   */
  calibrationEnvelopeContentHash(envelopeJson: CalibrationEnvelope | string): string;
  /**
   * Canonical content hash of a calibration result envelope (twin of Python `CalibrationResult.content_hash`).
   * @param resultJson - `CalibrationResultEnvelope` returned by `calibrate` (object or JSON).
   * @returns `"sha256:<hex>"` content hash.
   * @throws Error - Throws a `CalibrationEnvelopeError` if the result is malformed, exceeds the default load limits, its schema marker is missing or unsupported, or it contains a non-finite number.
   */
  calibrationResultContentHash(resultJson: CalibrationResultEnvelope | string): string;
}

/**
 * Namespaced TypeScript entry point for calibration APIs.
 */
export declare const calibration: CalibrationNamespace;

/**
 * Namespaced TypeScript entry points for valuations calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * console.log(valuations.instruments.listStandardMetrics());
 * ```
 */
export interface ValuationsNamespace {
  /**
   * Generic cross-asset composite instruments, primitive exposures, and dated history.
   *
   * Frozen quantities are used for pricing. Host bindings expose `initialize`
   * for both fixed and dynamic weighting; there is no `initializeFixed` export.
   */
  composite: CompositeNamespace;
  /**
   * CDS-family JSON wrappers and pricing helpers.
   */
  creditDerivatives: CreditDerivativesNamespace;
  /**
   * Direct FX instrument wrappers.
   */
  fx: FxNamespace;
  /**
   * Instrument JSON validation and pricing helpers.
   */
  instruments: ValuationInstrumentsNamespace;
  /**
   * Listed-market coverage metadata and canonical instrument routing.
   */
  market: ValuationMarketNamespace;
  /**
   * Deserialize a `ValuationResult` from JSON and return the canonical JSON.
   *
   * Validates the input conforms to the `ValuationResult` schema.
   * @returns Canonical `ValuationResult` JSON after deserialization.
   * @param json - Canonical valuation-result JSON to validate and reserialize.
   * @throws Error - Throws a JavaScript exception if `json` is malformed or does not match the `ValuationResult` schema, or the canonical result cannot be serialized.
   */
  validateValuationResultJson(json: JsonInput): string;
  /**
   * Serialize a structured `ValuationResult` object to canonical JSON.
   *
   * The inverse of the structured `priceInstrument*` return: it accepts the
   * plain object those entry points return, with 64-bit fields such as the
   * Monte Carlo `seed` as `bigint`, and writes the same canonical JSON as
   * Python `ValuationResult.to_json()`, keeping every integer exact. Use it in
   * place of `JSON.stringify`, which throws on `bigint`.
   * @param result - `ValuationResult` object returned by `priceInstrument`, `priceInstrumentWithMarket`, a typed instrument's `price`, or a portfolio valuation's `valuation_result` entry; 64-bit fields must be `BigInt` or safe-integer numbers.
   * @returns Canonical `ValuationResult` JSON text.
   * @throws Error - Throws a JavaScript exception if `result` does not match the `ValuationResult` schema (for example a seed given as a string) or the canonical result cannot be serialized.
   */
  valuationResultToJson(result: ValuationResult): string;
  /**
   * Decoded series of one composite base metric from a valuation result (twin of Python `ValuationResult.metric_series`).
   * @param result - `ValuationResult` object returned by `priceInstrument` (or its canonical JSON); 64-bit fields may be `BigInt`.
   * @param base - Canonical base metric identifier (e.g. `"bucketed_dv01"`).
   * @returns `[components, value]` pairs in measure order.
   * @throws Error - Throws with kind `validation` if `result` does not match the `ValuationResult` schema or `base` is not a canonically encoded metric key.
   */
  valuationResultMetricSeries(result: ValuationResult | string, base: string): [string[], number][];
  /**
   * Simulated TARN coupon profile along a deterministic floating-rate path.
   *
   * Returns a JSON object:
   * ```text
   * {
   *   "coupons_paid": number[],
   *   "cumulative":   number[],
   *   "redemption_index": number | null,
   *   "redeemed_early":   boolean
   * }
   * ```
   *
   * Each period's coupon is `max(fixed_rate - L_i, coupon_floor) * day_count_fraction`.
   * Payments accumulate in a
   * [`CumulativeCouponTracker`](finstack_quant_valuations::instruments::rates::hw1f::cumulative_coupon::CumulativeCouponTracker) configured with
   * `target_coupon`; once cumulative hits the target, the final coupon is
   * capped and the instrument is considered redeemed.
   * @returns Period coupons, running cumulative, redemption index, and whether the TARN redeemed early.
   * @param fixedRate - Fixed coupon rate in decimal form before subtracting each floating fixing.
   * @param couponFloor - Minimum period coupon rate in decimal form after the TARN rate calculation.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param targetCoupon - Cumulative coupon target, as a fraction of notional, that redeems the TARN.
   * @param dayCountFraction - Accrual year fraction applied to each coupon period.
   * @throws Error - Throws a JavaScript exception if `fixed_rate`, `coupon_floor`, `target_coupon`, `day_count_fraction`, or any fixing is non-finite; `coupon_floor` is negative; `target_coupon` or `day_count_fraction` is non-positive; or the result cannot be converted to a JavaScript object.
   */
  tarnCouponProfile(
    fixedRate: number,
    couponFloor: number,
    floatingFixings: number[],
    targetCoupon: number,
    dayCountFraction: number
  ): {
    coupons_paid: number[];
    cumulative: number[];
    redemption_index: number | null;
    redeemed_early: boolean;
  };
  /**
   * Snowball coupon schedule.
   *
   *   `c_i = clip(c_{i-1} + fixed_rate - L_i, floor, cap)` with `c_0 = initial_coupon`.
   * @returns One decimal coupon rate per fixing, in the same order as `floatingFixings`.
   * @param initialCoupon - Starting coupon rate before the first snowball update, in decimal form.
   * @param fixedRate - Fixed coupon rate in decimal form added at each snowball step.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param couponFloor - Minimum permitted coupon rate in decimal form.
   * @param couponCap - Optional maximum permitted coupon rate in decimal form; `null`/`undefined` leaves the coupon uncapped.
   * @throws Error - Throws a JavaScript exception if `initial_coupon` or `coupon_floor` is negative; `initial_coupon`, `fixed_rate`, `coupon_floor`, or any fixing is non-finite; or `coupon_cap` is set and is non-finite or not greater than `coupon_floor`.
   */
  snowballCouponProfile(
    initialCoupon: number,
    fixedRate: number,
    floatingFixings: number[],
    couponFloor: number,
    couponCap: number | null | undefined
  ): Float64Array;
  /**
   * Path-independent inverse-floater coupon schedule.
   * @returns One decimal coupon rate per fixing, in the same order as `floatingFixings`.
   * @param fixedRate - Fixed coupon rate in decimal form before the geared floating deduction.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param couponFloor - Minimum permitted coupon rate in decimal form.
   * @param couponCap - Optional maximum permitted coupon rate in decimal form; `null`/`undefined` leaves the coupon uncapped.
   * @param gearing - Positive multiplier applied to each floating fixing in the inverse-floater coupon.
   * @throws Error - Throws a JavaScript exception if `coupon_floor` is negative; `fixed_rate`, `coupon_floor`, `gearing`, or any fixing is non-finite; `gearing` is non-positive; or `coupon_cap` is set and is non-finite or not greater than `coupon_floor`.
   */
  inverseFloaterCouponProfile(
    fixedRate: number,
    floatingFixings: number[],
    couponFloor: number,
    couponCap: number | null | undefined,
    gearing: number
  ): Float64Array;
  /**
   * Intrinsic (undiscounted, unhedged) payoff of a CMS spread option.
   *
   * `call:  notional * max(long_cms - short_cms - strike, 0)`
   * `put:   notional * max(strike - (long_cms - short_cms), 0)`
   * @returns Undiscounted intrinsic payoff in the same units as `notional`.
   * @param longCms - Long-tenor CMS rate in decimal form.
   * @param shortCms - Short-tenor CMS rate in decimal form.
   * @param strike - CMS rate-spread strike in decimal form.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @param notional - Signed trade notional in the instrument's native currency units.
   * @throws Error - Throws a JavaScript exception if a CMS rate or `strike` is non-finite, or if `notional` is negative or non-finite.
   */
  cmsSpreadOptionIntrinsic(
    longCms: number,
    shortCms: number,
    strike: number,
    isCall: boolean,
    notional: number
  ): number;
  /**
   * Accrued coupon on a range-accrual leg over a set of observations.
   *
   * Counts the fraction of observations with a rate in the inclusive interval
   * `[lower, upper]` and scales by the period day-count fraction:
   *
   * `accrued = coupon_rate * day_count_fraction * (#in-range / #observations)`.
   *
   * The call provision is not applied here.
   * @returns Accrued coupon as a decimal fraction of notional for the observation period.
   * @param lower - Inclusive lower bound of the observed-rate range, in decimal form.
   * @param upper - Inclusive upper bound of the observed-rate range, in decimal form.
   * @param observations - Observed floating rates in decimal form for the accrual period.
   * @param couponRate - Contractual coupon rate in decimal form before range weighting.
   * @param dayCountFraction - Accrual year fraction for the coupon period.
   * @throws Error - Throws a JavaScript exception if the range bounds are non-finite or not strictly ordered; `observations` is empty or contains a non-finite value; or `coupon_rate` or `day_count_fraction` is negative or non-finite.
   */
  callableRangeAccrualAccrued(
    lower: number,
    upper: number,
    observations: number[],
    couponRate: number,
    dayCountFraction: number
  ): number;
}

/**
 * Namespaced TypeScript entry point for valuations APIs.
 */
export declare const valuations: ValuationsNamespace;

// --- attribution -----------------------------------------------------------

/**
 * Owned JSON fragments for P&L attribution via `attributePnl`: the fields of
 * Rust `AttributionJsonInputs`, which `attributePnl` passes to
 * `AttributionSpec::from_json_inputs`. JavaScript has no keyword arguments,
 * so the inputs are bundled in this class; Python passes the same fields as
 * keyword arguments of `attribute_pnl`.
 *
 * Optional `modelParamsT0Json` and `creditFactorModelJson` attach an opening
 * model-parameter snapshot and a credit-factor model after construction.
 */
export interface AttributionJsonInputs extends WasmOwned {
  /**
   * Optional serialized opening `ModelParamsSnapshot` JSON.
   */
  modelParamsT0Json?: string | null;
  /**
   * Optional serialized `CreditFactorModel` JSON.
   */
  creditFactorModelJson?: string | null;
}

/**
 * Namespaced TypeScript entry points for attribution calculations and types.
 * @example
 * ```typescript
 * import init, { attribution } from "finstack-quant-wasm";
 * await init();
 * console.log(attribution.defaultWaterfallOrder());
 * ```
 */
export interface AttributionNamespace {
  /**
   * Parameters constructor emitted by wasm-bindgen for attribution calls.
   *
   * `configJson` may include `{ "execution_policy": "parallel" }` to opt into
   * inner Rayon when the host is not already parallelizing attribution at the
   * portfolio or batch level. Serial is the default. Set
   * `modelParamsT0Json` / `creditFactorModelJson` on the constructed object
   * to attach an opening model-parameter snapshot or credit-factor model.
   */
  AttributionJsonInputs: new (
    instrumentJson: JsonInput,
    marketT0Json: JsonInput,
    marketT1Json: JsonInput,
    asOfT0: string,
    asOfT1: string,
    methodJson: JsonInput,
    configJson?: JsonInput | null,
    fullCrossAttribution?: boolean | null
  ) => AttributionJsonInputs;
  /**
   * Run P&L attribution for a single instrument.
   *
   * Accepts an [`AttributionJsonInputs`] struct with the instrument JSON, two market
   * snapshots, dates, and a method descriptor. Returns the `PnlAttribution`
   * result as a structured object with the canonical Rust serde field names;
   * use `attributePnlJson` for the JSON wire string. `config_json` may include
   * `"execution_policy": "parallel"` to opt into inner Rayon when the host
   * is not already parallelizing attribution at a higher level. Serial is
   * the default.
   * @returns Structured `PnlAttribution` result object for the instrument.
   * @param params - Fully specified AttributionJsonInputs object containing instrument, markets, dates, and method.
   * @throws Error - Throws a `FinstackError` whose `kind` is the Rust classification (`not_found` for missing market data, `computation` for a caught panic or solver failure, otherwise `validation`). Rejects malformed instrument, market, method, or configuration JSON; invalid ISO attribution dates; instrument or market reconstruction, pricing, FX, rounding, metric, or method-specific attribution failures; a caught attribution panic; or failure to convert the result to a JavaScript value.
   */
  attributePnl(params: AttributionJsonInputs): PnlAttribution;
  /**
   * Run P&L attribution for a single instrument and return wire JSON.
   *
   * Wire twin of `attributePnl`: same inputs, validation, and panic
   * containment, returning the `PnlAttribution` as a JSON string instead of
   * a structured object.
   * @returns JSON-serialized `PnlAttribution` wire document.
   * @param params - Fully specified AttributionJsonInputs object containing instrument, markets, dates, and method.
   * @throws Error - Rejects the same conditions as [`attribute_pnl`], plus failure to serialize the result to JSON.
   */
  attributePnlJson(params: AttributionJsonInputs): string;
  /**
   * Run attribution from a full `AttributionEnvelope` and return the result envelope.
   *
   * Returns the Rust `AttributionResultEnvelope` as a plain object:
   * `{ schema: "finstack_quant.attribution/1", result: { attribution, results_meta } }`.
   * Use `attributePnlEnvelopeJson` for the JSON wire string.
   * @returns The `AttributionResultEnvelope` as a plain object.
   * @param specJson - JSON-serialized AttributionEnvelope (schema `finstack_quant.attribution/1`) to validate and execute.
   * @throws Error - Rejects malformed, schema-incompatible, or unsupported-version `spec_json`; instrument or market reconstruction, pricing, FX, rounding, metric, or method-specific attribution failures; a caught execution panic; or failure to convert the result envelope to a JavaScript value.
   */
  attributePnlEnvelope(specJson: JsonInput): AttributionResultEnvelope;
  /**
   * Run attribution from a full JSON `AttributionEnvelope` and return JSON.
   *
   * Wire twin of `attributePnlEnvelope` for full envelope round-trip
   * workflows.
   * @returns JSON attribution result envelope for the supplied spec.
   * @param specJson - JSON-serialized AttributionEnvelope (schema `finstack_quant.attribution/1`) to validate and execute.
   * @throws Error - Rejects the same conditions as [`attribute_pnl_envelope`], plus failure to serialize the result envelope.
   */
  attributePnlEnvelopeJson(specJson: JsonInput): string;
  /**
   * Validate an attribution specification JSON.
   *
   * Deserializes against the `AttributionEnvelope` schema, checks the
   * `schema` version tag (the same gate `execute` applies, so a payload that
   * validates here cannot later be rejected at execution), and returns the
   * canonical JSON.
   * @returns Canonical attribution-envelope JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed, schema-incompatible, or unsupported-version `json`, or failure to serialize the canonical attribution envelope.
   */
  validateAttributionJson(json: JsonInput): string;
  /**
   * Return the default waterfall factor ordering as canonical snake-case values.
   * @returns Default waterfall factor names in execution order.
   * @throws Error - Rejects if the default factor identifiers cannot be serialized to JavaScript.
   */
  defaultWaterfallOrder(): string[];
  /**
   * Return the default metric IDs used by metrics-based attribution.
   * @returns Default metric identifiers used by metrics-based attribution.
   * @throws Error - Rejects if the default metric identifiers cannot be serialized to JavaScript.
   */
  defaultAttributionMetrics(): string[];
  /**
   * Headline P&L bridge: `value(T1) - value(T0)` in one currency (mirrors Python `pnl_bridge`).
   * @param instrumentJson - Canonical `finstack_quant.instrument/1` envelope (object or JSON).
   * @param marketT0Json - Canonical MarketContext at the opening date.
   * @param marketT1Json - Canonical MarketContext at the closing date.
   * @param asOfT0 - Opening valuation date as an ISO-8601 string.
   * @param asOfT1 - Closing valuation date as an ISO-8601 string.
   * @param targetCurrency - ISO-4217 currency of the returned P&L.
   * @returns The P&L as a `Money` handle in `targetCurrency`.
   * @throws Error - Throws with kind `validation` if an input is malformed, kind `not_found` if a curve, market item or FX leg is missing, and kind `computation` if pricing fails.
   */
  pnlBridge(
    instrumentJson: JsonInput,
    marketT0Json: JsonInput,
    marketT1Json: JsonInput,
    asOfT0: string,
    asOfT1: string,
    targetCurrency: string
  ): Money;
  /**
   * Run one attribution configuration against many instruments (mirrors Python `attribute_pnl_many`).
   * @param params - AttributionJsonInputs carrying the shared markets, dates, method and configuration.
   * @param instruments - Array of canonical instrument envelopes (objects or JSON), in output order.
   * @returns One `PnlAttribution` object per instrument, in input order.
   * @throws Error - Throws a `FinstackError` with the Rust classification for the first failing instrument (see `attributePnl`), or kind `validation` if an instrument envelope is malformed.
   */
  attributePnlMany(params: AttributionJsonInputs, instruments: JsonInput[]): PnlAttribution[];
  /**
   * Compute return-contribution attribution from a specification (mirrors Python `attribute_return_contribution`).
   * @param spec - `ReturnContributionSpec` (object or JSON): positions with weights and returns, weighting scheme and optional benchmark.
   * @returns Plain `ReturnContributionResult` object.
   * @throws Error - Throws with kind `validation` if the spec is malformed or violates the weighting/benchmark invariants (including a Brinson group with zero net weight but nonzero contribution).
   */
  attributeReturnContribution(spec: JsonInput): ReturnContributionResult;
  /**
   * Compute return-contribution attribution and return wire JSON (wire twin of `attributeReturnContribution`).
   * @param specJson - `ReturnContributionSpec` (object or JSON).
   * @returns Canonical `ReturnContributionResult` JSON text.
   * @throws Error - Throws with kind `validation` if the spec is malformed or violates the weighting/benchmark invariants.
   */
  attributeReturnContributionJson(specJson: JsonInput): string;
  /**
   * Validate a return-contribution specification and return its canonical JSON (mirrors Python `validate_return_contribution_json`).
   * @param specJson - `ReturnContributionSpec` (object or JSON).
   * @returns Canonical compact spec JSON.
   * @throws Error - Throws with kind `validation` if the spec is malformed or violates the weighting/benchmark invariants.
   */
  validateReturnContributionJson(specJson: JsonInput): string;
  /**
   * Human-readable tree explanation of an attribution, non-zero factors only (twin of Python `PnlAttribution.explain`).
   * @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
   * @returns Multi-line tree text.
   * @throws Error - Throws with kind `validation` if `pnl` is not a `PnlAttribution`.
   */
  pnlAttributionExplainText(pnl: PnlAttribution | string): string;
  /**
   * Verbose tree explanation of an attribution, including zero-valued factors (twin of Python `PnlAttribution.explain_verbose`).
   * @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
   * @returns Multi-line tree text.
   * @throws Error - Throws with kind `validation` if `pnl` is not a `PnlAttribution`.
   */
  pnlAttributionExplainVerboseText(pnl: PnlAttribution | string): string;
  /**
   * Whether the attribution residual is within tolerance (twin of Python `PnlAttribution.residual_within_tolerance`).
   * @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
   * @param pctTolerance - Optional percentage tolerance (`0.1` = 0.1%); omitted uses the run's `meta.tolerance_pct`.
   * @param absTolerance - Optional absolute tolerance in `total_pnl` currency units; omitted uses `meta.tolerance_abs`.
   * @returns `true` when the residual is within tolerance.
   * @throws Error - Throws with kind `validation` if `pnl` is not a `PnlAttribution`, and kind `invalid_type` if a tolerance is not a number.
   */
  pnlAttributionResidualWithinTolerance(
    pnl: PnlAttribution | string,
    pctTolerance?: number | null,
    absTolerance?: number | null
  ): boolean;
  /**
   * Check that every factor's currency matches the total P&L currency (twin of Python `PnlAttribution.validate_currencies`).
   * @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
   * @throws Error - Throws with kind `validation` if `pnl` is not a `PnlAttribution` or a factor is denominated in another currency.
   */
  pnlAttributionValidateCurrencies(pnl: PnlAttribution | string): void;
  /**
   * Metric identifiers the attribution's method needs pre-computed (twin of Python `PnlAttribution.required_metrics`).
   * @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
   * @returns Canonical metric identifiers.
   * @throws Error - Throws with kind `validation` if `pnl` is not a `PnlAttribution`.
   */
  pnlAttributionRequiredMetrics(pnl: PnlAttribution | string): string[];
}

/**
 * Namespaced TypeScript entry point for attribution APIs.
 */
export declare const attribution: AttributionNamespace;

// --- statements ------------------------------------------------------------

/**
 * Namespaced TypeScript entry points for statements calculations and types.
 * @example
 * ```typescript
 * import init, { statements } from "finstack-quant-wasm";
 * await init();
 * console.log(statements.parseFormula("revenue-expenses")); // "revenue - expenses"
 * ```
 */
export interface StatementsNamespace {
  /**
   * Validate a `FinancialModelSpec` JSON string.
   *
   * Deserializes the input against the model schema, runs semantic validation,
   * and returns the canonical (re-serialized) JSON.
   * @returns Canonical financial-model JSON after semantic validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, an empty or invalid period timeline, reserved node identifiers, incompatible node fields or value types, invalid formulas or dimensions, an invalid waterfall, or failure to serialize the normalized model.
   */
  validateFinancialModelJson(json: JsonInput): string;
  /**
   * Get the node identifiers from a model specification JSON.
   *
   * Returns a JS array of node ID strings in declaration order.
   * @returns Node identifiers in model-declaration order.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`; an empty or invalid period timeline, reserved node identifiers, incompatible node fields or value types, invalid formulas, or an invalid capital structure; or failure to serialize the node identifiers to JavaScript.
   */
  modelNodeIds(json: JsonInput): string[];
  /**
   * Validate a `CheckSuiteSpec` JSON string.
   *
   * Deserializes the spec, re-serializes to canonical form, and
   * returns the JSON string. Useful for client-side validation.
   * @returns Canonical check-suite JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, or failure to serialize the decoded check-suite specification.
   */
  validateCheckSuiteSpecJson(json: JsonInput): string;
  /**
   * Validate a `CapitalStructureSpec` JSON string.
   * @returns Canonical capital-structure JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`; an invalid waterfall; a bond, convertible, swap, cap/floor, or swaption instrument alongside a prepayment rung; an interest-rate swap with side `Receive`; or failure to serialize the validated capital-structure specification.
   */
  validateCapitalStructureSpecJson(json: JsonInput): string;
  /**
   * Validate a `WaterfallSpec` JSON string.
   *
   * Performs both serde deserialization and the waterfall's internal
   * consistency check (for example rejecting `Sweep` ordered after `Equity`
   * when an ECF sweep is configured).
   * @returns Canonical waterfall JSON after schema validation.
   * @param json - Canonical JSON string for a `WaterfallSpec`, including `priority_of_payments`, `available_cash_node`, optional `ecf_sweep`, `pik_toggle`, `payment_classes`, `mandatory_prepay_node`, and `voluntary_prepay_node`.
   * @throws Error - Rejects malformed or schema-incompatible `json`; duplicate or inconsistent payment priorities; incomplete available-cash priorities; invalid PIK, payment-class, prepay-node, or ECF-sweep settings; or failure to serialize the validated waterfall.
   */
  validateWaterfallSpecJson(json: JsonInput): string;
  /**
   * Validate an `EcfSweepSpec` JSON string.
   * @returns Canonical ECF-sweep JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, a `sweep_percentage` outside `[0.0, 1.0]`, or failure to serialize the validated ECF-sweep specification.
   */
  validateEcfSweepSpecJson(json: JsonInput): string;
  /**
   * Validate a `PikToggleSpec` JSON string.
   * @returns Canonical PIK-toggle JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, a missing or empty `target_instrument_ids` list, or failure to serialize the validated PIK-toggle specification.
   */
  validatePikToggleSpecJson(json: JsonInput): string;
  /**
   * Evaluate a `FinancialModelSpec` and return the `StatementResult`.
   *
   * Returns a structured JavaScript object (the Python binding returns a typed
   * `StatementResult` from the same Rust evaluator). Non-finite node values and
   * warning values use canonical strings `"nan"`, `"inf"`, and `"-inf"`, so a
   * `JSON.stringify`/parse round trip preserves missing-data semantics.
   * @returns Evaluated statement result with node values and optional audit metadata.
   * @param modelJson - JSON-serialized FinancialModelSpec to evaluate across its statement periods.
   * @throws Error - Rejects malformed `model_json`, model semantic failures, invalid formula or dependency graphs, missing evaluation inputs, unsupported capital-structure requirements, or failure to serialize the statement result to JavaScript.
   */
  evaluateModel(modelJson: JsonInput): StatementResult;
  /**
   * Evaluate a `FinancialModelSpec` against a `MarketContext` as of a given date.
   *
   * Required for capital-structure-aware models. The `as_of` argument is an
   * ISO 8601 date string (e.g. `"2025-01-15"`).
   * @returns Evaluated statement result using the supplied market as of `asOf`.
   * @param modelJson - JSON-serialized FinancialModelSpec to evaluate across its statement periods.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Rejects malformed model or market JSON, model semantic failures, an invalid ISO `as_of` date, invalid formulas or dependencies, missing market data, or failure to serialize the statement result to JavaScript.
   */
  evaluateModelWithMarket(
    modelJson: JsonInput,
    marketJson: JsonInput,
    asOf: string
  ): StatementResult;
  /**
   * Evaluate a financial model under Monte Carlo simulation.
   *
   * Takes JSON inputs and returns a structured JavaScript object; the Python
   * twin is the `Evaluator.evaluate_monte_carlo` method, which returns a typed
   * `MonteCarloResults` from the same Rust `Evaluator::evaluate_monte_carlo`.
   * @returns Structured Monte Carlo results with percentile summaries and optional path data.
   * @param modelJson - Financial-model specification JSON.
   * @param configJson - Monte Carlo configuration JSON.
   * @throws Error - Rejects malformed model or configuration JSON, model semantic failures, zero simulation paths, a model containing capital structure, model compilation or dependency failures, any path-evaluation failure, or failure to serialize the results to JavaScript.
   */
  evaluateMonteCarlo(modelJson: JsonInput, configJson: JsonInput): MonteCarloResults;
  /**
   * Export one evaluated node as a dated schedule (twin of Python `StatementResult.to_dated_schedule`).
   * @param modelJson - The `FinancialModelSpec` that produced the result (its periods supply the dates).
   * @param resultJson - The `StatementResult` returned by `evaluateModel` / `evaluateModelWithMarket` (object or JSON).
   * @param nodeId - Node identifier to export.
   * @param convention - Optional `"end"` (default: the period's last inclusive day, `end - 1 day`, since periods are half-open `[start, end)`) or `"start"`.
   * @returns `[isoDate, value]` pairs in timeline order, in the node's own units.
   * @throws Error - Throws with kind `not_found` if `nodeId` has no values in the result, and kind `validation` if an input is malformed or `convention` is not `"start"` / `"end"`.
   */
  nodeToDatedSchedule(
    modelJson: JsonInput,
    resultJson: StatementResult | string,
    nodeId: string,
    convention?: 'end' | 'start' | null
  ): [string, number][];
  /**
   * Probability that a metric exceeds a threshold in any forecast period (twin of Python `MonteCarloResults.breach_probability`).
   * @param resultsJson - `MonteCarloResults` returned by `evaluateMonteCarlo` (object or JSON).
   * @param metric - Node identifier to test.
   * @param threshold - Breach level in the metric's own units.
   * @returns Fraction of paths that breach in at least one forecast period, or `undefined` when the metric has no path data (including results run without `include_path_data`), there are no forecast periods, or the simulation is incomplete.
   * @throws Error - Throws with kind `validation` if the results input is malformed, and kind `invalid_type` if `threshold` is not a number.
   */
  monteCarloBreachProbability(
    resultsJson: MonteCarloResults | string,
    metric: string,
    threshold: number
  ): number | undefined;
  /**
   * Percentile time series of one metric across the forecast periods (twin of Python `MonteCarloResults.percentile_by_period`).
   * @param resultsJson - `MonteCarloResults` returned by `evaluateMonteCarlo` (object or JSON).
   * @param metric - Node identifier to read.
   * @param percentile - Percentile as a fraction in `[0, 1]` (e.g. `0.95`); must be one of the configured percentiles.
   * @returns Object mapping period id (e.g. `"2025Q1"`) to the percentile value in the metric's own units, or `undefined` when the metric or percentile is not in the results.
   * @throws Error - Throws with kind `validation` if the results input is malformed, and kind `invalid_type` if `percentile` is not a number.
   */
  monteCarloPercentileByPeriod(
    resultsJson: MonteCarloResults | string,
    metric: string,
    percentile: number
  ): Record<string, number> | undefined;
  /**
   * Export a statement result as a long-format table (twin of Python `StatementResult.to_arrow_long`, Rust `StatementResult::to_table_long`).
   * @param resultJson - The `StatementResult` returned by `evaluateModel` / `evaluateModelWithMarket` (object or JSON).
   * @returns `TableEnvelope` with columns `node_id`, `period_id`, `value`, `value_money`, `currency`, `value_type`; monetary nodes repeat their value in `value_money` and set `currency`, scalar nodes leave both null.
   * @throws Error - Throws with kind `validation` if the result input is malformed or table construction fails.
   */
  statementResultToTableLong(resultJson: StatementResult | string): TableEnvelope;
  /**
   * Export a statement result as a wide-format table (twin of Python `StatementResult.to_arrow_wide`, Rust `StatementResult::to_table_wide`).
   * @param resultJson - The `StatementResult` returned by `evaluateModel` / `evaluateModelWithMarket` (object or JSON).
   * @returns `TableEnvelope` with a `period_id` column followed by one value column per node; a node with no value in a period holds `NaN` (serialized as `null`), not zero.
   * @throws Error - Throws with kind `validation` if the result input is malformed or table construction fails.
   */
  statementResultToTableWide(resultJson: StatementResult | string): TableEnvelope;
  /**
   * Canonical content hash of a financial model (twin of Python `FinancialModelSpec.content_hash`).
   * @param modelJson - `FinancialModelSpec` (object or JSON).
   * @returns `"sha256:<hex>"` content hash.
   * @throws Error - Throws with kind `validation` if the model is malformed or fails semantic validation, or contains a non-finite number.
   */
  financialModelContentHash(modelJson: JsonInput): string;
  /**
   * Parse a DSL formula and return its canonical source text.
   *
   * The formula is parsed into the statements AST and rendered back through
   * the AST's `Display`: whitespace normalised, operators spaced, and
   * parentheses kept only where precedence requires them. Parsing the returned
   * text again yields the same AST, so it is a stable form for previewing,
   * diffing, or hashing formulas. Mirrors Python `parse_formula`.
   * @param formula - Financial-model formula string to parse into its canonical expression representation.
   * @returns Canonical formula text, e.g. `"(revenue - cogs) / revenue"`.
   * @throws Error - Rejects trailing tokens, malformed or incomplete syntax, or a formula that exceeds the parser's nesting or term limits.
   */
  parseFormula(formula: string): string;
  /**
   * Parse and compile a DSL formula, throwing if either step fails.
   *
   * Compilation lowers the AST onto the core expression engine and rejects
   * unsupported functions, wrong arities, and malformed capital-structure
   * references that a bare parse would accept. Returns `undefined` when the
   * formula is valid; an invalid formula throws a `FinstackError`, so
   * `if (parseAndCompile(f))` is not a validity check. Mirrors Python
   * `parse_and_compile`.
   * @returns nothing; failure is reported by throwing.
   * @param formula - Financial-model formula string to parse and validate without evaluation.
   * @throws Error - Rejects any formula that cannot be parsed as one complete DSL expression or compiled because it contains an unsupported component, function, or operator form.
   */
  parseAndCompile(formula: string): void;
}

/**
 * Namespaced TypeScript entry point for statements APIs.
 */
export declare const statements: StatementsNamespace;

// --- statements_analytics -------------------------------------------------

/**
 * Namespaced TypeScript entry points for statements analytics calculations and types.
 * @example
 * ```typescript
 * import init, { statements_analytics } from "finstack-quant-wasm";
 * await init();
 * console.log(statements_analytics.wacc(0.6, 0.1, 0.4, 0.05, 0.25));
 * ```
 */
export interface StatementsAnalyticsNamespace {
  /**
   * Run a sensitivity analysis on a financial model.
   *
   * Accepts JSON strings for the model spec and sensitivity configuration,
   * evaluates all perturbation scenarios, and returns the `SensitivityResult`
   * as a structured JavaScript object. `generateTornadoEntries` still takes the
   * result as JSON, so pass `JSON.stringify(result)` when chaining the two.
   * @returns Sensitivity results for each perturbation scenario.
   * @param modelJson - Financial-model specification JSON.
   * @param configJson - Configuration JSON for this call.
   * @throws Error - Rejects malformed model or configuration JSON, invalid sensitivity modes or parameter perturbations, missing model nodes or periods, model-evaluation failures, or failure to serialize the sensitivity result to JavaScript.
   */
  runSensitivity(modelJson: JsonInput, configJson: JsonInput): SensitivityResult;
  /**
   * Run a variance analysis comparing two evaluated statement results.
   *
   * Returns the variance report as a structured JavaScript object.
   * @returns Variance report comparing the two evaluated statement results.
   * @param baseJson - Base statement-result JSON.
   * @param comparisonJson - Comparison statement-result JSON.
   * @param configJson - Configuration JSON for this call.
   * @throws Error - Rejects malformed result or configuration JSON, empty metric or period selections, mismatched metric types or currencies, a requested value missing from either result, or failure to serialize the variance report to JavaScript.
   */
  runVariance(
    baseJson: JsonInput,
    comparisonJson: JsonInput,
    configJson: JsonInput
  ): VarianceReport;
  /**
   * Evaluate all scenarios in a scenario set against a base model.
   *
   * Returns a structured JavaScript object mapping scenario names to their
   * statement results.
   * @returns Statement results keyed by scenario name.
   * @param modelJson - Financial-model specification JSON.
   * @param scenarioSetJson - Scenario-set JSON keyed by scenario name.
   * @throws Error - Rejects malformed model or scenario-set JSON, an empty scenario set, invalid parent chains, overrides of missing nodes, failure to evaluate any scenario, or failure to serialize the result map to JavaScript.
   */
  evaluateScenarioSet(modelJson: JsonInput, scenarioSetJson: JsonInput): ScenarioResults;
  /**
   * Scenario comparison table with a `<scenario>_vs_<baseline>_frac` column per non-baseline scenario (twin of Python `ScenarioResults.to_comparison_table`). The baseline is `"base"` when present, otherwise the first scenario.
   * @param scenarioResultsJson - The scenario map returned by `evaluateScenarioSet` (object or JSON).
   * @param metrics - Node identifiers to include, in row order within each period.
   * @returns `TableEnvelope` with `period` and `metric` columns, one value column per scenario and the `_frac` change columns.
   * @throws Error - Throws with kind `validation` if the results input is malformed, the result set or metric list is empty, or table construction fails.
   */
  scenarioComparisonTable(scenarioResultsJson: ScenarioResults | string, metrics: string[]): TableEnvelope;
  /**
   * Sensitivity parameter whose perturbations are percentage moves of a base value (twin of Python `ParameterSpec.with_percentages`).
   * @param nodeId - Node identifier to vary.
   * @param periodId - Period to vary, e.g. `"2025Q1"`.
   * @param baseValue - Base value of the node in its own units.
   * @param pctRange - Percentage moves, e.g. `[-10, 0, 10]` for ±10%.
   * @returns Plain `ParameterSpec` object (`node_id`, `period_id`, `base_value`, absolute `perturbations`) for a `SensitivityConfig`.
   * @throws Error - Throws with kind `validation` if `periodId` is not a valid period, and kind `invalid_type` if a number argument is not a number.
   */
  parameterSpecWithPercentages(
    nodeId: string,
    periodId: string,
    baseValue: number,
    pctRange: number[] | Float64Array
  ): generated.statements_analytics.ParameterSpec;
  /**
   * Compute forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).
   *
   * Takes two float arrays (actual, forecast) and returns the serde form of
   * the Rust `ForecastMetrics` (`mae`, `mape`, `mape_effective_n`, `smape`,
   * `rmse`, `n`). A non-finite metric (for example `mape` when every actual is
   * zero) is the JavaScript number `NaN` or `±Infinity`; the Rust JSON form
   * writes it as the string `"nan"`, `"inf"` or `"-inf"`.
   * @returns Backtest forecast accuracy metrics for the selected series.
   * @param actual - Actual realized values aligned one-for-one with the forecast series.
   * @param forecast - Forecast values aligned one-for-one with the actual realized series.
   * @throws Error - Rejects inputs that cannot be decoded as numeric JavaScript arrays, arrays with unequal lengths, empty arrays, or metrics that cannot be serialized to JavaScript.
   */
  backtestForecast(actual: number[], forecast: number[]): ForecastMetrics;
  /**
   * Generate tornado chart entries for a sensitivity result.
   * @param resultJson - Result JSON produced by a prior call.
   * @param metricNode - Statement metric node identifier selected for the requested analysis.
   * @param period - Model period label for the requested statement value or calculation.
   * @returns Structured tornado entries sorted by descending absolute swing.
   * @throws Error - Rejects malformed `result_json`, an invalid optional `period` identifier, or failure to convert the entries to JavaScript. A missing metric produces no entry rather than rejecting.
   */
  generateTornadoEntries(
    resultJson: JsonInput,
    metricNode: string,
    period?: string | null
  ): TornadoEntry[];
  /**
   * Find the driver value that makes a target node reach a target value.
   *
   * Returns the serde form of the Rust `GoalSeekResult`: `solved_value` plus
   * `model`, the input model with the solved driver written in when
   * `update_model` is `true` and `null` otherwise. The input model is never
   * modified; pass `model` straight back as the `modelJson` of another call.
   * @returns Solved driver value and the updated model (or `null`).
   * @param modelJson - Financial-model specification JSON.
   * @param targetNode - Statement node identifier whose value is driven toward the target.
   * @param targetPeriod - Model period label in which the goal-seek target is evaluated.
   * @param targetValue - Numeric target value the goal-seek routine attempts to reach.
   * @param driverNode - Statement node identifier adjusted by the goal-seek routine.
   * @param driverPeriod - Model period label of the adjustable goal-seek driver.
   * @param updateModel - Whether the result carries the model with the solved driver value applied.
   * @param bounds - Optional `[lower, upper]` search bracket for the driver, in the driver node's units.
   * @throws Error - Rejects malformed `model_json`, invalid target or driver period identifiers, `bounds` that is not a two-number array, missing target or driver nodes or periods, non-finite or unordered bounds, model-evaluation or solver-convergence failures, or failure to serialize the result.
   */
  goalSeek(
    modelJson: JsonInput,
    targetNode: string,
    targetPeriod: string,
    targetValue: number,
    driverNode: string,
    driverPeriod: string,
    updateModel: boolean,
    bounds?: [number, number] | null
  ): GoalSeekResult;
  /**
   * Rank the headline DCF assumptions by enterprise-value impact.
   *
   * The statement model is evaluated once; each shocked point re-runs only the
   * DCF. Returns a structured JavaScript object with the baseline enterprise
   * value, tornado entries as deltas versus that baseline sorted by descending
   * absolute swing, and the effective (possibly clamped) shock levels.
   * @returns Ranked DCF-assumption impacts on enterprise value.
   * @param modelJson - Financial-model specification JSON.
   * @param wacc - Baseline weighted average cost of capital in decimal form (0.10 = 10%).
   * @param terminalValueJson - Terminal-value spec JSON selecting whether growth or the exit multiple is shocked.
   * @param ufcfNode - Node identifier holding unlevered free cash flow for the forecast periods; omitted uses the canonical "ufcf" node.
   * @param netDebtOverride - Optional net debt in model currency; otherwise requires debt and cash in that currency from a period ending on or before valuation.
   * @param optionsJson - Optional Rust `DcfOptions` JSON; every field is optional and a missing one takes its default: `mid_year_convention` (false), `wacc_sensitivity_bump` (0.01 = +/-100 bp), `wacc_denominator_epsilon` (0.005), `max_stable_growth_rate` (0.05), `exit_multiple_bump` (`{"absolute": 1.0}` turns or `{"relative": 0.10}`), `exit_multiple_metric_node` (flow node whose complete trailing year supplies the exit-multiple metric), `equity_bridge`, `shares_outstanding`, `valuation_discounts`, `discount_curve_id`. Unknown keys are rejected.
   * @param marketJson - Optional canonical market-context JSON used for statement evaluation, not WACC discounting.
   * @throws Error - Rejects malformed model, terminal-value, options, or market JSON (an unknown options key included), model-evaluation failures, a missing UFCF series or model currency, inconsistent WACC or terminal-value assumptions, a missing or incomplete exit-multiple metric node, missing bridge inputs, valuation failures, or failure to serialize the sensitivity result.
   */
  dcfSensitivity(
    modelJson: JsonInput,
    wacc: number,
    terminalValueJson: JsonInput,
    ufcfNode?: string | null,
    netDebtOverride?: number | null,
    optionsJson?: JsonInput | null,
    marketJson?: JsonInput | null
  ): DcfSensitivityResult;
  /**
   * Evaluate a leveraged-buyout transaction against a statement model.
   *
   * Entry enterprise value is priced at the model's first period, the sponsor
   * equity check is solved as the sources-and-uses residual, and exit proceeds
   * are the exit enterprise value less the modelled net debt at the exit
   * period. When the config carries `check_mappings`, the Rust LBO check suite
   * runs against the same evaluation and fills `checks`; otherwise `checks` is
   * `null`. IRR is out of scope: pair the returned `exit_equity_proceeds` with
   * the equity outflow at close and call `portfolio.mwrXirr`.
   * @returns Leveraged-buyout evaluation result against the statement model.
   * @param modelJson - Financial-model specification JSON.
   * @param configJson - Rust `LboConfig` JSON: `entry_multiple` (8.5 = 8.5x), `entry_metric_node`, `transaction_fees` (model currency), `sources` (`[{"name", "amount"}]` funded at close, model currency), `exit_multiple`, `exit_metric_node`, `exit_net_debt_node`, `exit_period` (e.g. "2029"), and optional `check_mappings` (`{"three_statement", "credit"}`). Every field except `check_mappings` is required.
   * @throws Error - Rejects malformed model or config JSON (unknown config or mapping keys included), an invalid `exit_period`, model evaluation or lookup failures, a missing model currency or period, non-finite transaction inputs or model values, negative tranche amounts, a non-positive sponsor equity check, check-suite failures, or failure to serialize the result to JavaScript. The result is a structured JavaScript object.
   */
  evaluateLbo(modelJson: JsonInput, configJson: JsonInput): LboResult;
  /**
   * Weighted-average cost of capital (WACC).
   *
   * Blends the required return on equity with the after-tax cost of debt:
   * `WACC = w_E * r_E + w_D * r_D * (1 - T)`.
   * @returns Returns the blended discount rate as a decimal fraction.
   * @param equityWeight - Equity share of total capital as a decimal fraction (0.6 = 60% equity-funded).
   * @param costOfEquity - Required return on equity in decimal form, typically from CAPM (0.115 = 11.5%).
   * @param debtWeight - Debt share of total capital as a decimal fraction; must sum with the equity weight to 1.0.
   * @param costOfDebt - Pre-tax marginal borrowing yield in decimal form, before the interest tax shield (0.06 = 6%).
   * @param taxRate - Marginal corporate tax rate as a decimal fraction in [0, 1] (0.25 = 25%).
   * @throws Error - Rejects any non-finite input, negative capital weights, weights that do not sum to one within tolerance, or a `tax_rate` outside `[0, 1]`.
   */
  wacc(
    equityWeight: number,
    costOfEquity: number,
    debtWeight: number,
    costOfDebt: number,
    taxRate: number
  ): number;
  /**
   * Build a node's dependency tree.
   *
   * Returns the serde form of the Rust `DependencyTree`: `node_id`, `formula`
   * (the node's formula text, or `null` for a value node) and `children`, one
   * tree per direct dependency. A dependency already on the current path
   * appears once more as a leaf named `"<id> (cycle)"`. Twin of Python
   * `DependencyTracer.dependency_tree`.
   * @returns Dependency tree rooted at the selected node.
   * @param modelJson - Financial-model specification JSON.
   * @param nodeId - Root node whose dependencies are traced.
   * @throws Error - Rejects malformed `model_json`, a model that fails semantic validation, formulas or clauses whose dependencies cannot be parsed, unknown formula references, a missing `node_id` or reachable dependency, or a dependency cycle.
   */
  dependencyTree(modelJson: JsonInput, nodeId: string): DependencyTree;
  /**
   * Render a node's dependency tree as ASCII text.
   *
   * The root on the first line, then one line per dependency drawn with
   * `├──` / `└──` connectors and indented by depth, each followed by its
   * formula in parentheses. Twin of Python
   * `DependencyTracer.dependency_tree_text` (Rust
   * `DependencyTracer::dependency_tree_text`).
   * @returns ASCII dependency tree, one node per line.
   * @param modelJson - Financial-model specification JSON.
   * @param nodeId - Root node whose dependencies are traced.
   * @throws Error - Rejects malformed `model_json`, a model that fails semantic validation, formulas or clauses whose dependencies cannot be parsed, unknown formula references, a missing `node_id` or reachable dependency, or a dependency cycle.
   */
  dependencyTreeText(modelJson: JsonInput, nodeId: string): string;
  /**
   * Explain a formula for a specific node and period (JSON in, structured
   * object out).
   *
   * Returns the Rust `Explanation` as a structured object; a non-finite
   * `final_value` or breakdown `value` (for example a `lag` node at the first
   * period) is the JavaScript number `NaN` or `±Infinity`. The Rust JSON form
   * writes it as the string `"nan"`, `"inf"` or `"-inf"`.
   * @returns Structured formula breakdown for the selected node and period.
   * @param modelJson - Financial-model specification JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param nodeId - Stable node identifier used to select the required domain object.
   * @param period - Model period label for the requested statement value or calculation.
   * @throws Error - Rejects malformed model or result JSON, an invalid `period` identifier, a missing model node or node-period result, an invalid formula used to build the breakdown, or failure to serialize the explanation to JavaScript.
   */
  explainFormula(
    modelJson: JsonInput,
    resultsJson: JsonInput,
    nodeId: string,
    period: string
  ): Explanation;
  /**
   * Explain a formula for a specific node and period as formatted text.
   * @returns Formatted formula explanation for the selected node and period.
   * @param modelJson - Financial-model specification JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param nodeId - Stable node identifier used to select the required domain object.
   * @param period - Model period label for the requested statement value or calculation.
   * @throws Error - Rejects malformed model or result JSON, an invalid `period` identifier, a missing model node or node-period result, or an invalid formula used to build the explanation breakdown.
   */
  explainFormulaText(
    modelJson: JsonInput,
    resultsJson: JsonInput,
    nodeId: string,
    period: string
  ): string;
  /**
   * Generate a P&L summary report as formatted text.
   * @returns Returns a human-readable text report, not JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param lineItems - Ordered statement line-item definitions included in the summary report.
   * @param periods - Ordered period labels or observations aligned with the supplied data.
   * @throws Error - Rejects malformed `results_json`, `line_items` or `periods` values that are not JavaScript string arrays, or any period string that is not a valid statement period identifier.
   */
  plSummaryReportText(resultsJson: JsonInput, lineItems: string[], periods: string[]): string;
  /**
   * Generate a credit assessment report as formatted text.
   * @returns Returns a human-readable text report, not JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param period - Statement period identifier, such as `2025Q4` or `2025A`.
   * @throws Error - Rejects malformed `results_json` or an `period` value that is not a valid statement period identifier.
   */
  creditAssessmentReportText(resultsJson: JsonInput, period: string): string;
  /**
   * Compute a structured credit assessment (leverage, coverage, FCF).
   *
   * Returns a structured JavaScript object.
   * @returns Credit-assessment result object from the statement results.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param period - Statement period identifier, such as `2025Q4` or `2025A`.
   * @throws Error - Rejects malformed `results_json`, an `period` value that is not a valid statement period identifier, or failure to serialize the assessment to JavaScript.
   */
  creditAssessment(resultsJson: JsonInput, period: string): CreditAssessment;
  /**
   * Run checks from a suite spec against a model.
   *
   * Evaluates the model only when results are absent, then runs built-in and
   * formula checks against the canonical statement results.
   * @param modelJson - Financial-model specification JSON.
   * @param suiteSpecJson - Check-suite specification JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @returns Structured check report with individual results and aggregate summary.
   * @throws Error - Rejects malformed model, suite, or supplied result JSON; check-suite resolution failures; model-evaluation failures when results are omitted; missing nodes, incompatible data, or invalid check configuration during execution; or failure to convert the report to JavaScript.
   */
  runChecks(
    modelJson: JsonInput,
    suiteSpecJson: JsonInput,
    resultsJson?: JsonInput | null
  ): CheckReport;
  /**
   * Run three-statement checks using node mappings.
   *
   * Accepts a model and mapping JSON, builds the appropriate suite, and
   * evaluates the model only when results are absent.
   * @param modelJson - Financial-model specification JSON.
   * @param mappingJson - Node-mapping JSON from statement nodes to check inputs.
   * @param resultsJson - Evaluated statement-result JSON.
   * @returns Structured three-statement check report with results and aggregate summary.
   * @throws Error - Rejects malformed model, mapping, or supplied result JSON; model-evaluation failures when results are omitted; missing mapped nodes, incompatible data, or invalid check configuration; or failure to convert the report to JavaScript.
   */
  runThreeStatementChecks(
    modelJson: JsonInput,
    mappingJson: JsonInput,
    resultsJson?: JsonInput | null
  ): CheckReport;
  /**
   * Run credit underwriting checks using credit-specific mappings.
   * @param modelJson - Financial-model specification JSON.
   * @param mappingJson - Node-mapping JSON from statement nodes to check inputs.
   * @param resultsJson - Evaluated statement-result JSON.
   * @returns Structured credit-underwriting check report with results and aggregate summary.
   * @throws Error - Rejects malformed model, mapping, or supplied result JSON; model-evaluation failures when results are omitted; missing mapped nodes, incompatible data, or invalid check configuration; or failure to convert the report to JavaScript.
   */
  runCreditUnderwritingChecks(
    modelJson: JsonInput,
    mappingJson: JsonInput,
    resultsJson?: JsonInput | null
  ): CheckReport;
  /**
   * Render a check report as plain text.
   * @returns Plain-text check report.
   * @param reportJson - Check-report JSON.
   * @throws Error - Rejects `report_json` when it is malformed or incompatible with the check report schema.
   */
  renderCheckReportText(reportJson: JsonInput): string;
  /**
   * Render a check report as HTML.
   * @returns HTML check report.
   * @param reportJson - Check-report JSON.
   * @throws Error - Rejects `report_json` when it is malformed or incompatible with the check report schema.
   */
  renderCheckReportHtml(reportJson: JsonInput): string;
  // Comps — comparable company analysis
  /**
   * Percentile rank of `value` within `values` on a 0-1 scale (Rust `percentile_rank(values, value)`).
   *
   * Returns `undefined` when `values` is empty rather than a synthetic 0.5.
   * @returns Percentile rank in `[0, 1]`, or `undefined` when `values` is empty.
   * @param values - Peer observations forming the comparison universe; non-finite entries are ignored.
   * @param value - Subject-company metric value to rank against the peer sample.
   * @throws Error - Rejects when `values` is not a numeric JavaScript array or the finite rank cannot be serialized. Empty/non-finite peer data or a non-finite `value` return `undefined` rather than rejecting.
   */
  percentileRank(values: number[], value: number): number | undefined;
  /**
   * Z-score of `value` within `values` (Rust `z_score(values, value)`).
   *
   * Returns `undefined` when fewer than two observations are provided or the
   * peer variance is zero, instead of a synthetic zero.
   * @returns Standardized z-score, or `undefined` when variance is zero or the sample is too small.
   * @param values - Peer observations the subject is standardized against; non-finite entries are ignored.
   * @param value - Subject-company metric value to standardize against the peer sample.
   * @throws Error - Rejects when `values` is not a numeric JavaScript array or the computed score cannot be serialized. Insufficient data, zero variance, or a non-finite `value` return `undefined` rather than rejecting.
   */
  zScore(values: number[], value: number): number | undefined;
  /**
   * Descriptive statistics over a peer distribution (Rust `peer_stats(values)`).
   *
   * Returns `undefined` (matching the other comps helpers) when `values` is
   * empty.
   * @returns Descriptive peer statistics, or `undefined` when `values` is empty.
   * @param values - Peer metric observations; non-finite entries are ignored.
   * @throws Error - Rejects when `values` is not a numeric JavaScript array or the statistics cannot be serialized. No finite observations return `undefined`.
   */
  peerStats(values: number[]): PeerStats | undefined;
  /**
   * Single-factor OLS fit of `y_values` on `x_values` evaluated at the subject
   * observation (Rust `regression_fair_value(x_values, y_values, subject_x, subject_y)`).
   * @returns Fitted intercept, slope, R², subject fitted value and residual, or `undefined` if unidentifiable.
   * @param xValues - Comparable-company independent-variable values aligned with y_values.
   * @param yValues - Comparable-company dependent-variable values aligned with x_values.
   * @param subjectX - Subject company's independent-variable value for the fitted regression.
   * @param subjectY - Subject company's observed dependent-variable value for relative-value comparison.
   * @throws Error - Rejects when `x_values` or `y_values` is not a numeric JavaScript array, or the regression result cannot be serialized. Fewer than three paired values or an unidentifiable fit returns `undefined`, as do unequal lengths and non-finite numeric inputs or outputs.
   */
  regressionFairValue(
    xValues: number[],
    yValues: number[],
    subjectX: number,
    subjectY: number
  ): RegressionResult | undefined;
  /**
   * Compute a canonical valuation multiple for a company-metric bag.
   * @returns The requested multiple, or `undefined` when inputs are missing or the denominator is not positive.
   * @param companyMetrics - Flat snake_case metric object (`enterprise_value`, `ebitda`, ...) supplying numerator and denominator inputs; a `null` value means the metric is missing.
   * @param multiple - Supported valuation multiple identifier, such as EV/EBITDA or P/E.
   * @throws Error - Rejects when `company_metrics` is not an object of numbers (or `null`), `multiple` is not a supported canonical identifier, or the computed value cannot be serialized. Missing, `null` or non-finite inputs and non-positive denominators return `undefined`.
   */
  computeMultiple(
    companyMetrics: Record<string, number | null | undefined> | string,
    multiple: string
  ): number | undefined;
  /**
   * Composite rich/cheap scoring across multiple dimensions.
   * @returns Composite rich/cheap score with per-dimension diagnostics.
   * @param peerSet - Comparable-company metric records used to score relative value.
   * @param dimensions - Metric dimensions and weights; each has one optional `x_extractor` for single-factor regression, or null for distribution scoring.
   * @throws Error - Rejects when `peer_set` or `dimensions` cannot be decoded into its declared schema, when no scoring dimensions are supplied, or when the result cannot be serialized to JavaScript.
   */
  scoreRelativeValue(peerSet: unknown, dimensions: unknown[]): RelativeValueResult;
}

/**
 * Namespaced TypeScript entry point for statements analytics APIs.
 */
export declare const statements_analytics: StatementsAnalyticsNamespace;

// --- portfolio -------------------------------------------------------------

/**
 * Browser-native materialization input accepted without Node.js APIs.
 */
export type MaterializationBundleInput = string | Uint8Array;

/**
 * Error kind carried by every structured error the package throws.
 *
 * Mirrors Rust `finstack_quant_core::error::ErrorKind` (`not_found`,
 * `validation`, `computation`), which the Python binding maps to `KeyError`,
 * `ValueError` and `RuntimeError`. `invalid_type` marks a JavaScript argument
 * of the wrong type, thrown as a `TypeError` (Python raises `TypeError`).
 */
export type FinstackErrorKind = 'not_found' | 'validation' | 'computation' | 'invalid_type';

/**
 * Structured error thrown by the namespace functions and handle methods.
 *
 * `name` is `'FinstackError'`, or `'TypeError'` for an argument of the wrong
 * JavaScript type (`kind` `'invalid_type'`). `kind` is decided by the Rust
 * error type, never by the message text. `code` refines `kind` where the Rust
 * error defines one (portfolio contract failures: `'report'`,
 * `'limit_exceeded'`).
 */
export interface FinstackError extends Error {
  /**
   * `'FinstackError'`, or `'TypeError'` for a wrong-type argument.
   */
  name: 'FinstackError' | 'TypeError';
  /**
   * Rust-owned error kind.
   */
  kind: FinstackErrorKind;
  /**
   * Stable machine-readable refinement of `kind`, when the Rust error defines one.
   */
  code?: string;
}

/**
 * Structured error thrown by `calibration.calibrate`, `validateCalibrationJson`,
 * `dryRun` and `dryRunJson`.
 *
 * `kind` and `step_id` are independent: never use a step id as the category.
 */
export interface CalibrationEnvelopeError extends Error {
  /**
   * Stable public error class name.
   */
  name: 'CalibrationEnvelopeError';
  /**
   * Rust-owned execution category, such as `'strict_load'` or `'solver_not_converged'`.
   */
  kind: string;
  /**
   * Execution stage that failed.
   */
  stage: 'ingestion' | 'configuration' | 'context' | 'preflight' | 'target' | 'solver';
  /**
   * Offending step id, or `undefined` for a plan-wide failure.
   */
  step_id?: string;
  /**
   * Structured solver fit diagnostics, or `undefined` when unavailable.
   */
  solver_diagnostics?: Record<string, unknown>;
  /**
   * Strict-load diagnostics; empty unless ingestion rejected the document.
   */
  diagnostics: StrictLoadDiagnostic[];
  /**
   * JSON-serialized stable execution-error payload.
   */
  details: string;
  /**
   * The same stable execution-error payload as a structured object.
   */
  cause: unknown;
}

/**
 * Typed error thrown when a persisted contract cannot be loaded.
 *
 * A `FinstackError`-shaped error with a distinct `name`, `kind` `'validation'`
 * and a `code`.
 */
export interface ContractValidationError extends Error {
  /**
   * Stable public error class name.
   */
  name: 'ContractValidationError';
  /**
   * Rust error kind; a contract that cannot be loaded is always a validation failure.
   */
  kind: 'validation';
  /**
   * Rust error code: `"report"` when structured diagnostics are attached,
   * `"limit_exceeded"` when a resource limit was breached.
   */
  code: 'report' | 'limit_exceeded';
  /**
   * Structured diagnostics when `code` is `"report"`.
   */
  report?: ValidationReport;
}

/**
 * Successful strict materialization result.
 */
export interface PortfolioMaterializationResult {
  /**
   * Reusable WebAssembly portfolio handle.
   */
  portfolio: Portfolio;
  /**
   * Structured counts, diagnostics, cache hits, and phase timings.
   */
  report: MaterializationReport;
}

/**
 * Reusable bounded cache of decoded content-addressed instruments.
 *
 * @example
 * ```typescript
 * const cache = new portfolio.InstrumentArtifactCache(5_000);
 * console.log(cache.size);
 * cache.free();
 * ```
 */
declare class InstrumentArtifactCache {
  /**
   * Create an empty cache with explicit bounds.
   * @param capacity - Maximum retained artifacts. Omit, `null`, or `undefined` to use the native default of 4,096.
   * @returns A reusable cache with a 64 MiB encoded-source byte bound.
   */
  constructor(capacity?: number);
  /**
   * Number of decoded artifacts currently retained.
   * @returns A non-negative entry count.
   */
  readonly size: number;
  /**
   * Cumulative number of successful cache-miss decodes.
   * @returns A non-negative decode count for this cache instance.
   */
  readonly decodeCount: number;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle afterward.
   */
  free(): void;
}

/**
 * Fluent builder returned by `Portfolio.builder(id, baseCurrency, asOf)` (Rust `PortfolioBuilder`).
 *
 * Every setter consumes the handle it is called on and returns the builder,
 * so chain the calls or reassign the returned handle; `build()` validates the
 * portfolio and returns a reusable `Portfolio`.
 *
 * @example
 * ```typescript
 * const book = portfolio.Portfolio.builder("book", "USD", "2025-01-01")
 *   .name("Desk book")
 *   .entity({ id: "ACME", name: null })
 *   .tag("desk", "rates")
 *   .build();
 * console.log(book.entityIds);
 * book.free();
 * ```
 */
declare class PortfolioBuilder {
  private constructor();
  /**
   * Set the human-readable portfolio name.
   * @param name - Display name stored alongside the portfolio identifier.
   * @returns The builder, for chaining.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `name` is not a string.
   */
  name(name: string): PortfolioBuilder;
  /**
   * Register an entity that positions can reference.
   * @param entity - `Entity` object or JSON: `{ id, name, tags?, meta? }` (`name` may be `null`).
   * @returns The builder, for chaining.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `entity` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if it does not match the `Entity` schema.
   */
  entity(entity: Entity | string): PortfolioBuilder;
  /**
   * Add a position from its serializable specification.
   * @param position - `PositionSpec` object or JSON: `position_id`, `entity_id`, `instrument_id`, `instrument_spec` (the tagged instrument JSON), signed `quantity`, `unit`, and optional `book_id`, `attributes` and `meta`.
   * @returns The builder, for chaining.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `position` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if it does not match the `PositionSpec` schema, carries no `instrument_spec`, the instrument cannot be built, or `quantity` is not finite.
   */
  position(position: PositionSpec | string): PortfolioBuilder;
  /**
   * Attach a portfolio-level tag.
   * @param key - Tag name used for grouping and filtering, such as `desk`; a repeated key replaces the earlier value.
   * @param value - Text stored under `key`, such as `rates`.
   * @returns The builder, for chaining.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if either argument is not a string.
   */
  tag(key: string, value: string): PortfolioBuilder;
  /**
   * Attach a JSON-shaped metadata entry.
   * @param key - Name the metadata entry is stored under; a repeated key replaces the earlier entry.
   * @param value - Any JSON value (object, array, string, number, boolean or `null`), or a JSON string encoding one.
   * @returns The builder, for chaining.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `key` is not a string or `value` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if a string `value` is not valid JSON.
   */
  meta(key: string, value: JsonInput): PortfolioBuilder;
  /**
   * Validate and build the portfolio, consuming the builder.
   * @returns The validated, reusable `Portfolio` handle.
   * @throws Error - Throws a `FinstackError` (kind `validation`) if a position references an unregistered entity, an identifier is duplicated, or another structural invariant fails.
   */
  build(): Portfolio;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Typed handle to a built portfolio. Construct once via
 * `Portfolio.fromSpec` and reuse it across cashflow / valuation calls to
 * skip the per-call `PortfolioSpec` parse + rebuild cost.
 */
declare class Portfolio {
  private constructor();
  /**
   * Start a fluent portfolio builder (Rust `PortfolioBuilder`).
   * @param id - Portfolio identifier.
   * @param baseCurrency - ISO-4217 reporting currency used for every base-currency rollup.
   * @param asOf - ISO-8601 valuation date.
   * @returns A `PortfolioBuilder`; chain `.entity(...)` / `.position(...)` / `.tag(...)` and finish with `.build()`.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if an argument is not a string, and a `FinstackError` (kind `validation`) if `baseCurrency` is not an ISO-4217 code or `asOf` is not an ISO date.
   */
  static builder(id: string, baseCurrency: string, asOf: string): PortfolioBuilder;
  /**
   * Build a runtime portfolio from a portable embedded specification.
   * @returns A reusable portfolio handle.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @throws Error - Throws a JavaScript exception if `specJson` is malformed or does not match the portfolio schema, a position has an invalid quantity or instrument specification, or portfolio validation finds duplicate identifiers or an unknown entity reference.
   */
  static fromSpec(specJson: JsonInput): Portfolio;
  /**
   * Build a runtime portfolio from one strict persisted materialization bundle.
   * @param bundle - Complete UTF-8 materialization JSON string or `Uint8Array`.
   * @param cache - Optional reusable decoded-artifact cache created outside any timed validation region. Omit, `null`, or `undefined` for a per-call cache with the native default bounds (the Python `cache=None` twin).
   * @returns An object containing the reusable portfolio and load report.
   * @throws Error - Throws `TypeError` for unsupported input types. Contract failures throw `ContractValidationError` (`kind` `validation`) with a `code` of `report` plus a structured `report` property if the persisted contract is malformed, invalid, or unsupported, or a `code` of `limit_exceeded` if it exceeds a resource limit.
   */
  static fromMaterialization(
    bundle: MaterializationBundleInput,
    cache?: InstrumentArtifactCache | null
  ): PortfolioMaterializationResult;
  /**
   * Validate a materialization bundle and return diagnostics for form UIs.
   * @param bundle - Complete UTF-8 materialization JSON string or `Uint8Array`.
   * @param cache - Optional reusable decoded-artifact cache used while validating. Omit, `null`, or `undefined` for a per-call cache with the native default bounds (the Python `cache=None` twin).
   * @returns A materialization report whose build/index phase counters are zero, or a `ValidationReport` when the contract is invalid but still reportable.
   * @throws Error - Throws `TypeError` for unsupported input types or a structured `ContractValidationError` when validation cannot produce a report.
   */
  static validateMaterialization(
    bundle: MaterializationBundleInput,
    cache?: InstrumentArtifactCache | null
  ): MaterializationReport | ValidationReport;
  /**
   * Portfolio identifier.
   * @returns Stable portfolio ID.
   */
  readonly id: string;
  /**
   * ISO-8601 valuation date.
   * @returns Portfolio as-of date.
   */
  readonly asOf: string;
  /**
   * Reporting currency.
   * @returns ISO-4217 base currency code.
   */
  readonly baseCurrency: string;
  /**
   * Human-readable portfolio name, or `null` when unset.
   */
  readonly name: string | null;
  /**
   * Portfolio-level tags.
   * @throws Error - Throws a JavaScript exception if the tags cannot be converted to a JavaScript value.
   */
  readonly tags: Record<string, string>;
  /**
   * Portfolio-level metadata as a JSON-shaped object.
   * @throws Error - Throws a JavaScript exception if the metadata cannot be converted to a JavaScript value.
   */
  readonly meta: Record<string, unknown>;
  /**
   * Entity identifiers in registration order.
   */
  readonly entityIds: string[];
  /**
   * Position identifiers in portfolio order.
   */
  readonly positionIds: string[];
  /**
   * Return the number of positions.
   * @returns Non-negative position count.
   */
  numPositions(): number;
  /**
   * Serialize the portable portfolio specification.
   * @returns Canonical JSON string.
   * @throws Error - Throws a JavaScript exception if the canonical portfolio specification cannot be serialized to JSON.
   */
  toJson(): string;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Namespaced TypeScript entry points for portfolio calculations and types.
 * @example
 * ```typescript
 * import init, { portfolio } from "finstack-quant-wasm";
 * await init();
 * const cache = new portfolio.InstrumentArtifactCache(32);
 * console.log(cache.size);
 * cache.free();
 * ```
 */
export interface PortfolioNamespace {
  /**
   * Reusable bounded decoded-instrument cache constructor.
   */
  InstrumentArtifactCache: typeof InstrumentArtifactCache;
  /**
   * Typed handle for cached portfolio builds.
   */
  Portfolio: typeof Portfolio;
  /**
   * Parse and validate a portfolio specification from JSON.
   *
   * Wire/validator surface: returns the re-serialized canonical JSON
   * **string**, suitable for storage or re-ingest by `Portfolio.fromSpec`.
   * @returns Canonical portfolio-specification JSON after validation.
   * @param jsonStr - Canonical JSON string to validate and re-serialize.
   * @throws Error - Throws a JavaScript exception if `jsonStr` is malformed or does not match the `PortfolioSpec` schema, or if the canonical form cannot be serialized.
   */
  parsePortfolioSpecJson(jsonStr: JsonInput): string;
  /**
   * Compute a single-period Brinson-Fachler attribution from sector JSON.
   *
   * Accepts a JSON array of `SectorPeriod` objects and returns a structured
   * `BrinsonPeriodResult` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param sectorsJson - Sector-classification JSON.
   * @throws Error - Throws a JavaScript exception if `sectorsJson` is malformed, contains no sectors or a non-finite weight or return, portfolio or benchmark weights do not sum to one, or the result cannot be converted to a JavaScript value.
   */
  brinsonFachler(sectorsJson: JsonInput): BrinsonPeriodResult;
  /**
   * Carino-link already-computed single-period Brinson-Fachler results.
   *
   * Binds Rust `carino_link`: accepts a chronological JSON array of
   * `BrinsonPeriodResult` objects (for example `brinsonFachler` outputs) and
   * returns a structured `CarinoLinkedAttribution` object whose linked effects
   * reconstruct the geometrically compounded active return. Use
   * `carinoLinkFromSectorPeriods` to link raw sector inputs instead.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param periodsJson - Chronological JSON array of `BrinsonPeriodResult` objects with identical sector ordering in every period.
   * @throws Error - Throws a JavaScript exception if `periodsJson` is malformed, the sequence is empty or changes sector ordering, or a period return is non-finite or at most `-1`.
   */
  carinoLink(periodsJson: JsonInput): CarinoLinkedAttribution;
  /**
   * Compute Carino-linked multi-period Brinson attribution from raw sector
   * periods.
   *
   * Binds Rust `carino_link_from_sector_periods`: runs `brinsonFachler` on
   * each period, then Carino-links the results. Returns a structured
   * `CarinoLinkedAttribution` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param periodsJson - Chronological JSON array of periods, each an array of `SectorPeriod` objects (`sector`, `portfolio_weight`, `benchmark_weight`, `portfolio_return`, `benchmark_return`).
   * @throws Error - Throws a JavaScript exception if `periodsJson` is malformed, any period fails Brinson validation, the sequence is empty or changes sector ordering, or a period return is non-finite or at most `-1`.
   */
  carinoLinkFromSectorPeriods(periodsJson: JsonInput): CarinoLinkedAttribution;
  /**
   * Compute a single-period Campisi fixed-income attribution from JSON.
   *
   * Decomposes both sides into carry / treasury / spread / selection and
   * splits the active return into allocation plus four active component
   * effects (Campisi 2000). Returns a structured `FiAttributionResult` object;
   * `JSON.stringify` it to chain into `campisiCarinoLink` or
   * `campisiReconciliationCheck`.
   *
   * Every snapshot must use the quote-reproducing `z_spread` basis:
   * `spread_duration` is the canonical Z-spread duration, and `spread` plus
   * `delta_spread` are the matching Z-spread level and move. OAS, G-spread, or
   * discount-margin values are incompatible. The numeric JSON shape has no
   * metric IDs, so this boundary cannot detect mislabeled spread provenance.
   *
   * Throws when JSON is malformed or canonical Rust validation rejects empty
   * sides, non-finite values, invalid weights or period length, or a sector
   * present on either side has `|net sector weight| <= 1e-6 * gross absolute
   * sector weight`. Spread-basis provenance cannot be validated from numeric
   * JSON alone.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param portfolioJson - Canonical JSON array of `FiPositionSnapshot` objects describing the portfolio side on the quote-reproducing Z-spread basis; weights must sum to 1.
   * @param benchmarkJson - Canonical JSON array of `FiPositionSnapshot` objects describing the benchmark side on the quote-reproducing Z-spread basis; weights must sum to 1.
   * @param configJson - Canonical JSON `FiAttributionConfig`; `period_years` is its only field, is required (no default), and unknown keys are rejected.
   * @throws Error - Throws a JavaScript exception if any JSON input is malformed; either side is empty; a value is non-finite; weights do not sum to one; `periodYears` is not finite and positive; a sector has a zero or near-zero net weight relative to gross weight; or the result cannot be converted to a JavaScript value.
   */
  campisiAttribution(
    portfolioJson: JsonInput,
    benchmarkJson: JsonInput,
    configJson: JsonInput
  ): FiAttributionResult;
  /**
   * Carino-link already-computed single-period Campisi results.
   *
   * Binds Rust `campisi_carino_link`. Each period carries its own
   * already-applied `period_years`, so periods of *different* lengths (e.g.
   * act/365 calendar months) link correctly here; prefer this entry point
   * whenever the periods are not all the same length. Returns a structured
   * `FiCarinoLinkedResult` object.
   *
   * Throws if no periods are supplied, sector ordering differs, a consumed
   * top-level return/effect, per-sector linked effect, or sector `total_active`
   * is non-finite, `active_return` disagrees with the portfolio-minus-benchmark
   * return, a sector `total_active` disagrees with its five effects, sector
   * effects do not reconcile to their declared top-level totals, the five
   * totals do not reconcile to `active_return` within the overflow-safe
   * scaled-L1 tolerance, a reconciliation residual is non-finite, or a return
   * is outside the Carino domain.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param periodsJson - Canonical JSON array of `FiAttributionResult` objects in chronological order, as returned by `campisiAttribution`.
   * @throws Error - Throws a JavaScript exception if `periodsJson` is malformed, the sequence is empty or changes sector ordering, a consumed value or reconciliation is non-finite or inconsistent, a return is at most `-1`, or the linked result cannot be converted to a JavaScript value.
   */
  campisiCarinoLink(periodsJson: JsonInput): FiCarinoLinkedResult;
  /**
   * Compute per-period Campisi attributions from snapshots and Carino-link them.
   *
   * Binds Rust `campisi_carino_link_from_snapshots`. One shared config — hence
   * one shared `period_years` — is applied to every period, so this entry point
   * is only correct for equal-length periods; use `campisiCarinoLink` for
   * unequal periods. Returns a structured `FiCarinoLinkedResult` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param periodsJson - Canonical JSON array of `FiPeriodInput` objects, each holding `portfolio` and `benchmark` arrays of `FiPositionSnapshot`.
   * @param configJson - Canonical JSON `FiAttributionConfig` applied to every period; `period_years` is its only field and is required (no default).
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, any period fails Campisi attribution validation, the computed periods fail Carino linking validation, or the result cannot be converted to a JavaScript value.
   */
  campisiCarinoLinkFromSnapshots(
    periodsJson: JsonInput,
    configJson: JsonInput
  ): FiCarinoLinkedResult;
  /**
   * Reconcile the five Campisi effect totals against the active return.
   *
   * Binds the Rust method `FiAttributionResult::reconciliation_check`. The
   * decomposition reconciles by construction (selection is the residual), so
   * this is a floating-point sanity gate rather than a model check; without it
   * callers must re-sum the five totals by hand. Returns a structured
   * `FiReconciliationReport` object with `total_residual`, `is_reconciled`
   * and `tolerance`.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param resultJson - Canonical JSON `FiAttributionResult` as returned by `campisiAttribution` (`JSON.stringify` its structured result); unknown fields are rejected.
   * @param tolerance - Absolute reconciliation tolerance in return units; `1e-10` suits return-space values.
   * @throws Error - Throws a JavaScript exception if `resultJson` is malformed or does not match `FiAttributionResult`, or if the reconciliation report cannot be converted to a JavaScript value.
   */
  campisiReconciliationCheck(resultJson: JsonInput, tolerance: number): FiReconciliationReport;
  /**
   * Build a duration-cell base-return table from a reference universe.
   *
   * Binds Rust `cell_returns_from_reference` (Dynkin, Hyman & Vankudre 1998,
   * Appendix B): buckets `referenceJson` into fixed-width duration cells and
   * averages each cell's member total returns, interpolating interior gaps
   * and flat-extrapolating leading/trailing gaps. Returns a structured
   * `DurationCellTable` object; `JSON.stringify` it to chain into
   * `excessReturns`.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param referenceJson - Canonical JSON array of `ReferenceReturn` objects (`duration`, `total_return`, both decimals with duration in years); must be non-empty.
   * @param baseLabel - Label identifying the resulting curve (e.g. `"UST"`), carried through to the output's `base_label` for policy visibility.
   * @param configJson - Canonical JSON `CellConfig`; `width` is its only field (cell width in years, finite and positive) and is required, with no default.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, the reference universe is empty or contains an invalid duration or return, the cell width is not finite and positive, labels collide, the grid exceeds its safety bound, or the result cannot be converted to a JavaScript value.
   */
  cellReturnsFromReference(
    referenceJson: JsonInput,
    baseLabel: string,
    configJson: JsonInput
  ): DurationCellTable;
  /**
   * Build a duration-cell base-return table from start/end discount curves.
   *
   * Binds Rust `cell_returns_from_curves`: each cell's base return is the
   * holding-period return of a hypothetical zero-coupon position bought at
   * the cell midpoint off `start` and revalued off `end` after
   * `horizonYears` have elapsed. Every resulting cell is observed, unlike
   * the reference-universe path in `cellReturnsFromReference`. Returns a
   * structured `DurationCellTable` object; `JSON.stringify` it to chain into
   * `excessReturns`.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param start - Discount curve observed at the start of the holding period.
   * @param end - Discount curve observed `horizonYears` later, at period end.
   * @param horizonYears - Length of the holding period, in years; must be finite and positive.
   * @param maxDuration - Upper bound of the duration grid, in years; must be finite and strictly greater than `horizonYears`.
   * @param baseLabel - Label identifying the base curve (e.g. `"UST"`, `"USD-SOFR"`), stamped into the result purely for policy visibility.
   * @param configJson - Canonical JSON `CellConfig`; `width` is its only field and is required, with no default.
   * @throws Error - Throws a JavaScript exception if `configJson` is malformed; the width, horizon, or maximum duration is invalid; a cell matures within the holding period; the grid is too large or has duplicate labels; a required discount factor is not finite and positive; or the result cannot be converted to a JavaScript value.
   */
  cellReturnsFromCurves(
    start: DiscountCurve,
    end: DiscountCurve,
    horizonYears: number,
    maxDuration: number,
    baseLabel: string,
    configJson: JsonInput
  ): DurationCellTable;
  /**
   * Compute duration-matched credit excess returns against a base-return table.
   *
   * Binds Rust `excess_returns` (Dynkin, Hyman & Vankudre 1998, Appendix B):
   * each position's `duration` is matched to its duration cell in
   * `tableJson` and the position's excess return is `total_return -
   * cell.base_return`, the credit-specific component of performance
   * isolated from the general level/shape move of the base curve. Returns a
   * structured `ExcessReturnResult` object with per-position and
   * portfolio-level totals.
   *
   * Throws if the table is empty or has empty/duplicate cell labels,
   * non-finite/negative/zero-width, non-ascending, or overlapping cells; any
   * position is invalid or falls in no cell (including a valid gap); or
   * position weights do not sum to one.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param positionsJson - Canonical JSON array of `ExcessReturnPosition` objects (`id`, `weight`, `duration`, `total_return`); weights must sum to 1.
   * @param tableJson - Canonical JSON `DurationCellTable`; `JSON.stringify` the structured table returned by `cellReturnsFromReference` or `cellReturnsFromCurves`.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, the cell table is invalid, a position is invalid or falls in no cell, position weights do not sum to one, or the result cannot be converted to a JavaScript value.
   */
  excessReturns(positionsJson: JsonInput, tableJson: JsonInput): ExcessReturnResult;
  /**
   * Compute a single-period hierarchical duration-cell x sector grid attribution.
   *
   * Binds Rust `grid_attribution` (Dynkin, Hyman & Vankudre 1998, Appendix
   * A): decomposes active return into a per-cell curve (positioning)
   * effect, a within-cell sector allocation effect, and a
   * security-selection residual per (cell, sector). Returns a structured
   * `GridAttributionResult` object (`JSON.stringify` it to chain into
   * `gridCarinoLink`) whose `total_curve`, `total_sector` and
   * `total_selection` sum to `active_return` to floating-point precision
   * for well-conditioned inputs; among accepted inputs, the reconciliation
   * residual grows the closer any bucket's net weight sits to the
   * near-zero-net-weight rejection boundary (see the Rust module docs for
   * measured magnitudes).
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param portfolioJson - Canonical JSON array of `GridPosition` objects (`cell`, `sector`, `weight`, `total_return`) for the portfolio side; weights must sum to 1.
   * @param benchmarkJson - Canonical JSON array of `GridPosition` objects for the benchmark side; same weight-sum requirement.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, a weight or return is non-finite, either side's weights do not sum to one, a cell or cell-sector bucket has a zero or near-zero net weight relative to gross weight, or the result cannot be converted to a JavaScript value.
   */
  gridAttribution(portfolioJson: JsonInput, benchmarkJson: JsonInput): GridAttributionResult;
  /**
   * Carino-link multi-period hierarchical grid attribution results.
   *
   * Binds Rust `grid_carino_link` (Carino 1999): applies Carino smoothing to
   * a chronological sequence of single-period `gridAttribution` results so
   * the three top-level effects (`linked_curve`, `linked_sector`,
   * `linked_selection`) sum exactly to the geometrically compounded active
   * return. Only the three top-level effects are linked; per-cell /
   * per-(cell, sector) multi-period linking is out of scope. Returns a
   * structured `GridCarinoLinkedResult` object.
   *
   * Throws if no periods are supplied; a consumed return or top-level effect
   * is non-finite; `active_return` disagrees with the portfolio-minus-
   * benchmark return; the three effect totals do not reconcile to
   * `active_return` within the overflow-safe scaled-L1 tolerance; a return-
   * identity or reconciliation residual is non-finite; or a return is outside
   * the Carino domain.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param periodsJson - Canonical JSON array of `GridAttributionResult` objects, in chronological order; `JSON.stringify` the structured results returned by `gridAttribution`.
   * @throws Error - Throws a JavaScript exception if `periodsJson` is malformed, the sequence is empty, a consumed value is non-finite or inconsistent, a return is at most `-1`, or the linked result cannot be converted to a JavaScript value.
   */
  gridCarinoLink(periodsJson: JsonInput): GridCarinoLinkedResult;
  /**
   * Compute Jeet-Partani (2023) factor-Brinson unified attribution.
   *
   * Binds Rust `factor_brinson_attribution`: generalizes classical
   * Brinson-Fachler allocation/selection to continuous factor exposures by
   * replacing the sector partition with a factor-exposure matrix and a
   * caller-supplied benchmark factor-return vector. Returns a structured
   * `FactorBrinsonResult` object with `allocation`, `selection`, and their
   * per-factor / per-asset breakdowns.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param inputJson - Canonical JSON `FactorBrinsonInput` with `asset_ids`, `asset_returns`, `exposures` (row-major n_assets x n_factors), `factor_names`, `portfolio_weights` and `benchmark_weights`; each weight vector must sum to 1.
   * @param factorReturns - Caller-supplied benchmark factor returns `f_b` as a `number[]` or `Float64Array`, length `input.factor_names`; the `Float64Array` returned by `analytics.constrainedLeastSquares` can be passed directly.
   * @throws Error - Throws a JavaScript exception if `inputJson` is malformed; the asset or factor sets are empty; dimensions disagree; a value is non-finite; either weight vector does not sum to one; benchmark factor completeness is outside tolerance; or the result cannot be converted to a JavaScript value.
   */
  factorBrinsonAttribution(inputJson: JsonInput, factorReturns: NumericArray): FactorBrinsonResult;
  /**
   * Compute a Modified-Dietz TWRR sub-period return from period JSON.
   * @returns Sub-period time-weighted return as a decimal.
   * @param periodJson - `TwrrPeriod` JSON: `beginning_market_value`, `ending_market_value` and optional `cashflows: [{ amount, fraction_of_period_remaining }]` (omitted means no flows); a positive `amount` is a contribution into the portfolio and the fraction, in `[0, 1]`, is the share of the period remaining after the flow. Unknown keys are rejected.
   * @throws Error - Throws a JavaScript exception if `periodJson` is malformed, has unknown keys, or the return is undefined (non-positive adjusted denominator, out-of-range cashflow weight, non-finite inputs).
   */
  twrrModifiedDietz(periodJson: JsonInput): number;
  /**
   * Geometrically link TWRR sub-period returns from returns JSON.
   * @returns Linked time-weighted return object, including the annualized rate over `horizonYears`.
   * @param returnsJson - Numeric return-series JSON.
   * @param horizonYears - Return-linking horizon measured in years for annualization.
   * @throws Error - Throws a JavaScript exception if `returnsJson` is malformed, the return series is invalid (non-finite sub-period return or return at most -1, non-positive compounded growth factor), or the linked result cannot be converted to a JavaScript value.
   */
  twrrLinked(returnsJson: JsonInput, horizonYears: number): LinkedReturn;
  /**
   * Compute money-weighted return via XIRR from dated cashflow JSON.
   *
   * Binds Rust `mwr_xirr_from_cashflows` (Act/365F).
   * @returns Annualized money-weighted return as a decimal.
   * @param cashflowsJson - JSON array of `{ date, amount }` flows from the investor's cash account: contributions negative, distributions and terminal value positive.
   * @throws Error - Throws a JavaScript exception if `cashflowsJson` is malformed, contains an invalid date or insufficient cash flows for XIRR, or the numerical root cannot be found.
   */
  mwrXirr(cashflowsJson: JsonInput): number;
  /**
   * Build a runtime portfolio from a JSON spec, validate, and round-trip.
   *
   * Wire/validator surface: deserializes the spec, constructs the portfolio
   * with live instruments, validates structural invariants, then
   * re-serializes the canonical JSON **string** for confirmation or
   * re-ingest.
   * @returns Canonical portfolio JSON after construction and validation.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @throws Error - Throws a JavaScript exception if `specJson` is malformed or violates the portfolio schema, a position has an invalid quantity or instrument specification, portfolio validation fails, or the round-trip form cannot be serialized.
   */
  buildPortfolioFromSpecJson(specJson: JsonInput): string;
  /**
   * Aggregate portfolio metrics from a valuation JSON.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param valuationJson - `PortfolioValuation` JSON, for example `JSON.stringify(valuePortfolio(...))`.
   * @param baseCurrency - ISO-4217 base currency in which aggregate portfolio values are reported.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, `baseCurrency` or `asOf` is invalid, valuation currency or date metadata is inconsistent, a required FX conversion is unavailable or invalid, or the metrics cannot be converted to a JavaScript value.
   */
  aggregateMetrics(
    valuationJson: JsonInput,
    baseCurrency: string,
    marketJson: JsonInput,
    asOf: string
  ): PortfolioMetrics;
  /**
   * Decoded series of one base metric from aggregated portfolio metrics (twin of Python `PortfolioMetrics.metric_series`).
   * @param metricsJson - `PortfolioMetrics` from `aggregateMetrics` (object or JSON).
   * @param base - Canonical base metric identifier (e.g. `"bucketed_dv01"`).
   * @returns `{ components, total, by_entity }` entries in aggregation order.
   * @throws Error - Throws with kind `validation` if the metrics input is malformed or `base` is not a canonically encoded metric key.
   */
  portfolioMetricsSeries(metricsJson: PortfolioMetrics | string, base: string): PortfolioMetricSeriesEntry[];
  /**
   * Value a portfolio from its spec and market context.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param strictRisk - Optional; when omitted or `undefined`, uses the Rust `PortfolioValuationOptions` default, `true` (fail closed when a requested risk metric fails to compute). Pass `false` only for an intentional PV-preserving fallback.
   * @param metrics - Optional risk-metric ids to offer every position. Omit for the standard set (PV plus `dv01`; pricer-specific metrics such as `theta` or `cs01` must be listed explicitly); an empty array performs PV-only valuation. Names resolve exactly as in `priceInstrument`: every id `listStandardMetrics()` returns is accepted and an unknown name throws. The list is a menu, not a per-position request: one list is chosen for a book of mixed instrument types, so each position is asked for exactly the entries its own instrument can compute (a composite position: the additive entries at least one leg supports), and the rest appear on that position's `inapplicable_metrics`. Narrowing covers structural inapplicability only; `strictRisk` still governs a metric an instrument supports but fails to compute. `priceInstrument` keeps the opposite contract and throws on a metric its instrument cannot produce. Mirrors the Python `metrics=` keyword. Each position's `valuation_result` keeps 64-bit fields (the Monte Carlo `seed` and path counts) as `BigInt`, exactly as `priceInstrument` returns them; serialize one with `valuations.valuationResultToJson`. `position_values` and `by_entity` are plain objects keyed by id: JavaScript enumerates integer-like ids (such as `"10"`, `"2"`) in ascending numeric order before other keys, not in the Rust (and Python) insertion order; take valuation order from `spec.positions` when it matters.
   * @throws Error - Throws a JavaScript exception if the portfolio or market JSON is malformed, a requested metric name is unknown, portfolio construction or valuation fails, strict risk calculation cannot produce a requested metric, a required FX conversion is unavailable, or the valuation cannot be converted to a JavaScript value.
   */
  valuePortfolio(
    specJson: JsonInput,
    marketJson: JsonInput,
    strictRisk?: boolean,
    metrics?: string[]
  ): PortfolioValuation;
  /**
   * Value an already-built [`Portfolio`] handle. Skips the per-call
   * `PortfolioSpec` parse + `Portfolio::from_spec` rebuild that
   * [`value_portfolio`] performs; use this when sweeping market scenarios
   * against a fixed portfolio.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param strictRisk - Optional; when omitted or `undefined`, uses the Rust `PortfolioValuationOptions` default, `true` (fail closed when a requested risk metric fails to compute). Pass `false` only for an intentional PV-preserving fallback.
   * @param metrics - Optional risk-metric ids to offer every position. Omit for the standard set (PV plus `dv01`; pricer-specific metrics such as `theta` or `cs01` must be listed explicitly); an empty array performs PV-only valuation. Names resolve exactly as in `priceInstrument`: every id `listStandardMetrics()` returns is accepted and an unknown name throws. The list is a menu, not a per-position request: one list is chosen for a book of mixed instrument types, so each position is asked for exactly the entries its own instrument can compute (a composite position: the additive entries at least one leg supports), and the rest appear on that position's `inapplicable_metrics`. Narrowing covers structural inapplicability only; `strictRisk` still governs a metric an instrument supports but fails to compute. `priceInstrument` keeps the opposite contract and throws on a metric its instrument cannot produce. Mirrors the Python `metrics=` keyword. Each position's `valuation_result` keeps 64-bit fields (the Monte Carlo `seed` and path counts) as `BigInt`, exactly as `priceInstrument` returns them; serialize one with `valuations.valuationResultToJson`. `position_values` and `by_entity` are plain objects keyed by id: JavaScript enumerates integer-like ids (such as `"10"`, `"2"`) in ascending numeric order before other keys, not in the Rust (and Python) insertion order; take valuation order from `spec.positions` when it matters.
   * @throws Error - Throws a JavaScript exception if `marketJson` is malformed, a requested metric name is unknown, portfolio valuation fails, strict risk calculation cannot produce a requested metric, a required FX conversion is unavailable, or the valuation cannot be converted to a JavaScript value.
   */
  valuePortfolioBuilt(
    portfolio: Portfolio,
    marketJson: JsonInput,
    strictRisk?: boolean,
    metrics?: string[]
  ): PortfolioValuation;
  /**
   * Aggregate the full classified cashflow ladder for a portfolio.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param allowPartial - Optional; when omitted or `undefined`, uses the Rust `CashflowAggregationOptions` default, `false` (fail closed if any position fails schedule construction). Pass `true` to keep a partial ladder with issues on the result.
   * @throws Error - Throws a JavaScript exception if the portfolio or market JSON is malformed, portfolio construction fails, any position fails schedule construction while `allowPartial` is not `true`, monetary cash-flow aggregation overflows, or the aggregate cannot be converted to a JavaScript value.
   */
  aggregateFullCashflows(
    specJson: JsonInput,
    marketJson: JsonInput,
    allowPartial?: boolean
  ): PortfolioCashflows;
  /**
   * Aggregate the full classified cashflow ladder for an already-built
   * [`Portfolio`] handle.
   *
   * Skips the per-call `PortfolioSpec` parse + `Portfolio::from_spec` rebuild.
   * For batched or chained workflows (repeated cashflow builds across market
   * scenarios on the same portfolio), this is the cheap path.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param allowPartial - Optional; when omitted or `undefined`, uses the Rust `CashflowAggregationOptions` default, `false` (fail closed if any position fails schedule construction). Pass `true` to keep a partial ladder with issues on the result.
   * @throws Error - Throws a JavaScript exception if `marketJson` is malformed, any position fails schedule construction while `allowPartial` is not `true`, monetary cash-flow aggregation overflows, or the aggregate cannot be converted to a JavaScript value.
   */
  aggregateFullCashflowsBuilt(
    portfolio: Portfolio,
    marketJson: JsonInput,
    allowPartial?: boolean
  ): PortfolioCashflows;
  /**
   * Net same-currency cashflow amounts per date from a cashflow ladder (mirrors Python `net_in_currency_by_date`).
   * @param cashflowsJson - `PortfolioCashflows` from `aggregateFullCashflows` (object or JSON), or a bare `{date: {ccy: {kind: money}}}` map.
   * @param currency - ISO-4217 code selecting which per-date currency bucket to net.
   * @returns `[isoDate, netAmount]` pairs sorted by date; dates with no flows in `currency` are omitted.
   * @throws Error - Throws with kind `validation` if the input is not JSON, `currency` is not a known ISO code, or `by_date` is not an object.
   */
  netInCurrencyByDate(cashflowsJson: PortfolioCashflows | string, currency: string): [string, number][];
  /**
   * Collapse a multi-currency cashflow ladder into the base currency per date and kind (twin of Python `PortfolioCashflows.collapse_to_base_by_date_kind_json`), converting at the CIP forward from the `asOf` spot.
   * @param cashflowsJson - `PortfolioCashflows` from `aggregateFullCashflows` (object or JSON).
   * @param marketJson - Canonical market-context JSON supplying the FX matrix and discount curves.
   * @param baseCurrency - ISO-4217 reporting currency.
   * @param asOf - ISO-8601 valuation date for spot FX and the start of each discount interval.
   * @param discountCurves - Optional `{ currency: curveId }` map; a missing entry uses the ISO code as the curve id.
   * @returns Nested `{ isoDate: { kind: Money } }` ladder in the base currency.
   * @throws Error - Throws if an input is malformed, an FX rate or discount factor needed for a conversion is missing or invalid, or monetary aggregation fails.
   */
  collapseToBaseByDateKind(
    cashflowsJson: PortfolioCashflows | string,
    marketJson: JsonInput,
    baseCurrency: string,
    asOf: string,
    discountCurves?: Record<string, string> | null
  ): Record<string, Record<string, MoneyValue>>;
  /**
   * Apply a scenario to a portfolio and revalue.
   *
   * Returns a JS object with structured `valuation` and `report` values.
   * @returns Revalued result object and scenario application report.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param scenarioJson - Scenario specification JSON.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data. Each position's `valuation_result` keeps 64-bit fields (the Monte Carlo `seed` and path counts) as `BigInt`, exactly as `priceInstrument` returns them; serialize one with `valuations.valuationResultToJson`. `position_values` and `by_entity` are plain objects keyed by id: JavaScript enumerates integer-like ids (such as `"10"`, `"2"`) in ascending numeric order before other keys, not in the Rust (and Python) insertion order; take valuation order from `spec.positions` when it matters.
   * @throws Error - Throws a JavaScript exception if the portfolio, scenario, or market JSON is malformed; portfolio construction, scenario application, or revaluation fails; or the structured result cannot be converted to a JavaScript value.
   */
  applyScenarioAndRevalue(
    specJson: JsonInput,
    scenarioJson: JsonInput,
    marketJson: JsonInput
  ): ScenarioRevalueView;
  /**
   * Apply a scenario to an already-built [`Portfolio`] handle and revalue.
   * Returns a JS object with structured `valuation` and `report` values.
   * @returns Revalued result object and scenario application report.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param scenarioJson - Scenario specification JSON.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data. Each position's `valuation_result` keeps 64-bit fields (the Monte Carlo `seed` and path counts) as `BigInt`, exactly as `priceInstrument` returns them; serialize one with `valuations.valuationResultToJson`. `position_values` and `by_entity` are plain objects keyed by id: JavaScript enumerates integer-like ids (such as `"10"`, `"2"`) in ascending numeric order before other keys, not in the Rust (and Python) insertion order; take valuation order from `spec.positions` when it matters.
   * @throws Error - Throws a JavaScript exception if the scenario or market JSON is malformed, scenario application or portfolio revaluation fails, or the structured result cannot be converted to a JavaScript value.
   */
  applyScenarioAndRevalueBuilt(
    portfolio: Portfolio,
    scenarioJson: JsonInput,
    marketJson: JsonInput
  ): ScenarioRevalueView;
  /**
   * Compute the profit and loss attributable to a scenario.
   *
   * Values the portfolio against the unshocked market and against the
   * scenario-shocked market, and returns a JS object with structured `pnl`
   * (base-currency `total` plus `by_position`) and `report` values.
   * @returns Scenario-attributable P&L ladder and application report.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param scenarioJson - Canonical JSON payload representing the scenario whose profit-and-loss impact is measured.
   * @param marketJson - Canonical market-context JSON supplying the unshocked curves, quotes, and FX data used for the base leg.
   * @throws Error - Throws a JavaScript exception if the portfolio, scenario, or market JSON is malformed; portfolio construction, scenario application, or either valuation fails; valuation currencies are inconsistent; or the structured result cannot be converted to JavaScript.
   */
  scenarioPnl(specJson: JsonInput, scenarioJson: JsonInput, marketJson: JsonInput): ScenarioPnlView;
  /**
   * Compute the profit and loss attributable to a scenario for an
   * already-built [`Portfolio`] handle.
   *
   * Values the portfolio against the unshocked market and against the
   * scenario-shocked market, and returns a JS object with structured `pnl`
   * (base-currency `total` plus `by_position`) and `report` values. Positions
   * added or removed by the scenario are zero-filled against the missing side,
   * so the drill-down always sums to the total.
   * @returns Scenario-attributable P&L ladder and application report.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param scenarioJson - Canonical JSON payload representing the scenario whose profit-and-loss impact is measured.
   * @param marketJson - Canonical market-context JSON supplying the unshocked curves, quotes, and FX data used for the base leg.
   * @throws Error - Throws a JavaScript exception if the scenario or market JSON is malformed, scenario application or either valuation fails, valuation currencies are inconsistent, or the structured result cannot be converted to JavaScript.
   */
  scenarioPnlBuilt(
    portfolio: Portfolio,
    scenarioJson: JsonInput,
    marketJson: JsonInput
  ): ScenarioPnlView;
  /**
   * Optimize portfolio weights using the LP-based optimizer.
   *
   * Accepts a `PortfolioOptimizationSpec` JSON (portfolio + objective +
   * constraints + options) and a `MarketContext` JSON, and returns a
   * structured `PortfolioOptimizationResult` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param specJson - `PortfolioOptimizationSpec` JSON: `portfolio` (a `PortfolioSpec`) plus `objective`, and optional `constraints`, `weighting`, `missing_metric_policy`, `label` and `trade_universe`.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, the portfolio, objective, constraints, weighting, or missing-metric policy is invalid, a required market-dependent valuation fails, the solver cannot produce a result, or the result cannot be converted to a JavaScript value.
   */
  optimizePortfolio(specJson: JsonInput, marketJson: JsonInput): PortfolioOptimizationResult;
  /**
   * Rebalance a spec's portfolio to an optimization result (mirrors Python `rebalance_from_spec`).
   * @param specJson - The `PortfolioOptimizationSpec` JSON passed to `optimizePortfolio`.
   * @param resultJson - The `PortfolioOptimizationResult` that `optimizePortfolio` returned for that spec (object or JSON).
   * @returns The rebalanced, validated `Portfolio` handle.
   * @throws Error - Throws with kind `validation` if either input is malformed, the solution is infeasible, or the result names a position that is neither in the spec portfolio nor a trade-universe candidate (a result paired with the wrong spec), and propagates portfolio validation failures.
   */
  rebalanceFromSpec(specJson: JsonInput, resultJson: PortfolioOptimizationResult | string): Portfolio;
  /**
   * Replay a portfolio through dated market snapshots.
   *
   * Accepts a portfolio spec, an array of dated market snapshots, and a
   * replay configuration. Returns a structured `ReplayResult` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param snapshotsJson - Market-snapshot JSON array.
   * @param configJson - `ReplayConfig` JSON with a required `mode` (`pv_only` | `pv_and_pnl` | `full_attribution`) and optional `attribution_method`, `valuation_options` and `on_error`; unknown keys are rejected. Each position's `valuation_result` keeps 64-bit fields (the Monte Carlo `seed` and path counts) as `BigInt`, exactly as `priceInstrument` returns them; serialize one with `valuations.valuationResultToJson`. `position_values` and `by_entity` are plain objects keyed by id: JavaScript enumerates integer-like ids (such as `"10"`, `"2"`) in ascending numeric order before other keys, not in the Rust (and Python) insertion order; take valuation order from `spec.positions` when it matters.
   * @throws Error - Throws a JavaScript exception if any JSON input is malformed; the portfolio, replay configuration, or snapshot dates and ordering are invalid; valuation, attribution, or currency conversion fails; best-effort replay retains no step; or the result cannot be converted to a JavaScript value.
   */
  replayPortfolio(
    specJson: JsonInput,
    snapshotsJson: JsonInput,
    configJson: JsonInput
  ): ReplayResult;
  /**
   * Compute first-order factor sensitivities and return the matrix.
   *
   * Accepts a JSON array of positions, a JSON array of `FactorDefinition`,
   * a `MarketContext` JSON, an ISO 8601 date, and an optional `BumpSizeConfig`
   * JSON.  Returns the canonical sensitivity-matrix wire object
   * `{ base_currency, position_ids, factor_ids, data }` with `data` as nested
   * rows (`data[position][factor]`): the same shape `decomposeFactorRisk`
   * accepts and the Python `SensitivityMatrix.to_json` emits.
   * @returns Returns a structured `SensitivityMatrixJson` object.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param baseCurrency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @throws Error - Throws a JavaScript exception if `asOf` is not a valid ISO date; any JSON input is malformed; a factor definition or bump configuration is invalid or unsupported; bumping or repricing fails; or the sensitivity matrix cannot be converted to a JavaScript value.
   */
  computeFactorSensitivities(
    positionsJson: JsonInput,
    factorsJson: JsonInput,
    marketJson: JsonInput,
    asOf: string,
    baseCurrency: string,
    bumpConfigJson?: JsonInput
  ): SensitivityMatrixJson;
  /**
   * Compute first-order factor sensitivities using a pre-parsed `core.MarketContext` handle.
   *
   * Avoids reparsing market JSON for repeated factor analytics calls.
   * @returns Returns a structured `SensitivityMatrixJson` object.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param baseCurrency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @throws Error - Throws a JavaScript exception if `asOf` is not a valid ISO date; a position, factor, or bump-config JSON input is malformed; a factor definition is invalid or unsupported; bumping or repricing fails; or the sensitivity matrix cannot be converted to a JavaScript value.
   */
  computeFactorSensitivitiesWithMarket(
    positionsJson: JsonInput,
    factorsJson: JsonInput,
    market: MarketContext,
    asOf: string,
    baseCurrency: string,
    bumpConfigJson?: JsonInput
  ): SensitivityMatrixJson;
  /**
   * Compute scenario P&L profiles via full repricing.
   *
   * Same position/factor/market inputs as `computeFactorSensitivities`, plus
   * an optional `n_scenario_points` integer. Returns a structured array with
   * one `FactorPnlProfile` (`{ base_currency, factor_id, position_ids, shifts,
   * position_pnls }`, the Rust serde form) per shocked factor.
   * @returns Returns a structured `FactorPnlProfile` array.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param baseCurrency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @param nScenarioPoints - Odd number of evenly spaced bump levels in each P-and-L profile, in `3..=1001`; omit for 5.
   * @throws Error - Throws a JavaScript exception if `asOf` is not a valid ISO date; any JSON input is malformed; a factor, bump configuration, or scenario-point count is invalid or unsupported; bumping or repricing fails; or the profiles cannot be converted to a JavaScript value.
   */
  computePnlProfiles(
    positionsJson: JsonInput,
    factorsJson: JsonInput,
    marketJson: JsonInput,
    asOf: string,
    baseCurrency: string,
    bumpConfigJson?: JsonInput,
    nScenarioPoints?: number
  ): FactorPnlProfile[];
  /**
   * Compute scenario P&L profiles using a pre-parsed `core.MarketContext` handle.
   * @returns Returns a structured `FactorPnlProfile` array.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param baseCurrency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @param nScenarioPoints - Odd number of evenly spaced bump levels in each P-and-L profile, in `3..=1001`; omit for 5.
   * @throws Error - Throws a JavaScript exception if `asOf` is not a valid ISO date; a position, factor, or bump-config JSON input is malformed; a factor or scenario-point count is invalid or unsupported; bumping or repricing fails; or the profiles cannot be converted to a JavaScript value.
   */
  computePnlProfilesWithMarket(
    positionsJson: JsonInput,
    factorsJson: JsonInput,
    market: MarketContext,
    asOf: string,
    baseCurrency: string,
    bumpConfigJson?: JsonInput,
    nScenarioPoints?: number
  ): FactorPnlProfile[];
  /**
   * Decompose portfolio risk into factor and position contributions.
   *
   * Uses the parametric (covariance-based) Euler decomposition.  Accepts
   * the canonical sensitivity-matrix wire object (the output of
   * `computeFactorSensitivities`, or the Python `SensitivityMatrix.to_json`),
   * a `FactorCovarianceMatrix` JSON, and an optional `RiskMeasure` JSON.
   *
   * Returns a structured object with `total_risk`, `measure`, `residual_risk`,
   * `factor_contributions` (array), `position_factor_contributions` (array),
   * and `position_residual_contributions` (array; empty for the parametric
   * decomposer — populated only by credit-aware position decomposers).
   *
   * `measure` is the risk measure in its canonical serde form, the same shape
   * as the `riskMeasureJson` input: `"variance"` or `"volatility"`, or an
   * object carrying `confidence` for `var` / `expected_shortfall`. The Python
   * `RiskDecomposition.measure` getter returns the same value.
   * @returns Returns a structured `RiskDecomposition` object.
   * @param sensitivitiesJson - Canonical sensitivity-matrix JSON `{ base_currency, position_ids, factor_ids, data }` with one `data` row per position and one entry per factor; unknown keys are rejected.
   * @param covarianceJson - Factor covariance-matrix JSON aligned with the supplied sensitivities.
   * @param riskMeasureJson - Risk-measure JSON selecting the decomposition metric; omit for `"variance"`.
   * @throws Error - Throws a JavaScript exception (`kind` `validation`) if any JSON input is malformed or has unknown keys; `base_currency` is not an ISO-4217 code; the sensitivity rows do not match `position_ids` / `factor_ids`; sensitivity and covariance factor axes disagree; the covariance matrix or risk measure is invalid; or decomposition produces invalid variance or another non-finite value.
   */
  decomposeFactorRisk(
    sensitivitiesJson: JsonInput,
    covarianceJson: JsonInput,
    riskMeasureJson?: JsonInput
  ): RiskDecomposition;
  /**
   * Fluent portfolio builder class; instances come from `Portfolio.builder`.
   */
  PortfolioBuilder: typeof PortfolioBuilder;
  /**
   * Allocate strategy weights from a `WeightAllocationSpec`.
   *
   * Returns the `WeightAllocationResult` object (`scheme`, per-strategy
   * `allocations` and portfolio `diagnostics`); `allocateWeightsJson` returns
   * the same result as a JSON string.
   * @param spec - `WeightAllocationSpec` object or JSON selecting the allocation scheme, the strategy inputs and any covariance data the scheme needs.
   * @returns The `WeightAllocationResult`.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `spec` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if it does not match the `WeightAllocationSpec` schema or violates a scheme invariant.
   */
  allocateWeights(spec: WeightAllocationSpec | string): WeightAllocationResult;
  /**
   * Allocate strategy weights and return the result as wire JSON.
   *
   * Wire twin of `allocateWeights` (Rust `allocate_weights_json`).
   * @param specJson - `WeightAllocationSpec` object or JSON.
   * @returns Compact JSON of the `WeightAllocationResult`.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `specJson` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if it does not match the `WeightAllocationSpec` schema or violates a scheme invariant.
   */
  allocateWeightsJson(specJson: WeightAllocationSpec | string): string;
  /**
   * Validate a strategy allocation specification and return its canonical JSON.
   *
   * The spec is parsed and executed solely for validation; the allocation
   * itself is discarded (Rust `validate_allocation_json`).
   * @param specJson - `WeightAllocationSpec` object or JSON.
   * @returns The canonical compact JSON of the accepted spec.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `specJson` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if it does not match the `WeightAllocationSpec` schema or violates a scheme invariant.
   */
  validateAllocationJson(specJson: WeightAllocationSpec | string): string;
  /**
   * Run a factor-stress scenario and revalue the portfolio under the stressed market.
   *
   * Each stress shifts one configured factor by an absolute amount in that
   * factor's own bump units. Returns the `StressResult`: base-currency
   * `total_pnl`, per-position `position_pnl`, and the `stressed_decomposition`
   * of risk under the stressed market.
   * @param portfolio - Built portfolio whose positions are revalued.
   * @param market - `core.MarketContext` handle holding the unstressed curves, quotes and FX data.
   * @param factorModelConfig - `FactorModelConfig` object or JSON: factor definitions, matching rules, covariance and risk measure.
   * @param asOf - ISO-8601 valuation date.
   * @param stresses - Array of `[factorId, shift]` pairs; every `factorId` must be a factor of the configured model.
   * @returns The `StressResult`.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a `FinstackError` if the config or stresses are malformed, `asOf` is not an ISO date, a stress names an unknown factor, or the market bump, valuation, sensitivity or decomposition step fails.
   */
  factorStress(
    portfolio: Portfolio,
    market: MarketContext,
    factorModelConfig: FactorModelConfig | string,
    asOf: string,
    stresses: [string, number][] | string
  ): StressResult;
  /**
   * Run a position remove/resize what-if analysis against a factor model.
   *
   * Decomposes the portfolio's baseline risk, applies the changes and
   * decomposes again. Returns the `WhatIfResult`: the `before` and `after`
   * risk decompositions and the per-factor `delta`.
   * @param portfolio - Built portfolio the changes are applied to.
   * @param market - `core.MarketContext` handle holding the curves, quotes and FX data.
   * @param factorModelConfig - `FactorModelConfig` object or JSON: factor definitions, matching rules, covariance and risk measure.
   * @param asOf - ISO-8601 valuation date.
   * @param changes - Array of `PositionChange` objects: `{ kind: "remove", position_id }` or `{ kind: "resize", position_id, new_quantity }`.
   * @returns The `WhatIfResult`.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a `FinstackError` if the config or changes are malformed, `asOf` is not an ISO date, a change names an unknown position, or factor assignment, sensitivity or decomposition fails.
   */
  positionWhatIf(
    portfolio: Portfolio,
    market: MarketContext,
    factorModelConfig: FactorModelConfig | string,
    asOf: string,
    changes: PositionChange[] | string
  ): WhatIfResult;
  /**
   * Build a credit volatility report from a risk decomposition and a credit factor model.
   *
   * Rolls the decomposition's factor contributions up by credit-hierarchy
   * level. Returns the `CreditVolReport`: `total`, `measure`, the `generic`
   * credit-factor contribution, `by_level` rollups, `idiosyncratic_total` and,
   * when requested, `by_position_optional` rows.
   * @param decomposition - `RiskDecomposition` object or JSON, e.g. the output of `decomposeFactorRisk`.
   * @param model - `models.factor.credit.CreditFactorModel` handle whose hierarchy names the levels.
   * @param byPosition - `true` to include the per-position breakdown; `false` leaves `by_position_optional` `null`.
   * @returns The `CreditVolReport`.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `decomposition` is not a JSON string or plain object or `byPosition` is not a boolean, and a `FinstackError` (kind `validation`) if `decomposition` does not match the `RiskDecomposition` schema.
   */
  buildCreditVolReport(
    decomposition: RiskDecomposition | string,
    model: CreditFactorModel,
    byPosition: boolean
  ): CreditVolReport;
  /**
   * Compute ordered portfolio P&L for a batch of scenarios.
   *
   * The Rust batch engine values the unstressed base leg once for the whole
   * request, then applies and revalues each scenario independently. Returns one
   * `ScenarioPnlBatchItem` (`scenario_id`, `pnl`, `report`) per input scenario,
   * in input order; an empty batch returns `[]` without a valuation.
   * @param portfolio - Built portfolio valued for the shared base and every scenario.
   * @param scenarios - Array of `ScenarioSpec` objects, or its JSON.
   * @param market - `core.MarketContext` handle holding the unshocked market snapshot.
   * @returns The ordered `ScenarioPnlBatchItem` array.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `scenarios` is not a JSON string or array, and a `FinstackError` if a scenario is malformed or inconsistent (kind `validation`), or scenario application, valuation or base-currency differencing fails; the error is the earliest failing scenario's.
   */
  scenarioPnlBatch(
    portfolio: Portfolio,
    scenarios: ScenarioSpec[] | string,
    market: MarketContext
  ): ScenarioPnlBatchItem[];
  /**
   * Attribute portfolio P&L between two market snapshots.
   *
   * Returns the `PortfolioAttribution`: base-currency `total_pnl`, one bucket
   * per factor (`carry`, `rates_curves_pnl`, `credit_curves_pnl`, `fx_pnl`,
   * `vol_pnl`, ..., `residual`) and the per-position `by_position` detail.
   * @param portfolio - Built portfolio whose positions are attributed.
   * @param marketT0 - `core.MarketContext` handle for the opening snapshot.
   * @param marketT1 - `core.MarketContext` handle for the closing snapshot.
   * @param asOfT0 - ISO-8601 date of the opening snapshot.
   * @param asOfT1 - ISO-8601 date of the closing snapshot.
   * @param method - `AttributionMethod` JSON, e.g. `'"parallel"'`, `'"metrics_based"'` or `{ waterfall: ["carry", "rates_curves"] }` (a string argument is JSON text, so a bare variant name keeps its quotes).
   * @param config - Optional `FinstackConfig` object or JSON; omit for the Rust default configuration.
   * @returns The `PortfolioAttribution`.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a `FinstackError` if a date is not an ISO date or `method` / `config` is malformed (kind `validation`), a required FX rate or market datum is missing (kind `not_found`), or a position valuation or attribution fails.
   */
  attributePortfolioPnl(
    portfolio: Portfolio,
    marketT0: MarketContext,
    marketT1: MarketContext,
    asOfT0: string,
    asOfT1: string,
    method: JsonInput,
    config?: JsonInput
  ): PortfolioAttribution;
  /**
   * Render a portfolio P&L attribution as an indented text tree.
   *
   * Twin of Python `PortfolioAttribution.explain` (Rust
   * `PortfolioAttribution::explain`): the total followed by one line per factor
   * bucket with its share of the total.
   * @param attribution - `PortfolioAttribution` object or JSON from `attributePortfolioPnl`.
   * @returns The multi-line explanation.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `attribution` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if it does not match the `PortfolioAttribution` schema.
   */
  portfolioAttributionExplainText(attribution: PortfolioAttribution | string): string;
  /**
   * Check that a portfolio attribution's factor buckets sum to its total P&L.
   *
   * Twin of Python `PortfolioAttribution.reconciliation_check` (Rust
   * `PortfolioAttribution::reconciliation_check`).
   * @param attribution - `PortfolioAttribution` object or JSON from `attributePortfolioPnl`.
   * @param tolerance - Largest absolute residual, in base-currency units, still reported as reconciled.
   * @returns The `ReconciliationReport`: `total_residual`, `is_reconciled` and the `tolerance` applied.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `attribution` is not a JSON string or plain object or `tolerance` is not a number, and a `FinstackError` (kind `validation`) if `attribution` does not match the `PortfolioAttribution` schema.
   */
  portfolioAttributionReconciliationCheck(
    attribution: PortfolioAttribution | string,
    tolerance: number
  ): ReconciliationReport;
  /**
   * Look up one position's value in a portfolio valuation.
   *
   * Twin of Python `PortfolioValuation.get_position_value` (Rust
   * `PortfolioValuation::get_position_value`). The returned
   * `valuation_result` keeps 64-bit fields as `BigInt`, as `valuePortfolio`
   * does.
   * @param valuation - `PortfolioValuation` object or JSON from `valuePortfolio`.
   * @param positionId - Position identifier to look up.
   * @returns The `PositionValue`, or `undefined` when the valuation has no such position.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `valuation` is not a JSON string or plain object or `positionId` is not a string, and a `FinstackError` (kind `validation`) if `valuation` does not match the `PortfolioValuation` schema.
   */
  portfolioValuationGetPositionValue(
    valuation: PortfolioValuation | string,
    positionId: string
  ): PositionValue | undefined;
  /**
   * Look up one entity's aggregated base-currency value in a portfolio valuation.
   *
   * Twin of Python `PortfolioValuation.get_entity_value` (Rust
   * `PortfolioValuation::get_entity_value`).
   * @param valuation - `PortfolioValuation` object or JSON from `valuePortfolio`.
   * @param entityId - Entity identifier to look up.
   * @returns The entity total as `Money`, or `undefined` when the valuation has no such entity.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `valuation` is not a JSON string or plain object or `entityId` is not a string, and a `FinstackError` (kind `validation`) if `valuation` does not match the `PortfolioValuation` schema.
   */
  portfolioValuationGetEntityValue(
    valuation: PortfolioValuation | string,
    entityId: string
  ): MoneyValue | undefined;
  /**
   * Look up one aggregated metric in portfolio metrics.
   *
   * Twin of Python `PortfolioMetrics.get_metric` (Rust
   * `PortfolioMetrics::get_metric`).
   * @param metrics - `PortfolioMetrics` object or JSON from `aggregateMetrics`.
   * @param metricId - Fully qualified metric key.
   * @returns The `AggregatedMetric` (`total` plus `by_entity`), or `undefined` when the metric was not aggregated.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `metrics` is not a JSON string or plain object or `metricId` is not a string, and a `FinstackError` (kind `validation`) if `metrics` does not match the `PortfolioMetrics` schema.
   */
  portfolioMetricsGetMetric(
    metrics: PortfolioMetrics | string,
    metricId: string
  ): AggregatedMetric | undefined;
  /**
   * Look up one position's raw metric values in portfolio metrics.
   *
   * Twin of Python `PortfolioMetrics.get_position_metrics` (Rust
   * `PortfolioMetrics::get_position_metrics`).
   * @param metrics - `PortfolioMetrics` object or JSON from `aggregateMetrics`.
   * @param positionId - Position identifier to look up.
   * @returns The position's `{ metricId: value }` map, or `undefined` when the position has no metrics.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `metrics` is not a JSON string or plain object or `positionId` is not a string, and a `FinstackError` (kind `validation`) if `metrics` does not match the `PortfolioMetrics` schema.
   */
  portfolioMetricsGetPositionMetrics(
    metrics: PortfolioMetrics | string,
    positionId: string
  ): PositionMetrics | undefined;
  /**
   * Look up one metric's portfolio-wide total in portfolio metrics.
   *
   * Twin of Python `PortfolioMetrics.get_total` (Rust
   * `PortfolioMetrics::get_total`).
   * @param metrics - `PortfolioMetrics` object or JSON from `aggregateMetrics`.
   * @param metricId - Fully qualified metric key.
   * @returns The total, or `undefined` when the metric was not aggregated.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `metrics` is not a JSON string or plain object or `metricId` is not a string, and a `FinstackError` (kind `validation`) if `metrics` does not match the `PortfolioMetrics` schema.
   */
  portfolioMetricsGetTotal(metrics: PortfolioMetrics | string, metricId: string): number | undefined;
  /**
   * Trades of an optimization result that open a new position.
   *
   * Twin of Python `PortfolioOptimizationResult.new_position_trades` (Rust
   * `PortfolioOptimizationResultWire::new_position_trades`): the entries of
   * `trades` whose `trade_type` is `new_position`, in trade-list order.
   * @param result - `PortfolioOptimizationResult` object or JSON from `optimizePortfolio`.
   * @returns The matching `TradeSpec` rows; empty for an infeasible solve.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `result` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if it does not match the optimization-result schema.
   */
  portfolioOptimizationResultNewPositionTrades(
    result: PortfolioOptimizationResult | string
  ): TradeSpec[];
  /**
   * Approximately binding constraints of an optimization result.
   *
   * Twin of Python `PortfolioOptimizationResult.binding_constraints` (Rust
   * `PortfolioOptimizationResultWire::binding_constraints`): the constraints
   * whose slack is zero within the solver tolerance.
   * @param result - `PortfolioOptimizationResult` object or JSON from `optimizePortfolio`.
   * @returns `[label, slack]` pairs in slack order; empty for an infeasible solve.
   * @throws Error - Throws a `TypeError` (kind `invalid_type`) if `result` is not a JSON string or plain object, and a `FinstackError` (kind `validation`) if it does not match the optimization-result schema.
   */
  portfolioOptimizationResultBindingConstraints(
    result: PortfolioOptimizationResult | string
  ): [string, number][];
}

/**
 * Namespaced TypeScript entry point for portfolio APIs.
 */
export declare const portfolio: PortfolioNamespace;

// --- scenarios -------------------------------------------------------------

/**
 * Namespaced TypeScript entry points for scenarios calculations and types.
 * @example
 * ```typescript
 * import init, { scenarios } from "finstack-quant-wasm";
 * await init();
 * console.log(scenarios.listBuiltinTemplates());
 * ```
 */
export interface ScenariosNamespace {
  /**
   * Parse and validate a scenario specification from JSON.
   *
   * Returns the validated scenario as a plain JavaScript object.
   * @returns Validated structured scenario specification.
   * @param jsonStr - Canonical JSON string to validate and re-serialize.
   * @throws Error - Rejects malformed or schema-incompatible `json_str`, a blank scenario ID, multiple time-roll operations, invalid operation identifiers or numeric fields, variant-specific operation violations, or serialization failure.
   */
  parseScenarioSpec(jsonStr: JsonInput): ScenarioSpec;
  /**
   * Compose multiple structured scenario specs into a single scenario.
   *
   * Specs are merged in priority order (lower number runs first).
   * @returns Structured composed scenario specification.
   * @param specs - ScenarioSpec objects to validate and compose in priority order.
   * @throws Error - Rejects malformed structured specs, any input spec that fails `ScenarioSpec` validation (blank ID, non-finite numbers, invalid identifiers, tenors or operations; the message names the scenario), input specs with mixed `hazard_bump_mode` values, composition that contains more than one time-roll operation, or failure to convert the composed specification.
   */
  composeScenarios(specs: ScenarioSpec[]): ScenarioSpec;
  /**
   * Validate a scenario specification JSON without executing it.
   *
   * Returns `undefined` when the spec is valid, throws on error. This mirrors
   * the Python `validate_scenario_spec` API, which returns `None` — an invalid
   * spec raises rather than returning a falsy value, so
   * `if (validateScenarioSpec(s))` is not a validity check.
   * @returns nothing; failure is reported by throwing.
   * @param jsonStr - Canonical JSON string to validate and re-serialize.
   * @throws Error - Rejects malformed or schema-incompatible `json_str`, a blank scenario ID, multiple time-roll operations, invalid operation identifiers or numeric fields, or variant-specific operation violations.
   */
  validateScenarioSpec(jsonStr: JsonInput): void;
  /**
   * List all built-in template identifiers.
   *
   * Returns a JSON array of template ID strings.
   * @returns Built-in scenario template identifiers.
   * @throws Error - Rejects if the embedded template registry cannot be parsed and validated, or if its template identifiers cannot be serialized to JavaScript.
   */
  listBuiltinTemplates(): string[];
  /**
   * Get typed metadata for all built-in templates.
   * @returns Metadata objects in deterministic registry order.
   * @throws Error - Rejects if the embedded template registry cannot be parsed and validated, or if its metadata cannot be serialized to JSON.
   */
  listBuiltinTemplateMetadata(): TemplateMetadata[];
  /**
   * Build a scenario spec from a built-in template.
   *
   * Returns a structured `ScenarioSpec`.
   * @returns Validated scenario specification from the built-in template.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @throws Error - Rejects a failure to load the embedded registry, an unknown `template_id`, a template whose resolved scenario fails validation, or failure to serialize the scenario.
   */
  buildFromTemplate(templateId: string): ScenarioSpec;
  /**
   * List component IDs for a built-in composite template.
   *
   * Returns a JS array of component ID strings.
   * @returns Component identifiers of the selected composite template.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @throws Error - Rejects a failure to load the embedded registry, an unknown `template_id`, or component identifiers that cannot be serialized to JavaScript.
   */
  listTemplateComponents(templateId: string): string[];
  /**
   * Build a specific component from a built-in composite template.
   * @returns Validated structured scenario specification for the selected component.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @param componentId - Identifier of a component within the selected composite template.
   * @throws Error - Rejects a failure to load the embedded registry, an unknown `template_id` or `component_id`, a component scenario that fails validation, or failure to serialize the scenario.
   */
  buildTemplateComponent(templateId: string, componentId: string): ScenarioSpec;
  /**
   * Build a scenario spec from fields.
   * @returns Validated structured scenario specification from the supplied fields.
   * @param id - Scenario identifier stored on the constructed spec.
   * @param operations - Structured scenario operation specifications in execution order.
   * @param name - Optional human-readable scenario name.
   * @param description - Optional human-readable description of the scenario purpose.
   * @param priority - Optional execution priority; lower values run earlier during composition. Omit for the Rust serde default (`0`), matching the Python `priority=0` keyword default.
   * @param resolutionMode - Optional hierarchy conflict policy: `"most_specific_wins"` (default) or `"cumulative"`.
   * @param hazardBumpMode - Optional ParCDS delivery: `"solve_to_par"` (default) rebootstraps par quotes; `"first_order_shift"` applies delta hazard = delta spread / (1 - recovery) and reports an approximation warning.
   * @throws Error - Rejects malformed or schema-incompatible `operations`, an unsupported `resolution_mode` or `hazard_bump_mode`, a blank scenario ID, multiple time-roll operations, invalid operation identifiers or numeric fields, variant-specific operation violations, or failure to serialize the scenario.
   */
  buildScenarioSpec(
    id: string,
    operations: OperationSpec[],
    name?: string,
    description?: string,
    priority?: number,
    resolutionMode?: 'most_specific_wins' | 'cumulative',
    hazardBumpMode?: 'solve_to_par' | 'first_order_shift'
  ): ScenarioSpec;
  /**
   * Apply a scenario to a market context and financial model.
   *
   * Returns a JavaScript object with `market` and `model` (the mutated
   * contexts as objects, not JSON strings), `operations_applied`,
   * `user_operations`, `expanded_operations`, `changes` (a
   * `ScenarioChangeManifest`), `warnings`, `meta` (a `ResultsMeta` audit stamp
   * carrying the numeric mode, rounding context, and FX policy; omitted when
   * absent), and `time_roll` (a `RollForwardReport`, only present when the
   * scenario contained a `time_roll_forward` operation).
   *
   * Optional instrument envelopes are copied and returned in `instruments`, in
   * input order. The Rust engine rejects instrument-scoped operations without
   * an inventory. No holiday calendar is supplied; business-day rolls use
   * weekends-only adjustment.
   * @returns Mutated market and optional model after applying the scenario.
   * @param scenarioJson - JSON-serialized ScenarioSpec to validate and apply.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param modelJson - JSON-serialized FinancialModelSpec that scenario operations may mutate.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param instrumentsJson - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
   * @param configJson - Optional FinstackConfig JSON; its rounding policy is stamped into `meta`. Omit for the library default.
   * @throws Error - Rejects a malformed or invalid scenario (checked before the market is parsed), malformed market, model, instrument, or configuration JSON, instrument-scoped operations without `instruments_json`, a model that fails semantic validation (the same `FinancialModelSpec::from_json` check the statements exports and Python `apply_scenario` apply), an invalid ISO `as_of` date, an invalid scenario operation, missing market objects or hierarchy context, statement-model execution failures, failure to encode the mutated contexts, or failure to serialize the application envelope to JavaScript.
   */
  applyScenario(
    scenarioJson: JsonInput,
    marketJson: JsonInput,
    modelJson: JsonInput,
    asOf: string,
    instrumentsJson?: JsonInput,
    configJson?: JsonInput
  ): ApplicationEnvelope;
  /**
   * Apply a scenario to a market context only (no model mutations).
   *
   * Returns the same envelope shape as [`apply_scenario`] minus `model`;
   * the same inventory, configuration and calendar rules apply.
   * @returns Mutated market after applying the scenario.
   * @param scenarioJson - JSON-serialized ScenarioSpec to validate and apply.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param instrumentsJson - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
   * @param configJson - Optional FinstackConfig JSON; its rounding policy is stamped into `meta`. Omit for the library default.
   * @throws Error - Rejects a malformed or invalid scenario (checked before the market is parsed), malformed market, instrument, or configuration JSON, instrument-scoped operations without `instruments_json`, an invalid ISO `as_of` date, an invalid scenario operation, missing market objects or hierarchy context, failure to encode the mutated market, or failure to serialize the application envelope to JavaScript.
   */
  applyScenarioToMarket(
    scenarioJson: JsonInput,
    marketJson: JsonInput,
    asOf: string,
    instrumentsJson?: JsonInput,
    configJson?: JsonInput
  ): ApplicationEnvelope;
  /**
   * Compute horizon total return under a scenario.
   *
   * Applies a scenario specification to project an instrument forward, then
   * decomposes the resulting P&L using factor-based attribution.
   *
   * @param instrumentJson - Canonical `finstack_quant.instrument/1` envelope.
   * @param marketJson - JSON-serialized `MarketContext`.
   * @param asOf - Valuation date (ISO 8601).
   * @param scenarioJson - JSON-serialized `ScenarioSpec`.
   * @param method - Attribution method: "parallel", "waterfall", "metrics_based", "taylor". Omit for the Rust default (`AttributionMethod::default()`, currently "parallel").
   * @param configJson - Optional FinstackConfig JSON for horizon analysis; omit to use defaults.
   * @param calendarId - Optional holiday calendar (e.g. "nyse", "target") used to business-day adjust `time_roll_forward` targets under `business_days` mode. Omit for a weekends-only calendar; unknown identifiers throw.
   * @returns The serde `HorizonResult` (`attribution`, `initial_value` and `terminal_value` as exact-decimal Money objects, `horizon_days`, null without a time roll, and `scenario_report`) plus `summary`: the Rust-computed `total_return`, `annualized_return`, `currency` and `factor_contributions` that Python exposes as `HorizonResult` accessors. Undefined returns (currency mismatch, non-positive initial value) are null here and NaN in Python.
   * @throws Error - Rejects a malformed or invalid scenario; malformed instrument, market, or configuration JSON; an invalid ISO `as_of` date; an unsupported attribution `method`; an unknown `calendar_id`; invalid, unsupported, or unresolved scenario operations; missing market data; pricing or attribution failures; or failure to serialize the horizon result to JavaScript.
   */
  computeHorizonReturn(
    instrumentJson: JsonInput,
    marketJson: JsonInput,
    asOf: string,
    scenarioJson: JsonInput,
    method?: 'parallel' | 'waterfall' | 'metrics_based' | 'taylor',
    configJson?: JsonInput,
    calendarId?: string
  ): HorizonReport;
}

/**
 * Namespaced TypeScript entry point for scenarios APIs.
 */
export declare const scenarios: ScenariosNamespace;
