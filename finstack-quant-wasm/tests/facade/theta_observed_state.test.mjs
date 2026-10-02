/**
 * Theta for seasoned FX barrier and touch options holds spot fixed: the roll
 * observes the as-of spot, so the barrier state over the roll is the one that
 * spot implies. Rust owns the convention; the same cases run in
 * `finstack-quant-py/tests/test_theta_observed_state.py`.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const golden = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/fx_barrier_option/eurusd_up_out_call_3m_quantlib.json',
      import.meta.url
    )
  )
);
const AS_OF = '2026-04-30';
const ROLLED = '2026-05-01';
const MARKET = JSON.stringify(golden.market.data);

/** Copy of an instrument envelope with extra spec fields. */
function withSpec(envelope, fields) {
  const copy = structuredClone(envelope);
  Object.assign(copy.instrument.spec, fields);
  return JSON.stringify(copy);
}

/** Up one-touch on the golden EURUSD market, monitored from the valuation date. */
function touchEnvelope(fields = {}) {
  return {
    schema: 'finstack_quant.instrument/1',
    instrument: {
      type: 'fx_touch_option',
      spec: {
        attributes: {},
        barrier: 1.25,
        barrier_direction: 'up',
        base_currency: 'EUR',
        day_count: 'act_365f',
        domestic_discount_curve_id: 'USD-OIS',
        expiry: '2026-07-30',
        foreign_discount_curve_id: 'EUR-OIS',
        id: 'FXTOUCH-EURUSD-UP-THETA',
        monitoring_start_date: AS_OF,
        payout_amount: { amount: '1000000', currency: 'USD' },
        payout_timing: 'at_expiry',
        quote_currency: 'USD',
        touch_type: 'one_touch',
        vol_surface_id: 'EURUSD-BARRIER-VOL-QL',
        ...fields,
      },
    },
  };
}

test('FxBarrierOption.theta rolls the barrier state with spot held fixed', () => {
  const option = valuations.fx.FxBarrierOption.fromJson(JSON.stringify(golden.instrument));
  const theta = option.theta(MARKET, AS_OF);
  assert.ok(Number.isFinite(theta));
  assert.ok(theta < 0, `a long up-and-out call far from its barrier decays, got ${theta}`);

  // Spot 1.10 sits below the 1.25 barrier, so the roll observes no breach.
  const observed = valuations.fx.FxBarrierOption.fromJson(
    withSpec(golden.instrument, { observed_barrier_breached: false })
  );
  assert.equal(theta, observed.theta(MARKET, AS_OF));
  assert.equal(
    theta,
    valuations.instruments.priceInstrument(JSON.stringify(golden.instrument), MARKET, AS_OF, null, [
      'theta',
    ]).measures.theta
  );
  // Ordinary seasoned pricing still requires the recorded state.
  assert.throws(() => option.price(MARKET, ROLLED), /observed_barrier_breached/);
});

test('FxTouchOption.theta rolls the touch state with spot held fixed', () => {
  const option = valuations.fx.FxTouchOption.fromJson(JSON.stringify(touchEnvelope()));
  const theta = option.theta(MARKET, AS_OF);
  assert.ok(Number.isFinite(theta));
  const observed = valuations.fx.FxTouchOption.fromJson(
    JSON.stringify(touchEnvelope({ observed_barrier_breached: false }))
  );
  assert.equal(theta, observed.theta(MARKET, AS_OF));
  assert.throws(() => option.price(MARKET, ROLLED), /observed_barrier_breached/);
});

test('FxBarrierOption.greeks includes theta for an option quoted through fx_spot_id', () => {
  const market = structuredClone(golden.market.data);
  market.prices['EURUSD-SPOT'] = { unitless: 1.1 };
  const option = valuations.fx.FxBarrierOption.fromJson(
    withSpec(golden.instrument, { fx_spot_id: 'EURUSD-SPOT' })
  );
  const marketJson = JSON.stringify(market);
  const greeks = option.greeks(marketJson, AS_OF);
  assert.deepEqual(Object.keys(greeks), [
    'delta',
    'gamma',
    'vega',
    'theta',
    'rho',
    'vanna',
    'volga',
  ]);
  for (const [name, value] of Object.entries(greeks)) {
    assert.ok(Number.isFinite(value), `${name} must be finite`);
  }
  assert.equal(greeks.theta, option.theta(marketJson, AS_OF));
});
