/**
 * Cashflows-namespace facade runtime tests.
 *
 * Loads the public facade (`index.js` + `exports/cashflows.js`), initializes
 * the web-target wasm module from bytes, asserts every exported key is a
 * live function (a renamed `js_name` would silently export `undefined`), and
 * exercises an end-to-end build/validate/flows/accrual round trip from a
 * JSON spec fixture.
 *
 * The spec fixture is byte-identical to `_cashflow_spec()` in
 * `finstack-quant-py/tests/test_cashflows.py` so the Python and WASM surfaces stay
 * directly comparable (cross-language determinism).
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = join(__dirname, '..', '..', 'pkg');
const WASM_BG = join(PKG_DIR, 'finstack_quant_wasm_bg.wasm');

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

const facade = await import('../../index.js');
const init = facade.default;
const { cashflows, valuations } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

// Same fixture as finstack-quant-py/tests/test_cashflows.py::_cashflow_spec().
const cashflowSpec = JSON.stringify({
  notional: {
    initial: { amount: '1000000', currency: 'USD' },
    amort: 'None',
  },
  issue: '2024-08-31',
  maturity: '2025-08-31',
  coupon_program: [
    {
      kind: 'fixed',
      spec: {
        coupon_type: 'Cash',
        rate: '0.06',
        freq: { count: 12, unit: 'months' },
        dc: 'Thirty360',
        bdc: 'following',
        calendar_id: 'weekends_only',
        stub: 'None',
        end_of_month: false,
        payment_lag_days: 0,
      },
    },
  ],
});

const EXPORTED_KEYS = [
  'accruedInterestJson',
  'buildCashflowScheduleJson',
  'datedFlowsJson',
  'validateCashflowScheduleJson',
];

test('cashflows namespace exposes exactly the contract surface as functions', () => {
  for (const key of EXPORTED_KEYS) {
    assert.equal(
      typeof cashflows[key],
      'function',
      `cashflows.${key} must be a function (got ${typeof cashflows[key]})`
    );
  }
  assert.deepEqual(Object.keys(cashflows).sort(), EXPORTED_KEYS);
});

test('cashflows end-to-end build/validate/flows/accrual from JSON spec', () => {
  const scheduleJson = cashflows.buildCashflowScheduleJson(cashflowSpec, null);
  const schedule = JSON.parse(scheduleJson);
  assert.equal(schedule.meta.issue_date, '2024-08-31');

  // Deterministic: a second build from the same spec is byte-identical.
  assert.equal(cashflows.buildCashflowScheduleJson(cashflowSpec, null), scheduleJson);

  const validated = cashflows.validateCashflowScheduleJson(scheduleJson);
  assert.deepEqual(JSON.parse(validated), schedule);

  const flows = JSON.parse(cashflows.datedFlowsJson(scheduleJson));
  assert.equal(flows.length, schedule.flows.length);

  const accrued = cashflows.accruedInterestJson(scheduleJson, '2025-02-28', null);
  assert.equal(typeof accrued, 'number');
  assert.ok(Number.isFinite(accrued));
  assert.ok(accrued > 0);

  const instrument = JSON.parse(
    valuations.instruments.bondFromCashflowsJson('CUSTOM-CF', scheduleJson, 'USD-OIS', 99.0)
  );
  assert.equal(instrument.type, 'bond');
  assert.equal(cashflows.bondFromCashflowsJson, undefined);
});

test('cashflows rejects malformed schedule JSON', () => {
  assert.throws(() => cashflows.validateCashflowScheduleJson('{not json'), /invalid/);
});
