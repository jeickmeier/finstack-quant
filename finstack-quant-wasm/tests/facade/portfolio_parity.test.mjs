/**
 * Facade tests for the portfolio entry points bound for Python parity (P8).
 *
 * `finstack-quant/portfolio/tests/fixtures/host_parity.json` holds one set of
 * inputs and the expected outputs; `finstack-quant-py/tests/test_portfolio_host_parity.py`
 * asserts the same file, so both hosts are pinned to the same numbers.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, models, portfolio } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const { inputs, expected } = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant/portfolio/tests/fixtures/host_parity.json', import.meta.url),
    'utf8'
  )
);

/** Structural equality with a tolerance for native-vs-wasm32 float drift. */
function assertClose(actual, wanted, path = '$') {
  if (typeof wanted === 'string' && /^-?\d+(\.\d+)?$/.test(wanted) && typeof actual === 'string') {
    // Money amounts are decimal strings; compare them as numbers.
    assertClose(Number(actual), Number(wanted), path);
  } else if (typeof wanted === 'number') {
    assert.equal(typeof actual, 'number', path);
    const tolerance = 1e-9 * Math.max(1, Math.abs(wanted));
    assert.ok(Math.abs(actual - wanted) <= tolerance, `${path}: ${actual} vs ${wanted}`);
  } else if (Array.isArray(wanted)) {
    assert.ok(Array.isArray(actual), path);
    assert.equal(actual.length, wanted.length, path);
    wanted.forEach((item, index) => assertClose(actual[index], item, `${path}[${index}]`));
  } else if (wanted && typeof wanted === 'object') {
    assert.ok(actual && typeof actual === 'object' && !(actual instanceof Map), path);
    assert.deepEqual(Object.keys(actual).sort(), Object.keys(wanted).sort(), path);
    for (const key of Object.keys(wanted)) assertClose(actual[key], wanted[key], `${path}.${key}`);
  } else {
    assert.equal(actual ?? null, wanted, path);
  }
}

/** A plain object survives `JSON.stringify`; an ES `Map` would not. */
const wire = (value) => JSON.parse(JSON.stringify(value));

const book = portfolio.Portfolio.fromSpec(inputs.portfolio_spec);
const marketT0 = core.MarketContext.fromJson(inputs.market_t0);
const marketT1 = core.MarketContext.fromJson(inputs.market_t1);

test('allocateWeights and its JSON twins match Python', () => {
  assertClose(wire(portfolio.allocateWeights(inputs.allocation_spec)), expected.allocate_weights);
  assertClose(
    JSON.parse(portfolio.allocateWeightsJson(JSON.stringify(inputs.allocation_spec))),
    expected.allocate_weights
  );
  assertClose(
    JSON.parse(portfolio.validateAllocationJson(inputs.allocation_spec)),
    expected.validate_allocation_json
  );
  assert.throws(
    () => portfolio.allocateWeights({ ...inputs.allocation_spec, scheme: 'nope' }),
    (error) => error.kind === 'validation'
  );
  assert.throws(() => portfolio.allocateWeights(7), TypeError);
});

test('scenarioPnlBatch matches Python and keeps input order', () => {
  const batch = portfolio.scenarioPnlBatch(book, inputs.scenarios, marketT0);
  assertClose(wire(batch), expected.scenario_pnl_batch);
  assert.deepEqual(
    batch.map((item) => item.scenario_id),
    ['up_10bp', 'down_15bp']
  );
  assert.deepEqual(portfolio.scenarioPnlBatch(book, [], marketT0), []);
  assert.throws(
    () => portfolio.scenarioPnlBatch(book, [{ id: '', operations: [] }], marketT0),
    (error) => error.kind === 'validation'
  );
});

test('attributePortfolioPnl and its result methods match Python', () => {
  const attribution = portfolio.attributePortfolioPnl(
    book,
    marketT0,
    marketT1,
    inputs.as_of_t0,
    inputs.as_of_t1,
    JSON.stringify(inputs.attribution_method)
  );
  assertClose(wire(attribution), expected.attribute_portfolio_pnl);
  assert.equal(
    portfolio.portfolioAttributionExplainText(attribution),
    expected.attribution_explain
  );
  assertClose(
    wire(
      portfolio.portfolioAttributionReconciliationCheck(
        attribution,
        inputs.reconciliation_tolerance
      )
    ),
    expected.attribution_reconciliation_check
  );
  assert.throws(
    () =>
      portfolio.attributePortfolioPnl(
        book,
        marketT0,
        marketT1,
        inputs.as_of_t0,
        inputs.as_of_t1,
        '"nope"'
      ),
    (error) => error.kind === 'validation'
  );
});

test('factorStress and positionWhatIf match Python', () => {
  assertClose(
    wire(
      portfolio.factorStress(
        book,
        marketT0,
        inputs.factor_model_config,
        inputs.as_of_t0,
        inputs.stresses
      )
    ),
    expected.factor_stress
  );
  const whatIf = portfolio.positionWhatIf(
    book,
    marketT0,
    inputs.factor_model_config,
    inputs.as_of_t0,
    inputs.changes
  );
  assertClose(wire(whatIf), expected.position_what_if);
  assert.ok(whatIf.before.total_risk > whatIf.after.total_risk);
  assert.throws(
    () =>
      portfolio.positionWhatIf(book, marketT0, inputs.factor_model_config, inputs.as_of_t0, [
        { kind: 'add', position: {} },
      ]),
    (error) => error.kind === 'validation' && /unknown variant/.test(error.message)
  );
  assert.throws(
    () =>
      portfolio.positionWhatIf(book, marketT0, inputs.factor_model_config, inputs.as_of_t0, [
        { kind: 'remove', position_id: 'NOPE' },
      ]),
    (error) => error.kind === 'validation'
  );
});

test('buildCreditVolReport matches Python', () => {
  const model = models.factor.credit.CreditFactorModel.fromJson(
    JSON.stringify(inputs.credit_model)
  );
  const report = portfolio.buildCreditVolReport(inputs.credit_decomposition, model, true);
  assertClose(wire(report), expected.build_credit_vol_report);
  assert.equal(
    portfolio.buildCreditVolReport(inputs.credit_decomposition, model, false).by_position_optional,
    null
  );
  assert.throws(
    () => portfolio.buildCreditVolReport(inputs.credit_decomposition, model, 1),
    TypeError
  );
  model.free();
});

test('valuation and metrics lookups match Python', () => {
  const valuation = portfolio.valuePortfolioBuilt(book, inputs.market_t0, undefined, ['dv01']);
  const position = portfolio.portfolioValuationGetPositionValue(valuation, 'POS-1');
  assertClose(Number(position.value_base.amount), expected.valuation_position_value_base);
  assert.equal(portfolio.portfolioValuationGetPositionValue(valuation, 'NOPE'), undefined);
  assertClose(
    Number(portfolio.portfolioValuationGetEntityValue(valuation, 'FUND').amount),
    expected.valuation_entity_value
  );
  assert.equal(portfolio.portfolioValuationGetEntityValue(valuation, 'NOPE'), undefined);

  const metrics = portfolio.aggregateMetrics(valuation, 'USD', inputs.market_t0, inputs.as_of_t0);
  assertClose(portfolio.portfolioMetricsGetTotal(metrics, 'dv01'), expected.metrics_total_dv01);
  assert.equal(portfolio.portfolioMetricsGetTotal(metrics, 'nope'), undefined);
  assertClose(
    portfolio.portfolioMetricsGetMetric(metrics, 'dv01').total,
    expected.metrics_total_dv01
  );
  assert.equal(portfolio.portfolioMetricsGetMetric(metrics, 'nope'), undefined);
  assertClose(
    portfolio.portfolioMetricsGetPositionMetrics(metrics, 'POS-1').metrics.dv01,
    expected.metrics_position_dv01
  );
  assert.equal(portfolio.portfolioMetricsGetPositionMetrics(metrics, 'NOPE'), undefined);
});

test('Portfolio.builder builds the same portfolio as Python', () => {
  const spec = inputs.portfolio_spec;
  let builder = portfolio.Portfolio.builder(spec.id, spec.base_currency, spec.as_of)
    .name('Desk book')
    .entity({ id: 'FUND', name: null });
  for (const position of spec.positions) builder = builder.position(position);
  const built = builder.tag('desk', 'rates').meta('owner', { team: 'rates' }).build();
  assertClose(JSON.parse(built.toJson()), expected.builder_spec);
  assert.equal(built.name, 'Desk book');
  assert.deepEqual(built.positionIds, ['POS-0', 'POS-1']);
  built.free();

  assert.throws(
    () => portfolio.Portfolio.builder('book', 'USD', 'not-a-date'),
    (error) => error.kind === 'validation'
  );
  assert.throws(() => portfolio.Portfolio.builder('book', 'USD', 20250101), TypeError);
  assert.throws(
    () =>
      portfolio.Portfolio.builder('book', 'USD', '2025-01-01')
        .position({ ...spec.positions[0], entity_id: 'MISSING' })
        .build(),
    (error) => error.kind === 'not_found' && /unknown entity 'MISSING'/.test(error.message)
  );
  assert.throws(
    () => portfolio.Portfolio.builder('book', 'USD', '2025-01-01').position({ nope: 1 }),
    (error) => error.kind === 'validation'
  );
});

test('optimization result methods read the wire result', () => {
  const optimized = portfolio.optimizePortfolio(
    {
      portfolio: inputs.portfolio_spec,
      objective: { maximize: { weighted_sum: { metric: { constant: 1.0 } } } },
      constraints: [{ budget: { rhs: 1.0 } }],
      weighting: 'notional_weight',
      missing_metric_policy: 'zero',
      label: null,
    },
    inputs.market_t0
  );
  assert.deepEqual(portfolio.portfolioOptimizationResultNewPositionTrades(optimized), []);
  const binding = portfolio.portfolioOptimizationResultBindingConstraints(optimized);
  assert.ok(binding.length >= 1, 'the budget constraint binds');
  for (const [label, slack] of binding) {
    assert.equal(typeof label, 'string');
    assert.ok(Math.abs(slack) < 1e-6);
  }
  assert.throws(
    () => portfolio.portfolioOptimizationResultBindingConstraints({ nope: 1 }),
    (error) => error.kind === 'validation'
  );
  assert.throws(() => portfolio.portfolioOptimizationResultNewPositionTrades(3), TypeError);
});
