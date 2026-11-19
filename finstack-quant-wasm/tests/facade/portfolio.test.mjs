import { core as portfolioCore } from '../../index.js';
/**
 * Portfolio-namespace facade runtime contract test.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = join(__dirname, '..', '..', 'pkg');
const WASM_BG = join(PKG_DIR, 'finstack_quant_wasm_bg.wasm');

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

const facade = await import('../../index.js');
const init = facade.default;
const { portfolio, analytics, core, models } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

const EXPORTED_KEYS = [
  'FactorModel',
  'InstrumentArtifactCache',
  'Portfolio',
  'PortfolioBuilder',
  'Position',
  'aggregateFullCashflows',
  'aggregateMetrics',
  'allocateWeights',
  'applyScenarioAndRevalue',
  'attributePortfolioPnl',
  'attributeScenarioPnl',
  'brinsonFachler',
  'brinsonLinked',
  'buildCreditVolReport',
  'campisiAttribution',
  'campisiCarinoLink',
  'carinoLink',
  'cellReturnsFromCurves',
  'cellReturnsFromReference',
  'collapseToBaseByDateKind',
  'computeFactorSensitivities',
  'computePnlProfiles',
  'constraintExposureLimit',
  'constraintExposureMinimum',
  'csrQuoteDelta',
  'excessReturns',
  'factorBrinsonAttribution',
  'fiAttributionResultReconciliationCheck',
  'girrQuoteDelta',
  'gridAttribution',
  'gridCarinoLink',
  'mwrXirr',
  'netInCurrencyByDate',
  'optimizePortfolio',
  'parsePortfolioSpecJson',
  'portfolioAttributionExplainText',
  'portfolioAttributionPctOfTotalByBucket',
  'portfolioAttributionReconciliationCheck',
  'portfolioFrtbSensitivities',
  'portfolioMargin',
  'portfolioMetricsGetMetric',
  'portfolioMetricsGetPositionMetrics',
  'portfolioMetricsGetTotal',
  'portfolioMetricsRequireTotal',
  'portfolioMetricsSeries',
  'portfolioOptimizationResultBindingConstraints',
  'portfolioOptimizationResultIsFeasible',
  'portfolioOptimizationResultNewPositionTrades',
  'portfolioOptimizationResultToTradeList',
  'portfolioValuationGetEntityValue',
  'portfolioValuationGetPositionValue',
  'primitiveExposures',
  'rebalanceFromSpec',
  'replayPortfolio',
  'scenarioPnl',
  'scenarioPnlBatch',
  'schema',
  'sensitivityMatrixDelta',
  'sensitivityMatrixFactorDeltas',
  'sensitivityMatrixPositionDeltas',
  'twrrLinked',
  'twrrModifiedDietz',
  'valuePortfolio',
];

/**
 * Assert a computation result is a plain structured object, not an ES2015
 * `Map` and not a JSON string.
 *
 * `JSON.stringify` is the load-bearing check: a `Map` stringifies to `{}`,
 * and its property reads silently yield `undefined`.
 */
function assertStructured(value, label) {
  assert.ok(!(value instanceof Map), `${label} must not be an ES2015 Map`);
  assert.equal(typeof value, 'object', `${label} must be an object, not a JSON string`);
  assert.notEqual(value, null, `${label} must not be null`);
  const text = JSON.stringify(value);
  assert.ok(text.length > 2, `${label} must JSON.stringify to a non-empty payload (got ${text})`);
  return value;
}

test('portfolio namespace exposes exactly the pinned contract surface', () => {
  assert.deepEqual(Object.keys(portfolio).sort(), EXPORTED_KEYS);
  for (const key of EXPORTED_KEYS) {
    // `schema` is the nested JSON Schema namespace; every other key is callable.
    const expected = key === 'schema' ? 'object' : 'function';
    assert.equal(
      typeof portfolio[key],
      expected,
      `portfolio.${key} must be a ${expected} (got ${typeof portfolio[key]})`
    );
  }
});

test('models.factor.risk.decomposeFactors consumes the canonical Python monetary matrix wire', () => {
  const sensitivities = {
    base_currency: 'EUR',
    position_ids: ['A', 'B'],
    factor_ids: ['F'],
    data: [[2.0], [3.0]],
  };
  const covariance = JSON.stringify({ factor_ids: ['F'], n: 1, data: [0.04] });
  const result = assertStructured(
    models.factor.risk.decomposeFactors(JSON.stringify(sensitivities), covariance),
    'factor decomposition'
  );
  assert.ok(Math.abs(result.total_risk - 1.0) < 1e-12);
  assert.deepEqual(
    result.position_factor_contributions.map((row) => row.position_id),
    ['A', 'B']
  );
  assert.ok(
    Math.abs(
      result.position_factor_contributions.reduce((sum, row) => sum + row.risk_contribution, 0) -
        result.total_risk
    ) < 1e-12
  );
  assert.equal(result.measure, 'variance');
  for (const measure of [
    'volatility',
    { var: { confidence: 0.99 } },
    { expected_shortfall: { confidence: 0.975 } },
  ]) {
    const measured = models.factor.risk.decomposeFactors(
      JSON.stringify(sensitivities),
      covariance,
      measure
    );
    assert.deepEqual(measured.measure, measure);
    assert.ok(Number.isFinite(measured.total_risk));
  }
});

test('tail risk factories validate confidence before decomposition', () => {
  const risk = models.factor.risk;
  const es = risk.riskMeasureExpectedShortfall(0.975);
  assert.deepEqual(es, { expected_shortfall: { confidence: 0.975 } });
  assert.deepEqual(risk.riskMeasureVar(0.99), { var: { confidence: 0.99 } });
  assert.throws(() => risk.riskMeasureVar(1.0), /confidence/);
  assert.deepEqual(risk.decomposeFactors(SENSITIVITY_WIRE, SENSITIVITY_COVARIANCE, es).measure, es);
});

test('models.factor.risk.decomposeFactors rejects malformed matrices and missing reporting currency', () => {
  const valid = { base_currency: 'USD', position_ids: ['P'], factor_ids: ['F'], data: [[2.0]] };
  const covariance = JSON.stringify({ factor_ids: ['F'], n: 1, data: [0.04] });
  for (const data of [[], [[], []], [[]], [[2.0, 3.0]], [2.0], [[null]]]) {
    assert.throws(() =>
      models.factor.risk.decomposeFactors(JSON.stringify({ ...valid, data }), covariance)
    );
  }
  for (const extra of [{ n_factors: 1 }, { unexpected: true }, { base_currency: 'BAD' }]) {
    assert.throws(() =>
      models.factor.risk.decomposeFactors(JSON.stringify({ ...valid, ...extra }), covariance)
    );
  }
  const missingCurrency = { position_ids: ['P'], factor_ids: ['F'], data: [[2.0]] };
  assert.throws(() =>
    models.factor.risk.decomposeFactors(JSON.stringify(missingCurrency), covariance)
  );
});

// Runtime arity gate. An extra argument is silently discarded at the JS
// boundary, so pinning Function.length keeps these exports at the arity their
// callers rely on; the declarations come from the same Rust signatures.
test('portfolio Campisi exports keep their declared arity', () => {
  assert.equal(portfolio.campisiAttribution.length, 3);
  assert.equal(portfolio.campisiCarinoLink.length, 1);
  assert.equal(portfolio.fiAttributionResultReconciliationCheck.length, 2);
});

// Same arity gate as above, for the credit excess-return / grid-attribution /
// factor-Brinson surfaces.
test('portfolio credit excess / grid / factor-Brinson exports keep their declared arity', () => {
  assert.equal(portfolio.cellReturnsFromReference.length, 3);
  assert.equal(portfolio.cellReturnsFromCurves.length, 6);
  assert.equal(portfolio.excessReturns.length, 2);
  assert.equal(portfolio.gridAttribution.length, 2);
  assert.equal(portfolio.gridCarinoLink.length, 1);
  assert.equal(portfolio.factorBrinsonAttribution.length, 2);
});

test('analytics.constrainedLeastSquares keeps its declared arity', () => {
  assert.equal(analytics.constrainedLeastSquares.length, 4);
});

const campisiSnapshot = (id, sector, weight, totalReturn, coupon, duration = 5) => ({
  id,
  sector,
  weight,
  total_return: totalReturn,
  opening_market_value: { amount: '80', currency: 'USD' },
  annual_coupon: { amount: coupon, currency: 'USD' },
  treasury: {
    sensitivity_basis: 'UST 5Y settlement yield',
    duration,
    opening: { basis: 'UST 5Y settlement yield', rate: 0.03 },
    closing: { basis: 'UST 5Y settlement yield', rate: 0.031 },
  },
  spread: null,
});
const CAMPISI_PORTFOLIO = [
  campisiSnapshot('P-GOVT', 'GOVT', '0.6', 0.016, '4'),
  campisiSnapshot('P-CORP', 'CORP', '0.4', 0.012, '6', 4),
];
const CAMPISI_BENCHMARK = [
  campisiSnapshot('B-GOVT', 'GOVT', '0.5', 0.015, '4'),
  campisiSnapshot('B-CORP', 'CORP', '0.5', 0.011, '5.5', 4.5),
];
const campisiConfig = (start = '2025-01-01', end = '2025-03-15', sectors = ['GOVT', 'CORP']) => ({
  start,
  end,
  sectors,
  currency: 'USD',
  basis: {
    portfolio_series_id: 'portfolio',
    benchmark_series_id: 'benchmark',
    return_policy: 'reinvested',
    valuation: 'dirty-settlement',
    fx: 'native USD',
    fees: 'net',
  },
});

const brinsonPeriod = (wpA, wbA, rpA, rbA, rpB, rbB) => [
  {
    sector: 'A',
    portfolio_weight: wpA,
    benchmark_weight: wbA,
    portfolio_return: rpA,
    benchmark_return: rbA,
  },
  {
    sector: 'B',
    portfolio_weight: 1 - wpA,
    benchmark_weight: 1 - wbA,
    portfolio_return: rpB,
    benchmark_return: rbB,
  },
];

test('portfolio.brinsonLinked links raw sector periods to the compounded active return', () => {
  assert.equal(portfolio.brinsonLinked.length, 1);
  const periods = [
    brinsonPeriod(0.7, 0.5, 0.1, 0.06, 0.04, 0.05),
    brinsonPeriod(0.6, 0.5, 0.02, 0.03, -0.01, 0.0),
  ];
  const report = assertStructured(portfolio.brinsonLinked(periods), 'brinsonLinked result');
  const active = report.portfolio_return_compounded - report.benchmark_return_compounded;
  const linked = report.linked_allocation + report.linked_selection + report.linked_interaction;
  assert.ok(Math.abs(linked - active) < 1e-12, `${linked} vs ${active}`);
  assert.equal(report.periods.length, 2);
  assert.deepEqual(
    report.linked_sectors.map((row) => row.sector),
    ['A', 'B']
  );
  // Identical to linking each period's brinsonFachler result.
  assert.deepEqual(
    JSON.parse(JSON.stringify(report)),
    JSON.parse(
      JSON.stringify(portfolio.carinoLink(periods.map((p) => portfolio.brinsonFachler(p))))
    )
  );
  assert.deepEqual(
    JSON.parse(JSON.stringify(portfolio.brinsonLinked(JSON.stringify(periods)))),
    JSON.parse(JSON.stringify(report))
  );

  const bad = brinsonPeriod(0.6, 0.5, 0.02, 0.03, -0.01, 0.0);
  bad[0].portfolio_weight = 0.9;
  assert.throws(
    () => portfolio.brinsonLinked([periods[0], bad]),
    (error) => error.kind === 'validation' && /period 1/.test(error.message)
  );
  assert.throws(
    () => portfolio.brinsonLinked([]),
    (error) => error.kind === 'validation'
  );
  assert.throws(() => portfolio.brinsonLinked([[{ ...periods[0][0], bogus: 1 }]]));
  assert.throws(() => portfolio.brinsonLinked(7), TypeError);
});

test('portfolio.campisiAttribution reconciles the five checked effects', () => {
  const result = assertStructured(
    portfolio.campisiAttribution(CAMPISI_PORTFOLIO, CAMPISI_BENCHMARK, campisiConfig()),
    'campisiAttribution result'
  );
  assert.ok(Math.abs(result.portfolio_components.carry - 0.012) < 1e-12);
  assert.ok(Math.abs(result.portfolio_components.treasury + 0.0046) < 1e-12);
  assert.ok(Math.abs(result.active_return - 0.0014) < 1e-12);
  const reconstructed =
    result.total_allocation +
    result.total_active_carry +
    result.total_active_treasury +
    result.total_active_spread +
    result.total_residual;
  assert.ok(Math.abs(reconstructed - result.active_return) < 1e-12);
  assert.deepEqual(result.source.config.sectors, ['GOVT', 'CORP']);
});

test('portfolio.campisiAttribution rejects legacy inputs and inexact net weights', () => {
  assert.throws(() => portfolio.campisiAttribution(CAMPISI_PORTFOLIO, CAMPISI_BENCHMARK, {}));
  assert.throws(() =>
    portfolio.campisiAttribution(CAMPISI_PORTFOLIO, CAMPISI_BENCHMARK, {
      ...campisiConfig(),
      period_years: 0.25,
    })
  );
  const short = structuredClone(CAMPISI_PORTFOLIO);
  short[0].weight = '0.599999999999999999999';
  assert.throws(
    () => portfolio.campisiAttribution(short, CAMPISI_BENCHMARK, campisiConfig()),
    /weights must sum to one/
  );
});

test('portfolio.campisiCarinoLink links exact periods of unequal length', () => {
  const jan = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig('2025-01-01', '2025-02-01')
  );
  const feb = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig('2025-02-01', '2025-03-01')
  );
  assert.notEqual(jan.total_active_carry, feb.total_active_carry);
  const linked = assertStructured(
    portfolio.campisiCarinoLink([jan, feb]),
    'campisiCarinoLink result'
  );
  const geometric = linked.portfolio_return_compounded - linked.benchmark_return_compounded;
  const reconstructed =
    linked.linked_allocation +
    linked.linked_active_carry +
    linked.linked_active_treasury +
    linked.linked_active_spread +
    linked.linked_residual;
  assert.ok(Math.abs(reconstructed - geometric) < 1e-10);
  assert.equal(linked.periods.length, 2);
  assert.throws(() => portfolio.campisiCarinoLink([jan, jan]), /contiguous/);
  assert.throws(() => portfolio.campisiCarinoLink([feb, jan]), /contiguous/);
  assert.throws(() => portfolio.campisiCarinoLink([CAMPISI_PORTFOLIO]));
});

test('portfolio.campisiCarinoLink rejects a tampered result independently of tolerance', () => {
  const result = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig()
  );
  const badActive = { ...result, active_return: result.active_return + 0.001 };
  const badSector = structuredClone(result);
  badSector.sectors[0].residual += 0.001;
  badSector.sectors[0].total_active += 0.001;
  for (const value of [badActive, badSector]) {
    assert.throws(() => portfolio.campisiCarinoLink([value]), /source observations/);
    assert.throws(
      () => portfolio.fiAttributionResultReconciliationCheck(value, 1),
      /source observations/
    );
  }
});

test('portfolio.campisiAttribution distinguishes zero net weight from an absent side', () => {
  const core = campisiSnapshot('CORE', 'CORE', '1', 0.015, '4.8');
  const hedge = [
    campisiSnapshot('LONG', 'HEDGE', '0.5', 0.04, '6'),
    campisiSnapshot('SHORT', 'HEDGE', '-0.5', 0.01, '2'),
  ];
  const config = campisiConfig('2025-01-01', '2025-03-15', ['HEDGE', 'CORE']);
  assert.throws(
    () => portfolio.campisiAttribution([core, ...hedge], [core], config),
    /HEDGE.*exactly zero/
  );
  const oneSided = [
    { ...core, weight: '0.8' },
    campisiSnapshot('EXTRA', 'HEDGE', '0.2', 0.021, '7'),
  ];
  const result = portfolio.campisiAttribution(oneSided, [core], config);
  assert.deepEqual(
    result.sectors.map((s) => s.sector),
    ['HEDGE', 'CORE']
  );
  assert.equal(result.sectors[0].benchmark_weight, 0);
  assert.equal(result.sectors[0].benchmark_return, null);
  assert.equal(result.sectors[0].benchmark_comparator, 0.015);
});

test('portfolio.fiAttributionResultReconciliationCheck validates its diagnostic tolerance', () => {
  const result = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig()
  );
  const report = assertStructured(
    portfolio.fiAttributionResultReconciliationCheck(result, 1e-10),
    'fiAttributionResultReconciliationCheck result'
  );
  assert.equal(report.is_reconciled, true);
  assert.equal(report.tolerance, 1e-10);
  for (const tolerance of [-1, NaN, Infinity]) {
    assert.throws(() => portfolio.fiAttributionResultReconciliationCheck(result, tolerance));
  }
  const unknown = { ...result, bogus_field: 1 };
  assert.throws(() => portfolio.fiAttributionResultReconciliationCheck(unknown, 1e-10));
  assert.throws(() => portfolio.campisiCarinoLink([unknown]));
});

// Credit excess returns, hierarchical grid attribution, and factor-Brinson
// (Dynkin, Hyman & Vankudre 1998; Carino 1999; Jeet & Partani 2023).

// Lehman Brothers Fixed Income Research (1998), Figure B-1: populated
// duration cells of May 1997 Treasury returns by duration, width 0.5y.
const LEHMAN_B1_REFERENCE = JSON.stringify(
  [
    [0.25, 0.0054],
    [0.75, 0.0059],
    [1.25, 0.0066],
    [1.75, 0.0071],
    [2.25, 0.0073],
    [2.75, 0.0075],
    [3.25, 0.0078],
    [3.75, 0.0078],
    [4.25, 0.008],
    [4.75, 0.0092],
    [5.25, 0.0097],
    [5.75, 0.0103],
    [6.25, 0.0111],
    [6.75, 0.0105],
    [7.25, 0.0105],
    [9.25, 0.011],
    [9.75, 0.0111],
    [10.25, 0.0111],
    [10.75, 0.0115],
    [11.25, 0.0118],
    [11.75, 0.0121],
    [12.25, 0.0116],
  ].map(([duration, total_return]) => ({ duration, total_return }))
);
const lehmanCellConfig = JSON.stringify({ width: 0.5 });

test('portfolio.cellReturnsFromReference + excessReturns reproduce the Lehman Figure B-2 golden', () => {
  const table = assertStructured(
    portfolio.cellReturnsFromReference(LEHMAN_B1_REFERENCE, 'UST', lehmanCellConfig),
    'cellReturnsFromReference result'
  );
  assert.equal(table.base_label, 'UST');
  assert.equal(table.cells[0].label, '0.0-0.5');

  const positions = JSON.stringify([
    { id: 'Colombia', weight: 0.2, duration: 5.16, total_return: 0.0225 },
    { id: 'RiteAid', weight: 0.2, duration: 6.71, total_return: 0.0172 },
    { id: 'NewsAM', weight: 0.2, duration: 9.58, total_return: 0.0182 },
    { id: 'Delta', weight: 0.2, duration: 9.81, total_return: 0.0102 },
    { id: 'Quebec', weight: 0.2, duration: 11.08, total_return: 0.0185 },
  ]);
  const result = assertStructured(
    portfolio.excessReturns(positions, JSON.stringify(table)),
    'excessReturns result'
  );
  assert.ok(Math.abs(result.portfolio_excess_return - 0.00648) < 1e-9);
  assert.equal(result.positions[0].cell, '5.0-5.5');
  assert.ok(
    Math.abs(
      result.portfolio_excess_return -
        (result.portfolio_total_return - result.portfolio_base_return)
    ) < 1e-9
  );
});

test('portfolio.excessReturns fails closed on a duration outside the table range, naming the position', () => {
  const tableJson = JSON.stringify(
    portfolio.cellReturnsFromReference(LEHMAN_B1_REFERENCE, 'UST', lehmanCellConfig)
  );
  const farPosition = JSON.stringify([
    { id: 'X', weight: 1.0, duration: 99.0, total_return: 0.01 },
  ]);
  assert.throws(() => portfolio.excessReturns(farPosition, tableJson), /X/);
});

test('portfolio.excessReturns validates hand-built duration cell tables', () => {
  const positions = JSON.stringify([{ id: 'P', weight: 1.0, duration: 0.75, total_return: 0.03 }]);
  const overlapping = JSON.stringify({
    base_label: 'CUSTOM',
    cells: [
      { label: 'first', lower: 0.0, upper: 1.0, base_return: 0.01, observed: true },
      { label: 'second', lower: 0.5, upper: 1.5, base_return: 0.02, observed: true },
    ],
  });
  assert.throws(() => portfolio.excessReturns(positions, overlapping), /cell\[1\].*overlaps/);
});

test('portfolio.excessReturns rejects empty and duplicate inbound cell labels', () => {
  const positions = JSON.stringify([{ id: 'P', weight: 1.0, duration: 0.25, total_return: 0.03 }]);
  const table = (labels) =>
    JSON.stringify({
      base_label: 'CUSTOM',
      cells: [
        { label: labels[0], lower: 0.0, upper: 1.0, base_return: 0.01, observed: true },
        { label: labels[1], lower: 1.0, upper: 2.0, base_return: 0.02, observed: true },
      ],
    });

  assert.throws(
    () => portfolio.excessReturns(positions, table(['', 'long'])),
    /cell\[0\].*non-empty/
  );
  assert.throws(
    () => portfolio.excessReturns(positions, table(['duplicate', 'duplicate'])),
    /cell\[1\].*unique/
  );
});

test('portfolio.cellReturnsFromReference rejects an empty reference universe', () => {
  assert.throws(() => portfolio.cellReturnsFromReference('[]', 'UST', lehmanCellConfig));
});

// FFI-level regression for the Minor-2 finding: a units mistake (e.g. days
// instead of years) producing an astronomically large `duration` must fail
// closed with a clear error, not crash the WASM module with an unrecoverable
// panic ("capacity overflow") while allocating the duration grid.
test('portfolio.cellReturnsFromReference rejects an astronomically large duration, not a panic', () => {
  const reference = JSON.stringify([{ duration: 1e30, total_return: 0.01 }]);
  assert.throws(
    () => portfolio.cellReturnsFromReference(reference, 'UST', JSON.stringify({ width: 1.0 })),
    (error) => /sanity bound/.test(String(error))
  );
});

const flatCurveKnots = (id) =>
  new core.DiscountCurve({
    id,
    baseDate: '2024-01-01',
    knots: [0.0, 1.0, 0.25, 0.99004983, 0.5, 0.98019867, 1.25, 0.95122942, 1.5, 0.94176453],
    interp: 'monotone_convex',
    extrapolation: 'extend',
    dayCount: 'act_365f',
  });

test('portfolio.cellReturnsFromCurves reproduces the flat-curve pure-carry golden', () => {
  const start = flatCurveKnots('UST');
  const end = flatCurveKnots('UST');
  const table = assertStructured(
    portfolio.cellReturnsFromCurves(start, end, 0.25, 2.0, 'UST', JSON.stringify({ width: 1.0 })),
    'cellReturnsFromCurves result'
  );
  assert.equal(table.cells.length, 2);
  for (const cell of table.cells) {
    assert.ok(Math.abs(cell.base_return - 0.01005017) < 1e-6);
    assert.equal(cell.observed, true);
  }
});

// The mutation-catching argument-order test the task brief calls for:
// distinct start/end curves (4% -> 5%, rising rates) so swapping the two
// `DiscountCurve` arguments is not a no-op, unlike the pure-carry golden
// above whose start === end.
test('portfolio.cellReturnsFromCurves distinguishes start and end curves under rising rates', () => {
  const start = new core.DiscountCurve({
    id: 'UST',
    baseDate: '2024-01-01',
    knots: [0.0, 1.0, 0.5, 0.98019867, 1.5, 0.94176453],
    interp: 'monotone_convex',
    extrapolation: 'extend',
    dayCount: 'act_365f',
  });
  const end = new core.DiscountCurve({
    id: 'UST',
    baseDate: '2024-01-01',
    knots: [0.0, 1.0, 0.25, 0.9875778, 1.25, 0.93941306],
    interp: 'monotone_convex',
    extrapolation: 'extend',
    dayCount: 'act_365f',
  });
  const table = portfolio.cellReturnsFromCurves(
    start,
    end,
    0.25,
    2.0,
    'UST',
    JSON.stringify({ width: 1.0 })
  );
  assert.ok(Math.abs(table.cells[0].base_return - 0.0075281983) < 1e-8);
  assert.ok(Math.abs(table.cells[1].base_return - -0.00249688) < 1e-8);
});

test('portfolio.cellReturnsFromCurves fails closed when a cell matures inside the holding period', () => {
  const knots = [0.0, 1.0, 0.25, 0.99004983, 0.5, 0.98019867];
  const start = new core.DiscountCurve({
    id: 'UST',
    baseDate: '2024-01-01',
    knots,
    interp: 'monotone_convex',
    extrapolation: 'extend',
    dayCount: 'act_365f',
  });
  const end = new core.DiscountCurve({
    id: 'UST',
    baseDate: '2024-01-01',
    knots,
    interp: 'monotone_convex',
    extrapolation: 'extend',
    dayCount: 'act_365f',
  });
  assert.throws(() =>
    portfolio.cellReturnsFromCurves(start, end, 0.25, 0.5, 'UST', JSON.stringify({ width: 0.25 }))
  );
});

// Hand-derived golden fixture (finstack-quant/portfolio/src/grid_attribution.rs
// `grid_attribution_matches_hand_derived_golden`): r^P=0.0293, r^B=0.0245,
// active=0.0048, curve=0.0021, sector=0.0004, selection=0.0023.
const gridPos = (cell, sector, weight, total_return) => ({ cell, sector, weight, total_return });
const GRID_PORTFOLIO = JSON.stringify([
  gridPos('0.0-3.0', 'GOVT', 0.2, 0.012),
  gridPos('0.0-3.0', 'CORP', 0.2, 0.025),
  gridPos('3.0-6.0', 'GOVT', 0.3, 0.028),
  gridPos('3.0-6.0', 'CORP', 0.3, 0.045),
]);
const GRID_BENCHMARK = JSON.stringify([
  gridPos('0.0-3.0', 'GOVT', 0.3, 0.01),
  gridPos('0.0-3.0', 'CORP', 0.2, 0.02),
  gridPos('3.0-6.0', 'GOVT', 0.25, 0.03),
  gridPos('3.0-6.0', 'CORP', 0.25, 0.04),
]);

test('portfolio.gridAttribution matches the hand-derived golden and reconciles', () => {
  const result = assertStructured(
    portfolio.gridAttribution(GRID_PORTFOLIO, GRID_BENCHMARK),
    'gridAttribution result'
  );
  const close = (a, b) => assert.ok(Math.abs(a - b) < 1e-12, `${a} vs ${b}`);
  close(result.portfolio_return, 0.0293);
  close(result.benchmark_return, 0.0245);
  close(result.active_return, 0.0048);
  close(result.total_curve, 0.0021);
  close(result.total_sector, 0.0004);
  close(result.total_selection, 0.0023);
  close(result.total_curve + result.total_sector + result.total_selection, result.active_return);
});

// Argument-order mutation gate: `portfolio`/`benchmark` are same-typed JSON
// arrays, and the golden above has portfolio_return != benchmark_return, so
// swapping them changes every total.
test('portfolio.gridAttribution is not invariant under swapping portfolio and benchmark', () => {
  const swapped = portfolio.gridAttribution(GRID_BENCHMARK, GRID_PORTFOLIO);
  assert.notEqual(swapped.active_return, 0.0048);
  assert.ok(Math.abs(swapped.active_return - -0.0048) < 1e-12);
});

test('portfolio.gridAttribution fails closed on a zero-net-weight bucket, naming cell/sector/side', () => {
  const portfolioPositions = JSON.stringify([
    gridPos('0.0-3.0', 'GOVT', 0.5, 0.01),
    gridPos('0.0-3.0', 'GOVT', -0.5, 0.02),
    gridPos('0.0-3.0', 'CORP', 1.0, 0.03),
  ]);
  const benchmarkPositions = JSON.stringify([gridPos('0.0-3.0', 'CORP', 1.0, 0.025)]);
  assert.throws(
    () => portfolio.gridAttribution(portfolioPositions, benchmarkPositions),
    (error) =>
      /0\.0-3\.0/.test(String(error)) &&
      /GOVT/.test(String(error)) &&
      /Portfolio/.test(String(error))
  );
});

// FFI-level regression for the whole-branch review's Important-1 finding: an
// exact-equality-only guard would let a benchmark cell netting to `eps`
// (not `0.0`) through, and `weighted_return / weight` would then blow up
// without bound as `eps` shrinks. Re-pins the Rust
// `near_zero_net_weight_bucket_fails_closed_before_rate_blows_up` fixture
// through the WASM facade: `eps = 1e-8` (ratio ~1e-8, far below the 1e-6
// relative bound) must be rejected, naming the cell and the side.
test('portfolio.gridAttribution fails closed on a near-zero (not just exact-zero) net weight', () => {
  const eps = 1e-8;
  const portfolioPositions = JSON.stringify([
    gridPos('X', 'GOVT', 0.5, 0.018),
    gridPos('Y', 'GOVT', 0.5, 0.02),
  ]);
  const benchmarkPositions = JSON.stringify([
    gridPos('X', 'GOVT', 0.5, 0.02),
    gridPos('X', 'CORP', -(0.5 - eps), 0.01),
    gridPos('Y', 'GOVT', 1.0 - eps, 0.015),
  ]);
  assert.throws(
    () => portfolio.gridAttribution(portfolioPositions, benchmarkPositions),
    (error) => /bucket 'X'/.test(String(error)) && /Benchmark/.test(String(error))
  );
});

test('portfolio.gridCarinoLink reconstructs the geometrically compounded active return over two periods', () => {
  const period = portfolio.gridAttribution(GRID_PORTFOLIO, GRID_BENCHMARK);
  const linked = assertStructured(
    portfolio.gridCarinoLink(JSON.stringify([period, period])),
    'gridCarinoLink result'
  );
  const close = (a, b, tol) => assert.ok(Math.abs(a - b) < tol, `${a} vs ${b}`);
  close(linked.portfolio_return_compounded, 0.05945849, 1e-8);
  close(linked.benchmark_return_compounded, 0.04960025, 1e-8);
  const sum = linked.linked_curve + linked.linked_sector + linked.linked_selection;
  close(sum, linked.portfolio_return_compounded - linked.benchmark_return_compounded, 1e-12);
  assert.equal(linked.periods.length, 2);
});

test('portfolio.gridCarinoLink rejects an empty period array', () => {
  assert.throws(() => portfolio.gridCarinoLink('[]'), /at least one period/);
});

test('portfolio.gridCarinoLink rejects inconsistent result contracts', () => {
  const period = portfolio.gridAttribution(GRID_PORTFOLIO, GRID_BENCHMARK);

  const badActive = { ...period, active_return: period.active_return + 0.001 };
  assert.throws(() => portfolio.gridCarinoLink(JSON.stringify([badActive])), /active_return/);

  const badTotal = { ...period, total_selection: period.total_selection + 0.001 };
  assert.throws(() => portfolio.gridCarinoLink(JSON.stringify([badTotal])), /effect totals/);
});

// Jeet & Partani (2023) Exhibits 1-2, binary sector-indicator exposures:
// A -> Healthcare, B -> Energy, C -> Healthcare.
const FACTOR_BRINSON_INPUT = JSON.stringify({
  asset_ids: ['A', 'B', 'C'],
  asset_returns: [0.05, 0.02, 0.01],
  exposures: [0.0, 1.0, 1.0, 0.0, 0.0, 1.0],
  factor_names: ['Energy', 'Healthcare'],
  portfolio_weights: [1.25, -0.3, 0.05],
  benchmark_weights: [0.6, 0.3, 0.1],
});
const FACTOR_BRINSON_F_B = [0.02, 0.31 / 7.0];

test('portfolio.factorBrinsonAttribution matches the binary Jeet-Partani golden', () => {
  const result = assertStructured(
    portfolio.factorBrinsonAttribution(FACTOR_BRINSON_INPUT, FACTOR_BRINSON_F_B),
    'factorBrinsonAttribution result'
  );
  const close = (a, b) => assert.ok(Math.abs(a - b) < 1e-12, `${a} vs ${b}`);
  close(result.active_return, 0.02);
  close(result.allocation, 0.0145714285714286);
  close(result.selection, 0.0054285714285714);
  close(result.allocation + result.selection, result.active_return);
});

test('portfolio.factorBrinsonAttribution fails closed when factor_returns do not explain the benchmark', () => {
  assert.throws(
    () => portfolio.factorBrinsonAttribution(FACTOR_BRINSON_INPUT, [0.05, 0.01]),
    /constrained_least_squares/
  );
});

// Jeet & Partani (2023) Exhibit 1 hand-derivation:
// f = (0.02 + 0.3*lambda, 0.03 + 0.35*lambda) = (0.0289552239, 0.0404477612).
test('analytics.constrainedLeastSquares matches the binary hand-derived golden', () => {
  const f = analytics.constrainedLeastSquares(
    [0.0, 1.0, 1.0, 0.0, 0.0, 1.0],
    2,
    [0.05, 0.02, 0.01],
    [0.6, 0.3, 0.1]
  );
  assert.ok(f instanceof Float64Array);
  assert.equal(f.length, 2);
  assert.ok(Math.abs(f[0] - 0.0289552239) < 1e-9);
  assert.ok(Math.abs(f[1] - 0.0404477612) < 1e-9);
});

test('analytics.constrainedLeastSquares fails closed on orthogonal weights', () => {
  assert.throws(
    () => analytics.constrainedLeastSquares([1.0, -1.0], 1, [0.02, 0.01], [0.5, 0.5]),
    /constraint/i
  );
});

test('analytics.constrainedLeastSquares returns OLS when the constraint already holds', () => {
  const f = analytics.constrainedLeastSquares([1.0, -1.0], 1, [0.02, -0.02], [0.5, 0.5]);
  assert.ok(Math.abs(f[0] - 0.02) < 1e-15);
});

test('analytics.constrainedLeastSquares rejects fractional nFactors before conversion', () => {
  assert.throws(
    () => analytics.constrainedLeastSquares([1.0, -1.0], 1.5, [0.02, -0.02], [0.5, 0.5]),
    /^TypeError: nFactors: expected a non-negative whole number, got 1\.5$/
  );
});

test('analytics.constrainedLeastSquares rejects truncating nFactors before conversion', () => {
  assert.throws(
    () => analytics.constrainedLeastSquares([1.0, -1.0], 2 ** 32 + 1, [0.02, -0.02], [0.5, 0.5]),
    /^TypeError: nFactors: 4294967297 is out of range$/
  );
});

test('analytics.constrainedLeastSquares accepts Float64Array inputs, matching the plain-array golden', () => {
  const f = analytics.constrainedLeastSquares(
    Float64Array.from([0.0, 1.0, 1.0, 0.0, 0.0, 1.0]),
    2,
    Float64Array.from([0.05, 0.02, 0.01]),
    Float64Array.from([0.6, 0.3, 0.1])
  );
  assert.ok(Math.abs(f[0] - 0.0289552239) < 1e-9);
  assert.ok(Math.abs(f[1] - 0.0404477612) < 1e-9);
});

// End-to-end workflow: fit f_b with `analytics.constrainedLeastSquares`
// against benchmark weights, then feed it into
// `portfolio.factorBrinsonAttribution` and confirm the completeness
// condition holds (no rejection) on the continuous (non-binary) exposures
// fixture — the workflow the brief's own doc comments describe.
test('constrainedLeastSquares output satisfies factorBrinsonAttribution completeness', () => {
  const exposures = [1.2, -0.8, 0.5, 1.2, -0.7, 0.7];
  const returns = [0.05, 0.02, 0.01];
  const benchmarkWeights = [0.6, 0.3, 0.1];
  const fB = analytics.constrainedLeastSquares(exposures, 2, returns, benchmarkWeights);

  const input = JSON.stringify({
    asset_ids: ['A', 'B', 'C'],
    asset_returns: returns,
    exposures,
    factor_names: ['Energy', 'Healthcare'],
    portfolio_weights: [1.25, -0.3, 0.05],
    benchmark_weights: benchmarkWeights,
  });
  const result = portfolio.factorBrinsonAttribution(input, fB);
  assert.ok(Math.abs(result.allocation - 0.00977) < 1e-4);
  assert.ok(Math.abs(result.selection - 0.010231) < 1e-4);
  assert.ok(Math.abs(result.allocation + result.selection - result.active_return) < 1e-12);
});

// Structured-return contract for the remaining portfolio computation results.
//
// Every export below used to hand back a JSON string. The assertions read a
// property directly (an ES2015 Map would yield `undefined`) and round-trip the
// value through `JSON.stringify` (a Map renders as `{}`).

const EMPTY_SPEC = JSON.stringify({
  id: 'facade_portfolio',
  name: 'Facade',
  base_currency: 'USD',
  as_of: '2024-01-15',
  entities: {},
  positions: [],
});
// An empty `MarketContext`. `MarketContextState` denies unknown fields and
// requires every key except `fx`, so the payload has to be spelled out in full.
const EMPTY_MARKET = JSON.stringify({
  schema_version: 1,
  curves: [],
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

test('portfolio spec wire surfaces keep returning canonical JSON strings', () => {
  const canonical = portfolio.parsePortfolioSpecJson(EMPTY_SPEC);
  assert.equal(typeof canonical, 'string', 'parsePortfolioSpecJson is a wire surface');
  assert.equal(JSON.parse(canonical).id, 'facade_portfolio');

  const rebuilt = portfolio.Portfolio.fromSpec(canonical).toJson();
  assert.equal(typeof rebuilt, 'string', 'Portfolio.fromSpec(...).toJson() is a wire surface');
  assert.equal(JSON.parse(rebuilt).id, 'facade_portfolio');

  const handle = portfolio.Portfolio.fromSpec(canonical);
  assert.equal(typeof handle.toJson(), 'string', 'Portfolio.toJson is a wire surface');
  handle.free();
});

test('portfolio.valuePortfolio returns a structured valuation', () => {
  const valuation = assertStructured(
    portfolio.valuePortfolio(
      portfolio.Portfolio.fromSpec(EMPTY_SPEC),
      portfolioCore.MarketContext.fromJson(EMPTY_MARKET),
      false
    ),
    'valuePortfolio result'
  );
  // Direct property reads: an ES2015 Map would yield `undefined` for all of
  // these, and `position_values` would not survive JSON.stringify.
  assert.equal(valuation.as_of, '2024-01-15');
  assert.deepEqual(valuation.position_values, {});
  assert.equal(valuation.total_base_currency.currency, 'USD');
  assert.equal(typeof valuation.fx_collapse_policy, 'string');
});

test('portfolio.aggregateFullCashflows returns a structured ladder', () => {
  const ladder = assertStructured(
    portfolio.aggregateFullCashflows(
      portfolio.Portfolio.fromSpec(EMPTY_SPEC),
      portfolioCore.MarketContext.fromJson(EMPTY_MARKET)
    ),
    'aggregateFullCashflows result'
  );
  assert.deepEqual(ladder.events, []);
  assert.deepEqual(ladder.issues, []);

  const handle = portfolio.Portfolio.fromSpec(EMPTY_SPEC);
  const built = assertStructured(
    portfolio.aggregateFullCashflows(handle, portfolioCore.MarketContext.fromJson(EMPTY_MARKET)),
    'aggregateFullCashflows result'
  );
  assert.deepEqual(built, ladder);
  handle.free();
});

test('portfolio.aggregateMetrics returns a structured metrics object', () => {
  const valuation = portfolio.valuePortfolio(
    portfolio.Portfolio.fromSpec(EMPTY_SPEC),
    portfolioCore.MarketContext.fromJson(EMPTY_MARKET),
    false
  );
  const metrics = assertStructured(
    portfolio.aggregateMetrics(
      JSON.stringify(valuation),
      portfolioCore.MarketContext.fromJson(EMPTY_MARKET)
    ),
    'aggregateMetrics result'
  );
  assert.equal(typeof metrics, 'object');
});

test('portfolio.replayPortfolio returns a structured replay result', () => {
  const market = JSON.parse(EMPTY_MARKET);
  const snapshots = JSON.stringify([
    { date: '2024-01-15', market },
    { date: '2024-01-16', market },
  ]);
  const config = JSON.stringify({ mode: 'pv_only', attribution_method: 'parallel' });
  const replay = assertStructured(
    portfolio.replayPortfolio(portfolio.Portfolio.fromSpec(EMPTY_SPEC), snapshots, config),
    'replayPortfolio result'
  );
  assert.equal(replay.steps.length, 2);
});

test('portfolio.brinsonFachler and carinoLink return structured attributions', () => {
  const sectors = JSON.stringify([
    {
      sector: 'A',
      portfolio_weight: 0.6,
      benchmark_weight: 0.4,
      portfolio_return: 0.08,
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
  const single = assertStructured(portfolio.brinsonFachler(sectors), 'brinsonFachler result');
  const reconstructed = single.total_allocation + single.total_selection + single.total_interaction;
  assert.ok(Math.abs(reconstructed - single.total_excess_return) < 1e-12);

  // carinoLink binds Rust `carino_link`: it links precomputed period results
  // (the brinsonFachler output), like campisiCarinoLink and gridCarinoLink.
  const linked = assertStructured(
    portfolio.carinoLink(JSON.stringify([single, single])),
    'carinoLink result'
  );
  const geometric = linked.portfolio_return_compounded - linked.benchmark_return_compounded;
  const sum = linked.linked_allocation + linked.linked_selection + linked.linked_interaction;
  assert.ok(Math.abs(sum - geometric) < 1e-10);
  assert.throws(
    () => portfolio.carinoLink(JSON.stringify([JSON.parse(sectors), JSON.parse(sectors)])),
    (error) => error instanceof Error && error.kind === 'validation'
  );
});

test('portfolio.twrrModifiedDietz treats omitted cashflows as none and rejects unknown keys', () => {
  const bare = { beginning_market_value: 100.0, ending_market_value: 110.0 };
  assert.ok(Math.abs(portfolio.twrrModifiedDietz(JSON.stringify(bare)) - 0.1) < 1e-15);
  assert.equal(
    portfolio.twrrModifiedDietz(JSON.stringify({ ...bare, cashflows: [] })),
    portfolio.twrrModifiedDietz(JSON.stringify(bare))
  );
  assert.throws(
    () => portfolio.twrrModifiedDietz(JSON.stringify({ ...bare, bogus: 1 })),
    (error) => error.kind === 'validation' && /unknown field `bogus`/.test(error.message)
  );
});

test('portfolio.twrrLinked returns a structured linked return', () => {
  const linked = assertStructured(
    portfolio.twrrLinked(JSON.stringify([0.05, 0.03]), 1.0),
    'twrrLinked result'
  );
  assert.ok(Math.abs(linked.cumulative - 0.0815) < 1e-12);
  assert.equal(linked.is_annualised, true);
  assert.equal(linked.num_periods, 2);
  assert.equal(portfolio.twrrLinked([0.05, 0.03], 0.5).is_annualised, false);
});

test('portfolio.twrrLinked rejects a non-finite horizon', () => {
  for (const horizon of [Number.NaN, Infinity, -Infinity])
    assert.throws(
      () => portfolio.twrrLinked([0.05, 0.03], horizon),
      (error) =>
        error.name === 'FinstackError' &&
        error.kind === 'validation' &&
        error.message.includes('horizon_years'),
      String(horizon)
    );
});

// Position ids / weights / covariance are all shared by the three
// decomposition entry points. The key sets and values below are asserted
// identically by finstack-quant-py/tests/test_factor_risk_wire.py: both hosts
// return the canonical Rust result types.
const VAR_IDS = ['A', 'B'];
const VAR_WEIGHTS = [0.6, 0.4];
const VAR_COVARIANCE = [
  [0.04, 0.01],
  [0.01, 0.09],
];
const DECOMPOSITION_KEYS = [
  'confidence',
  'es_contributions',
  'euler_residual',
  'method',
  'n_positions',
  'portfolio_es',
  'portfolio_var',
  'var_contributions',
];
const VAR_ROW_KEYS = [
  'component_var',
  'incremental_var',
  'marginal_var',
  'position_id',
  'relative_var',
];
const ES_ROW_KEYS = ['component_es', 'marginal_es', 'position_id', 'relative_es'];
// Parametric 95% VaR of the book above (losses negative), pinned in both hosts.
const PARAMETRIC_VAR_95 = -0.30150665010511113;

const parametric = (extra = {}, source = {}) =>
  models.factor.risk.positionRiskReport({
    position_ids: VAR_IDS,
    source: { kind: 'parametric', weights: VAR_WEIGHTS, covariance: VAR_COVARIANCE, ...source },
    ...extra,
  });

test('models.factor.risk.positionRiskReport returns the canonical parametric PositionRiskDecomposition', () => {
  const report = assertStructured(parametric(), 'positionRiskReport result');
  assert.deepEqual(Object.keys(report).sort(), ['budget', 'decomposition', 'stress']);
  assert.equal(report.budget, null);
  assert.equal(report.stress, null);
  const { decomposition } = report;
  assert.deepEqual(Object.keys(decomposition).sort(), DECOMPOSITION_KEYS);
  assert.deepEqual(Object.keys(decomposition.var_contributions[0]).sort(), VAR_ROW_KEYS);
  assert.deepEqual(Object.keys(decomposition.es_contributions[0]).sort(), ES_ROW_KEYS);
  // Omitted confidence resolves to the Rust `parametric_95()` preset.
  assert.equal(decomposition.confidence, 0.95);
  assert.equal(decomposition.method, 'parametric');
  assert.equal(decomposition.n_positions, 2);
  assert.ok(Math.abs(decomposition.portfolio_var - PARAMETRIC_VAR_95) < 1e-12);
  assert.equal(decomposition.var_contributions[0].position_id, 'A');
  assert.equal(decomposition.var_contributions[0].incremental_var, null);
  const shares = decomposition.var_contributions.map((row) => row.relative_var);
  assert.ok(Math.abs(shares[0] + shares[1] - 1) < 1e-12);

  const explicit = parametric(
    { confidence: 0.99, budget: { targets: [0.5, 0.5] } },
    { compute_incremental: true }
  );
  assert.equal(explicit.decomposition.confidence, 0.99);
  assert.equal(typeof explicit.decomposition.var_contributions[0].incremental_var, 'number');
  assert.equal(explicit.budget.positions.length, 2);
});

test('models.factor.risk.positionRiskReport returns historical ES rows and stress and takes position-major P&L only', () => {
  // (1 - confidence) * n_scenarios must be at least 1 to resolve the tail, so
  // 0.9 confidence needs >= 10 scenarios per position.
  const scenarios = 20;
  const pnls = [
    Array.from({ length: scenarios }, (_, i) => i - scenarios / 2),
    Array.from({ length: scenarios }, (_, i) => (i - scenarios / 2) / 2),
  ];
  const historical = (rows, confidence) =>
    models.factor.risk.positionRiskReport({
      position_ids: VAR_IDS,
      confidence,
      source: { kind: 'historical', pnls: rows },
    });
  const report = assertStructured(historical(pnls, 0.9), 'positionRiskReport result');
  const { decomposition } = report;
  assert.deepEqual(Object.keys(decomposition).sort(), DECOMPOSITION_KEYS);
  assert.equal(decomposition.method, 'historical');
  assert.equal(decomposition.es_contributions.length, 2);
  assert.equal(decomposition.euler_residual, null);
  assert.deepEqual(report.stress, models.factor.risk.buildStressAttribution(VAR_IDS, pnls, 0.9));
  // Omitted confidence resolves to the Rust `historical_95()` preset (two
  // tail scenarios need at least 40 at 95%).
  const longer = [
    Array.from({ length: 40 }, (_, i) => i - 20),
    Array.from({ length: 40 }, (_, i) => (i - 20) / 2),
  ];
  assert.equal(historical(longer).decomposition.confidence, 0.95);
  // A scenario-major (20 x 2) matrix is rejected in both hosts.
  const scenarioMajor = pnls[0].map((value, i) => [value, pnls[1][i]]);
  assert.throws(
    () => historical(scenarioMajor, 0.9),
    (error) => error.kind === 'validation' && /must have 2 rows, got 20/.test(error.message)
  );
});

test('models.factor.risk.evaluateRiskBudget returns the canonical RiskBudgetResult', () => {
  const budget = assertStructured(
    models.factor.risk.evaluateRiskBudget(VAR_IDS, [60.0, 40.0], [0.5, 0.5], 100.0, 1.1),
    'evaluateRiskBudget result'
  );
  assert.deepEqual(Object.keys(budget).sort(), ['has_breach', 'positions', 'total_overbudget']);
  assert.deepEqual(Object.keys(budget.positions[0]).sort(), [
    'actual_component_var',
    'excess',
    'position_id',
    'target_component_var',
    'utilization',
  ]);
  assert.equal(budget.positions[0].position_id, 'A');
  assert.equal(budget.has_breach, true);
  assert.ok(Math.abs(budget.total_overbudget - 10.0) < 1e-12);
});

test('a position risk request rejects an unknown source kind and field', () => {
  assert.throws(
    () => parametric({}, { kind: 'monte_carlo' }),
    (error) => error.kind === 'validation' && /unknown variant/.test(error.message)
  );
  assert.throws(
    () => parametric({ horizon: 10 }),
    (error) => error.kind === 'validation' && /unknown field/.test(error.message)
  );
});

test('factor-risk kernels are absent from the portfolio namespace', () => {
  assert.equal('positionRiskReport' in portfolio, false);
  assert.equal('evaluateRiskBudget' in portfolio, false);
});

// Cross-host golden, asserted identically by
// finstack-quant-py/tests/test_portfolio_sensitivity_wire.py: the canonical
// sensitivity-matrix wire object, a 2x2 covariance and the resulting risk.
const SENSITIVITY_WIRE = {
  base_currency: 'USD',
  position_ids: ['A', 'B'],
  factor_ids: ['F1', 'F2'],
  data: [
    [1.0, 2.0],
    [3.0, -1.0],
  ],
};
const SENSITIVITY_COVARIANCE = JSON.stringify({
  factor_ids: ['F1', 'F2'],
  n: 2,
  data: [0.04, 0.01, 0.01, 0.09],
});

test('models.factor.risk.decomposeFactors reads the canonical wire and returns the Rust result', () => {
  const variance = assertStructured(
    models.factor.risk.decomposeFactors(JSON.stringify(SENSITIVITY_WIRE), SENSITIVITY_COVARIANCE),
    'decomposeFactors result'
  );
  // exposures e = [4, 1]; e' S e = 16*0.04 + 2*4*0.01 + 0.09 = 0.81
  assert.ok(Math.abs(variance.total_risk - 0.81) < 1e-12);
  assert.equal(variance.measure, 'variance');
  assert.equal(variance.position_factor_contributions.length, 4);
  assert.deepEqual(variance.position_residual_contributions, []);

  // A plain object is accepted too, and the measure keeps its serde form.
  for (const [measure, expected] of [
    [{ var: { confidence: 0.99 } }, { var: { confidence: 0.99 } }],
    [{ expected_shortfall: { confidence: 0.975 } }, { expected_shortfall: { confidence: 0.975 } }],
    ['volatility', 'volatility'],
  ]) {
    const decomposition = models.factor.risk.decomposeFactors(
      SENSITIVITY_WIRE,
      SENSITIVITY_COVARIANCE,
      measure
    );
    assert.deepEqual(decomposition.measure, expected);
  }

  // A unit variant is its bare wire label, as the declared type says.
  assert.equal(
    models.factor.risk.decomposeFactors(SENSITIVITY_WIRE, SENSITIVITY_COVARIANCE, 'volatility')
      .measure,
    'volatility'
  );
  assert.throws(
    () => models.factor.risk.decomposeFactors(SENSITIVITY_WIRE, SENSITIVITY_COVARIANCE, 'nope'),
    (error) => error.kind === 'validation' && /riskMeasure: unknown variant/.test(error.message)
  );
});

test('models.factor.risk.decomposeFactors rejects malformed sensitivity wire with a validation error', () => {
  const isValidation = (pattern) => (error) =>
    error instanceof Error && error.kind === 'validation' && pattern.test(error.message);
  for (const [wire, pattern] of [
    [{ ...SENSITIVITY_WIRE, data: [[1.0, 2.0]] }, /1 row\(s\) but position_ids declares 2/],
    [{ ...SENSITIVITY_WIRE, data: [[1.0, 2.0], [3.0]] }, /row 1 has 1 element\(s\)/],
    [{ ...SENSITIVITY_WIRE, n_factors: 2 }, /unknown field `n_factors`/],
    [{ ...SENSITIVITY_WIRE, base_currency: 'NOT_A_CCY' }, /NOT_A_CCY/],
    [{ ...SENSITIVITY_WIRE, data: [1.0, 2.0, 3.0, -1.0] }, /invalid type/],
  ]) {
    assert.throws(
      () => models.factor.risk.decomposeFactors(JSON.stringify(wire), SENSITIVITY_COVARIANCE),
      isValidation(pattern)
    );
  }
});

test('portfolio sensitivity outputs report in the portfolio base currency', () => {
  const book = portfolio.Portfolio.fromSpec(
    JSON.stringify({ ...JSON.parse(EMPTY_SPEC), base_currency: 'EUR' })
  );
  const market = portfolioCore.MarketContext.fromJson(EMPTY_MARKET);
  const result = portfolio.computeFactorSensitivities(book, [], market, '2024-01-15');
  assert.equal(result.base_currency, 'EUR');
  assert.deepEqual(result.position_ids, []);
  assert.deepEqual(result.data, []);
  assert.deepEqual(portfolio.computePnlProfiles(book, '[]', market, '2024-01-15'), []);
  const creditHierarchy = [
    {
      id: 'credit::generic',
      factor_type: 'credit',
      market_mapping: { curve_parallel: { curve_ids: [], units: 'rate_bp' } },
    },
  ];
  for (const call of [portfolio.computeFactorSensitivities, portfolio.computePnlProfiles]) {
    assert.throws(
      () => call(book, creditHierarchy, market, '2024-01-15'),
      (error) => error.kind === 'validation' && /credit::generic/.test(error.message)
    );
  }
  assert.throws(
    () => portfolio.computePnlProfiles(book, [], market, '2024-01-15', undefined, 1.5),
    (error) => error.kind === 'invalid_type' && /nPoints/.test(error.message)
  );
});

test('portfolio.portfolioMargin returns a plain PortfolioMarginResult', () => {
  const book = portfolio.Portfolio.fromSpec(EMPTY_SPEC);
  const market = portfolioCore.MarketContext.fromJson(EMPTY_MARKET);
  try {
    const result = portfolio.portfolioMargin(book, market, '2025-01-15');
    assert.equal(result.as_of, '2025-01-15');
    assert.equal(result.base_currency, 'USD');
    assert.deepEqual(result.netting_sets, []);
    assert.deepEqual(result.by_csa, {});
    assert.equal(result.total_positions, 0);
    assert.equal(result.total_initial_margin.currency, 'USD');
    assert.equal(Number(result.total_initial_margin.amount), 0);
    // A repeated run is identical; an explicit empty balance map is the same as none.
    assert.deepEqual(portfolio.portfolioMargin(book, market, '2025-01-15', {}), result);
    assert.deepEqual(portfolio.portfolioMargin(book, market, '2025-01-15', null), result);
    // Collateral for a CSA the portfolio does not have is rejected, and the
    // date is type-checked rather than coerced.
    assert.throws(
      () =>
        portfolio.portfolioMargin(book, market, '2025-01-15', {
          'NO-SUCH-CSA': { amount: '1', currency: 'USD' },
        }),
      (error) => error.name === 'FinstackError' && error.kind === 'validation'
    );
    assert.throws(
      () => portfolio.portfolioMargin(book, market, 20250115),
      (error) => error instanceof TypeError && error.kind === 'invalid_type'
    );
  } finally {
    market.free();
    book.free();
  }
});

test('typed positions round-trip and append to a copied portfolio', () => {
  const deposit = facade.valuations.instruments.Deposit.example();
  const position = new portfolio.Position('DEP-LOT', deposit.toJson(), -2, null, {
    notional: 'USD',
  });
  const loaded = portfolio.Position.fromJson(position.toJson());
  const empty = portfolio.Portfolio.builder('book', 'USD', '2025-01-01')
    .tag('desk', 'rates')
    .build();
  const built = empty.withPosition(loaded);
  assert.deepEqual(empty.positions, []);
  const [holding] = built.positions;
  assert.equal(holding.positionId, 'DEP-LOT');
  assert.equal(holding.quantity, -2);
  assert.deepEqual(holding.unit, { notional: 'USD' });
  assert.throws(() => built.withPosition(loaded), /Duplicate position ID/i);
  const extra = new portfolio.Position('DEP-LOT-2', deposit.toJson(), 1, null, 'units');
  const expanded = built.toBuilder().addPosition(extra).build();
  assert.deepEqual(expanded.positionIds, ['DEP-LOT', 'DEP-LOT-2']);
  for (const handle of [expanded, extra, holding, built, empty, loaded, position, deposit])
    handle.free();
});

test('typed inflation-linked bond edits preserve decimal coupons and the source', () => {
  const Type = facade.valuations.instruments.InflationLinkedBond;
  const source = Type.example();
  const coupon = '0.025000000000000000000000001';
  const edited = source.toBuilder().id('TIPS-EDITED').realCoupon(coupon).lag('90D').build();
  const restored = Type.fromJson(edited.toJson());
  try {
    assert.equal(restored.id, 'TIPS-EDITED');
    assert.equal(restored.realCoupon, coupon);
    assert.equal(restored.lag, '90D');
    assert.equal(source.lag, '3M');
    assert.equal(restored.toJson(), edited.toJson());
  } finally {
    source.free();
    edited.free();
    restored.free();
  }
});

test('typed bond futures distinguish live and final settlement marks', () => {
  const instruments = facade.valuations.instruments;
  const initial = instruments.BondFuture.example();
  const terms = instruments.listedFutureTermsWithMarks(
    instruments.listedFutureTermsWithContracts(initial.terms, 2),
    112.75,
    112.5
  );
  const edited = initial.toBuilder().terms(terms).build();
  const market = new core.MarketContext();
  try {
    assert.equal(Number(edited.price(market.toJson(), '2025-09-01').value.amount), 1000);
    assert.equal(Number(edited.price(market.toJson(), '2025-09-23').value.amount), 500);
    assert.equal(initial.terms.contracts, 10);
    assert.equal(edited.terms.contracts, 2);
  } finally {
    market.free();
    edited.free();
    initial.free();
  }
});

test('typed listed option terms preserve contract economics and validate strike inputs', () => {
  const instruments = facade.valuations.instruments;
  const terms = instruments.futureOptionTermsExample();
  assert.deepEqual(instruments.futureOptionTerms(terms), terms);
  assert.throws(() => instruments.futureOptionTerms({ ...terms, strike: -1.0 }), /strike/);
  const initial = instruments.EquityFutureOption.example();
  const edited = initial.toBuilder().id('EQUITY-OPTION-REPRICE').terms(terms).build();
  try {
    assert.notEqual(initial.id, edited.id);
    assert.deepEqual(edited.terms, terms);
  } finally {
    edited.free();
    initial.free();
  }
});

test('ordinary GIRR uses exact tenors and preserves the signed quote change under WASM', () => {
  const date = '2025-01-02';
  const id = 'GBP-SONIA';
  const vertices = ['0.25Y', '0.5Y', '1Y', '2Y', '3Y', '5Y', '10Y', '15Y', '20Y', '30Y'];
  const quotes = vertices.map((vertex, index) => ({
    kind: 'rate_quote',
    type: 'deposit',
    id: vertex,
    index: 'GBP-SONIA-OIS',
    pillar: {
      tenor: {
        count: index < 2 ? 3 * (index + 1) : Number.parseInt(vertex, 10),
        unit: index < 2 ? 'months' : 'years',
      },
    },
    rate: 0.057,
  }));
  const calibration = facade.calibration.calibrate({
    schema: 'finstack_quant.calibration/1',
    plan: {
      id: 'gbp',
      quote_sets: { gbp: vertices },
      steps: [
        {
          id,
          quote_set: 'gbp',
          kind: 'discount',
          curve_id: id,
          currency: 'GBP',
          base_date: date,
          method: 'bootstrap',
          interpolation: 'log_linear',
          extrapolation: 'extend',
          conventions: { curve_day_count: 'act_365f' },
        },
      ],
      settings: {},
    },
    market_data: quotes,
  });
  const market = core.MarketContext.fromJson(calibration.result.final_market);
  const curve = market.getDiscount(id);
  const currency = new core.Currency('GBP');
  const principal = new core.Money(100_000, currency);
  const bond = facade.valuations.instruments.Bond.zeroCoupon(
    'ZC',
    principal,
    date,
    '2026-01-02',
    { periodic: 1 },
    id
  );
  const position = new portfolio.Position('SHORT', bond.toJson(), -2);
  const builder = portfolio.Portfolio.builder('book', 'GBP', date);
  const book = builder.addPosition(position).build();
  const pricing = new core.FinstackConfig();

  try {
    const targets = curve.rateCalibration.execution.bindings.map(({ quote_id, pillar_date }) => ({
      risk_currency: 'GBP',
      risk_curve_id: id,
      vertex: quote_id,
      source: { kind: 'discount', curve_id: id },
      quote: { quote_id, pillar_date },
    }));
    const result = portfolio.girrQuoteDelta(book, market, pricing, targets);
    const year = result.quote_risk.find((row) => row.target.vertex === '1Y');
    const expected = -200_000 * (1 / 1.0571 - 1 / 1.057);
    assert.equal(result.as_of, date);
    assert.equal(result.quote_risk.length, vertices.length);
    assert.equal(year.positions[0].position_scale, -2);
    assert.ok(Math.abs(year.positions[0].pnl_base_per_pp - 100 * expected) < 2);
    const risk = facade.margin.FrtbSensitivities.fromJson(result.sensitivities);
    try {
      assert.ok(facade.margin.frtbSbaCharge(risk).total > 0);
    } finally {
      risk.free();
    }
    assert.throws(
      () => portfolio.girrQuoteDelta(book, market, pricing, targets.slice(1)),
      /Missing GIRR delta vertex/
    );
  } finally {
    for (const handle of [pricing, book, position, bond, principal, currency, curve, market])
      handle.free();
  }
});

test('ordinary CSR uses the executed CDS quote and signed book position', () => {
  const asOf = '2024-03-20';
  const discountId = 'USD-OIS';
  const creditId = 'CORP-HAZARD';
  const vertices = ['0.5Y', '1Y', '3Y', '5Y', '10Y'];
  const rateTenors = {
    '0.25Y': [3, 'months'],
    '0.5Y': [6, 'months'],
    '1Y': [1, 'years'],
    '2Y': [2, 'years'],
    '3Y': [3, 'years'],
    '5Y': [5, 'years'],
    '10Y': [10, 'years'],
    '15Y': [15, 'years'],
    '20Y': [20, 'years'],
    '30Y': [30, 'years'],
  };
  const rateQuotes = Object.entries(rateTenors).map(([vertex, [count, unit]]) => ({
    kind: 'rate_quote',
    type: 'deposit',
    id: `usd-${vertex}`,
    index: 'USD-SOFR-OIS',
    pillar: { tenor: { count, unit } },
    rate: 0.04,
  }));

  const fit = (bump5y) => {
    const quotes = vertices.map((vertex, index) => ({
      kind: 'cds_quote',
      type: 'cds_par_spread',
      id: vertex,
      entity: 'CORP',
      convention: { currency: 'USD', doc_clause: 'isda_na' },
      pillar: {
        tenor: {
          count: index === 0 ? 6 : Number.parseInt(vertex, 10),
          unit: index === 0 ? 'months' : 'years',
        },
      },
      spread_bp: [80, 85, 95, 105 + bump5y, 115][index],
      recovery_rate: 0.4,
    }));
    const calibration = facade.calibration.calibrate({
      schema: 'finstack_quant.calibration/1',
      plan: {
        id: 'usd-credit',
        quote_sets: { rates: rateQuotes.map(({ id }) => id), credit: vertices },
        steps: [
          {
            id: discountId,
            quote_set: 'rates',
            kind: 'discount',
            curve_id: discountId,
            currency: 'USD',
            base_date: asOf,
            method: 'bootstrap',
            interpolation: 'log_linear',
            extrapolation: 'extend',
            conventions: { curve_day_count: 'act_365f' },
          },
          {
            id: creditId,
            quote_set: 'credit',
            kind: 'hazard',
            curve_id: creditId,
            currency: 'USD',
            base_date: asOf,
            discount_curve_id: discountId,
            entity: 'CORP',
            seniority: 'senior',
            recovery_rate: 0.4,
            notional: 1,
            method: 'bootstrap',
          },
        ],
        settings: {},
      },
      market_data: [...rateQuotes, ...quotes],
    });
    return core.MarketContext.fromJson(calibration.result.final_market);
  };
  const market = fit(0);
  const shocked = fit(1);
  const curve = market.getHazard(creditId);
  const cds = facade.valuations.instruments.CreditDefaultSwap.example();
  const position = new portfolio.Position('short-cds', cds.toJson(), -2);
  const book = portfolio.Portfolio.builder('credit-book', 'USD', asOf)
    .addPosition(position)
    .build();
  const pricing = new core.FinstackConfig();
  try {
    const source = {
      curve_id: creditId,
      classification: {
        issuer: 'CORP',
        currency: 'USD',
        seniority: 'senior',
        bucket: 5,
        source_id: 'issuer-review',
        as_of: asOf,
      },
      vertices: Object.fromEntries(
        curve.hazardCalibration.spread_risk_inputs.map(({ quote, pillar_date }) => [
          quote.id,
          { quote_id: quote.id, pillar_date },
        ])
      ),
    };
    const result = portfolio.csrQuoteDelta(book, market, pricing, [source]);
    assert.deepEqual(
      result.quote_risk.map(({ vertex }) => vertex),
      vertices
    );
    const row = result.quote_risk[3].positions[0];
    assert.equal(row.position_scale, -2);
    const expected =
      -2 *
      (Number(cds.price(shocked.toJson(), asOf).value.amount) -
        Number(cds.price(market.toJson(), asOf).value.amount));
    assert.ok(Math.abs(expected) > 1);
    assert.ok(Math.abs(row.pnl_base_per_pp - 100 * expected) < 2);
    const rateCurve = market.getDiscount(discountId);
    const girrTargets = rateCurve.rateCalibration.execution.bindings.map(
      ({ quote_id, pillar_date }) => ({
        risk_currency: 'USD',
        risk_curve_id: discountId,
        vertex: quote_id.slice('usd-'.length),
        source: { kind: 'discount', curve_id: discountId },
        quote: { quote_id, pillar_date },
      })
    );
    rateCurve.free();
    const combined = portfolio.portfolioFrtbSensitivities(book, market, pricing, {
      girr_targets: girrTargets,
      csr_sources: [source],
    });
    assert.equal(combined.positions_mapped, 1);
    assert.deepEqual(combined.positions_unmapped, []);
    assert.equal(combined.girr.quote_risk.length, 10);
    assert.deepEqual(combined.csr.quote_risk, result.quote_risk);
    assert.throws(
      () =>
        portfolio.portfolioFrtbSensitivities(book, market, pricing, {
          girr_targets: [],
          csr_sources: [source],
        }),
      /Missing GIRR delta source/
    );
    [source.vertices['3Y'], source.vertices['5Y']] = [source.vertices['5Y'], source.vertices['3Y']];
    assert.throws(
      () => portfolio.csrQuoteDelta(book, market, pricing, [source]),
      /exact 3Y issuer vertex/
    );
  } finally {
    for (const handle of [pricing, book, position, cds, curve, shocked, market]) handle.free();
  }
});
