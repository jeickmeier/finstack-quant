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
  'InstrumentArtifactCache',
  'Portfolio',
  'aggregateFullCashflows',
  'aggregateFullCashflowsBuilt',
  'aggregateMetrics',
  'applyScenarioAndRevalue',
  'applyScenarioAndRevalueBuilt',
  'brinsonFachler',
  'buildPortfolioFromSpecJson',
  'campisiAttribution',
  'campisiCarinoLink',
  'campisiCarinoLinkFromSnapshots',
  'campisiReconciliationCheck',
  'carinoLink',
  'cellReturnsFromCurves',
  'cellReturnsFromReference',
  'computeFactorSensitivities',
  'computeFactorSensitivitiesWithMarket',
  'computePnlProfiles',
  'computePnlProfilesWithMarket',
  'decomposeFactorRisk',
  'excessReturns',
  'factorBrinsonAttribution',
  'gridAttribution',
  'gridCarinoLink',
  'mwrXirr',
  'optimizePortfolio',
  'parsePortfolioSpecJson',
  'portfolioResultGetMetric',
  'portfolioResultTotalValue',
  'replayPortfolio',
  'scenarioPnl',
  'scenarioPnlBuilt',
  'twrrLinked',
  'twrrModifiedDietz',
  'valuePortfolio',
  'valuePortfolioBuilt',
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
    assert.equal(
      typeof portfolio[key],
      'function',
      `portfolio.${key} must be a function (got ${typeof portfolio[key]})`
    );
  }
});

// Runtime arity gate. `index.d.ts` is hand-maintained, so a declaration with
// the wrong argument count would compile clean for a TypeScript caller while
// the extra argument is silently discarded at the JS boundary. Pinning
// Function.length here forces the real exports to stay at the arity the
// declarations promise; `dts_contract.rs` pins the declarations themselves.
test('portfolio Campisi exports keep their declared arity', () => {
  assert.equal(portfolio.campisiAttribution.length, 3);
  assert.equal(portfolio.campisiCarinoLink.length, 1);
  assert.equal(portfolio.campisiCarinoLinkFromSnapshots.length, 2);
  assert.equal(portfolio.campisiReconciliationCheck.length, 2);
});

// Same lesson as the Campisi arity gate above, applied to the credit
// excess-return / grid-attribution / factor-Brinson surfaces landed here:
// `index.d.ts` is hand-maintained, so a declaration with the wrong argument
// count would compile clean for a TypeScript caller while an extra argument
// is silently discarded at the JS boundary. `dts_contract.rs` pins the
// declared signatures themselves.
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

const campisiSnapshot = (sector, weight, totalReturn, yieldAnnual, modifiedDuration) => ({
  sector,
  weight,
  total_return: totalReturn,
  yield_annual: yieldAnnual,
  modified_duration: modifiedDuration,
  spread_duration: 0.0,
  spread: 0.0,
  delta_treasury_yield: -0.001,
  delta_spread: 0.0,
});

const CAMPISI_PORTFOLIO = JSON.stringify([
  campisiSnapshot('GOVT', 0.6, 0.016, 0.04, 5.0),
  campisiSnapshot('CORP', 0.4, 0.012, 0.06, 4.0),
]);
const CAMPISI_BENCHMARK = JSON.stringify([
  campisiSnapshot('GOVT', 0.5, 0.015, 0.04, 5.5),
  campisiSnapshot('CORP', 0.5, 0.011, 0.055, 4.5),
]);
const campisiConfig = (periodYears) => JSON.stringify({ period_years: periodYears });

test('portfolio.campisiAttribution reconciles the five effects to active return', () => {
  const result = assertStructured(
    portfolio.campisiAttribution(CAMPISI_PORTFOLIO, CAMPISI_BENCHMARK, campisiConfig(0.25)),
    'campisiAttribution result'
  );
  // Direct property read: an ES Map would give undefined here.
  assert.equal(typeof result.active_return, 'number');
  const reconstructed =
    result.total_allocation +
    result.total_active_carry +
    result.total_active_treasury +
    result.total_active_spread +
    result.total_selection;
  assert.ok(Math.abs(reconstructed - result.active_return) < 1e-12);
  assert.equal('spread_mode' in result, false);
});

test('portfolio.campisiAttribution fails closed on unknown and missing config fields', () => {
  // `period_years` is the config's only field and has no default: omitting it
  // must be rejected, not guessed.
  assert.throws(() =>
    portfolio.campisiAttribution(CAMPISI_PORTFOLIO, CAMPISI_BENCHMARK, JSON.stringify({}))
  );
  // The retired `spread_mode` key is now an unknown field. A stale caller must
  // be told, not silently served a result computed without it.
  assert.throws(() =>
    portfolio.campisiAttribution(
      CAMPISI_PORTFOLIO,
      CAMPISI_BENCHMARK,
      JSON.stringify({ period_years: 0.25, spread_mode: 'dts' })
    )
  );
});

test('portfolio.campisiCarinoLink links precomputed periods of unequal length', () => {
  // 31/365 and 28/365 — an act/365 monthly pair the snapshot-based entry
  // point cannot express, because it applies one shared period_years.
  const jan = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig(31 / 365)
  );
  const feb = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig(28 / 365)
  );
  assert.notEqual(jan.total_active_carry, feb.total_active_carry);

  const linked = assertStructured(
    portfolio.campisiCarinoLink(JSON.stringify([jan, feb])),
    'campisiCarinoLink result'
  );
  const geometric = linked.portfolio_return_compounded - linked.benchmark_return_compounded;
  const reconstructed =
    linked.linked_allocation +
    linked.linked_active_carry +
    linked.linked_active_treasury +
    linked.linked_active_spread +
    linked.linked_selection;
  assert.ok(Math.abs(reconstructed - geometric) < 1e-10);
  assert.equal(linked.periods.length, 2);
});

test('portfolio.campisiCarinoLink rejects inconsistent result contracts', () => {
  const result = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig(0.25)
  );

  const badActive = { ...result, active_return: result.active_return + 0.001 };
  assert.throws(() => portfolio.campisiCarinoLink(JSON.stringify([badActive])), /active_return/);

  const badSector = structuredClone(result);
  badSector.sectors[0].selection += 0.001;
  badSector.sectors[0].total_active += 0.001;
  assert.throws(() => portfolio.campisiCarinoLink(JSON.stringify([badSector])), /total_selection/);

  const badSectorTotal = structuredClone(result);
  badSectorTotal.sectors[0].total_active += 0.001;
  assert.throws(
    () => portfolio.campisiCarinoLink(JSON.stringify([badSectorTotal])),
    /total_active/
  );
});

test('portfolio.campisiCarinoLink accepts huge finite cancelling effects', () => {
  const period = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig(0.25)
  );
  period.portfolio_return = 0.01;
  period.benchmark_return = 0.01;
  period.active_return = 0;
  for (const name of [
    'total_allocation',
    'total_active_carry',
    'total_active_treasury',
    'total_active_spread',
    'total_selection',
  ]) {
    period[name] = 0;
  }
  for (const sector of period.sectors) {
    for (const name of [
      'allocation',
      'active_carry',
      'active_treasury',
      'active_spread',
      'selection',
      'total_active',
    ]) {
      sector[name] = 0;
    }
  }
  period.total_allocation = 1e308;
  period.total_active_carry = -1e308;
  period.sectors[0].allocation = 1e308;
  period.sectors[0].active_carry = -1e308;

  const linked = portfolio.campisiCarinoLink(JSON.stringify([period]));
  assert.equal(linked.linked_allocation, 1e308);
  assert.equal(linked.linked_active_carry, -1e308);
});

test('portfolio.campisiCarinoLinkFromSnapshots links raw snapshot periods', () => {
  const period = {
    portfolio: JSON.parse(CAMPISI_PORTFOLIO),
    benchmark: JSON.parse(CAMPISI_BENCHMARK),
  };
  const linked = assertStructured(
    portfolio.campisiCarinoLinkFromSnapshots(JSON.stringify([period, period]), campisiConfig(0.25)),
    'campisiCarinoLinkFromSnapshots result'
  );
  const geometric = linked.portfolio_return_compounded - linked.benchmark_return_compounded;
  const reconstructed =
    linked.linked_allocation +
    linked.linked_active_carry +
    linked.linked_active_treasury +
    linked.linked_active_spread +
    linked.linked_selection;
  assert.ok(Math.abs(reconstructed - geometric) < 1e-10);

  // The two link entry points take different payloads and are not aliases.
  assert.throws(() => portfolio.campisiCarinoLink(JSON.stringify([period])));
  assert.throws(() =>
    portfolio.campisiCarinoLinkFromSnapshots(
      JSON.stringify([
        portfolio.campisiAttribution(CAMPISI_PORTFOLIO, CAMPISI_BENCHMARK, campisiConfig(0.25)),
      ]),
      campisiConfig(0.25)
    )
  );
});

test('portfolio.campisiAttribution fails closed on a zero-net-weight sector', () => {
  // A long/short pair (or a CDS hedge against a cash bond) inside one bucket
  // nets to exactly zero weight. Its real contribution stays in the side
  // return while every per-sector effect would be forced to zero, so the
  // decomposition must be rejected with the offending sector named.
  const core = campisiSnapshot('CORE', 1.0, 0.015, 0.048, 5.0);
  const cleanBenchmark = JSON.stringify([campisiSnapshot('CORE', 1.0, 0.014, 0.044, 5.5)]);

  const hedgedPortfolio = JSON.stringify([
    core,
    campisiSnapshot('HEDGE', 0.5, 0.04, 0.06, 3.0),
    campisiSnapshot('HEDGE', -0.5, 0.01, 0.02, 1.0),
  ]);
  assert.throws(
    () => portfolio.campisiAttribution(hedgedPortfolio, cleanBenchmark, campisiConfig(0.25)),
    (error) => /HEDGE/.test(String(error)) && /Portfolio/.test(String(error))
  );

  const hedgedBenchmark = JSON.stringify([
    campisiSnapshot('CORE', 1.0, 0.014, 0.044, 5.5),
    campisiSnapshot('HEDGE', 0.4, 0.03, 0.055, 4.0),
    campisiSnapshot('HEDGE', -0.4, 0.005, 0.015, 1.0),
  ]);
  assert.throws(
    () =>
      portfolio.campisiAttribution(JSON.stringify([core]), hedgedBenchmark, campisiConfig(0.25)),
    (error) => /HEDGE/.test(String(error)) && /Benchmark/.test(String(error))
  );

  // A sector genuinely absent from one side is the legitimate zero-weight
  // case and must keep working.
  const oneSided = JSON.stringify([
    campisiSnapshot('CORE', 0.8, 0.015, 0.048, 5.0),
    campisiSnapshot('EXTRA', 0.2, 0.021, 0.07, 3.0),
  ]);
  const result = portfolio.campisiAttribution(oneSided, cleanBenchmark, campisiConfig(0.25));
  assert.deepEqual(
    result.sectors.map((s) => s.sector),
    ['CORE', 'EXTRA']
  );
  assert.equal(result.sectors[1].benchmark_weight, 0.0);
  const reconstructed =
    result.total_allocation +
    result.total_active_carry +
    result.total_active_treasury +
    result.total_active_spread +
    result.total_selection;
  assert.ok(Math.abs(reconstructed - result.active_return) < 1e-12);
});

test('portfolio.campisiReconciliationCheck reports the residual and fails closed', () => {
  const result = portfolio.campisiAttribution(
    CAMPISI_PORTFOLIO,
    CAMPISI_BENCHMARK,
    campisiConfig(0.25)
  );
  const resultJson = JSON.stringify(result);
  const report = assertStructured(
    portfolio.campisiReconciliationCheck(resultJson, 1e-10),
    'campisiReconciliationCheck result'
  );
  assert.equal(report.is_reconciled, true);
  assert.equal(report.tolerance, 1e-10);
  assert.ok(Math.abs(report.total_residual) <= 1e-10);

  // The tolerance argument is load-bearing, not decorative: a result whose
  // active_return has been tampered with breaks the identity at 1e-10 and
  // passes only under an absurdly loose tolerance.
  const tampered = JSON.stringify({ ...result, active_return: result.active_return + 0.01 });
  assert.equal(portfolio.campisiReconciliationCheck(tampered, 1e-10).is_reconciled, false);
  assert.equal(portfolio.campisiReconciliationCheck(tampered, 1.0).is_reconciled, true);

  // `FiAttributionResult` denies unknown fields, so a stale key fails closed
  // instead of being silently dropped into a report that still "reconciles".
  const withBogus = JSON.stringify({ ...result, bogus_field: 1.0 });
  assert.throws(() => portfolio.campisiReconciliationCheck(withBogus, 1e-10));
  assert.throws(() => portfolio.campisiCarinoLink(`[${withBogus}]`));
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
  new core.DiscountCurve(
    id,
    '2024-01-01',
    [0.0, 1.0, 0.25, 0.99004983, 0.5, 0.98019867, 1.25, 0.95122942, 1.5, 0.94176453]
  );

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
  const start = new core.DiscountCurve(
    'UST',
    '2024-01-01',
    [0.0, 1.0, 0.5, 0.98019867, 1.5, 0.94176453]
  );
  const end = new core.DiscountCurve(
    'UST',
    '2024-01-01',
    [0.0, 1.0, 0.25, 0.9875778, 1.25, 0.93941306]
  );
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
  const start = new core.DiscountCurve('UST', '2024-01-01', knots);
  const end = new core.DiscountCurve('UST', '2024-01-01', knots);
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
    /nFactors.*positive integer/i
  );
});

test('analytics.constrainedLeastSquares rejects truncating nFactors before conversion', () => {
  assert.throws(
    () => analytics.constrainedLeastSquares([1.0, -1.0], 2 ** 32 + 1, [0.02, -0.02], [0.5, 0.5]),
    /nFactors.*positive integer/i
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
  collateral: {},
  hierarchy: null,
});

test('portfolio spec wire surfaces keep returning canonical JSON strings', () => {
  const canonical = portfolio.parsePortfolioSpecJson(EMPTY_SPEC);
  assert.equal(typeof canonical, 'string', 'parsePortfolioSpecJson is a wire surface');
  assert.equal(JSON.parse(canonical).id, 'facade_portfolio');

  const rebuilt = portfolio.buildPortfolioFromSpecJson(canonical);
  assert.equal(typeof rebuilt, 'string', 'buildPortfolioFromSpecJson is a wire surface');
  assert.equal(JSON.parse(rebuilt).id, 'facade_portfolio');

  const handle = portfolio.Portfolio.fromSpec(canonical);
  assert.equal(typeof handle.toJson(), 'string', 'Portfolio.toJson is a wire surface');
  handle.free();
});

test('portfolio.valuePortfolio returns a structured valuation', () => {
  const valuation = assertStructured(
    portfolio.valuePortfolio(EMPTY_SPEC, EMPTY_MARKET, false),
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
    portfolio.aggregateFullCashflows(EMPTY_SPEC, EMPTY_MARKET),
    'aggregateFullCashflows result'
  );
  assert.deepEqual(ladder.events, []);
  assert.deepEqual(ladder.issues, []);

  const handle = portfolio.Portfolio.fromSpec(EMPTY_SPEC);
  const built = assertStructured(
    portfolio.aggregateFullCashflowsBuilt(handle, EMPTY_MARKET),
    'aggregateFullCashflowsBuilt result'
  );
  assert.deepEqual(built, ladder);
  handle.free();
});

test('portfolio.aggregateMetrics returns a structured metrics object', () => {
  const valuation = portfolio.valuePortfolio(EMPTY_SPEC, EMPTY_MARKET, false);
  const metrics = assertStructured(
    portfolio.aggregateMetrics(JSON.stringify(valuation), 'USD', EMPTY_MARKET, '2024-01-15'),
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
    portfolio.replayPortfolio(EMPTY_SPEC, snapshots, config),
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

  const linked = assertStructured(
    portfolio.carinoLink(JSON.stringify([JSON.parse(sectors), JSON.parse(sectors)])),
    'carinoLink result'
  );
  const geometric = linked.portfolio_return_compounded - linked.benchmark_return_compounded;
  const sum = linked.linked_allocation + linked.linked_selection + linked.linked_interaction;
  assert.ok(Math.abs(sum - geometric) < 1e-10);
});

test('portfolio.twrrLinked returns a structured linked return', () => {
  const linked = assertStructured(
    portfolio.twrrLinked(JSON.stringify([0.05, 0.03]), 1.0),
    'twrrLinked result'
  );
  assert.ok(Math.abs(linked.cumulative - 0.0815) < 1e-12);
  assert.equal(linked.num_periods, 2);
});

// Position ids / weights / covariance are all shared by the three
// decomposition entry points.
const VAR_IDS = JSON.stringify(['A', 'B']);
const VAR_WEIGHTS = JSON.stringify([0.6, 0.4]);
const VAR_COVARIANCE = JSON.stringify([
  [0.04, 0.01],
  [0.01, 0.09],
]);

test('models.factor.risk.parametricVarDecomposition returns the Python-parity object', () => {
  const decomposition = assertStructured(
    models.factor.risk.parametricVarDecomposition(VAR_IDS, VAR_WEIGHTS, VAR_COVARIANCE, 0.95),
    'parametricVarDecomposition result'
  );
  assert.equal(decomposition.confidence, 0.95);
  assert.equal(decomposition.n_positions, 2);
  assert.equal(decomposition.contributions.length, 2);
  assert.equal(decomposition.contributions[0].position_id, 'A');
  assert.equal(typeof decomposition.contributions[0].component_var, 'number');
  assert.equal(typeof decomposition.contributions[0].pct_contribution, 'number');
});

test('models.factor.risk.parametricEsDecomposition returns the Python-parity object', () => {
  const decomposition = assertStructured(
    models.factor.risk.parametricEsDecomposition(VAR_IDS, VAR_WEIGHTS, VAR_COVARIANCE, 0.95),
    'parametricEsDecomposition result'
  );
  assert.equal(decomposition.n_positions, 2);
  assert.equal(typeof decomposition.portfolio_es, 'number');
  assert.equal(typeof decomposition.contributions[0].component_es, 'number');
  assert.equal(typeof decomposition.contributions[0].pct_contribution, 'number');
});

test('models.factor.risk.historicalVarDecomposition returns a structured decomposition', () => {
  // (1 - confidence) * n_scenarios must be at least 1 to resolve the tail, so
  // 0.9 confidence needs >= 10 scenarios per position.
  const scenarios = 20;
  const pnls = JSON.stringify([
    Array.from({ length: scenarios }, (_, i) => i - scenarios / 2),
    Array.from({ length: scenarios }, (_, i) => (i - scenarios / 2) / 2),
  ]);
  const decomposition = assertStructured(
    models.factor.risk.historicalVarDecomposition(VAR_IDS, pnls, 0.9),
    'historicalVarDecomposition result'
  );
  assert.equal(decomposition.n_positions, 2);
  assert.equal(decomposition.contributions.length, 2);
});

test('models.factor.risk.evaluateRiskBudget returns a structured budget report', () => {
  const budget = assertStructured(
    models.factor.risk.evaluateRiskBudget(
      VAR_IDS,
      JSON.stringify([60.0, 40.0]),
      JSON.stringify([0.5, 0.5]),
      100.0,
      1.1
    ),
    'evaluateRiskBudget result'
  );
  assert.equal(budget.portfolio_var, 100.0);
  assert.equal(budget.utilization_threshold, 1.1);
  assert.equal(budget.positions.length, 2);
  assert.equal(budget.positions[0].position_id, 'A');
  assert.equal(typeof budget.positions[0].breach, 'boolean');
});

test('factor-risk kernels are absent from the portfolio namespace', () => {
  assert.equal('parametricVarDecomposition' in portfolio, false);
  assert.equal('parametricEsDecomposition' in portfolio, false);
  assert.equal('historicalVarDecomposition' in portfolio, false);
  assert.equal('evaluateRiskBudget' in portfolio, false);
});

test('standalone sensitivity outputs require and retain the reporting currency', () => {
  const result = portfolio.computeFactorSensitivities(
    '[]',
    '[]',
    EMPTY_MARKET,
    '2025-01-15',
    'EUR'
  );
  assert.equal(result.base_currency, 'EUR');
  assert.deepEqual(result.position_ids, []);
  assert.deepEqual(result.data, []);
  assert.deepEqual(portfolio.computePnlProfiles('[]', '[]', EMPTY_MARKET, '2025-01-15', 'EUR'), []);
  assert.throws(() =>
    portfolio.computeFactorSensitivities('[]', '[]', EMPTY_MARKET, '2025-01-15', 'INVALID')
  );
});
