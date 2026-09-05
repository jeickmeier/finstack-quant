/**
 * Return-floor MOIC/XIRR WASM facade tests.
 *
 * The return-floor feature is exposed via the existing JSON-native Bond
 * pricing path: `return_floor` is a serde field on the Rust Bond type, so it
 * round-trips through `validateInstrumentJson` and the four return metrics
 * (`moic`, `moic_to_worst`, `xirr`, `xirr_to_worst`) are computed via the
 * standard `priceInstrument` entry-point.  No new Rust binding code
 * is required.
 *
 * The bond fixture is the same 5-year 10% annual bullet used in
 * `finstack-quant-py/tests/test_return_floor.py` so the two surfaces stay
 * directly comparable (cross-language determinism).
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
const { valuations } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

// Fixtures

/** Minimal 5-year flat discount market context for bond pricing. */
const MARKET_JSON = JSON.stringify({
  schema_version: 1,
  curves: [
    {
      type: 'discount',
      id: 'USD-OIS',
      base: '2024-01-01',
      day_count: 'act_365f',
      knot_points: [
        [0.0, 1.0],
        [5.0, 0.85],
      ],
      interp_style: 'monotone_convex',
      extrapolation: 'flat_forward',
      min_forward_rate: null,
      allow_non_monotonic: false,
      min_forward_tenor: 1e-6,
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
});

/**
 * 5-year 10% annual fixed-rate bullet bond spec.
 * Mirrors `_plain_bond_spec()` in `finstack-quant-py/tests/test_return_floor.py`.
 *
 * @param {object|null} returnFloor  Optional return_floor sub-object.
 * @returns {string}  Canonical instrument-envelope JSON.
 */
function bondInstrumentJson(returnFloor = null) {
  const spec = {
    id: 'WASM-RETURN-FLOOR-BOND',
    notional: { amount: '1000000', currency: 'USD' },
    issue_date: '2024-01-01',
    maturity: '2029-01-01',
    cashflow_spec: {
      fixed: {
        rate: '0.10',
        frequency: { count: 12, unit: 'months' },
        day_count: '30_360',
        business_day_convention: 'following',
        calendar_id: 'weekends_only',
      },
    },
    discount_curve_id: 'USD-OIS',
    settlement_days: 0,
    ex_coupon_days: 0,
    attributes: {},
  };
  if (returnFloor !== null) {
    spec.return_floor = returnFloor;
  }
  return JSON.stringify({
    schema: 'finstack_quant.instrument/1',
    instrument: { type: 'bond', spec },
  });
}

/**
 * Price a bond and return the structured ValuationResult object.
 *
 * @param {string} instrumentJson  Canonical instrument-envelope JSON.
 * @param {string[]} metrics  Metric IDs to compute.
 * @returns {object}  ValuationResult object (no JSON.parse — the binding
 *   returns a plain JS object, matching the Python `ValuationResult`).
 */
function priceWithMetrics(instrumentJson, metrics) {
  return valuations.instruments.priceInstrument(
    instrumentJson,
    MARKET_JSON,
    '2024-01-01',
    'tree',
    metrics,
    null,
    null
  );
}

function numericAmount(result) {
  const a = result?.value?.amount;
  return typeof a === 'number' ? a : parseFloat(a);
}

// Tests: structured (non-JSON-string) pricing returns

test('priceInstrument returns a plain ValuationResult object, not a JSON string', () => {
  const result = valuations.instruments.priceInstrument(
    bondInstrumentJson(),
    MARKET_JSON,
    '2024-01-01',
    'discounting'
  );
  assert.equal(typeof result, 'object', 'priceInstrument returns an object');
  assert.ok(!(result instanceof Map), 'result must not be an ES2015 Map');
  // Fields are readable directly — parity with Python's ValuationResult getters.
  assert.equal(result.instrument_id, 'WASM-RETURN-FLOOR-BOND');
  assert.equal(typeof result.value.currency, 'string');
  assert.equal(typeof result.measures, 'object');
  assert.ok(!(result.measures instanceof Map), 'measures must not be an ES2015 Map');
  // A Map would stringify to `{}`; a plain object round-trips losslessly.
  const roundTripped = JSON.parse(JSON.stringify(result));
  assert.equal(roundTripped.instrument_id, result.instrument_id);
  assert.equal(roundTripped.as_of, result.as_of);
  assert.ok(numericAmount(roundTripped) > 0, 'round-tripped value survives stringify');
});

test('priceInstrument returns readable measures without JSON.parse', () => {
  const result = priceWithMetrics(bondInstrumentJson(), ['moic', 'xirr']);
  assert.equal(typeof result, 'object');
  assert.ok(!(result instanceof Map));
  assert.equal(typeof result.measures.moic, 'number', 'measures.moic is directly readable');
  const roundTripped = JSON.parse(JSON.stringify(result));
  assert.equal(roundTripped.measures.moic, result.measures.moic);
});

test('priceInstrumentWithMarket / …WithMetricsAndMarket return objects too', () => {
  const market = new valuations.Market(MARKET_JSON);
  try {
    const priced = valuations.instruments.priceInstrumentWithMarket(
      bondInstrumentJson(),
      market,
      '2024-01-01',
      'discounting'
    );
    assert.equal(typeof priced, 'object');
    assert.ok(!(priced instanceof Map));
    assert.equal(priced.instrument_id, 'WASM-RETURN-FLOOR-BOND');

    const withMetrics = valuations.instruments.priceInstrumentWithMarket(
      bondInstrumentJson(),
      market,
      '2024-01-01',
      'discounting',
      ['moic'],
      null,
      null
    );
    assert.equal(typeof withMetrics, 'object');
    assert.equal(typeof withMetrics.measures.moic, 'number');
    assert.equal(JSON.parse(JSON.stringify(withMetrics)).measures.moic, withMetrics.measures.moic);
  } finally {
    market.free();
  }
});

test('validateValuationResultJson still accepts a stringified price result', () => {
  const result = valuations.instruments.priceInstrument(
    bondInstrumentJson(),
    MARKET_JSON,
    '2024-01-01',
    'discounting'
  );
  const canonical = valuations.validateValuationResultJson(JSON.stringify(result));
  assert.equal(typeof canonical, 'string');
  assert.equal(JSON.parse(canonical).instrument_id, 'WASM-RETURN-FLOOR-BOND');
});

// Tests: metric registration

test('return-floor metrics appear in listStandardMetrics', () => {
  const metrics = valuations.instruments.listStandardMetrics();
  assert.ok(Array.isArray(metrics), 'listStandardMetrics returns an array');
  for (const id of ['moic', 'moic_to_worst', 'xirr', 'xirr_to_worst']) {
    assert.ok(metrics.includes(id), `'${id}' missing from standard metrics`);
  }
});

// Tests: JSON round-trip with return_floor

test('validateInstrumentJson accepts bond with MOIC return_floor', () => {
  const inst = bondInstrumentJson({
    kind: { moic: 1.25 },
    issue_price: 'par',
    window: 'full',
  });
  const canonical = valuations.instruments.validateInstrumentJson(inst);
  assert.ok(typeof canonical === 'string' && canonical.length > 0);
  const parsed = JSON.parse(canonical);
  assert.equal(parsed.instrument.type, 'bond');
  assert.ok(parsed.instrument.spec.return_floor != null, 'return_floor survives round-trip');
  assert.deepEqual(parsed.instrument.spec.return_floor.kind, { moic: 1.25 });
});

test('validateInstrumentJson accepts bond with XIRR return_floor and pct_of_par issue_price', () => {
  const inst = bondInstrumentJson({
    kind: { xirr: 0.12 },
    issue_price: { pct_of_par: 98.0 },
    window: { from: '2026-01-01' },
  });
  const canonical = valuations.instruments.validateInstrumentJson(inst);
  const parsed = JSON.parse(canonical);
  assert.ok(parsed.instrument.spec.return_floor != null, 'return_floor survives round-trip');
  assert.deepEqual(parsed.instrument.spec.return_floor.kind, { xirr: 0.12 });
});

// Tests: metric computation

test('10% 5Y par bullet: MOIC ≈ 1.50 (5 × 0.10 coupons + 1.0 principal)', () => {
  const result = priceWithMetrics(bondInstrumentJson(), ['moic', 'moic_to_worst', 'xirr']);
  const moic = result.measures.moic;
  assert.equal(typeof moic, 'number', 'moic is a number');
  assert.ok(moic > 1.0, `MOIC must be > 1.0, got ${moic}`);
  // 5 × 0.10 + 1.0 = 1.50 undiscounted (tight tolerance)
  assert.ok(Math.abs(moic - 1.5) < 0.02, `MOIC ≈ 1.50, got ${moic}`);
});

test('10% 5Y par bullet: XIRR ≈ 10% (equals coupon rate at par)', () => {
  const result = priceWithMetrics(bondInstrumentJson(), ['xirr', 'xirr_to_worst']);
  const xirr = result.measures.xirr;
  assert.equal(typeof xirr, 'number', 'xirr is a number');
  assert.ok(Math.abs(xirr - 0.1) < 0.005, `XIRR ≈ 0.10, got ${xirr}`);
});

test('plain bullet: moic_to_worst == moic (single exit path, no calls)', () => {
  const result = priceWithMetrics(bondInstrumentJson(), ['moic', 'moic_to_worst']);
  const { moic, moic_to_worst } = result.measures;
  assert.ok(
    Math.abs(moic - moic_to_worst) < 1e-6,
    `Bullet: moic_to_worst must equal moic; got ${moic_to_worst} vs ${moic}`
  );
});

test('floored bond (1.25× MOIC): prices successfully and moic_to_worst ≤ moic', () => {
  const result = priceWithMetrics(
    bondInstrumentJson({ kind: { moic: 1.25 }, issue_price: 'par', window: 'full' }),
    ['moic', 'moic_to_worst', 'xirr', 'xirr_to_worst']
  );
  // Price > 0
  assert.ok(numericAmount(result) > 0, 'floored bond price must be positive');
  // All four metrics present and numeric
  for (const k of ['moic', 'moic_to_worst', 'xirr', 'xirr_to_worst']) {
    assert.equal(typeof result.measures[k], 'number', `${k} must be a number`);
  }
  // moic_to_worst ≤ moic (worst-case is never better than to-maturity)
  assert.ok(
    result.measures.moic_to_worst <= result.measures.moic + 1e-9,
    `moic_to_worst must be ≤ moic; got ${result.measures.moic_to_worst} vs ${result.measures.moic}`
  );
});

test('floored bond (12% XIRR): prices successfully and xirr metric is positive', () => {
  const result = priceWithMetrics(
    bondInstrumentJson({ kind: { xirr: 0.12 }, issue_price: 'par', window: 'full' }),
    ['xirr', 'xirr_to_worst']
  );
  assert.ok(numericAmount(result) > 0, 'xirr-floored bond price must be positive');
  assert.ok(result.measures.xirr > 0, `xirr must be positive, got ${result.measures.xirr}`);
});

test('floored bond: MOIC metric matches unfloored bullet (floor only affects early calls)', () => {
  // A return-floor on a bullet bond (no embedded calls) does not change the
  // held-to-maturity MOIC: the floor only governs EARLY redemption pricing.
  const unfloored = priceWithMetrics(bondInstrumentJson(), ['moic']);
  const floored = priceWithMetrics(
    bondInstrumentJson({ kind: { moic: 1.25 }, issue_price: 'par', window: 'full' }),
    ['moic']
  );
  assert.ok(
    Math.abs(floored.measures.moic - unfloored.measures.moic) < 1e-6,
    `Floored bullet MOIC (${floored.measures.moic}) should equal unfloored (${unfloored.measures.moic})`
  );
});
