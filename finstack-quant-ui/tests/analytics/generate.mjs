import { createRequire } from "node:module";
import { checking, emit } from "../../scripts/fixture-output.mjs";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// The reporting_performance_tearsheet.ipynb panel: 756 business days from
// 2023-01-02, fund 0.0008 + 0.012·sin(i/30), plus a slower benchmark wave.
const dates = [];
for (let day = Date.UTC(2023, 0, 2); dates.length < 756; day += 86_400_000) {
  const weekday = new Date(day).getUTCDay();
  if (weekday !== 0 && weekday !== 6)
    dates.push(new Date(day).toISOString().slice(0, 10));
}
const panel = {
  dates,
  values: [
    dates.map((_, i) => 0.0008 + 0.012 * Math.sin(i / 30)),
    dates.map((_, i) => 0.0004 + 0.008 * Math.sin(i / 45)),
  ],
  kind: "returns",
  tickerNames: ["Global Macro Composite", "Benchmark"],
  benchmarkTicker: "Benchmark",
};
// Each case's request is the panel plus its options.
const requests = [
  // Every option omitted: Rust's defaults apply (window 63, five episodes,
  // rf 0, confidence 0.95, calendar-year FYTD); the lookback ends on the last
  // active date.
  {
    name: "defaults",
    summary: "with every option left to Rust",
    options: {},
  },
  {
    name: "explicit",
    summary:
      "with an explicit rate, confidence, window, episode count, lookback date and April fiscal year",
    options: {
      frequency: "daily",
      riskFreeRate: 0.02,
      confidence: 0.99,
      window: 126,
      episodes: 3,
      lookbackDate: "2025-06-30",
      fiscalYearStart: { month: 4, day: 1 },
    },
  },
];
const option = (value) => value ?? undefined;
const series = (value) => ({ ...value, values: Array.from(value.values) });
// The worker's AnalyticsApi.performanceReport over the raw package. The
// facade joins lookbackReturns(refDate, fiscal) onto this raw
// lookbackReturnsWithFiscalConfig export; the raw lookbackReturns takes no
// fiscal argument.
function report(request) {
  const performance =
    request.kind === "returns"
      ? native.Performance.fromReturns(
          request.dates,
          request.values,
          request.tickerNames,
          option(request.benchmarkTicker),
          option(request.frequency),
        )
      : new native.Performance(
          request.dates,
          request.values,
          request.tickerNames,
          option(request.benchmarkTicker),
          option(request.frequency),
        );
  let fiscal;
  try {
    if (request.fiscalYearStart)
      fiscal = new native.FiscalConfig(
        request.fiscalYearStart.month,
        request.fiscalYearStart.day,
      );
    const window = option(request.window);
    const rate = option(request.riskFreeRate);
    const activeDates = performance.getActiveDates();
    const cumulative = performance.cumulativeReturns();
    const drawdowns = performance.getDrawdownSeries();
    const tickerNames = performance.getTickerNames();
    const refDate = request.lookbackDate ?? activeDates.at(-1);
    const correlation = performance.correlationMatrix();
    return {
      tickerNames,
      benchmarkIdx: request.benchmarkTicker
        ? performance.getBenchmarkIdx()
        : null,
      frequency: performance.getFrequency(),
      activeDates,
      summary: performance.summary(rate, option(request.confidence)),
      tickers: tickerNames.map((_, index) => ({
        activeDates: performance.getActiveDatesForTicker(index),
        cumulativeReturns: Array.from(cumulative[index]),
        drawdowns: Array.from(drawdowns[index]),
        drawdownEpisodes: performance.drawdownDetails(
          index,
          option(request.episodes),
        ),
        rollingSharpe: series(performance.rollingSharpe(index, window, rate)),
        rollingVolatility: series(performance.rollingVolatility(index, window)),
      })),
      monthly: performance.periodicReturns("monthly"),
      annual: performance.periodicReturns("annual"),
      lookback: {
        refDate,
        returns: fiscal
          ? performance.lookbackReturnsWithFiscalConfig(refDate, fiscal)
          : performance.lookbackReturns(refDate),
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
    performance.free();
    fiscal?.free();
  }
}
// fixture-output writes null for NaN and ±Infinity, so the pinned panel must be finite.
function assertFinite(value, path) {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error(`${path} is ${value}`);
  } else if (value && typeof value === "object")
    for (const [key, child] of Object.entries(value))
      assertFinite(child, `${path}.${key}`);
}
// One file per case keeps each under the repository's 500 KB added-file limit;
// the gallery reads only the explicit case.
for (const { name, options, summary } of requests) {
  const request = { ...panel, ...options };
  const value = report(request);
  assertFinite(value, name);
  await emit(
    new URL(`${name}.json`, import.meta.url),
    JSON.stringify({
      description: `Native analytics.Performance report of the reporting_performance_tearsheet.ipynb panel (756 business days, fund 0.0008 + 0.012·sin(i/30), benchmark 0.0004 + 0.008·sin(i/45)) ${summary}.`,
      name,
      request,
      report: value,
    }),
  );
}
console.log(
  `${checking ? "Checked" : "Generated"} ${requests.length} native performance reports.`,
);
