/**
 * Cross-host goldens and entry-point tests for the `statements_analytics`
 * parity surface.
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
  orNull,
  plain,
} from './statements_parity_support.mjs';

const { statements, statements_analytics: sa } = facade;
const MODEL = INPUTS.model;

function evaluate(model) {
  const evaluator = new statements.Evaluator();
  try {
    return evaluator.evaluate(model);
  } finally {
    evaluator.free();
  }
}

const valueOf = (model, node, period) =>
  orNull(statements.statementResultGet(evaluate(model), node, period));

test('statements_analytics matches the cross-host golden', () => {
  const result = evaluate(MODEL);
  const tracer = new sa.DependencyTracer(MODEL);

  const corkBuilder = new statements.ModelBuilder('cork');
  corkBuilder.periods('2025Q1..Q2');
  corkBuilder.valueScalar('cash', { [Q1]: 100.0, [Q2]: 120.0 });
  corkBuilder.valueScalar('inflow', { [Q1]: 0.0, [Q2]: 20.0 });
  const corkModel = corkBuilder.build();
  const corkResult = evaluate(corkModel);
  const corkscrew = new sa.CorkscrewExtension(INPUTS.corkscrew_config);
  const scorecard = new sa.CreditScorecardExtension(INPUTS.scorecard_config);

  const rolled = sa.addRollForwardWithOpening(MODEL, 'debt', ['revenue'], ['cogs'], 10.0);
  const vintage = sa.addVintageBuildup(MODEL, 'book', 'revenue', [1.0, 0.5]);
  const noi = sa.addNoiBuildup(MODEL, 'total_rev', ['revenue'], 'total_exp', ['cogs'], 'noi');
  const ncf = sa.addNcfBuildup(noi, 'noi', ['cogs'], 'ncf');
  const rentRoll = sa.addRentRoll(MODEL, [INPUTS.lease]);
  const property = sa.addPropertyOperatingStatement(MODEL, [INPUTS.lease], null, ['cogs']);

  const exposure = INPUTS.exposure;
  const schedule = INPUTS.pd_schedule;
  const stressed = schedule.map(([t, pd]) => [t, pd * 1.5]);

  const dcfBuilder = new statements.ModelBuilder('dcf');
  dcfBuilder.periods('2025..2026');
  dcfBuilder.valueMoney('ufcf', {
    2025: { amount: '100', currency: 'USD' },
    2026: { amount: '110', currency: 'USD' },
  });
  dcfBuilder.withMeta('currency', 'USD');
  const dcf = sa.evaluateDcf(
    dcfBuilder.build(),
    0.1,
    sa.terminalValueSpecGordonGrowth(0.02),
    undefined,
    0.0
  );

  assert.throws(
    () => sa.runCorporateAnalysis(MODEL, { wacc: 0.1 }),
    (e) => e.kind === 'validation' && /terminal_value is required/.test(e.message)
  );
  const corporate = sa.runCorporateAnalysis(MODEL);

  const scenarioSet = INPUTS.scenario_set;
  const scenarioResults = sa.evaluateScenarioSet(MODEL, scenarioSet);
  const down = sa.scenarioResultsGet(scenarioResults, 'down');

  const sensitivity = sa.runSensitivity(MODEL, INPUTS.sensitivity_config);
  assert.throws(
    () => sa.sensitivityResultGetValue(sensitivity, 9, 'profit', Q2),
    (e) => e.kind === 'validation' && /scenario index 9 out of range/.test(e.message)
  );

  const actual = {
    tracer: {
      direct: tracer.directDependencies('margin'),
      all: tracer.allDependencies('margin'),
      dependents: tracer.dependents('revenue'),
      tree: tracer.dependencyTree('margin'),
      text: tracer.dependencyTreeText('margin'),
      detailed: tracer.dependencyTreeDetailedText(result, 'margin', Q1),
    },
    corkscrew: {
      config: corkscrew.config(),
      report: corkscrew.execute(corkModel, corkResult),
    },
    scorecard: {
      config: scorecard.config(),
      report: scorecard.execute(MODEL, result),
    },
    templates: {
      'roll_forward.node_ids': statements.modelNodeIds(rolled),
      'roll_forward.end': [valueOf(rolled, 'debt_end', Q1), valueOf(rolled, 'debt_end', Q2)],
      'vintage.book': [valueOf(vintage, 'book', Q1), valueOf(vintage, 'book', Q2)],
      noi: [valueOf(noi, 'noi', Q1), valueOf(noi, 'noi', Q2)],
      ncf: [valueOf(ncf, 'ncf', Q1), valueOf(ncf, 'ncf', Q2)],
      'rent_roll.node_ids': statements.modelNodeIds(rentRoll),
      'property.node_ids': statements.modelNodeIds(property),
      'property.noi': [valueOf(property, 'noi', Q1), valueOf(property, 'noi', Q2)],
    },
    ecl: {
      stage: sa.classifyStage(exposure, 0.03, 0.02).stage,
      single: sa.computeEcl(exposure, schedule, 'stage1').ecl,
      lifetime: sa.computeEcl(exposure, schedule, 'stage2').ecl,
      weighted: sa.computeEclWeighted(
        exposure,
        [
          [0.6, schedule],
          [0.4, stressed],
        ],
        'stage2'
      ).ecl,
    },
    dcf: {
      enterprise_value: Number(dcf.enterprise_value.amount),
      equity_value: Number(dcf.equity_value.amount),
      currency: dcf.enterprise_value.currency,
    },
    corporate: {
      node_count: Object.keys(corporate.statement.nodes).length,
      equity: corporate.equity,
    },
    terminal_specs: [
      sa.terminalValueSpecGordonGrowth(0.02),
      sa.terminalValueSpecExitMultiple(8.5, 0.0),
      sa.terminalValueSpecHModel(0.08, 0.02, 5.0),
    ],
    scenario_diff: sa.scenarioDiff(scenarioSet, scenarioResults, 'base', 'down', ['profit'], [Q2]),
    variance_bridge: sa.varianceBridge(result, down, 'profit', Q2, ['revenue'], 'base', 'down'),
    scenario_get: [
      statements.statementResultGet(down, 'profit', Q2),
      sa.scenarioResultsGet(scenarioResults, 'nope') === undefined,
    ],
    scenario_trace: sa.scenarioSetTrace(scenarioSet, 'down'),
    pl_summary_text: sa.plSummaryReportText(result, ['revenue', 'profit'], [Q1, Q2]),
    company_get: [
      orNull(sa.companyMetricsGet(INPUTS.subject, 'ebitda')),
      orNull(sa.companyMetricsGet(INPUTS.subject, 'rule_of_40')),
      orNull(sa.companyMetricsGet(INPUTS.subject, 'nope')),
    ],
    peer_accepts: INPUTS.universe.map((row) => sa.peerFilterAccepts(INPUTS.peer_filter, row)),
    peer_set: sa
      .peerSetFromUniverse(INPUTS.subject, INPUTS.universe, INPUTS.peer_filter, 'ltm')
      .peers.map((row) => row.id),
    sensitivity: {
      config: sa.sensitivityConfigAddParameter(INPUTS.sensitivity_config, {
        node_id: 'cogs',
        period_id: Q2,
        base_value: 50.0,
        perturbations: [45.0, 55.0],
      }),
      parameter_value: orNull(
        sa.sensitivityResultGetParameterValue(sensitivity, 0, `revenue@${Q2}`)
      ),
      parameter_missing: orNull(
        sa.sensitivityResultGetParameterValue(sensitivity, 0, `cogs@${Q2}`)
      ),
      value: orNull(sa.sensitivityResultGetValue(sensitivity, 0, 'profit', Q2)),
    },
    explanation_text: sa.explanationToText(sa.explainFormula(MODEL, result, 'profit', Q1)),
    forecast_summary: sa.forecastMetricsSummaryText(
      sa.backtestForecast([100.0, 110.0], [98.0, 112.0])
    ),
  };
  tracer.free();
  corkscrew.free();
  scorecard.free();
  assertMatches(plain(actual), EXPECTED.statements_analytics, 'statements_analytics');
});

test('plSummaryReport returns the Rust table of the text report', () => {
  const result = evaluate(MODEL);
  const table = sa.plSummaryReport(result, ['revenue', 'profit'], [Q1, Q2]);
  assert.ok(Array.isArray(table.columns) && table.columns.length > 0);
  assert.ok(JSON.stringify(table).includes('profit'));
  assert.throws(
    () => sa.plSummaryReport(result, ['revenue'], ['not-a-period']),
    (e) => e.kind === 'validation'
  );
});

test('validateScorecardConfig and the spec validators run the Rust checks', () => {
  assert.equal(sa.validateScorecardConfig(INPUTS.scorecard_config), undefined);
  assert.throws(
    () => sa.validateScorecardConfig({ ...INPUTS.scorecard_config, rating_scale: 'Martian' }),
    (e) => e.kind === 'validation'
  );
  assert.equal(JSON.parse(sa.validateLeaseSpecJson(INPUTS.lease)).node_id, 'lease_a');
  assert.throws(
    () => sa.validateLeaseSpecJson({ ...INPUTS.lease, occupancy: 1.5 }),
    (e) => e.kind === 'validation'
  );
  const renewal = { term_periods: 4, probability: 0.5, rent_factor: 1.1 };
  assert.equal(JSON.parse(sa.validateRenewalSpecJson(renewal)).term_periods, 4);
  assert.throws(
    () => sa.validateRenewalSpecJson({ ...renewal, probability: 2 }),
    (e) => e.kind === 'validation'
  );
});

test('addRollForward opens at zero and the templates reject bad inputs', () => {
  const rolled = sa.addRollForward(MODEL, 'debt', ['revenue'], ['cogs']);
  assert.equal(valueOf(rolled, 'debt_end', Q1), 60);
  assert.throws(
    () => sa.addRentRoll(MODEL, []),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => sa.addVintageBuildup(MODEL, 'book', 'revenue', ['x']),
    (e) => e instanceof TypeError && e.kind === 'invalid_type'
  );
  const named = sa.addRentRoll(MODEL, [INPUTS.lease], {
    rent_pgi_node: 'pgi',
    free_rent_node: 'free',
    vacancy_loss_node: 'vacancy',
    rent_effective_node: 'effective',
  });
  assert.ok(statements.modelNodeIds(named).includes('effective'));
});

test('classifyStage honours a StagingConfig and rejects bad PDs', () => {
  const strict = sa.classifyStage(INPUTS.exposure, 0.03, 0.02, INPUTS.staging_config);
  assert.equal(strict.stage, 'stage2');
  assert.throws(
    () => sa.classifyStage(INPUTS.exposure, 1.5, 0.02),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => sa.computeEcl(INPUTS.exposure, INPUTS.pd_schedule, 'stage9'),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => sa.computeEclWeighted(INPUTS.exposure, [], 'stage1'),
    (e) => e.kind === 'validation'
  );
});

test('peerSetFromUniverse takes the serde period basis', () => {
  const custom = sa.peerSetFromUniverse(INPUTS.subject, INPUTS.universe, INPUTS.peer_filter, {
    custom: 'FY2025E',
  });
  assert.deepEqual(custom.period_basis, { custom: 'FY2025E' });
  assert.throws(
    () => sa.peerSetFromUniverse(INPUTS.subject, INPUTS.universe, INPUTS.peer_filter, 'yearly'),
    (e) => e.kind === 'validation'
  );
});

test('DependencyTracer and the extensions reject malformed inputs', () => {
  assert.throws(
    () => new sa.DependencyTracer('{'),
    (e) => e.kind === 'validation'
  );
  const tracer = new sa.DependencyTracer(MODEL);
  assert.throws(
    () => tracer.dependencyTree('nope'),
    (e) => e.kind === 'not_found'
  );
  tracer.free();
  assert.throws(
    () => new sa.CorkscrewExtension({ accounts: [], surprise: true }),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => new sa.CreditScorecardExtension({ metrics: 'none' }),
    (e) => e.kind === 'validation'
  );
});
