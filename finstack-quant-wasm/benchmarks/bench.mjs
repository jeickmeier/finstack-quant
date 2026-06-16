#!/usr/bin/env node
/* eslint-disable no-console -- CLI benchmark output */
/**
 * finstack-quant-wasm (Node.js / wasm-pack target) micro-benchmarks.
 *
 * Requires: npm run build:node
 */

import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_NODE_DIR = join(__dirname, '..', 'pkg-node');
const WASM_JS = join(PKG_NODE_DIR, 'finstack_quant_wasm.js');
const WASM_BIN = join(PKG_NODE_DIR, 'finstack_quant_wasm_bg.wasm');

function printHelp() {
  console.log(`finstack-quant-wasm benchmarks (Node.js)

Usage:
  node benchmarks/bench.mjs [options]

Options:
  --help, -h    Show this message

Prerequisites:
  The wasm-pack Node build must exist under pkg-node/. If missing, run:
    npm run build:node

Timing uses performance.now(); each row is one measured call per iteration
(unless noted). Monte Carlo uses 50,000 paths per call.
`);
}

if (process.argv.includes('--help') || process.argv.includes('-h')) {
  printHelp();
  process.exit(0);
}

if (!existsSync(WASM_JS) || !existsSync(WASM_BIN)) {
  console.error(
    'finstack-quant-wasm Node build not found.\n' +
      `Expected:\n  ${WASM_JS}\n  ${WASM_BIN}\n\n` +
      'Generate them with:\n  npm run build:node\n'
  );
  process.exit(1);
}

// --- Minimal valid JSON fixtures (inline) -----------------------------------

const FINANCIAL_MODEL_JSON = JSON.stringify({
  id: 'bench-model',
  periods: [
    {
      id: '2025Q1',
      start: '2025-01-01',
      end: '2025-04-01',
      is_actual: true,
    },
  ],
  nodes: {
    revenue: {
      node_id: 'revenue',
      node_type: 'value',
      values: {
        '2025Q1': 100000.0,
      },
    },
  },
  schema_version: 1,
});

const SENSITIVITY_CONFIG_JSON = JSON.stringify({
  mode: 'Diagonal',
  parameters: [
    {
      node_id: 'revenue',
      period_id: '2025Q1',
      base_value: 100000.0,
      perturbations: [90000.0, 100000.0, 110000.0],
    },
  ],
  target_metrics: ['revenue'],
});

const PORTFOLIO_SPEC_JSON = JSON.stringify({
  id: 'bench-portfolio',
  base_ccy: 'USD',
  as_of: '2024-01-01',
  entities: {},
  positions: [],
});

const PORTFOLIO_RESULT_JSON = JSON.stringify({
  valuation: {
    as_of: '2024-01-01',
    position_values: {},
    total_base_ccy: { amount: 1000000.0, currency: 'USD' },
    by_entity: {},
  },
  metrics: {
    aggregated: {},
    by_position: {},
  },
  meta: {
    numeric_mode: 'F64',
    rounding: {
      mode: 'Bankers',
      ingest_scale_by_ccy: {},
      output_scale_by_ccy: {},
      tolerances: {
        rate_epsilon: 1e-12,
        generic_epsilon: 1e-10,
      },
      version: 1,
    },
    fx_policy_applied: null,
    timestamp: null,
    version: null,
  },
});

const INSTRUMENT_JSON = JSON.stringify({
  type: 'deposit',
  spec: {
    id: 'DEP-BENCH',
    notional: { amount: 100000.0, currency: 'USD' },
    start_date: '2024-01-01',
    maturity: '2024-07-01',
    day_count: 'Act360',
    quote_rate: 0.045,
    discount_curve_id: 'USD-OIS',
    attributes: {},
    bdc: 'modified_following',
  },
});

const SCENARIO_SPEC_JSON = JSON.stringify({
  id: 'bench-scenario',
  operations: [],
});

/** Minimal `MarketContext` JSON for discounting (schema v2). */
const MARKET_CONTEXT_JSON = JSON.stringify({
  version: 2,
  curves: [
    {
      type: 'discount',
      id: 'USD-OIS',
      base: '2024-01-01',
      day_count: 'Act360',
      knot_points: [
        [0.0, 1.0],
        [1.0, 0.98],
        [5.0, 0.88],
      ],
      interp_style: 'monotone_convex',
      extrapolation: 'flat_forward',
    },
  ],
  fx: null,
  surfaces: [],
  prices: {},
  series: [],
  inflation_indices: [],
  dividends: [],
  credit_indices: [],
  fx_delta_vol_surfaces: [],
  collateral: {},
});

const STATEMENT_RESULT_BASE_JSON = JSON.stringify({
  nodes: { revenue: { '2025Q1': 100000.0 } },
  meta: {
    num_nodes: 1,
    num_periods: 1,
    numeric_mode: 'float64',
    parallel: false,
    warnings: [],
  },
});

const STATEMENT_RESULT_CMP_JSON = JSON.stringify({
  nodes: { revenue: { '2025Q1': 95000.0 } },
  meta: {
    num_nodes: 1,
    num_periods: 1,
    numeric_mode: 'float64',
    parallel: false,
    warnings: [],
  },
});

const VARIANCE_CONFIG_JSON = JSON.stringify({
  baseline_label: 'base',
  comparison_label: 'cmp',
  metrics: ['revenue'],
  periods: ['2025Q1'],
});

const SCENARIO_SET_JSON = JSON.stringify({
  scenarios: {
    base: { overrides: {} },
    downside: { parent: 'base', overrides: { revenue: 90000.0 } },
  },
});

const GOAL_SEEK_MODEL_JSON = JSON.stringify({
  id: 'bench-goal-seek',
  periods: [
    {
      id: '2025Q1',
      start: '2025-01-01',
      end: '2025-04-01',
      is_actual: true,
    },
  ],
  nodes: {
    revenue: {
      node_id: 'revenue',
      node_type: 'value',
      values: { '2025Q1': 100000.0 },
    },
    cogs: {
      node_id: 'cogs',
      node_type: 'calculated',
      formula_text: 'revenue * 0.4',
    },
    ebitda: {
      node_id: 'ebitda',
      node_type: 'calculated',
      formula_text: 'revenue - cogs',
    },
  },
  schema_version: 1,
});

const MONTE_CARLO_MODEL_JSON = JSON.stringify({
  id: 'bench-mc',
  periods: [
    {
      id: '2025Q1',
      start: '2025-01-01',
      end: '2025-04-01',
      is_actual: true,
    },
    {
      id: '2025Q2',
      start: '2025-04-01',
      end: '2025-07-01',
      is_actual: false,
    },
  ],
  nodes: {
    revenue: {
      node_id: 'revenue',
      node_type: 'mixed',
      values: { '2025Q1': 100000.0 },
      forecast: {
        method: 'normal',
        params: { mean: 100000.0, std_dev: 5000.0, seed: 42 },
      },
    },
  },
  schema_version: 1,
});

const MONTE_CARLO_CONFIG_JSON = JSON.stringify({
  n_paths: 80,
  seed: 7,
  percentiles: [0.05, 0.5, 0.95],
});

const COMPOSE_SCENARIOS_JSON = JSON.stringify([
  { id: 'sc-a', operations: [], priority: 0 },
  { id: 'sc-b', operations: [], priority: 1 },
]);

const returns = Array.from({ length: 252 }, (_, i) => ((i % 17) - 8) * 0.001);
const prices = Array.from({ length: 128 }, (_, i) => 100 * (1 + 0.001 * Math.sin(i)));
const priceDates = prices.map((_, i) =>
  new Date(Date.UTC(2025, 0, 1 + i)).toISOString().slice(0, 10)
);
const returnDates = returns.map((_, i) =>
  new Date(Date.UTC(2025, 0, 1 + i)).toISOString().slice(0, 10)
);
const statsArr = Array.from({ length: 512 }, (_, i) => i * 0.01 + Math.sin(i));
const cholMat = [
  [4, 2, 0],
  [2, 5, 1],
  [0, 1, 3],
];

async function main() {
  const wasmHref = pathToFileURL(WASM_JS).href;
  const wasm = await import(wasmHref);
  if (typeof wasm.default === 'function') {
    await wasm.default();
  }

  const w = wasm;

  /**
   * @param {number} iterations
   * @param {() => void} fn
   */
  function measureRuns(iterations, fn) {
    const samples = new Array(iterations);
    for (let i = 0; i < iterations; i++) {
      const t0 = performance.now();
      fn();
      samples[i] = performance.now() - t0;
    }
    const total = samples.reduce((a, b) => a + b, 0);
    const avg = total / iterations;
    const best = Math.min(...samples);
    const opsPerSec = avg > 0 ? 1000 / avg : 0;
    return { best, avg, opsPerSec, iterations };
  }

  /** @type {{ domain: string; name: string; best: number; avg: number; opsPerSec: number; iterations: number }[]} */
  const rows = [];

  /**
   * @param {string} domain
   * @param {string} name
   * @param {number} iterations
   * @param {() => void} fn
   */
  function bench(domain, name, iterations, fn) {
    const r = measureRuns(iterations, fn);
    rows.push({ domain, name, ...r });
  }

  /**
   * @param {string} domain
   * @param {string} name
   * @param {string} reason
   */
  function skipBench(domain, name, reason) {
    console.warn(`[bench skip] ${domain} / ${name}: ${reason}`);
    rows.push({
      domain,
      name: `${name} (skipped)`,
      best: 0,
      avg: 0,
      opsPerSec: 0,
      iterations: 0,
    });
  }

  /**
   * @param {string} domain
   * @param {string} name
   * @param {number} iterations
   * @param {() => void} fn
   */
  function benchTry(domain, name, iterations, fn) {
    try {
      fn();
    } catch (e) {
      skipBench(domain, name, e instanceof Error ? e.message : String(e));
      return;
    }
    bench(domain, name, iterations, fn);
  }

  const usd = new w.Currency('USD');
  const moneyA = new w.Money(100.0, usd);
  const moneyB = new w.Money(25.5, usd);
  const dc = w.DayCount.act360();
  const t0d = w.createDate(2024, 1, 2);
  const t1d = w.createDate(2025, 1, 2);
  const curve = new w.DiscountCurve('USD-OIS', '2024-01-02', [0.0, 1.0, 1.0, 0.98, 5.0, 0.88]);
  const pricePerf = new w.Performance(priceDates, [prices], ['bench'], null, 'daily');
  const returnPerf = w.Performance.fromReturns(
    returnDates,
    [returns, returns.map((x) => x * 0.95)],
    ['bench', 'benchmark'],
    'benchmark',
    'daily'
  );

  for (let i = 0; i < 20; i++) {
    w.mean(statsArr);
    new w.Currency('EUR');
  }

  bench('core', 'Currency constructor', 5000, () => {
    new w.Currency('USD');
  });

  bench('core', 'Money add / sub', 8000, () => {
    moneyA.add(moneyB);
    moneyA.sub(moneyB);
  });

  bench('core', 'DayCount.yearFraction', 10000, () => {
    dc.yearFraction(t0d, t1d);
  });

  bench('core', 'DiscountCurve.df', 15000, () => {
    curve.df(1.25);
  });

  bench('core', 'choleskyDecomposition', 3000, () => {
    w.choleskyDecomposition(cholMat);
  });

  bench('core', 'mean + variance (512)', 5000, () => {
    w.mean(statsArr);
    w.variance(statsArr);
  });

  bench('analytics', 'sharpe', 20000, () => {
    pricePerf.sharpe(0.02);
  });

  bench('analytics', 'volatility', 8000, () => {
    pricePerf.volatility(true);
  });

  bench('analytics', 'cumulativeReturns', 6000, () => {
    pricePerf.cumulativeReturns();
  });

  bench('analytics', 'drawdownSeries', 6000, () => {
    returnPerf.drawdownSeries();
  });

  bench('analytics', 'maxDrawdown', 8000, () => {
    returnPerf.maxDrawdown();
  });

  bench('correlation', 'correlationBounds', 20000, () => {
    w.correlationBounds(0.05, 0.08);
  });

  bench('correlation', 'jointProbabilities', 20000, () => {
    w.jointProbabilities(0.05, 0.08, 0.2);
  });

  bench('monte_carlo', 'priceEuropeanCall (50k paths)', 5, () => {
    w.priceEuropeanCall(100, 100, 0.05, 0.0, 0.2, 1.0, 50000, 42n, 64, 'USD');
  });

  bench('monte_carlo', 'blackScholesCall', 20000, () => {
    w.blackScholesCall(100, 100, 0.05, 0.0, 0.2, 1.0);
  });

  const csaCanonical = w.csaUsdRegulatory();

  bench('margin', 'csaUsdRegulatory', 2000, () => {
    w.csaUsdRegulatory();
  });

  bench('margin', 'validateCsaJson', 2000, () => {
    w.validateCsaJson(csaCanonical);
  });

  bench('statements', 'validateFinancialModelJson', 1500, () => {
    w.validateFinancialModelJson(FINANCIAL_MODEL_JSON);
  });

  bench('statements', 'modelNodeIds', 3000, () => {
    w.modelNodeIds(FINANCIAL_MODEL_JSON);
  });

  bench('statements_analytics', 'runSensitivity', 200, () => {
    w.runSensitivity(FINANCIAL_MODEL_JSON, SENSITIVITY_CONFIG_JSON);
  });

  const actualSeries = returns.slice(0, 120);
  const forecastSeries = actualSeries.map((x) => x * 1.01);
  bench('statements_analytics', 'backtestForecast', 5000, () => {
    w.backtestForecast(actualSeries, forecastSeries);
  });

  bench('portfolio', 'parsePortfolioSpec', 4000, () => {
    w.parsePortfolioSpec(PORTFOLIO_SPEC_JSON);
  });

  bench('portfolio', 'portfolioResultTotalValue', 8000, () => {
    w.portfolioResultTotalValue(PORTFOLIO_RESULT_JSON);
  });

  bench('valuations', 'validateInstrumentJson', 2000, () => {
    w.validateInstrumentJson(INSTRUMENT_JSON);
  });

  bench('valuations', 'listStandardMetrics', 3000, () => {
    w.listStandardMetrics();
  });

  const templateIds = w.listBuiltinTemplates();

  bench('scenarios', 'listBuiltinTemplates', 500, () => {
    w.listBuiltinTemplates();
  });

  const firstTemplate =
    Array.isArray(templateIds) && templateIds.length > 0 ? String(templateIds[0]) : null;

  if (firstTemplate) {
    bench('scenarios', `buildFromTemplate(${firstTemplate})`, 200, () => {
      w.buildFromTemplate(firstTemplate);
    });
  } else {
    rows.push({
      domain: 'scenarios',
      name: 'buildFromTemplate (skipped — no templates)',
      best: 0,
      avg: 0,
      opsPerSec: 0,
      iterations: 0,
    });
  }

  bench('scenarios', 'validateScenarioSpec', 5000, () => {
    w.validateScenarioSpec(SCENARIO_SPEC_JSON);
  });

  // --- Extended benchmarks (additional coverage) --------------------------------

  bench('core', 'Rate / Bps / Percentage conversions', 12000, () => {
    const r = new w.Rate(0.05);
    const _sum = r.asDecimal + r.asPercent + r.asBps;
    void _sum;
    new w.Bps(50).asDecimal();
    new w.Percentage(5).asDecimal();
  });

  bench('core', 'Tenor construction + toYearsSimple', 8000, () => {
    const t = new w.Tenor('3M');
    t.toYearsSimple();
  });

  bench('core', 'ForwardCurve construction + rate()', 4000, () => {
    const fc = new w.ForwardCurve('USD-FWD', 0.25, '2024-01-02', [0.0, 0.03, 5.0, 0.035]);
    fc.rate(1.25);
  });

  bench('core', 'FxMatrix setQuote + rate', 4000, () => {
    const fx = new w.FxMatrix();
    fx.setQuote('USD', 'EUR', 0.92);
    fx.rate('USD', 'EUR', '2024-01-02');
  });

  const cholFactor = w.choleskyDecomposition(cholMat);
  const cholRhs = [1.0, 2.0, 3.0];
  bench('core', 'choleskySolve', 5000, () => {
    w.choleskySolve(cholFactor, cholRhs);
  });

  bench('core', 'normCdf', 20000, () => {
    w.normCdf(0.42);
  });

  bench('core', 'quantile (512)', 6000, () => {
    w.quantile(statsArr, 0.25);
  });

  bench('core', 'kahanSum (512)', 6000, () => {
    w.kahanSum(statsArr);
  });

  let calCode = 'target2';
  try {
    const cals = w.availableCalendars();
    if (Array.isArray(cals) && cals.length > 0) {
      calCode = String(cals[0]);
    }
  } catch {
    /* use default */
  }

  bench('core', 'availableCalendars', 800, () => {
    w.availableCalendars();
  });

  bench('core', 'adjust', 4000, () => {
    w.adjust(t0d, 'modified_following', calCode);
  });

  const benchRf = returns.map(() => 0.0001);

  bench('analytics', 'sortino', 6000, () => {
    returnPerf.sortino(0);
  });

  bench('analytics', 'meanReturn', 6000, () => {
    returnPerf.meanReturn(true);
  });

  bench('analytics', 'downsideDeviation', 6000, () => {
    returnPerf.downsideDeviation(0.0);
  });

  bench('analytics', 'valueAtRisk', 6000, () => {
    returnPerf.valueAtRisk(0.95);
  });

  bench('analytics', 'expectedShortfall', 6000, () => {
    returnPerf.expectedShortfall(0.95);
  });

  bench('analytics', 'parametricVar', 6000, () => {
    returnPerf.parametricVar(0.95);
  });

  bench('analytics', 'skewness', 8000, () => {
    returnPerf.skewness();
  });

  bench('analytics', 'kurtosis', 8000, () => {
    returnPerf.kurtosis();
  });

  bench('analytics', 'geometricMean', 5000, () => {
    returnPerf.geometricMean();
  });

  bench('analytics', 'cumulativeReturns', 8000, () => {
    returnPerf.cumulativeReturns();
  });

  bench('analytics', 'excessReturns', 5000, () => {
    returnPerf.excessReturns(benchRf, 252);
  });

  bench('analytics', 'drawdownSeries', 4000, () => {
    returnPerf.drawdownSeries();
  });

  bench('analytics', 'rollingSharpe', 2000, () => {
    returnPerf.rollingSharpe(0, 60, 0.02);
  });

  bench('analytics', 'rollingVolatility', 2000, () => {
    returnPerf.rollingVolatility(0, 60);
  });

  bench('analytics', 'trackingError', 4000, () => {
    returnPerf.trackingError();
  });

  bench('analytics', 'informationRatio', 4000, () => {
    returnPerf.informationRatio();
  });

  bench('analytics', 'rSquared', 5000, () => {
    returnPerf.rSquared();
  });

  bench('analytics', 'drawdownDetails', 4000, () => {
    returnPerf.drawdownDetails(0, 3);
  });

  bench('analytics', 'calmar', 20000, () => {
    returnPerf.calmar();
  });

  bench('core', 'countConsecutive', 8000, () => {
    w.countConsecutive(statsArr);
  });

  const gaussCop = w.CopulaSpec.gaussian().build();
  bench('correlation', 'Copula gaussian + conditionalDefaultProb', 15000, () => {
    gaussCop.conditionalDefaultProb(-0.5, [0.1], 0.25);
  });

  const recoveryBuilt = w.RecoverySpec.constant(0.4).build();
  bench('correlation', 'RecoverySpec.constant + conditionalRecovery', 15000, () => {
    recoveryBuilt.conditionalRecovery(-0.15);
  });

  bench('monte_carlo', 'priceEuropeanPut (50k paths)', 5, () => {
    w.priceEuropeanPut(100, 100, 0.05, 0.0, 0.2, 1.0, 50000, 42n, 64, 'USD');
  });

  bench('monte_carlo', 'blackScholesPut', 20000, () => {
    w.blackScholesPut(100, 100, 0.05, 0.0, 0.2, 1.0);
  });

  benchTry('monte_carlo', 'priceAsianCall (50k paths)', 4, () => {
    w.priceAsianCall(100, 100, 0.05, 0.0, 0.2, 1.0, 50000, 42n, 64, 'USD');
  });

  benchTry('monte_carlo', 'priceAmericanPut (50k paths)', 3, () => {
    w.priceAmericanPut(100, 100, 0.05, 0.0, 0.2, 1.0, 50000, 42n, 50, 'USD');
  });

  bench('margin', 'csaEurRegulatory', 2000, () => {
    w.csaEurRegulatory();
  });

  benchTry('margin', 'calculateVm', 3000, () => {
    w.calculateVm(csaCanonical, 1000000.0, 800000.0, 'USD', 2024, 6, 15);
  });

  benchTry('statements_analytics', 'runVariance', 800, () => {
    w.runVariance(STATEMENT_RESULT_BASE_JSON, STATEMENT_RESULT_CMP_JSON, VARIANCE_CONFIG_JSON);
  });

  benchTry('statements_analytics', 'evaluateScenarioSet', 200, () => {
    w.evaluateScenarioSet(FINANCIAL_MODEL_JSON, SCENARIO_SET_JSON);
  });

  let sensitivityResultJson = '';
  try {
    sensitivityResultJson = w.runSensitivity(FINANCIAL_MODEL_JSON, SENSITIVITY_CONFIG_JSON);
  } catch (e) {
    console.warn(
      `[bench skip] statements_analytics / generateTornadoEntries (no sensitivity result): ${
        e instanceof Error ? e.message : e
      }`
    );
  }
  if (sensitivityResultJson) {
    benchTry('statements_analytics', 'generateTornadoEntries', 1500, () => {
      w.generateTornadoEntries(sensitivityResultJson, 'revenue', '2025Q1');
    });
  } else {
    skipBench('statements_analytics', 'generateTornadoEntries', 'missing sensitivity output');
  }

  benchTry('statements_analytics', 'runMonteCarlo', 30, () => {
    w.runMonteCarlo(MONTE_CARLO_MODEL_JSON, MONTE_CARLO_CONFIG_JSON);
  });

  benchTry('statements_analytics', 'goalSeek', 400, () => {
    w.goalSeek(
      GOAL_SEEK_MODEL_JSON,
      'ebitda',
      '2025Q1',
      50000.0,
      'revenue',
      '2025Q1',
      false,
      undefined,
      undefined
    );
  });

  benchTry('statements_analytics', 'traceDependencies', 2000, () => {
    w.traceDependencies(FINANCIAL_MODEL_JSON, 'revenue');
  });

  benchTry('statements_analytics', 'explainFormula', 1500, () => {
    w.explainFormula(FINANCIAL_MODEL_JSON, STATEMENT_RESULT_BASE_JSON, 'revenue', '2025Q1');
  });

  benchTry('portfolio', 'buildPortfolioFromSpec', 3000, () => {
    w.buildPortfolioFromSpec(PORTFOLIO_SPEC_JSON);
  });

  benchTry('portfolio', 'portfolioResultGetMetric', 6000, () => {
    w.portfolioResultGetMetric(PORTFOLIO_RESULT_JSON, 'dv01');
  });

  benchTry('portfolio', 'valuePortfolio', 200, () => {
    w.valuePortfolio(PORTFOLIO_SPEC_JSON, MARKET_CONTEXT_JSON, false);
  });

  benchTry('portfolio', 'aggregateFullCashflows', 200, () => {
    w.aggregateFullCashflows(PORTFOLIO_SPEC_JSON, MARKET_CONTEXT_JSON);
  });

  let sampleValuationJson = '';
  let firstStandardMetric = 'accrued';
  try {
    const metricList = w.listStandardMetrics();
    if (Array.isArray(metricList) && metricList.length > 0) {
      firstStandardMetric = String(metricList[0]);
    }
  } catch {
    /* keep default */
  }

  try {
    sampleValuationJson = w.priceInstrument(
      INSTRUMENT_JSON,
      MARKET_CONTEXT_JSON,
      '2024-01-02',
      'discounting'
    );
  } catch (e) {
    console.warn(`[bench skip] valuations / price paths: ${e instanceof Error ? e.message : e}`);
  }

  if (sampleValuationJson) {
    bench('valuations', 'priceInstrument (discounting)', 150, () => {
      w.priceInstrument(INSTRUMENT_JSON, MARKET_CONTEXT_JSON, '2024-01-02', 'discounting');
    });

    benchTry('valuations', 'priceInstrumentWithMetrics', 120, () => {
      w.priceInstrumentWithMetrics(
        INSTRUMENT_JSON,
        MARKET_CONTEXT_JSON,
        '2024-01-02',
        'discounting',
        [firstStandardMetric]
      );
    });

    bench('valuations', 'validateValuationResultJson', 2000, () => {
      w.validateValuationResultJson(sampleValuationJson);
    });
  } else {
    skipBench('valuations', 'priceInstrument (discounting)', 'pricing fixture failed');
    skipBench('valuations', 'priceInstrumentWithMetrics', 'pricing fixture failed');
    skipBench('valuations', 'validateValuationResultJson', 'pricing fixture failed');
  }

  bench('scenarios', 'parseScenarioSpec', 5000, () => {
    w.parseScenarioSpec(SCENARIO_SPEC_JSON);
  });

  bench('scenarios', 'composeScenarios', 3000, () => {
    w.composeScenarios(COMPOSE_SCENARIOS_JSON);
  });

  bench('scenarios', 'buildScenarioSpec', 4000, () => {
    w.buildScenarioSpec('composed-inline', '[]', undefined, undefined, 0);
  });

  let firstTemplateComponentId = null;
  if (firstTemplate) {
    try {
      const compIds = w.listTemplateComponents(firstTemplate);
      if (Array.isArray(compIds) && compIds.length > 0) {
        firstTemplateComponentId = String(compIds[0]);
      }
    } catch {
      /* ignore */
    }
  }

  if (firstTemplate) {
    benchTry('scenarios', `listTemplateComponents(${firstTemplate})`, 300, () => {
      w.listTemplateComponents(firstTemplate);
    });
  } else {
    skipBench('scenarios', 'listTemplateComponents', 'no builtin template');
  }

  if (firstTemplate && firstTemplateComponentId) {
    benchTry('scenarios', `buildTemplateComponent(${firstTemplate})`, 200, () => {
      w.buildTemplateComponent(firstTemplate, firstTemplateComponentId);
    });
  } else {
    skipBench('scenarios', 'buildTemplateComponent', 'no template component id');
  }

  benchTry('scenarios', 'applyScenario', 100, () => {
    w.applyScenario(SCENARIO_SPEC_JSON, MARKET_CONTEXT_JSON, FINANCIAL_MODEL_JSON, '2024-01-02');
  });

  benchTry('scenarios', 'applyScenarioToMarket', 150, () => {
    w.applyScenarioToMarket(SCENARIO_SPEC_JSON, MARKET_CONTEXT_JSON, '2024-01-02');
  });

  const dW = Math.max(12, ...rows.map((r) => r.domain.length));
  const nW = Math.max(36, ...rows.map((r) => r.name.length));

  console.log('\nfinstack-quant-wasm benchmarks (pkg-node)\n');
  console.log(
    `${'Domain'.padEnd(dW)}  ${'Benchmark'.padEnd(nW)}  ${'Iter'.padStart(5)}  ${'Best (ms)'.padStart(10)}  ${'Avg (ms)'.padStart(10)}  ${'Ops/sec'.padStart(12)}`
  );
  console.log(
    `${''.padEnd(dW, '-')}  ${''.padEnd(nW, '-')}  ${''.padStart(5, '-')}  ${''.padStart(10, '-')}  ${''.padStart(10, '-')}  ${''.padStart(12, '-')}`
  );

  for (const r of rows) {
    const bestStr = r.iterations ? r.best.toFixed(4) : '—';
    const avgStr = r.iterations ? r.avg.toFixed(4) : '—';
    const opsStr = r.iterations ? r.opsPerSec.toFixed(0) : '—';
    const iterStr = r.iterations ? String(r.iterations) : '—';
    console.log(
      `${r.domain.padEnd(dW)}  ${r.name.padEnd(nW)}  ${iterStr.padStart(5)}  ${bestStr.padStart(10)}  ${avgStr.padStart(10)}  ${opsStr.padStart(12)}`
    );
  }
  console.log('');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
