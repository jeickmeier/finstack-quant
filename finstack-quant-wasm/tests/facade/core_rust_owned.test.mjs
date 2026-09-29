/**
 * Core primitives, dates and market data: behaviour owned by Rust, identical
 * in both hosts.
 *
 * Every value and message below is produced by the Rust crates (currency
 * parser, `Money::checked_neg`, `try_repr_div_f64`, `RoundingMode::FromStr`,
 * `DayCount::*year_fraction(start, end, ctx)`, `FxDeltaVolSurface::new`,
 * `DayCountContextState::try_new`, `HazardCurveBuilder::build`), not by the
 * bindings. `finstack-quant-py/tests/test_core_rust_owned.py` asserts the same
 * strings for the Python twins.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const validation = (message) => (error) => {
  assert.ok(error instanceof Error, `expected an Error, got ${typeof error}: ${error}`);
  assert.equal(error.kind, 'validation', `kind of: ${error.message}`);
  assert.equal(error.message, message);
  return true;
};

test('one Rust currency parser: case-insensitive, not trimmed, error names the code', () => {
  assert.equal(new core.Currency('usd').code, 'USD');
  assert.equal(new core.Currency('usd').numeric, 840);
  for (const [build, code] of [
    [() => new core.Currency(' USD '), ' USD '],
    [() => new core.Currency('XXX'), 'XXX'],
    [() => core.Money.fromDecimalStr('1', ' usd '), ' usd '],
    [() => core.fxPipSize(' usd', 'EUR'), ' usd'],
  ]) {
    assert.throws(
      build,
      validation(`Invalid currency code "${code}": not a supported ISO-4217 alphabetic code`)
    );
  }
});

test('Money.negate is the exact Rust checked_neg and cannot throw', () => {
  const usd = new core.Currency('USD');
  const cases = [
    [new core.Money(0, usd), '{"amount":"-0","currency":"USD"}'],
    [core.Money.fromDecimalStr('0.00', 'USD'), '{"amount":"-0.00","currency":"USD"}'],
    [core.Money.fromDecimalStr('1.2500', 'USD'), '{"amount":"-1.2500","currency":"USD"}'],
  ];
  for (const [money, expected] of cases) {
    const negated = money.negate();
    assert.equal(negated.toJson(), expected);
    assert.equal(negated.negate().toJson(), money.toJson());
  }
  assert.equal(new core.Money(0, usd).negate().toString(), 'USD -0.00');
});

test('zero scalar division is the Rust validation error', () => {
  const m = new core.Money(10, new core.Currency('USD'));
  assert.throws(() => m.divScalar(0), validation('Validation error: division by zero'));
});

test('formatWith parses rounding with the serde names and defaults in Rust', () => {
  const m = core.Money.fromDecimalStr('2.345', 'USD');
  assert.equal(m.formatWith(2), m.formatWith(2, true, null, 'bankers'));
  assert.throws(
    () => m.formatWith(2, true, null, 'BANKERS'),
    validation(
      'Validation error: invalid value "BANKERS": unknown variant `BANKERS`, expected one of ' +
        '`bankers`, `away_from_zero`, `toward_zero`, `floor`, `ceil`'
    )
  );
});

test('yearFraction and signedYearFraction take an optional context', () => {
  const start = core.createDate(2024, 3, 1);
  const end = core.createDate(2024, 9, 1);
  const act365l = core.DayCount.act365l();
  const semiAnnual = new core.DayCountContext().withFrequency(core.Tenor.semiAnnual());
  // No context: annual Act/365L; a 6M context changes the denominator.
  assert.equal(act365l.yearFraction(start, end), 0.5041095890410959);
  assert.equal(act365l.yearFraction(start, end, null), 0.5041095890410959);
  assert.equal(act365l.yearFraction(start, end, semiAnnual), 0.5027322404371585);
  assert.equal(act365l.signedYearFraction(end, start, semiAnnual), -0.5027322404371585);
  assert.equal(act365l.signedYearFraction(end, start), -0.5041095890410959);

  const bus252 = core.DayCount.bus252();
  const nyse = new core.DayCountContext().withCalendar('nyse');
  assert.equal(bus252.yearFraction(start, end, nyse), 0.503968253968254);
  assert.equal(bus252.signedYearFraction(end, start, nyse), -0.503968253968254);
  assert.throws(
    () => bus252.signedYearFraction(end, start),
    validation('DayCount::Bus252 requires a holiday calendar in DayCountContext')
  );
  // The borrowed context is still usable after the call.
  assert.equal(bus252.yearFraction(start, end, nyse), 0.503968253968254);
  assert.equal('yearFractionWithContext' in act365l, false);
});

test('DayCountContext.withCouponPeriod validates through DayCountContextState::try_new', () => {
  assert.throws(
    () => new core.DayCountContext().withCouponPeriod(10, 5),
    validation(
      'Validation error: coupon period start must be before end, got start=1970-01-11 end=1970-01-06'
    )
  );
});

test('FxMatrix.rate takes an optional policy; rateDefault is gone', () => {
  const fx = new core.FxMatrix();
  fx.setQuote('EUR', 'USD', 1.1);
  const cashflowDate = core.FxConversionPolicy.cashflowDate();
  assert.equal(fx.rate('EUR', 'USD', '2025-01-02').rate, 1.1);
  assert.equal(fx.rate('EUR', 'USD', '2025-01-02', null).rate, 1.1);
  assert.equal(fx.rate('EUR', 'USD', '2025-01-02', cashflowDate).rate, 1.1);
  assert.equal('rateDefault' in fx, false);
  assert.equal('rateWithDefaultPolicy' in fx, false);
});

test('FxDeltaVolSurface wing pairing is the Rust constructor check', () => {
  const message = 'Validation error: rr_10d and bf_10d must both be provided or both omitted';
  assert.throws(
    () => new core.FxDeltaVolSurface('X', [1], [0.1], [0.01], [0.005], [0.02]),
    validation(message)
  );
  assert.throws(
    () => new core.FxDeltaVolSurface('X', [1], [0.1], [0.01], [0.005], null, [0.01]),
    validation(message)
  );
});

test('VolCube keeps the Rust from_grid interpolation default when omitted', () => {
  const params = [0.01, 0.5, -0.2, 0.4, Number.NaN];
  assert.equal(new core.VolCube('C', [1], [2], params, [0.02]).interpolationMode, 'vol');
  assert.equal(
    new core.VolCube('C', [1], [2], params, [0.02], 'total_variance').interpolationMode,
    'total_variance'
  );
});

test('HazardCurve recovery errors and precedence come from the Rust builder', () => {
  assert.throws(
    () => new core.HazardCurve('HZ', '2025-01-01', [1.0, 0.01, 5.0, 0.02], 1.5),
    validation('Validation error: recovery_rate must be a decimal fraction in [0, 1], got 1.5')
  );
  assert.throws(
    () => new core.HazardCurve('HZ', '2025-01-01', [1.0, -0.02], 1.5),
    validation('Values must be non-negative')
  );
});
