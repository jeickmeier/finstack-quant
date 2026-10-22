import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});
const fixtures = {
  'beta_result.schema.json': {
    beta: 'nan',
    std_err: 'nan',
    ci_lower: 'nan',
    ci_upper: 'nan',
    n_obs: 2,
    confidence_level: 0.95,
  },
  'greeks_result.schema.json': {
    alpha: 'nan',
    beta: 'nan',
    r_squared: 'nan',
    adjusted_r_squared: 'nan',
    n_obs: 1,
  },
  'multi_factor_result.schema.json': {
    alpha: 0.0,
    betas: [1.0],
    r_squared: 'nan',
    adjusted_r_squared: 'nan',
    residual_vol: 0.0,
    n_obs: 4,
  },
  'drawdown_episode.schema.json': {
    start: '2025-01-01',
    valley: '2025-01-02',
    end: null,
    duration_days: 2,
    max_drawdown: -0.1,
    near_recovery_threshold: -0.001,
    truncated_at_start: true,
  },
  'period_stats.schema.json': {
    best: 0.01,
    worst: -0.01,
    consecutive_wins: 1,
    consecutive_losses: 1,
    win_rate: 0.5,
    avg_return: 0.0,
    avg_win: 0.01,
    avg_loss: -0.01,
    payoff_ratio: 1.0,
    profit_factor: 1.0,
    cpc_ratio: 0.5,
    kelly_criterion: 0.0,
  },
  'rolling_greeks.schema.json': { dates: ['2025-01-02'], alphas: [0.0], betas: [1.0] },
  'lookback_returns.schema.json': {
    ticker_names: ['FUND'],
    mtd: [0.1],
    qtd: [0.1],
    ytd: [0.1],
    fytd: [0.1],
  },
};
const required = {
  'beta_result.schema.json': ['n_obs', 'confidence_level'],
  'greeks_result.schema.json': ['n_obs'],
  'multi_factor_result.schema.json': ['n_obs'],
  'drawdown_episode.schema.json': ['truncated_at_start'],
};

const contract = (selector) =>
  JSON.parse(
    readFileSync(
      new URL(`../../../finstack-quant/analytics/schemas/analytics/1/${selector}`, import.meta.url),
      'utf8'
    )
  );

for (const [selector, value] of Object.entries(fixtures)) {
  test(`${selector} declares a closed result with required metadata`, () => {
    const document = contract(selector);
    assert.equal(document.additionalProperties, false);
    for (const field of Object.keys(value)) {
      assert.ok(Object.hasOwn(document.properties, field), field);
    }
    for (const field of required[selector] ?? []) {
      assert.ok(document.required.includes(field), field);
      assert.equal(Object.hasOwn(document.properties[field], 'default'), false, field);
    }
  });
}

test('optional recovery dates and numeric nonfinite host outputs are preserved', () => {
  const document = contract('drawdown_episode.schema.json');
  assert.equal(document.required.includes('end'), false);
  const perf = analytics.Performance.fromReturns(
    ['2025-01-01', '2025-01-02'],
    [[0.01, 0.02]],
    ['FUND']
  );
  const beta = perf.beta()[0];
  assert.equal(beta.n_obs, 2);
  assert.equal(beta.confidence_level, 0.95);
  assert.ok(Number.isNaN(beta.beta));
  perf.free();
});
