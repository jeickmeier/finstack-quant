/**
 * Typed covenants surface: every entry point bound for Python parity, with
 * cross-host goldens.
 *
 * Each case computes one JSON value from fixed inputs. The values are pinned
 * in `golden/covenants_parity.json`, and
 * `finstack-quant-py/tests/test_covenants_wasm_parity.py` asserts the Python
 * twins against the same file. Regenerate deliberately with
 * `UPDATE_PARITY_GOLDEN=1 node --test tests/facade/covenants_parity.test.mjs`, then
 * `npx prettier --write tests/facade/golden`.
 */
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { covenants } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const GOLDEN = new URL('./golden/covenants_parity.json', import.meta.url);
const UPDATE = process.env.UPDATE_PARITY_GOLDEN === '1';

const QUARTERLY = { count: 3, unit: 'months' };
const leverage = () =>
  covenants.covenantNew({ max_debt_to_ebitda: { threshold: 4.5 } }, QUARTERLY, 'max_leverage');
const leverageSpec = () => covenants.covenantSpecWithMetric(leverage(), 'debt_to_ebitda');
const series = [
  { date: '2026-06-30', metrics: { debt_to_ebitda: 4.8, ebitda: 100 } },
  { date: '2026-03-31', metrics: { debt_to_ebitda: 4.0, ebitda: 110 } },
  { date: '2026-09-30', metrics: { debt_to_ebitda: 4.4, ebitda: 105 } },
];

function withEngine(specs, body) {
  const engine = covenants.CovenantEngine.fromSpecs(specs);
  try {
    return body(engine);
  } finally {
    engine.free();
  }
}

const cases = {
  covenant_types: () => [
    { max_debt_to_ebitda: { threshold: 4.5 } },
    { min_interest_coverage: { threshold: 2.0 } },
    { min_fixed_charge_coverage: { threshold: 1.2 } },
    { max_total_leverage: { threshold: 6.0 } },
    { max_senior_leverage: { threshold: 4.0 } },
    { min_asset_coverage: { threshold: 1.5 } },
    { negative: { restriction: 'no additional senior debt' } },
    { affirmative: { requirement: 'deliver audited financials within 90 days' } },
    { custom: { metric: 'net_working_capital', test: { minimum: 25.0 } } },
    { custom: { metric: 'capex_to_sales', test: { maximum: 0.08 } } },
    { basket: { name: 'restricted_payments', limit: 5_000_000 } },
    { min_dscr: { threshold: 1.25 } },
    { max_net_debt_to_ebitda: { threshold: 3.5 } },
    { max_capex: { threshold: 10_000_000 } },
    { min_liquidity: { threshold: 2_500_000 } },
  ],
  consequences: () => [
    'default',
    { rate_increase: { bp_increase: 200 } },
    { cash_sweep: { sweep_percentage: 0.5 } },
    'block_distributions',
    { require_collateral: { description: 'first lien on receivables' } },
    { accelerate_maturity: { new_maturity: '2027-12-31' } },
  ],
  covenant_modifiers: () => {
    const base = leverage();
    return [
      base,
      { ...base, cure_period_days: 30 },
      { ...base, cure_period_days: null },
      { ...base, consequences: [{ rate_increase: { bp_increase: 200 } }] },
      { ...base, scope: 'incurrence' },
      {
        ...base,
        springing_condition: {
          metric_id: 'revolver_utilization',
          test: { minimum: 0.35 },
        },
      },
    ];
  },
  spec_modifiers: () => {
    const netLeverage = covenants.covenantSpecWithMetric(
      covenants.covenantNew(
        { max_net_debt_to_ebitda: { threshold: 3.5 } },
        QUARTERLY,
        'max_net_leverage'
      ),
      'net_debt'
    );
    const schedule = [
      ['2026-01-01', 4.5],
      ['2026-07-01', 4.0],
    ];
    return {
      new: covenants.covenantSpecNew(
        covenants.covenantNew(
          { max_net_debt_to_ebitda: { threshold: 3.5 } },
          QUARTERLY,
          'max_net_leverage'
        )
      ),
      with_metric: [leverageSpec(), netLeverage],
      with_denominator_metric: { ...netLeverage, denominator_metric_id: 'adjusted_ebitda' },
      with_threshold_schedule: { ...leverageSpec(), threshold_schedule: schedule },
      threshold_for: ['2025-12-31', '2026-01-01', '2026-06-30', '2026-07-01'].map(
        (date) => covenants.thresholdScheduleThresholdFor(schedule, date) ?? null
      ),
    };
  },
  templates: () => [
    covenants.covenantTemplate({
      kind: 'lbo_standard',
      initial_leverage: 5.0,
      interest_coverage: 1.5,
      fixed_charge_coverage: 1.2,
      max_capex: 10_000_000,
    }),
    covenants.covenantTemplate({ kind: 'cov_lite', max_leverage: 7.0, max_senior_leverage: 4.5 }),
    covenants.covenantTemplate({
      kind: 'real_estate',
      min_dscr: 1.25,
      min_debt_yield: 0.08,
      max_ltv: 0.65,
    }),
    covenants.covenantTemplate({
      kind: 'project_finance',
      min_dscr: 1.3,
      distribution_lockup_dscr: 1.15,
      min_liquidity: 5_000_000,
      max_net_leverage: 4.0,
    }),
  ],
  engine_evaluate: () =>
    withEngine(
      covenants.covenantTemplate({ kind: 'cov_lite', max_leverage: 7.0, max_senior_leverage: 4.5 }),
      (engine) => ({
        pass: engine.evaluate({ total_leverage: 5.0, senior_leverage: 3.0 }, '2026-03-31'),
        breach: engine.evaluate(
          JSON.stringify({ total_leverage: 7.5, senior_leverage: 3.0 }),
          '2026-03-31'
        ),
        specs: engine.specs,
        waivers: engine.waivers,
        breach_history: engine.breachHistory,
      })
    ),
  engine_track_waive_and_round_trip: () => {
    const engine = new covenants.CovenantEngine();
    try {
      engine.addSpec(
        covenants.covenantSpecWithMetric({ ...leverage(), cure_period_days: 30 }, 'debt_to_ebitda')
      );
      engine.validate();
      const breached = engine.evaluateAndTrack(
        { debt_to_ebitda: 5.0 },
        '2026-03-31',
        'maintenance'
      );
      const history = engine.breachHistory;
      engine.addWaiver({
        covenant_id: 'max_leverage',
        effective_date: '2026-06-01',
        expiry_date: null,
        amended_threshold: 5.5,
        description: 'amendment no. 1',
      });
      const amended = engine.evaluate({ debt_to_ebitda: 5.0 }, '2026-06-30');
      const restored = covenants.CovenantEngine.fromJson(engine.toJson());
      try {
        return {
          breached,
          history,
          amended,
          waivers: restored.waivers,
          engine: JSON.parse(restored.toJson()),
        };
      } finally {
        restored.free();
      }
    } finally {
      engine.free();
    }
  },
  engine_evaluate_series: () =>
    withEngine([leverageSpec()], (engine) => engine.evaluateSeries(series)),
  forecast: () => ({
    deterministic: covenants.forecastCovenant(leverageSpec(), series),
    stochastic: covenants.forecastCovenant(leverageSpec(), series, {
      stochastic: true,
      volatility: 0.2,
      reference_date: '2025-12-31',
    }),
    monte_carlo: covenants.forecastCovenant(leverageSpec(), series, {
      stochastic: true,
      volatility: 0.2,
      num_paths: 2000,
      random_seed: 7,
      antithetic: true,
      reference_date: '2025-12-31',
    }),
    breaches: withEngine([leverageSpec()], (engine) => covenants.forecastBreaches(engine, series)),
    breaches_stochastic: withEngine([leverageSpec()], (engine) =>
      covenants.forecastBreaches(engine, series, {
        stochastic: true,
        volatility: 0.2,
        reference_date: '2025-12-31',
        breach_probability_threshold: 0.2,
      })
    ),
    with_scope: {
      scope: 'incurrence',
      stochastic: true,
      num_paths: 0,
      volatility: 0.2,
      random_seed: null,
      antithetic: false,
      breach_probability_threshold: 0.05,
    },
  }),
};

const golden = UPDATE ? {} : JSON.parse(readFileSync(GOLDEN, 'utf8'));

for (const [name, compute] of Object.entries(cases)) {
  test(`covenants parity golden: ${name}`, () => {
    const actual = JSON.parse(JSON.stringify(compute()));
    if (UPDATE) golden[name] = actual;
    else assert.deepEqual(actual, golden[name]);
  });
}

test('covenants parity golden: file is complete', () => {
  if (UPDATE) writeFileSync(GOLDEN, `${JSON.stringify(golden, null, 2)}\n`);
  assert.deepEqual(Object.keys(golden).sort(), Object.keys(cases).sort());
});

const kind = (expected, pattern) => (error) => {
  assert.ok(error instanceof Error, `expected an Error, got ${typeof error}: ${error}`);
  assert.equal(error.kind, expected, `kind of: ${error.message}`);
  assert.match(error.message, pattern);
  return true;
};

test('covenantTemplate expands each package and the engine agrees with the JSON bridge', () => {
  assert.equal(
    covenants.covenantTemplate({ kind: 'cov_lite', max_leverage: 7.0, max_senior_leverage: 4.5 })
      .length,
    3
  );
  assert.deepEqual(
    covenants.covenantTemplate(
      JSON.stringify({ kind: 'real_estate', min_dscr: 1.25, min_debt_yield: 0.08, max_ltv: 0.65 })
    ),
    covenants.covenantTemplate({
      kind: 'real_estate',
      min_dscr: 1.25,
      min_debt_yield: 0.08,
      max_ltv: 0.65,
    })
  );
  withEngine(
    covenants.covenantTemplate({ kind: 'cov_lite', max_leverage: 7.0, max_senior_leverage: 4.5 }),
    (engine) => {
      const metrics = { total_leverage: 5.0, senior_leverage: 3.0 };
      assert.deepEqual(
        engine.evaluate(metrics, '2026-03-31'),
        covenants.evaluateEngine(engine.toJson(), metrics, '2026-03-31')
      );
      assert.equal(engine.addSpec(leverageSpec()), undefined);
      assert.equal(engine.specs.length, 4);
    }
  );
});

test('covenant errors are structured and Rust-owned', () => {
  assert.throws(
    () =>
      covenants.covenantTemplate({ kind: 'cov_lite', max_leverage: -1, max_senior_leverage: 4.5 }),
    kind('validation', /finite|negative/i)
  );
  assert.throws(
    () =>
      covenants.covenantTemplate({ kind: 'cov_lite', max_leverage: '7', max_senior_leverage: 4.5 }),
    kind('validation', /template: invalid type/)
  );
  assert.throws(
    () => covenants.covenantTemplate({ kind: 'unitranche', max_leverage: 7 }),
    kind('validation', /unknown variant/)
  );
  assert.throws(
    () => covenants.covenantSpecNew({ ...leverage(), scope: 'sometimes' }),
    kind('validation', /covenant: unknown variant/)
  );
  assert.throws(
    () =>
      covenants.thresholdScheduleThresholdFor(
        [
          ['2026-01-01', 4.5],
          ['2026-01-01', 4.0],
        ],
        '2026-01-01'
      ),
    kind('validation', /schedule: .*duplicate date 2026-01-01/)
  );
  assert.throws(
    () => covenants.CovenantEngine.fromSpecs([{ covenant: 1 }]),
    kind('validation', /specs: invalid type/)
  );
  assert.throws(
    () => covenants.forecastCovenant(leverageSpec(), []),
    kind('validation', /at least one dated row/)
  );
  assert.throws(
    () => covenants.forecastCovenant(leverageSpec(), [series[0], series[0]]),
    kind('validation', /duplicate date 2026-06-30/)
  );
  assert.throws(
    () => covenants.forecastCovenant(leverageSpec(), [{ date: '2026-03-31', metrics: {} }]),
    kind('not_found', /debt_to_ebitda/)
  );
  assert.throws(
    () =>
      covenants.forecastCovenant(leverageSpec(), series, {
        stochastic: true,
        reference_date: '2025-12-31',
      }),
    kind('validation', /volatility/)
  );
  // Dated rows carry no period start, so a stochastic forecast must anchor its horizon.
  assert.throws(
    () => covenants.forecastCovenant(leverageSpec(), series, { stochastic: true, volatility: 0.2 }),
    kind('validation', /reference_date/)
  );
  withEngine([leverageSpec()], (engine) => {
    assert.throws(() => engine.evaluate({}, '2026-03-31'), kind('not_found', /debt_to_ebitda/));
    assert.throws(
      () => engine.evaluate({ debt_to_ebitda: 'high' }, '2026-03-31'),
      kind('validation', /must be a finite JSON number/)
    );
    assert.throws(
      () => engine.evaluateAndTrack({ debt_to_ebitda: 5 }, '2026-03-31', 'both'),
      kind('validation', /scope: unknown variant/)
    );
    assert.equal(engine.breachHistory.length, 0);
    assert.throws(
      () => engine.evaluateSeries([{ date: '2026-03-31', metrics: {} }]),
      kind('not_found', /debt_to_ebitda/)
    );
    assert.throws(
      () => engine.addWaiver({ covenant_id: 'x' }),
      kind('validation', /waiver: missing field `effective_date`/)
    );
  });
  assert.throws(
    () =>
      withEngine([leverageSpec(), leverageSpec()], (engine) =>
        engine.evaluate({ debt_to_ebitda: 4 }, '2026-03-31')
      ),
    kind('validation', /max_leverage/)
  );
});
