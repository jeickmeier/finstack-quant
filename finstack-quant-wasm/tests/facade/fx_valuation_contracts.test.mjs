/** Typed FX result serialization and financial-unit contracts. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function instrument(kind) {
  return JSON.parse(
    readFileSync(
      new URL(
        `../../../finstack-quant/valuations/tests/instruments/json_examples/${kind}.json`,
        import.meta.url
      )
    )
  );
}

function market(domesticRate = 0.04, foreignRate = 0.02) {
  const state = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_equity.json',
        import.meta.url
      )
    )
  ).market;
  state.curves = [
    ['USD-OIS', domesticRate],
    ['EUR-OIS', foreignRate],
  ].map(([id, rate]) => ({
    ...state.curves[0],
    id,
    base: '2024-01-01',
    knot_points: [
      [0, 1],
      [10, Math.exp(-rate * 10)],
    ],
  }));
  state.prices = { 'EURUSD-SPOT': { unitless: 1.1 } };
  state.fx = {
    config: {},
    quotes: [['EUR', 'USD', 1.1]],
    provider_quotes: [],
    provider_pinned_quotes: [],
    pinned_quotes: [],
  };
  return state;
}

test('typed FX Monte Carlo pricing preserves full-width seeds like generic pricing', () => {
  const envelope = instrument('fx_barrier_option');
  envelope.instrument.spec.instrument_pricing_overrides = {
    model_config: { mc_paths: 32 },
    market_quotes: { implied_volatility: 0.2 },
  };
  const instrumentJson = JSON.stringify(envelope);
  const marketJson = JSON.stringify(market());
  const asOf = '2024-01-01';
  const model = 'monte_carlo_gbm';
  const generic = valuations.instruments.priceInstrument(instrumentJson, marketJson, asOf, model);
  assert.equal(generic.details.type, 'monte_carlo');
  assert.equal(typeof generic.details.data.seed, 'bigint');
  assert.ok(generic.details.data.seed > BigInt(Number.MAX_SAFE_INTEGER));

  const typed = valuations.fx.FxBarrierOption.fromJson(instrumentJson);
  try {
    const result = typed.price(marketJson, asOf, model);
    assert.deepEqual(result.value, generic.value);
    assert.deepEqual(result.measures, generic.measures);
    assert.deepEqual(result.details, generic.details);
    assert.deepEqual(typed.price(marketJson, asOf, model).details, result.details);
  } finally {
    typed.free();
  }
});

test('typed FX Greeks use volatility points, basis points, and daily theta', () => {
  const envelope = instrument('fx_option');
  envelope.instrument.spec.expiry = '2025-01-01';
  envelope.instrument.spec.instrument_pricing_overrides = {
    market_quotes: { implied_volatility: 0.2 },
  };
  const asOf = '2024-01-02';
  const model = 'default';
  const price = (vol = 0.2, domesticRate = 0.04, foreignRate = 0.02, date = asOf) => {
    const bumped = structuredClone(envelope);
    bumped.instrument.spec.instrument_pricing_overrides.market_quotes.implied_volatility = vol;
    return Number(
      valuations.instruments.priceInstrument(
        JSON.stringify(bumped),
        JSON.stringify(market(domesticRate, foreignRate)),
        date,
        model
      ).value.amount
    );
  };
  const h = 1e-5;
  const expected = {
    vega: ((price(0.2 + h) - price(0.2 - h)) / (2 * h)) * 0.01,
    rho: ((price(0.2, 0.04 + h) - price(0.2, 0.04 - h)) / (2 * h)) * 0.0001,
    foreignRho: ((price(0.2, 0.04, 0.02 + h) - price(0.2, 0.04, 0.02 - h)) / (2 * h)) * 0.0001,
    theta: (price(0.2, 0.04, 0.02, '2024-01-03') - price(0.2, 0.04, 0.02, '2024-01-01')) / 2,
  };
  const typed = valuations.fx.FxOption.fromJson(JSON.stringify(envelope));
  try {
    const marketJson = JSON.stringify(market());
    for (const [metric, expectedValue] of Object.entries(expected)) {
      const actual = typed[metric](marketJson, asOf, model);
      assert.ok(Math.abs(expectedValue) > 1, `${metric} probe must have material exposure`);
      assert.ok(
        Math.abs(actual - expectedValue) < Math.abs(expectedValue) * 1e-5,
        `${metric}: ${actual} versus finite-difference ${expectedValue}`
      );
    }
  } finally {
    typed.free();
  }
});

test('typed quanto theta preserves configured period P&L and the capped horizon', () => {
  const envelope = instrument('quanto_option');
  envelope.instrument.spec.expiry = '2025-01-01';
  const quantoMarket = (base) => {
    const state = market();
    state.curves[1].id = 'JPY-OIS';
    for (const curve of state.curves) curve.base = base;
    state.prices = {
      'NKY-SPOT': { price: { amount: '35000', currency: 'JPY' } },
      'NKY-DIV': { unitless: 0.01 },
      'JPYUSD-SPOT': { unitless: 1 / 140 },
    };
    const surface = state.surfaces[0];
    state.surfaces = [
      { ...surface, id: 'NKY-VOL', strikes: [25000, 35000, 45000] },
      {
        ...surface,
        id: 'JPYUSD-VOL',
        strikes: [0.005, 1 / 140, 0.01],
        vols_row_major: surface.vols_row_major.map(() => 0.1),
      },
    ];
    state.fx.quotes = [['JPY', 'USD', 1 / 140]];
    return state;
  };
  const price = (instrument, date, metrics = []) =>
    valuations.instruments.priceInstrument(
      JSON.stringify(instrument),
      JSON.stringify(quantoMarket(date)),
      date,
      'default',
      metrics
    );
  for (const [period, asOf, rolledDate, expectedDays] of [
    [null, '2024-01-01', '2024-01-02', 1],
    [{ count: 1, unit: 'weeks' }, '2024-01-01', '2024-01-08', 7],
    [{ count: 1, unit: 'weeks' }, '2024-12-30', '2025-01-01', 2],
  ]) {
    const configured = structuredClone(envelope);
    configured.instrument.spec.metric_pricing_overrides = { theta_period: period };
    const generic = price(configured, asOf, ['theta', 'theta_period_days']);
    const expected =
      Number(price(configured, rolledDate).value.amount) - Number(generic.value.amount);
    assert.equal(generic.measures.theta_period_days, expectedDays);
    assert.ok(Math.abs(expected) > 1, 'carry probe must have material period P&L');
    assert.ok(Math.abs(generic.measures.theta - expected) < Math.abs(expected) * 1e-10);
    const typed = valuations.fx.QuantoOption.fromJson(JSON.stringify(configured));
    try {
      assert.equal(typed.theta(JSON.stringify(quantoMarket(asOf)), asOf), generic.measures.theta);
      assert.equal(
        typed.greeks(JSON.stringify(quantoMarket(asOf)), asOf).theta,
        generic.measures.theta
      );
    } finally {
      typed.free();
    }
  }
});
