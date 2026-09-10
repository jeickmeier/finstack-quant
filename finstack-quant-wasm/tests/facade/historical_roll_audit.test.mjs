import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, scenarios, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function fixture(name) {
  return JSON.parse(
    readFileSync(
      new URL(`../../../finstack-quant/valuations/tests/fixtures/${name}.json`, import.meta.url)
    )
  );
}

test('first-order credit spread shocks use recovery conversion and disclose approximation', () => {
  const f = fixture('production_cds_option');
  const original = f.market.curves.find((curve) => curve.type === 'hazard');
  const spec = {
    id: 'credit-spread-units',
    hazard_bump_mode: 'first_order_shift',
    operations: [
      { kind: 'curve_parallel_bp', curve_kind: 'par_cds', curve_id: original.id, bp: 10 },
    ],
  };
  const result = scenarios.applyScenarioToMarket(
    JSON.stringify(spec),
    JSON.stringify(f.market),
    f.as_of
  );
  const actual = result.market.curves.find((curve) => curve.id === original.id).knot_points[0][1];
  const expected = original.knot_points[0][1] + 0.001 / (1 - original.recovery_rate);
  assert.ok(Math.abs(actual - expected) < 1e-12, `${actual} versus ${expected}`);
  assert.ok(result.warnings.some((warning) => warning.kind.includes('first_order')));
});

test('time roll preserves the raw crossed fixing and supports repeated rolls', () => {
  const f = fixture('production_quanto_range');
  f.market.curves = f.market.curves.filter((curve) => curve.id === 'USD-OIS');
  f.market.curves[0].knot_points = [[0, 1], [1, 1], [10, 1]];
  const bond = valuations.instruments.Bond.floating(
    'ROLL-FRN',
    new core.Money(1000000, new core.Currency('USD')),
    'USD-SOFR-3M',
    new core.Bps(150),
    '2025-01-03',
    '2026-01-03',
    core.Tenor.quarterly(),
    core.DayCount.act360(),
    'USD-OIS'
  );
  const instrument = JSON.parse(bond.toJson());
  instrument.instrument.spec.cashflow_spec.floating.rate_spec.reset_lag_days = 0;
  f.market.curves.push({
    type: 'forward',
    id: 'USD-SOFR-3M',
    base: f.as_of,
    reset_lag: 0,
    day_count: 'act_360',
    tenor: 0.25,
    knot_points: [
      [0, 0.03],
      [1, 0.03],
      [2, 0.03],
    ],
    projection_grid: null,
    interp_style: 'linear',
    extrapolation: 'flat_forward',
    rate_calibration: null,
    fx_policy: null,
  });
  const spec = {
    id: 'cross-reset',
    operations: [
      { kind: 'time_roll_forward', period: '4D', apply_shocks: false, roll_mode: 'calendar_days' },
    ],
  };
  const result = scenarios.applyScenarioToMarket(
    JSON.stringify(spec),
    JSON.stringify(f.market),
    f.as_of,
    JSON.stringify([instrument])
  );
  assert.deepEqual(result.time_roll.failed_instruments, []);
  const observed = result.market.series.find((series) => series.id === 'FIXING:USD-SOFR-3M');
  assert.ok(observed, 'crossed reset must have a fixing series');
  const rate = observed.observations.find(([date]) => date === '2025-01-03')[1];
  assert.ok(Math.abs(rate - 0.03) < 1e-12, 'the 150bp spread must not enter the index fixing');
  spec.operations[0].period = '1D';
  const next = scenarios.applyScenarioToMarket(
    JSON.stringify(spec),
    JSON.stringify(result.market),
    result.time_roll.new_date,
    JSON.stringify(result.instruments)
  );
  assert.deepEqual(next.time_roll.failed_instruments, []);
  assert.deepEqual(
    next.market.series.find((series) => series.id === observed.id).observations,
    observed.observations
  );
});

test('overnight time roll records daily raw observations across a weekend', () => {
  const f = fixture('production_hw');
  const option = f.instrument.instrument.spec;
  const fixed = { ...option.underlying_fixed_leg, start: '2025-01-02', end: '2026-01-02' };
  const floating = { ...option.underlying_float_leg, start: fixed.start, end: fixed.end };
  f.market.curves[0].knot_points = [[0, 1], [1, Math.exp(-0.04)], [2, Math.exp(-0.08)]];
  const instrument = {
    schema: 'finstack_quant.instrument/1',
    instrument: {
      type: 'interest_rate_swap',
      spec: { id: 'ROLL-OIS', notional: option.notional, side: 'pay', fixed, float: floating, attributes: {} },
    },
  };
  const scenario = { id: 'overnight-roll', operations: [
    { kind: 'time_roll_forward', period: '5D', apply_shocks: false, roll_mode: 'calendar_days' },
  ] };
  const result = scenarios.applyScenarioToMarket(JSON.stringify(scenario), JSON.stringify(f.market), f.as_of, JSON.stringify([instrument]));
  assert.deepEqual(result.time_roll.failed_instruments, []);
  const observations = result.market.series.find((series) => series.id === 'FIXING:USD-OIS').observations;
  const friday = observations.find(([date]) => date === '2025-01-03')[1];
  const expected = Math.expm1(0.04 * 3 / 365) / (3 / 365);
  assert.ok(Math.abs(friday - expected) < 1e-11, `${friday} versus ${expected}`);
  assert.ok(friday < 0.041, 'the 100bp coupon spread must not enter daily fixings');
  assert.deepEqual(observations.map(([date]) => date), ['2025-01-02', '2025-01-03', '2025-01-06']);
});

function swapExample(name) {
  return JSON.parse(readFileSync(new URL(`../../../finstack-quant/valuations/tests/instruments/json_examples/${name}.json`, import.meta.url)));
}

function flatRollMarket() {
  const f = fixture('production_quanto_range');
  f.market.curves = f.market.curves.filter((curve) => curve.id === 'USD-OIS');
  f.market.curves[0].knot_points = [[0, 1], [1, 1], [10, 1]];
  return f;
}

function addForward(market, id, rate) {
  market.curves.push({ type: 'forward', id, base: '2025-01-02', reset_lag: 0,
    day_count: 'act_360', tenor: 0.25, knot_points: [[0, rate], [1, rate], [10, rate]],
    projection_grid: null, interp_style: 'linear', extrapolation: 'flat_forward',
    rate_calibration: null, fx_policy: null });
}

test('basis swap rolls both raw overnight indices without spread contamination', () => {
  const f = flatRollMarket();
  const instrument = swapExample('basis_swap');
  const spec = instrument.instrument.spec;
  for (const [leg, rate] of [[spec.primary_leg, 0.03], [spec.reference_leg, 0.04]]) {
    leg.start = '2025-01-03'; leg.end = '2026-01-03';
    leg.compounding = { compounded_in_arrears: { lookback_days: 0 } };
    addForward(f.market, leg.forward_curve_id, rate);
  }
  const scenario = { id: 'basis-roll', operations: [
    { kind: 'time_roll_forward', period: '4D', apply_shocks: false, roll_mode: 'calendar_days' },
  ] };
  const result = scenarios.applyScenarioToMarket(JSON.stringify(scenario), JSON.stringify(f.market), f.as_of, JSON.stringify([instrument]));
  assert.deepEqual(result.time_roll.failed_instruments, []);
  for (const [leg, rate] of [[spec.primary_leg, 0.03], [spec.reference_leg, 0.04]]) {
    const observations = result.market.series.find((series) => series.id === `FIXING:${leg.forward_curve_id}`).observations;
    assert.ok(observations.length >= 1);
    for (const [, value] of observations) assert.ok(Math.abs(value - rate) < 1e-10);
  }
});

test('crossed fixing projection reports a missing dependency before rolling the market', () => {
  const f = flatRollMarket();
  const instrument = swapExample('basis_swap');
  for (const leg of [instrument.instrument.spec.primary_leg, instrument.instrument.spec.reference_leg]) {
    leg.start = '2025-01-03'; leg.end = '2026-01-03';
  }
  const before = JSON.stringify(f.market);
  const scenario = { id: 'missing-projection', operations: [
    { kind: 'time_roll_forward', period: '4D', apply_shocks: false, roll_mode: 'calendar_days' },
  ] };
  assert.throws(() => scenarios.applyScenarioToMarket(JSON.stringify(scenario), before, f.as_of, JSON.stringify([instrument])), /pre-roll fixing projection|missing pre-roll projection/i);
  assert.equal(JSON.stringify(f.market), before);
});
