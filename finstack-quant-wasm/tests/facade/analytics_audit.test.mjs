import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function dates(n) {
  return Array.from({ length: n + 1 }, (_, i) =>
    new Date(Date.UTC(2024, 0, 1 + i)).toISOString().slice(0, 10)
  );
}

function panel(values, frequency = 'daily') {
  return new analytics.Performance({
    dates: dates(values.length),
    series: [values],
    kind: 'returns',
    ticker_names: ['A'],
    frequency,
    benchmark_ticker: 'A',
  });
}

function close(actual, expected) {
  assert.ok(Math.abs(actual - expected) < 1e-12, `${actual} != ${expected}`);
}

test('tail metrics retain exact empirical probability mass', () => {
  const values = [-0.2, ...Array(99).fill(0)];
  close(panel(values).expectedShortfall(0.95)[0], -0.04);
  values[1] = 0.25;
  close(panel(values).cdar(0.95)[0], -0.04);
  close(panel([-0.2, -0.1, 0.1, 0.2]).expectedShortfall(0.625)[0], -0.25 / 1.5);
});

test('invalid scalar domains throw while insufficient history stays unavailable', () => {
  assert.throws(() => analytics.maxDrawdown([NaN, -0.2]));
  assert.throws(() => analytics.sortino([NaN, 0.01], 0, 252));
  assert.throws(() => analytics.sharpe([0, 0], NaN, 252));
  assert.ok(Number.isNaN(analytics.sharpe([0.01], 0, 252)));
  assert.ok(Number.isNaN(panel([0.01]).sharpe(0)[0]));
  assert.ok(Array.from(panel([0.01, 0.02]).rollingSharpe(0, 1, 0).values).every(Number.isNaN));
});

test('zero drawdown has undefined zero reward and signed infinite nonzero reward', () => {
  for (const metric of ['calmar', 'martinRatio']) {
    assert.ok(Number.isNaN(panel([0, 0, 0])[metric](0)[0]));
    assert.equal(panel([0.01, 0.01, 0.01])[metric](0)[0], Infinity);
  }
  for (const metric of ['martinRatio', 'burkeRatio', 'painRatio']) {
    assert.throws(() => panel([0.01, 0.01])[metric](NaN));
  }
});

test('extreme wealth preserves annualized growth and relative drawdowns', () => {
  for (const [grid, values, expected] of [
    [
      Array.from({ length: 21 }, (_, i) => `${2000 + i}-01-01`),
      Array(20).fill(-0.9),
      Math.pow(0.1, 20 / (7305 / 365)) - 1,
    ],
    [['2020-01-01', '2021-01-01', '2022-01-01'], [1e200, 1e200], Math.pow(1e200, 2 / (731 / 365))],
  ]) {
    const perf = new analytics.Performance({
      dates: grid,
      series: [values],
      kind: 'returns',
      ticker_names: ['A'],
      frequency: 'annual',
    });
    const result = perf.cagr()[0];
    assert.ok(Number.isFinite(result));
    close(result / expected, 1);
  }
  for (const values of [Array(20).fill(-0.9), Array(4).fill(1e300)]) {
    const perf = new analytics.Performance({
      dates: dates(values.length),
      series: [values, values],
      kind: 'returns',
      ticker_names: ['BENCH', 'A'],
      benchmark_ticker: 'BENCH',
    });
    for (const series of perf.cumulativeReturnsOutperformance()) {
      assert.ok(Array.from(series).every((value) => value === 0));
    }
  }
  const perf = panel([1e300, 1e300, -0.5, 0]);
  Array.from(perf.getDrawdownSeries()[0]).forEach((value, i) =>
    close(value, [0, 0, -0.5, -0.5][i])
  );
  close(perf.maxDrawdown()[0], -0.5);
  close(perf.drawdownDetails(0, 5)[0].max_drawdown, -0.5);
  close(panel([1e-18, 1e-18]).cumulativeReturns()[0][1] / 2e-18, 1);
  const nearWipeout = -0.9999999999999999;
  const mean = panel([nearWipeout, ...Array(7).fill(0)]).geometricMean()[0];
  assert.ok(mean > -1);
  close(mean, Math.pow(1 + nearWipeout, 1 / 8) - 1);
});

test('lookback returns accept the maximum calendar date', () => {
  const perf = new analytics.Performance({
    dates: ['9999-12-30', '9999-12-31'],
    series: [[100, 110]],
    kind: 'prices',
    ticker_names: ['A'],
  });
  const lookback = perf.lookbackReturns('9999-12-31');
  for (const values of [lookback.mtd, lookback.qtd, lookback.ytd, lookback.fytd])
    close(values[0], 0.1);
});

test('rolling returns preserve tiny windows after a positive outlier leaves', () => {
  const values = [0.1, 1e-18, -1e-18, 1e-18, -1e-18];
  for (const window of [1, 3]) {
    const rolling = panel(values).rollingReturns(0, window);
    assert.deepEqual(rolling.dates, dates(values.length).slice(window));
    assert.equal(rolling.value_column, 'return');
    assert.equal(rolling.values.length, values.length - window + 1);
    Array.from(rolling.values).forEach((actual, start) => {
      const expected = panel(values.slice(start, start + window)).cumulativeReturns()[0][
        window - 1
      ];
      close(actual / expected, 1);
    });
  }
});

test('multi-factor extremes return finite statistics or a JavaScript error', () => {
  const scale = 1e200;
  const factors = [[-0.2, -0.1, 0, 0.1, 0.2]];
  const fit = panel([3 * scale, scale, 3 * scale, scale, 3 * scale]).multiFactorGreeks(
    0,
    factors,
    0
  );
  close(fit.alpha / scale, 2.2 * 252);
  assert.ok(Math.abs(fit.betas[0]) / scale < 1e-12);
  close(fit.r_squared, 0);
  close(fit.adjusted_r_squared, -1 / 3);
  close(fit.residual_vol / scale, Math.sqrt((4.8 / 3) * 252));
  for (const [values, factorValues] of [
    [Array(5).fill(1e307), factors],
    [[2e199, 4e199, 6e199, 8e199, 1e200], [[2e-151, 4e-151, 6e-151, 8e-151, 1e-150]]],
  ]) {
    assert.throws(
      () => panel(values).multiFactorGreeks(0, factorValues, 0),
      (error) => {
        assert.equal(error instanceof WebAssembly.RuntimeError, false);
        assert.match(String(error), /finite/);
        return true;
      }
    );
  }
});

test('rolling Sortino preserves small returns after a positive outlier leaves', () => {
  for (const tail of [1e-12, 1e-18]) {
    for (const values of [
      [0.1, tail, -tail, tail, -tail, tail],
      [0.1, tail, tail, -tail, tail, -tail],
    ]) {
      const rolling = panel(values).rollingSortino(0, 3, 0);
      assert.deepEqual(rolling.dates, dates(values.length).slice(3));
      assert.equal(rolling.value_column, 'sortino');
      assert.equal(rolling.values.length, values.length - 2);
      Array.from(rolling.values).forEach((actual, start) => {
        const expected = analytics.sortino(values.slice(start, start + 3), 0, 252);
        if (!Number.isFinite(expected)) assert.equal(actual, expected);
        else assert.ok(Math.abs(actual - expected) <= 1e-12 * Math.max(1, Math.abs(expected)));
      });
    }
  }
});

test('integer arguments reject truncation, wrapping and non-finite values', () => {
  const perf = panel([0.01, -0.02, 0.03]);
  for (const bad of [0.9, -1, NaN, Infinity, 4294967296]) {
    for (const call of [
      () => perf.getReturnsForTicker(bad),
      () => perf.getActiveDatesForTicker(bad),
      () => perf.rollingReturns(0, bad),
      () => perf.rollingSharpe(0, bad, 0),
      () => perf.rollingSortino(0, bad, 0),
      () => perf.rollingVolatility(0, bad),
      () => perf.rollingGreeks(0, bad, 0),
      () => perf.drawdownDetails(0, bad),
      () => perf.sterlingRatio(0, bad),
      () => perf.periodStats(bad, 'daily'),
    ])
      assert.throws(call, (error) => error instanceof TypeError && error.kind === 'invalid_type');
  }
  assert.deepEqual(Array.from(perf.getReturnsForTicker(0)), [0.01, -0.02, 0.03]);
  assert.throws(() => perf.rollingReturns(0, 0), /at least 1/);
});

test('duplicate ticker labels and unrepresentable price returns are rejected', () => {
  assert.throws(
    () =>
      new analytics.Performance({
        dates: dates(2),
        series: [
          [0.01, 0.02],
          [0.03, 0.04],
        ],
        kind: 'returns',
        ticker_names: ['A', 'A'],
      }),
    /duplicate ticker/
  );
  assert.throws(
    () =>
      new analytics.Performance({
        dates: dates(1),
        series: [[1e-300, 1e300]],
        kind: 'prices',
        ticker_names: ['A'],
      }),
    /representable/
  );
});

test('drawdown dates, cash basis, Kelly and undefined factor statistics agree with Rust', () => {
  const perf = new analytics.Performance({
    dates: dates(2),
    series: [[100, 90, 100]],
    kind: 'prices',
    ticker_names: ['A'],
  });
  const episode = perf.drawdownDetails(0, 5)[0];
  assert.equal(episode.duration_days, 2);
  assert.equal(Object.hasOwn(episode, 'truncated_at_start'), false);
  close(panel([0.01, -0.02, 0.03, 0.01], 'monthly').mSquared(0.12)[0], 0.09);
  close(panel([0.02, 0.02, -0.01, 0]).periodStats(0, 'daily').kelly_risk_fraction, 0.5);
  for (const n of [5, 29, 57, 58]) {
    const fit = panel(Array(n).fill(0.01)).multiFactorGreeks(
      0,
      [Array.from({ length: n }, (_, i) => 0.001 * i)],
      0
    );
    assert.ok(Number.isNaN(fit.r_squared));
    assert.ok(Number.isNaN(fit.adjusted_r_squared));
  }
});

test('beta confidence intervals use the actual sample degrees of freedom', () => {
  const benchmark = Array.from({ length: 42 }, (_, i) => -0.02 + i * 0.001);
  const fund = benchmark.map((value, i) => 1.4 * value + (i % 2 === 0 ? 0.002 : -0.0015));
  const perf = new analytics.Performance({
    dates: dates(42),
    series: [fund, benchmark],
    kind: 'returns',
    ticker_names: ['FUND', 'BENCH'],
    benchmark_ticker: 'BENCH',
  });
  const result = perf.beta()[0];
  const critical = (result.ci_upper - result.beta) / result.std_err;
  assert.ok(Math.abs(critical - 2.0210753903062733) < 1e-9, `df=40: critical value ${critical}`);
  perf.free();
});

test('constrained and multi-factor regressions preserve zero loadings under extreme scaling', () => {
  const fSubnormal = analytics.constrainedLeastSquares(
    [1, 1e-320, 1, 2e-320, 1, 3e-320],
    2,
    [1e-320, 4e-320, 2e-320],
    [1, 0, 0]
  );
  assert.equal(fSubnormal[0], 0);
  close(fSubnormal[1], 1);

  const fLarge = analytics.constrainedLeastSquares(
    [1e308, 1, 1e308, 2, 1e308, 3],
    2,
    [0.01, 0.04, 0.02],
    [1, 0, 0]
  );
  assert.equal(fLarge[0], 0);
  close(fLarge[1], 0.01);

  const fit = analytics.multiFactorGreeks(
    [2e-320, 1e-320, 2e-320, 5e-320],
    [[1e-320, 2e-320, 3e-320, 4e-320]],
    1,
    'excess'
  );
  assert.equal(fit.alpha, 0);
  close(fit.betas[0], 1);
  close(fit.r_squared, 5 / 9);

  assert.throws(
    () =>
      analytics.constrainedLeastSquares(
        [1e-305, 0, 1e-305, 0, 0, 1e308],
        2,
        [1e-305, 3e-305, 1e-320],
        [1, 0, 0]
      ),
    /underflowed/
  );
});
