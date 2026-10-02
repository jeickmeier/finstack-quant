/**
 * Every payload that embeds a `ValuationResult` keeps its 64-bit fields (the
 * Monte Carlo `seed` and path counts) as `bigint`, exactly as `priceInstrument`
 * returns them, while the counts around it stay plain numbers.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { portfolio, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const golden = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/fx_barrier_option/eurusd_up_out_call_3m_quantlib.json',
      import.meta.url
    )
  )
);
const AS_OF = '2026-04-30';
const MARKET = JSON.stringify(golden.market.data);

/** Discretely monitored barrier: its default model is Monte Carlo GBM. */
function discreteBarrier() {
  const envelope = structuredClone(golden.instrument);
  Object.assign(envelope.instrument.spec, {
    monitoring: {
      type: 'discrete',
      observation_dates: ['2026-05-29', '2026-06-30', '2026-07-30'],
    },
    monitoring_start_date: '2026-05-29',
    instrument_pricing_overrides: { model_config: { mc_paths: 256 } },
  });
  return envelope;
}

function portfolioSpec() {
  const envelope = discreteBarrier();
  return JSON.stringify({
    id: 'mc_book',
    name: 'MC book',
    base_currency: 'USD',
    as_of: AS_OF,
    entities: { E: { id: 'E', name: null, meta: {} } },
    positions: [
      {
        position_id: 'BARRIER',
        entity_id: 'E',
        instrument_id: envelope.instrument.spec.id,
        instrument_spec: envelope.instrument,
        quantity: 1,
        unit: 'units',
      },
    ],
  });
}

/** JSON paths of every `bigint` in a structured result. */
function bigintPaths(value, path = '$', out = []) {
  if (typeof value === 'bigint') {
    out.push(path);
  } else if (Array.isArray(value)) {
    value.forEach((item, i) => bigintPaths(item, `${path}[${i}]`, out));
  } else if (value && typeof value === 'object') {
    for (const [key, item] of Object.entries(value)) bigintPaths(item, `${path}.${key}`, out);
  }
  return out;
}

function assertBigintsOnlyInValuations(result, label) {
  const paths = bigintPaths(result);
  assert.ok(
    paths.some((p) => p.endsWith('.valuation_result.details.data.seed')),
    `${label}: expected a bigint Monte Carlo seed, got ${JSON.stringify(paths)}`
  );
  const stray = paths.filter((p) => !p.includes('.valuation_result.'));
  assert.deepEqual(stray, [], `${label}: counts outside valuation results must be numbers`);
}

test('typed FX price returns the same lossless valuation as priceInstrument', () => {
  const json = JSON.stringify(discreteBarrier());
  const generic = valuations.instruments.priceInstrument(
    json,
    MARKET,
    AS_OF,
    'monte_carlo_gbm',
    []
  );
  const typed = valuations.fx.FxBarrierOption.fromJson(json).price(
    MARKET,
    AS_OF,
    'monte_carlo_gbm'
  );
  assert.equal(generic.details.type, 'monte_carlo');
  assert.equal(typeof typed.details.data.seed, 'bigint');
  assert.deepEqual(typed.value, generic.value);
  assert.deepEqual(typed.details, generic.details);
});

test('valuationResultToJson writes the canonical JSON with an exact seed', () => {
  const json = JSON.stringify(discreteBarrier());
  const result = valuations.instruments.priceInstrument(json, MARKET, AS_OF, 'monte_carlo_gbm', []);
  const text = valuations.valuationResultToJson(result);
  assert.ok(text.includes(`"seed":${result.details.data.seed}`), 'seed digits are exact');
  assert.equal(valuations.validateValuationResultJson(text), text);
  assert.throws(() => JSON.stringify(result), TypeError);
});

test('portfolio valuation keeps Monte Carlo seeds as bigint', () => {
  const valuation = portfolio.valuePortfolio(portfolioSpec(), MARKET, false, []);
  assertBigintsOnlyInValuations(valuation, 'valuePortfolio');
});

test('scenario revaluation keeps seeds as bigint and report counts as numbers', () => {
  const out = portfolio.applyScenarioAndRevalue(
    portfolioSpec(),
    JSON.stringify({ id: 's', operations: [] }),
    MARKET
  );
  assertBigintsOnlyInValuations(out, 'applyScenarioAndRevalue');
  assert.equal(typeof out.report.operations_applied, 'number');
});

test('replay keeps seeds as bigint and the step count as a number', () => {
  const market = JSON.parse(MARKET);
  const snapshots = JSON.stringify([
    { date: AS_OF, market },
    { date: '2026-05-01', market },
  ]);
  const config = JSON.stringify({ mode: 'pv_only', attribution_method: 'parallel' });
  const replay = portfolio.replayPortfolio(portfolioSpec(), snapshots, config);
  assertBigintsOnlyInValuations(replay, 'replayPortfolio');
  assert.equal(replay.summary.num_steps, 2);
});
