import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { scenarios, valuations } from '../../index.js';

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

test('time roll preserves the raw crossed fixing and supports repeated rolls', () => {
  const f = fixture('production_quanto_range');
  f.market.curves = f.market.curves.filter((curve) => curve.id === 'USD-OIS');
  f.market.curves[0].knot_points = [
    [0, 1],
    [1, 1],
    [10, 1],
  ];
  const bond = valuations.instruments.Bond.floating({
    id: 'ROLL-FRN',
    notional: { amount: '1000000', currency: 'USD' },
    forward_curve_id: 'USD-SOFR-3M',
    spread_bp: 150,
    issue_date: '2025-01-03',
    maturity: '2026-01-03',
    frequency: { count: 3, unit: 'months' },
    day_count: 'act_360',
    quote_compounding: { periodic: 4 },
    discount_curve_id: 'USD-OIS',
  });
  const instrument = JSON.parse(bond.toJson());
  instrument.instrument.spec.cashflow_spec.floating.rate_spec.reset_lag_days = 0;
  f.market.curves.push({
    type: 'forward',
    id: 'USD-SOFR-3M',
    base: f.as_of,
    day_count: 'act_360',
    index_tenor: { count: 3, unit: 'months' },
    knot_points: [0, 1, 2].map((t) => [t, Math.exp(-0.03 * t)]),
    interp_style: 'log_linear',
    extrapolation: 'extend',
    rate_calibration: null,
    fx_policy: null,
  });
  const spec = {
    id: 'cross-reset',
    operations: [
      { kind: 'time_roll_forward', period: '4D', apply_shocks: false, roll_mode: 'calendar_days' },
    ],
  };
  const result = scenarios.applyScenario(
    JSON.stringify(spec),
    JSON.stringify(f.market),
    f.as_of,
    undefined,
    JSON.stringify([instrument])
  );
  assert.deepEqual(result.report.time_roll.failed_instruments, []);
  const observed = result.market.series.find((series) => series.id === 'FIXING:USD-SOFR-3M');
  assert.ok(observed, 'crossed reset must have a fixing series');
  const rate = observed.observations.find(([date]) => date === '2025-01-03')[1];
  // Flat 3% continuous factor curve; the 2025-01-03 to 2025-04-03 accrual is 90/360.
  const expected = Math.expm1(0.03 * 0.25) / 0.25;
  assert.ok(
    Math.abs(rate - expected) < 1e-12,
    `${rate} versus ${expected}: the 150bp spread must not enter the index fixing`
  );
  spec.operations[0].period = '1D';
  const next = scenarios.applyScenario(
    JSON.stringify(spec),
    JSON.stringify(result.market),
    result.report.time_roll.new_date,
    undefined,
    JSON.stringify(result.instruments)
  );
  assert.deepEqual(next.report.time_roll.failed_instruments, []);
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
  f.market.curves[0].knot_points = [
    [0, 1],
    [1, Math.exp(-0.04)],
    [2, Math.exp(-0.08)],
  ];
  const instrument = {
    schema: 'finstack_quant.instrument/1',
    instrument: {
      type: 'interest_rate_swap',
      spec: {
        id: 'ROLL-OIS',
        notional: option.notional,
        side: 'pay',
        fixed_leg: fixed,
        float_leg: floating,
        attributes: {},
      },
    },
  };
  const scenario = {
    id: 'overnight-roll',
    operations: [
      { kind: 'time_roll_forward', period: '5D', apply_shocks: false, roll_mode: 'calendar_days' },
    ],
  };
  const result = scenarios.applyScenario(
    JSON.stringify(scenario),
    JSON.stringify(f.market),
    f.as_of,
    undefined,
    JSON.stringify([instrument])
  );
  assert.deepEqual(result.report.time_roll.failed_instruments, []);
  const observations = result.market.series.find(
    (series) => series.id === 'FIXING:USD-OIS'
  ).observations;
  const friday = observations.find(([date]) => date === '2025-01-03')[1];
  const expected = Math.expm1((0.04 * 3) / 365) / (3 / 365);
  assert.ok(Math.abs(friday - expected) < 1e-11, `${friday} versus ${expected}`);
  assert.ok(friday < 0.041, 'the 100bp coupon spread must not enter daily fixings');
  assert.deepEqual(
    observations.map(([date]) => date),
    ['2025-01-02', '2025-01-03', '2025-01-06']
  );
});

function swapExample(name) {
  return JSON.parse(
    readFileSync(
      new URL(
        `../../../finstack-quant/valuations/tests/instruments/json_examples/${name}.json`,
        import.meta.url
      )
    )
  );
}

function flatRollMarket() {
  const f = fixture('production_quanto_range');
  f.market.curves = f.market.curves.filter((curve) => curve.id === 'USD-OIS');
  f.market.curves[0].knot_points = [
    [0, 1],
    [1, 1],
    [10, 1],
  ];
  return f;
}

function addForward(market, id, rate) {
  market.curves.push({
    type: 'forward',
    id,
    base: '2025-01-02',
    day_count: 'act_360',
    knot_points: [0, 1, 10].map((t) => [t, Math.exp(-rate * t)]),
    interp_style: 'log_linear',
    extrapolation: 'extend',
    rate_calibration: null,
    fx_policy: null,
  });
}

test('basis swap rolls both raw overnight indices without spread contamination', () => {
  const f = flatRollMarket();
  const instrument = swapExample('basis_swap');
  const spec = instrument.instrument.spec;
  for (const [leg, rate] of [
    [spec.primary_leg, 0.03],
    [spec.reference_leg, 0.04],
  ]) {
    leg.start = '2025-01-03';
    leg.end = '2026-01-03';
    leg.compounding = { compounded_in_arrears: { lookback_days: 0 } };
    addForward(f.market, leg.forward_curve_id, rate);
  }
  const scenario = {
    id: 'basis-roll',
    operations: [
      { kind: 'time_roll_forward', period: '4D', apply_shocks: false, roll_mode: 'calendar_days' },
    ],
  };
  const result = scenarios.applyScenario(
    JSON.stringify(scenario),
    JSON.stringify(f.market),
    f.as_of,
    undefined,
    JSON.stringify([instrument])
  );
  assert.deepEqual(result.report.time_roll.failed_instruments, []);
  for (const [leg, rate] of [
    [spec.primary_leg, 0.03],
    [spec.reference_leg, 0.04],
  ]) {
    const observations = result.market.series.find(
      (series) => series.id === `FIXING:${leg.forward_curve_id}`
    ).observations;
    assert.ok(observations.length >= 1);
    for (const [date, value] of observations) {
      // Daily fixing over the next weekends-only business day under ACT/360.
      const days = new Date(`${date}T00:00:00Z`).getUTCDay() === 5 ? 3 : 1;
      const expected = Math.expm1((rate * days) / 360) / (days / 360);
      assert.ok(Math.abs(value - expected) < 1e-10, `${date}: ${value} versus ${expected}`);
    }
  }
});

test('crossed fixing projection reports a missing dependency before rolling the market', () => {
  const f = flatRollMarket();
  const instrument = swapExample('basis_swap');
  for (const leg of [
    instrument.instrument.spec.primary_leg,
    instrument.instrument.spec.reference_leg,
  ]) {
    leg.start = '2025-01-03';
    leg.end = '2026-01-03';
  }
  const before = JSON.stringify(f.market);
  const scenario = {
    id: 'missing-projection',
    operations: [
      { kind: 'time_roll_forward', period: '4D', apply_shocks: false, roll_mode: 'calendar_days' },
    ],
  };
  assert.throws(
    () =>
      scenarios.applyScenario(
        JSON.stringify(scenario),
        before,
        f.as_of,
        undefined,
        JSON.stringify([instrument])
      ),
    /pre-roll fixing projection|missing pre-roll projection/i
  );
  assert.equal(JSON.stringify(f.market), before);
});
