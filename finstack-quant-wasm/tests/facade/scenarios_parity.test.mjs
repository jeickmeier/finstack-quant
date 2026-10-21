/**
 * Facade tests for the scenarios free-function twins of the Python
 * `OperationSpec`, scenario enum, `RateBindingSpec`, `ScenarioSpec` and
 * `HorizonResult` members (parity slice P3).
 *
 * Expected values come from `finstack-quant-py/tests/data/scenarios_wasm_parity.json`,
 * which holds the Python outputs for the same inputs and is asserted by
 * `finstack-quant-py/tests/test_scenarios_wasm_parity.py`, so each case is a
 * cross-host golden: both hosts call the same Rust entry point.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { scenarios } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const GOLDEN = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/data/scenarios_wasm_parity.json', import.meta.url),
    'utf8'
  )
);
const camel = (name) => name.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
const pascal = (name) => camel(`_${name}`);
// Python reports "Validation error: <text>"; the WASM message carries <text>.
const core = (message) => message.replace(/^Validation error: /, '');
const validation = (text) => (error) =>
  error.kind === 'validation' && error.message.includes(core(text));

// Python `OperationSpec.<variant>(...)` constructors only build the wire
// object, so their WASM twin is that TypeScript literal (`literal` in the
// parity ledger). The golden `expected` value is the Python-built literal;
// WASM must accept it unchanged. `parallel_bp_many` and `time_roll_forward`
// do work in Rust and keep WASM functions.
const COMPUTING_CONSTRUCTORS = {
  parallel_bp_many: 'parallelBpMany',
  time_roll_forward: 'operationSpecTimeRollForward',
};

for (const [index, { py, args, expected }] of GOLDEN.constructors.entries()) {
  const name = COMPUTING_CONSTRUCTORS[py];
  if (name !== undefined) {
    test(`${name} matches Python ${py} (case ${index})`, () => {
      const built = scenarios[name](...args);
      assert.deepEqual(built, expected);
      for (const operation of [built].flat()) {
        assert.equal(scenarios.operationSpecValidate(operation), undefined);
      }
    });
    continue;
  }
  test(`Python OperationSpec.${py} literal is a valid WASM operation (case ${index})`, () => {
    assert.equal(scenarios[`operationSpec${pascal(py)}`], undefined);
    assert.equal(scenarios.operationSpecValidate(expected), undefined);
    assert.equal(scenarios.operationSpecValidate(JSON.stringify(expected)), undefined);
  });
}

test('only the computing OperationSpec members have WASM functions', () => {
  const functions = Object.keys(scenarios)
    .filter((key) => key.startsWith('operationSpec'))
    .sort();
  assert.deepEqual(functions, [
    'operationSpecMutatesInstruments',
    'operationSpecRequiresInstruments',
    'operationSpecTimeRollForward',
    'operationSpecValidate',
  ]);
  const covered = new Set(GOLDEN.constructors.map(({ py }) => py));
  assert.equal(covered.size, 25);
});

test('computing constructors reject wrong types and unknown labels', () => {
  assert.deepEqual(scenarios.parallelBpMany('discount', [], 1), []);
  assert.throws(
    () => scenarios.parallelBpMany('bogus', ['USD-OIS'], 1),
    (error) => error.kind === 'validation' && /bogus/.test(error.message)
  );
  assert.throws(() => scenarios.parallelBpMany('discount', 'USD-OIS', 1), TypeError);
  assert.throws(
    () => scenarios.operationSpecTimeRollForward('1M', true, 'weekly'),
    (error) => error.kind === 'validation'
  );
  assert.throws(() => scenarios.operationSpecTimeRollForward('1M', 'yes'), TypeError);
});

test('operationSpecValidate rejects malformed literals', () => {
  assert.throws(
    () =>
      scenarios.operationSpecValidate({
        kind: 'market_fx_pct',
        base: 'EUR',
        quote: 'ZZZ',
        pct: 5,
      }),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () =>
      scenarios.operationSpecValidate({
        kind: 'curve_node_bp',
        curve_kind: 'discount',
        curve_id: 'USD-OIS',
        nodes: [['2Y', 1]],
        match_mode: 'nearest',
      }),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () =>
      scenarios.operationSpecValidate({
        kind: 'instrument_price_pct_by_type',
        instrument_types: ['not_an_instrument'],
        pct: 1,
      }),
    (error) => error.kind === 'validation' && /not_an_instrument/.test(error.message)
  );
});

test('operationSpecRequiresInstruments / MutatesInstruments match Python', () => {
  for (const { expected, requires_instruments, mutates_instruments } of GOLDEN.constructors) {
    if (Array.isArray(expected)) continue;
    assert.equal(scenarios.operationSpecRequiresInstruments(expected), requires_instruments);
    assert.equal(scenarios.operationSpecMutatesInstruments(expected), mutates_instruments);
    assert.equal(
      scenarios.operationSpecMutatesInstruments(JSON.stringify(expected)),
      mutates_instruments
    );
  }
  assert.throws(
    () => scenarios.operationSpecRequiresInstruments({ kind: 'no_such_operation' }),
    (error) => error.kind === 'validation'
  );
});

test('operationSpecValidate rejects what Python OperationSpec.validate rejects', () => {
  for (const { operation, error } of GOLDEN.invalid_operations) {
    assert.equal(error.exception, 'ValueError');
    assert.throws(() => scenarios.operationSpecValidate(operation), validation(error.message));
  }
});

test('scenario enum literals are the Python wire values', () => {
  // Python `CurveKind.par_cds()` and friends have no WASM function: the
  // TypeScript literal of the generated type is the twin, and it is the value
  // operation literals and rate bindings carry.
  const enums = GOLDEN.enums;
  for (const [type, variants] of Object.entries(enums)) {
    const prefix = type[0].toLowerCase() + type.slice(1);
    for (const variant of Object.keys(variants)) {
      assert.equal(scenarios[`${prefix}${pascal(variant)}`], undefined, `${type}.${variant}`);
    }
  }
  assert.equal(enums.CurveKind.par_cds, 'par_cds');
  assert.deepEqual(enums.Compounding.quarterly, { periodic: 4 });
  for (const curveKind of Object.values(enums.CurveKind)) {
    const operation = { kind: 'curve_parallel_bp', curve_kind: curveKind, curve_id: 'ACME', bp: 1 };
    assert.equal(scenarios.operationSpecValidate(operation), undefined);
  }
  for (const rollMode of Object.values(enums.TimeRollMode)) {
    assert.equal(scenarios.operationSpecTimeRollForward('1M', false, rollMode).roll_mode, rollMode);
  }
  for (const matchMode of Object.values(enums.TenorMatchMode)) {
    const operation = {
      kind: 'vol_index_node_pts',
      curve_id: 'VIX',
      nodes: [['1M', 1]],
      match_mode: matchMode,
    };
    assert.equal(scenarios.operationSpecValidate(operation), undefined);
  }
  for (const compounding of Object.values(enums.Compounding)) {
    const binding = { node_id: 'rate', curve_id: 'USD-OIS', tenor: '1Y', compounding };
    assert.equal(scenarios.rateBindingSpecValidate(binding), undefined);
    assert.equal(scenarios.operationSpecValidate({ kind: 'rate_binding', binding }), undefined);
  }
});

test('rateBindingSpecValidate matches Python RateBindingSpec.validate', () => {
  const [{ args }] = GOLDEN.constructors.filter(({ py }) => py === 'rate_binding');
  assert.equal(scenarios.rateBindingSpecValidate(args[0]), undefined);
  assert.equal(scenarios.rateBindingSpecValidate(JSON.stringify(args[0])), undefined);
  for (const { binding, error } of GOLDEN.invalid_bindings) {
    assert.equal(error.exception, 'ValueError');
    assert.throws(() => scenarios.rateBindingSpecValidate(binding), validation(error.message));
  }
  assert.throws(
    () => scenarios.rateBindingSpecValidate({ node_id: 'n', curve_id: 'c', tenor: '1Y', extra: 1 }),
    (error) => error.kind === 'validation'
  );
});

test('scenarioSpec predicates match Python', () => {
  const { cases } = GOLDEN.scenario_spec;
  for (const { spec, requires_instruments, mutates_instruments } of cases) {
    assert.equal(scenarios.scenarioSpecRequiresInstruments(spec), requires_instruments, spec.id);
    assert.equal(scenarios.scenarioSpecMutatesInstruments(spec), mutates_instruments, spec.id);
    assert.equal(
      scenarios.scenarioSpecRequiresInstruments(JSON.stringify(spec)),
      requires_instruments
    );
  }
  assert.throws(
    () => scenarios.scenarioSpecRequiresInstruments({ id: '', operations: [] }),
    (error) => error.kind === 'validation' && /Scenario ID cannot be empty/.test(error.message)
  );
});

test('horizonResultExplainText and horizonResultFactorContribution match Python', () => {
  const { as_of, market, instrument, scenario, explain, factor_contribution, bad_factor_error } =
    GOLDEN.horizon;
  const report = scenarios.computeHorizonReturn(instrument, market, as_of, scenario);
  assert.equal(scenarios.horizonResultExplainText(report), explain);
  const { summary, ...result } = report;
  assert.equal(scenarios.horizonResultExplainText(result), explain);
  assert.equal(scenarios.horizonResultExplainText(JSON.stringify(result)), explain);
  for (const [factor, expected] of Object.entries(factor_contribution)) {
    const actual = scenarios.horizonResultFactorContribution(report, factor);
    assert.ok(Math.abs(actual - expected) < 1e-12, `${factor}: ${actual} vs ${expected}`);
    assert.equal(actual, summary.factor_contributions[factor]);
  }
  assert.equal(bad_factor_error.exception, 'ValueError');
  assert.throws(
    () => scenarios.horizonResultFactorContribution(report, 'nope'),
    (error) => error.kind === 'validation' && /nope/.test(error.message)
  );
  assert.throws(
    () => scenarios.horizonResultExplainText({ horizon_days: 3 }),
    (error) => error.kind === 'validation'
  );
});
