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
  assert.equal(typeof result.summary.total_return, 'number');
  assert.ok(Math.abs(result.summary.total_return - totalPnl / initial) < 1e-12);
  assert.ok(
    Math.abs(
      result.summary.annualized_return - ((1 + result.summary.total_return) ** (365 / 31) - 1)
    ) < 1e-12
  );
  assert.deepEqual(Object.keys(result.summary.factor_contributions).sort(), [
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
      result.summary.factor_contributions.carry - Number(result.attribution.carry.amount) / initial
    ) < 1e-12
  );
  assert.equal(result.scenario_report.user_operations, 1);
});

test('horizon facade uses null for undefined returns', () => {
  const { instrument, market } = depositFixture();
  const spec = JSON.stringify(scenarios.buildScenarioSpec('no-roll', []));
  const live = scenarios.computeHorizonReturn(instrument, market, '2025-01-15', spec);
  assert.equal(live.horizon_days, null);
  assert.equal(live.summary.total_return, 0);
  assert.equal(live.summary.annualized_return, null);
  const matured = scenarios.computeHorizonReturn(instrument, market, '2025-08-15', spec);
  assert.equal(Number(matured.initial_value.amount), 0);
  assert.equal(matured.summary.total_return, null);
  assert.equal(matured.summary.annualized_return, null);
  assert.ok(Object.values(matured.summary.factor_contributions).every((value) => value === null));
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
      assert.ok(Number.isFinite(result.summary.total_return));
      assert.equal(result.summary.annualized_return, null);
      assert.ok(Object.values(result.summary.factor_contributions).every(Number.isFinite));
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
      assert.ok(Number.isFinite(result.summary.total_return));
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
  assert.throws(
    () => scenarios.applyScenarioToMarket(spec, market, '2025-01-15'),
    (error) => error.kind === 'validation' && /no instruments were supplied/.test(error.message)
  );
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

const AS_OF = '2025-01-15';
// Same market and deposit as finstack-quant-py/tests/test_scenarios_engine_parity.py.
const OIS_MARKET = {
  schema_version: 1,
  curves: [
    {
      type: 'discount',
      id: 'USD-OIS',
      base: AS_OF,
      day_count: 'act_365f',
      knot_points: [
        [0.0, 1.0],
        [0.5, 0.98],
        [1.0, 0.96],
        [2.0, 0.92],
      ],
      interp_style: 'monotone_convex',
      extrapolation: 'flat_forward',
      min_forward_rate: -0.005,
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
};
const DEPOSIT = {
  schema: 'finstack_quant.instrument/1',
  instrument: {
    type: 'deposit',
    spec: {
      id: 'DEP-0',
      notional: { amount: '1000000', currency: 'USD' },
      start_date: AS_OF,
      maturity: '2025-07-15',
      day_count: 'act_360',
      fixed_rate: '0.04',
      discount_curve_id: 'USD-OIS',
      attributes: {},
    },
  },
};
const UP_25 = { kind: 'curve_parallel_bp', curve_kind: 'discount', curve_id: 'USD-OIS', bp: 25 };
const HOLD_1M_UP_25 = scenarios.buildScenarioSpec('hold_1m_up25', [
  { kind: 'time_roll_forward', period: '1M', apply_shocks: true, roll_mode: 'calendar_days' },
  UP_25,
]);

function withRounding(mode) {
  return JSON.stringify({
    rounding: { mode, ingest_scale: { overrides: {} }, output_scale: { overrides: {} } },
  });
}

test('applyScenario and applyScenarioToMarket stamp the caller configuration into meta', () => {
  const spec = scenarios.buildScenarioSpec('up25', [UP_25]);
  const fallback = scenarios.applyScenarioToMarket(spec, OIS_MARKET, AS_OF);
  assert.equal(fallback.meta.rounding.mode, 'bankers');
  const floor = scenarios.applyScenarioToMarket(
    spec,
    OIS_MARKET,
    AS_OF,
    undefined,
    withRounding('floor')
  );
  assert.equal(floor.meta.rounding.mode, 'floor');
  const model = {
    schema_version: 1,
    id: 'm',
    periods: [{ id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: false }],
    nodes: {},
  };
  const withModel = scenarios.applyScenario(
    spec,
    OIS_MARKET,
    model,
    AS_OF,
    undefined,
    withRounding('away_from_zero')
  );
  assert.equal(withModel.meta.rounding.mode, 'away_from_zero');
  assert.throws(
    () => scenarios.applyScenarioToMarket(spec, OIS_MARKET, AS_OF, undefined, '{not json'),
    (error) => error.kind === 'validation'
  );
});

test('scenario errors surface before market errors and match Python text', () => {
  assert.throws(
    () => scenarios.applyScenarioToMarket({ id: '', operations: [] }, { curves: 5 }, AS_OF),
    /Scenario ID cannot be empty/
  );
  for (const call of [
    () => scenarios.validateScenarioSpec({ id: '', operations: [] }),
    () => scenarios.parseScenarioSpec({ id: '', operations: [] }),
  ]) {
    assert.throws(
      call,
      (error) => error.kind === 'validation' && /Scenario ID cannot be empty/.test(error.message)
    );
  }
  assert.equal(scenarios.validateScenarioSpec(HOLD_1M_UP_25), undefined);
  assert.deepEqual(scenarios.parseScenarioSpec(JSON.stringify(HOLD_1M_UP_25)), HOLD_1M_UP_25);
});

test('applyScenario rejects a statement model that fails semantic validation', () => {
  const spec = scenarios.buildScenarioSpec('up25', [UP_25]);
  const model = { schema_version: 1, id: 'm', periods: [], nodes: {} };
  assert.throws(
    () => scenarios.applyScenario(spec, OIS_MARKET, model, AS_OF),
    /at least one period/
  );
});

test('instrument inventories use the shared Rust envelope loader', () => {
  const spec = scenarios.buildScenarioSpec('up25', [UP_25]);
  assert.throws(
    () => scenarios.applyScenarioToMarket(spec, OIS_MARKET, AS_OF, [DEPOSIT.instrument]),
    /invalid instrument envelope JSON/
  );
  assert.throws(
    () => scenarios.computeHorizonReturn(DEPOSIT.instrument, OIS_MARKET, AS_OF, spec),
    /invalid instrument envelope JSON/
  );
});

test('composeScenarios validates every input spec in Rust', () => {
  const valid = scenarios.buildScenarioSpec('a', [UP_25]);
  assert.throws(
    () => scenarios.composeScenarios([valid, { id: '', operations: [] }]),
    (error) =>
      error.kind === 'validation' &&
      /Cannot compose scenario ''.*Scenario ID cannot be empty/.test(error.message)
  );
  assert.throws(
    () =>
      scenarios.composeScenarios([
        valid,
        { id: 'roll', operations: [{ kind: 'time_roll_forward', period: 'bogus' }] },
      ]),
    /Cannot compose scenario 'roll'/
  );
  assert.equal(scenarios.composeScenarios([valid, { ...valid, id: 'b' }]).id, 'a+b');
});

test('built-in templates round-trip through build, component and validate', () => {
  const ids = scenarios.listBuiltinTemplates();
  assert.deepEqual(
    scenarios.listBuiltinTemplateMetadata().map((metadata) => metadata.id),
    ids
  );
  for (const id of ids) {
    const spec = scenarios.buildFromTemplate(id);
    assert.equal(spec.id, id);
    assert.equal(scenarios.validateScenarioSpec(spec), undefined);
    for (const component of scenarios.listTemplateComponents(id)) {
      assert.equal(scenarios.buildTemplateComponent(id, component).id, component);
    }
  }
  assert.throws(() => scenarios.buildFromTemplate('no-such-template'));
});

test('computeHorizonReturn returns the Rust summary and matches Python', () => {
  const result = scenarios.computeHorizonReturn(DEPOSIT, OIS_MARKET, AS_OF, HOLD_1M_UP_25);
  assert.deepEqual(Object.keys(result).sort(), [
    'attribution',
    'horizon_days',
    'initial_value',
    'scenario_report',
    'summary',
    'terminal_value',
  ]);
  assert.equal(result.horizon_days, 31);
  assert.equal(result.summary.currency, 'USD');
  // Python: compute_horizon_return(...).total_return / annualized_return /
  // factor_contribution("carry") on the same inputs.
  assert.ok(Math.abs(result.summary.total_return - 0.0023899669679482228) < 1e-12);
  assert.ok(Math.abs(result.summary.annualized_return - 0.028505070795274978) < 1e-12);
  assert.ok(Math.abs(result.summary.factor_contributions.carry - 0.0034203488888837974) < 1e-12);
  assert.equal(Object.keys(result.summary.factor_contributions).length, 9);
  const ratio = Number(result.attribution.total_pnl.amount) / Number(result.initial_value.amount);
  assert.ok(Math.abs(result.summary.total_return - ratio) < 1e-12);

  const explicit = scenarios.computeHorizonReturn(
    DEPOSIT,
    OIS_MARKET,
    AS_OF,
    HOLD_1M_UP_25,
    'parallel'
  );
  assert.deepEqual(explicit.summary, result.summary);
  assert.throws(
    () => scenarios.computeHorizonReturn(DEPOSIT, OIS_MARKET, AS_OF, HOLD_1M_UP_25, 'bogus'),
    (error) => error.kind === 'validation' && /Unknown attribution method/.test(error.message)
  );
  assert.throws(
    () =>
      scenarios.computeHorizonReturn(
        DEPOSIT,
        OIS_MARKET,
        AS_OF,
        HOLD_1M_UP_25,
        undefined,
        undefined,
        'not-a-calendar'
      ),
    (error) => error.kind === 'not_found'
  );
  const shock = scenarios.buildScenarioSpec('px', [
    { kind: 'instrument_price_pct_by_type', instrument_types: ['bond'], pct: -5 },
  ]);
  assert.throws(
    () => scenarios.computeHorizonReturn(DEPOSIT, OIS_MARKET, AS_OF, shock),
    /HorizonAnalysis/
  );
});
