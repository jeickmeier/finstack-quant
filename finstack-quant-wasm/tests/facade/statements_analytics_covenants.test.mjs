/**
 * Covenant evaluation and breach forecasting over statement results
 * (statements_analytics.evaluateCovenants / forecastBreaches).
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import init, { covenants, statements, statements_analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const QUARTERS = ['2024Q1', '2024Q2', '2024Q3', '2024Q4'];
const PERIOD_STARTS = ['2024-01-01', '2024-04-01', '2024-07-01', '2024-10-01', '2025-01-01'];
const values = (series) =>
  Object.fromEntries(QUARTERS.map((quarter, index) => [quarter, series[index]]));
const node = (nodeId, extra) => ({ node_id: nodeId, ...extra });

// LTM leverage is NaN until four quarters of EBITDA exist (2024Q4).
const MODEL = JSON.stringify({
  id: 'borrower',
  periods: {
    calendar: 'gregorian',
    periods: QUARTERS.map((id, index) => ({
      id,
      start: PERIOD_STARTS[index],
      end: PERIOD_STARTS[index + 1],
      is_actual: false,
    })),
  },
  nodes: {
    ebitda: node('ebitda', { node_type: 'value', values: values([25, 25, 25, 25]) }),
    interest_expense: node('interest_expense', {
      node_type: 'value',
      values: values([5, 5, 5, 10]),
    }),
    total_debt: node('total_debt', { node_type: 'value', values: values([300, 300, 300, 300]) }),
    ltm_ebitda: node('ltm_ebitda', { node_type: 'calculated', formula_text: 'ttm(ebitda)' }),
    leverage: node('leverage', {
      node_type: 'calculated',
      formula_text: 'total_debt / ltm_ebitda',
    }),
    interest_coverage: node('interest_coverage', {
      node_type: 'calculated',
      formula_text: 'ebitda / interest_expense',
    }),
  },
  schema_version: 1,
});
const evaluator = new statements.Evaluator();
const RESULTS = evaluator.evaluate(MODEL);
evaluator.free();

const spec = (covenantType, label, metricId) => ({
  covenant: {
    covenant_type: covenantType,
    test_frequency: { count: 3, unit: 'months' },
    consequences: [],
    is_active: true,
    scope: 'maintenance',
    label,
  },
  metric_id: metricId,
});
const COVERAGE = spec(
  { min_interest_coverage: { threshold: 3.0 } },
  'min_interest_coverage',
  'interest_coverage'
);
const LEVERAGE = spec({ max_debt_to_ebitda: { threshold: 3.5 } }, 'max_debt_to_ebitda', 'leverage');

/** Coverage tested throughout; leverage added by a window from `start`. */
function withEngine(start, body) {
  const engine = covenants.CovenantEngine.fromJson(
    JSON.stringify({
      specs: [COVERAGE],
      windows: [{ start, end: '2024-12-31', covenants: [LEVERAGE, COVERAGE] }],
    })
  );
  try {
    return body(engine);
  } finally {
    engine.free();
  }
}

test('a covenant window keeps early non-finite leverage out of the evaluation', () => {
  const series = withEngine('2024-10-01', (engine) =>
    statements_analytics.evaluateCovenants(RESULTS, engine)
  );
  assert.deepEqual(
    series.map((row) => [row.as_of, Object.keys(row.reports)]),
    [
      ['2024-03-31', ['min_interest_coverage']],
      ['2024-06-30', ['min_interest_coverage']],
      ['2024-09-30', ['min_interest_coverage']],
      ['2024-12-31', ['max_debt_to_ebitda', 'min_interest_coverage']],
    ]
  );
  assert.equal(series[3].reports.max_debt_to_ebitda.actual_value, 3);
  assert.equal(series[3].reports.min_interest_coverage.passed, false);

  // The same rows through the dated-row engine series give the same reports.
  const rows = series.map((row) => ({
    date: row.as_of,
    metrics: Object.fromEntries(
      Object.values(row.reports).map((report) => [
        report.covenant_id === 'max_debt_to_ebitda' ? 'leverage' : 'interest_coverage',
        report.actual_value,
      ])
    ),
  }));
  const viaRows = withEngine('2024-10-01', (engine) => engine.evaluateSeries(rows));
  assert.deepEqual(viaRows, series);
});

test('a required non-finite or missing metric throws with the Rust error kind', () => {
  withEngine('2024-07-01', (engine) => {
    assert.throws(
      () => statements_analytics.evaluateCovenants(RESULTS, engine),
      (error) => error.kind === 'validation' && /leverage/.test(error.message)
    );
    assert.throws(
      () => statements_analytics.forecastBreaches(RESULTS, engine),
      (error) => error.kind === 'validation' && /leverage/.test(error.message)
    );
  });
  const dscr = covenants.CovenantEngine.fromJson(
    JSON.stringify({ specs: [spec({ min_dscr: { threshold: 1.2 } }, 'min_dscr', 'dscr')] })
  );
  try {
    assert.throws(
      () => statements_analytics.evaluateCovenants(RESULTS, dscr),
      (error) => error.kind === 'not_found' && /dscr/.test(error.message)
    );
  } finally {
    dscr.free();
  }
});

test('forecastBreaches reads every result period and defaults to a deterministic forecast', () => {
  const breaches = withEngine('2024-10-01', (engine) =>
    statements_analytics.forecastBreaches(RESULTS, engine)
  );
  assert.deepEqual(
    breaches.map((breach) => [breach.covenant_id, breach.breach_date, breach.breach_probability]),
    [['min_interest_coverage', '2024-12-31', 1]]
  );
  // It is a different export from covenants.forecastBreaches over dated rows.
  assert.notEqual(statements_analytics.forecastBreaches, covenants.forecastBreaches);
});
