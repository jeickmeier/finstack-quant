import type {
  analytics,
  core,
  DatedSeries,
  FiscalConfig,
} from "finstack-quant-wasm";
import type {
  AnalyticsApi,
  PerformanceReport,
  PerformanceSeries,
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
/** AnalyticsApi over the analytics facade namespace; handles never outlive a call. */
export function createAnalyticsService(
  native: AnalyticsNative,
  context: ServiceContext,
): AnalyticsApi {
  return {
    performanceReport(request) {
      return context.result((): PerformanceReport => {
        const { Performance: Engine } = native.analytics;
        // Omitted options stay unset so Rust applies its own defaults.
        const option = <T>(value: T | null | undefined) => value ?? undefined;
        const args: Parameters<typeof Engine.fromReturns> = [
          [...request.dates],
          request.values.map((row) => [...row]),
          [...request.tickerNames],
          option(request.benchmarkTicker),
          option(request.frequency),
        ];
        let performance: InstanceType<typeof Engine> | undefined;
        let fiscal: FiscalConfig | undefined;
        try {
          performance =
            request.kind === "returns"
              ? Engine.fromReturns(...args)
              : new Engine(...args);
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
  };
}
