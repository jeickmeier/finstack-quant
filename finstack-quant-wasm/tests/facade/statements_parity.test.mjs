/**
 * Cross-host goldens and entry-point tests for the `statements` parity surface.
 *
 * The Python twin is `finstack-quant-py/tests/test_statements_wasm_parity.py`:
 * both hosts read the same inputs and assert the same pinned outputs.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  EXPECTED,
  INPUTS,
  Q1,
  Q2,
  assertMatches,
  facade,
  money,
  orNull,
  plain,
} from './statements_parity_support.mjs';

const { statements } = facade;
const MODEL = INPUTS.model;

function evaluate(model) {
  const evaluator = new statements.Evaluator();
  try {
    return evaluator.evaluate(model);
  } finally {
    evaluator.free();
  }
}

function builtModel() {
  const builder = new statements.ModelBuilder('built');
  builder.periods('2025Q1..Q2', '2025Q1');
  builder.valueScalar('revenue', { [Q1]: 100.0 });
  builder.forecast('revenue', statements.forecastSpecGrowth(0.05));
  builder.compute('profit', 'revenue * 0.5');
  builder.valueMoney('cash', {
    [Q1]: { amount: '100', currency: 'USD' },
    [Q2]: { amount: '110', currency: 'USD' },
  });
  builder.withMeta('currency', 'USD');
  const mixed = builder.mixed('capex');
  mixed.values({ [Q1]: 10.0 });
  mixed.formula('revenue * 0.1');
  mixed.name('Capex');
  const ready = mixed.build();
  return ready.build();
}

test('statements matches the cross-host golden', () => {
  const result = evaluate(MODEL);
  const built = builtModel();
  const builtResult = evaluate(built);

  const config = INPUTS.normalization_config;
  const oneOff = statements.adjustmentFixed('one_off', 'One-off cost', { [Q1]: 5.0 });
  const synergy = statements.adjustmentPercentage('synergy', 'Synergies', 'revenue', 0.1);
  let extended = { target_node: 'profit', adjustments: [] };
  extended = statements.normalizationConfigAddAdjustment(extended, oneOff);
  extended = statements.normalizationConfigAddAdjustment(
    extended,
    statements.adjustmentWithCap(synergy, 'revenue', 0.05)
  );

  const evaluator = new statements.Evaluator();
  evaluator.withChecks(INPUTS.check_suite);
  const report = evaluator.evaluate(MODEL).check_report;
  evaluator.free();

  const cs = INPUTS.cs_cashflows;
  const registry = statements.Registry.withBuiltins();
  const first = registry.metricIds()[0];

  const actual = {
    'result.get': orNull(statements.statementResultGet(result, 'profit', Q2)),
    'result.get_missing': orNull(statements.statementResultGet(result, 'nope', Q2)),
    'result.get_or': statements.statementResultGetOr(result, 'nope', Q2, -1.0),
    'result.all_periods': statements.statementResultAllPeriods(result, 'profit'),
    'result.get_node': orNull(statements.statementResultGetNode(result, 'profit')),
    'result.get_node_missing': orNull(statements.statementResultGetNode(result, 'nope')),
    'result.node_ids': statements.statementResultNodeIds(result),
    'result.get_scalar': orNull(statements.statementResultGetScalar(result, 'margin', Q1)),
    'result.get_money': money(statements.statementResultGetMoney(builtResult, 'cash', Q2)),
    'result.get_money_of_scalar': money(
      statements.statementResultGetMoney(builtResult, 'revenue', Q2)
    ),
    'result.get_scalar_of_money': orNull(
      statements.statementResultGetScalar(builtResult, 'cash', Q2)
    ),
    'model.has_node': [
      statements.financialModelHasNode(MODEL, 'profit'),
      statements.financialModelHasNode(MODEL, 'nope'),
    ],
    'model.get_node': statements.financialModelGetNode(MODEL, 'profit'),
    'model.node_ids': statements.modelNodeIds(MODEL),
    'builder.node_ids': statements.modelNodeIds(built),
    'builder.content_hash': statements.financialModelContentHash(built),
    'builder.values': [
      statements.statementResultGet(builtResult, 'revenue', Q2),
      statements.statementResultGet(builtResult, 'profit', Q2),
      statements.statementResultGet(builtResult, 'capex', Q1),
      statements.statementResultGet(builtResult, 'capex', Q2),
    ],
    normalize: statements.normalize(result, config),
    normalize_json: JSON.parse(statements.normalizeJson(result, config)),
    forecast_specs: [
      statements.forecastSpecForwardFill(),
      statements.forecastSpecGrowth(0.05),
      statements.forecastSpecCurve([0.05, 0.04]),
      statements.forecastSpecNormal(100.0, 10.0, 7),
      statements.forecastSpecLogNormal(0.0, 0.2, 7),
      statements.forecastSpecOverride({ [Q2]: 125.0 }),
      statements.forecastSpecSeasonal([1.0, 2.0, 3.0, 4.0], 4, 'additive'),
      statements.forecastSpecTimeSeries([1.0, 2.0, 3.0]),
      statements.forecastSpecFadeToTarget(50.0),
      statements.forecastSpecMeanReverting(100.0, 0.5, 5.0, 11),
      statements.forecastSpecBootstrap([1.0, 2.0, 3.0], 13),
    ],
    adjustments: [
      oneOff,
      synergy,
      statements.adjustmentWithCap(synergy, 'revenue', 0.05),
      statements.adjustmentWithCapMode(synergy, null, 3.0, 'progressive'),
      statements.adjustmentWithCategory(oneOff, 'non_recurring'),
    ],
    'normalization_config.add_adjustment': extended,
    check_report: {
      has_errors: statements.checkReportHasErrors(report),
      has_warnings: statements.checkReportHasWarnings(report),
      errors: statements
        .checkReportFindingsBySeverity(report, 'error')
        .map((finding) => [finding.check_id, finding.period]),
      warnings: statements.checkReportFindingsBySeverity(report, 'warning').length,
    },
    builtin_check_names: statements.checkSuiteSpecBuiltinCheckNames(),
    cs: {
      interest: statements.capitalStructureCashflowsGetInterest(cs, 'TL-A', Q1),
      interest_cash: statements.capitalStructureCashflowsGetInterestCash(cs, 'TL-A', Q1),
      interest_pik: statements.capitalStructureCashflowsGetInterestPik(cs, 'TL-A', Q1),
      principal: statements.capitalStructureCashflowsGetPrincipal(cs, 'TL-A', Q1),
      debt_balance: statements.capitalStructureCashflowsGetDebtBalance(cs, 'TL-A', Q1),
      fees: statements.capitalStructureCashflowsGetFees(cs, 'TL-A', Q1),
      accrued_interest: statements.capitalStructureCashflowsGetAccruedInterest(cs, 'TL-A', Q1),
      total_interest: statements.capitalStructureCashflowsGetTotalInterest(cs, Q1),
      total_principal: statements.capitalStructureCashflowsGetTotalPrincipal(cs, Q1),
      total_debt_balance: statements.capitalStructureCashflowsGetTotalDebtBalance(cs, Q1),
      total_fees: statements.capitalStructureCashflowsGetTotalFees(cs, Q1),
    },
    registry: {
      count: registry.metricIds().length,
      first,
      has: [registry.has(first), registry.has('fin.no_such_metric')],
      definition: registry.get(first),
      dependencies: registry.dependencies(first),
    },
  };
  registry.free();
  assertMatches(plain(actual), EXPECTED.statements, 'statements');
});

test('financialModelBuilder is the ModelBuilder constructor', () => {
  const builder = statements.financialModelBuilder('twin');
  assert.ok(builder instanceof statements.ModelBuilder);
  builder.periods('2025Q1..Q1');
  builder.value('revenue', { [Q1]: 1.0 });
  assert.deepEqual(statements.modelNodeIds(builder.build()), ['revenue']);
});

test('ModelBuilder.fromSpec resumes a model and periodsExplicit takes Period objects', () => {
  const resumed = statements.ModelBuilder.fromSpec(MODEL);
  resumed.compute('double', 'revenue * 2');
  resumed.whereClause('revenue > 0');
  assert.deepEqual(statements.modelNodeIds(resumed.build()), [
    'revenue',
    'cogs',
    'profit',
    'margin',
    'double',
  ]);

  const explicit = new statements.ModelBuilder('explicit');
  explicit.insertNode({ node_id: 'seed', node_type: 'value', values: { [Q1]: 2.0 } });
  explicit.periodsExplicit(MODEL.periods);
  explicit.availabilityDates('seed', { [Q1]: '2025-04-15' });
  const model = explicit.build();
  assert.equal(model.periods.length, 2);
  assert.equal(model.nodes.seed.availability_dates[Q1], '2025-04-15');
});

test('ModelBuilder reports misuse without discarding the model', () => {
  const builder = new statements.ModelBuilder('guard');
  assert.throws(
    () => builder.compute('x', 'y'),
    (e) => e.kind === 'validation' && /periods\(\)/.test(e.message)
  );
  assert.throws(
    () => builder.periods('not a range'),
    (e) => e.kind === 'validation'
  );
  builder.periods('2025Q1..Q2');
  assert.throws(
    () => builder.periods('2025Q1..Q2'),
    (e) => e.kind === 'validation' && /already set/.test(e.message)
  );
  assert.throws(
    () => builder.compute('bad', 'revenue @'),
    (e) => e.kind === 'validation'
  );
  builder.valueScalar('revenue', { [Q1]: 1.0 });
  builder.build();
  assert.throws(
    () => builder.build(),
    (e) => e.kind === 'validation' && /no longer usable/.test(e.message)
  );
});

test('ModelBuilder capital-structure and registry steps build a valid model', () => {
  const registry = new statements.Registry();
  registry.loadFromJsonStr({
    namespace: 'custom',
    schema_version: 1,
    metrics: [{ id: 'half', name: 'Half revenue', formula: 'revenue * 0.5' }],
  });
  assert.deepEqual(registry.metricIds(), ['custom.half']);
  assert.throws(
    () => registry.get('custom.nope'),
    (e) => e.kind === 'not_found'
  );

  const builder = new statements.ModelBuilder('debt');
  builder.reportingCurrency('USD');
  builder.fxPolicy('period_end');
  builder.periods('2025Q1..Q4');
  builder.valueScalar('revenue', { [Q1]: 100.0 });
  builder.addMetricFromRegistry('custom.half', registry);
  builder.addBond(
    'BOND-1',
    { amount: '1000000', currency: 'USD' },
    0.05,
    '2025-01-15',
    '2030-01-15',
    'USD-OIS'
  );
  builder.addBondWithConvention(
    'BOND-2',
    { amount: '1000000', currency: 'USD' },
    0.03,
    '2025-01-15',
    '2030-01-15',
    'us_treasury',
    'USD-OIS'
  );
  builder.addSwap(
    'SWAP-1',
    { amount: '1000000', currency: 'USD' },
    0.04,
    '2025-01-15',
    '2030-01-15',
    'USD-OIS',
    'USD-SOFR-3M'
  );
  builder.addSwapWithConventions(
    'SWAP-2',
    { amount: '1000000', currency: 'USD' },
    0.04,
    '2025-01-15',
    '2030-01-15',
    'USD-OIS',
    'USD-SOFR-3M',
    '1Y',
    'act_360',
    '3M',
    'act_360'
  );
  const model = builder.build();
  registry.free();
  const ids = model.capital_structure.debt_instruments.map((instrument) => instrument.id);
  assert.deepEqual(ids, ['BOND-1', 'BOND-2', 'SWAP-1', 'SWAP-2']);
  assert.equal(model.capital_structure.reporting_currency, 'USD');
  assert.equal(model.capital_structure.fx_policy, 'period_end');
  assert.ok(statements.modelNodeIds(model).includes('custom.half'));

  const withDebt = new statements.ModelBuilder('envelope');
  withDebt.periods('2025Q1..Q4');
  withDebt.addDebt('BOND-3', {
    schema: 'finstack_quant.instrument/1',
    instrument: model.capital_structure.debt_instruments[0].spec,
  });
  withDebt.valueScalar('cash', { [Q1]: 1000.0 });
  withDebt.waterfall({
    priority_of_payments: ['fees', 'interest', 'amortization', 'equity'],
    available_cash_node: 'cash',
  });
  withDebt.withBuiltinMetrics();
  assert.equal(withDebt.build().capital_structure.debt_instruments[0].id, 'BOND-3');

  const bad = new statements.ModelBuilder('bad');
  assert.throws(
    () => bad.fxPolicy('sometimes'),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () =>
      bad.addBondWithConvention(
        'B',
        { amount: '1', currency: 'USD' },
        0.01,
        '2025-01-15',
        '2030-01-15',
        'martian',
        'USD-OIS'
      ),
    (e) => e.kind === 'validation'
  );
});

test('MixedNodeBuilder.valuesMoney and forecast feed the mixed node', () => {
  const builder = new statements.ModelBuilder('mixed');
  builder.periods('2025Q1..Q2', '2025Q1');
  const mixed = builder.mixed('cash');
  mixed.valuesMoney({ [Q1]: { amount: '100', currency: 'USD' } });
  mixed.forecast(statements.forecastSpecGrowth(0.1));
  const result = evaluate(mixed.build().build());
  const cash = statements.statementResultGetMoney(result, 'cash', Q2);
  assert.equal(Number(cash.amount), 110);
  assert.equal(cash.currency, 'USD');
  assert.throws(
    () => mixed.name('again'),
    (e) => e.kind === 'validation' && /consumed/.test(e.message)
  );
});

test('Evaluator.evaluateWithMarket and evaluateMonteCarlo keep their Rust twins', () => {
  const evaluator = new statements.Evaluator();
  assert.throws(
    () => evaluator.evaluateWithMarket(MODEL, {}, 'not-a-date'),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => evaluator.evaluateMonteCarlo(MODEL, { n_paths: 0, seed: 1 }),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => evaluator.withChecks({ name: 'x', unknown_key: 1 }),
    (e) => e.kind === 'validation'
  );
  const results = evaluator.evaluateMonteCarlo(MODEL, { n_paths: 4, seed: 1 });
  assert.equal(results.n_paths, 4);
  evaluator.free();
});

test('statement accessors reject malformed inputs with a validation kind', () => {
  const result = evaluate(MODEL);
  assert.throws(
    () => statements.statementResultGet(result, 'profit', 'not-a-period'),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => statements.statementResultGetOr(result, 'profit', Q1, '0'),
    (e) => e instanceof TypeError && e.kind === 'invalid_type'
  );
  assert.throws(
    () => statements.checkReportFindingsBySeverity({ results: [], summary: {} }, 'fatal'),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => statements.capitalStructureCashflowsGetInterest(INPUTS.cs_cashflows, 'TL-B', Q1),
    (e) => e.kind === 'computation'
  );
  assert.throws(
    () => statements.capitalStructureCashflowsGetTotalFees(INPUTS.cs_cashflows, Q2),
    (e) => e.kind === 'computation'
  );
});

test('normalization validation and duplicate adjustments are rejected in Rust', () => {
  const config = INPUTS.normalization_config;
  assert.equal(
    JSON.parse(statements.validateNormalizationConfigJson(config)).target_node,
    'profit'
  );
  assert.throws(
    () => statements.normalizationConfigAddAdjustment(config, config.adjustments[0]),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () =>
      statements.validateNormalizationConfigJson({
        target_node: 'profit',
        adjustments: [config.adjustments[0], config.adjustments[0]],
      }),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => statements.adjustmentWithCapMode(config.adjustments[0], null, 1.0, 'sideways'),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => statements.forecastSpecSeasonal([1, 2], 2, 'sideways'),
    (e) => e.kind === 'validation'
  );
});
