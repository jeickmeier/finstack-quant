import type {
  PerformanceReportData,
  PerformanceTickerData,
  BenchmarkReportData,
  BenchmarkTickerData,
} from "@/components/finstack/analytics/lib/analytics-report-types";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import type { RoundingStamp } from "@/lib/finstack/format/format";
import type {
  ValuationResult,
  CalibrationResultEnvelope,
  CalibrationEnvelope,
  CalibrationValidationReport,
  MetricFieldUnit,
  TrancheSolveFieldUnits,
  MetricMetadata,
  MoneyValue,
  ScenarioTable,
  ScenarioTableFieldUnits,
  SimulationFieldUnits,
  EquityMetricsFieldUnits,
  StandardDealFamily,
  PoolReportFieldUnits,
  StructuredCredit,
  ScenarioResults,
  TrancheCashflows,
  TrancheMetrics,
  OasResult,
  StatementResult,
  Explanation,
  DependencyTree,
  CheckReport,
  CurrencyLadder,
  CreditAssessment,
  DcfSensitivityResult,
  LboResult,
  SensitivityResult,
  TornadoEntry,
  GoalSeekResult,
  VarianceReport,
  ForecastMetrics,
  MonteCarloResults,
  ArbitrageValidationResult,
  ApplicationReport,
  HorizonReport,
  ScenarioSpec,
  TemplateMetadata,
  PnlAttribution,
  PnlBucketShare,
  DatedSeries,
  MultiFactorResult,
  RollingGreeks,
  TableEnvelope,
  PortfolioMetrics,
  PortfolioMetricSeriesEntry,
  PortfolioValuation,
  ScenarioPnlBatchItem,
  ScenarioRevalueView,
  PortfolioMarginResult,
  PortfolioAttribution,
  ReconciliationReport,
  ImResult,
  CsaSpec,
  VmResult,
  MarginCall,
  PositionRiskDecomposition,
  RiskBudgetResult,
  RiskDecomposition,
  SensitivityMatrixJson,
  models,
  statements,
  statements_analytics,
  valuations,
} from "finstack-quant-wasm";
import type {
  attribution as attributionWire,
  core as wire,
  covenants as covenantsWire,
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
/** A pricing request whose metric ids are each priced on their own call. */
export interface MetricValuesRequest extends Omit<PriceRequest, "metrics"> {
  readonly metrics: readonly string[];
}
/** One metric id and its native value, or the native error of that id alone; a value is absent when Rust returned none under the id. */
export interface MetricValue {
  metric: string;
  value: Envelope<number | undefined>;
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
/** One of the deal families StructuredCredit.newStandard accepts; standardDealFamilies lists them. */
export type StandardDealType = StandardDealFamily["deal_type"];
/** A registry deal family and its complete native StandardDealParams JSON. */
export interface StandardDealRequest {
  readonly dealType: StandardDealType;
  readonly paramsJson: string;
}
/** The native capital structure of a deal, as StructuredCredit's tranches getter returns it. */
export type DealTranches = StructuredCredit["tranches"];
/** Native report of a deal's collateral pool. */
export type PoolReport = ReturnType<typeof valuations.instruments.poolReport>;
/** Yield and I-spread to every exit of a bond, worst marked by Rust. */
export type BondWorkoutLadder = ReturnType<
  typeof valuations.instruments.bondWorkoutLadder
>;
/** One bond on a complete market at a valuation date. */
export interface BondWorkoutLadderRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  /** ISO-8601 valuation date. */
  readonly asOf: string;
}
/** A convertible's price, delta, parity, premium, regime and floor by share price. */
export type ConvertibleSpotLadder = ReturnType<
  typeof valuations.instruments.convertibleSpotLadder
>;
/** One convertible on a complete market at a valuation date. */
export interface ConvertibleSpotLadderRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  /** ISO-8601 valuation date. */
  readonly asOf: string;
  /** Share prices; null or omitted is Rust's default ladder. */
  readonly spots?: readonly number[] | null;
}
/** Every price, yield and spread quote of a bond, from any one of them. */
export type BondQuoteSet = ReturnType<
  typeof valuations.instruments.computeQuotes
>;
/** One bond on a complete market at a valuation date, with the one quote to solve from. */
export interface ComputeQuotesRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  /** ISO-8601 valuation date. */
  readonly asOf: string;
  /** One BondQuoteInput: a single key naming the quote kind. */
  readonly quoteInput: Parameters<
    typeof valuations.instruments.computeQuotes
  >[3];
}
/** One deal's collateral pool at a date. */
export interface PoolReportRequest {
  readonly instrumentJson: string;
  /** ISO-8601 date the remaining maturities are measured from. */
  readonly asOf: string;
}
/** Native tranche flows and period-by-period deal accounting of one deterministic simulation. */
export type SimulationRun = ReturnType<StructuredCredit["runSimulation"]>;
/** One structured-credit deal simulated on a complete market at a valuation date. */
export interface DealSimulationRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  readonly asOf: string;
}
/** Native residual-class IRR, MOIC, NAV and cash-on-cash of one deterministic projection. */
export type EquityMetrics = ReturnType<StructuredCredit["equityMetrics"]>;
/** One structured-credit deal's equity metrics at a purchase price. */
export interface EquityMetricsRequest extends DealSimulationRequest {
  /** Percent of the equity's current balance; null leaves the price to Rust's default. */
  readonly purchasePricePct: number | null;
}
/** One tranche of a structured-credit deal, priced on a complete market at a valuation date. */
export interface TrancheRequest {
  readonly instrumentJson: string;
  readonly trancheId: string;
  readonly marketJson: string;
  readonly asOf: string;
}
/** One tranche's projected flows and Rust's one-row-per-date projection of them. */
export interface TrancheSchedule {
  readonly cashflows: TrancheCashflows;
  readonly table: TableEnvelope;
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
/** The statements_analytics.wacc arguments, each a decimal as Rust takes it. */
export interface WaccRequest {
  /** Equity share of total capital (0.6 = 60%). */
  readonly equityWeight: number;
  /** Required return on equity (0.115 = 11.5%). */
  readonly costOfEquity: number;
  /** Debt share of total capital; Rust rejects weights that do not sum to one. */
  readonly debtWeight: number;
  /** Pre-tax borrowing yield (0.06 = 6%). */
  readonly costOfDebt: number;
  /** Marginal tax rate in [0, 1] (0.25 = 25%). */
  readonly taxRate: number;
}
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
/** Native CapitalStructureSummary (not a top-level package export). */
export type CapitalStructureSummary = ReturnType<
  typeof statements_analytics.capitalStructureSummary
>;
/** The debt stack of an evaluated model at one period. */
export interface CapitalStructureSummaryRequest {
  /** Host-serialized StatementResult holding cs_cashflows and the configured nodes. */
  readonly resultJson: string;
  /** Canonical FinancialModelSpec JSON whose capital structure gives the instruments and payment classes. */
  readonly modelJson: string;
  /** Rust CapitalStructureSummaryConfig JSON; Rust rejects unknown keys. */
  readonly configJson: string;
}
/** Native RelativeValueReport (not a top-level package export). */
export type RelativeValueReport = ReturnType<
  typeof statements_analytics.relativeValueReport
>;
/** One company's CompanyMetrics serde document, as the report takes it. */
export type CompanyMetrics = Exclude<
  Parameters<typeof statements_analytics.relativeValueReport>[0],
  string
>;
/** One subject against a host-supplied universe of peers. */
export interface RelativeValueReportRequest {
  /** The subject's CompanyMetrics JSON; Rust rejects unknown keys. */
  readonly subjectJson: string;
  /** JSON array of the universe's CompanyMetrics, in the order the report returns peers. */
  readonly universeJson: string;
  /** Rust RelativeValueReportConfig JSON; Rust rejects unknown keys. */
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
/** Native NormalizationResult of one period of the target node (not a top-level package export). */
export type NormalizationResult = ReturnType<
  typeof statements.normalize
>[number];
/** An evaluated result and the NormalizationConfig to apply to it, both as JSON. */
export interface NormalizeRequest {
  /** Host-serialized StatementResult holding the target and every referenced node. */
  readonly resultJson: string;
  /** Canonical NormalizationConfig JSON: the target node and its adjustments in order. */
  readonly configJson: string;
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
/** A Monte Carlo evaluation of a model's stochastic forecasts under Rust MonteCarloConfig JSON. */
export interface EvaluateMonteCarloRequest {
  readonly modelJson: string;
  /** Rust MonteCarloConfig JSON; the uint64 seed stays an exact integer token. */
  readonly configJson: string;
}
/** Breach test of one metric over host-serialized MonteCarloResults. */
export interface MonteCarloBreachRequest {
  /** Host-serialized MonteCarloResults; only results run with path data carry a probability. */
  readonly resultsJson: string;
  readonly metric: string;
  /** Breach level in the metric's own units. */
  readonly threshold: number;
}
/** Native CovenantSpec wire object (a template's specs, ready to wrap as `{ specs }`). */
export type CovenantSpec = covenantsWire.CovenantSpec;
/** Native DatedCovenantReports row: the reports of the covenants applicable on one test date. */
export type DatedCovenantReports = covenantsWire.DatedCovenantReports;
/**
 * A Rust covenant package template and its thresholds, named after the facade
 * function and its parameters. Ratios are in turns (`3.5` means 3.5x), yields
 * and loan-to-value as decimals, amounts in the reporting currency; a cleared
 * field is NaN, which Rust rejects.
 */
export type CovenantTemplateRequest =
  | {
      readonly kind: "covLite";
      readonly maxLeverage: number;
      readonly maxSeniorLeverage: number;
    }
  | {
      readonly kind: "lboStandard";
      readonly initialLeverage: number;
      readonly interestCoverage: number;
      readonly fixedChargeCoverage: number;
      readonly maxCapex: number;
    }
  | {
      readonly kind: "realEstate";
      readonly minDscr: number;
      readonly minDebtYield: number;
      readonly maxLtv: number;
    }
  | {
      readonly kind: "projectFinance";
      readonly minDscr: number;
      readonly distributionLockupDscr: number;
      readonly minLiquidity: number;
      readonly maxNetLeverage: number;
    };
/** Native FutureBreach wire object: one projected breach. */
export type FutureBreach = covenantsWire.FutureBreach;
/** A breach forecast of an evaluated statement result under a CovenantEngine. */
export interface ForecastBreachesRequest extends EvaluateCovenantsRequest {
  /** CovenantForecastConfig JSON; omitted means Rust's deterministic maintenance-scope default. The uint64 random_seed stays an exact integer token. */
  readonly configJson?: string;
}
/** Covenant compliance of an evaluated statement result under a CovenantEngine. */
export interface EvaluateCovenantsRequest {
  /** CovenantEngine JSON; covenant metric ids are statement node ids. */
  readonly engineJson: string;
  /** FinancialModelSpec JSON whose period calendar gives each test date. */
  readonly modelJson: string;
  /** Host-serialized StatementResult of that model. */
  readonly resultsJson: string;
}
/** Roll an evaluated result of `modelJson` up to the native RollupSpec's frequency. */
export interface StatementRollupRequest {
  readonly modelJson: string;
  readonly resultsJson: string;
  readonly specJson: string;
}
/** The rollup methods `modelJson` declares, as a native RollupSpec at `frequency`. */
export interface StatementDefaultRollupRequest {
  readonly modelJson: string;
  readonly frequency: Parameters<typeof statements.rollupSpecDefaultFor>[1];
}
/** Native cashflow envelope object returned by the facade's instrument cashflow export. */
export type { CurrencyLadder };
export type CashflowEnvelope = ReturnType<
  typeof valuations.instruments.instrumentCashflowsWithMarket
>;
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
/** One built-in scenario template, or one component of a composite template. */
export interface TemplateComponentRequest {
  /** Built-in template id from scenarios.listBuiltinTemplateMetadata. */
  readonly templateId: string;
  /** Component id from that template's metadata `components`. */
  readonly componentId: string;
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
  /** scenarios.applicationReportEffectiveAsOf: the rolled valuation date, else the request's. */
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
  /** attribution.pnlAttributionPctOfTotalByBucket of the report's attribution. */
  readonly attributionBuckets: readonly PnlBucketShare[];
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
  /** attribution.pnlAttributionPctOfTotalByBucket: every money bucket with its share of total, in Rust's order. */
  readonly buckets: readonly PnlBucketShare[];
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
export interface PerformanceTickerReport extends Omit<
  PerformanceTickerData,
  "rollingSharpe" | "rollingVolatility"
> {
  rollingSharpe: PerformanceSeries;
  rollingVolatility: PerformanceSeries;
}
/** Native report including the host table envelope and correlation matrix. */
export interface PerformanceReport extends Omit<
  PerformanceReportData,
  "summary" | "tickers"
> {
  summary: TableEnvelope;
  tickers: PerformanceTickerReport[];
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
export interface BenchmarkTickerReport extends Omit<
  BenchmarkTickerData,
  "rollingGreeks" | "multiFactor"
> {
  /** rollingGreeks with its typed arrays copied out. */
  rollingGreeks: Omit<RollingGreeks, "alphas" | "betas"> & {
    alphas: number[];
    betas: number[];
  };
  /** Full native error data is retained for transport consumers. */
  multiFactor: Envelope<MultiFactorResult> | null;
}
/** Native benchmark report with complete per-ticker transport data. */
export interface BenchmarkReport extends Omit<BenchmarkReportData, "tickers"> {
  tickers: BenchmarkTickerReport[];
}
/** The request of one native models.credit.mertonReport. */
export type MertonReportRequest = modelsWire.MertonReportRequest;
/** The native models.credit.mertonReport result. */
export type MertonReport = modelsWire.MertonReport;
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
/** Years from the curve base date, or an ISO date Rust converts with the curve day count. */
export type CurvePoint = number | string;
/** One stored curve or surface of a market and the points to read it at. */
export type CurveQueryRequest = {
  readonly marketJson: string;
  /** The stored id, the MarketContext lookup key. */
  readonly id: string;
} & (
  | {
      readonly kind: "discount";
      readonly points: readonly CurvePoint[];
      /** Start and end of each forward rate, in years: DiscountCurve.forward takes no dates. */
      readonly forwards: readonly (readonly [number, number])[];
    }
  | {
      readonly kind: "forward" | "hazard";
      readonly points: readonly CurvePoint[];
    }
  | {
      readonly kind: "surface";
      readonly points: readonly {
        /** Years. */
        readonly expiry: number;
        /** In the surface's own units. */
        readonly strike: number;
      }[];
    }
);
/** The returned values per supplied point, each with its own native error; nothing is computed in TypeScript. */
export type CurveQueryResult =
  | {
      kind: "discount";
      points: {
        t: CurvePoint;
        df: Envelope<number>;
        zeroRate: Envelope<number>;
      }[];
      forwards: { t1: number; t2: number; rate: Envelope<number> }[];
    }
  | { kind: "forward"; points: { t: CurvePoint; rate: Envelope<number> }[] }
  | {
      kind: "hazard";
      points: {
        t: CurvePoint;
        sp: Envelope<number>;
        hazardRate: Envelope<number>;
      }[];
    }
  | {
      kind: "surface";
      points: { expiry: number; strike: number; vol: Envelope<number> }[];
    };
/** One distressed exchange offer; amounts in one caller-defined unit, no currency conversion. */
export interface ExchangeOfferRequest {
  /** Present value of the existing claim if not tendered. */
  readonly oldPv: number;
  /** Present value of the new instrument received on tendering. */
  readonly newPv: number;
  /** Cash consent or early-tender fee. */
  readonly consentFee: number;
  /** Estimated value of attached equity or warrants. */
  readonly equitySweetenerValue: number;
  readonly exchangeType: modelsWire.ExchangeType;
}
/** One liability management exercise; amounts in one caller-defined unit, percentages as fractions. */
export interface LmeRequest {
  readonly lmeType: modelsWire.LmeType;
  /** Outstanding face of the target instrument. */
  readonly notional: number;
  /** Price, extension fee or transferred-asset fraction, by structure. */
  readonly repurchasePricePct: number;
  /** Fraction of holders participating. */
  readonly optAcceptancePct: number;
  /** Unset leaves the leverage block out. */
  readonly ebitda?: number | null;
}
/** One absolute-priority recovery waterfall; amounts in one caller-defined unit, no currency conversion. */
export interface RecoveryWaterfallRequest {
  /** Distributable estate, including pledged collateral. */
  readonly estateValue: number;
  /** The claims as Rust takes them; seniority is a caller-defined reporting label. */
  readonly claims: readonly modelsWire.RecoveryClaim[];
}
/** One workout LGD in the facade's positional order; costs and rates are decimals. */
export interface WorkoutLgdRequest {
  /** Exposure at default, positive, in the collateral's unit. */
  readonly ead: number;
  /** Collateral pieces, highest priority first. */
  readonly collateral: readonly (readonly [
    collateralType: modelsWire.WorkoutCollateralType,
    bookValue: number,
    haircut: number,
  ])[];
  readonly directCostPct: number;
  readonly indirectCostPct: number;
  readonly timeToResolutionYears: number;
  readonly discountRate: number;
}
/** One seniority class under one exact registry calibration id. */
export interface SeniorityRecoveryRequest {
  readonly seniority: modelsWire.SeniorityClass;
  readonly calibrationId: string;
}
/** The getters of the BetaRecovery handle seniorityRecoveryStats returns, under their Rust names; rates are fractions. */
export interface SeniorityRecoveryStats {
  readonly mean: number;
  readonly std_dev: number;
  readonly variance: number;
  readonly alpha: number;
  readonly beta_param: number;
  /** Undefined when the distribution has no interior mode. */
  readonly mode: number | undefined;
  readonly mean_lgd: number;
}
/** Terms of one European option; each model takes its own, as the facade function documents them. */
export type OptionTerms =
  | {
      readonly model: "black_scholes";
      readonly spot: number;
      readonly strike: number;
      /** Risk-free rate, decimal, continuously compounded. */
      readonly rate: number;
      /** Dividend yield or foreign rate, decimal, continuously compounded. */
      readonly divYield: number;
      /** Years. */
      readonly expiry: number;
      readonly isCall: boolean;
      /** Theta denominator; omitted is left to Rust's default. */
      readonly thetaDaysPerYear?: number | null;
    }
  | {
      readonly model: "black76";
      readonly forward: number;
      readonly strike: number;
      /** Discount factor from valuation to expiry. */
      readonly df: number;
      /** Years. */
      readonly expiry: number;
      readonly isCall: boolean;
    }
  | {
      readonly model: "bachelier";
      readonly forward: number;
      readonly strike: number;
      /** Years. */
      readonly expiry: number;
      readonly isCall: boolean;
    };
/** An option's terms with a volatility to price, or a price to invert: per unit, discounted under Black-76, undiscounted under Bachelier. */
export type OptionCalculatorRequest = OptionTerms &
  (
    | { readonly mode: "price"; readonly vol: number }
    | { readonly mode: "implied_vol"; readonly price: number }
  );
/** The returned price or implied volatility and the Greeks at that volatility; nothing is computed in TypeScript. */
export type OptionCalculatorResult = {
  model: OptionTerms["model"];
  /** BsGreeks under Black-Scholes, ForwardGreeks otherwise, or Rust's error of that one call. */
  greeks: Envelope<modelsWire.BsGreeks | modelsWire.ForwardGreeks>;
} & (
  { mode: "price"; price: number } | { mode: "implied_vol"; impliedVol: number }
);
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
  /** Target shares of portfolio VaR per position for models.factor.risk.evaluateRiskBudgetFromDecomposition; omitted skips the budget. */
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
  /** evaluateRiskBudgetFromDecomposition over the decomposition, or its native error; null without a budget. */
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
/** A book attributed between two host-supplied market snapshots; every other option is left to Rust. */
export interface PortfolioAttributionRequest {
  /** PortfolioSpec JSON; canonicalized (Portfolio#toJson) through the shared portfolio handle cache. */
  readonly specJson: string;
  /** MarketContext JSON of the opening snapshot; canonicalized through the shared market cache. */
  readonly marketT0Json: string;
  /** MarketContext JSON of the closing snapshot; canonicalized through the shared market cache. */
  readonly marketT1Json: string;
  /** ISO date of the opening snapshot. */
  readonly asOfT0: string;
  /** ISO date of the closing snapshot. */
  readonly asOfT1: string;
  /** AttributionMethod wire value; Rust requires one. */
  readonly method: attributionWire.AttributionMethod;
  /** Largest absolute reconciliation gap, in base-currency units, Rust still reports as reconciled; host-supplied, Rust has no default. */
  readonly reconciliationTolerance: number;
}
/** One position's native-currency attribution with the Rust helpers its report shows. */
export interface PortfolioPositionAttribution {
  /** Position id, in the order Rust keyed by_position. */
  readonly positionId: string;
  /** The position's PnlAttribution from by_position, in its instrument's currency. */
  readonly attribution: PnlAttribution;
  /** attribution.pnlAttributionExplainText of that attribution. */
  readonly explain: string;
  /** attribution.pnlAttributionResidualWithinTolerance against the position run's own tolerances. */
  readonly residualWithinTolerance: boolean;
  /** attribution.pnlAttributionPctOfTotalByBucket of that attribution. */
  readonly buckets: readonly PnlBucketShare[];
}
/** A native portfolio attribution with the Rust helpers the report shows beside it. */
export interface PortfolioAttributionReport {
  /** portfolio.attributePortfolioPnl, as returned. */
  readonly attribution: PortfolioAttribution;
  /** portfolio.portfolioAttributionExplainText. */
  readonly explain: string;
  /** portfolio.portfolioAttributionReconciliationCheck at the requested tolerance. */
  readonly reconciliation: ReconciliationReport;
  /** portfolio.portfolioAttributionPctOfTotalByBucket: every base-currency bucket with its share of total, in Rust's order. */
  readonly buckets: readonly PnlBucketShare[];
  /** Each by_position entry with Rust's explain text, residual check and buckets, in by_position order. */
  readonly positions: readonly PortfolioPositionAttribution[];
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
/** A stored Black lognormal strike-axis surface and host-supplied forwards for models.volatility.checkSurface. */
export interface CheckSurfaceRequest {
  readonly surface: StoredSurfaceWire;
  /** Forward prices in the surface's strike units: one to broadcast or one per expiry. Host-supplied only; Rust rejects a missing, non-positive or mis-sized list. */
  readonly forwards: readonly number[];
  /** Non-negative tolerance in total-variance units; omitted leaves Rust's DEFAULT_ARBITRAGE_TOLERANCE. */
  readonly tolerance?: number | null;
}
/** The native butterfly, calendar-spread and local-vol density report, violations most severe first. */
export type ArbitrageReport = modelsWire.ArbitrageReport;
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
  /** MarketContext.getDiscount (df, zeroRate, forward), getForward (rate), getHazard (sp, hazardRate) or getSurface (models.volatility.getSurfaceVol), per supplied point, over the cached market handle; each point keeps its own native error and the curve handle is freed. */
  curveQuery(request: CurveQueryRequest): Promise<Envelope<CurveQueryResult>>;
  /** core.availableCalendars. */
  calendars(): Promise<Envelope<string[]>>;
}
/** valuations: instrument validation, pricing, cashflows, scenarios, registries and export. */
export interface ValuationsApi {
  /** valuations.instruments.validateInstrumentJson + priceInstrumentWithMarket over the cached market handle. */
  price(request: PriceRequest): Promise<Envelope<ValuationResult>>;
  /** valuations.instruments.priceInstrumentWithMarket once per supplied metric id: one failing id fails a whole native request, so each id keeps its own value or native error. */
  metricValues(request: MetricValuesRequest): Promise<Envelope<MetricValue[]>>;
  /** valuations.instruments.validateInstrumentJson: validate and canonicalize instrument JSON. */
  validate(instrumentJson: string): Promise<Envelope<string>>;
  /** valuations.instruments.validateInstrumentJson + instrumentCashflowsWithMarket over the cached market handle; the native envelope object. */
  cashflows(request: CashflowRequest): Promise<Envelope<CashflowEnvelope>>;
  /** valuations.instruments.standardDealFamilies: the families newStandard accepts, each with its registry profile id and example inputs. */
  standardDealFamilies(): Promise<Envelope<StandardDealFamily[]>>;
  /** valuations.instruments.StructuredCredit.newStandard + toJson: the canonical instrument JSON of a deal under its family's registry assumptions. */
  newStandardDeal(request: StandardDealRequest): Promise<Envelope<string>>;
  /** valuations.instruments.StructuredCredit.fromJson + its tranches getter. */
  dealTranches(instrumentJson: string): Promise<Envelope<DealTranches>>;
  /** valuations.instruments.StructuredCredit.fromJson + its trancheCashflows, handed to trancheCashflowsTable; dates are joined and summed in Rust. */
  trancheSchedule(request: TrancheRequest): Promise<Envelope<TrancheSchedule>>;
  /** StructuredCredit.runSimulation: the deal's tranche flows and accounting by period. */
  runSimulation(
    request: DealSimulationRequest,
  ): Promise<Envelope<SimulationRun>>;
  /** StructuredCredit.equityMetrics: the deal's equity tranche, or the native failure when it has none. */
  equityMetrics(
    request: EquityMetricsRequest,
  ): Promise<Envelope<EquityMetrics>>;
  /** valuations.instruments.ConvertibleBond.fromJson + convertibleSpotLadder: the bond at each share price. */
  convertibleSpotLadder(
    request: ConvertibleSpotLadderRequest,
  ): Promise<Envelope<ConvertibleSpotLadder>>;
  /** valuations.instruments.Bond.fromJson + bondWorkoutLadder: yield and I-spread to every exit. */
  bondWorkoutLadder(
    request: BondWorkoutLadderRequest,
  ): Promise<Envelope<BondWorkoutLadder>>;
  /** valuations.instruments.Bond.fromJson + computeQuotes: the whole quote set from any one quote. */
  computeQuotes(request: ComputeQuotesRequest): Promise<Envelope<BondQuoteSet>>;
  /** valuations.instruments.poolReport over the deal's pool: statistics, WARF and concentration. */
  poolReport(request: PoolReportRequest): Promise<Envelope<PoolReport>>;
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
  /** valuations.instruments.metricsForInstrument: every metric registered for one instrument type, by native display group, with those outside a group under "Instrument Specific". */
  metrics(instrumentType: string): Promise<Envelope<Record<string, string[]>>>;
  /** valuations.instruments.metricMetadata: native per-key interpretation (units, decoded coordinates, groups) for metric keys. */
  metricMetadata(keys: readonly string[]): Promise<Envelope<MetricMetadata[]>>;
  /** valuations.instruments.priceDriverFields: wire names of the mutually exclusive price-driving market quotes. */
  priceDriverFields(): Promise<Envelope<string[]>>;
  /** valuations.instruments.trancheMetricFieldUnits: units of the scalar TrancheMetrics fields, in struct order. */
  trancheMetricFieldUnits(
    currency: string,
  ): Promise<Envelope<MetricFieldUnit[]>>;
  /** valuations.instruments.scenarioTableFieldUnits: units of a scenario table's axes and measures. */
  scenarioTableFieldUnits(
    currency: string,
  ): Promise<Envelope<ScenarioTableFieldUnits>>;
  /** valuations.instruments.simulationFieldUnits: units of a simulation's charted series. */
  simulationFieldUnits(
    currency: string,
  ): Promise<Envelope<SimulationFieldUnits>>;
  /** valuations.instruments.equityMetricsFieldUnits: units of an equity tranche's metrics. */
  equityMetricsFieldUnits(
    currency: string,
  ): Promise<Envelope<EquityMetricsFieldUnits>>;
  /** valuations.instruments.bondQuoteSetFieldUnits: units of a quote set's fields. */
  bondQuoteSetFieldUnits(
    currency: string,
  ): Promise<Envelope<MetricFieldUnit[]>>;
  /** valuations.instruments.poolReportFieldUnits: units of a pool report's fields. */
  poolReportFieldUnits(
    currency: string,
  ): Promise<Envelope<PoolReportFieldUnits>>;
  /** valuations.instruments.trancheSolveFieldUnits: units of the break-even CDR, discount margin and OAS results. */
  trancheSolveFieldUnits(): Promise<Envelope<TrancheSolveFieldUnits>>;
  /** valuations.valuationResultToJson over the host-serialized result: native canonical JSON. */
  exportResult(value: ValuationResult): Promise<Envelope<string>>;
}
/** Aggregation of an exported cashflow envelope. */
export interface CashflowsApi {
  /** valuations.instruments.calendarYearLadderByCurrency: one native ladder per row currency, in first-appearance order. */
  cashflowLadder(
    cashflows: CashflowEnvelope,
  ): Promise<Envelope<CurrencyLadder[]>>;
}
/** covenants: package templates, engine validation and statement covenant compliance. */
export interface CovenantsApi {
  /** covenants.covLite, lboStandard, realEstate or projectFinance, by request kind: the Rust package's CovenantSpec objects. */
  covenantTemplate(
    request: CovenantTemplateRequest,
  ): Promise<Envelope<CovenantSpec[]>>;
  /** covenants.validateCovenantEngineJson: canonical CovenantEngine text. */
  validateCovenantEngine(json: string): Promise<Envelope<string>>;
  /** covenants.CovenantEngine.fromJson + statements_analytics.evaluateCovenants: one row per result period, tested on its last day with the applicable covenants; the handle is freed. */
  evaluateCovenants(
    request: EvaluateCovenantsRequest,
  ): Promise<Envelope<DatedCovenantReports[]>>;
  /** covenants.CovenantEngine.fromJson + statements_analytics.forecastBreaches (config passed only when supplied): breaches sorted by date, then covenant id; the handle is freed. */
  forecastBreaches(
    request: ForecastBreachesRequest,
  ): Promise<Envelope<FutureBreach[]>>;
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
  /** scenarios.parseScenarioSpec: canonical ScenarioSpec text, or the native error. */
  validateScenarioSpec(json: string): Promise<Envelope<string>>;
  /** scenarios.listBuiltinTemplateMetadata: every built-in template in registry order. */
  listBuiltinTemplateMetadata(): Promise<Envelope<TemplateMetadata[]>>;
  /** scenarios.buildFromTemplate: the whole built-in template as a ScenarioSpec. */
  buildFromTemplate(templateId: string): Promise<Envelope<ScenarioSpec>>;
  /** scenarios.buildTemplateComponent: one component of a built-in composite template. */
  buildTemplateComponent(
    request: TemplateComponentRequest,
  ): Promise<Envelope<ScenarioSpec>>;
  /** scenarios.composeScenarios over the ScenarioSpec texts, in Rust's priority order. */
  composeScenarios(
    specJsons: readonly string[],
  ): Promise<Envelope<ScenarioSpec>>;
}
/** attribution: single-instrument T0 -> T1 P&L attribution and its Rust helpers. */
export interface AttributionApi {
  /** attribution.attributePnlMany over canonical market texts, then pnlAttributionExplainText, pnlAttributionResidualWithinTolerance and pnlAttributionPctOfTotalByBucket; one entry per instrument in input order. */
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
  /** portfolio.attributePortfolioPnl over the cached Portfolio and both MarketContext handles, then portfolioAttributionExplainText, portfolioAttributionReconciliationCheck and portfolioAttributionPctOfTotalByBucket, and per by_position entry attribution.pnlAttributionExplainText, pnlAttributionResidualWithinTolerance and pnlAttributionPctOfTotalByBucket. */
  attributePortfolioPnl(
    request: PortfolioAttributionRequest,
  ): Promise<Envelope<PortfolioAttributionReport>>;
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
  /** core.VolSurface.fromJson + models.volatility.checkSurface with every check enabled and the supplied forwards (and tolerance, when given); the handle is freed. */
  checkSurface(
    request: CheckSurfaceRequest,
  ): Promise<Envelope<ArbitrageReport>>;
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
  /** models.credit.mertonReport: the full report in one native call; a measure Rust rejects for the model is null with its message in errors. */
  mertonReport(request: MertonReportRequest): Promise<Envelope<MertonReport>>;
  /** One models.credit.scoring function over its input ratios in the facade's positional order, then models.credit.pd.MasterScale.defaultCalibration().mapScore of the result (kept as an envelope); the scale handle is freed. */
  creditScore(request: CreditScoreRequest): Promise<Envelope<CreditScore>>;
  /** models.credit.liabilityManagement.analyzeExchangeOffer, including Rust's tender_recommended. */
  analyzeExchangeOffer(
    request: ExchangeOfferRequest,
  ): Promise<Envelope<modelsWire.ExchangeOfferAnalysis>>;
  /** models.credit.liabilityManagement.analyzeLme. */
  analyzeLme(request: LmeRequest): Promise<Envelope<modelsWire.LmeAnalysis>>;
  /** models.credit.liabilityManagement.tenderRecommendationHurdle. */
  tenderRecommendationHurdle(): Promise<Envelope<number>>;
  /** models.credit.recoveryWaterfall.allocateRecovery: per-claim allocations, totals and the undistributed estate. */
  allocateRecovery(
    request: RecoveryWaterfallRequest,
  ): Promise<Envelope<modelsWire.RecoveryWaterfallResult>>;
  /** models.credit.lgd.workoutLgd: net recovery, LGD and recovery rate. */
  workoutLgd(
    request: WorkoutLgdRequest,
  ): Promise<Envelope<modelsWire.WorkoutLgdResult>>;
  /** models.credit.lgd.seniorityRecoveryStats, read into plain data; the handle is freed. */
  seniorityRecoveryStats(
    request: SeniorityRecoveryRequest,
  ): Promise<Envelope<SeniorityRecoveryStats>>;
  /** models.credit.lgd.seniorityCalibrationIds: the registry's calibration ids, default first. */
  seniorityCalibrationIds(): Promise<Envelope<string[]>>;
  /** One closed-form price (bsPrice, black76Price, bachelierPrice) or implied-volatility solve (bsImpliedVol, black76ImpliedVol, models.volatility.impliedVolBachelier) in the facade's positional order, then the model's Greeks at that volatility, kept as an envelope. */
  optionCalculator(
    request: OptionCalculatorRequest,
  ): Promise<Envelope<OptionCalculatorResult>>;
  /** models.factor.risk parametric or historical VaR/ES decomposition, evaluateRiskBudgetFromDecomposition, and buildStressAttribution of a historical source; the budget and stress attribution keep their own native errors. */
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
  /** statements.validateNormalizationConfigJson: validate and canonicalize NormalizationConfig JSON. */
  validateNormalizationConfig(json: string): Promise<Envelope<string>>;
  /** statements.normalize: one NormalizationResult per period of the target node, in period order. */
  normalize(
    request: NormalizeRequest,
  ): Promise<Envelope<NormalizationResult[]>>;
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
  /** statements.rollupSpecDefaultFor: the rollup methods the model's lines declare. */
  defaultRollupSpec(
    request: StatementDefaultRollupRequest,
  ): Promise<Envelope<ReturnType<typeof statements.rollupSpecDefaultFor>>>;
  /** statements_analytics.creditAssessment: leverage, interest coverage and free cash flow at a period, with the per-period series. */
  creditAssessment(
    request: CreditAssessmentRequest,
  ): Promise<Envelope<CreditAssessment>>;
  /** statements.statementResultToTableWide: one row per period, one column per node; a node with no value holds NaN. */
  statementResultToTableWide(
    resultJson: string,
  ): Promise<Envelope<TableEnvelope>>;
  /** statements.statementResultToTableLong: one row per node and period. */
  statementResultToTableLong(
    resultJson: string,
  ): Promise<Envelope<TableEnvelope>>;
  /** statements_analytics.wacc: the blended rate as a decimal. */
  wacc(request: WaccRequest): Promise<Envelope<number>>;
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
  /** statements_analytics.capitalStructureSummary: the debt stack by seniority, leverage and coverage through each class, and the maturity wall. */
  capitalStructureSummary(
    request: CapitalStructureSummaryRequest,
  ): Promise<Envelope<CapitalStructureSummary>>;
  /** statements_analytics.relativeValueReport: the comps table, peer statistics, regression scatter and composite score of one subject against its filtered peers. */
  relativeValueReport(
    request: RelativeValueReportRequest,
  ): Promise<Envelope<RelativeValueReport>>;
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
  /** statements.Evaluator.evaluateMonteCarlo: percentile summaries per metric and, when requested, the path table. */
  evaluateMonteCarlo(
    request: EvaluateMonteCarloRequest,
  ): Promise<Envelope<MonteCarloResults>>;
  /** statements.monteCarloBreachProbability: fraction of paths above the threshold in any forecast period, or null when Rust returns none. */
  monteCarloBreachProbability(
    request: MonteCarloBreachRequest,
  ): Promise<Envelope<number | null>>;
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
  CovenantsApi &
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
