import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { statements, statements_analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function model(values, forecast) {
  return {
    id: 'statement-production',
    schema_version: 1,
    periods: [1, 2, 3, 4].map((q) => ({
      id: `2025Q${q}`,
      start: `2025-${String(q * 3 - 2).padStart(2, '0')}-01`,
      end: q === 4 ? '2026-01-01' : `2025-${String(q * 3 + 1).padStart(2, '0')}-01`,
      is_actual: q === 1,
    })),
    nodes: {
      revenue: {
        node_id: 'revenue',
        node_type: 'value',
        values,
        ...(forecast ? { forecast } : {}),
      },
    },
  };
}

for (const forecast of [
  { method: 'growth_pct', params: { rate: 0.1 } },
  {
    method: 'seasonal',
    params: {
      historical: [100, 60, 120, 80, 100, 60, 120, 80],
      season_length: 4,
      mode: 'additive',
    },
  },
]) {
  test(`${forecast.method} retains run anchoring and phase across leading explicit forecasts`, () => {
    const baseline = model({ '2025Q1': 80 }, forecast);
    const overridden = structuredClone(baseline);
    overridden.nodes.revenue.values['2025Q2'] = 999;
    const expected = statements.evaluateModel(JSON.stringify(baseline)).nodes.revenue;
    const actual = statements.evaluateModel(JSON.stringify(overridden)).nodes.revenue;
    assert.equal(actual['2025Q2'], 999);
    assert.equal(actual['2025Q3'], expected['2025Q3']);
    assert.equal(actual['2025Q4'], expected['2025Q4']);
  });
}

test('formula residual checks accept the tolerance boundary and reject larger residuals', () => {
  const m = model({ '2025Q1': 0, '2025Q2': 0.1, '2025Q3': -0.1, '2025Q4': 0.2 });
  const suite = {
    name: 'residual',
    builtin_checks: [],
    formula_checks: [
      {
        id: 'residual',
        name: 'Residual',
        category: 'accounting_identity',
        severity: 'error',
        formula: 'revenue',
        message_template: 'residual {period}',
        tolerance: 0.1,
      },
    ],
  };
  const report = statements_analytics.runChecks(JSON.stringify(m), JSON.stringify(suite));
  assert.deepEqual(
    report.results[0].findings.map((finding) => finding.period),
    ['2025Q4']
  );
  suite.formula_checks[0].formula = 'revenue <= 0.1';
  suite.formula_checks[0].tolerance = null;
  assert.deepEqual(
    statements_analytics
      .runChecks(JSON.stringify(m), JSON.stringify(suite))
      .results[0].findings.map((f) => f.period),
    ['2025Q4']
  );
});

test('retained earnings accepts the elected inflow-positive dividend convention', () => {
  const m = model({ '2025Q1': 100, '2025Q2': 100, '2025Q3': 100, '2025Q4': 100 });
  m.nodes.re = {
    node_id: 're',
    node_type: 'value',
    values: { '2025Q1': 1000, '2025Q2': 1080, '2025Q3': 1160, '2025Q4': 1240 },
  };
  m.nodes.div = {
    node_id: 'div',
    node_type: 'value',
    values: { '2025Q1': -20, '2025Q2': -20, '2025Q3': -20, '2025Q4': -20 },
  };
  const suite = {
    name: 'signed-dividends',
    formula_checks: [],
    builtin_checks: [
      {
        type: 'retained_earnings_reconciliation',
        retained_earnings_node: 're',
        net_income_node: 'revenue',
        dividends_node: 'div',
        dividends_sign_convention: 'inflow_positive',
      },
    ],
  };
  const report = statements_analytics.runChecks(JSON.stringify(m), JSON.stringify(suite));
  assert.equal(report.summary.failed, 0);
});

test('goal seek rejects a discontinuity that brackets the target without reaching it', () => {
  const m = model({ '2025Q1': 1, '2025Q2': 1, '2025Q3': 1, '2025Q4': 1 });
  m.nodes.target = {
    node_id: 'target',
    node_type: 'calculated',
    formula_text: 'if(revenue < 0, 0, 1)',
  };
  assert.throws(
    () =>
      statements_analytics.goalSeek(
        JSON.stringify(m),
        'target',
        '2025Q4',
        0.5,
        'revenue',
        '2025Q4',
        true,
        -1,
        1
      ),
    /residual|target.*tolerance/i
  );
});
