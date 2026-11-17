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
  builder.forecast('revenue', { method: 'growth_pct', params: { rate: 0.05 } });
  builder.compute('profit', 'revenue * 0.5');
  builder.valueMoney('cash', {
    [Q1]: { amount: '100', currency: 'USD' },
    [Q2]: { amount: '110', currency: 'USD' },
  });
  builder.rollup('cash', 'period_end');
  builder.withMeta('currency', 'USD');
  const mixed = builder.mixed('capex');
  mixed.values({ [Q1]: 10.0 });
  mixed.formula('revenue * 0.1');
  mixed.name('Capex');
  mixed.rollup('formula');
  const ready = mixed.build();
  return ready.build();
}

test('statements matches the cross-host golden', () => {
  const result = evaluate(MODEL);
  const stochastic = evaluate(INPUTS.stochastic_model);
  const built = builtModel();
  const builtResult = evaluate(built);

  const config = INPUTS.normalization_config;
  const oneOff = {
    id: 'one_off',
    name: 'One-off cost',
    value: { type: 'fixed', amounts: { [Q1]: 5.0 } },
  };
  const synergy = {
    id: 'synergy',
    name: 'Synergies',
    value: { type: 'percentage_of_node', node_id: 'revenue', percentage: 0.1 },
  };
  let extended = { target_node: 'profit', adjustments: [] };
  extended = statements.normalizationConfigAddAdjustment(extended, oneOff);
  extended = statements.normalizationConfigAddAdjustment(extended, {
    ...synergy,
    cap: { base_node: 'revenue', value: 0.05 },
  });

  const evaluator = new statements.Evaluator();
  evaluator.withChecks(INPUTS.check_suite);
  const report = evaluator.evaluate(MODEL).check_report;
  evaluator.free();

  const cs = INPUTS.cs_cashflows;
  const registry = statements.Registry.withBuiltins();
  const first = registry.metricIds()[0];

  const rollupSpec = statements.rollupSpecDeclaredMethods(INPUTS.rollup_model, 'annual');

  const rolledUp = statements.statementResultRollup(
    evaluate(INPUTS.rollup_model),
    INPUTS.rollup_model,
    rollupSpec
  );

  const actual = {
    'rollup.declared_methods': rollupSpec,
    'rollup.annual': rolledUp.cells,
    'rollup.empty_spec': statements.statementResultRollup(
      evaluate(INPUTS.rollup_model),
      INPUTS.rollup_model,
      { frequency: 'annual' }
    ).cells,
    'rollup.spec_overrides': statements.statementResultRollup(
      evaluate(INPUTS.rollup_model),
      INPUTS.rollup_model,
      { frequency: 'annual', methods: { cash: 'sum' } }
    ).cells.cash,
    'rollup.periods': rolledUp.periods,
    'result.get': orNull(statements.statementResultGet(result, 'profit', Q2)),
    'result.get_missing': orNull(statements.statementResultGet(result, 'nope', Q2)),
    'result.get_or': statements.statementResultGetOr(result, 'profit', Q2, -1.0),
    'result.get_cell': statements.statementResultGetCell(result, 'profit', Q2),
    'result.get_cell_missing': orNull(statements.statementResultGetCell(result, 'nope', Q2)),
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
    'stochastic.values': Object.fromEntries(
      statements
        .statementResultNodeIds(stochastic)
        .map((node) => [
          node,
          ['2025Q2', '2025Q3', '2025Q4'].map((period) =>
            statements.statementResultGet(stochastic, node, period)
          ),
        ])
    ),
    'stochastic.streams': stochastic.provenance.forecast_streams,
    forecast_specs: [
      { method: 'forward_fill' },
      { method: 'growth_pct', params: { rate: 0.05 } },
      { method: 'curve_pct', params: { curve: [0.05, 0.04] } },
      { method: 'normal', params: { mean: 100.0, std_dev: 10.0, seed: '7' } },
      { method: 'log_normal', params: { mean: 0.0, std_dev: 0.2, seed: '7' } },
      { method: 'override', params: { overrides: { [Q2]: 125.0 } } },
      {
        method: 'seasonal',
        params: { historical: [1.0, 2.0, 3.0, 4.0], season_length: 4, mode: 'additive' },
      },
      { method: 'time_series', params: { historical: [1.0, 2.0, 3.0], method: 'linear' } },
      { method: 'fade_to_target', params: { target: 50.0, shape: 'linear' } },
      {
        method: 'mean_reverting',
        params: { long_run_mean: 100.0, reversion_speed: 0.5, std_dev: 5.0, seed: '11' },
      },
      {
        method: 'bootstrap',
        params: { historical: [1.0, 2.0, 3.0], mode: 'growth', seed: '13' },
      },
    ],
    adjustments: [
      oneOff,
      synergy,
      { ...synergy, cap: { base_node: 'revenue', value: 0.05 } },
      { ...synergy, cap: { base_node: null, value: 3.0, base_mode: 'progressive' } },
      { ...oneOff, category: 'non_recurring' },
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
      interest: statements.capitalStructureCashflowsGetComponent(
        cs,
        'interest_expense',
        'TL-A',
        Q1
      ),
      interest_cash: statements.capitalStructureCashflowsGetComponent(
        cs,
        'interest_expense_cash',
        'TL-A',
        Q1
      ),
      interest_pik: statements.capitalStructureCashflowsGetComponent(
        cs,
        'interest_expense_pik',
        'TL-A',
        Q1
      ),
      principal: statements.capitalStructureCashflowsGetComponent(
        cs,
        'principal_payment',
        'TL-A',
        Q1
      ),
      debt_balance: statements.capitalStructureCashflowsGetComponent(
        cs,
        'debt_balance',
        'TL-A',
        Q1
      ),
      fees: statements.capitalStructureCashflowsGetComponent(cs, 'fees', 'TL-A', Q1),
      accrued_interest: statements.capitalStructureCashflowsGetComponent(
        cs,
        'accrued_interest',
        'TL-A',
        Q1
      ),
      total_interest: statements.capitalStructureCashflowsGetComponent(
        cs,
        'interest_expense',
        'total',
        Q1
      ),
      total_principal: statements.capitalStructureCashflowsGetComponent(
        cs,
        'principal_payment',
        'total',
        Q1
      ),
      total_debt_balance: statements.capitalStructureCashflowsGetComponent(
        cs,
        'debt_balance',
        'total',
        Q1
      ),
      total_fees: statements.capitalStructureCashflowsGetComponent(cs, 'fees', 'total', Q1),
    },
    registry: {
      count: registry.metricIds().length,
      first,
      has: [registry.has(first), registry.has('fin.no_such_metric')],
      definition: registry.get(first),
      dependencies: registry.getDependencies(first),
    },
  };
  registry.free();
  assertMatches(plain(actual), EXPECTED.statements, 'statements');
});

test('ModelBuilder.fromSpec resumes a model and periodsExplicit takes a PeriodPlan', () => {
  const resumed = statements.ModelBuilder.fromSpec(MODEL);
  resumed.compute('double', 'revenue * 2');
  resumed.whereClause('double', 'revenue > 0');
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
  assert.equal(model.periods.periods.length, 2);
  assert.equal(model.periods.calendar, 'gregorian');
  assert.equal(model.nodes.seed.availability_dates[Q1], '2025-04-15');
});

test('ModelBuilder reports misuse without discarding the model', () => {
  // Nodes may be added before `periods`; only `build` needs the timeline.
  const noTimeline = new statements.ModelBuilder('no-timeline');
  noTimeline.valueScalar('revenue', { [Q1]: 1.0 });
  assert.throws(() => noTimeline.build());
  const builder = new statements.ModelBuilder('guard');
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
  // Instruments come from the Valuations API; Statements has no bond or swap
  // construction helpers.
  const { core, valuations } = facade;
  const usd = new core.Currency('USD');
  const bond = (id, convention, coupon) =>
    valuations.instruments.Bond.withConvention(
      id,
      new core.Money(1_000_000, usd),
      new core.Rate(coupon),
      '2025-01-15',
      '2030-01-15',
      convention,
      'USD-OIS'
    );
  const swap = valuations.instruments.InterestRateSwap.fromConventions({
    id: 'SWAP-1',
    notional: { amount: '1000000', currency: 'USD' },
    side: 'pay',
    fixed_rate: 0.04,
    start_date: '2025-01-15',
    maturity: '2030-01-15',
    index_id: 'USD-SOFR-3M',
    discount_curve_id: 'USD-OIS',
    forward_curve_id: 'USD-SOFR-3M',
  });
  const treasury = bond('BOND-2', 'us_treasury', 0.03);
  builder.addDebt('BOND-1', bond('BOND-1', 'us_corporate', 0.05).toJson());
  builder.addDebt('BOND-2', treasury.toJson());
  builder.addDebt('SWAP-1', swap.toJson());
  for (const helper of ['addBond', 'addBondWithConvention', 'addSwap', 'addSwapWithConventions'])
    assert.equal(builder[helper], undefined);
  const model = builder.build();
  registry.free();
  const ids = model.capital_structure.debt_instruments.map((instrument) => instrument.id);
  assert.deepEqual(ids, ['BOND-1', 'BOND-2', 'SWAP-1']);
  // The model holds the canonical instrument unchanged: resolved terms and
  // the elected convention survive attachment.
  assert.deepEqual(
    model.capital_structure.debt_instruments[1].spec,
    JSON.parse(treasury.toJson()).instrument
  );
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
  const debtModel = withDebt.build();
  assert.equal(debtModel.capital_structure.debt_instruments[0].id, 'BOND-3');
  // The whole catalogue reads inputs this model does not define, and build
  // resolves every formula reference.
  const wholeCatalogue = new statements.ModelBuilder('whole-catalogue');
  wholeCatalogue.periods('2025Q1..Q4');
  wholeCatalogue.valueScalar('revenue', { [Q1]: 100.0 });
  wholeCatalogue.withBuiltinMetrics();
  assert.throws(() => wholeCatalogue.build(), /Unknown identifier/);

  const bad = new statements.ModelBuilder('bad');
  assert.throws(
    () => bad.fxPolicy('sometimes'),
    (e) => e.kind === 'validation'
  );
  // A capital-structure call rejected by Rust leaves the builder usable.
  assert.throws(
    () =>
      bad.addDebt('B', { schema: 'finstack_quant.instrument/1', instrument: { type: 'martian' } }),
    (e) => e.kind === 'validation'
  );
  bad.periods('2025Q1..Q2');
  bad.valueScalar('revenue', { [Q1]: 1.0 });
  assert.equal(bad.build().id, 'bad');
});

test('MixedNodeBuilder.valuesMoney and forecast feed the mixed node', () => {
  const builder = new statements.ModelBuilder('mixed');
  builder.periods('2025Q1..Q2', '2025Q1');
  const mixed = builder.mixed('cash');
  mixed.valuesMoney({ [Q1]: { amount: '100', currency: 'USD' } });
  mixed.forecast({ method: 'growth_pct', params: { rate: 0.1 } });
  const result = evaluate(mixed.build().build());
  const cash = statements.statementResultGetMoney(result, 'cash', Q2);
  // The growth forecast is evaluated in binary64: 100 * (1 + 0.1).
  assert.equal(Number(cash.amount), 100 * (1 + 0.1));
  assert.equal(cash.currency, 'USD');
  assert.throws(
    () => mixed.name('again'),
    (e) => e.kind === 'validation' && /consumed/.test(e.message)
  );
});

test('builder lines roll up with their declared method under an empty spec', () => {
  const quarters = { '2025Q1': 10, '2025Q2': 20, '2025Q3': 30, '2025Q4': 40 };
  const builder = new statements.ModelBuilder('declared');
  builder.periods('2025Q1..Q4');
  builder.value('flow', quarters);
  builder.rollup('flow', 'sum');
  builder.value('balance', quarters);
  builder.rollup('balance', 'period_end');
  assert.throws(
    () => builder.rollup('ghost', 'sum'),
    (e) => e.kind === 'validation' && /ghost/.test(e.message)
  );
  const mixed = builder.mixed('mixed_balance');
  mixed.values(quarters);
  mixed.rollup('period_end');
  const model = mixed.build().build();
  assert.equal(model.nodes.mixed_balance.rollup, 'period_end');
  const annual = statements.statementResultRollup(evaluate(model), model, { frequency: 'annual' });
  assert.equal(annual.cells.flow['2025'].value, 100);
  assert.equal(annual.cells.balance['2025'].value, 40);
  assert.equal(annual.cells.mixed_balance['2025'].value, 40);
});

test('Evaluator.evaluateWithMarket and evaluateMonteCarlo keep their Rust twins', () => {
  const evaluator = new statements.Evaluator();
  assert.throws(
    () => evaluator.evaluateWithMarket(MODEL, {}, 'not-a-date', 'full_snapshot'),
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
    () =>
      statements.capitalStructureCashflowsGetComponent(
        INPUTS.cs_cashflows,
        'interest_expense',
        'TL-B',
        Q1
      ),
    (e) => e.kind === 'computation'
  );
  assert.throws(
    () =>
      statements.capitalStructureCashflowsGetComponent(INPUTS.cs_cashflows, 'fees', 'total', Q2),
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
});

test('mergeIntoResults writes normalized values back as a node', () => {
  const result = evaluate(MODEL);
  const normalized = statements.normalize(result, INPUTS.normalization_config);
  const merged = statements.mergeIntoResults(result, normalized, 'adjusted_profit');
  for (const row of normalized) {
    assert.equal(
      statements.statementResultGet(merged, 'adjusted_profit', row.period),
      row.final_value
    );
  }
  assert.equal(statements.statementResultGet(result, 'adjusted_profit', Q1), undefined);
  assert.throws(
    () => statements.mergeIntoResults(result, [normalized[0], normalized[0]], 'x'),
    (e) => e.name === 'FinstackError'
  );
});

test('capital-structure interest-income and total accessors are Rust twins', () => {
  const cs = INPUTS.cs_cashflows;
  const cash = statements.capitalStructureCashflowsGetComponent(
    cs,
    'interest_expense_cash',
    'total',
    Q1
  );
  const pik = statements.capitalStructureCashflowsGetComponent(
    cs,
    'interest_expense_pik',
    'total',
    Q1
  );
  assert.equal(
    cash + pik,
    statements.capitalStructureCashflowsGetComponent(cs, 'interest_expense', 'total', Q1)
  );
  assert.equal(
    typeof statements.capitalStructureCashflowsGetComponent(cs, 'interest_income', 'total', Q1),
    'number'
  );
  assert.equal(
    typeof statements.capitalStructureCashflowsGetComponent(cs, 'accrued_interest', 'total', Q1),
    'number'
  );
  assert.equal(
    typeof statements.capitalStructureCashflowsGetComponent(cs, 'interest_income', 'TL-A', Q1),
    'number'
  );
});

test('component lookup covers all flows and scopes', () => {
  const expected = {
    interest_expense: 35,
    interest_expense_cash: 30,
    interest_expense_pik: 5,
    interest_income: 0,
    principal_payment: 100,
    debt_balance: 900,
    fees: 2,
    accrued_interest: 1.5,
  };
  for (const scope of ['TL-A', 'total']) {
    for (const [component, value] of Object.entries(expected)) {
      assert.equal(
        statements.capitalStructureCashflowsGetComponent(INPUTS.cs_cashflows, component, scope, Q1),
        value
      );
    }
  }
  assert.throws(
    () =>
      statements.capitalStructureCashflowsGetComponent(INPUTS.cs_cashflows, 'unknown', 'total', Q1),
    (e) => e.kind === 'computation'
  );
  assert.throws(
    () =>
      statements.capitalStructureCashflowsGetComponent(
        INPUTS.cs_cashflows,
        'fees',
        'total',
        'bad period'
      ),
    (e) => e.kind === 'validation'
  );
});

test('named edits, snapshot identity and the observation cutoff replay a revision', () => {
  const snapshot = () => {
    const builder = new statements.ModelBuilder('replay');
    builder.periods('2025Q1..Q2', '2025Q1');
    builder.valueScalar('revenue', { '2025Q1': 100.0, '2025Q2': 125.0 });
    builder.availabilityDates('revenue', { '2025Q1': '2025-04-25' });
    builder.sourceSnapshot('original');
    return builder.build();
  };
  const original = snapshot();
  assert.equal(original.source_snapshot, 'original');
  const revised = statements.applyModelEdits(original, [
    {
      edit: 'set_observation',
      node_id: 'revenue',
      period: '2025Q1',
      value: 110.0,
      availability_date: '2025-05-15',
    },
  ]);
  assert.equal(original.nodes.revenue.values['2025Q1'], 100.0);

  const read = (model, cutoff) => {
    const evaluator = new statements.Evaluator();
    try {
      return evaluator.evaluateWithMarket(
        model,
        INPUTS.capital_structure_market,
        '2025-06-30',
        cutoff
      ).cells.revenue['2025Q1'];
    } finally {
      evaluator.free();
    }
  };
  const hidden = { status: 'unavailable', reason: 'hidden_by_cutoff' };
  assert.equal(read(original, '2025-04-25').value, 100.0);
  assert.deepEqual(read(original, '2025-04-24'), hidden);
  assert.equal(read(revised, '2025-05-15').value, 110.0);
  assert.deepEqual(read(revised, '2025-05-14'), hidden);
  assert.deepEqual(read(revised, 'full_snapshot'), {
    status: 'value',
    value: 110.0,
    source: 'value',
  });

  assert.throws(
    () => statements.applyModelEdits(original, [{ edit: 'remove_node', node_id: 'absent' }]),
    /unknown node 'absent'/
  );
  const duplicate = new statements.ModelBuilder('duplicates');
  duplicate.periods('2025Q1..Q1');
  duplicate.valueScalar('revenue', { '2025Q1': 1.0 });
  duplicate.valueScalar('revenue', { '2025Q1': 2.0 });
  assert.throws(() => duplicate.build(), /defined more than once/);
  const edited = statements.ModelBuilder.fromSpec(original);
  edited.removeNode('revenue');
  edited.valueScalar('revenue', { '2025Q1': 1.0 });
  edited.replaceNode({ node_id: 'revenue', node_type: 'value', values: { '2025Q1': 2.0 } });
  assert.equal(edited.build().nodes.revenue.values['2025Q1'], 2.0);
});
