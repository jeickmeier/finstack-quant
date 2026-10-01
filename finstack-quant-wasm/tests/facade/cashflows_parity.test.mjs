/**
 * Typed cashflows surface: every entry point bound for Python parity, with
 * cross-host goldens.
 *
 * Each case below computes one JSON value from fixed inputs. The values are
 * pinned in `golden/cashflows_parity.json`, and
 * `finstack-quant-py/tests/test_cashflows_wasm_parity.py` asserts the Python
 * twins against the same file, so both hosts are held to one set of numbers.
 * Regenerate deliberately with
 * `UPDATE_PARITY_GOLDEN=1 node --test tests/facade/cashflows_parity.test.mjs`, then
 * `npx prettier --write tests/facade/golden`.
 */
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { cashflows, core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const GOLDEN = new URL('./golden/cashflows_parity.json', import.meta.url);
const MARKET_JSON = readFileSync(new URL('./golden/parity_market.json', import.meta.url), 'utf8');
const UPDATE = process.env.UPDATE_PARITY_GOLDEN === '1';

const usd = (amount) => ({ amount, currency: 'USD' });
const fixedSpec = {
  coupon_type: 'cash',
  rate: '0.06',
  frequency: { count: 6, unit: 'months' },
  day_count: '30_360',
  business_day_convention: 'following',
  calendar_id: 'weekends_only',
  stub: 'none',
  end_of_month: false,
  payment_lag_days: 0,
};
const floatingSpec = {
  rate_spec: {
    forward_curve_id: 'USD-SOFR-3M',
    spread_bp: '150',
    reset_frequency: { count: 3, unit: 'months' },
    reset_lag_days: 0,
  },
  coupon_type: 'cash',
  frequency: { count: 3, unit: 'months' },
  day_count: 'act_360',
  business_day_convention: 'following',
  calendar_id: 'weekends_only',
  stub: 'none',
  end_of_month: false,
  payment_lag_days: 0,
};
const ISSUE = '2025-01-15';
const MATURITY = '2027-01-15';
// The quarters `build_periods("2025Q1..2027Q1")` yields in Python.
const periods = Array.from({ length: 9 }, (_, index) => {
  const year = 2025 + Math.floor(index / 4);
  const quarter = index % 4;
  const month = (value) => String(value).padStart(2, '0');
  return {
    id: `${year}Q${quarter + 1}`,
    start: `${year}-${month(quarter * 3 + 1)}-01`,
    end: quarter === 3 ? `${year + 1}-01-01` : `${year}-${month(quarter * 3 + 4)}-01`,
    is_actual: false,
  };
});

const bond = () =>
  cashflows.CashFlowSchedule.builder()
    .principal(usd('1000000'), ISSUE, MATURITY)
    .fixedCf(fixedSpec)
    .build();

/** Run `body` with handles that are freed afterwards. */
function withHandles(make, body) {
  const handles = make();
  try {
    return body(...handles);
  } finally {
    for (const handle of handles) handle.free();
  }
}

const withBond = (body) => withHandles(() => [bond()], body);
const withMarket = (body) => withHandles(() => [core.MarketContext.fromJson(MARKET_JSON)], body);
const json = (handle) => {
  try {
    return JSON.parse(handle.toJson());
  } finally {
    handle.free();
  }
};

const cases = {
  // --- CashFlowBuilder / CashFlowSchedule -------------------------------------
  'builder.fixed_bond': () => json(bond()),
  'builder.amortizing_with_fee_and_event': () =>
    json(
      cashflows.CashFlowSchedule.builder()
        .principal(usd('1000000'), ISSUE, MATURITY)
        .principalExchange('initial_and_final')
        .amortization(cashflows.amortizationSpecLinearTo(usd('400000')))
        .fixedCf(fixedSpec)
        .fee(cashflows.feeSpecFixed('2025-07-15', usd('2500')))
        .addPrincipalEvent('2025-10-15', '2025-10-17', usd('-100000'), 'amortization')
        .build()
    ),
  'builder.step_up': () =>
    json(
      cashflows.CashFlowSchedule.builder()
        .principal(usd('1000000'), ISSUE, MATURITY)
        .stepUpCf({
          ...fixedSpec,
          rate: undefined,
          initial_rate: '0.05',
          step_schedule: [['2026-01-15', '0.07']],
        })
        .build()
    ),
  'builder.windows_and_pik_program': () =>
    json(
      cashflows.CashFlowSchedule.builder()
        .principal(usd('1000000'), ISSUE, MATURITY)
        .addFixedWindow(ISSUE, '2026-01-15', { ...fixedSpec, rate: '0.04' })
        .addFixedWindow('2026-01-15', MATURITY, { ...fixedSpec, rate: '0.08' })
        .paymentSplitProgram([
          ['2026-01-15', 'pik'],
          [MATURITY, cashflows.couponTypeSplit('0.5', '0.5')],
        ])
        .build()
    ),
  'builder.payment_window': () =>
    json(
      cashflows.CashFlowSchedule.builder()
        .principal(usd('1000000'), ISSUE, MATURITY)
        .fixedCf(fixedSpec)
        .addPaymentWindow(ISSUE, '2026-01-15', 'pik')
        .build()
    ),
  'builder.floating_with_market': () =>
    withMarket((market) =>
      json(
        cashflows.CashFlowSchedule.builder()
          .principal(usd('1000000'), ISSUE, MATURITY)
          .floatingCf(floatingSpec)
          .build(market)
      )
    ),
  'builder.fixed_to_float': () =>
    withMarket((market) =>
      json(
        cashflows.CashFlowSchedule.builder()
          .principal(usd('1000000'), ISSUE, MATURITY)
          .fixedToFloat(
            '2026-01-15',
            { ...fixedSpec, frequency: { count: 3, unit: 'months' } },
            floatingSpec
          )
          .build(market)
      )
    ),
  'builder.float_margin_steps_and_floating_window': () =>
    withMarket((market) => [
      json(
        cashflows.CashFlowSchedule.builder()
          .principal(usd('1000000'), ISSUE, MATURITY)
          .floatMarginSteps([['2026-01-15', '250']], floatingSpec)
          .build(market)
      ),
      json(
        cashflows.CashFlowSchedule.builder()
          .principal(usd('1000000'), ISSUE, MATURITY)
          .addFloatingWindow(ISSUE, MATURITY, floatingSpec)
          .build(market)
      ),
    ]),
  'schedule.accessors': () =>
    withBond((schedule) => ({
      flows: schedule.getFlows(),
      coupons: schedule.coupons(),
      dates: schedule.dates(),
      notional: schedule.getNotional(),
      day_count: schedule.getDayCount(),
      meta: schedule.getMeta(),
    })),
  'schedule.wal': () => withBond((schedule) => schedule.wal(ISSUE)),
  'schedule.outstanding_by_date': () => withBond((schedule) => schedule.outstandingByDate()),
  'schedule.scale_amounts': () => withBond((schedule) => json(schedule.scaleAmounts(-0.25))),
  'schedule.with_representation': () =>
    withBond((schedule) => json(schedule.withRepresentation('projected'))),
  'schedule.with_notional': () =>
    withBond((schedule) => json(schedule.withNotional(cashflows.notionalPar(2_000_000, 'USD')))),
  'schedule.pv_by_period': () =>
    withBond((schedule) =>
      withMarket((market) => ({
        default_day_count: schedule.pvByPeriod(periods, market, 'USD-OIS', ISSUE),
        act_360: schedule.pvByPeriod(periods, market, 'USD-OIS', ISSUE, 'act_360'),
      }))
    ),
  'schedule.calendar_year_ladder': () =>
    withBond((schedule) =>
      schedule.calendarYearLadder(schedule.getFlows().map((_, index) => index + 0.5))
    ),
  'schedule.from_parts_and_from_flows': () =>
    withBond((schedule) => {
      const flows = schedule.getFlows();
      return [
        json(
          cashflows.CashFlowSchedule.fromParts(
            flows,
            schedule.getNotional(),
            '30_360',
            schedule.getMeta()
          )
        ),
        json(
          cashflows.CashFlowSchedule.fromFlows(
            flows.slice(1, 3),
            cashflows.notionalPar(1000, 'USD'),
            'act_365f'
          )
        ),
      ];
    }),
  build_cashflow_schedule: () =>
    withMarket((market) => {
      const spec = {
        notional: { initial: usd('1000000'), amort: 'none' },
        issue_date: ISSUE,
        maturity: MATURITY,
        coupon_program: [{ kind: 'floating', spec: floatingSpec }],
      };
      return json(cashflows.buildCashflowSchedule(spec, market));
    }),
  dated_flows: () => withBond((schedule) => cashflows.datedFlows(schedule)),
  schedule_from_flows: () => {
    const flows = [
      { date: '2025-06-30', amount: usd('1500') },
      { date: '2025-03-31', amount: usd('1000') },
    ];
    const opts = { notional_hint: usd('100000') };
    return withBond((schedule) => [
      json(cashflows.scheduleFromDatedFlows(flows, 'fee', 'act_360', opts)),
      json(cashflows.scheduleFromDatedFlows(flows, 'fixed', 'act_360')),
      json(cashflows.scheduleFromClassifiedFlows(schedule.getFlows().slice(0, 3), '30_360', opts)),
    ]);
  },
  merge_cashflow_schedules: () =>
    withBond((first) =>
      withHandles(
        () => [
          cashflows.scheduleFromDatedFlows(
            [{ date: '2025-06-30', amount: usd('1500') }],
            'fee',
            'act_360'
          ),
        ],
        (second) =>
          json(
            cashflows.mergeCashflowSchedules(
              [first, second.toJson()],
              cashflows.notionalPar(1_000_000, 'USD'),
              '30_360'
            )
          )
      )
    ),
  // --- accrual -----------------------------------------------------------------
  accrual: () =>
    withBond((schedule) =>
      withHandles(
        () => [
          cashflows.AccrualIndex.build(schedule),
          cashflows.AccrualIndex.build(schedule, {
            method: 'compounded',
            ex_coupon: { days_before_coupon: 7, calendar_id: null },
            include_pik: true,
            frequency: null,
          }),
        ],
        (linear, compounded) => ({
          amount: cashflows.accruedInterestAmount(schedule, '2025-04-15'),
          amount_compounded: cashflows.accruedInterestAmount(schedule, '2025-04-15', {
            method: 'compounded',
            ex_coupon: null,
            include_pik: true,
            frequency: null,
          }),
          index: ['2025-01-15', '2025-04-15', '2025-07-10', '2026-12-31'].map((date) =>
            linear.accruedAt(date)
          ),
          index_ex_coupon: ['2025-04-15', '2025-07-10'].map((date) => compounded.accruedAt(date)),
          ex_date_calendar_days: cashflows.exCouponRuleExDate(
            { days_before_coupon: 7, calendar_id: null },
            '2025-07-15'
          ),
          ex_date_business_days: cashflows.exCouponRuleExDate(
            { days_before_coupon: 7, calendar_id: 'usny' },
            '2025-07-15'
          ),
        })
      )
    ),
  // --- aggregation -------------------------------------------------------------
  aggregation: () => {
    const flows = [
      { date: '2025-03-15', amount: usd('100.25') },
      { date: '2025-09-15', amount: usd('50') },
      { date: '2026-02-01', amount: { amount: '75', currency: 'EUR' } },
      { date: '2026-02-02', amount: usd('0.1') },
    ];
    const aggregation = cashflows.aggregateByPeriod(flows, periods);
    return {
      by_period: aggregation,
      get_amount: [
        cashflows.periodAggregationGetAmount(aggregation, '2026Q1', 'EUR') ?? null,
        cashflows.periodAggregationGetAmount(aggregation, '2025Q1', 'EUR') ?? null,
        cashflows.periodAggregationGetAmount(aggregation, 'nope', 'USD') ?? null,
      ],
      checked: cashflows.aggregateCashflowsChecked(flows.slice(0, 2), 'USD'),
      ladder: cashflows.calendarYearLadder(
        ['2025-03-15', '2025-09-15', '2026-02-01'],
        ['fixed', 'amortization', 'fee'],
        [100.25, 50, 75],
        [99, 48.5, 70.125]
      ),
    };
  },
  // --- primitives --------------------------------------------------------------
  primitives: () => {
    const flow = {
      date: '2025-07-15',
      reset_date: null,
      amount: usd('30000'),
      kind: 'fixed',
      accrual_factor: 0.5,
      rate: 0.06,
    };
    const accrual = { start: '2025-01-15', end: '2025-07-15', day_count: '30_360' };
    return {
      parse: ['fixed', 'prepayment', 'collateral_substitution_out'].map((name) =>
        cashflows.cfKindParse(name)
      ),
      interest_like: ['fixed', 'float_reset', 'fee', 'notional'].map((kind) =>
        cashflows.cfKindIsInterestLike(kind)
      ),
      principal_like: ['fixed', 'pik', 'amortization', 'recovery'].map((kind) =>
        cashflows.cfKindIsPrincipalLike(kind)
      ),
      cash_settlement: ['fixed', 'pik', 'defaulted_notional', 'notional'].map((kind) =>
        cashflows.isCashSettlementKind(kind)
      ),
      balance_date: [
        cashflows.cashFlowGetBalanceDate(flow),
        cashflows.cashFlowGetBalanceDate(cashflows.cashFlowWithPrincipalDate(flow, '2025-07-11')),
      ],
      with_accrual: cashflows.cashFlowWithAccrual(flow, accrual),
      with_principal_delta: cashflows.cashFlowWithPrincipalDelta(flow, usd('-1000')),
    };
  },
  // --- specs ---------------------------------------------------------------------
  'specs.amortization': () => [
    cashflows.amortizationSpecLinearTo(usd('250000')),
    cashflows.amortizationSpecStepRemaining([
      ['2026-01-15', usd('750000')],
      ['2027-01-15', usd('0')],
    ]),
    cashflows.amortizationSpecPercentOfOriginalPerPeriod(0.05),
    cashflows.amortizationSpecPercentOfRemainingPerPeriod(0.1),
    cashflows.amortizationSpecLinearBetween('2025-07-15', '2027-01-15'),
    cashflows.amortizationSpecCustomPrincipal([['2026-01-15', usd('250000')]]),
  ],
  'specs.coupon_fee_floating': () => [
    cashflows.couponTypeSplit('0.6', '0.4'),
    cashflows.feeBaseUndrawn(usd('5000000')),
    cashflows.feeSpecFixed('2025-03-01', usd('25000')),
    cashflows.feeSpecPeriodicBp({
      base: cashflows.feeBaseUndrawn(usd('5000000')),
      bp: '37.5',
      frequency: { count: 3, unit: 'months' },
      day_count: 'act_360',
      business_day_convention: 'modified_following',
      calendar_id: 'usny',
    }),
    cashflows.floatingRateFallbackFixedRate('0.03'),
    cashflows.floatingLegCompoundingCompoundedInArrears(5),
    cashflows.floatingLegCompoundingCompoundedWithObservationShift(2),
    cashflows.floatingLegCompoundingCompoundedWithRateCutoff(3),
    cashflows.floatingRateSpecSofr('150'),
    cashflows.floatingRateSpecSonia('25.5'),
    cashflows.floatingRateSpecEuribor3m('-10'),
  ],
  'specs.notional': () => [
    cashflows.notionalPar(1_000_000.005, 'USD'),
    cashflows.notionalCurrency(cashflows.notionalPar(5, 'JPY')),
  ],
  'specs.default_model': () => {
    const models = [
      cashflows.defaultModelSpecConstantCdr(0.03),
      cashflows.defaultModelSpecSda(1.5),
      cashflows.defaultModelSpecCdr2pct(),
      cashflows.defaultModelSpecVector([0.01, 0.02, 0.04]),
      cashflows.defaultModelSpecCumulativeLoss([0.5, 1.5, 2.0], 0.4),
      cashflows.defaultModelSpecTiming(0.1, [15, 30, 30, 15, 10]),
    ];
    return {
      models,
      mdr: models.map((model) =>
        [1, 12, 31, 90].map((month) => cashflows.defaultModelSpecMdr(model, month))
      ),
    };
  },
  'specs.prepayment_model': () => {
    const models = [
      cashflows.prepaymentModelSpecConstantCpr(0.06),
      cashflows.prepaymentModelSpecPsa(1.5),
      cashflows.prepaymentModelSpecPsa100(),
      cashflows.prepaymentModelSpecCmbsWithLockout(60, 0.1),
      cashflows.prepaymentModelSpecAbs(0.015),
      cashflows.prepaymentModelSpecVector([0.02, 0.05, 0.08]),
    ];
    return {
      models,
      smm: models.map((model) =>
        [1, 12, 31, 90].map((month) => cashflows.prepaymentModelSpecSmm(model, month))
      ),
    };
  },
  'specs.recovery_model': () => {
    const flat = { rate: 0.4, recovery_lag: 6 };
    const vector = cashflows.recoveryModelSpecWithSeverityVector(flat, [0.7, 0.6, 0.55]);
    return {
      vector,
      recovery_rate: [flat, vector].map((model) =>
        [0, 1, 2, 12].map((month) => cashflows.recoveryModelSpecRecoveryRate(model, month))
      ),
    };
  },
  'specs.schedule_params': () => [
    cashflows.scheduleParamsQuarterlyAct360(),
    cashflows.scheduleParamsSemiannual30360(),
    cashflows.scheduleParamsAnnualActact(),
    cashflows.scheduleParamsUsdSofrSwap(),
    cashflows.scheduleParamsUsdCorporateBond(),
    cashflows.scheduleParamsUsdTreasury(),
    cashflows.scheduleParamsEurEstrSwap(),
    cashflows.scheduleParamsEurGovBond(),
    cashflows.scheduleParamsGbpSoniaSwap(),
    cashflows.scheduleParamsJpyTonaSwap(),
  ],
  // --- fixings -----------------------------------------------------------------
  materialize_fixings: () =>
    withMarket((market) =>
      withHandles(
        () => [
          cashflows.CashFlowSchedule.fromFlows([], cashflows.notionalPar(1, 'USD'), 'act_360', {
            projected_fixings: [{ series_id: 'FIXING:USD-SOFR', date: '2025-01-03', value: 0.03 }],
            representation: 'contractual',
            calendar_ids: [],
            commitment: null,
          }),
        ],
        (schedule) =>
          json(cashflows.materializeFixings(market, [schedule], '2025-01-02', '2025-01-06')).series
      )
    ),
};

const golden = UPDATE ? {} : JSON.parse(readFileSync(GOLDEN, 'utf8'));

for (const [name, compute] of Object.entries(cases)) {
  test(`cashflows parity golden: ${name}`, () => {
    // Round-trip through JSON so `undefined` and key order compare as on the wire.
    const actual = JSON.parse(JSON.stringify(compute()));
    if (UPDATE) golden[name] = actual;
    else assert.deepEqual(actual, golden[name]);
  });
}

test('cashflows parity golden: file is complete', () => {
  if (UPDATE) writeFileSync(GOLDEN, `${JSON.stringify(golden, null, 2)}\n`);
  assert.deepEqual(Object.keys(golden).sort(), Object.keys(cases).sort());
});

const kind = (expected, pattern) => (error) => {
  assert.ok(error instanceof Error, `expected an Error, got ${typeof error}: ${error}`);
  assert.equal(error.kind, expected, `kind of: ${error.message}`);
  assert.match(error.message, pattern);
  return true;
};

test('typed and JSON schedule builders agree', () => {
  const spec = {
    notional: { initial: usd('1000000'), amort: 'none' },
    issue_date: ISSUE,
    maturity: MATURITY,
    coupon_program: [{ kind: 'fixed', spec: fixedSpec }],
  };
  const fromJson = JSON.parse(cashflows.buildCashflowScheduleJson(spec));
  assert.deepEqual(json(cashflows.buildCashflowSchedule(spec)), fromJson);
  assert.deepEqual(json(cashflows.buildCashflowSchedule(JSON.stringify(spec), null)), fromJson);
  assert.deepEqual(json(bond()), fromJson);
  withBond((schedule) => {
    assert.deepEqual(json(cashflows.CashFlowSchedule.fromJson(schedule.toJson())), fromJson);
    assert.deepEqual(json(cashflows.CashFlowSchedule.fromJson(fromJson)), fromJson);
    assert.equal(cashflows.scheduleWal(schedule.toJson(), ISSUE), schedule.wal(ISSUE));
    assert.deepEqual(
      cashflows.scheduleOutstandingByDate(schedule.toJson()),
      schedule.outstandingByDate()
    );
    assert.deepEqual(
      cashflows.datedFlows(schedule),
      JSON.parse(cashflows.datedFlowsJson(schedule.toJson()))
    );
    assert.equal(
      Number(cashflows.accruedInterestAmount(schedule, '2025-04-15').amount),
      cashflows.accruedInterest(schedule.toJson(), '2025-04-15')
    );
    assert.equal(schedule.validate(), undefined);
  });
});

test('a builder setter consumes its receiver; build does not', () => {
  const start = cashflows.CashFlowSchedule.builder();
  const next = start.principal(usd('1000'), ISSUE, MATURITY);
  // The consumed receiver no longer owns a builder.
  assert.throws(() => start.fixedCf(fixedSpec));
  const first = next.build();
  const second = next.build();
  assert.equal(first.toJson(), second.toJson());
  for (const handle of [first, second, next]) handle.free();
});

test('builder and schedule errors are structured and Rust-owned', () => {
  const empty = cashflows.CashFlowSchedule.builder();
  assert.throws(() => empty.build(), kind('not_found', /call principal\(\) first/));
  empty.free();
  assert.throws(
    () => cashflows.CashFlowSchedule.builder().principal(usd('1'), 'not-a-date', MATURITY),
    kind('validation', /date/i)
  );
  assert.throws(
    () =>
      cashflows.CashFlowSchedule.builder().principal(
        { amount: 1, currency: 'USD' },
        ISSUE,
        MATURITY
      ),
    kind('validation', /initial: invalid type/)
  );
  assert.throws(
    () => cashflows.CashFlowSchedule.builder().fixedCf({ ...fixedSpec, bogus: 1 }),
    kind('validation', /spec: unknown field `bogus`/)
  );
  assert.throws(() => cashflows.CashFlowSchedule.fromJson(7), kind('invalid_type', /^json: /));
  withBond((schedule) => {
    assert.throws(() => schedule.scaleAmounts(Number.NaN), kind('validation', /scale/i));
    assert.throws(() => schedule.scaleAmounts('2'), kind('invalid_type', /^scale: /));
    assert.throws(() => schedule.calendarYearLadder([1]), kind('validation', /equal lengths/));
    assert.throws(
      () => schedule.withRepresentation('bogus'),
      kind('validation', /representation: unknown variant `bogus`/)
    );
    withMarket((market) => {
      assert.throws(
        () => schedule.pvByPeriod(periods, market, 'MISSING', ISSUE),
        kind('not_found', /MISSING/)
      );
    });
    // A floating leg without its forward curve fails with the default `error` fallback.
    assert.throws(
      () =>
        cashflows.CashFlowSchedule.builder()
          .principal(usd('1000000'), ISSUE, MATURITY)
          .floatingCf(floatingSpec)
          .build(),
      kind('not_found', /forward curve 'USD-SOFR-3M' not found/)
    );
  });
});

test('spec functions validate through Rust and reject wrong host types', () => {
  // A wire value is a plain object or bare wire string; the same document as JSON text also works.
  assert.equal(
    cashflows.notionalValidate(JSON.stringify(cashflows.notionalPar(100, 'USD'))),
    undefined
  );
  assert.equal(cashflows.cfKindIsInterestLike('"fixed"'), true);
  assert.equal(cashflows.cfKindIsInterestLike('fixed'), true);
  assert.equal(cashflows.scheduleParamsValidate(cashflows.scheduleParamsUsdSofrSwap()), undefined);
  assert.throws(
    () =>
      cashflows.scheduleParamsValidate({
        ...cashflows.scheduleParamsUsdSofrSwap(),
        calendar_id: 'nowhere',
      }),
    kind('validation', /nowhere/)
  );
  assert.equal(
    cashflows.floatingRateSpecValidate(cashflows.floatingRateSpecSofr('100')),
    undefined
  );
  assert.throws(
    () =>
      cashflows.floatingRateSpecValidate({
        ...cashflows.floatingRateSpecSofr('100'),
        reset_lag_days: -1,
      }),
    kind('validation', /reset_lag_days must be non-negative/)
  );
  assert.equal(cashflows.notionalValidate(cashflows.notionalPar(100, 'USD')), undefined);
  assert.throws(
    () =>
      cashflows.notionalValidate({
        initial: usd('100'),
        amort: cashflows.amortizationSpecLinearTo(usd('200')),
      }),
    kind('validation', /cannot exceed initial notional/)
  );
  assert.equal(cashflows.defaultModelSpecValidate(cashflows.defaultModelSpecSda(1)), undefined);
  assert.throws(
    () => cashflows.defaultModelSpecValidate(cashflows.defaultModelSpecConstantCdr(-0.1)),
    kind('validation', /must be a decimal in \[0,1\]; got -0\.1/)
  );
  assert.equal(
    cashflows.prepaymentModelSpecValidate(cashflows.prepaymentModelSpecPsa100()),
    undefined
  );
  assert.throws(
    () => cashflows.prepaymentModelSpecValidate(cashflows.prepaymentModelSpecAbs(1.5)),
    kind('validation', /abs|speed/i)
  );
  assert.equal(cashflows.recoveryModelSpecValidate({ rate: 0.4, recovery_lag: 0 }), undefined);
  assert.throws(
    () => cashflows.recoveryModelSpecValidate({ rate: 1.4, recovery_lag: 0 }),
    kind('validation', /rate/)
  );
  assert.equal(
    cashflows.cashFlowValidate({
      date: '2025-07-15',
      reset_date: null,
      amount: usd('1'),
      kind: 'fixed',
      accrual_factor: 0.5,
      rate: null,
    }),
    undefined
  );
  assert.throws(
    () =>
      cashflows.cashFlowValidate({
        date: '2025-07-15',
        reset_date: '2025-08-01',
        amount: usd('1'),
        kind: 'fixed',
        accrual_factor: 0.5,
        rate: null,
      }),
    kind('validation', /reset_date must not be after payment date/)
  );
  assert.throws(
    () => cashflows.cfKindParse('coupon-ish'),
    kind('validation', /unknown cashflow kind/)
  );
  assert.throws(() => cashflows.cfKindParse(3), kind('invalid_type', /^name: /));
  assert.throws(
    () => cashflows.cfKindIsInterestLike('coupon-ish'),
    kind('validation', /kind: unknown variant `coupon-ish`/)
  );
  assert.throws(
    () => cashflows.couponTypeSplit(0.5, '0.5'),
    kind('invalid_type', /^cashFraction: /)
  );
  assert.throws(
    () => cashflows.couponTypeSplit('half', '0.5'),
    kind('validation', /cashFraction: invalid value/)
  );
  assert.throws(
    () => cashflows.defaultModelSpecMdr(cashflows.defaultModelSpecCdr2pct(), 1.5),
    kind('invalid_type', /^seasoningMonths: /)
  );
  assert.throws(
    () => cashflows.feeSpecPeriodicBp({ bp: '10' }),
    kind('validation', /fields: missing field `base`/)
  );
  assert.throws(
    () => cashflows.notionalPar(100, 'usd!'),
    kind('validation', /currency: unknown variant `usd!`/)
  );
  assert.throws(
    () =>
      cashflows.exCouponRuleExDate({ days_before_coupon: 400, calendar_id: null }, '2025-07-15'),
    kind('validation', /366|days/)
  );
  assert.throws(
    () => cashflows.aggregateCashflowsChecked([{ date: '2025-01-01', amount: usd('1') }], 'EUR'),
    kind('validation', /currency/i)
  );
  assert.throws(
    () => cashflows.aggregateByPeriod([], [periods[1], periods[0]]),
    kind('validation', /sorted|order|overlap/i)
  );
  assert.throws(
    () => cashflows.calendarYearLadder(['2025-01-01'], ['fixed'], [1], []),
    kind('validation', /equal lengths/)
  );
  assert.throws(
    () => cashflows.mergeCashflowSchedules('nope', cashflows.notionalPar(1, 'USD'), 'act_360'),
    kind('invalid_type', /^schedules: /)
  );
  withMarket((market) => {
    assert.throws(
      () => cashflows.materializeFixings(market, [], '2025-01-06', '2025-01-02'),
      kind('validation', /forward date window/)
    );
  });
});
