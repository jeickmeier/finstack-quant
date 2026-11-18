import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function read(relative) {
  return JSON.parse(readFileSync(new URL(relative, import.meta.url)));
}

function fixture(kind) {
  const instrument = read(
    `../../../finstack-quant/valuations/tests/instruments/json_examples/${kind}.json`
  );
  const { market } = read(
    '../../../finstack-quant/valuations/tests/fixtures/production_convertible.json'
  );
  market.curves[0].base = '2024-01-02';
  market.prices['SPX-SPOT'] = { unitless: 100 };
  market.surfaces = read(
    '../../../finstack-quant/valuations/tests/fixtures/production_equity.json'
  ).market.surfaces;
  market.surfaces[0].id = 'SPX-VOL';
  return { instrument, market };
}

function value(f, asOf = '2024-01-02', model = 'discounting') {
  return Number(
    valuations.instruments.priceInstrument(
      JSON.stringify(f.instrument),
      JSON.stringify(f.market),
      asOf,
      model,
      []
    ).value.amount
  );
}

function pdeFixture() {
  const f = read('../../../finstack-quant/valuations/tests/fixtures/production_quanto_range.json');
  f.instrument = fixture('barrier_option').instrument;
  f.market.prices['SPX-SPOT'] = { unitless: 100 };
  Object.assign(f.instrument.instrument.spec, {
    strike: 100,
    barrier: 120,
    quantity: 1,
    currency: 'USD',
    rebate: null,
    rebate_timing: null,
    div_yield_id: null,
    expiry: '2026-01-02',
    option_type: 'call',
    barrier_type: 'up_and_out',
    observed_barrier_breached: false,
    expiry_fixing: null,
  });
  return f;
}

test('PDE expiry-only monitoring matches an independent truncated lognormal payoff', () => {
  const f = pdeFixture();
  f.instrument.instrument.spec.monitoring = {
    type: 'discrete',
    observation_dates: ['2026-01-02'],
  };
  // S=K=100, H=120, r=q=0, sigma=20%, T=1. The exact expectation is
  // C(100) - C(120) - 20*N(d2(120)); the public PDE uses 200 spatial nodes.
  const expected = 2.7010124185237636;
  const actual = value(f, f.as_of, 'pde_crank_nicolson_1d');
  assert.ok(Math.abs(actual - expected) < 0.08, `${actual} versus ${expected}`);
});

test('PDE total rebates follow continuous closed forms and payment timing', () => {
  const f = pdeFixture();
  f.market.curves.find((c) => c.id === 'USD-OIS').knot_points = [
    [0, 1],
    [10, Math.exp(-0.5)],
  ];
  const s = f.instrument.instrument.spec;
  s.quantity = 1000;
  s.rebate = { amount: '25000', currency: 'USD' };
  s.monitoring = { type: 'continuous' };
  for (const barrierType of ['up_and_out', 'down_and_in']) {
    s.barrier_type = barrierType;
    s.barrier = barrierType === 'up_and_out' ? 120 : 80;
    for (const timing of ['at_hit', 'at_expiry']) {
      s.rebate_timing = timing;
      if (barrierType === 'down_and_in' && timing === 'at_hit') {
        // A no-hit rebate is only known at expiry, so at-hit timing is rejected.
        assert.throws(() => value(f, f.as_of, 'barrier_bs_continuous'), /at_expiry/);
        continue;
      }
      const analytical = value(f, f.as_of, 'barrier_bs_continuous');
      const pde = value(f, f.as_of, 'pde_crank_nicolson_1d');
      assert.ok(
        Math.abs(pde - analytical) < 5,
        `${barrierType}/${timing}: ${pde} vs ${analytical}`
      );
    }
  }
});

test('GBM and Heston observe discrete barriers only on contractual dates', () => {
  const f = pdeFixture();
  const s = f.instrument.instrument.spec;
  s.option_type = 'put';
  s.barrier = 90;
  s.rebate = { amount: '2.5', currency: 'USD' };
  s.rebate_timing = 'at_hit';
  s.div_yield_id = 'DIV';
  s.instrument_pricing_overrides = { model_config: { mc_paths: 128 } };
  f.market.surfaces.find((surface) => surface.id === 'SPX-VOL').vols_row_major.fill(1e-8);
  for (const [id, scalar] of Object.entries({
    DIV: -Math.log(0.8),
    HESTON_KAPPA: 1,
    HESTON_THETA: 1e-16,
    HESTON_V0: 1e-16,
    HESTON_SIGMA_V: 1e-12,
    HESTON_RHO: 0,
  })) {
    f.market.prices[id] = { unitless: scalar };
  }
  for (const model of ['monte_carlo_gbm', 'monte_carlo_heston']) {
    s.monitoring = { type: 'continuous' };
    assert.ok(Math.abs(value(f, f.as_of, model) - 2.5) < 1e-10);
    s.monitoring = { type: 'discrete', observation_dates: ['2026-01-02'] };
    // Spot moves deterministically from 100 to 80. Its initial level above 90
    // is outside the discrete observation schedule, so the terminal put pays 20.
    const actual = value(f, f.as_of, model);
    assert.ok(Math.abs(actual - 20) < 0.001, `${model}: ${actual}`);
  }
});

test('reciprocal NDF quotes preserve the long-base loss', () => {
  const f = fixture('ndf');
  const s = f.instrument.instrument.spec;
  s.contract_rate = 7;
  s.observed_fixing = 8;
  s.notional.amount = '7000000';
  assert.ok(Math.abs(value(f) + 125000) < 1e-8);
  s.quote_convention = 'settlement_per_base';
  s.contract_rate = 1 / 7;
  s.observed_fixing = 1 / 8;
  assert.ok(Math.abs(value(f) + 125000) < 1e-8);
});

test('known expiry rebate is a total trade amount', () => {
  const f = fixture('barrier_option');
  const s = f.instrument.instrument.spec;
  s.div_yield_id = null;
  s.strike = 100;
  s.barrier = 120;
  s.quantity = 1000;
  s.rebate.amount = '25';
  s.rebate_timing = 'at_expiry';
  s.observed_barrier_breached = true;
  s.monitoring = { type: 'continuous' };
  assert.equal(value(f, '2024-01-02', 'barrier_bs_continuous'), 25);
});

test('expired at-hit rebate has already been paid', () => {
  const f = fixture('barrier_option');
  const s = f.instrument.instrument.spec;
  s.div_yield_id = null;
  s.expiry_fixing = 100;
  s.observed_barrier_breached = true;
  s.rebate.amount = '25';
  s.monitoring = { type: 'continuous' };
  assert.equal(value(f, s.expiry, 'barrier_bs_continuous'), 0);
});

test('quanto range uses asset financing and requires actual FX market inputs', () => {
  const f = read('../../../finstack-quant/valuations/tests/fixtures/production_quanto_range.json');
  // 8000 * [Phi((ln(1.2)-.07)/.2) - Phi((ln(.8)-.07)/.2)].
  assert.ok(Math.abs(value(f, f.as_of, 'static_replication') - 5131.566118) < 0.02);
  f.instrument.instrument.spec.instrument_pricing_overrides = {
    market_quotes: { implied_volatility: 0.4 },
  };
  // At 40% asset vol, the log mean is .10-.5*.4*.1-.5*.4*.4 = 0.
  assert.ok(Math.abs(value(f, f.as_of, 'static_replication') - 3098.112965071095) < 0.02);
  delete f.market.prices.EURUSD;
  assert.throws(() => value(f, f.as_of, 'static_replication'), /EURUSD/);
});

test('quanto range rejects the wrong monetary asset currency', () => {
  const f = read('../../../finstack-quant/valuations/tests/fixtures/production_quanto_range.json');
  f.market.prices['SPX-SPOT'].price.currency = 'USD';
  assert.throws(() => value(f, f.as_of, 'static_replication'), /currency/i);
});

test('discrete barrier cannot use the continuous analytical engine', () => {
  const f = fixture('barrier_option');
  const s = f.instrument.instrument.spec;
  s.div_yield_id = null;
  s.monitoring = { type: 'discrete', observation_dates: [s.expiry] };
  assert.throws(() => value(f, '2024-01-02', 'barrier_bs_continuous'), /continuous monitoring/);
});

test('fixed NDF exports signed settlement cash without principal', () => {
  const f = fixture('ndf');
  f.instrument.instrument.spec.observed_fixing = 7.3;
  const asOf = '2025-03-14';
  const result = valuations.instruments.instrumentCashflows(
    JSON.stringify(f.instrument),
    JSON.stringify(f.market),
    asOf,
    'discounting'
  );
  assert.equal(result.flows.length, 1);
  const [row] = result.flows;
  assert.equal(row.kind, 'derivative_settlement');
  assert.equal(row.currency, 'USD');
  assert.ok(Math.abs(row.amount - 10000000 * (1 / 7.3 - 1 / 7.25)) < 1e-8);
  assert.equal(row.principal_delta, undefined);
  assert.equal(row.principal_date, undefined);
  assert.ok(Math.abs(row.native_pv - value(f, asOf)) < 1e-8);
});

test('variance swap settles every target bar including the first overnight gap', () => {
  const f = fixture('variance_swap');
  const spec = f.instrument.instrument.spec;
  Object.assign(spec, {
    start_date: '2025-01-06',
    maturity: '2025-01-08',
    previous_close_date: '2025-01-03',
    realized_var_method: 'yang_zhang',
    open_series_id: 'bars',
    high_series_id: 'bars',
    low_series_id: 'bars',
    close_series_id: 'bars',
  });
  f.market.series = [
    {
      id: 'bars',
      interpolation: 'step',
      extrapolation: 'error',
      observations: [
        ['2025-01-03', 100],
        ['2025-01-06', 110],
        ['2025-01-07', 110],
        ['2025-01-08', 110],
      ],
    },
  ];
  const expected = 1000000 * ((252 * Math.log(1.1) ** 2) / 3 - 0.04);
  assert.ok(Math.abs(value(f, spec.maturity) - expected) < 1e-7);
  delete spec.previous_close_date;
  assert.throws(() => value(f, spec.maturity), /previous_close_date/);
});
