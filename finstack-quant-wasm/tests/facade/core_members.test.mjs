/**
 * Core handle members and JSON round-trips bound in S20: Rate/Bps/Percentage,
 * DayCount (`fromName`, `parse`, `nl365`), Tenor, ForwardCurve, VolCube,
 * FxDeltaVolSurface, FxRateResult and the realized-variance estimators.
 *
 * Every value is computed by the Rust crates. The numeric goldens are shared
 * with `finstack-quant-py/tests/test_core_members_parity.py`.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

// Shared with test_core_members_parity.py.
const PRICES = [100.0, 101.0, 99.5, 100.2, 102.0];
const OHLC = {
  open: [100.0, 101.0, 99.5, 100.2],
  high: [101.5, 102.0, 100.8, 101.0],
  low: [99.0, 99.2, 98.9, 99.6],
  close: [101.0, 99.5, 100.2, 100.6],
};

test('realizedVariance and realizedVarianceOhlc take the Rust defaults', () => {
  assert.equal(core.realizedVariance(PRICES), 0.04341008921510013);
  assert.equal(core.realizedVariance(PRICES, null, null), 0.04341008921510013);
  assert.equal(core.realizedVariance(PRICES, 'close_to_close', 52), 0.008957637457084153);
  const { open, high, low, close } = OHLC;
  // The OHLC default estimator is Yang-Zhang (RealizedVarMethod::OHLC_DEFAULT).
  assert.equal(core.realizedVarianceOhlc(open, high, low, close), 0.04489594471070698);
  assert.equal(
    core.realizedVarianceOhlc(open, high, low, close, 'yang_zhang'),
    0.04489594471070698
  );
  assert.equal(core.realizedVarianceOhlc(open, high, low, close, 'parkinson'), 0.04439216444474536);
  assert.throws(
    () => core.realizedVariance(PRICES, 'parkinson'),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () => core.realizedVariance(PRICES, 'nope'),
    (error) => error.kind === 'validation'
  );
});

test('Rate, Bps and Percentage: parse, getters, conversions and JSON', () => {
  const rate = core.Rate.parse('12.5bp');
  assert.equal(rate.asDecimal, 0.00125);
  assert.equal(new core.Percentage(0.175).asBp, 18);
  assert.equal(new core.Bps(-1995).asPercent, -19.95);
  const bps = new core.Bps(125);
  assert.equal(typeof bps.asDecimal, 'number');
  assert.equal(bps.asBp, 125);
  assert.equal(bps.asRate.asDecimal, 0.0125);
  assert.equal(bps.asPercentage.asPercent, 1.25);
  assert.equal(new core.Percentage(5).asRate.asDecimal, 0.05);
  assert.equal(new core.Percentage(5).asBasisPoints.asBp, 500);
  assert.equal(new core.Rate(0.05).asBasisPoints.asBp, 500);
  assert.equal(new core.Rate(0.05).asPercentage.asPercent, 5);
  assert.equal(new core.Rate(-0.01).abs().asDecimal, 0.01);
  assert.equal(new core.Rate(0).isZero(), true);
  assert.equal(new core.Bps(-5).isNegative(), true);
  assert.equal(new core.Percentage(1).isPositive(), true);
  assert.equal(new core.Rate(0.0525).toJson(), '0.0525');
  assert.equal(new core.Bps(25).toJson(), '25');
  assert.equal(new core.Percentage(5).toJson(), '5.0');
  assert.equal(core.Rate.fromJson('0.0525').asDecimal, 0.0525);
  assert.equal(core.Bps.fromJson('25').asBp, 25);
  assert.equal(core.Percentage.fromJson('5.0').asPercent, 5);
  assert.throws(
    () => core.Rate.parse('abc'),
    (error) => error.kind === 'validation'
  );
});

test('DayCount.fromName is strict, DayCount.parse is the lenient Rust parser', () => {
  assert.equal(core.DayCount.fromName('act_360').toString(), 'act_360');
  assert.throws(
    () => core.DayCount.fromName('ACT/360'),
    (error) => error.kind === 'validation'
  );
  assert.equal(core.DayCount.parse('Act/Act ICMA').toString(), 'act_act_isma');
  assert.equal(core.DayCount.parse('ACT/360').toString(), 'act_360');
  assert.equal(core.DayCount.nl365().toString(), 'nl_365');
  assert.equal(typeof core.DayCount.fromName, 'function');
});

test('Tenor members pass through the Rust Tenor', () => {
  const t = core.Tenor.parse('3M');
  assert.equal(t.unit, 'M');
  assert.equal(t.months, 3);
  assert.equal(t.days, undefined);
  assert.equal(t.paymentsPerYear(), 4);
  assert.equal(t.toDaysApprox(), 91);
  assert.equal(core.Tenor.biweekly().days, 14);
  assert.equal(core.Tenor.bimonthly().months, 2);
  assert.equal(core.Tenor.fromPaymentsPerYear(4).toString(), '3M');
  assert.equal(core.Tenor.fromYears(0.5, core.DayCount.act365f()).toString(), '6M');
  const oneMonth = new core.Tenor('1M');
  // Month-end clamp; no calendar means no roll.
  assert.deepEqual(
    [...core.dateFromEpochDays(oneMonth.addToDate(core.createDate(2025, 1, 31)))],
    [2025, 2, 28]
  );
  // With a calendar and no convention: the Rust default, modified following.
  assert.deepEqual(
    [...core.dateFromEpochDays(oneMonth.addToDate(core.createDate(2025, 5, 31), 'nyse'))],
    [2025, 6, 30]
  );
  assert.equal(
    new core.Tenor('6M').toYearsWithContext(core.createDate(2025, 1, 15), core.DayCount.act360()),
    0.5027777777777778
  );
  assert.throws(
    () => core.Tenor.fromPaymentsPerYear(5),
    (error) => error.kind === 'validation'
  );
});

test('ForwardCurve flat, queries, getters and JSON round trip', () => {
  const curve = core.ForwardCurve.flat('USD-3M', 0.25, '2025-01-02', 0.04);
  assert.equal(curve.tenor, 0.25);
  assert.equal(typeof curve.dayCount, 'string');
  assert.equal(typeof curve.interpStyle, 'string');
  assert.equal(typeof curve.extrapolation, 'string');
  assert.ok(curve.knots instanceof Float64Array);
  assert.ok(curve.forwards.every((f) => f === 0.04));
  assert.ok(Math.abs(curve.ratePeriod(0.5, 1.0) - 0.04) < 1e-12);
  assert.ok(curve.df(1.0) > 0 && curve.df(1.0) < 1);
  assert.equal(curve.dfOnDateCurve('2025-01-02'), 1);
  const back = core.ForwardCurve.fromJson(curve.toJson());
  assert.equal(back.toJson(), curve.toJson());
});

test('VolCube round trips JSON and exposes its nodes', () => {
  const cube = new core.VolCube(
    'CUBE',
    [1, 2],
    [5],
    [0.02, 0.5, -0.2, 0.4, Number.NaN, 0.03, 0.5, -0.1, 0.3, 0.01],
    [0.03, 0.035]
  );
  assert.deepEqual([...cube.expiries], [1, 2]);
  assert.deepEqual([...cube.tenors], [5]);
  assert.deepEqual([...cube.gridShape], [2, 1]);
  assert.deepEqual([...cube.forwards], [0.03, 0.035]);
  assert.equal(cube.params.length, 2);
  assert.equal(cube.paramsAt(1, 0).alpha, 0.03);
  assert.equal(cube.paramsAt(1, 0).shift, 0.01);
  assert.equal(cube.forwardAt(1, 0), 0.035);
  assert.throws(
    () => cube.forwardAt(2, 0),
    (error) =>
      error.kind === 'validation' &&
      error.message.includes('grid index (2, 0) outside shape (2, 1)')
  );
  assert.throws(
    () => cube.paramsAt(-1, 0),
    (error) => error.kind === 'invalid_type'
  );
  assert.equal(core.VolCube.fromJson(cube.toJson()).toJson(), cube.toJson());
});

test('FxDeltaVolSurface round trips JSON and exposes its quotes', () => {
  const three = new core.FxDeltaVolSurface('EURUSD', [1], [0.12], [0.01], [0.002]);
  assert.deepEqual([...three.atmVols], [0.12]);
  assert.deepEqual([...three.rr25d], [0.01]);
  assert.deepEqual([...three.bf25d], [0.002]);
  assert.equal(three.rr10d, undefined);
  assert.equal(three.bf10d, undefined);
  const five = new core.FxDeltaVolSurface('EURUSD', [1], [0.12], [0.01], [0.002], [0.02], [0.005]);
  assert.deepEqual([...five.rr10d], [0.02]);
  assert.deepEqual([...five.bf10d], [0.005]);
  assert.equal(core.FxDeltaVolSurface.fromJson(five.toJson()).toJson(), five.toJson());
});

test('FxRateResult round trips its JSON', () => {
  const fx = new core.FxMatrix();
  fx.setQuote('EUR', 'USD', 1.1);
  const result = fx.rate('EUR', 'USD', '2025-01-02');
  const back = core.FxRateResult.fromJson(result.toJson());
  assert.equal(back.rate, result.rate);
  assert.equal(back.triangulated, result.triangulated);
  assert.throws(
    () => core.FxRateResult.fromJson('{"rate":1.1}'),
    (error) => error.kind === 'validation'
  );
});
