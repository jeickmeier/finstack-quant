import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { statements, statements_analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function model(nodes) {
  return {
    id: 'senior-review',
    schema_version: 1,
    periods: [1, 2, 3].map((q) => ({
      id: `2025Q${q}`,
      start: `2025-${String(q * 3 - 2).padStart(2, '0')}-01`,
      end: `2025-${String(q * 3 + 1).padStart(2, '0')}-01`,
      is_actual: false,
    })),
    nodes,
  };
}

function values(id, amounts) {
  return {
    node_id: id,
    node_type: 'value',
    values: Object.fromEntries(amounts.map((amount, i) => [`2025Q${i + 1}`, amount])),
  };
}

function formula(id, expression) {
  return { node_id: id, node_type: 'calculated', formula_text: expression };
}

test('monetary quantile preserves USD through the published facade', () => {
  const input = model({
    cash: values(
      'cash',
      [100, 300, 200].map((amount) => ({ amount: String(amount), currency: 'USD' }))
    ),
    lower: formula('lower', 'quantile(cash, 0.25)'),
    combined: formula('combined', 'lower + cash'),
  });
  const result = statements.evaluateModel(JSON.stringify(input));
  assert.equal(result.nodes.lower['2025Q3'], 150);
  assert.equal(result.nodes.combined['2025Q3'], 350);
  assert.equal(result.node_value_types.lower.currency, 'USD');
});

test('coalesce skips infinite values and ranks honor direction', () => {
  const input = model({
    observation: values('observation', [20, 30, 10]),
    fallback: formula('fallback', 'coalesce(exp(1000), -exp(1000), 7)'),
    ascending: formula('ascending', 'rank(observation, 1)'),
    descending: formula('descending', 'rank(observation, 0)'),
  });
  const result = statements.evaluateModel(JSON.stringify(input));
  assert.equal(result.nodes.fallback['2025Q3'], 7);
  assert.equal(result.nodes.ascending['2025Q3'], 1);
  assert.equal(result.nodes.descending['2025Q3'], 3);
  input.nodes.ascending.formula_text = 'rank(observation, 1, 2)';
  assert.throws(() => statements.evaluateModel(JSON.stringify(input)), /1 or 2 arguments/);
});

test('accounting identities reject numerically balanced incompatible currencies', () => {
  const input = model({
    assets: values('assets', [{ amount: '100', currency: 'USD' }]),
    liabilities: values('liabilities', [{ amount: '60', currency: 'EUR' }]),
    equity: values('equity', [{ amount: '40', currency: 'EUR' }]),
  });
  input.periods = input.periods.slice(0, 1);
  const suite = {
    name: 'currency-check',
    builtin_checks: [
      {
        type: 'balance_sheet_articulation',
        assets_nodes: ['assets'],
        liabilities_nodes: ['liabilities'],
        equity_nodes: ['equity'],
      },
    ],
  };
  assert.throws(
    () => statements_analytics.runChecks(JSON.stringify(input), JSON.stringify(suite)),
    /incompatible|currency|USD.*EUR/i
  );
  const stale = statements.evaluateModel(JSON.stringify(input));
  for (const node of ['assets', 'liabilities', 'equity']) {
    stale.node_value_types[node] = { type: 'scalar' };
  }
  assert.throws(
    () =>
      statements_analytics.runChecks(
        JSON.stringify(input),
        JSON.stringify(suite),
        JSON.stringify(stale)
      ),
    /incompatible|units|currency/i
  );
});

test('suite validation and execution reject negative thresholds', () => {
  const suite = { name: 'invalid', config: { default_tolerance: -1 } };
  assert.throws(
    () => statements.validateCheckSuiteSpecJson(JSON.stringify(suite)),
    /default_tolerance/
  );
  assert.throws(
    () =>
      statements_analytics.runChecks(
        JSON.stringify(model({ x: values('x', [1]) })),
        JSON.stringify(suite)
      ),
    /default_tolerance/
  );
});

test('waterfall validation requires funded prepayment and sweep priorities', () => {
  const waterfall = {
    priority_of_payments: ['fees', 'interest', 'amortization', 'equity'],
    available_cash_node: 'cash',
    mandatory_prepay_node: 'prepay',
  };
  assert.throws(
    () => statements.validateWaterfallSpecJson(JSON.stringify(waterfall)),
    /MandatoryPrepayment|mandatory_prepay/i
  );
  delete waterfall.mandatory_prepay_node;
  waterfall.ecf_sweep = { sweep_percentage: 0.5, ebitda_node: 'cash' };
  assert.throws(
    () => statements.validateWaterfallSpecJson(JSON.stringify(waterfall)),
    /Sweep|sweep/i
  );
});

for (const [name, side, fixedRate, forwardRate] of [
  ['receive with negative floating', 'receive', '0.01', -0.01],
  ['receive with zero fixed', 'receive', '0', -0.01],
  ['pay with negative fixed', 'pay', '-0.01', 0.01],
]) {
  test(`swap receipts: ${name}`, () => {
    const fixture = JSON.parse(
      readFileSync(new URL('./statements_negative_swap.fixture.json', import.meta.url), 'utf8')
    );
    const swap = fixture.model.capital_structure.debt_instruments[0].spec.spec;
    swap.side = side;
    swap.fixed_leg.rate = fixedRate;
    const forward = fixture.market.curves.find((curve) => curve.type === 'forward');
    forward.knot_points = forward.knot_points.map(([time]) => [time, forwardRate]);
    const result = statements.evaluateModelWithMarket(
      JSON.stringify(fixture.model),
      JSON.stringify(fixture.market),
      '2025-01-01'
    );
    const flow = result.cs_cashflows.by_instrument.IRS['2025'];
    const income = Number(flow.interest_income_cash.amount);
    assert.equal(Number(flow.interest_expense_cash.amount), 0);
    for (const key of ['principal_payment', 'debt_balance', 'accrued_interest']) {
      assert.equal(Number(flow[key].amount), 0);
    }
    assert.ok(income > 0);
    assert.equal(result.nodes.income['2025'], income);
  });
}

test('model validation rejects PIK capitalization of a swap payment', () => {
  const fixture = JSON.parse(
    readFileSync(new URL('./statements_negative_swap.fixture.json', import.meta.url), 'utf8')
  );
  fixture.model.capital_structure.waterfall = {
    priority_of_payments: ['fees', 'interest', 'amortization', 'equity'],
    available_cash_node: 'cash',
    pik_toggle: { liquidity_metric: 'cash', threshold: 20, target_instrument_ids: ['IRS'] },
  };
  assert.throws(
    () => statements.validateFinancialModelJson(JSON.stringify(fixture.model)),
    /only borrowing debt coupons/
  );
});
