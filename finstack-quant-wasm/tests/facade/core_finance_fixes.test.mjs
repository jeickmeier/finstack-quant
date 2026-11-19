/** Public facade regressions for core financial conventions and numerics. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, portfolio } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('monotone-convex interpolation uses uncollared Hagan-West on a flat first strip', () => {
  const curve = new core.DiscountCurve({
    id: 'USD-OIS',
    baseDate: '2025-01-01',
    knots: [0, 1, 1, 1, 2, Math.exp(-0.005), 3, Math.exp(-0.035)],
    interp: 'monotone_convex',
    extrapolation: 'extend',
    dayCount: 'act_365f',
  });
  try {
    assert.equal(curve.df(0), 1);
    assert.equal(curve.df(1), 1);
    for (const t of [0.1, 1 / 3, 0.5, 0.9]) {
      const expected = Math.exp(0.00125 * t * (1 - t) * (1 + t));
      assert.ok(Math.abs(curve.df(t) - expected) < 1e-14);
    }
  } finally {
    curve.free();
  }
});

test('ACT/ACT ICMA enforces contained single-period slices and preserves roll day across February', () => {
  const dc = core.DayCount.actActIsma();
  const start = core.createDate(2025, 1, 30);
  const february = core.createDate(2025, 2, 28);
  const end = core.createDate(2025, 3, 15);
  const march30 = core.createDate(2025, 3, 30);
  const monthly = core.Tenor.monthly().toString();
  const firstCtx = new core.DayCountContext(null, monthly, null, [start, february]);
  const secondCtx = new core.DayCountContext(null, monthly, null, [february, march30]);
  try {
    assert.throws(() => dc.yearFraction(start, end, firstCtx), /coupon_period/);
    const total =
      dc.yearFraction(start, february, firstCtx) + dc.yearFraction(february, end, secondCtx);
    assert.ok(Math.abs(total - 0.125) < 1e-14);
  } finally {
    firstCtx.free();
    secondCtx.free();
    dc.free();
  }
});

test('ACT/365L partial accrual uses the enclosing coupon denominator', () => {
  const dc = core.DayCount.act365l();
  const start = core.createDate(2023, 10, 15);
  const middle = core.createDate(2023, 12, 15);
  const end = core.createDate(2024, 4, 15);
  const semiAnnual = core.Tenor.semiAnnual().toString();
  const frequency = new core.DayCountContext(null, semiAnnual);
  const context = new core.DayCountContext(null, semiAnnual, null, [start, end]);
  const validation = (fragment) => (error) => {
    assert.equal(error.kind, 'validation');
    assert.match(error.message, fragment);
    return true;
  };
  try {
    const partial = dc.yearFraction(start, middle, context);
    const remaining = dc.yearFraction(middle, end, context);
    assert.ok(Math.abs(partial - 61 / 366) < 1e-14);
    assert.ok(Math.abs(partial + remaining - dc.yearFraction(start, end, context)) < 1e-14);
    assert.throws(() => dc.yearFraction(start, middle), validation(/coupon_period/));
    assert.throws(() => dc.yearFraction(start, middle, frequency), validation(/coupon_period/));
  } finally {
    context.free();
    frequency.free();
    dc.free();
  }
});

test('ICMA rejects reference dates that do not share the nominal coupon grid', () => {
  const dc = core.DayCount.actActIsma();
  const context = new core.DayCountContext(null, core.Tenor.semiAnnual().toString(), null, [
    core.createDate(2025, 1, 15),
    core.createDate(2025, 7, 16),
  ]);
  try {
    assert.throws(
      () => dc.yearFraction(core.createDate(2024, 7, 15), core.createDate(2025, 1, 15), context),
      (error) => {
        assert.equal(error.kind, 'validation');
        assert.match(error.message, /unadjusted/);
        return true;
      }
    );
  } finally {
    context.free();
    dc.free();
  }
});

test('Hong Kong and Japanese date adjustment observes substitutions and collisions', () => {
  for (const code of ['hkex', 'hkhk']) {
    assert.equal(
      core.adjust(core.createDate(2026, 4, 3), 'following', code),
      core.createDate(2026, 4, 8)
    );
    for (const [month, day] of [
      [5, 25],
      [6, 19],
      [10, 19],
    ]) {
      const holiday = core.createDate(2026, month, day);
      assert.ok(core.adjust(holiday, 'following', code) > holiday);
    }
  }
  for (const code of ['jpx', 'jpto']) {
    assert.equal(
      core.adjust(core.createDate(2026, 5, 5), 'following', code),
      core.createDate(2026, 5, 7)
    );
    assert.equal(
      core.adjust(core.createDate(2026, 9, 22), 'following', code),
      core.createDate(2026, 9, 24)
    );
  }
});

test('XIRR ignores zero dated anchors and rejects rootless cashflows', () => {
  const flows = [
    { date: '2030-01-01', amount: -100 },
    { date: '2031-01-01', amount: 110 },
  ];
  const zero = { date: '2025-01-01', amount: 0 };
  assert.ok(
    Math.abs(
      portfolio.mwrXirr(JSON.stringify([zero, ...flows])) - portfolio.mwrXirr(JSON.stringify(flows))
    ) < 1e-12
  );
  for (const amounts of [
    [-100, 100, -100],
    [-1, 10000, -100000000],
  ]) {
    const rootless = amounts.map((amount, i) => ({ date: `${2030 + i}-01-01`, amount }));
    assert.throws(() => portfolio.mwrXirr(JSON.stringify([zero, ...rootless])));
  }
});

test('volatility-index queries reject negative extensions and forbidden tails', () => {
  const params = {
    id: 'VIX',
    dayCount: 'act_365f',
    interp: 'linear',
    baseDate: '2025-01-01',
    knots: [0, 30, 0.25, 20],
  };
  for (const extrapolation of ['extend', 'error']) {
    const curve = new core.PriceCurve({ ...params, kind: 'vol_index', extrapolation });
    try {
      assert.equal(curve.price(0), 30);
      assert.equal(curve.price(0.25), 20);
      for (const query of [1, '2026-01-01']) assert.throws(() => curve.price(query));
    } finally {
      curve.free();
    }
  }
  const commodity = new core.PriceCurve({ ...params, kind: 'price', extrapolation: 'extend' });
  try {
    assert.equal(commodity.price(1), -10);
  } finally {
    commodity.free();
  }
});

test('dividend schedules round-trip entitlement and payment dates and reject mixed currency', () => {
  const empty = new core.MarketContext();
  const state = JSON.parse(empty.toJson());
  empty.free();
  const event = {
    event_id: 'Q1',
    ex_date: '2025-04-01',
    pay_date: '2025-07-01',
    amount: { amount: '10', currency: 'USD' },
  };
  state.dividends = [
    { id: 'EQ-DIV', underlying: 'EQ', share_basis: 'EQ', currency: 'USD', events: [event] },
  ];
  const restored = core.MarketContext.fromJson(JSON.stringify(state));
  try {
    assert.deepEqual(JSON.parse(restored.toJson()).dividends, state.dividends);
  } finally {
    restored.free();
  }
  event.amount.currency = 'JPY';
  assert.throws(() => core.MarketContext.fromJson(JSON.stringify(state)));
});
