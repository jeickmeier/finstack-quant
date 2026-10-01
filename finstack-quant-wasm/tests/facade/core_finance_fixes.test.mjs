/** Public facade regressions for core financial conventions and numerics. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, portfolio } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('monotone-convex interpolation preserves a flat zero-forward strip', () => {
  const curve = new core.DiscountCurve({
    id: 'USD-OIS',
    baseDate: '2025-01-01',
    knots: [0, 1, 1, 1, 2, Math.exp(-0.005), 3, Math.exp(-0.035)],
  });
  try {
    for (const t of [0.1, 1 / 3, 0.5, 0.9]) {
      assert.ok(Math.abs(curve.df(t) - 1) < 1e-14);
    }
  } finally {
    curve.free();
  }
});

test('ACT/ACT ICMA keeps the January roll day after February clamping', () => {
  const dc = core.DayCount.actActIsma();
  const start = core.createDate(2025, 1, 30);
  const february = core.createDate(2025, 2, 28);
  const end = core.createDate(2025, 3, 15);
  const base = new core.DayCountContext();
  const tenor = core.Tenor.monthly();
  const frequency = base.withFrequency(tenor);
  const context = frequency.withCouponPeriod(start, february);
  try {
    assert.ok(Math.abs(dc.yearFractionWithContext(start, end, context) - 0.125) < 1e-14);
  } finally {
    context.free();
    frequency.free();
    tenor.free();
    base.free();
    dc.free();
  }
});

test('ACT/365L partial accrual uses the enclosing coupon denominator', () => {
  const dc = core.DayCount.act365l();
  const start = core.createDate(2023, 10, 15);
  const middle = core.createDate(2023, 12, 15);
  const end = core.createDate(2024, 4, 15);
  const base = new core.DayCountContext();
  const tenor = core.Tenor.semiAnnual();
  const frequency = base.withFrequency(tenor);
  const context = frequency.withCouponPeriod(start, end);
  try {
    const partial = dc.yearFractionWithContext(start, middle, context);
    const remaining = dc.yearFractionWithContext(middle, end, context);
    assert.ok(Math.abs(partial - 61 / 366) < 1e-14);
    assert.ok(
      Math.abs(partial + remaining - dc.yearFractionWithContext(start, end, context)) < 1e-14
    );
    assert.throws(() => dc.yearFraction(start, middle), /frequency/);
    assert.throws(() => dc.yearFractionWithContext(start, middle, frequency), /coupon_period/);
  } finally {
    context.free();
    frequency.free();
    tenor.free();
    base.free();
    dc.free();
  }
});

test('ICMA rejects reference dates that do not share the nominal coupon grid', () => {
  const dc = core.DayCount.actActIsma();
  const base = new core.DayCountContext();
  const tenor = core.Tenor.semiAnnual();
  const frequency = base.withFrequency(tenor);
  const context = frequency.withCouponPeriod(
    core.createDate(2025, 1, 15),
    core.createDate(2025, 7, 16)
  );
  try {
    assert.throws(
      () =>
        dc.yearFractionWithContext(
          core.createDate(2024, 7, 15),
          core.createDate(2025, 1, 15),
          context
        ),
      /unadjusted/
    );
  } finally {
    context.free();
    frequency.free();
    tenor.free();
    base.free();
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
