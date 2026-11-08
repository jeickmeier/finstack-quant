import type {
  ApplicationReport,
  CalibrationReport,
  CheckReport,
  ForecastMetrics,
  GoalSeekResult,
  HorizonReport,
  ImResult,
  MarginCall,
  MetricMetadata,
  MonteCarloResults,
  PnlAttribution,
  PnlBucketShare,
  PortfolioMarginResult,
  ScenarioPnlBatchItem,
  StatementResult,
  StructuredCredit,
  ValuationResult,
  VmResult,
} from "finstack-quant-wasm";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import type {
  BenchmarkReportData,
  PerformanceReportData,
} from "@/components/finstack/analytics/lib/analytics-report-types";
import type { FactorRiskData } from "@/components/finstack/models/components/portfolio-risk-report/portfolio-risk-report";
import type {
  MigrationMatrixData,
  MigrationProjectionData,
} from "@/components/finstack/models/components/transition-matrix-heatmap/transition-matrix-heatmap";
import type { PortfolioAttributionData } from "@/components/finstack/portfolio/components/portfolio-attribution-report/portfolio-attribution-report";
import type {
  PortfolioMetricSeriesData,
  PortfolioMetricsData,
} from "@/components/finstack/portfolio/components/portfolio-metrics/portfolio-metrics";
import type { PortfolioSummaryData } from "@/components/finstack/portfolio/components/portfolio-summary/portfolio-summary";
import type { PositionsTableData } from "@/components/finstack/portfolio/components/positions-table/positions-table";
import type { CorporateValuationResultWire } from "@/components/finstack/statements/components/dcf-report/dcf-report";
import type { GoalSeekDefaults } from "@/components/finstack/statements/components/goal-seek-form/goal-seek-form";
import type { VarianceReportWire } from "@/components/finstack/statements/components/variance-table/variance-table";
import type { CashflowLadder } from "@/components/finstack/valuations/components/cashflow-viewer/cashflow-data";
import type { MetricValueRow } from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import type { TrancheScheduleValue } from "@/components/finstack/valuations/components/tranche-analytics/tranche-analytics";
import type { CreditAssessmentWire } from "@/lib/finstack/generated/types/credit_assessment";
import type { DatedCovenantReportsWire } from "@/lib/finstack/generated/types/dated_covenant_reports";
import type { DcfSensitivityResultWire } from "@/lib/finstack/generated/types/dcf_sensitivity_result";
import type { FutureBreachWire } from "@/lib/finstack/generated/types/future_breach";
import type { InstrumentCashflowWire } from "@/lib/finstack/generated/types/instrument_cashflow";
import type { LboResultWire } from "@/lib/finstack/generated/types/lbo_result";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import type {
  BridgeChart,
  StatementSensitivity,
  WorkoutLgdRequest,
} from "@/workers/finstack-contract";
import data from "./data.json";

// A JSON import widens Rust's string unions and nullable fields, so each
// native section of data.json is retyped once here, as the type it holds.

/** `T` with the fields of `O` retyped. */
type Retyped<T, O> = Omit<T, keyof O> & O;
/** The entries of a fixture case list with the fields of `O` retyped. */
type Cases<T extends readonly unknown[], O> = Retyped<T[number], O>[];
type SimulationRun = ReturnType<StructuredCredit["runSimulation"]>;
type MetricCase = { rows: MetricValueRow[]; metadata: MetricMetadata[] };

export const bondResult = data.bond.result as unknown as ValuationResult;
export const supplementalMarket = data.market
  .supplemental as unknown as MarketContextStateWire;
export const curveQueryMarket = data.curveQuery
  .market as unknown as MarketContextStateWire;
export const cubesMarket = data.cubes
  .market as unknown as MarketContextStateWire;
/** The calibration cases; the last carries diagnostics. */
export const calibration = data.calibration as unknown as Cases<
  typeof data.calibration,
  {
    result: Retyped<
      (typeof data.calibration)[number]["result"],
      {
        result: Retyped<
          (typeof data.calibration)[number]["result"]["result"],
          {
            step_reports: Record<string, CalibrationReport>;
            final_market: MarketContextStateWire;
          }
        >;
      }
    >;
  }
>;
export const bondCredit = data.bondCredit as unknown as Retyped<
  typeof data.bondCredit,
  {
    metadata: MetricMetadata[];
    cases: {
      [K in keyof typeof data.bondCredit.cases]: Retyped<
        (typeof data.bondCredit.cases)[K],
        { rows: MetricValueRow[] }
      >;
    };
  }
>;
export const loanYield = data.loanYield as unknown as Retyped<
  typeof data.loanYield,
  {
    cases: {
      [K in keyof typeof data.loanYield.cases]: Retyped<
        (typeof data.loanYield.cases)[K],
        MetricCase
      >;
    };
  }
>;
export const instrumentCashflows = data.cashflows as unknown as Retyped<
  typeof data.cashflows,
  {
    cases: Cases<
      typeof data.cashflows.cases,
      { cashflows: InstrumentCashflowWire; ladder: CashflowLadder[] }
    >;
  }
>;
export const trancheAnalytics = data.trancheAnalytics as unknown as Retyped<
  typeof data.trancheAnalytics,
  {
    cases: Cases<
      typeof data.trancheAnalytics.cases,
      {
        results: Retyped<
          (typeof data.trancheAnalytics.cases)[number]["results"],
          { schedule: { value: TrancheScheduleValue } }
        >;
      }
    >;
  }
>;
export const waterfallReport = data.waterfallReport as unknown as Retyped<
  typeof data.waterfallReport,
  { cases: Cases<typeof data.waterfallReport.cases, { run: SimulationRun }> }
>;
export const dealCharts = data.dealCharts as unknown as Retyped<
  typeof data.dealCharts,
  { cases: Cases<typeof data.dealCharts.cases, { run: SimulationRun }> }
>;
export const equityTranche = data.equityTranche as unknown as Retyped<
  typeof data.equityTranche,
  {
    cases: Cases<
      typeof data.equityTranche.cases,
      {
        result: { value: ReturnType<StructuredCredit["equityMetrics"]> };
      }
    >;
  }
>;
export const marketScenario = data.marketScenario as unknown as Retyped<
  typeof data.marketScenario,
  {
    cases: Cases<
      typeof data.marketScenario.cases,
      { report: ApplicationReport }
    >;
  }
>;
export const horizonReturn = data.horizonReturn as unknown as Retyped<
  typeof data.horizonReturn,
  {
    cases: Cases<
      typeof data.horizonReturn.cases,
      { report: HorizonReport; attributionBuckets: PnlBucketShare[] }
    >;
  }
>;
export const pnlAttribution = data.pnlAttribution as unknown as Retyped<
  typeof data.pnlAttribution,
  {
    cases: Cases<
      typeof data.pnlAttribution.cases,
      { attribution: PnlAttribution; buckets: PnlBucketShare[] }
    >;
  }
>;
export const performanceReport = data.performance
  .report as unknown as PerformanceReportData;
// This numeric result array uses the core's wire tokens in stored JSON;
// restore native IEEE values for the display-only report. Ticker labels stay text.
export const benchmarkReport = {
  ...data.benchmark.report,
  informationRatio: data.benchmark.report.informationRatio.map((value) =>
    value === "nan"
      ? Number.NaN
      : value === "inf"
        ? Number.POSITIVE_INFINITY
        : value === "-inf"
          ? Number.NEGATIVE_INFINITY
          : value,
  ),
} as unknown as BenchmarkReportData;
export const migration = data.credit as unknown as Retyped<
  typeof data.credit,
  { matrix: MigrationMatrixData; projection: MigrationProjectionData }
>;
export const positionRisk = data.positionRisk as unknown as Retyped<
  typeof data.positionRisk,
  {
    parametric: { report: modelsWire.PositionRiskReport };
    historical: { report: modelsWire.PositionRiskReport };
    factor: { report: FactorRiskData };
  }
>;
export const workoutLgdRequest = data.recovery.workout.secured
  .request as unknown as WorkoutLgdRequest;
export const simmResult = data.simm.example.result as unknown as ImResult;
export const vm = data.vm as unknown as Retyped<
  typeof data.vm,
  {
    vm: Retyped<
      typeof data.vm.vm,
      {
        collect: Retyped<
          typeof data.vm.vm.collect,
          {
            report: Retyped<
              typeof data.vm.vm.collect.report,
              { result: VmResult }
            >;
          }
        >;
      }
    >;
    calls: Retyped<typeof data.vm.calls, { result: MarginCall[] }>;
  }
>;
/** The gallery book's valuation, which every portfolio display reads. */
export const portfolioValuation = data.portfolioValuation as unknown as Retyped<
  typeof data.portfolioValuation,
  {
    report: PositionsTableData & PortfolioMetricsData & PortfolioSummaryData;
    series: PortfolioMetricSeriesData;
    metadata: MetricMetadata[];
  }
>;
export const scenarioPnlItems = data.scenarioPnl.batch
  .items as unknown as ScenarioPnlBatchItem[];
export const portfolioMargin = data.portfolioMargin as unknown as Retyped<
  typeof data.portfolioMargin,
  {
    cases: Cases<
      typeof data.portfolioMargin.cases,
      { result: PortfolioMarginResult }
    >;
  }
>;
export const portfolioAttribution =
  data.portfolioAttribution as unknown as Retyped<
    typeof data.portfolioAttribution,
    {
      cases: Cases<
        typeof data.portfolioAttribution.cases,
        { report: PortfolioAttributionData }
      >;
    }
  >;
export const statementResult =
  data.statementResult as unknown as StatementResult;
export const statementCheckReport =
  data.statementCheckReport as unknown as CheckReport;
export const statementPlSummary =
  data.statementPlSummary as unknown as TableEnvelopeWire;
export const statementCreditAssessment =
  data.statementCreditAssessment as unknown as CreditAssessmentWire;
export const statementDcf =
  data.statementDcf as unknown as CorporateValuationResultWire;
export const statementDcfSensitivity =
  data.statementDcfSensitivity as unknown as DcfSensitivityResultWire;
export const statementLbo = data.statementLbo as unknown as LboResultWire;
export const statementSensitivity =
  data.statementSensitivity as unknown as StatementSensitivity;
export const statementGoalSeekRun =
  data.statementGoalSeekRun as unknown as GoalSeekDefaults;
export const statementGoalSeek =
  data.statementGoalSeek as unknown as GoalSeekResult;
export const statementBridge = data.statementBridge as unknown as BridgeChart;
export const statementBacktest =
  data.statementBacktest as unknown as ForecastMetrics;
export const statementMonteCarlo =
  data.statementMonteCarlo as unknown as MonteCarloResults;
export const statementScenarioDiff =
  data.statementScenarioDiff as unknown as Cases<
    typeof data.statementScenarioDiff,
    { variance: VarianceReportWire }
  >;
export const covenantSeries =
  data.covenantSeries as unknown as DatedCovenantReportsWire[];
export const covenantBreachForecast =
  data.covenantBreachForecast as unknown as Retyped<
    typeof data.covenantBreachForecast,
    { stochastic: FutureBreachWire[] }
  >;
