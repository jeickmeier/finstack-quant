// Host-only TypeScript for the published package, appended to wasm-bindgen's
// pkg/finstack_quant_wasm.d.ts by the `typescript_custom_section` in src/lib.rs.
// It holds what no Rust export declares: the schema-generated types (imported
// so `unchecked_*_type` attributes can name them, and re-exported at the
// package root) and the JavaScript-side shapes the bindings accept or return
// (`JsonInput`, `NumericArray`, `FinstackError`, ...). Paths are relative to
// pkg/, which sits beside src/ and types/.

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
import type * as generated from '../types/generated/index.js';
import type {
  CalibrationEnvelope,
  CalibrationResultEnvelope,
  CalibrationValidationReport,
  StrictLoadDiagnostic,
} from '../types/generated/calibration/index.js';
import type {
  HazardCalibrationRecipe,
  MarketScalar,
  PeriodPlan,
  RateCalibrationRecipe,
  RatingLevel,
  SabrParameterData,
  ScheduleSpec,
  ScheduleWarning,
  ScorecardScale,
  TableColumn,
  TableColumnData,
  TableEnvelope,
} from '../types/generated/core/index.js';
import type {
  DrawdownEpisode,
  LookbackReturns,
  PerformancePanel,
  PeriodicReturn,
} from '../types/generated/analytics/index.js';
import type { CovenantReport } from '../types/generated/covenants/index.js';
import type {
  CrossSectionalOp,
  PairwiseOp,
  PanelOperation,
  PanelTransformColumn,
  PanelTransformResult,
  PanelTransformSpec,
  TimeSeriesOp,
} from '../types/generated/features/index.js';
import type {
  ClearingStatus,
  CollateralAssetClass,
  ConcentrationBreach,
  CorrelationScenario,
  CsaSpec,
  DrcSector,
  DrcSeniority,
  EadResult,
  EligibleCollateralSchedule,
  ExcessCollateral,
  ExposureProfile,
  FrtbRiskClass,
  FrtbSbaResult,
  FrtbSensitivities as FrtbSensitivitiesWire,
  FundingConfig,
  Haircut01,
  ImCollateralResult,
  ImDecayProfile,
  ImMethodology,
  ImProfile,
  ImResult,
  MarginCall,
  MarginCallType,
  MarginConstants,
  MarginFundingCost,
  MarginTenor,
  MarginUtilization,
  MvaResult,
  NettingSetId,
  SaCcrNettingSetConfig,
  SaCcrTrade,
  SimmCurvatureSensitivity,
  SimmSensitivitiesJson,
  VmResult,
  XvaResult,
} from '../types/generated/margin/index.js';
import type {
  ArbitrageValidationResult,
  SabrCalibrationOutcome,
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
  OptionCalculation,
  OptionCalculationResult,
  PositionEsContribution,
  PositionFactorContribution,
  PositionRiskDecomposition,
  PositionRiskReport,
  PositionRiskRequest,
  PositionVarContribution,
  RiskDecomposition,
  RiskMeasure,
  SviParams,
  TrancheLossStatistics,
  VolatilityConvention,
} from '../types/generated/models/index.js';
import type {
  BorrowingBaseReport,
  CompositeExposureReport,
  CompositeHistoryRow,
  CompositeRebalanceResult,
  CompositeTrade,
  EnhancedMonteCarloResult,
  CurrencyLadder,
  ListedProductCoverage,
  MetricFieldUnit,
  MetricMetadata,
  OasResult,
  PoolReportFieldUnits,
  ScenarioTableFieldUnits,
  SimulationFieldUnits,
  EquityMetricsFieldUnits,
  StandardDealFamily,
  TrancheSolveFieldUnits,
  PrimitiveAggregate,
  PrimitiveExposure,
  ResultsMeta,
  ScenarioCell,
  ScenarioTable,
  TrancheCashflows,
  TrancheMetrics,
  ToleranceConfig,
  ValidationReport,
} from '../types/generated/valuations/index.js';
import type {
  AttributionInputs,
  AttributionSpec,
  AttributionResultEnvelope,
  PnlAttribution,
  PnlBucketShare,
} from '../types/generated/attribution/index.js';
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
} from '../types/generated/statements/index.js';
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
} from '../types/generated/statements_analytics/index.js';
import type {
  AggregatedMetric,
  BrinsonPeriodResult,
  CarinoLinkedAttribution,
  CreditVolReport,
  DurationCellTable,
  Entity,
  ExcessReturnResult,
  FactorAssignmentReport,
  FactorBrinsonResult,
  FactorPnlProfile,
  FiAttributionResult,
  FiCarinoLinkedResult,
  GridAttributionResult,
  GridCarinoLinkedResult,
  LinkedReturn,
  MaterializationReport,
  NettingSetMargin,
  PortfolioAttribution,
  PortfolioCashflows,
  PortfolioMarginResult,
  PortfolioMetrics,
  PortfolioPrimitiveExposureReport,
  PositionChange,
  PositionMetrics,
  PositionSpec,
  ReconciliationReport,
  ScenarioPnlBatchItem,
  SensitivityMatrixJson,
  StressPnl,
  StressResult,
  TradeSpec,
  WeightAllocationResult,
  WeightAllocationSpec,
  WhatIfResult,
} from '../types/generated/portfolio/index.js';
import type {
  ApplicationEnvelope,
  ApplicationReport,
  AttributionFactor,
  Compounding,
  CurveKind,
  HierarchyTarget,
  HorizonResult,
  HorizonSummary,
  OperationSpec,
  RateBindingSpec,
  RollForwardReport,
  ScenarioChangeManifest,
  ScenarioSpec,
  TemplateMetadata,
  TenorMatchMode,
  TimeRollMode,
  Warning,
} from '../types/generated/scenarios/index.js';

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
  CdsTrancheQuote,
  InflationQuote,
  MarketDatum,
  PriorMarketObject,
  StepParams,
  XccyQuote,
} from '../types/generated/calibration/index.js';
/**
 * Payload of a `MarketDatum` tagged `kind: "fx_spot"` (Rust `FxSpotDatum`); twin of Python `calibration.FxSpotDatum`.
 */
export type FxSpotDatum = Omit<
  Extract<generated.calibration.MarketDatum, { kind: 'fx_spot' }>,
  'kind'
>;
/**
 * Payload of a `MarketDatum` tagged `kind: "price"` (Rust `PriceDatum`); twin of Python `calibration.PriceDatum`.
 */
export type PriceDatum = Omit<
  Extract<generated.calibration.MarketDatum, { kind: 'price' }>,
  'kind'
>;
/**
 * Payload of a `MarketDatum` tagged `kind: "collateral"` (Rust `CollateralEntry`); twin of Python `calibration.CollateralEntry`.
 */
export type CollateralEntry = Omit<
  Extract<generated.calibration.MarketDatum, { kind: 'collateral' }>,
  'kind'
>;
export type {
  MarketScalar,
  PeriodPlan,
  ScheduleSpec,
  ScheduleWarning,
  ScorecardScale,
  TableColumn,
  TableColumnData,
  TableEnvelope,
  ToleranceConfig,
};
export type { Period, RatingLevel } from '../types/generated/core/index.js';
export type { DrawdownEpisode, LookbackReturns, PerformancePanel, PeriodicReturn };
export type { CovenantReport };
export type {
  CrossSectionalOp,
  PairwiseOp,
  PanelOperation,
  PanelTransformColumn,
  PanelTransformResult,
  PanelTransformSpec,
  TimeSeriesOp,
};
export type {
  ClearingStatus,
  CollateralAssetClass,
  ConcentrationBreach,
  CorrelationScenario,
  CsaSpec,
  DrcSector,
  DrcSeniority,
  EadResult,
  EligibleCollateralSchedule,
  ExcessCollateral,
  ExposureProfile,
  FrtbRiskClass,
  FrtbSbaResult,
  FundingConfig,
  Haircut01,
  ImCollateralResult,
  ImDecayProfile,
  ImMethodology,
  ImProfile,
  ImResult,
  MarginCall,
  MarginCallType,
  MarginConstants,
  MarginFundingCost,
  MarginTenor,
  MarginUtilization,
  MvaResult,
  NettingSetId,
  SaCcrNettingSetConfig,
  SaCcrTrade,
  SimmCurvatureSensitivity,
  SimmSensitivitiesJson,
  VmResult,
  XvaResult,
};
export type {
  ArbitrageValidationResult,
  SabrCalibrationOutcome,
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
  OptionCalculation,
  OptionCalculationResult,
  PositionEsContribution,
  PositionFactorContribution,
  PositionRiskDecomposition,
  PositionRiskReport,
  PositionRiskRequest,
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
  CurrencyLadder,
  ListedProductCoverage,
  MetricFieldUnit,
  MetricMetadata,
  OasResult,
  PoolReportFieldUnits,
  ScenarioTableFieldUnits,
  SimulationFieldUnits,
  EquityMetricsFieldUnits,
  StandardDealFamily,
  TrancheSolveFieldUnits,
  PrimitiveAggregate,
  PrimitiveExposure,
  ResultsMeta,
  ScenarioCell,
  ScenarioTable,
  TrancheCashflows,
  TrancheMetrics,
  ValidationReport,
};
export type { Diagnostic } from '../types/generated/valuations/index.js';
export type {
  AttributionInputs,
  AttributionSpec,
  AttributionResultEnvelope,
  PnlAttribution,
  PnlBucketShare,
};
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
  FactorAssignmentReport,
  FactorBrinsonResult,
  FactorPnlProfile,
  FiAttributionResult,
  FiCarinoLinkedResult,
  GridAttributionResult,
  GridCarinoLinkedResult,
  LinkedReturn,
  MaterializationReport,
  NettingSetMargin,
  PortfolioAttribution,
  PortfolioCashflows,
  PortfolioMarginResult,
  PortfolioMetrics,
  PortfolioPrimitiveExposureReport,
  PositionChange,
  PositionMetrics,
  PositionSpec,
  ReconciliationReport,
  ScenarioPnlBatchItem,
  SensitivityMatrixJson,
  StressPnl,
  StressResult,
  TradeSpec,
  WeightAllocationResult,
  WeightAllocationSpec,
  WhatIfResult,
};
export type { MaterializationPhases } from '../types/generated/portfolio/index.js';
export type {
  ApplicationEnvelope,
  ApplicationReport,
  AttributionFactor,
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

type HostValuationResult = import('../types/valuation-result.js').ValuationResult;

/**
 * Valuation envelope returned by the `priceInstrument*` entry points (Rust `ValuationResult`).
 *
 * Generated from the host valuation-result schema: the 64-bit Monte Carlo
 * `seed` is a lossless `bigint` and path counts are plain numbers, so serialize
 * stochastic results with `valuations.valuationResultToJson` rather than
 * `JSON.stringify`. The emitted-field presence is stricter than the schema's
 * input view: `covenants` is always present (`null` when the instrument has
 * none), while `details`, `explanation` and `provenance` are omitted rather
 * than `null` when absent.
 * `provenance` is stamped on every priced result.
 */
export type ValuationResult = Omit<
  HostValuationResult,
  'covenants' | 'details' | 'explanation' | 'provenance'
> & {
  covenants: NonNullable<HostValuationResult['covenants']> | null;
  details?: NonNullable<HostValuationResult['details']>;
  explanation?: NonNullable<HostValuationResult['explanation']>;
  provenance?: NonNullable<HostValuationResult['provenance']>;
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
 * Solution of a portfolio optimization problem (Rust `PortfolioOptimizationResult`).
 */
export type PortfolioOptimizationResult = generated.portfolio.PortfolioOptimizationResult;

/**
 * A dated amount (Rust `DatedFlowJson`); in `CashFlowSchedule.outstandingByDate`
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
  benchmark_relative?: Record<string, unknown> | null;
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

// --- core -----------------------------------------------------------------

// wasm-bindgen declares `[Symbol.dispose](): void` on every class (an alias of
// `free()`, installed only when the host defines the symbol). Declaring the
// symbol here, as `@types/node` does, keeps the package's declarations valid
// for consumers compiling against `lib: ["ES2020"]` without
// `esnext.disposable`; it merges with that library's declaration when present.
declare global {
  interface SymbolConstructor {
    readonly dispose: unique symbol;
  }
}

/**
 * Lifecycle contract for a WebAssembly-backed value that owns a wasm heap allocation.
 */
export interface WasmOwned {
  /**
   * Release the underlying wasm heap allocation. Do not use this handle afterward.
   */
  free(): void;
}

/**
 * 30/360 day-count variant label accepted by `core.days30360` (Rust
 * `Thirty360Convention`; parsing is ASCII case-insensitive).
 */
export type Thirty360Convention = 'us_sia' | 'isda' | 'european' | 'italian';

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
   * Registered holiday calendar of the time axis (e.g. `"brbd"`); required with `"bus_252"`.
   */
  calendarId?: string;
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
   * Extrapolation policy outside the knots; the Rust builder default is `"flat_zero"`, which holds the first and last forward rate flat (market-standard projection-curve tail). `"flat_forward"` extends the boundary rate slope instead.
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
 * Identifier, display name and weekend rule of a built-in holiday calendar
 * (the Rust `CalendarMetadata`), as returned by `HolidayCalendar.metadata`.
 */
export interface CalendarMetadata {
  /**
   * Lowercase registry identifier of the calendar, such as `"nyse"`.
   */
  id: string;
  /**
   * Human-readable name of the calendar.
   */
  name: string;
  /**
   * Whether weekends are ignored when classifying holidays.
   */
  ignore_weekends: boolean;
}

/**
 * Named options for constructing an `InflationCurve`; unknown keys are rejected.
 */
export interface InflationCurveOptions {
  /**
   * Curve identifier; the lookup key inside a `MarketContext`.
   */
  id: string;
  /**
   * ISO-8601 base date; knot times are year fractions from it under `dayCount`.
   */
  baseDate: string;
  /**
   * Strictly positive CPI index level at the base date.
   */
  baseCpi: number;
  /**
   * Flat `[t0, cpi0, t1, cpi1, …]` pairs: `t` in years, `cpi` the projected index level; even length.
   */
  knots: NumericArray;
  /**
   * Day-count convention for the time axis; omitted uses the Rust builder default.
   */
  dayCount?: string;
  /**
   * Indexation (publication) lag in whole months; omitted uses the Rust builder default.
   */
  indexationLagMonths?: number;
  /**
   * Interpolation style between pillars; omitted uses the Rust builder default.
   */
  interp?: string;
  /**
   * Extrapolation policy beyond the pillar range; omitted uses the Rust builder default.
   */
  extrapolation?: string;
}

/**
 * Named options for constructing a `PriceCurve`; unknown keys are rejected.
 */
export interface PriceCurveOptions {
  /**
   * Curve identifier; the lookup key inside a `MarketContext`.
   */
  id: string;
  /**
   * ISO-8601 base date; knot times are year fractions from it under `dayCount`.
   */
  baseDate: string;
  /**
   * Flat `[t0, p0, t1, p1, …]` pairs: `t` in years, `p` the forward price or index level; even length.
   */
  knots: NumericArray;
  /**
   * Level family: `"price"` (the default, any finite level) or `"vol_index"` (non-negative levels).
   */
  kind?: 'price' | 'vol_index';
  /**
   * Spot level at `t = 0`; omitted uses the Rust builder default.
   */
  spotPrice?: number;
  /**
   * Extrapolation policy beyond the pillar range; omitted uses the Rust builder default.
   */
  extrapolation?: string;
  /**
   * Interpolation style between pillars; omitted uses the Rust builder default.
   */
  interp?: string;
  /**
   * Day-count convention for the time axis; omitted uses the Rust builder default.
   */
  dayCount?: string;
}

/**
 * Optional keys of the `VolSurface` constructor; every omitted key takes the Rust default.
 */
export interface VolSurfaceOptions {
  /**
   * What `strikes` holds: `"strike"` (default) or `"tenor"`.
   */
  secondaryAxis?: string | null;
  /**
   * `"vol"` (default) or `"total_variance"`.
   */
  interpolationMode?: string | null;
  /**
   * `"black_lognormal"` (default), `"shifted_black_lognormal"` or `"normal"`.
   */
  quoteType?: string | null;
  /**
   * One shifted-Black displacement per expiry; required for `"shifted_black_lognormal"` quotes.
   */
  displacements?: NumericArray | null;
}

/**
 * Counts of the market data held by a `MarketContext` (the Rust
 * `ContextStats`), as returned by `MarketContext.stats()`.
 */
export interface ContextStats {
  /**
   * Number of curves per curve type, keyed by the type label.
   */
  curve_counts: Record<string, number>;
  /**
   * Total number of curves of every type.
   */
  total_curves: number;
  /**
   * Whether an FX matrix is attached.
   */
  has_fx: boolean;
  /**
   * Number of volatility surfaces.
   */
  surface_count: number;
  /**
   * Number of SABR volatility cubes.
   */
  vol_cube_count: number;
  /**
   * Number of market scalars (prices and unitless values).
   */
  price_count: number;
  /**
   * Number of scalar time series.
   */
  series_count: number;
  /**
   * Number of inflation indices.
   */
  inflation_index_count: number;
  /**
   * Number of credit indices.
   */
  credit_index_count: number;
  /**
   * Number of dividend schedules.
   */
  dividend_schedule_count: number;
  /**
   * Number of FX delta-quoted volatility surfaces.
   */
  fx_delta_vol_surface_count: number;
}

/**
 * Any curve or surface handle that `MarketContext.insert` stores.
 */
export type MarketContextCurve =
  | DiscountCurve
  | ForwardCurve
  | HazardCurve
  | InflationCurve
  | PriceCurve
  | BaseCorrelationCurve
  | VolSurface
  | FxDeltaVolSurface
  | VolCube;

/**
 * Rendering of a published JSON Schema: the validation contract, or the
 * self-contained projection for structured-output generation.
 */
export type SchemaProfile = 'canonical' | 'llm';

/**
 * One published JSON Schema, as listed by a `schema.index()` call.
 */
export interface SchemaIndexRow {
  /**
   * Owning domain (`"core"`, `"valuations"`, …). Present only in the workspace-wide `schema.index()`.
   */
  domain?: string;
  /**
   * Registry path of the schema, e.g. `"schemas/dates/1/period_plan.schema.json"`.
   */
  path: string;
  /**
   * Absolute `$id` of the schema document.
   */
  $id: string;
  /**
   * `title` of the schema document.
   */
  title: string;
  /**
   * Rust root type the schema describes.
   */
  type_name: string;
  /**
   * One-line description of the contract.
   */
  summary: string;
  /**
   * Size of the canonical rendering in bytes.
   */
  bytes: number;
  /**
   * `input` for documents you author, `output` for documents the library emits, `component` for shared definitions.
   */
  kind: 'input' | 'output' | 'component';
}

/**
 * Result of a `schema.index()` call.
 */
export interface SchemaIndex {
  /**
   * One row per published schema.
   */
  artifacts: SchemaIndexRow[];
  /**
   * Version of the index layout.
   */
  schema_index_version: number;
}

/**
 * One failure reported by a `schema.validate()` call.
 */
export interface SchemaValidationFailure {
  /**
   * JSON Pointer into the payload naming the value that failed; empty for the document root.
   */
  pointer: string;
  /**
   * What was wrong with that value.
   */
  message: string;
}

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
 * Dated rolling result derived from the Rust schema, with its numeric column
 * exposed as a typed array. Undefined windows contain `NaN`.
 * Dates and metric labels retain the generated Rust wire contract.
 */
export type DatedSeries = WithFields<
  generated.analytics.DatedSeries,
  {
    /** Rolling metric values, one per completed window. */
    values: Float64Array;
  }
>;

/**
 * Rolling greeks derived from the Rust schema, with numeric columns exposed as
 * typed arrays aligned with the generated window-end dates.
 */
export type RollingGreeks = WithFields<
  generated.analytics.RollingGreeks,
  {
    /** Annualized Jensen alpha at each window end, as decimal fractions. */
    alphas: Float64Array;
    /** OLS beta at each window end. */
    betas: Float64Array;
  }
>;

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
 * Constructor and factory functions of `LatentFactorKind`.
 */
export interface LatentFactorKindClass {
  /**
   * JavaScript prototype of `LatentFactorKind`; instances are returned by other `correlation` calls.
   */
  readonly prototype: LatentFactorKind;
}

/**
 * Convergence and reproducibility diagnostics for a Monte Carlo valuation.
 */
export type MonteCarloValuationDetails =
  import('../types/valuation-result.js').MonteCarloValuationDetails;

/**
 * Model-specific native detail, generated from Rust with exact host integer representations.
 */
export type ValuationDetails = import('../types/valuation-result.js').ValuationDetails;

/**
 * The flattened credit-model fields of a structured-credit deal or asset-backed facility (Rust `CreditModelConfig`).
 */
export type CreditModelConfig = Pick<
  generated.valuations.StructuredCredit,
  | 'prepayment_spec'
  | 'default_spec'
  | 'recovery_spec'
  | 'stochastic_prepay_spec'
  | 'stochastic_default_spec'
  | 'stochastic_recovery_spec'
  | 'correlation_structure'
  | 'delinquency'
  | 'card'
>;

/**
 * Chronological composite history rows returned by `valuations.composite.history` and `historyFromSpec`.
 */
export type CompositeHistoryResult = CompositeHistoryRow[];

/**
 * Members shared by every FX instrument handle (`id`, `toJson`, `price`, `metric`).
 */
export interface FxHandle extends WasmOwned {
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
   * @param metricPricingOverrides - Optional JSON object patch applied before validation. Only supplied fields replace stored overrides; `{}` preserves them. Supplied `bump_config` fields merge into the stored object. Explicit `null` clears nullable fields to their Rust fallback. Omit the argument or pass JavaScript `null`/`undefined` to retain the envelope configuration.
   * @param marketHistory - Optional serialized market-history JSON required by historical risk metrics such as historical VaR.
   * @param explain - Optional boolean; `true` attaches a step trace as `result.explanation`. Omit or pass `false` for no trace.
   * @returns Structured `ValuationResult` for the selected model, preserving Monte Carlo seeds as lossless JavaScript `BigInt` values.
   * @throws Error - Throws a JavaScript exception if the instrument, market, metric-pricing-override, or market-history JSON is invalid; `metrics` is not a string array; `asOf`, `model`, or a metric identifier is invalid; required market data is missing; pricing or a metric calculation fails; or the valuation cannot be converted to JavaScript.
   */
  price(
    marketJson: JsonInput,
    asOf: string,
    model?: string | null,
    metrics?: string[] | null,
    metricPricingOverrides?: JsonInput | null,
    marketHistory?: JsonInput | null,
    explain?: boolean | null
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
 * Optional arguments of `FxForward.fromTradeDate`; omitted fields take the Rust defaults.
 */
export interface FxForwardFromTradeDateOptions {
  /**
   * Base-currency holiday calendar; omitted counts weekdays only, except that a USD leg uses `"usny"`.
   */
  base_calendar_id?: string | null;
  /**
   * Quote-currency holiday calendar; omitted counts weekdays only, except that a USD leg uses `"usny"`.
   */
  quote_calendar_id?: string | null;
  /**
   * Contractual T+N spot lag in business days, honored as given (e.g. `1` for tom-next); omitted uses the pair's market standard.
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
 * FX vanilla option handle with first-order greeks.
 */
export interface FxOptionGreeks extends FxHandle {
  /**
   * Spot delta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Cash delta `dPV/dS`: PV change per unit move in the spot rate, including notional scaling.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; or the Greek is not produced by the selected model.
   */
  delta(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Spot gamma of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Cash gamma `d²PV/dS²`: PV per unit-spot squared, including notional scaling.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; or the Greek is not produced by the selected model.
   */
  gamma(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Vega of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Cash vega: PV change for a 1 vol-point (0.01 absolute) move in implied volatility.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; or the Greek is not produced by the selected model.
   */
  vega(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Theta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Model theta per day: annual analytic theta divided by the `metric_pricing_overrides.theta_day_basis` day count (default 365 calendar days); not annualized.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; or the Greek is not produced by the selected model.
   */
  theta(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Domestic-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Domestic rho: PV change for a 1bp (0.0001) move in the domestic discount rate.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; or the Greek is not produced by the selected model.
   */
  rho(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Foreign-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Foreign rho: PV change for a 1bp (0.0001) move in the foreign discount rate.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; or the Greek is not produced by the selected model.
   */
  foreignRho(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Vanna of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vanna `d²PV/(dS dσ)`: PV per unit spot per vol point (0.01 absolute vol).
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; or the Greek is not produced by the selected model.
   */
  vanna(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Volga of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Volga `d²PV/dσ²`: PV per vol-point squared (0.01 absolute vol).
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; or the Greek is not produced by the selected model.
   */
  volga(marketJson: JsonInput, asOf: string, model?: string | null): number;
  /**
   * Named first-order greeks produced by the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Map of Greek name to value, with keys in the Rust `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`, …); only the Greeks that apply to the instrument are present.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON, `asOf`, or `model` is invalid; required market data is missing; pricing fails; a returned Greek is non-finite; or the result cannot be converted to a JavaScript value.
   */
  greeks(marketJson: JsonInput, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * Factories for typed FX instrument handles. A bare spec becomes a handle
 * through `fromJson(valuations.instruments.instrumentEnvelopeFromSpec(type, spec))`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const envelope = valuations.instruments.instrumentEnvelopeFromSpec("fx_spot", {
 *   id: "EURUSD-SPOT",
 *   base_currency: "EUR",
 *   quote_currency: "USD",
 *   settlement_date: "2025-01-17",
 *   quoted_spot: 1.1,
 *   notional: { amount: "1000000", currency: "EUR" },
 *   attributes: {},
 * });
 * const spot = valuations.instruments.FxSpot.fromJson(envelope);
 * console.log(JSON.parse(spot.toJson()).instrument.type);
 * spot.free();
 * ```
 */
export interface FxConstructor<T extends FxHandle> {
  /**
   * JavaScript prototype of the FX class; instances come from the static factories, not `new`.
   */
  readonly prototype: T;
  /**
   * Parse an FX instrument handle from canonical JSON.
   * @param json - Canonical FX instrument JSON accepted by `fromJson`.
   * @returns A typed FX instrument handle.
   */
  fromJson(json: JsonInput): T;
}

/**
 * Constructor and factory functions of `WorkoutLgdBuilder`.
 */
export interface WorkoutLgdBuilderClass {
  /**
   * JavaScript prototype of `WorkoutLgdBuilder`; instances are returned by other `credit` calls.
   */
  readonly prototype: WorkoutLgdBuilder;
}

/**
 * A structured input: JSON text, or the equivalent plain object or array.
 *
 * Objects are written to JSON by the binding before Rust parses them, so the
 * Rust type's unknown-field checks apply either way. `NaN`/`Infinity`, array
 * holes, functions, `Map`/`Date`/class instances and WASM handles throw a
 * `TypeError` with `kind: "invalid_type"`; `bigint` values are written as
 * exact integers and typed arrays as arrays.
 *
 * The object arm is `object`, not `Record<string, unknown>`, so a value typed
 * as one of the generated interfaces (the `FinancialModelSpec` a builder
 * returns, a `MarketContextState`) is accepted without a cast.
 */
export type JsonInput = string | object;

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
 * Structured error thrown by `calibration.calibrate`, `validateCalibration`,
 * `dryRun`.
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
