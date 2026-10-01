/**
 * Runtime outputs satisfy the published, schema-generated TypeScript types.
 *
 * Runs the shipped calibration examples, a dry run, a canonicalized envelope,
 * two portfolio materializations (including a notional-unit position) and a
 * Monte Carlo valuation, writes each value as a typed literal
 * (`const x: CalibrationResultEnvelope = {...}`) and compiles the file with
 * `tsc --strict` under NodeNext with `skipLibCheck: false`. A generated type
 * that disagrees with the serde wire fails here.
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import init, {
  analytics,
  attribution,
  calibration,
  core,
  covenants,
  features,
  margin,
  models,
  portfolio,
  scenarios,
  statements,
  statements_analytics,
  valuations,
} from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const PACKAGE = fileURLToPath(new URL('../../', import.meta.url));
const EXAMPLES = new URL(
  '../../../finstack-quant/calibration/examples/market_bootstrap/',
  import.meta.url
);
const MATERIALIZATION = new URL(
  '../../../finstack-quant/portfolio/tests/data/canonical/portfolio_materialization.json',
  import.meta.url
);
const BARRIER = new URL(
  '../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/fx_barrier_option/eurusd_up_out_call_3m_quantlib.json',
  import.meta.url
);

/**
 * TypeScript source for a runtime value, keeping its JavaScript form: a
 * `bigint` stays a bigint literal, non-finite numbers stay `NaN` /
 * `Infinity`, and typed arrays stay typed arrays.
 */
function literal(value) {
  if (typeof value === 'bigint') return `${value}n`;
  if (typeof value === 'number') {
    if (Number.isNaN(value)) return 'NaN';
    if (!Number.isFinite(value)) return value > 0 ? 'Infinity' : '-Infinity';
    return JSON.stringify(value);
  }
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (ArrayBuffer.isView(value)) {
    return `new ${value.constructor.name}([${Array.from(value, literal).join(', ')}])`;
  }
  if (Array.isArray(value)) return `[${value.map(literal).join(', ')}]`;
  const fields = Object.entries(value)
    .filter(([, item]) => item !== undefined)
    .map(([key, item]) => `${JSON.stringify(key)}: ${literal(item)}`);
  return `{${fields.join(', ')}}`;
}

function typecheck(declarations) {
  const dir = mkdtempSync(join(tmpdir(), 'fq-contract-types-'));
  try {
    const source = [
      `import type * as fq from ${JSON.stringify(join(PACKAGE, 'index.js'))};`,
      `import type * as types from ${JSON.stringify(join(PACKAGE, 'types/generated/index.js'))};`,
      ...declarations.map(
        ([name, type, value], i) => `export const v${i}_${name}: ${type} = ${literal(value)};`
      ),
    ].join('\n');
    writeFileSync(join(dir, 'probe.mts'), source);
    writeFileSync(
      join(dir, 'tsconfig.json'),
      JSON.stringify({
        compilerOptions: {
          target: 'ES2022',
          module: 'NodeNext',
          moduleResolution: 'NodeNext',
          lib: ['ES2022', 'DOM'],
          strict: true,
          noEmit: true,
          skipLibCheck: false,
          types: [],
        },
        files: ['probe.mts'],
      })
    );
    const tsc = join(PACKAGE, 'node_modules/typescript/bin/tsc');
    const run = spawnSync(process.execPath, [tsc, '-p', dir], { encoding: 'utf8' });
    assert.equal(run.status, 0, `tsc rejected runtime outputs:\n${run.stdout}${run.stderr}`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('calibration envelopes, results, dry runs and canonical JSON match the generated types', () => {
  const declarations = [];
  for (const file of readdirSync(EXAMPLES)
    .filter((name) => name.endsWith('.json'))
    .sort()) {
    const name = file.replace(/\W/g, '_');
    const envelope = JSON.parse(readFileSync(new URL(file, EXAMPLES), 'utf8'));
    declarations.push([`${name}_input`, 'fq.CalibrationEnvelope', envelope]);
    const canonical = JSON.parse(calibration.validateCalibrationJson(envelope));
    declarations.push([`${name}_canonical`, 'fq.CalibrationEnvelope', canonical]);
    declarations.push([
      `${name}_dry_run`,
      'fq.CalibrationValidationReport',
      calibration.dryRun(envelope),
    ]);
    declarations.push([
      `${name}_result`,
      'fq.CalibrationResultEnvelope',
      calibration.calibrate(envelope),
    ]);
  }
  assert.equal(declarations.length, 48, 'all twelve shipped examples are exercised');
  const broken = {
    schema: 'finstack_quant.calibration/1',
    plan: {
      id: 'p',
      steps: [
        {
          id: 's',
          quote_set: 'missing',
          kind: 'discount',
          curve_id: 'USD-OIS',
          currency: 'USD',
          base_date: '2026-01-02',
        },
      ],
    },
  };
  const findings = calibration.dryRun(broken);
  assert.ok(findings.errors.length > 0, 'the dry run reports the undefined quote set');
  declarations.push(['dry_run_errors', 'fq.CalibrationValidationReport', findings]);
  typecheck(declarations);
});

test('materialization bundles and reports match the generated types', () => {
  const bundle = JSON.parse(readFileSync(MATERIALIZATION, 'utf8'));
  const notional = structuredClone(bundle);
  notional.positions[0].unit = { notional: 'USD' };
  notional.instruments[0].dependencies = null;
  const declarations = [];
  for (const [name, input] of [
    ['canonical', bundle],
    ['notional', notional],
  ]) {
    const text = JSON.stringify(input);
    const loaded = portfolio.Portfolio.fromMaterialization(text);
    try {
      declarations.push([
        `${name}_bundle`,
        'types.portfolio.PortfolioMaterializationEnvelope',
        input,
      ]);
      declarations.push([`${name}_report`, 'fq.MaterializationReport', loaded.report]);
      declarations.push([
        `${name}_validation`,
        'fq.MaterializationReport | fq.ValidationReport',
        portfolio.Portfolio.validateMaterialization(text),
      ]);
    } finally {
      loaded.portfolio.free();
    }
  }
  typecheck(declarations);
});

test('a Monte Carlo valuation matches the bigint host contract', () => {
  const golden = JSON.parse(readFileSync(BARRIER, 'utf8'));
  const envelope = structuredClone(golden.instrument);
  Object.assign(envelope.instrument.spec, {
    monitoring: { type: 'discrete', observation_dates: ['2026-05-29', '2026-06-30', '2026-07-30'] },
    monitoring_start_date: '2026-05-29',
    instrument_pricing_overrides: { model_config: { mc_paths: 256 } },
  });
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(envelope),
    JSON.stringify(golden.market.data),
    '2026-04-30',
    'monte_carlo_gbm',
    []
  );
  assert.equal(typeof result.details.data.seed, 'bigint');
  typecheck([['valuation', 'fq.ValuationResult', result]]);
});

const AS_OF = '2025-01-15';
const OIS_MARKET = {
  schema_version: 1,
  curves: [
    {
      type: 'discount',
      id: 'USD-OIS',
      base: AS_OF,
      day_count: 'act_365f',
      knot_points: [
        [0.0, 1.0],
        [0.5, 0.98],
        [1.0, 0.96],
        [2.0, 0.92],
      ],
      interp_style: 'monotone_convex',
      extrapolation: 'flat_forward',
      min_forward_rate: -0.005,
      allow_non_monotonic: false,
      min_forward_tenor: 1e-6,
      rate_calibration: null,
      calibration_ois_cutoff_days: null,
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
  collateral: {},
  hierarchy: null,
};
const DEPOSIT = {
  schema: 'finstack_quant.instrument/1',
  instrument: {
    type: 'deposit',
    spec: {
      id: 'DEP-0',
      notional: { amount: '1000000', currency: 'USD' },
      start_date: AS_OF,
      maturity: '2025-07-15',
      day_count: 'act_360',
      fixed_rate: '0.04',
      discount_curve_id: 'USD-OIS',
      attributes: {},
    },
  },
};

/** Two-quarter model whose `lag` node is undefined (NaN) in the first quarter. */
const LAG_MODEL = {
  id: 'lag',
  schema_version: 1,
  periods: [
    { id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: false },
    { id: '2025Q2', start: '2025-04-01', end: '2025-07-01', is_actual: false },
  ],
  nodes: {
    revenue: { node_id: 'revenue', node_type: 'value', values: { '2025Q1': 100, '2025Q2': 110 } },
    lagged: { node_id: 'lagged', node_type: 'calculated', formula_text: 'lag(revenue, 1)' },
  },
};

test('namespace results match their published result types', () => {
  const declarations = [];

  // analytics: typed-array survivors and numeric non-finite results.
  const dates = [
    '2024-01-01',
    '2024-01-02',
    '2024-01-03',
    '2024-01-04',
    '2024-01-05',
    '2024-01-08',
  ];
  const perf = analytics.Performance.fromReturns(
    dates,
    [
      [0.01, -0.02, 0.015, 0.003, -0.004, 0.02],
      [0.005, -0.01, 0.01, 0.001, -0.002, 0.01],
    ],
    ['A', 'B'],
    'B',
    'daily'
  );
  try {
    declarations.push(['beta', 'fq.BetaResult[]', perf.beta()]);
    declarations.push(['greeks', 'fq.GreeksResult[]', perf.greeks()]);
    declarations.push(['rolling_greeks', 'fq.RollingGreeks', perf.rollingGreeks(0, 3)]);
    declarations.push(['rolling_returns', 'fq.DatedSeries', perf.rollingReturns(0, 3)]);
    declarations.push(['drawdowns', 'fq.DrawdownEpisode[]', perf.drawdownDetails(0)]);
    declarations.push(['period_stats', 'fq.PeriodStats', perf.periodStats(0, 'daily')]);
    declarations.push(['lookback', 'fq.LookbackReturns', perf.lookbackReturns('2024-01-08')]);
  } finally {
    perf.free();
  }

  // attribution
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/attribution/tests/fixtures/production_convertible_credit.json',
        import.meta.url
      ),
      'utf8'
    )
  );
  const params = new attribution.AttributionJsonInputs(
    JSON.stringify(fixture.instrument),
    JSON.stringify(fixture.market_t0),
    JSON.stringify(fixture.market_t1),
    fixture.as_of_t0,
    fixture.as_of_t1,
    JSON.stringify('metrics_based'),
    null,
    null
  );
  try {
    declarations.push(['pnl_attribution', 'fq.PnlAttribution', attribution.attributePnl(params)]);
  } finally {
    params.free();
  }

  // covenants
  const specs = JSON.parse(covenants.lboStandardJson(5.0, 1.5, 1.2, 10_000_000.0));
  const engine = covenants.validateCovenantEngineJson(
    JSON.stringify({ specs: [specs[0]], breach_history: [], windows: [], waivers: [] })
  );
  declarations.push([
    'covenant_reports',
    'Record<string, fq.CovenantReport>',
    covenants.evaluateEngine(engine, JSON.stringify({ debt_to_ebitda: 4.0 }), '2026-03-31'),
  ]);

  // features
  declarations.push([
    'feature_values',
    'fq.FeatureValue[]',
    features.transformTimeseries(
      [1, 3, 6],
      ['ACME', 'ACME', 'ACME'],
      ['2026-01-01', '2026-01-02', '2026-01-03'],
      'diff'
    ),
  ]);

  // margin
  const discount = new core.DiscountCurve({
    id: 'USD-OIS',
    baseDate: '2025-01-01',
    knots: [0.0, 1.0, 4.0, 1.0],
    interp: 'log_linear',
  });
  const hazard = core.HazardCurve.flat('HZ', '2025-01-01', 0.02, 0.4);
  try {
    const xva = margin.computeBilateralXva(
      JSON.stringify({ times: [1.0, 2.0], mtm_values: [1e6, 1e6], epe: [1e6, 1e6], ene: [0, 0] }),
      hazard,
      hazard,
      discount,
      0.4,
      0.4,
      JSON.stringify({ funding_spread_bp: 50.0 })
    );
    assert.equal(typeof xva.meta.numeric_mode, 'string', 'XVA results carry their policy stamp');
    declarations.push(['xva', 'fq.XvaResult', xva]);
  } finally {
    discount.free();
    hazard.free();
  }

  // models
  declarations.push([
    'lvar_bangia',
    'fq.LvarBangiaScalar',
    models.liquidity.lvarBangia(-100_000, 0.002, 0.0005, 0.99, 1_000_000),
  ]);
  declarations.push([
    'risk_budget',
    'fq.RiskBudgetResult',
    models.factor.risk.evaluateRiskBudget(['A', 'B'], [60, 40], [1, 0], 100),
  ]);

  // portfolio: attribution results and the bigint valuation route.
  const sectors = JSON.stringify([
    {
      sector: 'A',
      portfolio_weight: 0.6,
      benchmark_weight: 0.4,
      portfolio_return: 0.05,
      benchmark_return: 0.06,
    },
    {
      sector: 'B',
      portfolio_weight: 0.4,
      benchmark_weight: 0.6,
      portfolio_return: 0.01,
      benchmark_return: 0.03,
    },
  ]);
  declarations.push(['brinson', 'fq.BrinsonPeriodResult', portfolio.brinsonFachler(sectors)]);
  declarations.push([
    'twrr',
    'fq.LinkedReturn',
    portfolio.twrrLinked(JSON.stringify([0.05, 0.03]), 1.0),
  ]);
  const golden = JSON.parse(readFileSync(BARRIER, 'utf8'));
  const barrier = structuredClone(golden.instrument);
  Object.assign(barrier.instrument.spec, {
    monitoring: { type: 'discrete', observation_dates: ['2026-05-29', '2026-06-30', '2026-07-30'] },
    monitoring_start_date: '2026-05-29',
    instrument_pricing_overrides: { model_config: { mc_paths: 256 } },
  });
  const book = JSON.stringify({
    id: 'mc_book',
    name: 'MC book',
    base_currency: 'USD',
    as_of: '2026-04-30',
    entities: { E: { id: 'E', name: null, meta: {} } },
    positions: [
      {
        position_id: 'BARRIER',
        entity_id: 'E',
        instrument_id: barrier.instrument.spec.id,
        instrument_spec: barrier.instrument,
        quantity: 1,
        unit: 'units',
      },
    ],
  });
  const barrierMarket = JSON.stringify(golden.market.data);
  const valuation = portfolio.valuePortfolio(book, barrierMarket, false, []);
  assert.equal(
    typeof valuation.position_values.BARRIER.valuation_result.details.data.seed,
    'bigint'
  );
  declarations.push(['portfolio_valuation', 'fq.PortfolioValuation', valuation]);
  declarations.push([
    'scenario_revalue',
    'fq.ScenarioRevalueView',
    portfolio.applyScenarioAndRevalue(
      book,
      JSON.stringify({ id: 's', operations: [] }),
      barrierMarket
    ),
  ]);

  // scenarios
  const up25 = { kind: 'curve_parallel_bp', curve_kind: 'discount', curve_id: 'USD-OIS', bp: 25 };
  const spec = scenarios.parseScenarioSpec(JSON.stringify({ id: 'up25', operations: [up25] }));
  assert.equal(spec.name, null, 'an absent name serializes as null');
  declarations.push(['scenario_spec', 'fq.ScenarioSpec', spec]);
  declarations.push([
    'application',
    'fq.ApplicationEnvelope',
    scenarios.applyScenarioToMarket(spec, OIS_MARKET, AS_OF, [DEPOSIT]),
  ]);
  const hold = scenarios.buildScenarioSpec('hold_1m_up25', [
    { kind: 'time_roll_forward', period: '1M', apply_shocks: true, roll_mode: 'calendar_days' },
    up25,
  ]);
  declarations.push([
    'horizon',
    'fq.HorizonReport',
    scenarios.computeHorizonReturn(DEPOSIT, OIS_MARKET, AS_OF, hold),
  ]);

  // statements: the evaluator keeps the JSON sentinel strings.
  const results = new statements.Evaluator().evaluate(LAG_MODEL);
  assert.equal(results.nodes.lagged['2025Q1'], 'nan');
  declarations.push(['statement_result', 'fq.StatementResult', results]);

  // statements_analytics: flat results restore non-finite values as numbers.
  const metrics = statements_analytics.backtestForecast([0.0, 0.0], [1.0, 2.0]);
  assert.ok(Number.isNaN(metrics.mape));
  declarations.push(['forecast_metrics', 'fq.ForecastMetrics', metrics]);
  const explanation = statements_analytics.explainFormula(
    LAG_MODEL,
    JSON.stringify(results),
    'lagged',
    '2025Q1'
  );
  assert.ok(Number.isNaN(explanation.final_value));
  declarations.push(['explanation', 'fq.Explanation', explanation]);
  declarations.push([
    'peer_stats',
    'fq.PeerStats',
    statements_analytics.peerStats([1.0, 2.0, 3.0, 4.0]),
  ]);

  typecheck(declarations);
});
