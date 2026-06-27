/**
 * Return-floor MOIC/XIRR WASM facade tests.
 *
 * The return-floor feature is exposed via the existing JSON-native Bond
 * pricing path: `return_floor` is a serde field on the Rust Bond type, so it
 * round-trips through `validateInstrumentJson` and the four return metrics
 * (`moic`, `moic_to_worst`, `xirr`, `xirr_to_worst`) are computed via the
 * standard `priceInstrumentWithMetrics` entry-point.  No new Rust binding code
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

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/** Minimal 5-year flat discount market context for bond pricing. */
const MARKET_JSON = JSON.stringify({
  version: 2,
  curves: [
    {
      type: 'discount',
      id: 'USD-OIS',
      base: '2024-01-01',
      day_count: 'Act365F',
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
});

/**
 * 5-year 10% annual fixed-rate bullet bond spec.
 * Mirrors `_plain_bond_spec()` in `finstack-quant-py/tests/test_return_floor.py`.
 *
 * @param {object|null} returnFloor  Optional return_floor sub-object.
 * @returns {string}  Tagged InstrumentJson string.
 */
function bondInstrumentJson(returnFloor = null) {
  const spec = {
    id: 'WASM-RETURN-FLOOR-BOND',
    notional: { amount: '1000000', currency: 'USD' },
    issue_date: '2024-01-01',
    maturity: '2029-01-01',
    cashflow_spec: {
      Fixed: {
        rate: '0.10',
        freq: { count: 12, unit: 'months' },
        dc: 'Thirty360',
        bdc: 'following',
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
  return JSON.stringify({ type: 'bond', spec });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Price a bond and return the parsed ValuationResult.
 *
 * @param {string} instrumentJson  Tagged InstrumentJson string.
 * @param {string[]} metrics  Metric IDs to compute.
 * @returns {object}  Parsed ValuationResult.
 */
function priceWithMetrics(instrumentJson, metrics) {
  const resultJson = valuations.instruments.priceInstrumentWithMetrics(
    instrumentJson,
    MARKET_JSON,
    '2024-01-01',
    'discounting',
    metrics,
    null,
    null
  );
  return JSON.parse(resultJson);
}

function numericAmount(result) {
  const a = result?.value?.amount;
  return typeof a === 'number' ? a : parseFloat(a);
}

// ---------------------------------------------------------------------------
// Tests: metric registration
// ---------------------------------------------------------------------------

test('return-floor metrics appear in listStandardMetrics', () => {
  const metrics = valuations.instruments.listStandardMetrics();
  assert.ok(Array.isArray(metrics), 'listStandardMetrics returns an array');
  for (const id of ['moic', 'moic_to_worst', 'xirr', 'xirr_to_worst']) {
    assert.ok(metrics.includes(id), `'${id}' missing from standard metrics`);
  }
});

// ---------------------------------------------------------------------------
// Tests: JSON round-trip with return_floor
// ---------------------------------------------------------------------------

test('validateInstrumentJson accepts bond with Moic return_floor', () => {
  const inst = bondInstrumentJson({
    kind: { Moic: 1.25 },
    issue_price: 'Par',
    window: 'Full',
  });
  const canonical = valuations.instruments.validateInstrumentJson(inst);
  assert.ok(typeof canonical === 'string' && canonical.length > 0);
  const parsed = JSON.parse(canonical);
  assert.equal(parsed.type, 'bond');
  assert.ok(parsed.spec.return_floor != null, 'return_floor survives round-trip');
  assert.deepEqual(parsed.spec.return_floor.kind, { Moic: 1.25 });
});

test('validateInstrumentJson accepts bond with Xirr return_floor and PctOfPar issue_price', () => {
  const inst = bondInstrumentJson({
    kind: { Xirr: 0.12 },
    issue_price: { PctOfPar: 98.0 },
    window: { From: '2026-01-01' },
  });
  const canonical = valuations.instruments.validateInstrumentJson(inst);
  const parsed = JSON.parse(canonical);
  assert.ok(parsed.spec.return_floor != null, 'return_floor survives round-trip');
  assert.deepEqual(parsed.spec.return_floor.kind, { Xirr: 0.12 });
});

// ---------------------------------------------------------------------------
// Tests: metric computation
// ---------------------------------------------------------------------------

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
    bondInstrumentJson({ kind: { Moic: 1.25 }, issue_price: 'Par', window: 'Full' }),
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
    bondInstrumentJson({ kind: { Xirr: 0.12 }, issue_price: 'Par', window: 'Full' }),
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
    bondInstrumentJson({ kind: { Moic: 1.25 }, issue_price: 'Par', window: 'Full' }),
    ['moic']
  );
  assert.ok(
    Math.abs(floored.measures.moic - unfloored.measures.moic) < 1e-6,
    `Floored bullet MOIC (${floored.measures.moic}) should equal unfloored (${unfloored.measures.moic})`
  );
});
