/**
 * Statements and statements-analytics behaviour that Rust owns for both
 * hosts: model ingest and validation, result shapes (`GoalSeekResult`,
 * `DependencyTree`, `LboResult.checks`), Rust config types as inputs
 * (`DcfOptions`, `LboConfig`), non-finite sentinels and strict mappings.
 *
 * `finstack-quant-py/tests/test_statements_rust_owned.py` asserts the same
 * cases against the Python bindings with the same inputs and numbers.
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { scenarios, statements, statements_analytics as sa } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const usd = (amount) => ({ amount: String(amount), currency: 'USD' });

function quarter(q, year = 2025) {
  const startMonth = (q - 1) * 3 + 1;
  const endYear = q === 4 ? year + 1 : year;
  const endMonth = q === 4 ? 1 : startMonth + 3;
  const pad = (m) => String(m).padStart(2, '0');
  return {
    id: `${year}Q${q}`,
    start: `${year}-${pad(startMonth)}-01`,
    end: `${endYear}-${pad(endMonth)}-01`,
    is_actual: false,
  };
}

/** One-quarter model: profit = revenue * 0.5, revenue = 100. */
function goalSeekModel() {
  return {
    id: 'goal-seek',
    schema_version: 1,
    periods: [quarter(1)],
    nodes: {
      revenue: { node_id: 'revenue', node_type: 'value', values: { '2025Q1': 100 } },
      profit: { node_id: 'profit', node_type: 'calculated', formula_text: 'revenue * 0.5' },
    },
  };
}

/** The Python `evaluate_lbo` doctest model, with monetary USD values. */
function lboModel() {
  const ids = ['2025Q1', '2025Q2', '2025Q3', '2025Q4', '2026Q1'];
  const series = (values) => Object.fromEntries(ids.map((id, i) => [id, usd(values[i])]));
  return {
    id: 'lbo',
    schema_version: 1,
    meta: { currency: 'USD' },
    periods: [quarter(1), quarter(2), quarter(3), quarter(4), quarter(1, 2026)],
    nodes: {
      ebitda: { node_id: 'ebitda', node_type: 'value', values: series([100, 100, 100, 100, 120]) },
      net_debt: {
        node_id: 'net_debt',
        node_type: 'value',
        values: series([300, 300, 300, 300, 200]),
      },
    },
  };
}

const LBO_CONFIG = {
  entry_multiple: 8.0,
  entry_metric_node: 'ebitda',
  transaction_fees: 0.0,
  sources: [{ name: 'debt', amount: 500.0 }],
  exit_multiple: 9.0,
  exit_metric_node: 'ebitda',
  exit_net_debt_node: 'net_debt',
  exit_period: '2026Q1',
};

const LBO_CHECK_MAPPINGS = {
  three_statement: {
    assets_nodes: ['ebitda'],
    liabilities_nodes: ['net_debt'],
    equity_nodes: ['ebitda'],
    cash_node: 'net_debt',
    retained_earnings_node: 'ebitda',
    net_income_node: 'ebitda',
  },
  credit: { debt_node: 'net_debt', ebitda_node: 'ebitda', interest_expense_node: 'ebitda' },
};

/** Two annual forecast periods with USD ufcf and ebitda flows. */
function dcfModel() {
  const annual = (year) => ({
    id: String(year),
    start: `${year}-01-01`,
    end: `${year + 1}-01-01`,
    is_actual: false,
  });
  return {
    id: 'dcf',
    schema_version: 1,
    meta: { currency: 'USD' },
    periods: [annual(2025), annual(2026)],
    nodes: {
      ufcf: { node_id: 'ufcf', node_type: 'value', values: { 2025: usd(100), 2026: usd(110) } },
      ebitda: { node_id: 'ebitda', node_type: 'value', values: { 2025: usd(50), 2026: usd(60) } },
    },
  };
}

const EXIT_MULTIPLE = { type: 'exit_multiple', terminal_metric: 999.0, multiple: 8.0 };

test('goalSeek returns the Rust GoalSeekResult with the model as an object', () => {
  const updated = sa.goalSeek(goalSeekModel(), 'profit', '2025Q1', 60, 'revenue', '2025Q1', true);
  assert.deepEqual(Object.keys(updated).sort(), ['model', 'solved_value']);
  assert.ok(Math.abs(updated.solved_value - 120) < 1e-6);
  assert.equal(typeof updated.model, 'object');
  assert.ok(Math.abs(updated.model.nodes.revenue.values['2025Q1'] - 120) < 1e-6);
  // The updated model chains straight back into another call.
  assert.ok(Math.abs(statements.evaluateModel(updated.model).nodes.profit['2025Q1'] - 60) < 1e-6);

  const bare = sa.goalSeek(
    goalSeekModel(),
    'profit',
    '2025Q1',
    60,
    'revenue',
    '2025Q1',
    false,
    [1, 200]
  );
  assert.equal(bare.model, null);
  assert.ok(Math.abs(bare.solved_value - 120) < 1e-6);
});

test('goalSeek requires updateModel and a two-number bounds array', () => {
  assert.throws(
    () => sa.goalSeek(goalSeekModel(), 'profit', '2025Q1', 60, 'revenue', '2025Q1'),
    (e) => e instanceof TypeError && e.kind === 'invalid_type'
  );
  assert.throws(
    () => sa.goalSeek(goalSeekModel(), 'profit', '2025Q1', 60, 'revenue', '2025Q1', true, [1]),
    (e) => e instanceof Error && e.kind === 'validation'
  );
});

test('evaluateLbo takes the Rust LboConfig and runs its check suite', () => {
  const plain = sa.evaluateLbo(lboModel(), LBO_CONFIG);
  // MOIC = (9 x 120 - 200) / (8 x 100 - 500) = 880 / 300.
  assert.ok(Math.abs(plain.moic - 880 / 300) < 1e-12);
  assert.equal(plain.checks, null);

  const checked = sa.evaluateLbo(lboModel(), { ...LBO_CONFIG, check_mappings: LBO_CHECK_MAPPINGS });
  assert.ok(Math.abs(checked.moic - plain.moic) < 1e-12);
  assert.ok(checked.checks.results.length > 0);
  assert.equal(typeof checked.checks.summary.total_checks, 'number');

  assert.throws(
    () => sa.evaluateLbo(lboModel(), { ...LBO_CONFIG, transaction_fee: 1.0 }),
    (e) => e.kind === 'validation' && /transaction_fee/.test(e.message)
  );
  const missingFees = { ...LBO_CONFIG };
  delete missingFees.transaction_fees;
  assert.throws(
    () => sa.evaluateLbo(lboModel(), missingFees),
    (e) => e.kind === 'validation' && /transaction_fees/.test(e.message)
  );
});

test('dcfSensitivity takes the Rust DcfOptions, including exit_multiple_metric_node', () => {
  const explicitMetric = sa.dcfSensitivity(dcfModel(), 0.1, EXIT_MULTIPLE, 'ufcf', 0.0);
  const byNode = sa.dcfSensitivity(dcfModel(), 0.1, EXIT_MULTIPLE, 'ufcf', 0.0, {
    exit_multiple_metric_node: 'ebitda',
  });
  assert.notEqual(
    byNode.baseline_enterprise_value.amount,
    explicitMetric.baseline_enterprise_value.amount
  );
  // Omitted ufcfNode is the Rust DEFAULT_UFCF_NODE ("ufcf").
  const defaulted = sa.dcfSensitivity(dcfModel(), 0.1, EXIT_MULTIPLE, null, 0.0, {
    exit_multiple_metric_node: 'ebitda',
  });
  assert.deepEqual(defaulted, byNode);
  // A relative exit-multiple bump is reachable through the options document.
  const relative = sa.dcfSensitivity(dcfModel(), 0.1, EXIT_MULTIPLE, 'ufcf', 0.0, {
    exit_multiple_bump: { relative: 0.1 },
  });
  assert.ok(relative.entries.length > 0);
  assert.throws(
    () => sa.dcfSensitivity(dcfModel(), 0.1, EXIT_MULTIPLE, 'ufcf', 0.0, { exit_multiple: 1 }),
    (e) => e.kind === 'validation' && /exit_multiple/.test(e.message)
  );
});

const EMPTY_MARKET = {
  collateral: {},
  credit_indices: [],
  curves: [],
  dividends: [],
  fx: null,
  fx_delta_vol_surfaces: [],
  hierarchy: { roots: {} },
  inflation_indices: [],
  prices: {},
  series: [],
  surfaces: [],
  schema_version: 1,
  vol_cubes: [],
};

test('every model entry point applies the Rust semantic validation', () => {
  const empty = { id: 'empty', schema_version: 1, periods: [], nodes: {} };
  const spec = scenarios.buildScenarioSpec('noop', []);
  for (const call of [
    () => statements.validateFinancialModelJson(empty),
    () => statements.modelNodeIds(empty),
    () => sa.dependencyTree(empty, 'x'),
    () => scenarios.applyScenario(spec, EMPTY_MARKET, empty, '2025-01-01'),
  ]) {
    assert.throws(call, (e) => e.kind === 'validation' && /at least one period/.test(e.message));
  }
});

test('capital-structure validators run the Rust validate()', () => {
  const invertedWaterfall = {
    priority_of_payments: ['fees', 'interest', 'amortization', 'equity', 'sweep'],
    available_cash_node: 'cash',
  };
  assert.throws(
    () => statements.validateCapitalStructureSpecJson({ waterfall: invertedWaterfall }),
    (e) => e.kind === 'validation' && /last entry/.test(e.message)
  );
  assert.throws(
    () => statements.validateEcfSweepSpecJson({ ebitda_node: 'ebitda', sweep_percentage: 5.0 }),
    (e) => e.kind === 'validation' && /sweep_percentage/.test(e.message)
  );
  assert.throws(
    () =>
      statements.validatePikToggleSpecJson({
        liquidity_metric: 'cash',
        threshold: 1.0,
        target_instrument_ids: [],
      }),
    (e) => e.kind === 'validation' && /target_instrument_ids/.test(e.message)
  );
  const ok = JSON.parse(
    statements.validateEcfSweepSpecJson({ ebitda_node: 'ebitda', sweep_percentage: 0.5 })
  );
  assert.equal(ok.sweep_percentage, 0.5);
});

test('computeMultiple treats a null metric as missing', () => {
  assert.equal(
    sa.computeMultiple({ enterprise_value: 8500, ebitda: 1000, revenue: null }, 'ev_ebitda'),
    8.5
  );
  assert.equal(
    sa.computeMultiple({ enterprise_value: 8500, ebitda: null }, 'ev_ebitda'),
    undefined
  );
});

test('non-finite forecast and explanation values are JavaScript numbers', () => {
  const metrics = sa.backtestForecast([0.0, 0.0], [1.0, 2.0]);
  assert.ok(Number.isNaN(metrics.mape));
  assert.equal(metrics.mae, 1.5);

  const model = {
    id: 'lag',
    schema_version: 1,
    periods: [quarter(1), quarter(2)],
    nodes: {
      revenue: { node_id: 'revenue', node_type: 'value', values: { '2025Q1': 100, '2025Q2': 110 } },
      lagged: { node_id: 'lagged', node_type: 'calculated', formula_text: 'lag(revenue, 1)' },
    },
  };
  const results = statements.evaluateModel(model);
  const explanation = sa.explainFormula(model, JSON.stringify(results), 'lagged', '2025Q1');
  assert.ok(Number.isNaN(explanation.final_value));
  // The statement result itself keeps the JSON sentinel for its nested node map.
  assert.equal(results.nodes.lagged['2025Q1'], 'nan');
});

test('mappings and variance config reject unknown keys', () => {
  const model = goalSeekModel();
  assert.throws(
    () =>
      sa.runCreditUnderwritingChecks(model, {
        debt_node: 'revenue',
        ebitda_node: 'revenue',
        interest_expense_node: 'revenue',
        fcf_nodes: 'revenue',
      }),
    (e) => e.kind === 'validation' && /fcf_nodes/.test(e.message)
  );
  const results = JSON.stringify(statements.evaluateModel(model));
  assert.throws(
    () =>
      sa.runVariance(results, results, {
        baseline_label: 'a',
        comparison_label: 'b',
        metrics: ['revenue'],
        metric: ['profit'],
        periods: ['2025Q1'],
      }),
    (e) => e.kind === 'validation' && /metric/.test(e.message)
  );
});

test('dependencyTree returns the Rust DependencyTree and dependencyTreeText renders it', () => {
  const model = goalSeekModel();
  const tree = sa.dependencyTree(model, 'profit');
  assert.deepEqual(tree, {
    node_id: 'profit',
    formula: 'revenue * 0.5',
    children: [{ node_id: 'revenue', formula: null, children: [] }],
  });
  assert.equal(sa.dependencyTreeText(model, 'profit'), 'profit (revenue * 0.5)\n└── revenue\n');
  assert.equal(sa.traceDependencies, undefined);
});

test('evaluateScenarioSet returns the ScenarioResults map', () => {
  const out = sa.evaluateScenarioSet(goalSeekModel(), {
    scenarios: { base: {}, up: { overrides: { revenue: 200 } } },
  });
  assert.deepEqual(Object.keys(out), ['base', 'up']);
  assert.equal(out.up.nodes.profit['2025Q1'], 100);
});
