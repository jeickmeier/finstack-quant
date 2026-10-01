/**
 * Behaviour the WASM host shares with Python for fluent mutators and for the
 * portfolio members that compute or validate in Rust.
 *
 * The same cases are asserted in
 * `finstack-quant-py/tests/test_host_behaviour_parity.py`.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, portfolio } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const throwsKind = (fn, kind, pattern) =>
  assert.throws(fn, (error) => {
    assert.equal(error.kind, kind, error.message);
    if (pattern) assert.match(error.message, pattern);
    return true;
  });

// Shared with test_host_behaviour_parity.py.
const MATRIX = {
  base_currency: 'USD',
  position_ids: ['A', 'B'],
  factor_ids: ['F1', 'F2', 'F3'],
  data: [
    [1.0, 2.0, 3.0],
    [4.0, 5.0, 6.0],
  ],
};

test('ScheduleBuilder setters update the builder in place and return it', () => {
  const builder = core.Schedule.builder(core.createDate(2025, 1, 15), core.createDate(2025, 7, 15));
  assert.equal(builder.frequency('3M'), builder);
  // Statement style (no chaining) applies the setting, as in Python.
  builder.endOfMonth(true);
  assert.equal(builder.stubRule('short_front').adjustWith('following', 'nyse'), builder);
  assert.equal(builder.paymentLagDays(2).fixingLagBusinessDays(1), builder);
  assert.equal(builder.cdsImm().imm().errorPolicy('strict'), builder);
  const spec = builder.toSpec();
  assert.deepEqual(spec.frequency, { count: 3, unit: 'months' });
  assert.equal(spec.end_of_month, true);
  assert.equal(spec.stub, 'short_front');
  assert.equal(spec.calendar_id, 'nyse');
  assert.equal(spec.payment_lag_days, 2);
  assert.equal(spec.fixing_lag_business_days, 1);
  assert.equal(spec.imm_mode, true);
  assert.equal(spec.cds_imm_mode, false);

  // A rejected setter leaves the builder unchanged and usable.
  throwsKind(() => builder.frequency('nope'), 'validation');
  throwsKind(() => builder.adjustWith('modified_following', 5), 'invalid_type');
  assert.deepEqual(builder.toSpec(), spec);
});

test('MarketContext inserts update the context in place and return it', () => {
  const market = new core.MarketContext();
  const curve = core.DiscountCurve.flat('USD-OIS', '2025-01-02', 0.04);
  assert.equal(market.insert(curve), market);
  assert.equal(market.insertPrice('SPX', 5900.0).insertPrice('SPOT', 185.25, 'USD'), market);
  assert.equal(market.mapCollateral('USD-CSA', 'USD-OIS'), market);
  assert.equal(market.contains('USD-OIS'), true);
  assert.deepEqual(market.getPrice('SPX'), { unitless: 5900.0 });
  throwsKind(() => market.insertPrice(5, 1.0), 'invalid_type');
});

test('sensitivityMatrix accessors read the wire matrix and reject out-of-range indices', () => {
  assert.equal(portfolio.sensitivityMatrixDelta(MATRIX, 1, 2), 6.0);
  assert.equal(portfolio.sensitivityMatrixDelta(JSON.stringify(MATRIX), 0, 1), 2.0);
  assert.deepEqual([...portfolio.sensitivityMatrixPositionDeltas(MATRIX, 1)], [4.0, 5.0, 6.0]);
  assert.deepEqual([...portfolio.sensitivityMatrixFactorDeltas(MATRIX, 1)], [2.0, 5.0]);
  // Row-major storage: (0, 3) would read (1, 0) without the Rust bounds check.
  throwsKind(
    () => portfolio.sensitivityMatrixDelta(MATRIX, 0, 3),
    'validation',
    /factor_idx 3 out of bounds for 3 factors/
  );
  throwsKind(
    () => portfolio.sensitivityMatrixDelta(MATRIX, 2, 0),
    'validation',
    /position_idx 2 out of bounds for 2 positions/
  );
  throwsKind(() => portfolio.sensitivityMatrixPositionDeltas(MATRIX, 2), 'validation');
  throwsKind(() => portfolio.sensitivityMatrixFactorDeltas(MATRIX, 3), 'validation');
  throwsKind(() => portfolio.sensitivityMatrixDelta(MATRIX, -1, 0), 'invalid_type');
  throwsKind(
    () => portfolio.sensitivityMatrixDelta({ ...MATRIX, data: [[1.0]] }, 0, 0),
    'validation'
  );
});

test('constraint constructors validate in Rust and return the wire object', () => {
  assert.deepEqual(portfolio.constraintBudget(1.0), { budget: { rhs: 1.0 } });
  assert.deepEqual(portfolio.constraintMaxTurnover(0.25, 'turnover'), {
    max_turnover: { label: 'turnover', max_turnover: 0.25 },
  });
  assert.deepEqual(portfolio.constraintWeightBounds('all', 0.0, 0.1), {
    weight_bounds: { label: null, filter: 'all', min: 0.0, max: 0.1 },
  });
  const limit = portfolio.constraintExposureLimit('rating', 'CCC', 0.1, 'ccc cap');
  const minimum = portfolio.constraintExposureMinimum('rating', 'CCC', 0.1, 'ccc cap');
  assert.equal(limit.metric_bound.op, 'le');
  assert.equal(limit.metric_bound.rhs, 0.1);
  assert.equal(limit.metric_bound.label, 'ccc cap');
  assert.deepEqual({ ...minimum.metric_bound, op: 'le' }, limit.metric_bound);

  throwsKind(() => portfolio.constraintBudget(-1.0), 'validation', /budget rhs/);
  throwsKind(() => portfolio.constraintBudget('1'), 'invalid_type');
  throwsKind(() => portfolio.constraintMaxTurnover(-0.1), 'validation', /max_turnover/);
  throwsKind(() => portfolio.constraintWeightBounds('all', 0.2, 0.1), 'validation', /min/);
  throwsKind(() => portfolio.constraintWeightBounds({ nope: 1 }, 0.0, 0.1), 'validation');
  throwsKind(() => portfolio.constraintExposureLimit('rating', 'CCC', 1.5), 'validation');
  throwsKind(() => portfolio.constraintExposureMinimum('rating', 'CCC', -0.5), 'validation');
});

test('portfolioMetricsRequireTotal fails with not_found for a metric that was not aggregated', () => {
  const metrics = {
    aggregated: { dv01: { metric_id: 'dv01', total: 12.5, by_entity: {} } },
    by_position: {},
  };
  assert.equal(portfolio.portfolioMetricsRequireTotal(metrics, 'dv01'), 12.5);
  assert.equal(portfolio.portfolioMetricsGetTotal(metrics, 'cs01'), undefined);
  throwsKind(
    () => portfolio.portfolioMetricsRequireTotal(metrics, 'cs01'),
    'not_found',
    /metric 'cs01'/
  );
});
