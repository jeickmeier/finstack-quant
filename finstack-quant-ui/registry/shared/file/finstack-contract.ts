import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import type { RoundingStamp } from "@/lib/finstack/format/format";
import type {
  ValuationResult,
  CalibrationResultEnvelope,
  CalibrationEnvelope,
  CalibrationValidationReport,
  MetricMetadata,
  MoneyValue,
  ScenarioTable,
  ScenarioResults,
  TrancheMetrics,
  OasResult,
  StatementResult,
  Explanation,
  DependencyTree,
  CheckReport,
  CalendarYearLadderRow,
  CreditAssessment,
  DcfSensitivityResult,
  LboResult,
  SensitivityResult,
  TornadoEntry,
  GoalSeekResult,
  VarianceReport,
  ForecastMetrics,
  ArbitrageValidationResult,
  ApplicationReport,
  HorizonReport,
  PnlAttribution,
  BetaResult,
  DatedSeries,
  DrawdownEpisode,
  GreeksResult,
  MultiFactorResult,
  RollingGreeks,
  LookbackReturns,
  PeriodicReturn,
  TableEnvelope,
  PortfolioMetrics,
  PortfolioMetricSeriesEntry,
  PortfolioValuation,
  ScenarioPnlBatchItem,
  ScenarioRevalueView,
  PortfolioMarginResult,
  ImResult,
  CsaSpec,
  VmResult,
  MarginCall,
  PositionRiskDecomposition,
  RiskBudgetResult,
  RiskDecomposition,
  SensitivityMatrixJson,
  models,
  statements_analytics,
  valuations,
} from "finstack-quant-wasm";
import type {
  attribution as attributionWire,
  core as wire,
  models as modelsWire,
  scenarios as scenarioWire,
} from "finstack-quant-wasm/types";
/** Complete immutable facade request; JSON strings retain wide integer tokens. */
export interface PriceRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  readonly asOf: string;
  readonly model?: string | null;
  readonly metrics?: readonly string[] | null;
  readonly metricPricingOverrides?: string | null;
  readonly marketHistory?: string | null;
}
/** Exact source amount plus the returned rounding stamp; both are snapshotted verbatim and never recomputed. */
export interface MoneyFormatRequest {
  readonly value: MoneyValue;
  readonly rounding?: RoundingStamp;
}
export interface ScenarioRequest {
  readonly instrumentJson: string;
  readonly trancheId: string;
  readonly marketJson: string;
  readonly asOf: string;
  /** Complete native ScenarioGrid JSON; no model inputs are inferred. */
  readonly gridJson: string;
}
/** One tranche of a structured-credit deal, priced on a complete market at a valuation date. */
export interface TrancheRequest {
  readonly instrumentJson: string;
  readonly trancheId: string;
  readonly marketJson: string;
  readonly asOf: string;
}
/** Tranche metrics; an omitted price or metric list is left to Rust (deal quote, else model price; no registry metrics). */
export interface TrancheMetricsRequest extends TrancheRequest {
  /** Clean settlement price, percent of the tranche's current balance. */
  readonly marketPricePct?: number | null;
  /** Registry metric ids valued on this note. */
  readonly metrics?: readonly string[] | null;
}
/** A tranche solve against an explicit clean price, percent of the tranche's current balance. */
export interface TranchePriceRequest extends TrancheRequest {
  readonly marketPricePct: number;
}
/** A tranche OAS solve; an omitted configuration is left to Rust's default OasConfig. */
export interface TrancheOasRequest extends TranchePriceRequest {
  /** Complete native OasConfig JSON. */
  readonly configJson?: string | null;
}
export interface CashflowRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  readonly asOf: string;
  readonly model: string;
}
/** Exact statement evaluation identity; market and date are supplied together. */
export interface StatementRequest {
  readonly modelJson: string;
  readonly marketJson?: string;
  readonly asOf?: string;
}
/** Match the model and its already evaluated result for diagnostics. */
export interface StatementExplanationRequest {
  readonly modelJson: string;
  readonly resultsJson: string;
  readonly nodeId: string;
  readonly period: string;
}
/** Root node whose dependency tree is traced in modelJson. */
export interface StatementTraceRequest {
  readonly modelJson: string;
  readonly nodeId: string;
}
export interface StatementChecksRequest {
  readonly modelJson: string;
  readonly resultsJson: string;
  readonly kind: "suite" | "three-statement" | "credit-underwriting";
  readonly configJson: string;
}
/** Native ScenarioSet JSON evaluated against one model; market and date are supplied together. */
export interface StatementScenarioRequest {
  readonly modelJson: string;
  readonly scenarioSetJson: string;
  readonly marketJson?: string;
  readonly asOf?: string;
}
/** One baseline/comparison pair of scenario names, both keyed in the scenario set. */
export interface StatementScenarioPair {
  readonly baseline: string;
  readonly comparison: string;
}
/** Native ScenarioSet JSON, its evaluated results and the pairs, metrics and periods to diff. */
export interface StatementScenarioDiffRequest {
  readonly scenarioSetJson: string;
  /** Host-serialized scenario name to StatementResult map from evaluateStatementScenarios. */
  readonly resultsJson: string;
  readonly pairs: readonly StatementScenarioPair[];
  /** Node ids in row order; every one must have a value at every period in both scenarios. */
  readonly metrics: readonly string[];
  readonly periods: readonly string[];
}
/** One scenario of native ScenarioSet JSON whose inheritance lineage is traced. */
export interface StatementScenarioTraceRequest {
  readonly scenarioSetJson: string;
  readonly scenario: string;
}
/** Native variance-style diff of one scenario pair (not a top-level package export). */
export type StatementScenarioDiff = ReturnType<
  typeof statements_analytics.scenarioDiff
>;
/** An evaluated StatementResult and the period its credit assessment is read at. */
export interface CreditAssessmentRequest {
  /** Host-serialized StatementResult from evaluateStatement. */
  readonly resultsJson: string;
  /** Statement period identifier, such as `2025Q4`. */
  readonly period: string;
}
/** An evaluated StatementResult and the line items and periods of its P&L summary table. */
export interface PlSummaryReportRequest {
  /** Host-serialized StatementResult from evaluateStatement. */
  readonly resultsJson: string;
  /** Node ids in row order. */
  readonly lineItems: readonly string[];
  /** Period identifiers in column order. */
  readonly periods: readonly string[];
}
/** Rust TerminalValueSpec as the facade takes it: Gordon growth, exit multiple or H-model. */
export type TerminalValueSpec = Exclude<
  Parameters<typeof statements_analytics.evaluateDcf>[2],
  string
>;
/** DCF assumptions for one statement model; an omitted optional argument stays unset so Rust applies its default. */
export interface DcfSensitivityRequest {
  readonly modelJson: string;
  /** Weighted average cost of capital as a decimal (0.10 = 10%). */
  readonly wacc: number;
  readonly terminalValue: TerminalValueSpec;
  /** Node of monetary unlevered free cash flow; omitted uses Rust's `ufcf`. */
  readonly ufcfNode?: string;
  /** Flat net debt in model currency in place of the model's debt and cash. */
  readonly netDebtOverride?: number;
  /** Rust DcfOptions JSON; Rust rejects unknown keys. */
  readonly optionsJson?: string;
}
/** A DCF of the statement model; `asOf` is the valuation date and Rust requires it when `marketJson` is supplied. */
export interface EvaluateDcfRequest extends DcfSensitivityRequest {
  readonly marketJson?: string;
  readonly asOf?: string;
}
/** Native CorporateValuationResult (not a top-level package export). */
export type CorporateValuationResult = ReturnType<
  typeof statements_analytics.evaluateDcf
>;
/** A leveraged buyout of the statement model under Rust LboConfig JSON. */
export interface EvaluateLboRequest {
  readonly modelJson: string;
  /** Rust LboConfig JSON; Rust rejects unknown keys. */
  readonly configJson: string;
}
/** One sensitivity parameter: statements_analytics.parameterSpecWithPercentages arguments. */
export interface SensitivityParameterRequest {
  readonly nodeId: string;
  /** Period whose value is varied, such as `2025Q4`. */
  readonly periodId: string;
  /** Base value of the node at the period, in the node's own units, read from the evaluated result. */
  readonly baseValue: number;
  /** Percentage moves of the base value, such as `[-10, 10]` for plus or minus 10%. */
  readonly pctRange: readonly number[];
}
/** A sensitivity run of one model and the target metric and period its tornado reads. */
export interface RunSensitivityRequest {
  readonly modelJson: string;
  /** Rust SensitivityMode wire value: `diagonal`, `full_grid` or `tornado`. */
  readonly mode: string;
  /** Parameters in the order Rust varies them. */
  readonly parameters: readonly SensitivityParameterRequest[];
  /** Target metric node tracked in every scenario and read by the tornado. */
  readonly targetMetric: string;
  /** Period at which the tornado reads the target metric. */
  readonly period: string;
}
/** Native SensitivityResult and its tornado entries; `tornado` is null for a full-grid run, which Rust cannot rank. */
export interface StatementSensitivity {
  readonly result: SensitivityResult;
  readonly tornado: TornadoEntry[] | null;
}
/** statements_analytics.goalSeek arguments; `bounds` stays unset unless supplied. */
export interface GoalSeekRequest {
  readonly modelJson: string;
  readonly targetNode: string;
  readonly targetPeriod: string;
  /** Value the target node must reach, in its own units. */
  readonly targetValue: number;
  readonly driverNode: string;
  readonly driverPeriod: string;
  /** Whether the result carries the model with the solved driver written in. */
  readonly updateModel: boolean;
  /** `[lower, upper]` search bracket in the driver node's units. */
  readonly bounds?: readonly [number, number];
}
/** Two evaluated StatementResults compared under a Rust VarianceConfig. */
export interface RunVarianceRequest {
  /** Host-serialized baseline StatementResult. */
  readonly baseJson: string;
  /** Host-serialized comparison StatementResult. */
  readonly comparisonJson: string;
  /** Rust VarianceConfig JSON: labels, metrics and periods. */
  readonly configJson: string;
}
/** One target metric's variance between two evaluated results, explained by driver nodes. */
export interface VarianceBridgeRequest {
  readonly baseJson: string;
  readonly comparisonJson: string;
  readonly targetMetric: string;
  /** Period identifier, such as `2025Q4`. */
  readonly period: string;
  /** Driver node ids, in the order Rust reports their contributions. */
  readonly drivers: readonly string[];
  readonly baselineLabel: string;
  readonly comparisonLabel: string;
}
/** Native BridgeChart (not a top-level package export). */
export type BridgeChart = ReturnType<
  typeof statements_analytics.varianceBridge
>;
/** Realized and forecast values aligned one-for-one, in the same units. */
export interface BacktestForecastRequest {
  readonly actual: readonly number[];
  readonly forecast: readonly number[];
}
/** Roll an evaluated result of `modelJson` up to the native RollupSpec's frequency. */
export interface StatementRollupRequest {
  readonly modelJson: string;
  readonly resultsJson: string;
  readonly specJson: string;
}
/** Native cashflow envelope object returned by the facade's instrument cashflow export. */
export type CashflowEnvelope = ReturnType<
  typeof valuations.instruments.instrumentCashflowsWithMarket
>;
/** One row currency's native calendar-year ladder, in the units of that currency. */
export interface CurrencyLadder {
  readonly currency: CashflowEnvelope["flows"][number]["currency"];
  readonly rows: CalendarYearLadderRow[];
}
/** One parallel shift: scenarios.parallelBpMany over the listed curve ids of one curve family. */
export interface MarketScenarioShift {
  readonly curveKind: scenarioWire.CurveKind;
  /** Market curve ids, in operation order; Rust resolves them when the scenario is applied. */
  readonly curveIds: readonly string[];
  /** Additive basis points; percent of forward for commodity curves. */
  readonly bp: number;
}
/** A scenarios.operationSpecTimeRollForward operation; an omitted roll mode is left to Rust. */
export interface MarketScenarioRoll {
  /** Tenor text such as "1M", parsed by Rust. */
  readonly period: string;
  readonly rollMode?: scenarioWire.TimeRollMode | null;
}
/** A market-only ScenarioSpec built from native operations; omitted options keep Rust's defaults. */
export interface MarketScenarioRequest {
  /** Scenario id; Rust rejects a blank one. */
  readonly id: string;
  readonly shifts: readonly MarketScenarioShift[];
  readonly roll?: MarketScenarioRoll | null;
  readonly hazardBumpMode?: scenarioWire.HazardBumpMode | null;
}
/** Canonical ScenarioSpec text applied to a priced instrument's market at its valuation date. */
export interface MarketScenarioApplicationRequest {
  readonly scenarioJson: string;
  readonly marketJson: string;
  readonly asOf: string;
  /** Instrument envelope JSON; supplied to Rust only when the scenario requires instruments. */
  readonly instrumentJson: string;
}
/** The native application report and the shocked pricing inputs it implies. */
export interface MarketScenarioApplication {
  readonly report: ApplicationReport;
  /** Canonical JSON of the shocked market. */
  readonly marketJson: string;
  /** The rolled valuation date (report.time_roll.new_date), else the request's. */
  readonly asOf: string;
  /** The shocked instrument when the scenario mutates instruments, else the request's. */
  readonly instrumentJson: string;
}
/** Horizon total return of one instrument under a canonical scenario, from its market and valuation date. */
export interface HorizonReturnRequest {
  /** Canonical instrument envelope JSON. */
  readonly instrumentJson: string;
  /** Canonical MarketContext JSON at the valuation date. */
  readonly marketJson: string;
  /** ISO valuation date (T0). */
  readonly asOf: string;
  /** Canonical ScenarioSpec JSON; a time roll sets the horizon. */
  readonly scenarioJson: string;
  /** AttributionMethod wire value; omitted is Rust's default method. */
  readonly method?: attributionWire.AttributionMethod | null;
  /** Built-in holiday calendar id for business-day roll targets; omitted is Rust's weekends-only calendar. */
  readonly calendarId?: string | null;
}
/** The native horizon report and its Rust explain text. */
export interface HorizonReturn {
  /** scenarios.computeHorizonReturn as returned (HorizonResult plus its Rust summary). */
  readonly report: HorizonReport;
  /** scenarios.horizonResultExplainText of the report. */
  readonly explain: string;
}
/** One attribution configuration over T0 -> T1 for an ordered list of instruments. */
export interface AttributePnlManyRequest {
  /** Canonical instrument envelope JSON texts, attributed in this order. */
  readonly instrumentJsons: readonly string[];
  /** MarketContext JSON at T0; canonicalized through the market cache and spliced, never parsed. */
  readonly marketT0Json: string;
  /** MarketContext JSON at T1; canonicalized through the market cache and spliced, never parsed. */
  readonly marketT1Json: string;
  /** ISO valuation date at T0. */
  readonly asOfT0: string;
  /** ISO valuation date at T1. */
  readonly asOfT1: string;
  /** AttributionMethod wire value (required by AttributionInputs; Rust has no default here). */
  readonly method: attributionWire.AttributionMethod;
}
/** One native attribution with the Rust helpers a report shows beside it. */
export interface AttributedPnl {
  /** attribution.attributePnlMany's entry for the instrument, as returned. */
  readonly attribution: PnlAttribution;
  /** attribution.pnlAttributionExplainText (zero factors hidden, Rust's default). */
  readonly explain: string;
  /** attribution.pnlAttributionResidualWithinTolerance against the run's own tolerances. */
  readonly residualWithinTolerance: boolean;
  /** attribution.pnlAttributionPctOfTotal per factor bucket, in percent; null where Rust returns none. */
  readonly pctOfTotal: Readonly<Record<string, number | null>>;
}
/** A ticker-major panel for analytics.Performance; every omitted option is left to Rust. */
export interface PerformanceReportRequest {
  /** ISO-8601 observation dates, ascending, one per value of each series. */
  readonly dates: readonly string[];
  /** `values[ticker][date]`: simple decimal returns ("returns") or price levels ("prices"). */
  readonly values: readonly (readonly number[])[];
  /** "returns" builds with Performance.fromReturns, "prices" with new Performance. */
  readonly kind: "returns" | "prices";
  /** Ticker labels aligned with the outer elements of `values`. */
  readonly tickerNames: readonly string[];
  /** Ticker used as the benchmark series; omitted leaves it to Rust. */
  readonly benchmarkTicker?: string | null;
  /** Observation frequency token such as "daily"; omitted is Rust's default. */
  readonly frequency?: string | null;
  /** Annualized decimal risk-free rate for summary() and rollingSharpe; omitted is Rust's default. */
  readonly riskFreeRate?: number | null;
  /** Tail confidence for summary() VaR, ES and tail ratio; omitted is Rust's default. */
  readonly confidence?: number | null;
  /** Rolling window in observations; omitted is Rust's default. */
  readonly window?: number | null;
  /** Number of drawdown episodes per ticker; omitted is Rust's default. */
  readonly episodes?: number | null;
  /** Lookback end date; omitted is the last active panel date. */
  readonly lookbackDate?: string | null;
  /** Fiscal-year start for FYTD (a core.FiscalConfig); omitted is Rust's calendar year. */
  readonly fiscalYearStart?: {
    readonly month: number;
    readonly day: number;
  } | null;
}
/** A native rolling series with its values as a plain array (non-finite windows stay NaN). */
export type PerformanceSeries = Omit<DatedSeries, "values"> & {
  values: number[];
};
/** One ticker's native series, each aligned with that ticker's own active dates. */
export interface PerformanceTickerReport {
  /** getActiveDatesForTicker: the dates `cumulativeReturns` and `drawdowns` align with. */
  activeDates: string[];
  cumulativeReturns: number[];
  drawdowns: number[];
  /** drawdownDetails, largest first. */
  drawdownEpisodes: DrawdownEpisode[];
  rollingSharpe: PerformanceSeries;
  rollingVolatility: PerformanceSeries;
}
/** The native analytics.Performance results of one panel; nothing is computed in TypeScript. */
export interface PerformanceReport {
  tickerNames: string[];
  /** getBenchmarkIdx when the request named a benchmark, else null. */
  benchmarkIdx: number | null;
  frequency: string;
  activeDates: string[];
  summary: TableEnvelope;
  /** In tickerNames order. */
  tickers: PerformanceTickerReport[];
  /** periodicReturns("monthly"), ticker-major. */
  monthly: PeriodicReturn[][];
  /** periodicReturns("annual"), ticker-major. */
  annual: PeriodicReturn[][];
  lookback: {
    /** The requested lookback date, else the last active panel date. */
    refDate: string;
    returns: LookbackReturns;
    /** The fiscal-year start passed to Rust, else null (calendar year). */
    fiscalYearStart: { month: number; day: number } | null;
  };
  /** correlationMatrix rows in tickerNames order. */
  correlation: { matrix: number[][]; repaired: boolean };
}
/** A ticker-major panel measured against a named benchmark; every omitted option is left to Rust. */
export interface BenchmarkReportRequest {
  /** ISO-8601 observation dates, ascending, one per value of each series. */
  readonly dates: readonly string[];
  /** `values[ticker][date]`: simple decimal returns ("returns") or price levels ("prices"). */
  readonly values: readonly (readonly number[])[];
  /** "returns" builds with Performance.fromReturns, "prices" with new Performance. */
  readonly kind: "returns" | "prices";
  /** Ticker labels aligned with the outer elements of `values`. */
  readonly tickerNames: readonly string[];
  /** Ticker every relative measure is taken against; Rust rejects an unknown one. */
  readonly benchmarkTicker: string;
  /** Observation frequency token such as "daily"; omitted is Rust's default. */
  readonly frequency?: string | null;
  /** Annualized decimal risk-free rate for greeks, treynor, mSquared and rollingGreeks; omitted is Rust's default. */
  readonly riskFreeRate?: number | null;
  /** rollingGreeks window in observations; omitted is Rust's default. */
  readonly window?: number | null;
  /** Factor panel for multiFactorGreeks of every ticker; omitted skips the regression. */
  readonly factors?: {
    /** Display labels, one per factor row. */
    readonly names: readonly string[];
    /** `returns[factor][observation]`: already-excess decimal factor returns. */
    readonly returns: readonly (readonly number[])[];
    /** "excess" or "total"; omitted is Rust's default. */
    readonly returnKind?: string | null;
    /** Annualized decimal risk-free rate of a "total" regression; omitted is Rust's default. */
    readonly riskFreeRate?: number | null;
  } | null;
}
/** One ticker's native relative series, aligned with that ticker's own active dates. */
export interface BenchmarkTickerReport {
  /** getActiveDatesForTicker: the dates `outperformance` and `drawdownDifference` align with. */
  activeDates: string[];
  /** cumulativeReturnsOutperformance. */
  outperformance: number[];
  drawdownDifference: number[];
  /** rollingGreeks with its typed arrays copied out. */
  rollingGreeks: Omit<RollingGreeks, "alphas" | "betas"> & {
    alphas: number[];
    betas: number[];
  };
  /** multiFactorGreeks of this ticker, or its native error; null without a factor panel. */
  multiFactor: Envelope<MultiFactorResult> | null;
}
/** The native analytics.Performance benchmark-relative results of one panel; nothing is computed in TypeScript. */
export interface BenchmarkReport {
  tickerNames: string[];
  /** getBenchmarkIdx. */
  benchmarkIdx: number;
  frequency: string;
  activeDates: string[];
  /** The factor labels of the request, else null. */
  factorNames: string[] | null;
  /** Per-ticker results in tickerNames order. */
  greeks: GreeksResult[];
  beta: BetaResult[];
  trackingError: number[];
  informationRatio: number[];
  treynor: number[];
  mSquared: number[];
  upCapture: number[];
  downCapture: number[];
  captureRatio: number[];
  battingAverage: number[];
  /** In tickerNames order. */
  tickers: BenchmarkTickerReport[];
}
/** Where a Merton report's model comes from. */
export type MertonSource =
  | {
      /** models.credit.MertonModel.fromJson of canonical MertonModel JSON. */
      readonly kind: "model";
      readonly json: string;
    }
  | {
      /** MertonModel.kmvDefaultPoint(shortTermDebt, longTermDebt), then MertonModel.fromEquity with it as total debt (KMV iteration). */
      readonly kind: "equity";
      /** Market value of equity, in the firm's monetary unit. */
      readonly equityValue: number;
      /** Annualized equity-return volatility, decimal. */
      readonly equityVol: number;
      /** Liabilities due within one year; a total-debt barrier is this with no long-term debt. */
      readonly shortTermDebt: number;
      /** Liabilities maturing beyond one year. */
      readonly longTermDebt: number;
      /** Continuously compounded risk-free rate, decimal. */
      readonly riskFreeRate: number;
      /** Continuous payout yield on assets, decimal. */
      readonly payoutRate: number;
      /** Calibration horizon in years. */
      readonly maturity: number;
    };
/** A Merton structural-credit report: one model, a horizon grid and the CDS terms. */
export interface MertonReportRequest {
  readonly source: MertonSource;
  /** Horizons in years for default probability, distance to default and spreads, in display order. */
  readonly horizons: readonly number[];
  /** CDS and implied-equity maturity in years. */
  readonly maturity: number;
  /** Recovery at default as a fraction of par, for the implied and CDS spreads. */
  readonly recovery: number;
}
/** The native MertonModel and MasterScale results of one request; nothing is computed in TypeScript. */
export interface MertonReport {
  /** The model's canonical JSON (MertonModel#toJson), parsed. */
  model: modelsWire.MertonModel;
  /** The KMV default point of an equity source, else null. */
  defaultPoint: number | null;
  /** The requested horizons, in request order; every per-horizon array aligns with them. */
  horizons: number[];
  /** defaultProbabilities(horizons). */
  defaultProbabilities: number[];
  /** distanceToDefault per horizon. */
  distanceToDefault: number[];
  /** MasterScale.defaultCalibration().mapPds(defaultProbabilities). */
  grades: modelsWire.MasterScaleResult[];
  /** impliedSpread(horizon, recovery) per horizon, or the native error. */
  impliedSpread: Envelope<number[]>;
  /** debtSpread per horizon, or the native error (Rust defines it for a terminal barrier only). */
  debtSpread: Envelope<number[]>;
  maturity: number;
  recovery: number;
  /** cdsParSpread(maturity, recovery), or the native error. */
  cdsParSpread: Envelope<number>;
  /** impliedEquity(maturity) as returned: [equity value, equity volatility], or the native error. */
  impliedEquity: Envelope<number[]>;
}
/** One models.credit.scoring function and its named input ratios; altmanEmScore takes the Z''-Score ratios. */
export type CreditScoreRequest =
  | {
      readonly model: "altmanZScore";
      readonly input: Readonly<modelsWire.AltmanZScoreInput>;
    }
  | {
      readonly model: "altmanZPrime";
      readonly input: Readonly<modelsWire.AltmanZPrimeInput>;
    }
  | {
      readonly model: "altmanZDoublePrime" | "altmanEmScore";
      readonly input: Readonly<modelsWire.AltmanZDoublePrimeInput>;
    }
  | {
      readonly model: "ohlsonOScore";
      readonly input: Readonly<modelsWire.OhlsonOScoreInput>;
    }
  | {
      readonly model: "zmijewskiScore";
      readonly input: Readonly<modelsWire.ZmijewskiInput>;
    };
/** A models.credit.scoring function name. */
export type CreditScoreModel = CreditScoreRequest["model"];
/** The native ScoringResult of one model (score, zone, implied_pd, model). */
export type ScoringResult = ReturnType<
  typeof models.credit.scoring.zmijewskiScore
>;
/** One scoring result and its master-scale grade; nothing is computed in TypeScript. */
export interface CreditScore {
  /** The facade function that scored the input. */
  model: CreditScoreModel;
  /** The returned ScoringResult. */
  result: ScoringResult;
  /** models.credit.pd.MasterScale.defaultCalibration().mapScore(result), or its native error (Rust rejects a result without an implied PD). */
  grade: Envelope<modelsWire.MasterScaleResult>;
}
/** Where a position VaR/ES decomposition comes from. */
export type PositionRiskSource =
  | {
      /** models.factor.risk.parametricVarDecomposition (Euler allocation under normal returns). */
      readonly kind: "parametric";
      /** Position weights or exposures in portfolio currency, one per position id. */
      readonly weights: readonly number[];
      /** Position-return covariance, n x n rows in position order. */
      readonly covariance: readonly (readonly number[])[];
      /** Also compute incremental VaR (one repricing per position); omitted is left to Rust (false). */
      readonly computeIncremental?: boolean | null;
    }
  | {
      /** models.factor.risk.historicalVarDecomposition, plus buildStressAttribution of the same P&L. */
      readonly kind: "historical";
      /** Position-major scenario P&L: one row per position, one column per scenario (losses negative). */
      readonly pnls: readonly (readonly number[])[];
    };
/** A position risk report request; omitted options are left to Rust. */
export interface PositionRiskRequest {
  /** Position identifiers, in the order of the source's rows. */
  readonly positionIds: readonly string[];
  /** Tail confidence as a decimal in (0.5, 1); omitted is left to Rust (0.95). */
  readonly confidence?: number | null;
  readonly source: PositionRiskSource;
  /** Target shares of portfolio VaR per position for models.factor.risk.evaluateRiskBudget; omitted skips the budget. */
  readonly budget?: {
    readonly targets: readonly number[];
    /** Actual-to-target ratio that flags a breach; omitted is left to Rust. */
    readonly utilizationThreshold?: number | null;
  } | null;
}
/** The native StressAttribution (VaR threshold, tail scenarios, per-position tail contributions). */
export type StressAttribution = ReturnType<
  typeof models.factor.risk.buildStressAttribution
>;
/** The native results of one position risk request; nothing is computed in TypeScript. */
export interface PositionRisk {
  /** The returned PositionRiskDecomposition (losses negative). */
  decomposition: PositionRiskDecomposition;
  /** evaluateRiskBudget over positionComponentVar of each position and the portfolio VaR, or its native error; null without a budget. */
  budget: Envelope<RiskBudgetResult> | null;
  /** buildStressAttribution of a historical source's P&L, or its native error; null for a parametric source. */
  stress: Envelope<StressAttribution> | null;
}
/** A portfolio factor-sensitivity request over the cached book and market handles; an omitted bump config is left to Rust. */
export interface FactorSensitivitiesRequest {
  /** PortfolioSpec JSON; canonicalized (Portfolio#toJson) through the shared portfolio handle cache. */
  readonly specJson: string;
  /** MarketContextState JSON bumped per factor; through the shared market handle cache. */
  readonly marketJson: string;
  /** FactorDefinition array JSON; every factor must map to market data. */
  readonly factorsJson: string;
  /** ISO-8601 valuation date. */
  readonly asOf: string;
  /** BumpSizeConfig JSON; omitted uses Rust's 1 bp / 1 % per factor type. */
  readonly bumpConfigJson?: string | null;
}
/** A factor risk decomposition request; an omitted risk measure is left to Rust (variance). */
export interface DecomposeFactorsRequest {
  /** Sensitivity-matrix JSON, usually computeFactorSensitivities' result serialized. */
  readonly sensitivitiesJson: string;
  /** FactorCovarianceMatrix JSON aligned with the sensitivities' factor ids. */
  readonly covarianceJson: string;
  /** RiskMeasure JSON ("variance", "volatility", {"var": {"confidence": 0.99}}, ...). */
  readonly riskMeasureJson?: string | null;
}
/** A portfolio valuation request; omitted options are left to Rust. */
export interface PortfolioValuationRequest {
  /** PortfolioSpec JSON; canonicalized (Portfolio#toJson) through the shared portfolio handle cache. */
  readonly specJson: string;
  /** MarketContextState JSON; canonicalized through the shared market handle cache. */
  readonly marketJson: string;
  /** Risk-metric menu offered to every position; omitted is Rust's standard set (PV plus dv01), empty is PV only. */
  readonly metrics?: readonly string[] | null;
  /** Fail closed on a requested metric that fails to compute; omitted is Rust's default (true). */
  readonly strictRisk?: boolean | null;
}
/** One native portfolio valuation and its aggregated metrics; nothing is summed or converted in TypeScript. */
export interface PortfolioValuationReport {
  /** Position ids in spec order (Portfolio#positionIds); `position_values` keys do not keep it. */
  positionIds: string[];
  /** Entity ids in registration order (Portfolio#entityIds); `by_entity` keys do not keep it. */
  entityIds: string[];
  /** portfolio.valuePortfolio; each `valuation_result` keeps 64-bit fields as bigint. */
  valuation: PortfolioValuation;
  /** portfolio.aggregateMetrics over `valuation` and the same market. */
  metrics: PortfolioMetrics;
}
/** One base metric's series request over returned aggregated metrics. */
export interface PortfolioMetricsSeriesRequest {
  /** PortfolioMetrics returned by valuePortfolio (plain numbers; no bigint crosses the query key). */
  readonly metrics: PortfolioMetrics;
  /** Canonical base metric id, for example "bucketed_dv01". */
  readonly base: string;
}
/** A host-supplied book and market under an ordered batch of canonical scenarios. */
export interface ScenarioPnlBatchRequest {
  /** PortfolioSpec JSON; canonicalized (Portfolio#toJson) through the shared portfolio handle cache. */
  readonly specJson: string;
  /** Canonical MarketContext JSON of the unshocked market. */
  readonly marketJson: string;
  /** ScenarioSpec JSON texts in batch order; passed to Rust as one JSON array, never parsed here. */
  readonly scenarioJsons: readonly string[];
}
/** The native batch and the book's position order. */
export interface ScenarioPnlBatch {
  /** Position ids in spec order (Portfolio#positionIds); `pnl.by_position` keys do not keep it. */
  readonly positionIds: string[];
  /** portfolio.scenarioPnlBatch: one item per scenario, in input order. */
  readonly items: ScenarioPnlBatchItem[];
}
/** A host-supplied book and market under one canonical scenario. */
export interface ScenarioRevalueRequest {
  /** PortfolioSpec JSON; canonicalized (Portfolio#toJson) through the shared portfolio handle cache. */
  readonly specJson: string;
  /** Canonical MarketContext JSON of the unshocked market. */
  readonly marketJson: string;
  /** Canonical ScenarioSpec JSON. */
  readonly scenarioJson: string;
}
/** The native stressed valuation and the book's id orders. */
export interface ScenarioRevalue {
  /** Position ids in spec order (Portfolio#positionIds). */
  readonly positionIds: string[];
  /** Entity ids in registration order (Portfolio#entityIds). */
  readonly entityIds: string[];
  /** portfolio.applyScenarioAndRevalue; each `valuation_result` keeps 64-bit fields as bigint. */
  readonly view: ScenarioRevalueView;
}
/** A host-supplied book's margin run over a market on one date. */
export interface PortfolioMarginRequest {
  /** PortfolioSpec JSON; canonicalized (Portfolio#toJson) through the shared portfolio handle cache. Positions whose instruments carry a margin_spec form the netting sets. */
  readonly specJson: string;
  /** MarketContext JSON; canonicalized through the shared market cache. */
  readonly marketJson: string;
  /** ISO valuation date of the margin run. */
  readonly asOf: string;
  /** One-way IM collateral already held, keyed by CSA id in that CSA's currency; omitted is none (Rust treats a missing key as zero), and an unknown CSA id is a native validation error. */
  readonly currentImCollateral?: Readonly<Record<string, MoneyValue>> | null;
}
/** Scenario name to its native statement result. */
export type StatementScenarioResults = ScenarioResults;
/** Omitted pricing models resolve to the native "default" (see pricing.rs price_instrument). */
export function resolveModel(model?: string | null): string {
  return model ?? "default";
}
export interface FxDeltaSampleRequest {
  readonly surface: MarketContextStateWire["fx_delta_vol_surfaces"][number];
  /** Explicit years, strike and quote/base forward per sample, in original order. */
  readonly coordinates: readonly {
    expiry: number;
    strike: number;
    forward: number;
  }[];
}
/** Explicit spot and continuously compounded decimal rates for models.volatility.materializeFxDeltaSurface. */
export interface FxStrikeSurfaceRequest {
  readonly surface: MarketContextStateWire["fx_delta_vol_surfaces"][number];
  /** FX spot, domestic per unit of foreign; Rust rejects non-positive values. */
  readonly spot: number;
  /** Continuously compounded domestic rate, as a decimal. */
  readonly domesticRate: number;
  /** Continuously compounded foreign rate, as a decimal. */
  readonly foreignRate: number;
}
/** Canonical stored-surface JSON of a native VolSurface (`toJson`). */
export type StoredSurfaceWire = MarketContextStateWire["surfaces"][number];
/** Canonical stored-surface JSON of the materialized FX strike grid. */
export type FxStrikeSurface = StoredSurfaceWire;
export interface CubeSampleRequest {
  readonly cube: MarketContextStateWire["vol_cubes"][number];
  readonly coordinates: readonly {
    expiry: number;
    tenor: number;
    strike: number;
  }[];
  readonly convention: "normal" | "black_lognormal";
}
/** One materialized slice of a SABR cube at explicit strikes; off-node values are Rust-interpolated. */
export interface CubeSliceRequest {
  readonly cube: MarketContextStateWire["vol_cubes"][number];
  /** "tenor" fixes the underlying tenor (expiry by strike grid); "expiry" fixes the option expiry (tenor by strike grid). */
  readonly axis: "tenor" | "expiry";
  /** Fixed tenor or expiry in years, within the cube grid. */
  readonly at: number;
  /** Absolute strikes in the stored forwards' rate units; Rust requires them strictly increasing. */
  readonly strikes: readonly number[];
  readonly convention: CubeSampleRequest["convention"];
}
/** Canonical JSON of the returned slice: a stored surface (tenor axis) or a VolCubeExpirySlice (expiry axis). */
export type CubeSlice =
  | { readonly axis: "tenor"; readonly surface: StoredSurfaceWire }
  | { readonly axis: "expiry"; readonly slice: wire.VolCubeExpirySlice };
/** SABR smile of one cube node at explicit strikes and an explicit continuously compounded decimal rate. */
export interface CubeSmileRequest {
  readonly cube: MarketContextStateWire["vol_cubes"][number];
  /** Zero-based index into cube.expiries. */
  readonly expiryIndex: number;
  /** Zero-based index into cube.tenors. */
  readonly tenorIndex: number;
  /** Absolute strikes in the model domain, strictly increasing. */
  readonly strikes: readonly number[];
  /** Continuously compounded decimal rate discounting the arbitrage-check prices; never defaulted. */
  readonly rate: number;
  /** Repair passes for SabrSmile#repairArbitrage; omitted means no repair is run. */
  readonly repairIterations?: number | null;
}
/** Native smile values for one node, in the facade's convention (normal when beta < 1e-4, else Black). */
export interface CubeSmile {
  readonly parameters: wire.SabrParameterData;
  readonly forward: number;
  /** The node's option expiry in years, the smile's time to expiry. */
  readonly expiry: number;
  readonly vols: number[];
  readonly arbitrage: ArbitrageValidationResult;
  /** Repaired vols aligned with the strikes; null when no repair was requested. */
  readonly repaired: number[] | null;
}
/** A labelled rating transition matrix: rows and columns share one label order, the last label absorbing (TransitionMatrix.fromLabeled). */
export interface MigrationMatrixRequest {
  /** Rating labels in row and column order; the last is the absorbing default state. */
  readonly labels: readonly string[];
  /** Transition probabilities, one row per origin state; each row sums to one (validated by Rust). */
  readonly rows: readonly (readonly number[])[];
  /** Positive horizon in years the probabilities cover. */
  readonly horizon: number;
}
/** The matrix as Rust returns it: labels and default state from its RatingScale, rows and default probabilities from the matrix. */
export interface MigrationMatrix {
  readonly labels: string[];
  /** RatingScale#defaultState; null when the scale has none. */
  readonly defaultState: number | null;
  readonly horizon: number;
  readonly rows: number[][];
  /** TransitionMatrix#defaultProbabilities; null when the scale has no default state. */
  readonly defaultProbabilities: number[] | null;
}
/** Generator estimation and projection of a labelled matrix to explicit horizons. */
export interface MigrationProjectionRequest extends MigrationMatrixRequest {
  /** Positive horizons in years for models.credit.migration.project, in display order. */
  readonly horizons: readonly number[];
  /** Largest accepted round-trip error of GeneratorMatrix.fromTransitionMatrix; omitted leaves Rust's default. */
  readonly roundTripTol?: number | null;
}
/** The regularized generator and the default probabilities of each projected matrix. */
export interface MigrationProjection {
  readonly labels: string[];
  readonly defaultState: number | null;
  readonly generator: {
    /** Intensities per year, one row per origin state. */
    readonly rows: number[][];
    readonly roundTripError: number;
    readonly regularizationL1: number;
  };
  /** One entry per requested horizon, in request order; horizon is the projected matrix's own. */
  readonly projections: {
    readonly horizon: number;
    readonly defaultProbabilities: number[] | null;
  }[];
}
/** A SIMM initial-margin calculation over one sensitivity set. */
export interface SimmImRequest {
  /** SimmSensitivitiesJson text; its own base_currency is the reporting currency (Rust accepts only USD). */
  readonly sensitivitiesJson: string;
  /** ISO calculation date stamped on the result. */
  readonly asOf: string;
  /** SIMM version label such as "v2_6"; omitted leaves Rust's default version. */
  readonly version?: string | null;
  /** Margin period of risk in business days; omitted leaves the registry default. */
  readonly mporDays?: number | null;
}
/** A Rust regulatory CSA preset: the canonical USD or EUR CSA, or regulatory terms for any supported currency. */
export type CsaPresetRequest =
  | { readonly kind: "usd" }
  | { readonly kind: "eur" }
  | {
      readonly kind: "regulatory";
      /** ISO-4217 base currency; Rust rejects an unknown one or one without regulatory terms. */
      readonly currency: string;
      /** Identifier stamped on the specification; Rust rejects an empty one. */
      readonly id: string;
      /** Collateral discounting curve id such as "USD-OIS". */
      readonly collateralCurve: string;
    };
/** One variation-margin calculation under one CSA. */
export interface VmRequest {
  /** CsaSpec text; Rust validates it and its base currency is the calculation currency. */
  readonly csaJson: string;
  /** Signed mark-to-market in the CSA base currency (positive: the counterparty owes the desk); a cleared field is NaN, never 0. */
  readonly exposure: number;
  /** Signed collateral balance in the CSA base currency (positive held, negative posted). */
  readonly postedCollateral: number;
  /** ISO calculation date; Rust dates the settlement on the CSA calendar. */
  readonly asOf: string;
}
/** A returned VmResult with Rust's own call decision and net margin. */
export interface VmReport {
  readonly result: VmResult;
  /** margin.vmResultRequiresCall: whether a post or collect amount is non-zero. */
  readonly requiresCall: boolean;
  /** margin.vmResultNetMargin: post minus collect in major units of the CSA base currency; positive means the desk posts. */
  readonly netMargin: number;
}
/** An exposure series run into a margin-call schedule under one CSA. */
export interface MarginCallsRequest {
  readonly csaJson: string;
  /** [ISO date, signed exposure in the CSA base currency] pairs, processed in the order given. */
  readonly exposures: readonly (readonly [string, number])[];
  /** Collateral balance before the first date, in major units of the CSA base currency. */
  readonly initialCollateral: number;
}
/** Contractual call dates of one CSA between two ISO dates, inclusive. */
export interface MarginCallDatesRequest {
  readonly csaJson: string;
  readonly start: string;
  readonly end: string;
}
/** Explicit cloneable exception data; never Comlink's lossy default thrown-error transfer. */
export interface ErrorValue {
  name: string;
  message: string;
  kind?: unknown;
  [field: string]: unknown;
}
export type Envelope<T> =
  { ok: true; value: T } | { ok: false; error: ErrorValue };
/** core: money display, market canonicalization and calendars. */
export interface CoreApi {
  /** core.Currency + core.Money.fromDecimalStr(...).formatWith: native display text for an exact amount under its returned rounding stamp; raw text keeping every supplied digit when the stamp has no scale for the currency. */
  formatMoney(request: MoneyFormatRequest): Promise<Envelope<string>>;
  /** core.MarketContext.fromJson + toJson: canonicalize the complete market and cache its handle. */
  validateMarket(marketJson: string): Promise<Envelope<string>>;
  /** core.availableCalendars. */
  calendars(): Promise<Envelope<string[]>>;
}
/** valuations: instrument validation, pricing, cashflows, scenarios, registries and export. */
export interface ValuationsApi {
  /** valuations.instruments.validateInstrumentJson + priceInstrumentWithMarket over the cached market handle. */
  price(request: PriceRequest): Promise<Envelope<ValuationResult>>;
  /** valuations.instruments.validateInstrumentJson: validate and canonicalize instrument JSON. */
  validate(instrumentJson: string): Promise<Envelope<string>>;
  /** valuations.instruments.validateInstrumentJson + instrumentCashflowsWithMarket over the cached market handle; the native envelope object. */
  cashflows(request: CashflowRequest): Promise<Envelope<CashflowEnvelope>>;
  /** valuations.instruments.StructuredCredit.fromJson + trancheScenarioTable. */
  scenarioTable(request: ScenarioRequest): Promise<Envelope<ScenarioTable>>;
  /** valuations.instruments.StructuredCredit.fromJson + trancheMetrics; no OasConfig or price default is supplied. */
  trancheMetrics(
    request: TrancheMetricsRequest,
  ): Promise<Envelope<TrancheMetrics>>;
  /** valuations.instruments.StructuredCredit.fromJson + trancheOas under the supplied OasConfig, else Rust's default. */
  trancheOas(request: TrancheOasRequest): Promise<Envelope<OasResult>>;
  /** valuations.instruments.StructuredCredit.fromJson + trancheDiscountMargin: decimal discount margin of a floating-rate tranche. */
  trancheDiscountMargin(
    request: TranchePriceRequest,
  ): Promise<Envelope<number>>;
  /** valuations.instruments.StructuredCredit.fromJson + trancheBreakevenCdr: decimal break-even constant default rate. */
  trancheBreakevenCdr(request: TrancheRequest): Promise<Envelope<number>>;
  /** valuations.instruments.listModelsGrouped. */
  models(): Promise<Envelope<Record<string, string[]>>>;
  /** valuations.instruments.metricsForInstrument: standard metrics registered for one instrument type, by native display group. */
  metrics(instrumentType: string): Promise<Envelope<Record<string, string[]>>>;
  /** valuations.instruments.metricMetadata: native per-key interpretation (units, decoded coordinates, groups) for metric keys. */
  metricMetadata(keys: readonly string[]): Promise<Envelope<MetricMetadata[]>>;
  /** valuations.valuationResultToJson over the host-serialized result: native canonical JSON. */
  exportResult(value: ValuationResult): Promise<Envelope<string>>;
}
/** cashflows: aggregation of an exported cashflow envelope. */
export interface CashflowsApi {
  /** cashflows.calendarYearLadder once per row currency, in first-appearance order, over each flow's date, kind, amount and native_pv (row-currency PV before FX conversion and any scenario price shock). */
  cashflowLadder(
    cashflows: CashflowEnvelope,
  ): Promise<Envelope<CurrencyLadder[]>>;
}
/** scenarios: market scenario construction, application and horizon total return. */
export interface ScenariosApi {
  /** scenarios.operationSpecTimeRollForward + parallelBpMany per shift + parseScenarioSpec: canonical ScenarioSpec text. */
  buildMarketScenario(
    request: MarketScenarioRequest,
  ): Promise<Envelope<string>>;
  /** scenarios.scenarioSpecRequiresInstruments + applyScenario (the instrument only when required) + scenarioSpecMutatesInstruments; the shocked market is canonicalized through the market cache. */
  applyMarketScenario(
    request: MarketScenarioApplicationRequest,
  ): Promise<Envelope<MarketScenarioApplication>>;
  /** scenarios.computeHorizonReturn (omitted method, config and calendar left to Rust) + horizonResultExplainText. */
  horizonReturn(
    request: HorizonReturnRequest,
  ): Promise<Envelope<HorizonReturn>>;
}
/** attribution: single-instrument T0 -> T1 P&L attribution and its Rust helpers. */
export interface AttributionApi {
  /** attribution.attributePnlMany over canonical market texts, then pnlAttributionExplainText, pnlAttributionResidualWithinTolerance and pnlAttributionPctOfTotal per factor; one entry per instrument in input order. */
  attributePnlMany(
    request: AttributePnlManyRequest,
  ): Promise<Envelope<AttributedPnl[]>>;
  /** attribution.defaultWaterfallOrder: Rust's waterfall factor order. */
  defaultWaterfallOrder(): Promise<Envelope<string[]>>;
}
/** analytics: performance reports over a host-supplied panel. */
export interface AnalyticsApi {
  /** analytics.Performance (fromReturns or the price constructor) + summary, series, drawdowns, rolling, periodic, lookback (core.FiscalConfig when a fiscal start is set) and correlation; every handle is freed. */
  performanceReport(
    request: PerformanceReportRequest,
  ): Promise<Envelope<PerformanceReport>>;
  /** analytics.Performance against a named benchmark: greeks, beta, the relative ratios and captures, outperformance and drawdown difference, rollingGreeks and (with a factor panel) multiFactorGreeks per ticker; the handle is freed. */
  benchmarkReport(
    request: BenchmarkReportRequest,
  ): Promise<Envelope<BenchmarkReport>>;
}
/** portfolio: valuation, metric aggregation, spec validation, scenario P&L and margin of a host-supplied book. */
export interface PortfolioApi {
  /** portfolio.valuePortfolio over the cached Portfolio and MarketContext handles, then aggregateMetrics over the same market. */
  valuePortfolio(
    request: PortfolioValuationRequest,
  ): Promise<Envelope<PortfolioValuationReport>>;
  /** portfolio.portfolioMetricsSeries: decoded entries of one base metric, in the order Rust returns them. */
  portfolioMetricsSeries(
    request: PortfolioMetricsSeriesRequest,
  ): Promise<Envelope<PortfolioMetricSeriesEntry[]>>;
  /** portfolio.parsePortfolioSpecJson, then Portfolio.fromSpec through the handle cache: canonical PortfolioSpec JSON of a book that builds. */
  validatePortfolio(specJson: string): Promise<Envelope<string>>;
  /** portfolio.scenarioPnlBatch over the cached Portfolio and MarketContext handles; the whole batch fails on the earliest failing scenario. */
  scenarioPnlBatch(
    request: ScenarioPnlBatchRequest,
  ): Promise<Envelope<ScenarioPnlBatch>>;
  /** portfolio.applyScenarioAndRevalue over the cached Portfolio and MarketContext handles. */
  applyScenarioAndRevalue(
    request: ScenarioRevalueRequest,
  ): Promise<Envelope<ScenarioRevalue>>;
  /** portfolio.portfolioMargin over the cached Portfolio and MarketContext handles; omitted collateral stays unset. */
  portfolioMargin(
    request: PortfolioMarginRequest,
  ): Promise<Envelope<PortfolioMarginResult>>;
  /** portfolio.computeFactorSensitivities over the cached Portfolio and MarketContext handles: the position-by-factor sensitivity matrix in the base currency. */
  computeFactorSensitivities(
    request: FactorSensitivitiesRequest,
  ): Promise<Envelope<SensitivityMatrixJson>>;
}
/** margin: SIMM sensitivity validation and indicative initial margin; CSA presets and validation, variation margin and margin calls. */
export interface MarginApi {
  /** margin.SimmSensitivities.fromJson + validate + toJson: canonical SimmSensitivitiesJson text; the handle is freed. */
  validateSimmSensitivities(json: string): Promise<Envelope<string>>;
  /** margin.SimmSensitivities.fromJson + new margin.SimmCalculator(version?, mporDays?) + calculateFromSensitivities in the set's base currency; both handles are freed. */
  simmIm(request: SimmImRequest): Promise<Envelope<ImResult>>;
  /** margin.csaSpecUsdRegulatory, csaSpecEurRegulatory or csaSpecRegulatory(currency, id, collateralCurve): the Rust preset CsaSpec. */
  csaPreset(request: CsaPresetRequest): Promise<Envelope<CsaSpec>>;
  /** margin.csaSpecValidate, then new margin.VmCalculator(json)#csa: canonical CsaSpec text; the handle is freed. */
  validateCsa(json: string): Promise<Envelope<string>>;
  /** new margin.VmCalculator(csaJson)#calculate in the CSA's own base currency, with margin.vmResultRequiresCall and vmResultNetMargin; the handle is freed. */
  vm(request: VmRequest): Promise<Envelope<VmReport>>;
  /** new margin.VmCalculator(csaJson)#generateMarginCalls: one MarginCall per date that calls; the handle is freed. */
  marginCalls(request: MarginCallsRequest): Promise<Envelope<MarginCall[]>>;
  /** new margin.VmCalculator(csaJson)#marginCallDates: ISO call dates on the CSA calendar and frequency; the handle is freed. */
  marginCallDates(request: MarginCallDatesRequest): Promise<Envelope<string[]>>;
}
/** calibration: envelope validation, static diagnostics and calibration runs. */
export interface CalibrationApi {
  /** calibration.validateCalibration: full envelope includes prior_market, quote sets, data and every plan setting. */
  validateCalibration(
    envelopeJson: string,
  ): Promise<Envelope<CalibrationEnvelope>>;
  /** calibration.dryRun: structured static diagnostics. */
  dryRun(envelopeJson: string): Promise<Envelope<CalibrationValidationReport>>;
  /** calibration.calibrate. */
  calibrate(envelopeJson: string): Promise<Envelope<CalibrationResultEnvelope>>;
}
/** models: volatility sampling over core surface handles and rating migration. */
export interface ModelsApi {
  /** core.VolCube.fromJson + models.volatility.getCubeNormalVol or getCubeVol, per coordinate. */
  sampleCube(request: CubeSampleRequest): Promise<Envelope<number[]>>;
  /** core.FxDeltaVolSurface.fromJson + models.volatility.getFxDeltaVol, per coordinate. */
  sampleFxDelta(request: FxDeltaSampleRequest): Promise<Envelope<number[]>>;
  /** core.FxDeltaVolSurface.fromJson + models.volatility.materializeFxDeltaSurface, as the returned surface's JSON. */
  materializeFxSurface(
    request: FxStrikeSurfaceRequest,
  ): Promise<Envelope<FxStrikeSurface>>;
  /** core.VolCube.fromJson + models.volatility.materializeCube{Tenor,Expiry}Slice[Normal], as the returned slice's JSON. */
  cubeSlice(request: CubeSliceRequest): Promise<Envelope<CubeSlice>>;
  /** core.VolCube.fromJson + paramsAt/forwardAt + models.volatility.SabrParameters.fromJson + SabrSmile generateSmile, validateNoArbitrage and (when requested) repairArbitrage. */
  cubeSmile(request: CubeSmileRequest): Promise<Envelope<CubeSmile>>;
  /** models.credit.migration.TransitionMatrix.fromLabeled + toRows, defaultProbabilities, horizon and the scale's labels; every handle (including the scale getter's) is freed. */
  migrationMatrix(
    request: MigrationMatrixRequest,
  ): Promise<Envelope<MigrationMatrix>>;
  /** TransitionMatrix.fromLabeled + GeneratorMatrix.fromTransitionMatrix + models.credit.migration.project per horizon; every handle is freed. */
  migrationProjection(
    request: MigrationProjectionRequest,
  ): Promise<Envelope<MigrationProjection>>;
  /** models.credit.MertonModel.fromJson + toJson: canonical MertonModel JSON text; the handle is freed. */
  validateMertonModel(json: string): Promise<Envelope<string>>;
  /** MertonModel (fromJson, or kmvDefaultPoint + fromEquity) + default probabilities, distance to default, spreads, CDS par spread, implied equity and models.credit.pd.MasterScale.defaultCalibration().mapPds; per-field native errors are kept; both handles are freed. */
  mertonReport(request: MertonReportRequest): Promise<Envelope<MertonReport>>;
  /** One models.credit.scoring function over its input ratios in the facade's positional order, then models.credit.pd.MasterScale.defaultCalibration().mapScore of the result (kept as an envelope); the scale handle is freed. */
  creditScore(request: CreditScoreRequest): Promise<Envelope<CreditScore>>;
  /** models.factor.risk parametric or historical VaR/ES decomposition, evaluateRiskBudget over positionComponentVar per position, and buildStressAttribution of a historical source; the budget and stress attribution keep their own native errors. */
  positionRisk(request: PositionRiskRequest): Promise<Envelope<PositionRisk>>;
  /** models.factor.risk.decomposeFactors: the parametric factor and position risk decomposition of a sensitivity matrix under a factor covariance. */
  decomposeFactors(
    request: DecomposeFactorsRequest,
  ): Promise<Envelope<RiskDecomposition>>;
}
/** statements and statements_analytics: model validation, evaluation and diagnostics. */
export interface StatementsApi {
  /** statements.validateFinancialModelJson. */
  validateStatementModel(modelJson: string): Promise<Envelope<string>>;
  /** statements.parseAndCompile + parseFormula. */
  validateStatementFormula(formula: string): Promise<Envelope<string>>;
  /** statements.validateCheckSuiteSpecJson: validate and canonicalize CheckSuiteSpec JSON. */
  validateCheckSuiteSpec(json: string): Promise<Envelope<string>>;
  /** statements.Evaluator evaluate, or evaluateWithMarket when market and date are supplied. */
  evaluateStatement(
    request: StatementRequest,
  ): Promise<Envelope<StatementResult>>;
  /** statements_analytics.explainFormula. */
  explainStatement(
    request: StatementExplanationRequest,
  ): Promise<Envelope<Explanation>>;
  /** statements_analytics.DependencyTracer.dependencyTree. */
  traceStatement(
    request: StatementTraceRequest,
  ): Promise<Envelope<DependencyTree>>;
  /** statements_analytics.runChecks, runThreeStatementChecks or runCreditUnderwritingChecks, by request kind. */
  runStatementChecks(
    request: StatementChecksRequest,
  ): Promise<Envelope<CheckReport>>;
  /** statements_analytics.evaluateScenarioSet, or evaluateScenarioSetWithMarket to price the capital structure from the market. */
  evaluateStatementScenarios(
    request: StatementScenarioRequest,
  ): Promise<Envelope<StatementScenarioResults>>;
  /** statements_analytics.scenarioDiff once per pair, in pair order. */
  diffStatementScenarios(
    request: StatementScenarioDiffRequest,
  ): Promise<Envelope<StatementScenarioDiff[]>>;
  /** statements_analytics.scenarioSetTrace: scenario names from the root ancestor down to the scenario. */
  traceStatementScenario(
    request: StatementScenarioTraceRequest,
  ): Promise<Envelope<string[]>>;
  /** statements.statementResultRollup: an evaluated result at a coarser frequency. */
  rollupStatement(
    request: StatementRollupRequest,
  ): Promise<Envelope<StatementResult>>;
  /** statements_analytics.creditAssessment: leverage, interest coverage and free cash flow at a period, with the per-period series. */
  creditAssessment(
    request: CreditAssessmentRequest,
  ): Promise<Envelope<CreditAssessment>>;
  /** statements_analytics.plSummaryReport: the P&L summary TableEnvelope over the line items and periods. */
  plSummaryReport(
    request: PlSummaryReportRequest,
  ): Promise<Envelope<TableEnvelope>>;
  /** statements_analytics.evaluateDcf: enterprise and equity value of the model's unlevered free cash flow, with the DCF working. */
  evaluateDcf(
    request: EvaluateDcfRequest,
  ): Promise<Envelope<CorporateValuationResult>>;
  /** statements_analytics.dcfSensitivity: enterprise-value tornado of the headline DCF assumptions (no market argument). */
  dcfSensitivity(
    request: DcfSensitivityRequest,
  ): Promise<Envelope<DcfSensitivityResult>>;
  /** statements_analytics.evaluateLbo: entry and exit values, sources and uses, MOIC and the optional LBO check report. */
  evaluateLbo(request: EvaluateLboRequest): Promise<Envelope<LboResult>>;
  /** statements_analytics.parameterSpecWithPercentages per parameter, runSensitivity, then generateTornadoEntries at the target metric and period (skipped for full_grid). */
  runSensitivity(
    request: RunSensitivityRequest,
  ): Promise<Envelope<StatementSensitivity>>;
  /** statements_analytics.goalSeek: the driver value that brings the target node to the target value, and optionally the updated model. */
  goalSeek(request: GoalSeekRequest): Promise<Envelope<GoalSeekResult>>;
  /** statements_analytics.runVariance: metric-by-period variance of two evaluated results. */
  runVariance(request: RunVarianceRequest): Promise<Envelope<VarianceReport>>;
  /** statements_analytics.varianceBridge: baseline and comparison values of a target metric, driver contributions in driver order and the unexplained residual. */
  varianceBridge(
    request: VarianceBridgeRequest,
  ): Promise<Envelope<BridgeChart>>;
  /** statements_analytics.backtestForecast: MAE, MAPE, sMAPE and RMSE of a forecast against realized values. */
  backtestForecast(
    request: BacktestForecastRequest,
  ): Promise<Envelope<ForecastMetrics>>;
}
/** Every worker method, grouped by the crate whose facade namespace it calls. */
export type WorkerApi = {
  /** Facade default init (module_or_path = wasmUrl), started once; resolves when ready. */
  initialize(
    wasmUrl?: string,
  ): Promise<Envelope<{ state: "ready"; worker: boolean }>>;
} & CoreApi &
  ValuationsApi &
  CashflowsApi &
  ScenariosApi &
  AttributionApi &
  AnalyticsApi &
  PortfolioApi &
  MarginApi &
  CalibrationApi &
  ModelsApi &
  StatementsApi;
/** Preserve native exception fields and recursively preserve Error causes. */
export function errorValue(error: unknown): ErrorValue {
  const object =
    error != null && typeof error === "object"
      ? (error as Record<string, unknown>)
      : {};
  const value: ErrorValue = {
    name: typeof object.name === "string" ? object.name : "Error",
    message:
      typeof object.message === "string" ? object.message : String(error),
    kind: object.kind,
  };
  for (const key of Object.getOwnPropertyNames(object)) {
    if (
      [
        "name",
        "message",
        "stack",
        "__proto__",
        "constructor",
        "prototype",
      ].includes(key)
    )
      continue;
    value[key] =
      object[key] instanceof Error ? errorValue(object[key]) : object[key];
  }
  return value;
}
/** Error consumed by Query; original facade fields remain on the exception and payload. */
export class FinstackError extends Error {
  readonly payload: ErrorValue;
  constructor(payload: ErrorValue) {
    super(payload.message);
    this.payload = payload;
    Object.assign(this, payload);
    if (
      payload.cause &&
      typeof payload.cause === "object" &&
      "message" in payload.cause
    )
      this.cause = new FinstackError(payload.cause as ErrorValue);
  }
}
export function unwrap<T>(result: Envelope<T>): T {
  if (!result.ok) throw new FinstackError(result.error);
  return result.value;
}
