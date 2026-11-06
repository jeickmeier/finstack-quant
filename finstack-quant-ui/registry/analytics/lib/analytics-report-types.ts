import type {
  BetaResult,
  DatedSeries,
  DrawdownEpisode,
  GreeksResult,
  LookbackReturns,
  MultiFactorResult,
  PeriodicReturn,
} from "finstack-quant-wasm";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";

/** One ticker's native series, each aligned with that ticker's own active dates. */
export interface PerformanceTickerData {
  /** getActiveDatesForTicker: dates aligned with cumulative returns and drawdowns. */
  activeDates: readonly string[];
  cumulativeReturns: readonly number[];
  drawdowns: readonly number[];
  /** drawdownDetails, largest first. */
  drawdownEpisodes: readonly DrawdownEpisode[];
  rollingSharpe: Omit<DatedSeries, "values"> & { values: readonly number[] };
  rollingVolatility: Omit<DatedSeries, "values"> & {
    values: readonly number[];
  };
}
/** The native analytics.Performance results the worker's performanceReport returns. */
export interface PerformanceReportData {
  tickerNames: readonly string[];
  /** Index of the named benchmark; null when none was named. */
  benchmarkIdx: number | null;
  frequency: string;
  activeDates: readonly string[];
  /** Performance.summary() as returned. */
  summary: TableEnvelopeWire;
  /** In tickerNames order. */
  tickers: readonly PerformanceTickerData[];
  /** periodicReturns("monthly"), ticker-major. */
  monthly: readonly (readonly PeriodicReturn[])[];
  /** periodicReturns("annual"), ticker-major. */
  annual: readonly (readonly PeriodicReturn[])[];
  lookback: {
    /** Requested lookback date, else the last active panel date. */
    refDate: string;
    returns: LookbackReturns;
    /** Fiscal-year start passed to Rust, else null for calendar year. */
    fiscalYearStart: { month: number; day: number } | null;
  };
}

/** One ticker's native relative series, each aligned with that ticker's own active dates. */
export interface BenchmarkTickerData {
  activeDates: readonly string[];
  /** cumulativeReturnsOutperformance, aligned to activeDates. */
  outperformance: readonly number[];
  drawdownDifference: readonly number[];
  rollingGreeks: {
    dates: readonly string[];
    alphas: readonly number[];
    betas: readonly number[];
  };
  /** The native regression, or its native error; null without a factor panel. */
  multiFactor:
    | { ok: true; value: MultiFactorResult }
    | { ok: false; error: { message: string } }
    | null;
}
/** The native analytics.Performance results the worker's benchmarkReport returns. */
export interface BenchmarkReportData {
  tickerNames: readonly string[];
  benchmarkIdx: number;
  frequency: string;
  activeDates: readonly string[];
  /** Factor labels of the regression, else null. */
  factorNames: readonly string[] | null;
  greeks: readonly GreeksResult[];
  beta: readonly BetaResult[];
  trackingError: readonly number[];
  informationRatio: readonly number[];
  treynor: readonly number[];
  mSquared: readonly number[];
  upCapture: readonly number[];
  downCapture: readonly number[];
  captureRatio: readonly number[];
  battingAverage: readonly number[];
  tickers: readonly BenchmarkTickerData[];
}
