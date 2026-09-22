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
const { core, valuations } = facade;

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

test('core ACT/ACT ICMA short-month rolls require a reference period', () => {
  const dayCount = core.DayCount.actActIsma();
  const context = new core.DayCountContext().withFrequency(core.Tenor.monthly());
  for (const [year, month, day] of [
    [2025, 2, 28],
    [2024, 2, 29],
    [2025, 4, 30],
  ]) {
    const start = core.createDate(year, month, day);
    const end = core.createDate(year, month + 1, 31);
    assert.throws(() => dayCount.yearFractionWithContext(start, end, context), /coupon_period/);
    const reference = context.withCouponPeriod(start, end);
    assert.ok(Math.abs(dayCount.yearFractionWithContext(start, end, reference) - 1 / 12) < 1e-12);
  }
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

test('ForwardCurve options expose resetLag', () => {
  const curve = new core.ForwardCurve({
    id: 'USD-SOFR',
    tenor: 0.25,
    baseDate: '2025-01-01',
    knots: [0, 0.04, 1, 0.045],
    dayCount: 'act_360',
    interp: 'linear',
    extrapolation: 'flat_forward',
    resetLag: 3,
  });
  assert.equal(curve.resetLag, 3);
});

test('ForwardCurve options accept typed arrays', () => {
  const curve = new core.ForwardCurve({
    id: 'USD-SOFR',
    tenor: 0.25,
    baseDate: '2025-01-01',
    knots: new Float64Array([0, 0.04, 1, 0.045]),
    dayCount: 'act_360',
  });
  assert.equal(curve.rate(1), 0.045);
});

test('coupon profile variants use separate explicit entrypoints', () => {
  const snowball = valuations.snowballCouponProfile(0.02, 0.05, [0.01, 0.04], 0, 0.1);
  const inverse = valuations.inverseFloaterCouponProfile(0.05, [0.01, 0.02], 0, 0.1, 2);
  assert.ok(Math.abs(snowball[0] - 0.06) < 1e-12);
  assert.ok(Math.abs(snowball[1] - 0.07) < 1e-12);
  assert.ok(Math.abs(inverse[0] - 0.03) < 1e-12);
  assert.ok(Math.abs(inverse[1] - 0.01) < 1e-12);
});

test('core VolCube is a data-only artifact', () => {
  const cube = new core.VolCube('NORMAL', [1], [2], [0.01, 0, -0.2, 0.4, Number.NaN], [0.02]);
  assert.equal(cube.id, 'NORMAL');
  assert.equal(cube.interpolationMode, 'vol');
  for (const name of ['vol', 'volClamped', 'volNormal', 'volNormalClamped']) {
    assert.equal(name in cube, false, `obsolete core evaluator ${name} remains`);
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

test('core FX pair convention helpers', () => {
  const pair = core.fxMarketPair('USD', 'EUR');
  assert.equal(pair[0].code, 'EUR');
  assert.equal(pair[1].code, 'USD');
  const conv = core.fxPairConvention('USD', 'JPY');
  assert.equal(conv.base.code, 'USD');
  assert.equal(conv.quote.code, 'JPY');
  assert.equal(conv.usdQuotation.toString(), 'indirect');
  assert.equal(conv.pipSize, 0.01);
  assert.equal(conv.spotLagDays, 2);
  assert.equal(core.fxPipSize('EUR', 'USD'), 0.0001);
  assert.ok(Math.abs(core.invertFxRate(1.1) - 1 / 1.1) < 1e-12);
  assert.equal(core.FxQuoteConvention.direct().toString(), 'direct');
});

test('core.FxMatrix quote updates invalidate cached crosses', () => {
  for (const pinned of [false, true]) {
    const fx = new core.FxMatrix();
    const policy = core.FxConversionPolicy.cashflowDate();
    const date = '2025-01-02';
    fx.setQuote('GBP', 'USD', 1.25);
    const setEurUsd = (rate) =>
      pinned ? fx.setQuoteOn('EUR', 'USD', date, policy, rate) : fx.setQuote('EUR', 'USD', rate);
    setEurUsd(1.1);
    assert.ok(Math.abs(fx.rateDefault('EUR', 'GBP', date).rate - 0.88) < 1e-12);
    setEurUsd(1.2);
    const result = fx.rateDefault('EUR', 'GBP', date);
    assert.ok(Math.abs(result.rate - 0.96) < 1e-12);
    assert.equal(result.triangulated, true);
    assert.ok(Math.abs(fx.rateDefault('GBP', 'EUR', date).rate - 1 / 0.96) < 1e-12);
    assert.equal(fx.rateDefault('GBP', 'USD', date).rate, 1.25);
  }
});

test('core.Money.fromDecimalStr accepts exact fixed and scientific text', () => {
  for (const [text, expected, currency] of [
    ['12345678901234567890.12345', '12345678901234567890.12345', 'USD'],
    ['1.2345e3', '1234.5', 'EUR'],
    ['0.1e29', '10000000000000000000000000000', 'USD'],
    ['-7.9228162514264337593543950335e28', '-79228162514264337593543950335', 'USD'],
    ['0e999999', '0', 'USD'],
  ]) {
    const m = core.Money.fromDecimalStr(text, currency);
    try {
      assert.equal(m.amountDecimal(), expected);
      assert.equal(m.currency.code, currency);
    } finally {
      m.free();
    }
  }
  for (const [text, formatted] of [
    ['1.00e-27', '0.0000000000000000000000000010'],
    ['2000e-31', '0.0000000000000000000000000002'],
  ]) {
    const m = core.Money.fromDecimalStr(text, 'USD');
    try {
      assert.equal(m.formatWith(28, false), formatted);
    } finally {
      m.free();
    }
  }
});

test('core.Money.fromDecimalStr rejects inexact amounts', () => {
  for (const text of [
    '1.24500000000000000000000000001',
    '1.23450000000000000000000000001e3',
    'NaN',
    '1e-29',
    '1e29',
    '1.23e-28',
  ]) {
    assert.throws(() => core.Money.fromDecimalStr(text, 'USD'), /exactly representable/);
  }
  assert.throws(() => core.Money.fromDecimalStr('1.0', 'NOPE'));
});

test('core.Money formatWith honours returned rounding modes', () => {
  for (const [mode, amount, expected] of [
    ['bankers', '1.245', 'USD 1.24'],
    ['bankers', '1.255', 'USD 1.26'],
    ['away_from_zero', '-1.245', 'USD -1.25'],
    ['away_from_zero', '1.241', 'USD 1.24'],
    ['toward_zero', '-1.259', 'USD -1.25'],
    ['floor', '-1.241', 'USD -1.25'],
    ['floor', '1.249', 'USD 1.24'],
    ['ceil', '1.241', 'USD 1.25'],
    ['ceil', '-1.249', 'USD -1.24'],
  ]) {
    const m = core.Money.fromDecimalStr(amount, 'USD');
    try {
      assert.equal(m.formatWith(2, true, ',', mode), expected);
      assert.equal(m.amountDecimal(), amount);
    } finally {
      m.free();
    }
  }
});

test('core.Money formatWith applies native defaults and JPY zero scale', () => {
  const usd = core.Money.fromDecimalStr('1234.567', 'USD');
  try {
    assert.equal(usd.formatWith(), 'USD 1234.57');
    assert.equal(usd.formatWith(2, true, ','), 'USD 1,234.57');
  } finally {
    usd.free();
  }
  const jpy = core.Money.fromDecimalStr('1.5', 'JPY');
  try {
    assert.equal(jpy.formatWith(0), 'JPY 2');
  } finally {
    jpy.free();
  }
});

test('core.Money formatWith rejects invalid options and still frees the handle', () => {
  const m = core.Money.fromDecimalStr('1.245', 'USD');
  try {
    for (const decimals of [-1, 1.5, Number.NaN, Infinity, '2', 1_000_001]) {
      assert.throws(
        () => m.formatWith(decimals),
        /non-negative integer|precision/,
        `decimals=${decimals}`
      );
    }
    assert.throws(() => m.formatWith(2, true, ',,'), /single character/);
    assert.throws(() => m.formatWith(2, true, ',', 'half_up'));
  } finally {
    m.free();
  }
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
