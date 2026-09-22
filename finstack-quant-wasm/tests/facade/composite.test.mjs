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
    shares: 1,
    price_quote: price,
    price_id: null,
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
    { instrument_id: 'A', instrument: equity('A', 100), weight: 1 },
    { instrument_id: 'B', instrument: equity('B', 90), weight: -1 },
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
      assert.match(error.message, /expected composite/);
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
