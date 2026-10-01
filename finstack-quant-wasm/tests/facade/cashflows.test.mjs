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
  issue_date: '2024-08-31',
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

test('cashflows CDS roll conserves interest across an unchanged payment election', () => {
  const spec = JSON.parse(cashflowSpec);
  spec.issue_date = '2024-03-20';
  spec.maturity = '2024-12-20';
  Object.assign(spec.coupon_program[0].spec, {
    rate: '0.05',
    frequency: { count: 3, unit: 'months' },
    day_count: 'act_360',
    business_day_convention: 'unadjusted',
    stub: 'short_back',
    roll_rule: 'cds_imm',
  });
  const couponTotal = (value) =>
    JSON.parse(cashflows.buildCashflowScheduleJson(JSON.stringify(value), null))
      .flows.filter((flow) => flow.kind === 'fixed' || flow.kind === 'stub')
      .reduce((total, flow) => total + Number(flow.amount.amount), 0);
  const before = couponTotal(spec);
  spec.payment_program = [
    { kind: 'window', start: '2024-06-10', end: spec.maturity, split: 'cash' },
  ];
  assert.ok(Math.abs(before - (1000000 * 0.05 * 275) / 360) < 1e-8);
  assert.ok(Math.abs(couponTotal(spec) - before) < 1e-8);
});

test('cashflows IMM coupons cover the contractual off-roll maturity', () => {
  const spec = JSON.parse(cashflowSpec);
  spec.issue_date = '2025-01-15';
  spec.maturity = '2025-12-31';
  Object.assign(spec.coupon_program[0].spec, {
    rate: '0.05',
    frequency: { count: 3, unit: 'months' },
    day_count: 'act_360',
    business_day_convention: 'unadjusted',
    stub: 'short_back',
    roll_rule: 'imm',
  });
  const schedule = JSON.parse(cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null));
  const coupons = schedule.flows.filter((flow) => flow.kind === 'fixed' || flow.kind === 'stub');
  assert.equal(coupons.at(-1).accrual.end, spec.maturity);
  const total = coupons.reduce((amount, flow) => amount + Number(flow.amount.amount), 0);
  assert.ok(Math.abs(total - (1000000 * 0.05 * 350) / 360) < 1e-8);
});

test('cashflows conserves exact decimal custom repayments through the JSON bridge', () => {
  const spec = JSON.parse(cashflowSpec);
  spec.issue_date = '2025-01-01';
  spec.maturity = '2025-07-01';
  spec.coupon_program = [];
  spec.notional = {
    initial: { amount: '0.30', currency: 'USD' },
    amort: {
      custom_principal: {
        items: [
          ['2025-04-01', { amount: '0.10', currency: 'USD' }],
          ['2025-07-01', { amount: '0.20', currency: 'USD' }],
        ],
      },
    },
  };
  const scheduleJson = cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null);
  cashflows.validateCashflowScheduleJson(scheduleJson);
  const settlements = JSON.parse(cashflows.datedFlowsJson(scheduleJson));
  const total = settlements.reduce((amount, flow) => amount + Number(flow.amount.amount), 0);
  assert.ok(Math.abs(total) < 1e-15);
});

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
      forward_curve_id: 'TEST-INDEX',
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
    issue_date: '2025-01-01',
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

test('cashflows step-up rates use contractual starts with adjusted accrual dates', () => {
  for (const [issueDate, maturity, convention, expectedRate] of [
    ['2025-01-05', '2025-07-05', 'following', 0.04],
    ['2025-01-06', '2025-07-06', 'preceding', 0.08],
  ]) {
    const spec = JSON.parse(cashflowSpec);
    Object.assign(spec, { issue_date: issueDate, maturity });
    spec.coupon_program[0].kind = 'step_up';
    const coupon = spec.coupon_program[0].spec;
    delete coupon.rate;
    Object.assign(coupon, {
      initial_rate: '0.04',
      step_schedule: [['2025-04-06', '0.08']],
      frequency: { count: 3, unit: 'months' },
      day_count: 'act_360',
      business_day_convention: convention,
      adjust_accrual_dates: true,
      stub: 'short_back',
    });
    const built = JSON.parse(
      cashflows.validateCashflowScheduleJson(
        cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null)
      )
    );
    const lastCoupon = built.flows
      .filter((flow) => flow.kind === 'fixed' || flow.kind === 'stub')
      .at(-1);
    assert.equal(lastCoupon.rate, expectedRate);
    assert.ok(
      Math.abs(
        Number(lastCoupon.amount.amount) - 1000000 * lastCoupon.accrual_factor * expectedRate
      ) < 1e-8
    );
  }
});

test('cashflows rejects principal events after the adjusted terminal accrual date', () => {
  for (const [kind, amount] of [
    ['notional', '100000'],
    ['amortization', '-100000'],
  ]) {
    const spec = JSON.parse(cashflowSpec);
    Object.assign(spec, { issue_date: '2024-12-31', maturity: '2025-03-30' });
    Object.assign(spec.coupon_program[0].spec, {
      frequency: { count: 3, unit: 'months' },
      day_count: 'act_360',
      business_day_convention: 'preceding',
      adjust_accrual_dates: true,
      stub: 'short_back',
    });
    spec.principal_events = [
      {
        date: '2025-03-30',
        payment_date: '2025-03-30',
        kind,
        delta: { amount, currency: 'USD' },
      },
    ];
    assert.throws(
      () => cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null),
      (error) => {
        assert.equal(error.kind, 'validation');
        assert.match(error.message, /effective terminal accrual date 2025-03-28/);
        return true;
      }
    );
    spec.coupon_program[0].spec.adjust_accrual_dates = false;
    cashflows.validateCashflowScheduleJson(
      cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null)
    );
  }
});

test('cashflows nonbinding overnight period cap preserves changing-balance interest', () => {
  const spec = JSON.parse(cashflowSpec);
  Object.assign(spec, { issue_date: '2025-01-06', maturity: '2025-01-08' });
  spec.coupon_program[0].kind = 'floating';
  const coupon = spec.coupon_program[0].spec;
  delete coupon.rate;
  Object.assign(coupon, {
    frequency: { count: 3, unit: 'months' },
    day_count: 'act_360',
    business_day_convention: 'unadjusted',
    stub: 'short_back',
    rate_spec: {
      forward_curve_id: 'RFR',
      spread_bp: '0',
      reset_frequency: { count: 3, unit: 'months' },
      reset_lag_days: 0,
      compounding: { compounded_in_arrears: { lookback_days: 0 } },
      overnight_index_constraints: 'period',
    },
  });
  spec.principal_events = [
    {
      date: '2025-01-07',
      payment_date: '2025-01-07',
      kind: 'amortization',
      delta: { amount: '-500000', currency: 'USD' },
    },
  ];
  const market = JSON.stringify({
    schema_version: 1,
    curves: [
      {
        type: 'forward',
        id: 'RFR',
        base: '2025-01-06',
        reset_lag: 0,
        day_count: 'act_360',
        tenor: 1 / 360,
        knot_points: [
          [0, 0.2],
          [1 / 360, 0],
          [1, 0],
        ],
        projection_grid: null,
        interp_style: 'linear',
        extrapolation: 'flat_forward',
        rate_calibration: null,
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
  });
  const unbounded = JSON.parse(cashflows.buildCashflowScheduleJson(JSON.stringify(spec), market));
  coupon.rate_spec.index_cap_bp = '600';
  const capped = JSON.parse(
    cashflows.validateCashflowScheduleJson(
      cashflows.buildCashflowScheduleJson(JSON.stringify(spec), market)
    )
  );
  const interest = (schedule) =>
    schedule.flows
      .filter((flow) => flow.kind === 'float_reset')
      .reduce((sum, flow) => sum + Number(flow.amount.amount), 0);
  assert.ok(Math.abs(interest(unbounded) - (1000000 * 0.1) / 360) < 1e-8);
  assert.ok(Math.abs(interest(capped) - interest(unbounded)) < 1e-8);
});

test('cashflows rejects malformed schedule JSON', () => {
  assert.throws(() => cashflows.validateCashflowScheduleJson('{not json'), /invalid/);
});

test('cashflows rejects invalid compounded rates and supports valid negative rates', () => {
  const spec = JSON.parse(cashflowSpec);
  spec.issue_date = '2025-01-01';
  spec.maturity = '2025-07-01';
  Object.assign(spec.coupon_program[0].spec, {
    frequency: { count: 6, unit: 'months' },
    business_day_convention: 'unadjusted',
  });
  const config = JSON.stringify({
    method: 'compounded',
    ex_coupon: null,
    include_pik: true,
    frequency: { count: 6, unit: 'months' },
  });
  for (const rate of ['-2', '-4']) {
    spec.coupon_program[0].spec.rate = rate;
    const schedule = cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null);
    assert.throws(
      () => cashflows.accruedInterest(schedule, '2025-04-01', config),
      (error) => {
        assert.ok(error instanceof Error);
        assert.equal(error.name, 'FinstackError');
        assert.equal(error.kind, 'validation');
        assert.match(error.message, /period rate.*greater than -1/);
        return true;
      }
    );
  }
  spec.coupon_program[0].spec.rate = '-0.4';
  const schedule = cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null);
  const accrued = cashflows.accruedInterest(schedule, '2025-04-01', config);
  assert.ok(Math.abs(accrued - 1000000 * (Math.sqrt(0.8) - 1)) < 1e-8);
});

test('cashflows preserves principal deltas and accrual calendars through JSON', () => {
  const spec = JSON.parse(cashflowSpec);
  spec.issue_date = '2025-01-01';
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
      payment_date: '2025-02-03',
      kind: 'notional',
      delta: { amount: '100', currency: 'USD' },
      cash: { amount: '98', currency: 'USD' },
    },
  ];
  const raw = cashflows.buildCashflowScheduleJson(JSON.stringify(spec), null);
  const built = JSON.parse(cashflows.validateCashflowScheduleJson(raw));
  const draw = built.flows.find((flow) => flow.date === '2025-02-03' && flow.kind === 'notional');
  assert.equal(draw.principal_date, '2025-02-01');
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
  spec.issue_date = '2025-01-01';
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
    Object.assign(spec, { issue_date: issue, maturity });
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
