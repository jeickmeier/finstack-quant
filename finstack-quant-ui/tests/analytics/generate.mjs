import { checking, emit } from "../../scripts/fixture-output.mjs";
import { native, readRepoJson } from "../../scripts/fixture-inputs.mjs";
// The reporting_performance_tearsheet.ipynb panel: 756 business days from
// 2023-01-02, fund 0.0008 + 0.012·sin(i/30), plus a slower benchmark wave.
// Rounded to twelve decimal places to remove last-bit Math.sin differences
// between JavaScript engines without materially changing the return series.
const dates = [];
for (let day = Date.UTC(2023, 0, 2); dates.length < 756; day += 86_400_000) {
  const weekday = new Date(day).getUTCDay();
  if (weekday !== 0 && weekday !== 6)
    dates.push(new Date(day).toISOString().slice(0, 10));
}
// Stabilize generated requests across JS engines' last-bit Math.sin/cos differences.
const round = (value) => Math.round(value * 1e12) / 1e12;
// The Rust PerformancePanel wire value every report case is built from.
const panel = {
  dates,
  series: [
    dates.map((_, i) => round(0.0008 + 0.012 * Math.sin(i / 30))),
    dates.map((_, i) => round(0.0004 + 0.008 * Math.sin(i / 45))),
  ],
  kind: "returns",
  ticker_names: ["Global Macro Composite", "Benchmark"],
  benchmark_ticker: "Benchmark",
};
// Each case's request is its panel plus its options.
const requests = [
  // Every option omitted: Rust's defaults apply (window 63, five episodes,
  // rf 0, confidence 0.95, calendar-year FYTD); the lookback ends on the last
  // active date.
  {
    name: "defaults",
    summary: "with every option left to Rust",
    panel,
    options: {},
  },
  {
    name: "explicit",
    summary:
      "with an explicit frequency, rate, confidence, window, episode count, lookback date and April fiscal year",
    panel: { ...panel, frequency: "daily" },
    options: {
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
  const performance = new native.Performance(request.panel);
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
    const lookback = fiscal
      ? performance.lookbackReturnsWithFiscalConfig(
          request.lookbackDate ?? undefined,
          fiscal,
        )
      : performance.lookbackReturns(request.lookbackDate ?? undefined);
    const correlation = performance.correlationMatrix();
    return {
      tickerNames,
      benchmarkIdx: request.panel.benchmark_ticker
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
        refDate: lookback.ref_date,
        returns: lookback,
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
for (const { name, panel: casePanel, options, summary } of requests) {
  const request = { panel: casePanel, ...options };
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
// The worker's AnalyticsApi.benchmarkReport over the raw package; every
// benchmark-relative method is a plain raw export.
function benchmarkReport(request) {
  const performance = new native.Performance(request.panel);
  try {
    const rate = option(request.riskFreeRate);
    const window = option(request.window);
    const factors = request.factors;
    const outperformance = performance.cumulativeReturnsOutperformance();
    const drawdownDifference = performance.drawdownDifference();
    const tickerNames = performance.getTickerNames();
    const attempt = (call) => {
      try {
        return { ok: true, value: call() };
      } catch (error) {
        return {
          ok: false,
          error: { name: error.name, message: error.message, kind: error.kind },
        };
      }
    };
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
          outperformance: Array.from(outperformance[index]),
          drawdownDifference: Array.from(drawdownDifference[index]),
          rollingGreeks: {
            ...rolling,
            alphas: Array.from(rolling.alphas),
            betas: Array.from(rolling.betas),
          },
          multiFactor: factors
            ? attempt(() =>
                performance.multiFactorGreeks(
                  index,
                  factors.returns,
                  option(factors.returnKind),
                  option(factors.riskFreeRate),
                ),
              )
            : null,
        };
      }),
    };
  } finally {
    performance.free();
  }
}
// Benchmark case 1 (the gallery's): the tear-sheet panel with every option
// left to Rust (rolling window 63, rf 0) and two illustrative already-excess
// factor waves.
const benchmarkRequest = {
  panel,
  factors: {
    names: ["Market", "Carry"],
    returns: [
      dates.map((_, i) => round(0.0003 + 0.007 * Math.sin(i / 40))),
      dates.map((_, i) => round(0.0001 + 0.003 * Math.cos(i / 25))),
    ],
  },
};
// Benchmark case 2: the shared analytics invariants panel
// (finstack-quant/analytics/tests/data/api_invariants_data.json), whose
// expected rolling greeks were produced with a five-observation window.
const invariants = await readRepoJson(
  "finstack-quant/analytics/tests/data/api_invariants_data.json",
);
const invariantsRequest = {
  panel: {
    dates: invariants.dates,
    series: [invariants.returns, invariants.benchmark],
    kind: "returns",
    ticker_names: ["TARGET", "BENCH"],
    benchmark_ticker: "BENCH",
    frequency: "daily",
  },
  window: 5,
  factors: { names: ["Factor 1", "Factor 2"], returns: invariants.factors },
};
const benchmark = benchmarkReport(benchmarkRequest);
const invariantsReport = benchmarkReport(invariantsRequest);
assertFinite(benchmark, "benchmark");
assertFinite(invariantsReport, "invariants");
// Pin the invariants report to the Rust, Python and WASM parity expectations.
const close = (actual, expected, path) => {
  if (Math.abs(actual - expected) > 1e-12)
    throw new Error(`${path}: ${actual} differs from ${expected}`);
};
const expected = invariants.expected;
const target = invariantsReport.tickers[0];
if (!target.multiFactor.ok) throw new Error(target.multiFactor.error.message);
for (const key of ["alpha", "r_squared", "adjusted_r_squared", "residual_vol"])
  close(target.multiFactor.value[key], expected.multi_factor_greeks[key], key);
expected.multi_factor_greeks.betas.forEach((beta, i) =>
  close(target.multiFactor.value.betas[i], beta, `betas[${i}]`),
);
for (const key of ["alphas", "betas"]) {
  if (target.rollingGreeks[key].length !== expected.rolling_greeks[key].length)
    throw new Error(`rolling ${key} length`);
  expected.rolling_greeks[key].forEach((value, i) =>
    close(target.rollingGreeks[key][i], value, `rolling ${key}[${i}]`),
  );
}
for (const [file, description, request, report] of [
  [
    "benchmark.json",
    "Native analytics.Performance benchmark report of the reporting_performance_tearsheet.ipynb panel (756 business days, fund 0.0008 + 0.012·sin(i/30) against benchmark 0.0004 + 0.008·sin(i/45)) with every option left to Rust and two illustrative already-excess factors (Market 0.0003 + 0.007·sin(i/40), Carry 0.0001 + 0.003·cos(i/25)).",
    benchmarkRequest,
    benchmark,
  ],
  [
    "benchmark-invariants.json",
    "Native analytics.Performance benchmark report of the shared analytics invariants panel (finstack-quant/analytics/tests/data/api_invariants_data.json: TARGET against BENCH, daily, a five-observation rolling window and its two factors); TARGET's rolling greeks and multi-factor regression equal that file's expected values to 1e-12.",
    invariantsRequest,
    invariantsReport,
  ],
])
  await emit(
    new URL(file, import.meta.url),
    JSON.stringify({ description, request, report }),
  );
console.log(
  `${checking ? "Checked" : "Generated"} ${requests.length} native performance reports and 2 benchmark reports.`,
);
