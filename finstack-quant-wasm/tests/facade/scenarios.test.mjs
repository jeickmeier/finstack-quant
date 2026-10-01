import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, scenarios, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function depositFixture() {
  return {
    instrument: JSON.stringify({
      schema: 'finstack_quant.instrument/1',
      instrument: {
        type: 'deposit',
        spec: {
          id: 'DEP-0',
          notional: { amount: '1000000', currency: 'USD' },
          start_date: '2025-01-15',
          maturity: '2025-07-15',
          day_count: 'act_360',
          fixed_rate: '0.04',
          discount_curve_id: 'USD-OIS',
          attributes: {},
        },
      },
    }),
    market: JSON.stringify({
      schema_version: 1,
      curves: [
        {
          type: 'discount',
          id: 'USD-OIS',
          base: '2025-01-15',
          day_count: 'act_365f',
          knot_points: [
            [0, 1],
            [0.5, 0.98],
            [1, 0.96],
            [2, 0.92],
          ],
          interp_style: 'log_linear',
          extrapolation: 'flat_forward',
          min_forward_rate: null,
          allow_non_monotonic: false,
          min_forward_tenor: 1e-6,
          rate_calibration: null,
          calibration_ois_cutoff_days: null,
          fx_policy: null,
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
    }),
  };
}

test('scenario metadata uses nullable serialized names and descriptions', () => {
  const built = scenarios.buildScenarioSpec('metadata', []);
  assert.equal(built.name, null);
  assert.equal(built.description, null);
  const parsed = scenarios.parseScenarioSpec(JSON.stringify(built));
  assert.deepEqual(parsed, built);
});

test('date-range overflow is a validation error in engine and horizon APIs', () => {
  const { instrument, market } = depositFixture();
  const isDateRangeError = (error) =>
    error instanceof Error &&
    error.name === 'FinstackError' &&
    error.kind === 'validation' &&
    /supported date range/.test(error.message);
  for (const mode of ['approximate', 'calendar_days', 'business_days']) {
    const spec = JSON.stringify(
      scenarios.buildScenarioSpec(`date-range-${mode}`, [
        { kind: 'time_roll_forward', period: '1D', apply_shocks: false, roll_mode: mode },
      ])
    );
    assert.throws(
      () => scenarios.applyScenarioToMarket(spec, market, '9999-12-31'),
      isDateRangeError
    );
    assert.throws(
      () => scenarios.computeHorizonReturn(instrument, market, '9999-12-31', spec),
      isDateRangeError
    );
  }
});

test('horizon facade includes Rust-computed returns and factor contributions', () => {
  const { instrument, market } = depositFixture();
  const spec = JSON.stringify(
    scenarios.buildScenarioSpec('hold-one-month', [
      {
        kind: 'time_roll_forward',
        period: '1M',
        apply_shocks: true,
        roll_mode: 'calendar_days',
      },
    ])
  );
  const result = scenarios.computeHorizonReturn(instrument, market, '2025-01-15', spec);
  const initial = Number(result.initial_value.amount);
  const totalPnl = Number(result.attribution.total_pnl.amount);
  assert.equal(result.initial_value.currency, 'USD');
  assert.equal(result.horizon_days, 31);
  assert.equal(typeof result.total_return, 'number');
  assert.ok(Math.abs(result.total_return - totalPnl / initial) < 1e-12);
  assert.ok(
    Math.abs(result.annualized_return - ((1 + result.total_return) ** (365 / 31) - 1)) < 1e-12
  );
  assert.deepEqual(Object.keys(result.factor_contributions).sort(), [
    'carry',
    'correlations',
    'credit_curves',
    'fx',
    'inflation_curves',
    'market_scalars',
    'model_parameters',
    'rates_curves',
    'volatility',
  ]);
  assert.ok(
    Math.abs(
      result.factor_contributions.carry - Number(result.attribution.carry.amount) / initial
    ) < 1e-12
  );
  assert.equal(result.scenario_report.user_operations, 1);
});

test('horizon facade uses null for undefined returns', () => {
  const { instrument, market } = depositFixture();
  const spec = JSON.stringify(scenarios.buildScenarioSpec('no-roll', []));
  const live = scenarios.computeHorizonReturn(instrument, market, '2025-01-15', spec);
  assert.equal(live.horizon_days, null);
  assert.equal(live.total_return, 0);
  assert.equal(live.annualized_return, null);
  const matured = scenarios.computeHorizonReturn(instrument, market, '2025-08-15', spec);
  assert.equal(Number(matured.initial_value.amount), 0);
  assert.equal(matured.total_return, null);
  assert.equal(matured.annualized_return, null);
  assert.ok(Object.values(matured.factor_contributions).every((value) => value === null));
});

for (const method of ['parallel', 'waterfall', 'metrics_based', 'taylor']) {
  test(`horizon ${method} supports standard fixed-income instruments`, () => {
    const { instrument, market } = depositFixture();
    const bond = valuations.instruments.Bond.fixed(
      'HORIZON-BOND',
      new core.Money(100, new core.Currency('USD')),
      new core.Rate(0.05),
      '2024-01-01',
      '2034-01-01',
      'none',
      'USD-OIS'
    );
    const spec = JSON.stringify(
      scenarios.buildScenarioSpec('rates-up', [
        { kind: 'curve_parallel_bp', curve_kind: 'discount', curve_id: 'USD-OIS', bp: 25 },
      ])
    );
    for (const input of [instrument, bond.toJson()]) {
      const result = scenarios.computeHorizonReturn(input, market, '2025-01-15', spec, method);
      assert.ok(Number(result.initial_value.amount) > 0);
      assert.ok(Number(result.terminal_value.amount) > 0);
      assert.equal(result.initial_value.currency, 'USD');
      assert.ok(Number.isFinite(result.total_return));
      assert.equal(result.annualized_return, null);
      assert.ok(Object.values(result.factor_contributions).every(Number.isFinite));
    }
  });
}

test('floating horizons preserve opening-day and crossed fixings for every method', () => {
  const origin = '2025-01-02';
  const state = JSON.parse(depositFixture().market);
  Object.assign(state.curves[0], {
    base: origin,
    knot_points: [
      [0, 1],
      [1, 0.95],
      [2, 0.9],
    ],
    interp_style: 'monotone_convex',
  });
  state.curves.push({
    type: 'forward',
    id: 'USD-SOFR-3M',
    base: origin,
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
  const market = JSON.stringify(state);
  const spec = JSON.stringify(
    scenarios.buildScenarioSpec('crossed-reset', [
      {
        kind: 'time_roll_forward',
        period: '4D',
        apply_shocks: false,
        roll_mode: 'calendar_days',
      },
    ])
  );
  for (const issueDate of [origin, '2025-01-03']) {
    const bond = valuations.instruments.Bond.floating(
      'HORIZON-FRN',
      new core.Money(1000000, new core.Currency('USD')),
      'USD-SOFR-3M',
      new core.Bps(150),
      issueDate,
      '2026-01-03',
      core.Tenor.quarterly(),
      core.DayCount.act360(),
      'USD-OIS'
    );
    const original = bond.toJson();
    const envelope = JSON.parse(original);
    envelope.instrument.spec.cashflow_spec.floating.rate_spec.reset_lag_days = 0;
    const instrument = JSON.stringify(envelope);
    const opening = valuations.instruments.priceInstrument(instrument, market, origin).value;
    const rolled = scenarios.applyScenarioToMarket(
      spec,
      market,
      origin,
      JSON.stringify([envelope])
    );
    const closing = valuations.instruments.priceInstrument(
      JSON.stringify(rolled.instruments[0]),
      JSON.stringify(rolled.market),
      rolled.time_roll.new_date
    ).value;
    for (const method of ['parallel', 'waterfall', 'metrics_based', 'taylor']) {
      const result = scenarios.computeHorizonReturn(instrument, market, origin, spec, method);
      assert.ok(Number.isFinite(Number(result.initial_value.amount)));
      assert.ok(Number.isFinite(Number(result.terminal_value.amount)));
      assert.deepEqual(result.initial_value, opening, `${issueDate} ${method} opening`);
      assert.deepEqual(result.terminal_value, closing, `${issueDate} ${method} closing`);
      assert.equal(result.horizon_days, 4);
      assert.ok(Number.isFinite(result.total_return));
      assert.deepEqual(result.scenario_report.warnings, []);
    }
    assert.equal(bond.toJson(), original);
    assert.equal(JSON.stringify(envelope), instrument);
  }
  assert.equal(JSON.stringify(state), market);
});

test('scenario facade returns reusable shocked instrument copies', () => {
  const bond = valuations.instruments.Bond.fixed(
    'BOND',
    new core.Money(100, new core.Currency('USD')),
    new core.Rate(0.05),
    '2024-01-01',
    '2034-01-01',
    'none',
    'USD-OIS'
  );
  const original = bond.toJson();
  const inventory = JSON.stringify([JSON.parse(original)]);
  const market = JSON.stringify({
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
  });
  const op = { kind: 'instrument_price_pct_by_type', instrument_types: ['bond'], pct: -60 };
  const spec = JSON.stringify(scenarios.buildScenarioSpec('losses', [op, op]));
  assert.throws(() => scenarios.applyScenarioToMarket(spec, market, '2025-01-15'), /instruments/);
  const result = scenarios.applyScenarioToMarket(spec, market, '2025-01-15', inventory);
  assert.equal(bond.toJson(), original);
  assert.equal(result.instruments.length, 1);
  const shock =
    result.instruments[0].instrument.spec.scenario_pricing_overrides.scenario_price_shock_decimal;
  assert.ok(Math.abs(100 * (1 + shock) - 16) < 1e-12);
  const restored = valuations.instruments.Bond.fromJson(JSON.stringify(result.instruments[0]));
  assert.equal(restored.id, 'BOND');
  const half = JSON.stringify(scenarios.buildScenarioSpec('half', [{ ...op, pct: -50 }]));
  const next = scenarios.applyScenarioToMarket(
    half,
    market,
    '2025-01-15',
    JSON.stringify(result.instruments)
  );
  const nextShock =
    next.instruments[0].instrument.spec.scenario_pricing_overrides.scenario_price_shock_decimal;
  assert.ok(Math.abs(100 * (1 + nextShock) - 8) < 1e-12);
});
