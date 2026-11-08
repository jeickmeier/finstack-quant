import { checking, emit } from "../../scripts/fixture-output.mjs";
import { native, readRepoJson } from "../../scripts/fixture-inputs.mjs";
// The reporting_performance_tearsheet.ipynb panel: 756 business days from
// 2023-01-02, fund 0.0008 + 0.012·sin(i/30), plus a slower benchmark wave.
// Rounded to twelve decimal places: Node patch releases disagree in the last
// bits of Math.sin, and the fixture check compares those bits.
const dates = [];
for (let day = Date.UTC(2023, 0, 2); dates.length < 756; day += 86_400_000) {
  const weekday = new Date(day).getUTCDay();
  if (weekday !== 0 && weekday !== 6)
    dates.push(new Date(day).toISOString().slice(0, 10));
}
// Stabilize generated requests across JS engines' last-bit Math.sin/cos differences.
const round = (value) => Math.round(value * 1e12) / 1e12;
// Explicit actual valuation grid: opening wealth on 2022-12-30, followed by
// 756 interval-return endpoints. Observation frequency remains a declared
// trading convention; calendar gaps do not change it.
const panel = {
  dates: ["2022-12-30", ...dates],
  series: [
    dates.map((_, i) => round(0.0008 + 0.012 * Math.sin(i / 30))),
    dates.map((_, i) => round(0.0004 + 0.008 * Math.sin(i / 45))),
  ],
  kind: "returns",
  ticker_names: ["Global Macro Composite", "Benchmark"],
  benchmark_ticker: "Benchmark",
  frequency: "daily",
};
// Each case's request is its panel plus its options.
const requests = [
  // Explicit baseline assumptions; the lookback ends on the last active date.
  {
    name: "baseline",
    summary:
      "with explicit daily observation frequency, zero cash and hurdles, 95% confidence, 63 observations and five episodes",
    panel,
    options: {
      riskFreeRate: 0,
      mar: 0,
      omegaThreshold: 0,
      confidence: 0.95,
      window: 63,
      episodes: 5,
    },
  },
  {
    name: "explicit",
    summary:
      "with an explicit frequency, rate, confidence, window, episode count, lookback date and April fiscal year",
    panel: { ...panel, frequency: "daily" },
    options: {
      riskFreeRate: 0.02,
      mar: 0,
      omegaThreshold: 0,
      confidence: 0.99,
      window: 126,
      episodes: 3,
      lookbackDate: "2025-06-30",
      fiscalYearStart: { month: 4, day: 1 },
    },
  },
];
const series = (value) => ({ ...value, values: Array.from(value.values) });
// Mirror the worker's AnalyticsApi.performanceReport using raw WASM exports.
// Financial assumptions are explicit. Only the lookback date and fiscal
// calendar are optional; the raw package exposes its fiscal lookback as
// lookbackReturnsWithFiscalConfig, which the public facade joins into one method.
function report(request) {
  const performance = new native.Performance(request.panel);
  let fiscal;
  try {
    if (request.fiscalYearStart)
      fiscal = new native.FiscalConfig(
        request.fiscalYearStart.month,
        request.fiscalYearStart.day,
      );
    const window = request.window;
    const rate = request.riskFreeRate;
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
    return {
      tickerNames,
      benchmarkIdx: request.panel.benchmark_ticker
        ? performance.getBenchmarkIdx()
        : null,
      frequency: performance.getFrequency(),
      activeDates,
      summary: performance.summary(
        rate,
        request.mar,
        request.omegaThreshold,
        request.confidence,
      ),
      tickers: tickerNames.map((_, index) => ({
        activeDates: performance.getActiveDatesForTicker(index),
        cumulativeReturns: Array.from(cumulative[index]),
        drawdowns: Array.from(drawdowns[index]),
        drawdownEpisodes: performance.drawdownDetails(index, request.episodes),
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
    };
  } finally {
    performance.free();
    fiscal?.free();
  }
}
// Keep ordinary fixture data finite. The self-benchmark information ratio is
// the one deliberately undefined result in these samples: zero excess / zero risk.
function assertFinite(value, path, undefinedPath) {
  if (typeof value === "number") {
    if (
      !Number.isFinite(value) &&
      !(path === undefinedPath && Number.isNaN(value))
    )
      throw new Error(`${path} is ${value}`);
  } else if (value && typeof value === "object")
    for (const [key, child] of Object.entries(value))
      assertFinite(child, `${path}.${key}`, undefinedPath);
}
// JSON has no nonfinite numbers; preserve result identity with the core wire tokens.
const resultJson = (value) =>
  JSON.stringify(value, (_, number) =>
    typeof number === "number" && !Number.isFinite(number)
      ? Number.isNaN(number)
        ? "nan"
        : number > 0
          ? "inf"
          : "-inf"
      : number,
  );
// One file per case keeps each under the repository's 500 KB added-file limit;
// the gallery reads only the explicit case.
for (const { name, panel: casePanel, options, summary } of requests) {
  const request = { panel: casePanel, ...options };
  const value = report(request);
  assertFinite(request, `${name}.request`);
  const irColumn = value.summary.columns.findIndex(
    (column) => column.name === "information_ratio",
  );
  assertFinite(
    value,
    name,
    `${name}.summary.columns.${irColumn}.data.values.${value.benchmarkIdx}`,
  );
  await emit(
    new URL(`${name}.json`, import.meta.url),
    resultJson({
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
    const rate = request.riskFreeRate;
    const window = request.window;
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
          activeDates: performance.getPairedDates(index),
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
                  factors.riskFreeRate,
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
// Benchmark case 1 uses explicit cash/window and two already-excess factor waves.
const benchmarkRequest = {
  panel,
  riskFreeRate: 0,
  window: 63,
  factors: {
    names: ["Market", "Carry"],
    riskFreeRate: 0,
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
    dates: ["2024-12-31", ...invariants.dates],
    series: [invariants.returns, invariants.benchmark],
    kind: "returns",
    ticker_names: ["TARGET", "BENCH"],
    benchmark_ticker: "BENCH",
    frequency: "daily",
  },
  window: 5,
  riskFreeRate: 0,
  factors: {
    names: ["Factor 1", "Factor 2"],
    returns: invariants.factors,
    riskFreeRate: 0,
  },
};
const benchmark = benchmarkReport(benchmarkRequest);
const invariantsReport = benchmarkReport(invariantsRequest);
assertFinite(benchmarkRequest, "benchmark.request");
assertFinite(invariantsRequest, "invariants.request");
assertFinite(
  benchmark,
  "benchmark",
  `benchmark.informationRatio.${benchmark.benchmarkIdx}`,
);
assertFinite(
  invariantsReport,
  "invariants",
  `invariants.informationRatio.${invariantsReport.benchmarkIdx}`,
);
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
    "Native analytics.Performance benchmark report of the reporting_performance_tearsheet.ipynb panel (756 business days, fund 0.0008 + 0.012·sin(i/30) against benchmark 0.0004 + 0.008·sin(i/45)) with explicit daily frequency, zero cash, a 63-observation window and two illustrative already-excess factors (Market 0.0003 + 0.007·sin(i/40), Carry 0.0001 + 0.003·cos(i/25)).",
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
    resultJson({ description, request, report }),
  );
console.log(
  `${checking ? "Checked" : "Generated"} ${requests.length} native performance reports and 2 benchmark reports.`,
);
