/**
 * `error.kind` is the Rust error kind, identical to the Python exception class.
 *
 * Every Rust error exposes `kind()` (`NotFound`, `Validation`, `Computation`);
 * the facade reports it as `not_found`, `validation` or `computation`, and the
 * Python bindings raise `KeyError`, `ValueError` or `RuntimeError` for the same
 * failures (`finstack-quant-py/tests/test_error_kinds.py`). `invalid_type` is
 * the one host-only kind: a JavaScript value of the wrong type.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, {
  analytics,
  core,
  models,
  portfolio,
  statements,
  statements_analytics,
} from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const kind = (expected, pattern) => (error) => {
  assert.ok(error instanceof Error, `expected an Error, got ${typeof error}: ${error}`);
  assert.equal(error.kind, expected, `kind of: ${error.message}`);
  if (pattern) assert.match(error.message, pattern);
  return true;
};

function model(formulas) {
  const nodes = {
    revenue: { node_id: 'revenue', node_type: 'value', values: { '2024Q1': 1.0, '2024Q2': 1.0 } },
  };
  for (const [node, formula] of formulas) {
    nodes[node] = { node_id: node, node_type: 'calculated', formula_text: formula };
  }
  return {
    id: 'kinds',
    periods: [
      { id: '2024Q1', start: '2024-01-01', end: '2024-04-01', is_actual: false },
      { id: '2024Q2', start: '2024-04-01', end: '2024-07-01', is_actual: false },
    ],
    nodes,
    schema_version: 1,
  };
}

function creditModel() {
  const calibrator = new models.factor.credit.CreditCalibrator({
    policy: 'globally_off',
    hierarchy: { levels: [] },
    min_bucket_size_per_level: { per_level: [] },
    vol_model: 'sample',
    covariance_strategy: 'diagonal',
    beta_shrinkage: 'none',
    use_returns_or_levels: 'returns',
    panel_frequency: 'monthly',
    bucket_weighting: 'equal',
  });
  return calibrator.calibrate({
    history_panel: { dates: ['2024-01-01', '2024-02-01'], spreads: { A: [0.01, 0.0101] } },
    issuer_tags: { tags: { A: {} } },
    generic_factor: { spec: { name: 'G', series_id: 'G' }, values: [0.01, 0.0101] },
    as_of: '2024-02-01',
    as_of_spreads: { A: 0.0101 },
    idiosyncratic_overrides: {},
  });
}

function performance() {
  const dates = Array.from({ length: 10 }, (_, i) => `2024-01-${String(i + 1).padStart(2, '0')}`);
  const returns = [0.01, -0.01, 0.02, 0.0, 0.01, -0.02, 0.01, 0.0, 0.01, 0.01];
  return analytics.Performance.fromReturns(dates, [returns], ['A']);
}

test('a statements dependency cycle is a computation failure', () => {
  assert.throws(
    () =>
      statements.evaluateModel(
        model([
          ['a', 'b + 1'],
          ['b', 'a + 1'],
        ])
      ),
    kind('computation', /circular/i)
  );
});

test('an unknown statements identifier is a validation failure', () => {
  assert.throws(
    () => statements.evaluateModel(model([['x', 'revenue + nope']])),
    kind('validation', /nope/)
  );
});

test('explaining a missing period is not found', () => {
  const spec = model([['x', 'revenue * 2']]);
  const result = statements.evaluateModel(spec);
  assert.throws(
    () => statements_analytics.explainFormula(spec, result, 'x', '2030Q1'),
    kind('not_found')
  );
});

test('an unknown portfolio entity is not found', () => {
  const spec = {
    id: 'P',
    as_of: '2025-01-15',
    base_currency: 'USD',
    entities: { FUND: { id: 'FUND' } },
    positions: [
      {
        position_id: 'X',
        entity_id: 'NOPE',
        instrument_id: 'D',
        instrument_spec: {
          type: 'deposit',
          spec: {
            id: 'D',
            notional: { amount: '1', currency: 'USD' },
            start_date: '2025-01-15',
            maturity: '2025-07-15',
            day_count: 'act_360',
            fixed_rate: '0.04',
            discount_curve_id: 'USD-OIS',
            attributes: {},
          },
        },
        quantity: 1.0,
        unit: 'units',
      },
    ],
  };
  assert.throws(() => portfolio.Portfolio.fromSpec(spec), kind('not_found', /NOPE/));
});

test('an issuer outside the factor model is not found', () => {
  assert.throws(
    () => models.factor.credit.decomposeLevels(creditModel(), { ZZZ: 0.0125 }, 0.012, '2025-06-30'),
    kind('not_found', /ZZZ/)
  );
});

test('nearest correlation that does not converge is a computation failure', () => {
  const matrix = [1.0, 0.95, -0.95, 0.95, 1.0, 0.95, -0.95, 0.95, 1.0];
  assert.throws(
    () => models.correlation.nearestCorrelation(matrix, 3, 1, 1e-16),
    kind('computation')
  );
});

test('COS separates invalid inputs from a degenerate range', () => {
  assert.throws(
    () => models.bsCosPrice(100, 100, 0.05, 0, 0.2, 1, true, 0),
    kind('validation', /num_terms/)
  );
  assert.throws(() => models.bsCosPrice(100, 100, 0.05, 0, -0.2, 1, true), kind('validation'));
  assert.throws(() => models.bsCosPrice(100, 100, 0.05, 0, 0.2, 0, true), kind('computation'));
});

test('inverting a zero FX rate is a validation failure', () => {
  assert.throws(() => core.invertFxRate(0), kind('validation'));
});

test('an unknown calendar is not found and a calendar union resolves', () => {
  const perf = performance();
  assert.throws(() => perf.cagr('bus_252', 'nope_cal'), kind('not_found', /nope_cal/));
  assert.ok(perf.cagr('bus_252', 'nyse+gblo')[0] > 0);
});

test('a JavaScript value of the wrong type is invalid_type', () => {
  assert.throws(
    () => portfolio.Portfolio.fromMaterialization(42),
    (error) => {
      assert.ok(error instanceof TypeError);
      return kind('invalid_type')(error);
    }
  );
});
