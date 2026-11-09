#!/usr/bin/env node
/* eslint-disable no-console -- CLI benchmark output */
/**
 * finstack-quant-wasm (Node.js / wasm-pack target) micro-benchmarks.
 *
 * Requires: npm run build:node
 */

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { delimiter, dirname, isAbsolute, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
  CCM_NODE_CASES,
  CCM_NUMERICS,
  captureOwned,
  ccmSettings,
  disposeAll,
  measureCheckedBatches,
  parseBenchmarkMode,
} from './ccm-measure.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_NODE_DIR = join(__dirname, '..', 'pkg-node');
const WASM_JS = join(PKG_NODE_DIR, 'finstack_quant_wasm.js');
const WASM_BIN = join(PKG_NODE_DIR, 'finstack_quant_wasm_bg.wasm');
const REPO_ROOT = join(__dirname, '..', '..');
const MATERIALIZATION_FIXTURE_DIR = join(REPO_ROOT, 'target', 'materialization-benchmarks');
const MATERIALIZATION_FIXTURE_A = join(
  MATERIALIZATION_FIXTURE_DIR,
  'materialization-a-5000-unique.json'
);
const MATERIALIZATION_FIXTURE_B = join(
  MATERIALIZATION_FIXTURE_DIR,
  'materialization-b-5000-50.json'
);

function materializationSampleCount() {
  const raw = process.env.FQ_MATERIALIZATION_P95_SAMPLES ?? '100';
  const samples = Number(raw);
  if (!Number.isFinite(samples) || !Number.isInteger(samples)) {
    throw new Error(`FQ_MATERIALIZATION_P95_SAMPLES must be a finite integer, got ${raw}`);
  }
  const minimum = process.env.FQ_MATERIALIZATION_SMOKE === '1' ? 1 : 100;
  if (samples < minimum) {
    throw new Error(`FQ_MATERIALIZATION_P95_SAMPLES must be at least ${minimum}, got ${samples}`);
  }
  return samples;
}

const OPTIONS = parseBenchmarkMode(process.argv.slice(2));
const MATERIALIZATION_ONLY = OPTIONS.mode === '--materialization-only';
const VALUATION_ONLY = OPTIONS.mode === '--valuation-only';
const CORE_MODELS_ONLY = OPTIONS.mode === '--core-models-only';
// The scoped CCM selector does not depend on materialization configuration.
const MATERIALIZATION_SAMPLES =
  CORE_MODELS_ONLY || OPTIONS.help ? undefined : materializationSampleCount();

function resolveCargo() {
  const executable = process.platform === 'win32' ? 'cargo.exe' : 'cargo';
  for (const directory of (process.env.PATH ?? '').split(delimiter)) {
    const candidate = join(directory, executable);
    if (directory && isAbsolute(directory) && existsSync(candidate)) {
      return candidate;
    }
  }
  throw new Error('cargo is required to regenerate materialization fixtures');
}

function loadMaterializationFixtures() {
  const generated = spawnSync(
    resolveCargo(),
    [
      'run',
      '--release',
      '--quiet',
      '-p',
      'finstack-quant-portfolio',
      '--example',
      'materialization_fixtures',
      '--',
      MATERIALIZATION_FIXTURE_DIR,
    ],
    { cwd: REPO_ROOT, stdio: 'inherit' }
  );
  if (generated.status !== 0) {
    throw new Error(`materialization fixture generation failed with status ${generated.status}`);
  }
  return {
    a: readFileSync(MATERIALIZATION_FIXTURE_A, 'utf8'),
    b: readFileSync(MATERIALIZATION_FIXTURE_B, 'utf8'),
  };
}

function printHelp() {
  console.log(`finstack-quant-wasm benchmarks (Node.js)

Usage:
  node benchmarks/bench.mjs [options]

Options:
  --help, -h    Show this message
  --materialization-only
                Run only validation and materialization benchmarks
  --valuation-only
                Run required deposit valuation and nonempty portfolio controls
  --core-models-only
                Run 22 checked, batched core/correlation/Black-Scholes controls

Prerequisites:
  The wasm-pack Node build must exist under pkg-node/. If missing, run:
    npm run build:node

Raw wasm-bindgen calls, including Rust pricing and JS result conversion.
Timing uses performance.now(). CCM controls use fixed batched calls, with
10 measured batches and one warmup batch by default. Their model rows are
Black-Scholes closed forms; no option Monte Carlo simulation is measured.
FQ_CCM_RAW_OUTPUT writes their complete raw observations and artifact identity.
`);
}

if (OPTIONS.help) {
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
      is_actual: false,
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
  mode: 'diagonal',
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

const INSTRUMENT_JSON = JSON.stringify({
  schema: 'finstack_quant.instrument/1',
  instrument: {
    type: 'deposit',
    spec: {
      id: 'DEP-BENCH',
      notional: { amount: '100000', currency: 'USD' },
      start_date: '2024-01-01',
      maturity: '2024-07-01',
      day_count: 'act_360',
      fixed_rate: '0.045',
      discount_curve_id: 'USD-OIS',
      attributes: {},
      business_day_convention: 'modified_following',
    },
  },
});

const PORTFOLIO_SPEC = {
  id: 'bench-two-deposits',
  base_currency: 'USD',
  as_of: '2024-01-02',
  entities: { 'BENCH-ENTITY': { id: 'BENCH-ENTITY' } },
  positions: [0, 1].map((index) => {
    const instrument = JSON.parse(INSTRUMENT_JSON).instrument;
    instrument.spec.id = `DEP-BENCH-${index}`;
    instrument.spec.notional.amount = String(100000 * (index + 1));
    instrument.spec.maturity = index === 0 ? '2024-07-01' : '2024-10-01';
    return {
      position_id: `POS-${index}`,
      entity_id: 'BENCH-ENTITY',
      instrument_id: instrument.spec.id,
      instrument_spec: instrument,
      quantity: 1,
      unit: 'units',
    };
  }),
};
const PORTFOLIO_SPEC_JSON = JSON.stringify(PORTFOLIO_SPEC);

const SCENARIO_SPEC_JSON = JSON.stringify({
  id: 'bench-scenario',
  operations: [],
});

/** Canonical `MarketContext` state for discounting (schema version 1). */
const MARKET_CONTEXT_JSON = JSON.stringify({
  schema_version: 1,
  curves: [
    {
      type: 'discount',
      id: 'USD-OIS',
      base: '2024-01-01',
      day_count: 'act_360',
      knot_points: [
        [0.0, 1.0],
        [1.0, 0.98],
        [5.0, 0.88],
      ],
      interp_style: 'monotone_convex',
      extrapolation: 'flat_forward',
      min_forward_rate: null,
      allow_non_monotonic: false,
      min_forward_tenor: 1e-6,
      rate_calibration: null,
      fx_policy: null,
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
  vol_cubes: [],
  hierarchy: null,
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

const COMPOSE_SCENARIOS = [
  { id: 'sc-a', operations: [], priority: 0 },
  { id: 'sc-b', operations: [], priority: 1 },
];

const returns = Array.from({ length: 252 }, (_, i) => ((i % 17) - 8) * 0.001);
const prices = Array.from({ length: 128 }, (_, i) => 100 * (1 + 0.001 * Math.sin(i)));
const priceDates = prices.map((_, i) =>
  new Date(Date.UTC(2025, 0, 1 + i)).toISOString().slice(0, 10)
);
const returnDates = [
  '2024-12-31',
  ...returns.map((_, i) => new Date(Date.UTC(2025, 0, 1 + i)).toISOString().slice(0, 10)),
];
const statsArr = Array.from({ length: 512 }, (_, i) => i * 0.01 + Math.sin(i));
// Flat row-major 3x3 SPD matrix (choleskyDecomposition takes a flat array and n).
const cholMat = [4, 2, 0, 2, 5, 1, 0, 1, 3];

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
  function measureRuns(iterations, fn, setup = () => undefined, cleanup = () => {}) {
    const samples = new Array(iterations);
    let lastResult;
    for (let i = 0; i < iterations; i++) {
      const resource = setup();
      const t0 = performance.now();
      lastResult = fn(resource);
      samples[i] = performance.now() - t0;
      cleanup(resource, lastResult);
    }
    const total = samples.reduce((a, b) => a + b, 0);
    const avg = total / iterations;
    const best = Math.min(...samples);
    const sorted = [...samples].sort((a, b) => a - b);
    const median = sorted[Math.ceil(sorted.length * 0.5) - 1];
    const p95 = sorted[Math.ceil(sorted.length * 0.95) - 1];
    const opsPerSec = avg > 0 ? 1000 / avg : 0;
    return { best, avg, median, p95, opsPerSec, iterations, samples, lastResult };
  }

  /** @type {{ domain: string; name: string; best: number; avg: number; p95: number; opsPerSec: number; iterations: number }[]} */
  const rows = [];

  /**
   * @param {string} domain
   * @param {string} name
   * @param {number} iterations
   * @param {() => void} fn
   */
  function bench(domain, name, iterations, fn, setup = () => undefined, cleanup = () => {}) {
    const r = measureRuns(iterations, fn, setup, cleanup);
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
      p95: 0,
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

  function benchRequired(domain, name, iterations, operation, validate, cleanup = () => {}) {
    const sample = operation();
    try {
      validate(sample);
    } finally {
      cleanup(sample);
    }
    bench(
      domain,
      name,
      iterations,
      operation,
      () => undefined,
      (_unused, result) => cleanup(result)
    );
  }

  function assertMoney(value, currency = 'USD') {
    assert.equal(value.currency, currency);
    assert.ok(typeof value.amount === 'number' || typeof value.amount === 'string');
    assert.ok(Number.isFinite(Number(value.amount)), 'finite monetary amount');
  }

  function assertValuation(result, instrumentId, requestedMetrics = []) {
    assert.equal(result.instrument_id, instrumentId);
    assert.equal(result.as_of, '2024-01-02');
    assertMoney(result.value);
    for (const metric of requestedMetrics) {
      assert.ok(Number.isFinite(result.measures[metric]), `finite requested ${metric}`);
    }
    assert.ok(Object.values(result.measures).every(Number.isFinite));
  }

  function assertPortfolioValuation(result) {
    assert.equal(result.as_of, PORTFOLIO_SPEC.as_of);
    assertMoney(result.total_base_currency);
    assert.deepEqual(result.degraded_positions ?? [], []);
    assert.deepEqual(Object.keys(result.position_values), ['POS-0', 'POS-1']);
    assert.deepEqual(Object.keys(result.by_entity), ['BENCH-ENTITY']);
    assertMoney(result.by_entity['BENCH-ENTITY']);
    for (const position of PORTFOLIO_SPEC.positions) {
      const value = result.position_values[position.position_id];
      assert.equal(value.risk_metrics_complete, true);
      assert.equal(value.risk_error ?? null, null);
      assert.deepEqual(value.inapplicable_metrics ?? [], []);
      assertMoney(value.value_native);
      assertMoney(value.value_base);
      assertValuation(value.valuation_result, position.instrument_id, ['dv01']);
    }
  }

  function assertPortfolioCashflows(result) {
    assert.deepEqual(result.issues, []);
    assert.deepEqual(Object.keys(result.by_position), ['POS-0', 'POS-1']);
    assert.deepEqual(Object.keys(result.position_summaries), ['POS-0', 'POS-1']);
    assert.ok(result.events.length > 0);
    for (const position of PORTFOLIO_SPEC.positions) {
      const events = result.by_position[position.position_id];
      assert.ok(events.length > 0);
      for (const event of events) assertMoney(event.amount);
    }
    for (const event of result.events) assertMoney(event.amount);
  }

  function runValuationBenchmarks() {
    benchRequired(
      'portfolio',
      'parsePortfolioSpecJson (2 deposits)',
      4000,
      () => w.parsePortfolioSpecJson(PORTFOLIO_SPEC_JSON),
      (result) => assert.equal(JSON.parse(result).positions.length, 2)
    );
    benchRequired(
      'valuations',
      'validateInstrumentJson (deposit)',
      2000,
      () => w.validateInstrumentJson(INSTRUMENT_JSON),
      (result) => assert.equal(JSON.parse(result).instrument.type, 'deposit')
    );
    benchRequired(
      'valuations',
      'listStandardMetrics',
      3000,
      () => w.listStandardMetrics(),
      (result) => assert.ok(result.includes('dv01'))
    );
    benchRequired(
      'portfolio',
      'Portfolio.fromSpec + toJson (2 deposits)',
      3000,
      () => {
        const book = w.Portfolio.fromSpec(PORTFOLIO_SPEC_JSON);
        try {
          return { book, json: book.toJson() };
        } catch (error) {
          book.free();
          throw error;
        }
      },
      ({ book, json }) => {
        assert.equal(book.numPositions(), 2);
        assert.equal(JSON.parse(json).positions.length, 2);
      },
      ({ book }) => book.free()
    );

    const portfolioBook = w.Portfolio.fromSpec(PORTFOLIO_SPEC_JSON);
    try {
      assert.equal(portfolioBook.numPositions(), 2);
      const portfolioMarket = w.MarketContext.fromJson(MARKET_CONTEXT_JSON);
      try {
        benchRequired(
          'portfolio',
          'valuePortfolio (2 deposits, strict dv01)',
          200,
          () => w.valuePortfolio(portfolioBook, portfolioMarket, true),
          assertPortfolioValuation
        );
        benchRequired(
          'portfolio',
          'aggregateFullCashflows (2 deposits)',
          200,
          () => w.aggregateFullCashflows(portfolioBook, portfolioMarket, false),
          assertPortfolioCashflows
        );
      } finally {
        portfolioMarket.free();
      }
    } finally {
      portfolioBook.free();
    }

    const priceDeposit = () =>
      w.priceInstrument(INSTRUMENT_JSON, MARKET_CONTEXT_JSON, '2024-01-02', 'discounting');
    const sampleValuation = priceDeposit();
    assertValuation(sampleValuation, 'DEP-BENCH');
    const sampleValuationJson = w.valuationResultToJson(sampleValuation);
    benchRequired(
      'valuations',
      'priceInstrument (deposit discounting PV)',
      150,
      priceDeposit,
      (result) => assertValuation(result, 'DEP-BENCH')
    );
    benchRequired(
      'valuations',
      'priceInstrument (deposit discounting dv01)',
      120,
      () =>
        w.priceInstrument(INSTRUMENT_JSON, MARKET_CONTEXT_JSON, '2024-01-02', 'discounting', [
          'dv01',
        ]),
      (result) => assertValuation(result, 'DEP-BENCH', ['dv01'])
    );
    benchRequired(
      'valuations',
      'valuationResultToJson (deposit result JSON)',
      2000,
      () => w.valuationResultToJson(sampleValuationJson),
      (result) => assert.deepEqual(JSON.parse(result), JSON.parse(sampleValuationJson))
    );
  }

  function runMaterializationBenchmarks() {
    const materializationFixtures = loadMaterializationFixtures();
    const warmMaterializationCache = new w.InstrumentArtifactCache(50);
    const warmSetup = w.Portfolio.fromMaterialization(
      materializationFixtures.b,
      warmMaterializationCache
    );
    console.log(
      `[materialization fixture] input_bytes=${warmSetup.report.input_bytes} ` +
        `unique_artifacts=${warmSetup.report.unique_instruments} ` +
        `positions=${warmSetup.report.positions} dependencies=${warmSetup.report.dependencies}`
    );
    warmSetup.portfolio.free();

    bench(
      'portfolio_materialization',
      'validateMaterialization B (5000/50)',
      MATERIALIZATION_SAMPLES,
      (cache) => w.Portfolio.validateMaterialization(materializationFixtures.b, cache),
      () => new w.InstrumentArtifactCache(50),
      (cache) => cache.free()
    );

    bench(
      'portfolio_materialization',
      'fromMaterialization cold A (5000/5000)',
      MATERIALIZATION_SAMPLES,
      (cache) => w.Portfolio.fromMaterialization(materializationFixtures.a, cache),
      () => new w.InstrumentArtifactCache(5_000),
      (cache, loaded) => {
        loaded.portfolio.free();
        cache.free();
      }
    );

    bench(
      'portfolio_materialization',
      'fromMaterialization cold B (5000/50)',
      MATERIALIZATION_SAMPLES,
      (cache) => w.Portfolio.fromMaterialization(materializationFixtures.b, cache),
      () => new w.InstrumentArtifactCache(50),
      (cache, loaded) => {
        loaded.portfolio.free();
        cache.free();
      }
    );

    bench(
      'portfolio_materialization',
      'fromMaterialization warm B (5000/50)',
      MATERIALIZATION_SAMPLES,
      () => {
        const loaded = w.Portfolio.fromMaterialization(
          materializationFixtures.b,
          warmMaterializationCache
        );
        return loaded;
      },
      () => undefined,
      (_unused, loaded) => {
        loaded.portfolio.free();
      }
    );
    warmMaterializationCache.free();

    const output = process.env.FQ_MATERIALIZATION_RAW_OUTPUT;
    if (output !== undefined) {
      const outputPath = output.startsWith('/') ? output : join(REPO_ROOT, output);
      mkdirSync(dirname(outputPath), { recursive: true });
      const materializationRows = Object.fromEntries(
        rows
          .filter((row) => row.domain === 'portfolio_materialization')
          .map((row) => [
            row.name,
            {
              samples: row.samples,
              counters:
                row.lastResult?.portfolio === undefined ? row.lastResult : row.lastResult.report,
            },
          ])
      );
      writeFileSync(
        outputPath,
        `${JSON.stringify(
          {
            language: 'wasm',
            timing_boundary:
              'fixture string and explicitly-sized cache exist before start; elapsed surrounds the WASM binding call and JavaScript result conversion; handle cleanup is after stop',
            fixture_sha256: {
              a: createHash('sha256').update(materializationFixtures.a).digest('hex'),
              b: createHash('sha256').update(materializationFixtures.b).digest('hex'),
            },
            cases: materializationRows,
          },
          null,
          2
        )}\n`
      );
    }
  }

  function runCoreModelBenchmarks() {
    const startedUtc = new Date().toISOString();
    const settings = ccmSettings(process.env);
    const requiredRows = [];
    const hashFile = (file) => createHash('sha256').update(readFileSync(file)).digest('hex');
    const hashValue = (value) => createHash('sha256').update(JSON.stringify(value)).digest('hex');
    const externalRecord = (name) => {
      const supplied = process.env[name];
      if (!supplied) return null;
      const file = isAbsolute(supplied) ? supplied : join(REPO_ROOT, supplied);
      return {
        path: file,
        sha256: hashFile(file),
        payload: JSON.parse(readFileSync(file, 'utf8')),
      };
    };
    // Artifact hashes establish identity. Only the root's successful release
    // build receipt can establish profile/source coherence; do not infer it.
    const provenance = {
      process_id: process.pid,
      executable: process.execPath,
      node_version: process.version,
      versions: process.versions,
      platform: process.platform,
      arch: process.arch,
      argv: process.argv,
      rayon_num_threads: process.env.RAYON_NUM_THREADS ?? null,
      harness_sha256: hashFile(fileURLToPath(import.meta.url)),
      helper_sha256: hashFile(join(__dirname, 'ccm-measure.mjs')),
      raw_js_sha256: hashFile(WASM_JS),
      wasm_sha256: hashFile(WASM_BIN),
      declarations_sha256: hashFile(join(PKG_NODE_DIR, 'finstack_quant_wasm.d.ts')),
      source_identity: externalRecord('FQ_CCM_SOURCE_IDENTITY'),
      build_receipt: externalRecord('FQ_CCM_BUILD_RECEIPT'),
      profile_evidence:
        'Artifact identity only; release provenance is supplied by the external build receipt.',
    };
    const near = (actual, expected, absolute = CCM_NUMERICS.scalar_absolute) => {
      assert.ok(Number.isFinite(actual), 'finite numeric result');
      assert.ok(
        Math.abs(actual - expected) <= absolute + CCM_NUMERICS.scalar_relative * Math.abs(expected),
        `expected ${expected}, received ${actual}`
      );
    };
    const vector = (actual, expected, absolute) => {
      assert.equal(actual.length, expected.length, 'complete result shape');
      Array.from(actual).forEach((value, index) => near(value, expected[index], absolute));
    };
    const moneyValue = (money) => {
      const currency = money.currency;
      try {
        return { amount: money.amount, currency: currency.toString() };
      } finally {
        currency.free();
      }
    };
    const bounds = (p1, p2) => {
      const denominator = Math.sqrt(p1 * (1 - p1) * p2 * (1 - p2));
      return [
        (Math.max(0, p1 + p2 - 1) - p1 * p2) / denominator,
        (Math.min(p1, p2) - p1 * p2) / denominator,
      ];
    };
    const bernoulli = (p1, p2, correlation) => {
      const p11 = p1 * p2 + correlation * Math.sqrt(p1 * (1 - p1) * p2 * (1 - p2));
      return [p11, p1 - p11, p2 - p11, 1 - p1 - p2 + p11];
    };
    const run = (
      id,
      parameters,
      operation,
      validate,
      snapshot = (value) => value,
      cleanup = () => {},
      boundary = 'Binding call and complete returned value'
    ) => {
      const definition = CCM_NODE_CASES.find((entry) => entry.id === id);
      assert.ok(definition, `registered CCM row ${id}`);
      assert.ok(!requiredRows.some((entry) => entry.id === id), `unique CCM row ${id}`);
      const measured = measureCheckedBatches({
        operation,
        validate,
        snapshot,
        cleanup,
        batchSize: definition.batch_size,
        samples: settings.samples,
        warmupSamples: settings.warmup_samples,
      });
      const row = {
        ...definition,
        parameters,
        fixture_sha256: hashValue(parameters),
        timing_boundary: `${boundary}; batch loop/result assignment included; preflight, warmup, result-buffer allocation, validation, snapshots and disposal excluded`,
        ...measured,
      };
      requiredRows.push(row);
      rows.push(row);
    };
    const disposeCaptured = (result) => disposeAll(result.handles.reverse());
    const shared = captureOwned((own) => {
      const usd = own(new w.Currency('USD'));
      const gaussianSpec = own(w.CopulaSpec.gaussian());
      const recoverySpec = own(w.RecoverySpec.constant(0.4));
      return {
        moneyA: own(new w.Money(100, usd)),
        moneyB: own(new w.Money(25.5, usd)),
        dayCount: own(w.DayCount.act360()),
        dayCountContext: own(new w.DayCountContext()),
        curve: own(
          new w.DiscountCurve({
            id: 'USD-OIS',
            baseDate: '2024-01-02',
            knots: [0, 1, 1, 0.98, 5, 0.88],
          })
        ),
        fxPolicy: own(new w.FxConversionPolicy()),
        pair: own(new w.CorrelatedBernoulli(0.05, 0.08, 0.2)),
        gaussian: own(gaussianSpec.build()),
        recovery: own(recoverySpec.build()),
      };
    });
    let firstError;
    try {
      const {
        moneyA,
        moneyB,
        dayCount,
        dayCountContext,
        curve,
        fxPolicy,
        pair,
        gaussian,
        recovery,
      } = shared.output;
      const start = w.createDate(2024, 1, 2);
      const end = w.createDate(2025, 1, 2);
      const dfKnots = [
        [0, 1],
        [1, 0.98],
        [5, 0.88],
      ];
      dfKnots.forEach(([time, df]) => near(curve.df(time), df));
      assert.equal(gaussian.numFactors, 1);
      assert.equal(gaussian.modelName, 'One-Factor Gaussian Copula');

      run(
        'core/currency_constructor',
        { code: 'USD' },
        () => new w.Currency('USD'),
        (value) => {
          assert.equal(value.toString(), 'USD');
          assert.equal(value.numeric, 840);
          assert.equal(value.decimals, 2);
        },
        (value) => ({ code: value.toString(), numeric: value.numeric, decimals: value.decimals }),
        (value) => value.free(),
        'Currency construction'
      );

      const moneySnapshot = (value) => value.output.map(moneyValue);
      run(
        'core/money_add_sub',
        { a: 100, b: 25.5, currency: 'USD', operations: ['a+b', 'a-b'] },
        () =>
          captureOwned((own) => [own(moneyA.checkedAdd(moneyB)), own(moneyA.checkedSub(moneyB))]),
        (value) => {
          assert.deepEqual(moneySnapshot(value), [
            { amount: 125.5, currency: 'USD' },
            { amount: 74.5, currency: 'USD' },
          ]);
        },
        moneySnapshot,
        disposeCaptured,
        'Two independent Money operations, shared inputs'
      );

      run(
        'core/daycount_year_fraction',
        { convention: 'act_360', start: '2024-01-02', end: '2025-01-02', actual_days: 366 },
        () => dayCount.yearFraction(start, end, dayCountContext),
        (value) => near(value, 366 / 360)
      );
      run(
        'core/discount_curve_df',
        {
          id: 'USD-OIS',
          base_date: '2024-01-02',
          knots: dfKnots,
          time_years: 1.25,
          interpolation: 'monotone_convex',
          extrapolation: 'flat_forward',
        },
        () => curve.df(1.25),
        (value) => {
          assert.ok(
            Number.isFinite(value) && value > 0.88 && value < 0.98,
            'DF inside supplied neighboring pillars'
          );
        }
      );

      const checkCholesky = (factor) => {
        assert.equal(factor.length, 9);
        for (let i = 0; i < 3; i++) {
          assert.ok(factor[i * 3 + i] > 0);
          for (let j = 0; j < 3; j++) {
            assert.ok(Number.isFinite(factor[i * 3 + j]));
            if (j > i) near(factor[i * 3 + j], 0);
            let reconstructed = 0;
            for (let k = 0; k < 3; k++) reconstructed += factor[i * 3 + k] * factor[j * 3 + k];
            near(reconstructed, cholMat[i * 3 + j], CCM_NUMERICS.matrix_absolute);
          }
        }
      };
      run(
        'core/cholesky_decomposition',
        { matrix: cholMat, dimension: 3 },
        () => w.choleskyDecomposition(cholMat, 3),
        checkCholesky,
        (value) => Array.from(value)
      );
      const referenceSum = statsArr.reduce((sum, value) => sum + value, 0);
      const referenceMean = referenceSum / statsArr.length;
      const referenceVariance =
        statsArr.reduce((sum, value) => sum + (value - referenceMean) ** 2, 0) /
        (statsArr.length - 1);
      run(
        'core/mean_variance_512',
        { data: statsArr, count: 512, variance_denominator: 'n-1' },
        () => [w.mean(statsArr), w.variance(statsArr)],
        (value) =>
          vector(value, [referenceMean, referenceVariance], CCM_NUMERICS.statistics_absolute)
      );

      run(
        'core/rate_bps',
        { decimal_rate: 0.05, basis_points: 50 },
        () =>
          captureOwned((own) => {
            const rate = own(new w.Rate(0.05));
            const bps = own(new w.Bps(50));
            return {
              decimal: rate.asDecimal,
              percent: rate.asPercent,
              bp: rate.asBp,
              bps_decimal: bps.asDecimal,
            };
          }),
        (value) =>
          assert.deepEqual(value.output, {
            decimal: 0.05,
            percent: 5,
            bp: 500,
            bps_decimal: 0.005,
          }),
        (value) => value.output,
        disposeCaptured,
        'Rate and Bps construction and numeric conversions'
      );
      run(
        'core/tenor_parse',
        { text: '3M', replaces_invalid_row: 'Tenor construction + toYears' },
        () =>
          captureOwned((own) => {
            const tenor = own(w.Tenor.parse('3M'));
            return { tenor, years: tenor.toYearsApprox() };
          }),
        (value) => {
          assert.equal(value.output.tenor.toString(), '3M');
          near(value.output.years, 0.25);
        },
        (value) => ({ text: value.output.tenor.toString(), years: value.output.years }),
        disposeCaptured,
        'Tenor parse and toYears'
      );
      const forwardParameters = {
        id: 'USD-FWD',
        tenor: 0.25,
        baseDate: '2024-01-02',
        knots: [0, 0.03, 5, 0.035],
      };
      run(
        'core/forward_curve',
        { ...forwardParameters, time_years: 1.25, interpolation: 'linear' },
        () =>
          captureOwned((own) => {
            const forward = own(new w.ForwardCurve(forwardParameters));
            return forward.rate(1.25);
          }),
        (value) => near(value.output, 0.03125),
        (value) => value.output,
        disposeCaptured,
        'ForwardCurve construction and rate lookup'
      );
      run(
        'core/fx_matrix',
        { quotes: [['USD', 'EUR', 0.92]], date: '2024-01-02', policy: 'cashflow_date' },
        () =>
          captureOwned((own) => {
            const fx = own(new w.FxMatrix());
            fx.setQuotes([['USD', 'EUR', 0.92]]);
            return own(fx.rate('USD', 'EUR', '2024-01-02', fxPolicy));
          }),
        (value) => {
          near(value.output.rate, 0.92);
          assert.equal(value.output.triangulated, false);
        },
        (value) => ({ rate: value.output.rate, triangulated: value.output.triangulated }),
        disposeCaptured,
        'FxMatrix construction, setQuotes and direct lookup; shared explicit policy'
      );

      const factor = w.choleskyDecomposition(cholMat, 3);
      checkCholesky(factor);
      const rhs = [1, 2, 3];
      run(
        'core/cholesky_solve',
        { matrix: cholMat, rhs, dimension: 3 },
        () => w.choleskySolve(factor, rhs),
        (value) => {
          assert.equal(value.length, 3);
          for (let i = 0; i < 3; i++) {
            let reconstructed = 0;
            for (let j = 0; j < 3; j++) reconstructed += cholMat[i * 3 + j] * value[j];
            near(reconstructed, rhs[i], CCM_NUMERICS.matrix_absolute);
          }
        },
        (value) => Array.from(value)
      );
      run(
        'core/norm_cdf',
        { x: 0.42 },
        () => w.normCdf(0.42),
        (value) => near(value, CCM_NUMERICS.normal_cdf_042)
      );
      const sorted = [...statsArr].sort((a, b) => a - b);
      const rank = 0.25 * (statsArr.length - 1);
      const lo = Math.floor(rank);
      const referenceQuantile = sorted[lo] + (rank - lo) * (sorted[lo + 1] - sorted[lo]);
      run(
        'core/quantile_512',
        { data: statsArr, count: 512, probability: 0.25, rank: 'R7' },
        () => w.quantile(statsArr, 0.25),
        (value) => near(value, referenceQuantile)
      );
      run(
        'core/kahan_sum_512',
        { data: statsArr, count: 512 },
        () => w.kahanSum(statsArr),
        (value) => near(value, referenceSum, CCM_NUMERICS.statistics_absolute)
      );
      const checkCalendars = (value) => {
        assert.ok(Array.isArray(value) && value.length > 0);
        assert.ok(value.every((code) => typeof code === 'string' && code.length > 0));
        assert.equal(new Set(value).size, value.length);
        assert.ok(value.includes('target2'), 'fixed TARGET2 calendar exists');
        assert.deepEqual(value, [...value].sort());
      };
      run(
        'core/available_calendars',
        { ordering: 'alphabetical', required_calendar: 'target2' },
        () => w.availableCalendars(),
        checkCalendars
      );
      run(
        'core/adjust',
        {
          date: '2024-01-02',
          convention: 'modified_following',
          calendar: 'target2',
          branch: 'already_business_day',
        },
        () => w.adjust(start, 'modified_following', 'target2'),
        (value) => assert.equal(value, start)
      );
      run(
        'correlation/bounds',
        { p1: 0.05, p2: 0.08 },
        () => w.correlationBounds(0.05, 0.08),
        (value) => vector(value, bounds(0.05, 0.08)),
        (value) => Array.from(value)
      );
      run(
        'correlation/bernoulli',
        { p1: 0.05, p2: 0.08, correlation: 0.2, order: ['p11', 'p10', 'p01', 'p00'] },
        () => pair.jointProbabilities(),
        (value) => {
          vector(value, bernoulli(0.05, 0.08, 0.2));
          assert.ok(Array.from(value).every((probability) => probability >= 0 && probability <= 1));
          near(
            Array.from(value).reduce((sum, probability) => sum + probability, 0),
            1
          );
        },
        (value) => Array.from(value),
        () => {},
        'Joint probability query; Bernoulli construction excluded'
      );
      run(
        'correlation/gaussian',
        {
          model: 'Gaussian',
          factors: 1,
          threshold: -0.5,
          factor_realization: [0.1],
          correlation: 0.25,
          replaces_misleading_label: 'Copula gaussian + conditionalDefaultProb',
        },
        () => gaussian.conditionalDefaultProb(-0.5, [0.1], 0.25),
        (value) => near(value, CCM_NUMERICS.gaussian_conditional),
        (value) => value,
        () => {},
        'Conditional probability query; specification/build excluded'
      );
      run(
        'correlation/recovery',
        {
          model: 'constant',
          recovery: 0.4,
          market_factor: -0.15,
          replaces_misleading_label: 'RecoverySpec.constant + conditionalRecovery',
        },
        () => recovery.conditionalRecovery(-0.15),
        (value) => near(value, 0.4),
        (value) => value,
        () => {},
        'Recovery query; specification/build excluded'
      );
      const bsParameters = {
        spot: 100,
        strike: 100,
        rate: 0.05,
        dividend_yield: 0,
        volatility: 0.2,
        expiry_years: 1,
      };
      run(
        'models/bs_call',
        { ...bsParameters, option_type: 'call' },
        () => w.bsPrice(100, 100, 0.05, 0, 0.2, 1, true),
        (value) => near(value, CCM_NUMERICS.black_scholes_call, CCM_NUMERICS.black_scholes_absolute)
      );
      run(
        'models/bs_put',
        { ...bsParameters, option_type: 'put' },
        () => w.bsPrice(100, 100, 0.05, 0, 0.2, 1, false),
        (value) => near(value, CCM_NUMERICS.black_scholes_put, CCM_NUMERICS.black_scholes_absolute)
      );
    } catch (error) {
      firstError = error;
    }
    try {
      disposeAll(shared.handles.reverse());
    } catch (error) {
      firstError ??= error;
    }
    if (firstError) throw firstError;
    assert.deepEqual(
      requiredRows.map((row) => row.id),
      CCM_NODE_CASES.map((row) => row.id)
    );
    const output = process.env.FQ_CCM_RAW_OUTPUT;
    if (output) {
      const file = isAbsolute(output) ? output : join(REPO_ROOT, output);
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(
        file,
        `${JSON.stringify(
          {
            schema_version: 1,
            protocol: 'CCM-B2-host-v1',
            selector: CORE_MODELS_ONLY ? '--core-models-only' : 'all',
            started_utc: startedUtc,
            completed_utc: new Date().toISOString(),
            provenance,
            settings,
            numerical_contract: CCM_NUMERICS,
            normal_reference_provenance:
              'Independent erf Taylor expansion, 40 terms, computed before the first host run; no project code used.',
            expected_ids: CCM_NODE_CASES.map((row) => row.id),
            direct_cashflows_coverage: 'absent',
            cases: requiredRows,
          },
          null,
          2
        )}\n`,
        { flag: 'wx' }
      );
    }
  }

  function printRows() {
    const dW = Math.max(12, ...rows.map((r) => r.domain.length));
    const nW = Math.max(36, ...rows.map((r) => r.name.length));

    console.log('\nfinstack-quant-wasm benchmarks (raw wasm-bindgen pkg-node)\n');
    console.log(
      `${'Domain'.padEnd(dW)}  ${'Benchmark'.padEnd(nW)}  ${'Iter'.padStart(5)}  ${'Best (ms)'.padStart(10)}  ${'Avg (ms)'.padStart(10)}  ${'Median (ms)'.padStart(11)}  ${'p95 (ms)'.padStart(10)}  ${'Ops/sec'.padStart(12)}`
    );
    console.log(
      `${''.padEnd(dW, '-')}  ${''.padEnd(nW, '-')}  ${''.padStart(5, '-')}  ${''.padStart(10, '-')}  ${''.padStart(10, '-')}  ${''.padStart(11, '-')}  ${''.padStart(10, '-')}  ${''.padStart(12, '-')}`
    );

    for (const r of rows) {
      const bestStr = r.iterations ? r.best.toFixed(4) : '—';
      const avgStr = r.iterations ? r.avg.toFixed(4) : '—';
      const medianStr = r.iterations ? r.median.toFixed(4) : '—';
      const p95Str = r.iterations ? r.p95.toFixed(4) : '—';
      const opsStr = r.iterations ? r.opsPerSec.toFixed(0) : '—';
      const iterStr = r.iterations ? String(r.iterations) : '—';
      console.log(
        `${r.domain.padEnd(dW)}  ${r.name.padEnd(nW)}  ${iterStr.padStart(5)}  ${bestStr.padStart(10)}  ${avgStr.padStart(10)}  ${medianStr.padStart(11)}  ${p95Str.padStart(10)}  ${opsStr.padStart(12)}`
      );
    }
    console.log('');
  }

  if (CORE_MODELS_ONLY) {
    runCoreModelBenchmarks();
    printRows();
    return;
  }

  if (MATERIALIZATION_ONLY) {
    runMaterializationBenchmarks();
    printRows();
    return;
  }

  runValuationBenchmarks();
  if (VALUATION_ONLY) {
    printRows();
    return;
  }

  runCoreModelBenchmarks();
  const pricePerf = new w.Performance({
    dates: priceDates,
    series: [prices],
    kind: 'prices',
    ticker_names: ['bench'],
    frequency: 'daily',
  });
  const returnPerf = new w.Performance({
    dates: returnDates,
    series: [returns, returns.map((x) => x * 0.95)],
    kind: 'returns',
    ticker_names: ['bench', 'benchmark'],
    benchmark_ticker: 'benchmark',
    frequency: 'daily',
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

  bench('analytics', 'getDrawdownSeries', 6000, () => {
    returnPerf.getDrawdownSeries();
  });

  bench('analytics', 'maxDrawdown', 8000, () => {
    returnPerf.maxDrawdown();
  });

  const csaCanonical = w.csaSpecUsdRegulatory();

  bench('margin', 'csaSpecUsdRegulatory', 2000, () => {
    w.csaSpecUsdRegulatory();
  });

  bench('margin', 'csaSpecValidate', 2000, () => {
    w.csaSpecValidate(csaCanonical);
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

  runMaterializationBenchmarks();

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
      p95: 0,
      opsPerSec: 0,
      iterations: 0,
    });
  }

  bench('scenarios', 'validateScenarioSpec', 5000, () => {
    w.validateScenarioSpec(SCENARIO_SPEC_JSON);
  });

  // --- Extended benchmarks (additional coverage) --------------------------------

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
    returnPerf.excessReturns(benchRf);
  });

  bench('analytics', 'getDrawdownSeries', 4000, () => {
    returnPerf.getDrawdownSeries();
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

  bench('margin', 'csaSpecEurRegulatory', 2000, () => {
    w.csaSpecEurRegulatory();
  });

  benchTry('margin', 'VmCalculator.calculate', 3000, () => {
    new w.VmCalculator(csaCanonical).calculate(1000000.0, 800000.0, 'USD', '2024-06-15');
  });

  benchTry('statements_analytics', 'runVariance', 800, () => {
    w.runVariance(STATEMENT_RESULT_BASE_JSON, STATEMENT_RESULT_CMP_JSON, VARIANCE_CONFIG_JSON);
  });

  benchTry('statements_analytics', 'evaluateScenarioSet', 200, () => {
    w.evaluateScenarioSet(FINANCIAL_MODEL_JSON, SCENARIO_SET_JSON);
  });

  let sensitivityResultJson = '';
  try {
    // `runSensitivity` returns a structured object; `generateTornadoEntries`
    // still takes the sensitivity result as JSON.
    sensitivityResultJson = JSON.stringify(
      w.runSensitivity(FINANCIAL_MODEL_JSON, SENSITIVITY_CONFIG_JSON)
    );
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

  benchTry('statements_analytics', 'evaluateMonteCarlo', 30, () => {
    const evaluator = new w.Evaluator();
    evaluator.evaluateMonteCarlo(MONTE_CARLO_MODEL_JSON, MONTE_CARLO_CONFIG_JSON);
    evaluator.free();
  });

  benchTry('statements_analytics', 'goalSeek', 400, () => {
    w.goalSeek(GOAL_SEEK_MODEL_JSON, {
      target_node: 'ebitda',
      target_period: '2025Q1',
      target_value: 50000.0,
      driver_node: 'revenue',
      driver_period: '2025Q1',
      update_model: false,
    });
  });

  benchTry('statements_analytics', 'dependencyTree', 2000, () => {
    const tracer = new w.DependencyTracer(FINANCIAL_MODEL_JSON);
    tracer.dependencyTree('revenue');
    tracer.free();
  });

  benchTry('statements_analytics', 'explainFormula', 1500, () => {
    w.explainFormula(FINANCIAL_MODEL_JSON, STATEMENT_RESULT_BASE_JSON, 'revenue', '2025Q1');
  });

  bench('scenarios', 'parseScenarioSpec', 5000, () => {
    w.parseScenarioSpec(SCENARIO_SPEC_JSON);
  });

  bench('scenarios', 'composeScenarios', 3000, () => {
    w.composeScenarios(COMPOSE_SCENARIOS);
  });

  bench('scenarios', 'parseScenarioSpec (object)', 4000, () => {
    w.parseScenarioSpec({ id: 'composed-inline', operations: [], priority: 0 });
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
    w.applyScenario(SCENARIO_SPEC_JSON, MARKET_CONTEXT_JSON, '2024-01-02', FINANCIAL_MODEL_JSON);
  });

  benchTry('scenarios', 'applyScenario (market only)', 150, () => {
    w.applyScenario(SCENARIO_SPEC_JSON, MARKET_CONTEXT_JSON, '2024-01-02');
  });

  printRows();
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
