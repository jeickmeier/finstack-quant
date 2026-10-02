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
  covenants.covenantNew(covenants.covenantTypeMaxDebtToEbitda(4.5), QUARTERLY, 'max_leverage');
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
    covenants.covenantTypeMaxDebtToEbitda(4.5),
    covenants.covenantTypeMinInterestCoverage(2.0),
    covenants.covenantTypeMinFixedChargeCoverage(1.2),
    covenants.covenantTypeMaxTotalLeverage(6.0),
    covenants.covenantTypeMaxSeniorLeverage(4.0),
    covenants.covenantTypeMinAssetCoverage(1.5),
    covenants.covenantTypeNegative('no additional senior debt'),
    covenants.covenantTypeAffirmative('deliver audited financials within 90 days'),
    covenants.covenantTypeCustom('net_working_capital', 'minimum', 25.0),
    covenants.covenantTypeCustom('capex_to_sales', 'maximum', 0.08),
    covenants.covenantTypeBasket('restricted_payments', 5_000_000),
    covenants.covenantTypeMinDscr(1.25),
    covenants.covenantTypeMaxNetDebtToEbitda(3.5),
    covenants.covenantTypeMaxCapex(10_000_000),
    covenants.covenantTypeMinLiquidity(2_500_000),
  ],
  consequences: () => [
    covenants.covenantConsequenceDefault(),
    covenants.covenantConsequenceRateIncrease(200),
    covenants.covenantConsequenceCashSweep(0.5),
    covenants.covenantConsequenceBlockDistributions(),
    covenants.covenantConsequenceRequireCollateral('first lien on receivables'),
    covenants.covenantConsequenceAccelerateMaturity('2027-12-31'),
  ],
  covenant_modifiers: () => {
    const base = leverage();
    return [
      base,
      covenants.covenantWithCurePeriod(base, 30),
      covenants.covenantWithCurePeriod(covenants.covenantWithCurePeriod(base, 30)),
      covenants.covenantWithConsequence(base, covenants.covenantConsequenceRateIncrease(200)),
      covenants.covenantWithScope(base, 'incurrence'),
      covenants.covenantWithSpringingCondition(base, {
        metric_id: 'revolver_utilization',
        test: { minimum: 0.35 },
      }),
    ];
  },
  spec_modifiers: () => {
    const netLeverage = covenants.covenantSpecWithMetric(
      covenants.covenantNew(
        covenants.covenantTypeMaxNetDebtToEbitda(3.5),
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
      with_metric: [leverageSpec(), netLeverage],
      with_denominator_metric: covenants.covenantSpecWithDenominatorMetric(
        netLeverage,
        'adjusted_ebitda'
      ),
      with_threshold_schedule: covenants.covenantSpecWithThresholdSchedule(
        leverageSpec(),
        schedule
      ),
      threshold_for: ['2025-12-31', '2026-01-01', '2026-06-30', '2026-07-01'].map(
        (date) => covenants.thresholdScheduleThresholdFor(schedule, date) ?? null
      ),
    };
  },
  templates: () => [
    covenants.lboStandard(5.0, 1.5, 1.2, 10_000_000),
    covenants.covLite(7.0, 4.5),
    covenants.realEstate(1.25, 0.08, 0.65),
    covenants.projectFinance(1.3, 1.15, 5_000_000, 4.0),
  ],
  engine_evaluate: () =>
    withEngine(covenants.covLite(7.0, 4.5), (engine) => ({
      pass: engine.evaluate({ total_leverage: 5.0, senior_leverage: 3.0 }, '2026-03-31'),
      breach: engine.evaluate(
        JSON.stringify({ total_leverage: 7.5, senior_leverage: 3.0 }),
        '2026-03-31'
      ),
      specs: engine.specs,
      waivers: engine.waivers,
      breach_history: engine.breachHistory,
    })),
  engine_track_waive_and_round_trip: () => {
    const engine = new covenants.CovenantEngine();
    try {
      engine.addSpec(
        covenants.covenantSpecWithMetric(
          covenants.covenantWithCurePeriod(leverage(), 30),
          'debt_to_ebitda'
        )
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
    with_scope: covenants.covenantForecastConfigWithScope(
      { stochastic: true, volatility: 0.2 },
      'incurrence'
    ),
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

test('typed templates and engine agree with the JSON bridge', () => {
  assert.deepEqual(covenants.covLite(7.0, 4.5), JSON.parse(covenants.covLiteJson(7.0, 4.5)));
  assert.deepEqual(
    covenants.lboStandard(5, 1.5, 1.2, 1e7),
    JSON.parse(covenants.lboStandardJson(5, 1.5, 1.2, 1e7))
  );
  assert.deepEqual(
    covenants.realEstate(1.25, 0.08, 0.65),
    JSON.parse(covenants.realEstateJson(1.25, 0.08, 0.65))
  );
  assert.deepEqual(
    covenants.projectFinance(1.3, 1.15, 5e6, 4),
    JSON.parse(covenants.projectFinanceJson(1.3, 1.15, 5e6, 4))
  );
  withEngine(covenants.covLite(7.0, 4.5), (engine) => {
    const metrics = { total_leverage: 5.0, senior_leverage: 3.0 };
    assert.deepEqual(
      engine.evaluate(metrics, '2026-03-31'),
      covenants.evaluateEngine(engine.toJson(), metrics, '2026-03-31')
    );
    assert.equal(engine.addSpec(leverageSpec()), undefined);
    assert.equal(engine.specs.length, covenants.covLite(7.0, 4.5).length + 1);
  });
});

test('covenant errors are structured and Rust-owned', () => {
  assert.throws(() => covenants.covLite(Number.NaN, 4.5), kind('validation', /finite|negative/i));
  assert.throws(() => covenants.covLite('7', 4.5), kind('invalid_type', /^maxLeverage: /));
  assert.throws(
    () => covenants.covenantTypeCustom('m', 'between', 1),
    kind('validation', /test: unknown variant `between`/)
  );
  assert.throws(
    () => covenants.covenantWithScope(leverage(), 'sometimes'),
    kind('validation', /scope: unknown variant/)
  );
  assert.throws(
    () => covenants.covenantWithCurePeriod(leverage(), 1.5),
    kind('invalid_type', /^days: /)
  );
  assert.throws(
    () =>
      covenants.covenantSpecWithThresholdSchedule(leverageSpec(), [
        ['2026-01-01', 4.5],
        ['2026-01-01', 4.0],
      ]),
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
    () => covenants.forecastCovenant(leverageSpec(), series, { stochastic: true }),
    kind('validation', /volatility/)
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
