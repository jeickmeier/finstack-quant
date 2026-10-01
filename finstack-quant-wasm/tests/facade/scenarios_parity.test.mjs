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

for (const [index, { py, args, expected }] of GOLDEN.constructors.entries()) {
  const name = `operationSpec${pascal(py)}`;
  test(`${name} matches Python OperationSpec.${py} (case ${index})`, () => {
    const built = scenarios[name](...args);
    assert.deepEqual(built, expected);
    for (const operation of [built].flat()) {
      assert.equal(scenarios.operationSpecValidate(operation), undefined);
    }
  });
}

test('every Python OperationSpec constructor has a golden case', () => {
  const covered = new Set(GOLDEN.constructors.map(({ py }) => py));
  const constructors = Object.keys(scenarios).filter(
    (key) =>
      key.startsWith('operationSpec') &&
      ![
        'operationSpecValidate',
        'operationSpecRequiresInstruments',
        'operationSpecMutatesInstruments',
      ].includes(key)
  );
  assert.equal(constructors.length, 24);
  assert.deepEqual(
    constructors.filter((key) => ![...covered].some((py) => key === `operationSpec${pascal(py)}`)),
    []
  );
});

test('operation constructors accept structured targets and bindings as JSON text', () => {
  const [{ args, expected }] = GOLDEN.constructors.filter(
    ({ py }) => py === 'hierarchy_equity_price_pct'
  );
  assert.deepEqual(
    scenarios.operationSpecHierarchyEquityPricePct(JSON.stringify(args[0]), args[1]),
    expected
  );
  const [binding] = GOLDEN.constructors.filter(({ py }) => py === 'rate_binding');
  assert.deepEqual(
    scenarios.operationSpecRateBinding(JSON.stringify(binding.args[0])),
    binding.expected
  );
});

test('operation constructors reject wrong types and unknown labels', () => {
  assert.throws(() => scenarios.operationSpecMarketFxPct('EUR', 'USD', '5'), TypeError);
  assert.throws(
    () => scenarios.operationSpecMarketFxPct('EUR', 'ZZZ', 5),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () => scenarios.operationSpecCurveParallelBp('bogus', 'USD-OIS', 1),
    (error) => error.kind === 'validation' && /bogus/.test(error.message)
  );
  assert.throws(() => scenarios.operationSpecCurveParallelBp('discount', 5, 1), TypeError);
  assert.throws(
    () => scenarios.operationSpecCurveNodeBp('discount', 'USD-OIS', [['2Y', 1]], 'nearest'),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () => scenarios.operationSpecInstrumentPricePctByType(['not_an_instrument'], 1),
    (error) => error.kind === 'validation' && /not_an_instrument/.test(error.message)
  );
  assert.throws(
    () => scenarios.operationSpecTimeRollForward('1M', true, 'weekly'),
    (error) => error.kind === 'validation'
  );
  assert.throws(() => scenarios.operationSpecTimeRollForward('1M', 'yes'), TypeError);
  assert.throws(
    () => scenarios.operationSpecHierarchyEquityPricePct({ paths: ['Credit'] }, 1),
    (error) => error.kind === 'validation'
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
  // the constructors and rate bindings consume.
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
    assert.equal(
      scenarios.operationSpecCurveParallelBp(curveKind, 'ACME', 1).curve_kind,
      curveKind
    );
  }
  for (const rollMode of Object.values(enums.TimeRollMode)) {
    assert.equal(scenarios.operationSpecTimeRollForward('1M', false, rollMode).roll_mode, rollMode);
  }
  for (const matchMode of Object.values(enums.TenorMatchMode)) {
    assert.equal(
      scenarios.operationSpecVolIndexNodePts('VIX', [['1M', 1]], matchMode).match_mode,
      matchMode
    );
  }
  for (const compounding of Object.values(enums.Compounding)) {
    const binding = { node_id: 'rate', curve_id: 'USD-OIS', tenor: '1Y', compounding };
    assert.equal(scenarios.rateBindingSpecValidate(binding), undefined);
    assert.deepEqual(scenarios.operationSpecRateBinding(binding).binding.compounding, compounding);
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

test('scenarioSpec predicates and withHazardBumpMode match Python', () => {
  const { cases, with_hazard_bump_mode: bump, bad_mode_error } = GOLDEN.scenario_spec;
  for (const { spec, requires_instruments, mutates_instruments } of cases) {
    assert.equal(scenarios.scenarioSpecRequiresInstruments(spec), requires_instruments, spec.id);
    assert.equal(scenarios.scenarioSpecMutatesInstruments(spec), mutates_instruments, spec.id);
    assert.equal(
      scenarios.scenarioSpecRequiresInstruments(JSON.stringify(spec)),
      requires_instruments
    );
  }
  const [{ spec }] = cases;
  // The default mode (`solve_to_par`) is omitted from the wire.
  assert.equal(spec.hazard_bump_mode, undefined);
  assert.deepEqual(scenarios.scenarioSpecWithHazardBumpMode(spec, bump.mode), bump.expected);
  // The input object is not mutated.
  assert.equal(spec.hazard_bump_mode, undefined);
  assert.equal(bad_mode_error.exception, 'ValueError');
  assert.throws(
    () => scenarios.scenarioSpecWithHazardBumpMode(spec, 'nope'),
    validation(bad_mode_error.message)
  );
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
