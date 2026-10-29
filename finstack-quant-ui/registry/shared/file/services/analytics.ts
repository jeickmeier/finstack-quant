import type {
  analytics,
  core,
  DatedSeries,
  FiscalConfig,
} from "finstack-quant-wasm";
import {
  errorValue,
  type AnalyticsApi,
  type BenchmarkReport,
  type BenchmarkReportRequest,
  type Envelope,
  type PerformanceReport,
  type PerformanceReportRequest,
  type PerformanceSeries,
} from "../finstack-contract";
import type { ServiceContext } from "./context";
/** analytics and core facade members this slice reads. */
export type AnalyticsNative = {
  analytics: Pick<typeof analytics, "Performance">;
  core: Pick<typeof core, "FiscalConfig">;
};
/** A native rolling series with its typed array copied out (NaN windows kept). */
const series = (value: DatedSeries): PerformanceSeries => ({
  ...value,
  values: Array.from(value.values),
});
/** Omitted options stay unset so Rust applies its own defaults. */
const option = <T>(value: T | null | undefined) => value ?? undefined;
/** A native call's value, or its error when that one call is rejected. */
function attempt<T>(call: () => T): Envelope<T> {
  try {
    return { ok: true, value: call() };
  } catch (error) {
    return { ok: false, error: errorValue(error) };
  }
}
/** AnalyticsApi over the analytics facade namespace; handles never outlive a call. */
export function createAnalyticsService(
  native: AnalyticsNative,
  context: ServiceContext,
): AnalyticsApi {
  type Engine = InstanceType<AnalyticsNative["analytics"]["Performance"]>;
  /** fromReturns or the price constructor over a host panel. */
  const build = (
    request: Pick<
      PerformanceReportRequest,
      | "dates"
      | "values"
      | "kind"
      | "tickerNames"
      | "benchmarkTicker"
      | "frequency"
    >,
  ): Engine => {
    const { Performance: Engine } = native.analytics;
    const args: Parameters<typeof Engine.fromReturns> = [
      [...request.dates],
      request.values.map((row) => [...row]),
      [...request.tickerNames],
      option(request.benchmarkTicker),
      option(request.frequency),
    ];
    return request.kind === "returns"
      ? Engine.fromReturns(...args)
      : new Engine(...args);
  };
  return {
    performanceReport(request) {
      return context.result((): PerformanceReport => {
        let performance: Engine | undefined;
        let fiscal: FiscalConfig | undefined;
        try {
          performance = build(request);
          const handle = performance;
          if (request.fiscalYearStart)
            fiscal = new native.core.FiscalConfig(
              request.fiscalYearStart.month,
              request.fiscalYearStart.day,
            );
          const window = option(request.window);
          const rate = option(request.riskFreeRate);
          const activeDates = handle.getActiveDates();
          const cumulative = handle.cumulativeReturns();
          const drawdowns = handle.getDrawdownSeries();
          const tickerNames = handle.getTickerNames();
          // The lookback date defaults to the last native active date; Rust rejects a missing one.
          const refDate = request.lookbackDate ?? activeDates.at(-1)!;
          const correlation = handle.correlationMatrix();
          return {
            tickerNames,
            benchmarkIdx: request.benchmarkTicker
              ? handle.getBenchmarkIdx()
              : null,
            frequency: handle.getFrequency(),
            activeDates,
            summary: handle.summary(rate, option(request.confidence)),
            tickers: tickerNames.map((_, index) => ({
              activeDates: handle.getActiveDatesForTicker(index),
              cumulativeReturns: Array.from(cumulative[index]!),
              drawdowns: Array.from(drawdowns[index]!),
              drawdownEpisodes: handle.drawdownDetails(
                index,
                option(request.episodes),
              ),
              rollingSharpe: series(handle.rollingSharpe(index, window, rate)),
              rollingVolatility: series(
                handle.rollingVolatility(index, window),
              ),
            })),
            monthly: handle.periodicReturns("monthly"),
            annual: handle.periodicReturns("annual"),
            lookback: {
              refDate,
              returns: handle.lookbackReturns(refDate, fiscal),
              fiscalYearStart: request.fiscalYearStart
                ? { ...request.fiscalYearStart }
                : null,
            },
            correlation: {
              matrix: correlation.matrix.map((row) => Array.from(row)),
              repaired: correlation.repaired,
            },
          };
        } finally {
          performance?.free();
          fiscal?.free();
        }
      });
    },
    benchmarkReport(request: BenchmarkReportRequest) {
      return context.result((): BenchmarkReport => {
        const performance = build(request);
        try {
          const rate = option(request.riskFreeRate);
          const window = option(request.window);
          const factors = request.factors;
          const factorReturns = factors?.returns.map((row) => [...row]);
          const outperformance = performance.cumulativeReturnsOutperformance();
          const drawdownDifference = performance.drawdownDifference();
          const tickerNames = performance.getTickerNames();
          return {
            tickerNames,
            benchmarkIdx: performance.getBenchmarkIdx(),
            frequency: performance.getFrequency(),
            activeDates: performance.getActiveDates(),
            factorNames: factors ? [...factors.names] : null,
            greeks: performance.greeks(rate),
            beta: performance.beta(),
            trackingError: Array.from(performance.trackingError()),
            informationRatio: Array.from(performance.informationRatio()),
            treynor: Array.from(performance.treynor(rate)),
            mSquared: Array.from(performance.mSquared(rate)),
            upCapture: Array.from(performance.upCapture()),
            downCapture: Array.from(performance.downCapture()),
            captureRatio: Array.from(performance.captureRatio()),
            battingAverage: Array.from(performance.battingAverage()),
            tickers: tickerNames.map((_, index) => {
              const rolling = performance.rollingGreeks(index, window, rate);
              return {
                activeDates: performance.getActiveDatesForTicker(index),
                outperformance: Array.from(outperformance[index]!),
                drawdownDifference: Array.from(drawdownDifference[index]!),
                rollingGreeks: {
                  ...rolling,
                  alphas: Array.from(rolling.alphas),
                  betas: Array.from(rolling.betas),
                },
                // One ticker's rejected regression does not hide the others.
                multiFactor: factorReturns
                  ? attempt(() =>
                      performance.multiFactorGreeks(
                        index,
                        factorReturns,
                        option(factors!.returnKind),
                        option(factors!.riskFreeRate),
                      ),
                    )
                  : null,
              };
            }),
          };
        } finally {
          performance.free();
        }
      });
    },
  };
}
