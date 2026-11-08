import type {
  analytics,
  core,
  DatedSeries,
  FiscalConfig,
} from "finstack-quant-wasm";
import type {
  AnalyticsApi,
  BenchmarkReport,
  BenchmarkReportRequest,
  CorrelationMatrixReport,
  PerformanceReport,
  PerformanceSeries,
} from "../finstack-contract";
import { attempt, type ServiceContext } from "./context";
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
/** Optional calendar/lookback inputs retain their explicit absence. */
const option = <T>(value: T | null | undefined) => value ?? undefined;
/** AnalyticsApi over the analytics facade namespace; handles never outlive a call. */
export function createAnalyticsService(
  native: AnalyticsNative,
  context: ServiceContext,
): AnalyticsApi {
  type Engine = InstanceType<AnalyticsNative["analytics"]["Performance"]>;
  return {
    performanceReport(request) {
      return context.result((): PerformanceReport => {
        let performance: Engine | undefined;
        let fiscal: FiscalConfig | undefined;
        try {
          performance = new native.analytics.Performance(request.panel);
          const handle = performance;
          if (request.fiscalYearStart)
            fiscal = new native.core.FiscalConfig(
              request.fiscalYearStart.month,
              request.fiscalYearStart.day,
            );
          const window = request.window;
          const rate = request.riskFreeRate;
          const activeDates = handle.getActiveDates();
          const cumulative = handle.cumulativeReturns();
          const drawdowns = handle.getDrawdownSeries();
          const tickerNames = handle.getTickerNames();
          const lookback = handle.lookbackReturns(
            option(request.lookbackDate),
            fiscal,
          );
          return {
            tickerNames,
            benchmarkIdx: request.panel.benchmark_ticker
              ? handle.getBenchmarkIdx()!
              : null,
            frequency: handle.getFrequency()!,
            activeDates,
            summary: handle.summary(
              rate,
              request.mar,
              request.omegaThreshold,
              request.confidence,
            ),
            // finstack-compose: one native call per ticker index, nothing read from the names
            tickers: tickerNames.map((_, index) => ({
              activeDates: handle.getActiveDatesForTicker(index),
              cumulativeReturns: Array.from(cumulative[index]!),
              drawdowns: Array.from(drawdowns[index]!),
              drawdownEpisodes: handle.drawdownDetails(index, request.episodes),
              rollingSharpe: series(handle.rollingSharpe(index, window, rate)),
              rollingVolatility: series(
                handle.rollingVolatility(index, window),
              ),
            })),
            monthly: handle.periodicReturns("monthly"),
            annual: handle.periodicReturns("annual"),
            lookback: {
              refDate: lookback.ref_date,
              returns: lookback,
              fiscalYearStart: request.fiscalYearStart
                ? { ...request.fiscalYearStart }
                : null,
            },
          };
        } finally {
          performance?.free();
          fiscal?.free();
        }
      });
    },
    correlationMatrix(request) {
      return context.result((): CorrelationMatrixReport => {
        const performance = new native.analytics.Performance(request.panel);
        try {
          const correlation = performance.correlationMatrix();
          return {
            tickerNames: correlation.tickers,
            matrix: correlation.matrix,
            nObs: correlation.n_obs,
            startDate: correlation.start_date,
            endDate: correlation.end_date,
          };
        } finally {
          performance.free();
        }
      });
    },
    benchmarkReport(request: BenchmarkReportRequest) {
      return context.result((): BenchmarkReport => {
        const performance = new native.analytics.Performance(request.panel);
        try {
          const rate = request.riskFreeRate;
          const window = request.window;
          const factors = request.factors;
          const factorReturns = factors?.returns.map((row) => [...row]);
          const outperformance = performance.cumulativeReturnsOutperformance();
          const drawdownDifference = performance.drawdownDifference();
          const tickerNames = performance.getTickerNames();
          return {
            tickerNames,
            benchmarkIdx: performance.getBenchmarkIdx()!,
            frequency: performance.getFrequency()!,
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
            // finstack-compose: one native call per ticker index, nothing read from the names
            tickers: tickerNames.map((_, index) => {
              const rolling = performance.rollingGreeks(index, window, rate);
              return {
                activeDates: performance.getPairedDates(index),
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
                        factors!.riskFreeRate,
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
