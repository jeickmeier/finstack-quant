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
    amort: 'none',
  },
  issue: '2024-08-31',
  maturity: '2025-08-31',
  coupon_program: [
    {
      kind: 'fixed',
      spec: {
        coupon_type: 'cash',
        rate: '0.06',
        frequency: { count: 12, unit: 'months' },
        day_count: '30_360',
        business_day_convention: 'following',
        calendar_id: 'weekends_only',
        stub: 'none',
        end_of_month: false,
        payment_lag_days: 0,
      },
    },
  ],
});

const EXPORTED_KEYS = [
  'accruedInterest',
  'buildCashflowScheduleJson',
  'cdrToMdr',
  'cprToSmm',
  'datedFlowsJson',
  'mdrToCdr',
  'smmToCpr',
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

  const accrued = cashflows.accruedInterest(scheduleJson, '2025-02-28', null);
  assert.equal(typeof accrued, 'number');
  assert.ok(Number.isFinite(accrued));
  assert.ok(accrued > 0);

  const instrument = JSON.parse(
    valuations.instruments.bondFromCashflowsJson('CUSTOM-CF', scheduleJson, 'USD-OIS', 99.0)
  );
  assert.equal(instrument.schema, 'finstack_quant.instrument/1');
  assert.equal(instrument.instrument.type, 'bond');
  assert.equal(cashflows.bondFromCashflowsJson, undefined);
});

test('cashflows facade builds fixed-to-float and preserves Rust window errors', () => {
  const schedule = {
    frequency: { count: 3, unit: 'months' },
    day_count: 'act_360',
    business_day_convention: 'following',
    calendar_id: 'weekends_only',
    stub: 'none',
    end_of_month: false,
    payment_lag_days: 0,
  };
  const floating = {
    rate_spec: {
      index_id: 'TEST-INDEX',
      spread_bp: '150',
      reset_frequency: { count: 3, unit: 'months' },
      reset_lag_days: 0,
      fallback: 'spread_only',
    },
    coupon_type: 'cash',
    ...schedule,
  };
  const base = {
    notional: {
      initial: { amount: '1000000', currency: 'USD' },
      amort: 'none',
    },
    issue: '2025-01-01',
    maturity: '2027-01-01',
  };
  const fixedToFloat = JSON.stringify({
    ...base,
    coupon_program: [
      {
        kind: 'fixed_to_float',
        switch: '2026-01-01',
        fixed: { coupon_type: 'cash', rate: '0.04', ...schedule },
        floating,
      },
    ],
  });
  const built = JSON.parse(cashflows.buildCashflowScheduleJson(fixedToFloat, null));
  const kinds = new Set(built.flows.map((flow) => flow.kind));
  assert.ok(kinds.has('fixed'));
  assert.ok(kinds.has('float_reset'));

  const overlapping = JSON.stringify({
    ...base,
    coupon_program: [{ kind: 'fixed', spec: { coupon_type: 'cash', rate: '0.04', ...schedule } }],
    payment_program: [
      {
        kind: 'window',
        start: '2025-01-01',
        end: '2026-06-01',
        split: 'pik',
      },
      {
        kind: 'window',
        start: '2026-01-01',
        end: '2027-01-01',
        split: 'cash',
      },
    ],
  });
  assert.throws(
    () => cashflows.buildCashflowScheduleJson(overlapping, null),
    /overlapping payment windows/
  );
});

test('cashflows rejects malformed schedule JSON', () => {
  assert.throws(() => cashflows.validateCashflowScheduleJson('{not json'), /invalid/);
});

test('cashflows preserves principal deltas and accrual calendars through JSON', () => {
  const spec = JSON.parse(cashflowSpec);
  spec.issue = '2025-01-01';
  spec.maturity = '2025-04-01';
  Object.assign(spec.coupon_program[0].spec, {
    rate: '0.1',
    frequency: { count: 3, unit: 'months' },
    day_count: 'bus_252',
    business_day_convention: 'unadjusted',
  });
  spec.principal_events = [
    {
      date: '2025-02-01',
      kind: 'notional',
      delta: { amount: '100', currency: 'USD' },
      cash: { amount: '98', currency: 'USD' },
    },
  ];
  const raw = cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null);
  const built = JSON.parse(cashflows.validateCashflowScheduleJson(raw));
  const draw = built.flows.find((flow) => flow.date === '2025-02-01');
  assert.equal(Number(draw.principal_delta.amount), 100);
  assert.equal(Number(draw.amount.amount), -98);
  assert.equal(
    built.flows.find((flow) => flow.kind === 'fixed').accrual.calendar_id,
    'weekends_only'
  );
  assert.ok(
    Math.abs(cashflows.accruedInterest(raw, '2025-02-01', null) - (100000 * 23) / 252) < 1e-8
  );
});

test('cashflows retains earned accrual until the delayed payment', () => {
  const spec = JSON.parse(cashflowSpec);
  spec.issue = '2025-01-01';
  spec.maturity = '2025-07-01';
  Object.assign(spec.coupon_program[0].spec, {
    rate: '0.1',
    frequency: { count: 6, unit: 'months' },
    day_count: 'act_360',
    business_day_convention: 'unadjusted',
    payment_lag_days: 2,
  });
  const raw = cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null);
  assert.ok(
    Math.abs(cashflows.accruedInterest(raw, '2025-07-02', null) - (100000 * 181) / 360) < 1e-8
  );
  assert.equal(cashflows.accruedInterest(raw, '2025-07-03', null), 0);
});

test('cashflows accrual preserves ICMA reference periods and ISDA termination dates', () => {
  for (const [issue, maturity, dayCount, asOf, expected] of [
    ['2025-01-15', '2025-10-15', 'act_act_isma', '2025-07-15', 50000],
    ['2024-08-31', '2025-02-28', '30e_360_isda', '2025-01-31', (100000 * 150) / 360],
  ]) {
    const spec = JSON.parse(cashflowSpec);
    Object.assign(spec, { issue, maturity });
    Object.assign(spec.coupon_program[0].spec, {
      rate: '0.1',
      frequency: { count: 6, unit: 'months' },
      day_count: dayCount,
      business_day_convention: 'unadjusted',
      stub: 'long_back',
    });
    const raw = cashflows.validateCashflowScheduleJson(
      cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null)
    );
    const cfg = JSON.stringify({
      method: 'linear',
      ex_coupon: null,
      include_pik: true,
      frequency: { count: 6, unit: 'months' },
    });
    assert.ok(Math.abs(cashflows.accruedInterest(raw, asOf, cfg) - expected) < 1e-8);
  }
});
