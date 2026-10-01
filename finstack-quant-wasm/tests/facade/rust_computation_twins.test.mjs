/**
 * Facade tests for the Rust computations that used to be reachable from
 * Python only (remediation slice S21).
 *
 * Expected values are the Python outputs for the same inputs, so each case
 * is a cross-host golden: both hosts call the same Rust entry point.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, {
  attribution,
  calibration,
  cashflows,
  core,
  portfolio,
  statements,
  statements_analytics,
  valuations,
} from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const repoFile = (path) => readFileSync(new URL(`../../../${path}`, import.meta.url), 'utf8');
const close = (actual, expected, tol, label) =>
  assert.ok(Math.abs(actual - expected) <= tol, `${label}: ${actual} vs ${expected}`);

const EMPTY_MARKET_FIELDS = {
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

/** Python `DiscountCurve(id, base, [(t, exp(-r t))], day_count="act_365f")`. */
const discountCurve = (id, base, rate) => ({
  type: 'discount',
  id,
  base,
  day_count: 'act_365f',
  knot_points: [0.0, 1.0, 5.0, 10.0].map((t) => [t, Math.exp(-rate * t)]),
  interp_style: 'monotone_convex',
  extrapolation: 'flat_forward',
  min_forward_rate: -0.005,
  allow_non_monotonic: false,
  min_forward_tenor: 1e-6,
  rate_calibration: null,
  calibration_ois_cutoff_days: null,
  fx_policy: null,
});

// Python `MarketContext().insert(DiscountCurve.flat("USD-OIS", 2024-01-15, 0.04))`.
const FLAT_USD_MARKET = JSON.stringify({
  schema_version: 1,
  curves: [
    {
      type: 'discount',
      id: 'USD-OIS',
      base: '2024-01-15',
      day_count: 'act_365f',
      knot_points: [
        [0.0, 1.0],
        [1.0, 0.9607894391523232],
      ],
      interp_style: 'log_linear',
      extrapolation: 'flat_forward',
      min_forward_rate: null,
      allow_non_monotonic: false,
      min_forward_tenor: 1e-6,
      rate_calibration: null,
      calibration_ois_cutoff_days: null,
      fx_policy: null,
    },
  ],
  fx: null,
  surfaces: [],
  ...EMPTY_MARKET_FIELDS,
});

// USD-OIS 4% / EUR-OIS 2%, EURUSD 1.10 spot and a flat 10% EURUSD vol surface
// (the `_fx_market()` of finstack-quant-py/tests/test_valuations_credit_equity_fx.py).
const FX_MARKET = JSON.stringify({
  schema_version: 1,
  curves: [
    discountCurve('EUR-OIS', '2025-01-15', 0.02),
    discountCurve('USD-OIS', '2025-01-15', 0.04),
  ],
  fx: {
    config: { pivot_currency: 'USD', enable_triangulation: true, cache_capacity: 256 },
    quotes: [['EUR', 'USD', 1.1]],
    provider_quotes: [],
    pinned_quotes: [],
  },
  surfaces: [
    {
      id: 'EURUSD-VOL',
      expiries: [0.5, 1.0, 2.0],
      strikes: [1.0, 1.12, 1.3],
      secondary_axis: 'strike',
      quote_type: 'black_lognormal',
      interpolation_mode: 'vol',
      vols_row_major: [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
    },
  ],
  ...EMPTY_MARKET_FIELDS,
});

const usd = () => new core.Currency('USD');

// ---------------------------------------------------------------- cashflows

// Same fixture as finstack-quant-py/tests/test_cashflows.py::_cashflow_spec().
const CASHFLOW_SPEC = JSON.stringify({
  notional: { initial: { amount: '1000000', currency: 'USD' }, amort: 'none' },
  issue_date: '2024-08-31',
  maturity: '2025-08-31',
  coupon_program: [
    {
      kind: 'fixed',
      spec: {
        coupon_type: 'cash',
        rate: '0.06',
        frequency: { count: 12, unit: 'months' },
        day_count: '30_360',
        business_day_convention: 'following',
        calendar_id: 'weekends_only',
        stub: 'none',
        end_of_month: false,
        payment_lag_days: 0,
      },
    },
  ],
});

test('cashflows.absToSmm matches the Rust ABS kernel', () => {
  assert.equal(cashflows.absToSmm(0.015, 1), 0.015);
  assert.equal(cashflows.absToSmm(0.015, 11), 0.01764705882352941);
  assert.throws(
    () => cashflows.absToSmm(1.5, 1),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => cashflows.absToSmm(0.015, 1.5),
    (e) => e.kind === 'invalid_type'
  );
});

test('cashflows schedule analytics match the typed Python CashFlowSchedule methods', () => {
  const schedule = cashflows.buildCashflowScheduleJson(CASHFLOW_SPEC, null);
  assert.equal(cashflows.scheduleWal(schedule, '2024-08-31'), 1.0027397260273974);
  const balances = cashflows.scheduleOutstandingByDate(schedule);
  assert.deepEqual(
    balances.map((row) => [row.date, Number(row.amount.amount)]),
    [
      ['2024-08-31', 1000000],
      ['2025-08-31', 0],
      ['2025-09-01', 0],
    ]
  );
  const flows = JSON.parse(schedule).flows;
  const pvs = flows.map((flow) => Number(flow.amount.amount));
  const ladder = cashflows.scheduleCalendarYearLadder(schedule, pvs);
  assert.deepEqual(
    ladder.map((row) => row.year),
    [2024, 2025]
  );
  assert.ok(ladder.every((row) => Number.isFinite(row.pv)));
  assert.throws(
    () => cashflows.scheduleCalendarYearLadder(schedule, pvs.slice(1)),
    (e) => e.kind === 'validation'
  );
});

// ------------------------------------------------------------------ bonds

test('Bond presets, zeroCoupon and return floors construct validated bonds', () => {
  const { Bond, TermLoan } = valuations.instruments;
  assert.equal(Bond.example().id, 'US912828XG33');
  for (const preset of [Bond.exampleFloating(), Bond.exampleCallable(), Bond.exampleAmortizing()]) {
    assert.equal(JSON.parse(preset.toJson()).instrument.type, 'bond');
  }
  const zc = Bond.zeroCoupon(
    'ZC',
    new core.Money(1_000_000, usd()),
    '2024-01-15',
    '2029-01-15',
    'USD-OIS'
  );
  assert.equal(JSON.parse(zc.toJson()).instrument.spec.cashflow_spec.fixed.rate, '0');
  const floored = zc.minMoic(1.3);
  assert.notEqual(floored.toJson(), zc.toJson());
  assert.notEqual(zc.minXirr(new core.Rate(0.12)).toJson(), zc.toJson());
  const frn = Bond.floatingWithConvention(
    'FRN',
    new core.Money(1_000_000, usd()),
    'USD-SOFR-3M',
    new core.Bps(150),
    '2024-01-15',
    '2029-01-15',
    core.Tenor.quarterly(),
    core.DayCount.act360(),
    'us_corporate',
    'USD-OIS'
  );
  assert.equal(frn.id, 'FRN');
  assert.throws(
    () =>
      Bond.floatingWithConvention(
        'FRN',
        new core.Money(1_000_000, usd()),
        'USD-SOFR-3M',
        new core.Bps(150),
        '2024-01-15',
        '2029-01-15',
        core.Tenor.quarterly(),
        core.DayCount.act360(),
        'not_a_convention',
        'USD-OIS'
      ),
    (e) => e.kind === 'validation'
  );
  assert.equal(JSON.parse(TermLoan.exampleCallable().toJson()).instrument.type, 'term_loan');
  assert.equal(
    JSON.parse(TermLoan.exampleFloatingWithDdtl().toJson()).instrument.type,
    'term_loan'
  );
});

test('instrumentCashflows is the typed twin of instrumentCashflowsJson', () => {
  const zc = valuations.instruments.Bond.zeroCoupon(
    'ZC',
    new core.Money(1_000_000, usd()),
    '2024-01-15',
    '2029-01-15',
    'USD-OIS'
  );
  const args = [zc.toJson(), FLAT_USD_MARKET, '2024-01-15', 'discounting'];
  const envelope = valuations.instruments.instrumentCashflows(...args);
  assert.equal(typeof envelope, 'object');
  // Same Rust envelope; JSON.stringify spells whole floats without `.0`.
  assert.deepEqual(envelope, JSON.parse(valuations.instruments.instrumentCashflowsJson(...args)));
  const market = core.MarketContext.fromJson(FLAT_USD_MARKET);
  const handled = valuations.instruments.instrumentCashflowsWithMarket(
    zc.toJson(),
    market,
    '2024-01-15',
    'discounting'
  );
  assert.deepEqual(handled, envelope);
  market.free();
});

test('valuationResultMetricSeries decodes composite metric keys', () => {
  const zc = valuations.instruments.Bond.zeroCoupon(
    'ZC',
    new core.Money(1_000_000, usd()),
    '2024-01-15',
    '2029-01-15',
    'USD-OIS'
  );
  const result = valuations.instruments.priceInstrument(
    zc.toJson(),
    FLAT_USD_MARKET,
    '2024-01-15',
    'discounting',
    ['bucketed_dv01']
  );
  const series = valuations.valuationResultMetricSeries(result, 'bucketed_dv01');
  assert.ok(series.length > 0);
  for (const [components, value] of series) {
    assert.equal(components[0], 'USD-OIS');
    assert.equal(typeof value, 'number');
  }
  assert.throws(() => valuations.valuationResultMetricSeries(result, 'bucketed_dv01::'));
});

// ------------------------------------------------------- facilities / loans

test('AssetBackedFacility exposes undrawn, revolving end, projection and IRR', () => {
  const facility = valuations.instruments.AssetBackedFacility.example();
  assert.equal(facility.undrawn.toString(), 'USD 10000000.00');
  assert.equal(facility.effectiveRevolvingEnd, '2026-01-15');
  const projection = facility.project(FLAT_USD_MARKET, '2024-01-15');
  assert.equal(projection.commitment_fees.length, 8);
  close(facility.facilityIrr(FLAT_USD_MARKET, '2024-01-15'), 0.0626888085294107, 1e-12, 'IRR');
  const deal = JSON.parse(facility.synthesizedDealJson());
  assert.equal(deal.schema, 'finstack_quant.instrument/1');
  assert.equal(deal.instrument.type, 'structured_credit');
  assert.equal(deal.instrument.spec.tranches.tranches.length, 2);
});

test('RevolvingCredit exposes isStochastic and the expected cashflow schedule', () => {
  const { RevolvingCredit } = valuations.instruments;
  const envelope = JSON.parse(RevolvingCredit.example().toJson());
  envelope.instrument.spec.rate = { fixed: { rate: 0.06 } };
  const facility = RevolvingCredit.fromJson(JSON.stringify(envelope));
  assert.equal(facility.isStochastic, false);
  const schedule = facility.expectedCashflows(FLAT_USD_MARKET, '2024-01-15');
  assert.ok(schedule.flows.length > 0);
  assert.equal(schedule.notional.initial.currency, 'USD');
});

// --------------------------------------------------------------------- FX

test('FxForward presets and the CIP market forward match Python', () => {
  const { FxForward } = valuations.fx;
  const forward = FxForward.example();
  close(forward.marketForwardRate(FX_MARKET, '2025-01-15'), 1.109139126165159, 1e-12, 'fwd');
  close(forward.metric(FX_MARKET, '2025-01-15', 'dv01'), 0.44193883896969055, 1e-9, 'dv01');
  assert.equal(FxForward.standardSettlementDays('EUR', 'USD'), 2);
  assert.equal(FxForward.standardSettlementDays('USD', 'CAD'), 1);
  const traded = FxForward.fromTradeDate(
    'EURUSD-3M',
    'EUR',
    'USD',
    '2025-01-15',
    core.Tenor.parse('3M'),
    new core.Money(1_000_000, new core.Currency('EUR')),
    'USD-OIS',
    'EUR-OIS'
  );
  assert.equal(JSON.parse(traded.toJson()).instrument.spec.maturity, '2025-04-17');
  const explicit = FxForward.fromTradeDate(
    'EURUSD-3M',
    'EUR',
    'USD',
    '2025-01-15',
    core.Tenor.parse('3M'),
    new core.Money(1_000_000, new core.Currency('EUR')),
    'USD-OIS',
    'EUR-OIS',
    { settlement_days: 2, business_day_convention: 'modified_following' }
  );
  assert.equal(explicit.toJson(), traded.toJson());
  assert.throws(() =>
    FxForward.fromTradeDate(
      'X',
      'EUR',
      'USD',
      '2025-01-15',
      core.Tenor.parse('3M'),
      new core.Money(1_000_000, new core.Currency('EUR')),
      'USD-OIS',
      'EUR-OIS',
      { settlement_lag: 2 }
    )
  );
  const withPoints = traded.withForwardPoints(1.1, 0.0025);
  assert.equal(JSON.parse(withPoints.toJson()).instrument.spec.contract_rate, 1.1025);
  const withPips = traded.withForwardPips(1.1, 25);
  close(JSON.parse(withPips.toJson()).instrument.spec.contract_rate, 1.1025, 1e-12, 'pips');
  assert.throws(
    () => traded.withForwardPoints(-1, 0),
    (e) => e.kind === 'validation'
  );
});

test('FxOption.european and impliedVol recover the surface vol', () => {
  const { FxOption } = valuations.fx;
  assert.equal(JSON.parse(FxOption.example().toJson()).instrument.type, 'fx_option');
  const option = FxOption.european(
    'EURUSD-CALL',
    'EUR',
    'USD',
    1.12,
    '2025-06-15',
    new core.Money(1_000_000, new core.Currency('EUR')),
    'EURUSD-VOL',
    'call',
    'spot',
    'USD',
    'desk'
  );
  const price = Number(option.price(FX_MARKET, '2025-01-15').value.amount);
  close(price, 23106.07324729069, 1e-6, 'price');
  close(option.impliedVol(FX_MARKET, '2025-01-15', price), 0.1, 1e-9, 'iv');
  close(option.metric(FX_MARKET, '2025-01-15', 'delta'), 448773.4593981477, 1e-6, 'delta');
  assert.throws(
    () =>
      FxOption.european(
        'X',
        'EUR',
        'USD',
        1.12,
        '2025-06-15',
        new core.Money(1_000_000, new core.Currency('EUR')),
        'EURUSD-VOL',
        'call',
        'sideways',
        'USD',
        'desk'
      ),
    (e) => e.kind === 'validation'
  );
});

// --------------------------------------------------------------- portfolio

const zeroCouponSpec = (id, notional, maturity) =>
  JSON.parse(
    valuations.instruments.Bond.zeroCoupon(
      id,
      new core.Money(notional, usd()),
      '2024-01-15',
      maturity,
      'USD-OIS'
    ).toJson()
  ).instrument;

const BOOK = {
  id: 'book',
  name: 'Book',
  base_currency: 'USD',
  as_of: '2024-01-15',
  entities: { E: { id: 'E', name: null } },
  positions: [
    {
      position_id: 'A',
      entity_id: 'E',
      instrument_id: 'ZC-A',
      instrument_spec: zeroCouponSpec('ZC-A', 1_000_000, '2029-01-15'),
      quantity: 1,
      unit: 'units',
    },
    {
      position_id: 'B',
      entity_id: 'E',
      instrument_id: 'ZC-B',
      instrument_spec: zeroCouponSpec('ZC-B', 500_000, '2027-01-15'),
      quantity: 1,
      unit: 'units',
    },
  ],
  tags: { desk: 'rates' },
  meta: { owner: 's21' },
};

test('Portfolio getters project the Rust fields', () => {
  const book = portfolio.Portfolio.fromSpec(BOOK);
  assert.equal(book.name, 'Book');
  assert.deepEqual(book.tags, { desk: 'rates' });
  assert.deepEqual(book.meta, { owner: 's21' });
  assert.deepEqual(book.entityIds, ['E']);
  assert.deepEqual(book.positionIds, ['A', 'B']);
  book.free();
});

test('portfolio cashflow readers net and collapse the Rust ladder', () => {
  const ladder = portfolio.aggregateFullCashflows(BOOK, FLAT_USD_MARKET);
  assert.deepEqual(portfolio.netInCurrencyByDate(ladder, 'USD'), [
    ['2024-01-15', -1500000],
    ['2025-01-15', 0],
    ['2026-01-15', 0],
    ['2027-01-15', 500000],
    ['2028-01-17', 0],
    ['2029-01-15', 1000000],
  ]);
  const collapsed = portfolio.collapseToBaseByDateKind(
    ladder,
    FLAT_USD_MARKET,
    'USD',
    '2024-01-15',
    { USD: 'USD-OIS' }
  );
  assert.deepEqual(collapsed['2027-01-15'].notional, { amount: '500000', currency: 'USD' });
  assert.deepEqual(
    portfolio.collapseToBaseByDateKind(ladder, FLAT_USD_MARKET, 'USD', '2024-01-15'),
    collapsed
  );
  assert.throws(
    () => portfolio.netInCurrencyByDate(ladder, 'XYZ'),
    (e) => e.kind === 'validation'
  );
});

test('portfolioMetricsSeries decodes bucketed metrics', () => {
  const valuation = portfolio.valuePortfolio(BOOK, FLAT_USD_MARKET, false, ['bucketed_dv01']);
  const metrics = portfolio.aggregateMetrics(
    JSON.stringify(valuation, (_, v) => (typeof v === 'bigint' ? Number(v) : v)),
    'USD',
    FLAT_USD_MARKET,
    '2024-01-15'
  );
  const series = portfolio.portfolioMetricsSeries(metrics, 'bucketed_dv01');
  assert.equal(series.length, 11);
  assert.deepEqual(series[0].components, ['USD-OIS', '10y']);
  assert.deepEqual(Object.keys(series[0].by_entity), ['E']);
});

test('rebalanceFromSpec applies the optimizer solution to the spec portfolio', () => {
  const spec = JSON.stringify({
    portfolio: BOOK,
    objective: { maximize: { weighted_sum: { metric: { constant: 1.0 } } } },
    constraints: [{ budget: { rhs: 1.0 } }],
    weighting: 'notional_weight',
    missing_metric_policy: 'zero',
    label: null,
  });
  const result = portfolio.optimizePortfolio(spec, FLAT_USD_MARKET);
  const rebalanced = portfolio.rebalanceFromSpec(spec, result);
  const positions = JSON.parse(rebalanced.toJson()).positions;
  assert.deepEqual(
    positions.map((p) => [p.position_id, p.quantity]),
    [
      ['A', 0],
      ['B', 3],
    ]
  );
  rebalanced.free();
  const foreign = structuredClone(result);
  foreign.implied_quantities.ZZZ = 1;
  assert.throws(
    () => portfolio.rebalanceFromSpec(spec, foreign),
    (e) => e.kind === 'validation' && /ZZZ/.test(e.message)
  );
});

// -------------------------------------------------------------- statements

const MODEL = JSON.stringify({
  id: 'facade-model',
  periods: [{ id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: false }],
  nodes: {
    revenue: { node_id: 'revenue', node_type: 'value', values: { '2025Q1': 100000.0 } },
  },
  schema_version: 1,
});

test('nodeToDatedSchedule applies the Rust period-date convention', () => {
  const result = statements.evaluateModel(MODEL);
  assert.deepEqual(statements.nodeToDatedSchedule(MODEL, result, 'revenue'), [
    ['2025-03-31', 100000],
  ]);
  assert.deepEqual(statements.nodeToDatedSchedule(MODEL, result, 'revenue', 'start'), [
    ['2025-01-01', 100000],
  ]);
  assert.throws(
    () => statements.nodeToDatedSchedule(MODEL, result, 'ebitda'),
    (e) => e.kind === 'not_found'
  );
  assert.throws(
    () => statements.nodeToDatedSchedule(MODEL, result, 'revenue', 'middle'),
    (e) => e.kind === 'validation'
  );
});

test('financialModelContentHash matches the Python/Rust hash', () => {
  assert.equal(
    statements.financialModelContentHash(MODEL),
    'sha256:43c8428179a2f9bb332af91d87fc7c91ea922286ec7a42b3e5ace45b85a0f17e'
  );
});

test('monteCarloBreachProbability reads the serialized path table', () => {
  const model = JSON.stringify({
    id: 'mc',
    periods: [
      { id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: true },
      { id: '2025Q2', start: '2025-04-01', end: '2025-07-01', is_actual: false },
      { id: '2025Q3', start: '2025-07-01', end: '2025-10-01', is_actual: false },
      { id: '2025Q4', start: '2025-10-01', end: '2026-01-01', is_actual: false },
    ],
    nodes: {
      revenue: {
        node_id: 'revenue',
        node_type: 'mixed',
        values: { '2025Q1': 100.0 },
        forecast: { method: 'normal', params: { mean: 100.0, std_dev: 10.0, seed: 7 } },
        value_type: { type: 'scalar' },
      },
    },
    schema_version: 1,
  });
  const config = { n_paths: 200, seed: 42, percentiles: [0.05, 0.5, 0.95] };
  const withPaths = statements.evaluateMonteCarlo(model, { ...config, include_path_data: true });
  assert.equal(statements.monteCarloBreachProbability(withPaths, 'revenue', 410), 0.22);
  assert.equal(statements.monteCarloBreachProbability(withPaths, 'missing', 410), undefined);
  const withoutPaths = statements.evaluateMonteCarlo(model, {
    ...config,
    include_path_data: false,
  });
  assert.equal(statements.monteCarloBreachProbability(withoutPaths, 'revenue', 410), undefined);

  const median = statements.monteCarloPercentileByPeriod(withPaths, 'revenue', 0.5);
  assert.deepEqual(Object.keys(median), ['2025Q2', '2025Q3', '2025Q4']);
  const pair = withPaths.percentile_results.revenue.values['2025Q2'].find(([q]) => q === 0.5);
  assert.equal(median['2025Q2'], pair[1]);
  assert.equal(statements.monteCarloPercentileByPeriod(withPaths, 'revenue', 0.25), undefined);
  assert.equal(statements.monteCarloPercentileByPeriod(withPaths, 'missing', 0.5), undefined);
});

test('statementResultToTableLong/Wide export the Rust table envelopes', () => {
  const result = statements.evaluateModel(MODEL);
  const long = statements.statementResultToTableLong(result);
  assert.deepEqual(
    long.columns.map((column) => column.name),
    ['node_id', 'period_id', 'value', 'value_money', 'currency', 'value_type']
  );
  const wide = statements.statementResultToTableWide(JSON.stringify(result));
  assert.deepEqual(
    wide.columns.map((column) => column.name),
    ['period_id', 'revenue']
  );
  assert.equal(JSON.stringify(wide).includes('100000'), true);
  assert.throws(
    () => statements.statementResultToTableLong('{'),
    (e) => e.kind === 'validation'
  );
});

test('scenarioComparisonTable and parameterSpecWithPercentages run the Rust helpers', () => {
  const results = statements_analytics.evaluateScenarioSet(MODEL, {
    scenarios: {
      low: { parent: null, period_overrides: {}, overrides: { revenue: 1.0 } },
      high: { parent: null, period_overrides: {}, overrides: { revenue: 4.0 } },
    },
  });
  const table = statements_analytics.scenarioComparisonTable(results, ['revenue']);
  const names = table.columns.map((column) => column.name);
  assert.deepEqual(names, ['period', 'metric', 'low', 'high', 'high_vs_low_frac']);
  const frac = table.columns.find((column) => column.name === 'high_vs_low_frac');
  assert.deepEqual(frac.data, { type: 'nullable_float64', values: [3] });
  assert.throws(() => statements_analytics.scenarioComparisonTable(results, []));

  const spec = statements_analytics.parameterSpecWithPercentages(
    'revenue',
    '2025Q1',
    100000,
    [-10, 0, 10]
  );
  assert.equal(spec.node_id, 'revenue');
  assert.deepEqual(
    spec.perturbations.map((v) => Math.round(v)),
    [90000, 100000, 110000]
  );
});

// ------------------------------------------------------------- calibration

test('calibration content hashes match the Python/Rust hashes', () => {
  const envelope = repoFile(
    'finstack-quant/calibration/examples/market_bootstrap/01_usd_discount.json'
  );
  assert.equal(
    calibration.calibrationEnvelopeContentHash(envelope),
    'sha256:17433701c1eb8fbea59159c679c47bce1bd6bff4d691d2e569bf9b50c424355d'
  );
  // The solved residuals differ from the native build in the last bits
  // (wasm32 libm), so the result hash is pinned against its own JSON text:
  // the typed load normalises number spelling and key order.
  const result = calibration.calibrate(envelope);
  const hash = calibration.calibrationResultContentHash(result);
  assert.match(hash, /^sha256:[0-9a-f]{64}$/);
  assert.equal(calibration.calibrationResultContentHash(JSON.stringify(result)), hash);
  assert.throws(() => calibration.calibrationEnvelopeContentHash('{ not json'));
});

// ------------------------------------------------------------- attribution

const CONVERTIBLE = JSON.parse(
  repoFile('finstack-quant/attribution/tests/fixtures/production_convertible_credit.json')
);

test('pnlBridge and attributePnlMany run the Rust attribution entry points', () => {
  const bridge = attribution.pnlBridge(
    CONVERTIBLE.instrument,
    CONVERTIBLE.market_t0,
    CONVERTIBLE.market_t1,
    CONVERTIBLE.as_of_t0,
    CONVERTIBLE.as_of_t1,
    'USD'
  );
  assert.equal(bridge.toString(), 'USD -61.80');

  const params = new attribution.AttributionJsonInputs(
    JSON.stringify(CONVERTIBLE.instrument),
    JSON.stringify(CONVERTIBLE.market_t0),
    JSON.stringify(CONVERTIBLE.market_t1),
    CONVERTIBLE.as_of_t0,
    CONVERTIBLE.as_of_t1,
    '"parallel"',
    undefined,
    false
  );
  const single = attribution.attributePnl(params);
  const many = attribution.attributePnlMany(params, [
    CONVERTIBLE.instrument,
    CONVERTIBLE.instrument,
  ]);
  assert.equal(many.length, 2);
  assert.deepEqual(many[0].total_pnl, single.total_pnl);

  assert.match(attribution.pnlAttributionExplainText(single), /Total P&L/);
  assert.ok(
    attribution.pnlAttributionExplainVerboseText(single).length >=
      attribution.pnlAttributionExplainText(single).length
  );
  assert.equal(
    attribution.pnlAttributionResidualWithinTolerance(single),
    attribution.pnlAttributionResidualWithinTolerance(
      single,
      single.meta.tolerance_pct,
      single.meta.tolerance_abs
    )
  );
  attribution.pnlAttributionValidateCurrencies(single);
  assert.deepEqual(attribution.pnlAttributionRequiredMetrics(single), []);
});

test('attributeReturnContribution runs the Rust return-contribution engine', () => {
  const spec = { as_of: '2026-01-02', positions: [{ id: 'A', market_value: 100.0, return: 0.02 }] };
  assert.equal(attribution.attributeReturnContribution(spec).portfolio_return, 0.02);
  assert.equal(
    JSON.parse(attribution.attributeReturnContributionJson(spec)).portfolio_return,
    0.02
  );
  assert.equal(typeof attribution.validateReturnContributionJson(JSON.stringify(spec)), 'string');
  assert.throws(
    () => attribution.attributeReturnContribution({ as_of: '2026-01-02', positions: [] }),
    (e) => e.kind === 'validation'
  );
});
