import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { statements, statements_analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('canonical registry name and compiled trailing-period function', () => {
  const registry = statements.Registry.withBuiltins();
  try {
    assert.equal(registry.dependencies, undefined);
    assert.ok(registry.getDependencies('fin.gross_margin').includes('fin.gross_profit'));
  } finally {
    registry.free();
  }
  assert.doesNotThrow(() => statements.parseFormula('ltm(revenue)'));
  assert.throws(
    () => statements.parseAndCompile('ltm(revenue)'),
    /Function 'ltm' is not supported/
  );
  assert.equal(statements.parseAndCompile('ttm(revenue)'), undefined);
});

/** One-shot `Evaluator.evaluate`, the branch twin of the retired `evaluateModel`. */
function evaluateModel(input) {
  const evaluator = new statements.Evaluator();
  try {
    return evaluator.evaluate(input);
  } finally {
    evaluator.free();
  }
}

/** One-shot `Evaluator.evaluateWithMarket`, the twin of the retired `evaluateModelWithMarket`. */
function evaluateModelWithMarket(input, market, asOf) {
  const evaluator = new statements.Evaluator();
  try {
    return evaluator.evaluateWithMarket(input, market, asOf);
  } finally {
    evaluator.free();
  }
}

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
  const result = evaluateModel(JSON.stringify(input));
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
  const result = evaluateModel(JSON.stringify(input));
  assert.equal(result.nodes.fallback['2025Q3'], 7);
  assert.equal(result.nodes.ascending['2025Q3'], 1);
  assert.equal(result.nodes.descending['2025Q3'], 3);
  input.nodes.ascending.formula_text = 'rank(observation, 1, 2)';
  assert.throws(() => evaluateModel(JSON.stringify(input)), /1 or 2 arguments/);
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
  const stale = evaluateModel(JSON.stringify(input));
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
    const result = evaluateModelWithMarket(
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

// Twin of `test_check_config_rejects_invalid_thresholds` and
// `test_check_config_json_validates_thresholds` in
// finstack-quant-py/tests/test_statements_senior_review.py: the same Rust
// `CheckConfig::validate` names the offending field in both hosts.
test('checkConfigValidate is the Rust CheckConfig::validate', () => {
  assert.equal(statements.checkConfigValidate({}), undefined);
  assert.equal(statements.checkConfigValidate('{"default_tolerance":0.01}'), undefined);
  for (const field of [
    'default_tolerance',
    'default_relative_tolerance',
    'materiality_threshold',
  ]) {
    for (const value of [-1.0, -0.01]) {
      assert.throws(
        () => statements.checkConfigValidate({ [field]: value }),
        (error) => error.kind === 'validation' && error.message.includes(field)
      );
    }
    // A non-finite number is not JSON: the boundary rejects it before Rust.
    assert.throws(
      () => statements.checkConfigValidate({ [field]: Number.NaN }),
      (error) => error instanceof TypeError
    );
  }
  assert.throws(
    () => statements.checkConfigValidate({ tolerance: 0.01 }),
    (error) => error.kind === 'validation'
  );
  assert.throws(() => statements.checkConfigValidate(1), TypeError);
});

test('empty explicit periods preserve the model builder for retry', () => {
  const builder = new statements.ModelBuilder('recoverable');
  try {
    builder.valueScalar('revenue', { '2025Q1': 100 });
    builder.withMeta('marker', { retained: true });
    assert.throws(() => builder.periodsExplicit([]), /at least one period/);
    builder.periods('2025Q1..Q2', '2025Q1');
    builder.forecast('revenue', { method: 'forward_fill', params: {} });
    const model = builder.build();
    assert.equal(model.nodes.revenue.values['2025Q1'], 100);
    assert.deepEqual(model.meta.marker, { retained: true });
  } finally {
    builder.free();
  }
});

test('failed mixed formula preserves values, forecast, name and parent', () => {
  const parent = new statements.ModelBuilder('mixed-recovery');
  let mixed;
  let ready;
  try {
    parent.periods('2025Q1..Q2', '2025Q1');
    parent.valueScalar('base', { '2025Q1': 100 });
    parent.withMeta('marker', 'retained');
    mixed = parent.mixed('mixed');
    mixed.values({ '2025Q1': 40 });
    mixed.forecast({ method: 'forward_fill', params: {} });
    mixed.name('Retained name');
    assert.throws(() => mixed.formula('('));
    mixed.formula('base * 0.5');
    ready = mixed.build();
    const model = ready.build();
    assert.equal(model.nodes.mixed.values['2025Q1'], 40);
    assert.equal(model.nodes.mixed.name, 'Retained name');
    assert.equal(model.nodes.mixed.formula_text, 'base * 0.5');
    assert.equal(model.meta.marker, 'retained');
    assert.ok(model.nodes.base);
  } finally {
    ready?.free();
    mixed?.free();
    parent.free();
  }
});

test('accounting overflow fails without publishing invalid numbers', () => {
  const input = model({
    a1: values('a1', [1e308, 40, 40]),
    a2: values('a2', [1e308, 60, 60]),
    l: values('l', [1e308, 70, 70]),
    e: values('e', [1e308, 30, 30]),
  });
  const suite = {
    name: 'overflow',
    config: { materiality_threshold: 1e300 },
    builtin_checks: [
      {
        type: 'balance_sheet_articulation',
        assets_nodes: ['a1', 'a2'],
        liabilities_nodes: ['l'],
        equity_nodes: ['e'],
      },
    ],
  };
  const report = statements_analytics.runChecks(input, suite);
  assert.equal(report.summary.failed, 1);
  const result = report.results[0];
  assert.equal(result.passed, false);
  assert.equal(result.comparisons.length, 2);
  assert.equal(result.findings[0].severity, 'error');
  assert.equal(result.findings[0].comparison ?? null, null);
  assert.equal(result.findings[0].materiality ?? null, null);
});
