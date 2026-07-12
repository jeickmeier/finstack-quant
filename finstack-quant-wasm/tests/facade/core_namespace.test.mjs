/**
 * Core-namespace facade smoke tests.
 *
 * Loads the public facade (`index.js` + `exports/core.js`), initializes the
 * web-target wasm module from bytes (Node has no `fetch`-able URL), and
 * exercises a minimal slice of `core`: Currency, Money (including the
 * lossless `amountDecimal()` accessor), FxDeltaVolSurface construction, and
 * the FxRateResult `rate` / `triangulated` getters.
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
const { core } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

test('core namespace exposes expected constructors', () => {
  assert.equal(typeof core.Currency, 'function');
  assert.equal(typeof core.Money, 'function');
  assert.equal(typeof core.FxDeltaVolSurface, 'function');
  assert.equal(typeof core.FxMatrix, 'function');
});

test('core.Currency creation', () => {
  const usd = new core.Currency('USD');
  assert.equal(usd.code, 'USD');
});

test('core.Money amount and lossless amountDecimal', () => {
  const usd = new core.Currency('USD');
  const m = new core.Money(1234.56, usd);
  assert.equal(m.amount, 1234.56);
  assert.equal(typeof m.amountDecimal(), 'string');
  assert.equal(m.amountDecimal(), '1234.56');
  const eur = new core.Currency('EUR');
  const converted = m.convertAtRate(eur, 0.9);
  assert.equal(converted.currency.code, 'EUR');
  assert.equal(converted.amount, 1111.104);
  const subCent = new core.Money(1.2345, usd);
  assert.equal(subCent.amountDecimal(), '1.2345');
});

test('core date integer widths match generated runtime types', () => {
  const start = core.createDate(2025, 1, 1);
  const end = core.createDate(2025, 1, 3);
  assert.ok(core.dateFromEpochDays(start) instanceof Int32Array);
  assert.equal(typeof core.DayCount.act360().calendarDays(start, end), 'bigint');
});

test('wasm-bindgen handles expose free and conditional Symbol.dispose', () => {
  const usd = new core.Currency('USD');
  assert.equal(typeof usd.free, 'function');
  if (Symbol.dispose) {
    assert.equal(usd[Symbol.dispose], usd.free);
  }
  usd.free();
});

test('DiscountCurve uses canonical forward and explicit negative-rate validation', () => {
  assert.throws(
    () => new core.DiscountCurve('CHF-OIS', '2025-01-01', [0, 1, 1, 1.002]),
    /non-increasing/
  );
  const curve = new core.DiscountCurve(
    'CHF-OIS',
    '2025-01-01',
    [0, 1, 1, 1.002],
    undefined,
    undefined,
    undefined,
    'negative_rate_friendly',
    -0.01
  );
  assert.ok(curve.forward(0, 1) < 0);
  assert.equal(curve.forwardRate, undefined);
});

test('ForwardCurve exposes resetLag without changing prior positional arguments', () => {
  const curve = new core.ForwardCurve(
    'USD-SOFR',
    0.25,
    '2025-01-01',
    [0, 0.04, 1, 0.045],
    'act_360',
    'linear',
    'flat_forward',
    undefined,
    3
  );
  assert.equal(curve.resetLag, 3);
});

test('VolCube canonical camelCase methods exist at runtime', () => {
  const cube = new core.VolCube('NORMAL', [1], [2], [0.01, 0, -0.2, 0.4, Number.NaN], [0.02]);
  for (const name of ['volClamped', 'volNormal', 'volNormalClamped']) {
    assert.equal(typeof cube[name], 'function', `missing ${name}`);
  }
});

test('core.FxDeltaVolSurface constructs from 25-delta quotes', () => {
  const surface = new core.FxDeltaVolSurface(
    'EURUSD-VOL',
    [0.25, 0.5, 1.0],
    [0.1, 0.11, 0.12],
    [-0.01, -0.012, -0.015],
    [0.002, 0.0025, 0.003]
  );
  assert.equal(surface.id, 'EURUSD-VOL');
  assert.equal(surface.numExpiries, 3);
});

test('core.FxMatrix rate returns FxRateResult with rate/triangulated getters', () => {
  const fx = new core.FxMatrix();
  const policy = core.FxConversionPolicy.cashflowDate();
  fx.setQuoteOn('EUR', 'USD', '2025-01-15', policy, 1.1);
  const result = fx.rate('EUR', 'USD', '2025-01-15', policy);
  assert.equal(typeof result.rate, 'number');
  assert.equal(result.rate, 1.1);
  assert.equal(typeof result.triangulated, 'boolean');
  assert.equal(result.triangulated, false);
  const reused = fx.rate('EUR', 'USD', '2025-01-15', policy);
  assert.equal(reused.rate, 1.1);
  assert.match(policy.toString(), /cashflow/i);
});
