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

/** The shared capital-structure model and its evaluation on the shared market. */
function evaluatedCapitalStructure() {
  const evaluator = new statements.Evaluator();
  try {
    const model = INPUTS.capital_structure_model;
    return {
      model,
      results: evaluator.evaluateWithMarket(
        model,
        INPUTS.capital_structure_market,
        INPUTS.capital_structure_as_of
      ),
    };
  } finally {
    evaluator.free();
  }
}

const valueOf = (model, node, period) =>
  orNull(statements.statementResultGet(evaluate(model), node, period));

test('statements_analytics matches the cross-host golden', () => {
  const result = evaluate(MODEL);
  const capitalStructure = evaluatedCapitalStructure();
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
    { type: 'gordon_growth', stable_growth_rate: 0.02 },
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
      { type: 'gordon_growth', stable_growth_rate: 0.02 },
      { type: 'exit_multiple', multiple: 8.5, terminal_metric: 0.0 },
      { type: 'h_model', high_growth_rate: 0.08, stable_growth_rate: 0.02, half_life_years: 5.0 },
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
      config: {
        ...INPUTS.sensitivity_config,
        parameters: [
          ...INPUTS.sensitivity_config.parameters,
          { node_id: 'cogs', period_id: Q2, base_value: 50.0, perturbations: [45.0, 55.0] },
        ],
      },
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
    // Execution goldens for the comps statistics, the LBO and the DCF tornado
    // (audit finding F240). The empty and one-element samples pin the Rust
    // "no statistic" answers on both hosts.
    comps_stats: {
      peer_stats: INPUTS.comps_samples.map((sample) => orNull(sa.peerStats(sample))),
      percentile_rank: INPUTS.comps_samples.map((sample) => orNull(sa.percentileRank(sample, 2.5))),
      z_score: INPUTS.comps_samples.map((sample) => orNull(sa.zScore(sample, 2.5))),
    },
    lbo: {
      plain: sa.evaluateLbo(INPUTS.lbo_model, INPUTS.lbo_config),
      checked: sa.evaluateLbo(INPUTS.lbo_model, {
        ...INPUTS.lbo_config,
        check_mappings: INPUTS.lbo_check_mappings,
      }),
    },
    capital_structure_summary: {
      full_year: sa.capitalStructureSummary(capitalStructure.results, capitalStructure.model, {
        period: '2025Q4',
        ebitda_node: 'ebitda',
        cash_node: 'cash',
      }),
      short_window: sa.capitalStructureSummary(capitalStructure.results, capitalStructure.model, {
        period: '2025Q3',
        ebitda_node: 'ebitda',
      }),
    },
    relative_value_report: {
      full: sa.relativeValueReport(
        INPUTS.relative_value_subject,
        INPUTS.relative_value_universe,
        INPUTS.relative_value_config
      ),
      table_only: sa.relativeValueReport(
        INPUTS.relative_value_subject,
        INPUTS.relative_value_universe,
        INPUTS.relative_value_table_only_config
      ),
      partial_filter: sa.relativeValueReport(
        INPUTS.relative_value_subject,
        INPUTS.relative_value_universe,
        INPUTS.relative_value_partial_filter_config
      ),
    },
    dcf_sensitivity: {
      gordon_growth: sa.dcfSensitivity(
        INPUTS.dcf_model,
        0.1,
        { type: 'gordon_growth', stable_growth_rate: 0.02 },
        'ufcf',
        0.0
      ),
      exit_multiple: sa.dcfSensitivity(
        INPUTS.dcf_model,
        0.1,
        { type: 'exit_multiple', multiple: 8.0, terminal_metric: 999.0 },
        'ufcf',
        0.0,
        { exit_multiple_metric_node: 'ebitda' }
      ),
    },
  };
  tracer.free();
  corkscrew.free();
  scorecard.free();
  assertMatches(plain(actual), EXPECTED.statements_analytics, 'statements_analytics');
});

test('capitalStructureSummary reads the LTM window and rejects a bad request', () => {
  const { model, results } = evaluatedCapitalStructure();
  const summary = sa.capitalStructureSummary(results, model, {
    period: '2025Q4',
    ebitda_node: 'ebitda',
    cash_node: 'cash',
  });
  assert.deepEqual(
    summary.tranches.map((tranche) => [tranche.id, tranche.rank, tranche.payment_class]),
    [
      ['SR-A', 0, '1L'],
      ['SR-B', 0, '1L'],
      ['SUB', 1, '2L'],
    ]
  );
  // 920,000 of debt over 100,000 + 110,000 + 120,000 + 130,000 of EBITDA.
  assert.equal(summary.total_leverage, 2);
  assert.deepEqual(
    summary.maturity_wall.map((bucket) => [bucket.year, bucket.amount.amount]),
    [
      [2029, '500000'],
      [2031, '420000'],
    ]
  );
  const short = sa.capitalStructureSummary(results, model, {
    period: '2025Q3',
    ebitda_node: 'ebitda',
  });
  assert.deepEqual(
    [short.ltm_periods_required, short.ltm_periods_found, short.total_leverage],
    [4, 3, null]
  );
  for (const request of [
    { period: '2025Q4', ebitda_node: 'nope' },
    { period: '2030Q1', ebitda_node: 'ebitda' },
    { period: '2025Q4', ebitda_node: 'ebitda', guess: true },
  ]) {
    assert.throws(
      () => sa.capitalStructureSummary(results, model, request),
      (error) => error.kind === 'validation'
    );
  }
});

test('relativeValueReport hands over the table, the scatter and the score', () => {
  const subject = INPUTS.relative_value_subject;
  const universe = INPUTS.relative_value_universe;
  const report = sa.relativeValueReport(subject, universe, INPUTS.relative_value_config);
  assert.deepEqual(
    report.companies.map((row) => [row.id, row.is_subject]),
    [
      ['SUBJ', true],
      ['P1', false],
      ['P2', false],
      ['P3', false],
      ['P4', false],
      ['P5', false],
    ]
  );
  assert.deepEqual([report.peer_count, report.excluded_count], [5, 1]);
  // 300 / 2.5; 1000 / 100; no P/E inputs.
  assert.deepEqual(report.companies[0].values, [120, 10, null]);
  const [regression, , disabled, unavailable] = report.dimensions;
  // y = 10 + 95x over the four pairs, so 105 at x = 1; P5 has no leverage and no point.
  assert.deepEqual(
    regression.points.map((point) => [point.company_id, point.is_subject]),
    [
      ['SUBJ', true],
      ['P1', false],
      ['P2', false],
      ['P3', false],
      ['P4', false],
    ]
  );
  assert.ok(Math.abs(regression.points[1].fitted_y - 105) < 1e-9);
  assert.equal(disabled.excluded_reason, 'zero_weight');
  assert.equal(unavailable.excluded_reason, 'no_subject_value');

  const tableOnly = sa.relativeValueReport(
    subject,
    universe,
    INPUTS.relative_value_table_only_config
  );
  assert.equal(tableOnly.composite, null);
  assert.equal(tableOnly.composite_excluded_reason, 'no_usable_dimension');
  assert.deepEqual(tableOnly.period_basis, { custom: 'FY2025E' });

  for (const config of [
    { period_basis: 'ltm', dimensions: [], multiples: [] },
    { period_basis: 'ltm', dimensions: [], multiples: ['ev_ebitda'], guess: true },
    { filter: { gics_sector: ['energy'] }, period_basis: 'ltm', dimensions: [], multiples: ['pe'] },
    {
      period_basis: 'ltm',
      dimensions: [{ label: 'bad', y_extractor: { named: 'oas_bp' }, weight: -1 }],
      multiples: [],
    },
  ]) {
    assert.throws(
      () => sa.relativeValueReport(subject, universe, config),
      (error) => error.kind === 'validation'
    );
  }
  assert.throws(
    () =>
      sa.relativeValueReport(subject, [{ id: 'TYPO', ebidta: 100 }], INPUTS.relative_value_config),
    (error) => error.kind === 'validation' && /ebidta/.test(error.message)
  );
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
