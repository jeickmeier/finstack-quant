/** Composite-instrument facade runtime contract tests. */

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
await facade.default({ module_or_path: readFileSync(WASM_BG) });

const market = {
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
};

const equity = (id, price) => ({
  type: 'equity',
  spec: {
    id,
    ticker: id,
    currency: 'USD',
    quantity: 1,
    quoted_spot: price,
    spot_id: null,
    div_yield_id: null,
    discrete_dividends: [],
    discount_curve_id: 'USD',
    attributes: {},
  },
});

const spec = {
  id: 'A-B',
  reporting_currency: 'USD',
  capital: { amount: '100', currency: 'USD' },
  legs: [
    { instrument_id: 'A', instrument: equity('A', 100), score: 1 },
    { instrument_id: 'B', instrument: equity('B', 90), score: -1 },
  ],
  weighting_method: { kind: 'fixed_quantity' },
  rebalance_rule: { kind: 'manual' },
  attributes: {},
};

test('composite facade initializes, decomposes, and reports flat history', () => {
  const initialized = facade.valuations.composite.initialize(spec, market, '2025-01-01');
  assert.equal(initialized.instrument.instrument.type, 'composite');
  assert.deepEqual(
    initialized.trades.map((trade) => [trade.instrument_id, trade.quantity_delta]),
    [
      ['A', 1],
      ['B', -1],
    ]
  );

  const exposures = facade.valuations.composite.primitiveExposures(
    initialized.instrument,
    market,
    '2025-01-02'
  );
  assert.deepEqual(
    exposures.aggregates.map((item) => [item.instrument_id, item.net_quantity]),
    [
      ['A', 1],
      ['B', -1],
    ]
  );

  const observations = ['2025-01-01', '2025-01-02', '2025-01-03'].map((date) => ({
    date,
    state: market,
  }));
  const history = facade.valuations.composite.historyFromSpec(spec, observations);
  assert.deepEqual(
    history.map((row) => row.return_index),
    [100, 100, 100]
  );
  assert.deepEqual(
    history.map((row) => Number(row.pnl.amount)),
    [0, 0, 0]
  );
});

test('composite metric requests reject obsolete wire spellings', () => {
  const initialized = facade.valuations.composite.initialize(spec, market, '2025-01-01');
  assert.throws(
    () =>
      facade.valuations.composite.primitiveExposures(initialized.instrument, market, '2025-01-01', [
        'pv01::USD_x2dOIS',
      ]),
    /noncanonical/
  );
});

test('composite errors preserve typed classification', () => {
  const initialized = facade.valuations.composite.initialize(spec, market, '2025-01-01');
  const wrongType = structuredClone(initialized.instrument);
  wrongType.instrument = equity('not found', 100);
  assert.throws(
    () => facade.valuations.composite.rebalance(wrongType, market, '2025-01-01'),
    (error) => {
      assert.equal(error.name, 'FinstackError');
      assert.equal(error.kind, 'validation');
      assert.match(error.message, /expected instrument type `composite`, got `equity`/);
      return true;
    }
  );
  assert.throws(
    () => facade.valuations.composite.rebalance('{', market, '2025-01-01'),
    (error) => {
      assert.equal(error.name, 'FinstackError');
      assert.equal(error.kind, 'validation');
      return true;
    }
  );
});

test('composite optional history, previous state, and warmup accept null like Python None', () => {
  const composite = facade.valuations.composite;
  const initialized = composite.initialize(spec, market, '2025-01-01', null);
  assert.equal(initialized.instrument.instrument.type, 'composite');
  const rebalanced = composite.rebalance(initialized.instrument, market, '2025-01-02', null);
  assert.equal(rebalanced.instrument.instrument.type, 'composite');
  assert.deepEqual(
    composite.executionTrades(initialized.instrument, null),
    composite.executionTrades(initialized.instrument)
  );
  const observations = ['2025-01-01', '2025-01-02'].map((date) => ({ date, state: market }));
  assert.equal(composite.historyFromSpec(spec, observations, null).length, 2);
});

test('portfolio.primitiveExposures nets composite legs against direct positions', () => {
  const initialized = facade.valuations.composite.initialize(spec, market, '2025-01-01');
  const book = facade.portfolio.Portfolio.fromSpec({
    id: 'PRIMITIVE',
    as_of: '2025-01-01',
    base_currency: 'USD',
    entities: { FUND: { id: 'FUND' } },
    positions: [
      {
        position_id: 'P-COMPOSITE',
        entity_id: 'FUND',
        instrument_id: 'A-B',
        instrument_spec: initialized.instrument.instrument,
        quantity: 2,
        unit: 'units',
      },
      {
        position_id: 'P-DIRECT',
        entity_id: 'FUND',
        instrument_id: 'A',
        instrument_spec: equity('A', 100),
        quantity: -2,
        unit: 'units',
      },
    ],
  });
  const ctx = facade.core.MarketContext.fromJson(market);
  try {
    const report = facade.portfolio.primitiveExposures(book, ctx, []);
    assert.equal(report.base_currency, 'USD');
    assert.equal(report.paths.length, 3);
    const byId = Object.fromEntries(report.aggregates.map((row) => [row.instrument_id, row]));
    assert.deepEqual(Object.keys(byId), ['A', 'B']);
    assert.equal(byId.A.net_quantity, 0);
    assert.equal(byId.A.gross_quantity, 4);
    assert.equal(Number(byId.A.gross_value.amount), 400);
    assert.equal(byId.B.net_quantity, -2);
    assert.throws(
      () => facade.portfolio.primitiveExposures(book, ctx, ['ytm']),
      (error) => error.name === 'FinstackError' && /non-additive/.test(error.message)
    );
  } finally {
    ctx.free();
    book.free();
  }
});

test('open-ended and partial-period calendar rules validate and rebalance on cadence', () => {
  const composite = facade.valuations.composite;
  const calendarRule = (end) => ({
    kind: 'calendar',
    start: '2024-01-02',
    ...(end === undefined ? {} : { end }),
    frequency: { count: 1, unit: 'months' },
    calendar_id: 'weekends_only',
    business_day_convention: 'following',
  });
  for (const end of [undefined, null, '2024-01-02', '2024-12-15']) {
    const ruled = { ...spec, rebalance_rule: calendarRule(end) };
    assert.equal(
      composite.initialize(ruled, market, '2024-01-02').instrument.instrument.type,
      'composite'
    );
  }
  const openEnded = {
    ...spec,
    weighting_method: {
      kind: 'notional_weighted',
      gross_notional: { amount: '100', currency: 'USD' },
    },
    rebalance_rule: calendarRule(undefined),
  };
  const observations = ['2024-01-02', '2024-01-20', '2024-02-02', '2024-02-15'].map((date) => ({
    date,
    state: market,
  }));
  const rows = composite.historyFromSpec(openEnded, observations);
  assert.deepEqual(
    rows.filter((row) => row.next_state_effective_date !== null).map((row) => row.date),
    ['2024-02-02']
  );
});

test('composite exposures and history price cs01 like priceInstrument', () => {
  const { calibration, valuations } = facade;
  const base = '2024-06-20';
  const envelope = JSON.parse(
    readFileSync(
      join(
        __dirname,
        '..',
        '..',
        '..',
        'finstack-quant',
        'calibration',
        'examples',
        'market_bootstrap',
        '03_single_name_hazard.json'
      ),
      'utf8'
    )
  );
  delete envelope.$schema;
  for (const step of envelope.plan.steps) {
    step.base_date = base;
    if (step.kind === 'hazard') {
      Object.assign(step, { id: 'CORP-HAZARD', curve_id: 'CORP-HAZARD', entity: 'CORP' });
    }
  }
  for (const quote of envelope.market_data) {
    if (quote.kind === 'cds_quote') quote.entity = 'CORP';
  }
  const hazardMarket = calibration.calibrate(envelope).result.final_market;
  const cds = JSON.parse(valuations.instruments.CreditDefaultSwap.example().toJson()).instrument;
  const cdsB = structuredClone(cds);
  cdsB.spec.id = `${cds.spec.id}-B`;
  const cdsSpec = {
    ...spec,
    capital: { amount: '1000000', currency: 'USD' },
    legs: [
      { instrument_id: cds.spec.id, instrument: cds, score: 1 },
      { instrument_id: cdsB.spec.id, instrument: cdsB, score: -0.5 },
    ],
  };
  const composite = valuations.composite;
  const instrument = composite.initialize(cdsSpec, hazardMarket, base).instrument;
  const expected = valuations.instruments.priceInstrument(
    instrument,
    hazardMarket,
    base,
    'default',
    ['cs01']
  ).measures.cs01;
  assert.notEqual(expected, 0);
  const report = composite.primitiveExposures(instrument, hazardMarket, base, ['cs01']);
  const total = report.aggregates.reduce((sum, row) => sum + row.net_measures.cs01, 0);
  assert.ok(Math.abs(total - expected) <= 1e-9 * Math.abs(expected));
  const observations = [{ date: base, state: hazardMarket }];
  assert.equal(composite.history(instrument, observations, ['cs01']).length, 1);
  assert.equal(composite.historyFromSpec(cdsSpec, observations, null, ['cs01']).length, 1);
  const weighted = {
    ...cdsSpec,
    weighting_method: {
      kind: 'metric_weighted',
      metric: 'cs01',
      anchor_leg_id: cds.spec.id,
      anchor_quantity: 1,
      neutralize: false,
    },
  };
  const weightedInstrument = composite.initialize(weighted, hazardMarket, base).instrument;
  assert.equal(
    composite.rebalance(weightedInstrument, hazardMarket, base).instrument.instrument.type,
    'composite'
  );
});
