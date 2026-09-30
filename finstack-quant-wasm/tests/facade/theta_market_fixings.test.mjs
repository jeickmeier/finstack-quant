/**
 * Theta holds market-held fixings at their as-of projections. The Bloomberg
 * SWPM SOFR OIS golden is valued on its effective date, so a one-day theta
 * crosses the SOFR fixing dated the valuation date. Rust owns the convention;
 * the same case runs in `finstack-quant-py/tests/test_theta_market_fixings.py`.
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import init, { calibration, scenarios, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const golden = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant/valuations/tests/golden/data/pricing/bloomberg/irs/usd_sofr_5y_receive_fixed_swpm.json',
      import.meta.url
    )
  )
);
const AS_OF = '2026-05-04';
const ROLLED = '2026-05-05';
const INSTRUMENT = JSON.stringify(golden.instrument);
const MARKET = JSON.stringify(
  calibration.calibrate(JSON.stringify(golden.market.envelope)).result.final_market
);

function pv(marketJson, asOf) {
  return Number(
    valuations.instruments.priceInstrument(INSTRUMENT, marketJson, asOf, null, []).value.amount
  );
}

test('OIS theta equals the reprice on the one-day time-rolled market', () => {
  const theta = valuations.instruments.priceInstrument(INSTRUMENT, MARKET, AS_OF, null, [
    'theta',
  ]).measures.theta;
  assert.ok(Number.isFinite(theta), `theta ${theta}`);

  // A scenario time roll materializes the same crossed fixing (as-of
  // projection) and rolls the curves, so it rebuilds theta's rolled market.
  const spec = {
    id: 'theta-1d',
    operations: [
      { kind: 'time_roll_forward', period: '1D', apply_shocks: false, roll_mode: 'calendar_days' },
    ],
  };
  const rolled = scenarios.applyScenarioToMarket(
    JSON.stringify(spec),
    MARKET,
    AS_OF,
    JSON.stringify([golden.instrument])
  );
  assert.deepEqual(rolled.time_roll.failed_instruments, []);
  const fixing = rolled.market.series.find((series) => series.id === 'FIXING:USD-SOFR');
  assert.ok(
    fixing.observations.some(([date]) => date === AS_OF),
    'the fixing dated the valuation date is crossed'
  );

  const expected = pv(JSON.stringify(rolled.market), ROLLED) - pv(MARKET, AS_OF);
  assert.ok(Math.abs(theta - expected) < 1e-6, `theta ${theta} versus reprice ${expected}`);
});

test('rolled OIS pricing still requires the SOFR fixing', () => {
  const spec = {
    id: 'curves-only',
    operations: [
      { kind: 'time_roll_forward', period: '1D', apply_shocks: false, roll_mode: 'calendar_days' },
    ],
  };
  const rolled = scenarios.applyScenarioToMarket(JSON.stringify(spec), MARKET, AS_OF);
  assert.throws(() => pv(JSON.stringify(rolled.market), ROLLED), /FIXING:USD-SOFR/);
});
