/**
 * Facade tests for the statements / statements_analytics computation results.
 *
 * These entry points return structured JavaScript objects (not JSON strings
 * and not ES2015 `Map`s), matching the typed results the Python bindings
 * return from the same Rust code.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = join(__dirname, '..', '..', 'pkg');
const WASM_BG = join(PKG_DIR, 'finstack_quant_wasm_bg.wasm');

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

const facade = await import('../../index.js');
const init = facade.default;
const { statements, statements_analytics } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

const MODEL_JSON = JSON.stringify({
  id: 'facade-model',
  periods: [{ id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: false }],
  nodes: {
    revenue: {
      node_id: 'revenue',
      node_type: 'value',
      values: { '2025Q1': 100000.0 },
    },
  },
  schema_version: 1,
});

const SENSITIVITY_CONFIG_JSON = JSON.stringify({
  mode: 'diagonal',
  parameters: [
    {
      node_id: 'revenue',
      period_id: '2025Q1',
      base_value: 100000.0,
      perturbations: [90000.0, 100000.0, 110000.0],
    },
  ],
  target_metrics: ['revenue'],
});

function assertStructured(value, label) {
  assert.ok(!(value instanceof Map), `${label} must not be an ES2015 Map`);
  assert.equal(typeof value, 'object', `${label} must be an object, not a JSON string`);
  assert.notEqual(value, null, `${label} must not be null`);
  const serialized = JSON.stringify(value);
  assert.ok(serialized.length > 2, `${label} must round-trip through JSON: ${serialized}`);
  return JSON.parse(serialized);
}

test('statements.evaluateModel returns a structured StatementResult', () => {
  const result = statements.evaluateModel(MODEL_JSON);
  const roundTripped = assertStructured(result, 'evaluateModel result');
  // Property reads resolve directly off the returned object, not only after
  // a JSON round trip.
  assert.ok(result.nodes, 'nodes is directly readable');
  assert.ok(Object.keys(result.nodes).includes('revenue'), 'revenue node present');
  assert.ok(roundTripped.nodes.revenue, 'revenue survives serialization');
});

test('statements.runMonteCarlo returns a structured object', () => {
  const config = JSON.stringify({ n_paths: 10, seed: 42 });
  const results = statements.runMonteCarlo(MODEL_JSON, config);
  assertStructured(results, 'runMonteCarlo result');
});

test('statements_analytics.runSensitivity returns a structured object', () => {
  const result = statements_analytics.runSensitivity(MODEL_JSON, SENSITIVITY_CONFIG_JSON);
  assertStructured(result, 'runSensitivity result');

  const entries = statements_analytics.generateTornadoEntries(
    JSON.stringify(result),
    'revenue',
    '2025Q1'
  );
  assert.ok(Array.isArray(entries), 'generateTornadoEntries returns an array');
  assert.equal(entries[0].parameter_id, 'revenue');
  assert.equal(typeof entries[0].downside, 'number');
  assert.equal(typeof entries[0].upside, 'number');
});

test('statements_analytics.runChecks returns a structured CheckReport', () => {
  const spec = JSON.stringify({
    name: 'formula suite',
    builtin_checks: [],
    formula_checks: [
      {
        id: 'revenue_positive',
        name: 'Revenue must be positive',
        category: 'internal_consistency',
        severity: 'error',
        formula: 'revenue > 0',
        message_template: 'Revenue not positive in {period}',
        tolerance: null,
      },
    ],
  });
  const report = statements_analytics.runChecks(MODEL_JSON, spec);
  assertStructured(report, 'runChecks result');
  assert.equal(report.results[0].check_id, 'revenue_positive');
  assert.equal(report.summary.failed, 0);
});

test('statements_analytics.evaluateScenarioSet returns a structured object', () => {
  const scenarioSet = JSON.stringify({
    scenarios: { upside: { parent: null, overrides: { revenue: 200000.0 } } },
  });
  const results = statements_analytics.evaluateScenarioSet(MODEL_JSON, scenarioSet);
  assertStructured(results, 'evaluateScenarioSet result');
  assert.ok(results.upside, 'scenario key is directly readable');
});

test('statements_analytics.creditAssessment returns a structured object', () => {
  const evaluated = statements.evaluateModel(MODEL_JSON);
  const assessment = statements_analytics.creditAssessment(JSON.stringify(evaluated), '2025Q1');
  assertStructured(assessment, 'creditAssessment result');
  assert.equal(assessment.period, '2025Q1', 'assessment period is directly readable');
});

test('statement missing values survive JSON.stringify with canonical sentinels', () => {
  const model = JSON.parse(MODEL_JSON);
  model.nodes.lagged = {
    node_id: 'lagged',
    node_type: 'calculated',
    formula_text: 'lag(revenue, 1)',
  };
  const result = statements.evaluateModel(JSON.stringify(model));
  assert.equal(result.nodes.lagged['2025Q1'], 'nan');
  assert.equal(JSON.parse(JSON.stringify(result)).nodes.lagged['2025Q1'], 'nan');
});

test('statement declarations cannot relabel explicit foreign money', () => {
  const model = JSON.parse(MODEL_JSON);
  model.nodes.revenue.values['2025Q1'] = { amount: '100', currency: 'EUR' };
  model.nodes.revenue.value_type = { type: 'monetary', currency: 'USD' };
  assert.throws(() => statements.evaluateModel(JSON.stringify(model)), /declares.*explicit values/);
});
