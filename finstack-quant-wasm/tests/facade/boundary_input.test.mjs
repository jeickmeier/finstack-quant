/**
 * Arguments are converted strictly at the WASM boundary: a wrong type throws a
 * `TypeError` with `kind: "invalid_type"` (never a trap, a coercion or a
 * leak), and structured inputs accept a JSON string or the equivalent plain
 * object, with the Rust types' unknown-field checks applied either way.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, {
  core,
  models,
  portfolio,
  scenarios,
  statements,
  statements_analytics,
  valuations,
} from '../../index.js';

const wasm = await init({
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
const AS_OF = golden.metadata.valuation_date;
const MODEL = golden.model;
const { priceInstrument } = valuations.instruments;

/** A valuation without its wall-clock `meta.timestamp`. */
function stable(result) {
  return { ...result, meta: { ...result.meta, timestamp: null } };
}

function invalidType(label) {
  return (error) => {
    assert.ok(error instanceof TypeError, `expected TypeError, got ${error}`);
    assert.equal(error.kind, 'invalid_type');
    assert.ok(error.message.startsWith(`${label}: `), error.message);
    return true;
  };
}

test('non-string scalar arguments throw TypeError instead of trapping', () => {
  assert.throws(() => new core.Currency({}), invalidType('code'));
  assert.throws(() => new core.Currency(840), invalidType('code'));
  assert.equal(new core.Currency('EUR').code, 'EUR');
});

test('epoch-day arguments reject ISO strings, fractions and NaN', () => {
  const dc = core.DayCount.act360();
  const start = core.createDate(2025, 1, 15);
  const end = core.createDate(2025, 4, 15);
  assert.equal(dc.yearFraction(start, end), 0.25);
  assert.throws(() => dc.yearFraction('2025-01-15', end), invalidType('startEpochDays'));
  assert.throws(() => dc.yearFraction(start, end + 0.5), invalidType('endEpochDays'));
  assert.throws(() => dc.yearFraction(Number.NaN, end), invalidType('startEpochDays'));
  assert.throws(() => dc.yearFraction(true, end), invalidType('startEpochDays'));
});

test('structured inputs accept a JSON string or the same plain object', () => {
  const fromText = priceInstrument(
    JSON.stringify(golden.instrument),
    JSON.stringify(golden.market.data),
    AS_OF,
    MODEL
  );
  const fromObjects = priceInstrument(
    structuredClone(golden.instrument),
    structuredClone(golden.market.data),
    AS_OF,
    MODEL
  );
  assert.deepEqual(stable(fromObjects), stable(fromText));
  assert.ok(Math.abs(fromText.value.amount - golden.expected.npv) < 1e-6);
});

test('plain-object inputs keep the Rust unknown-field checks', () => {
  const instrument = structuredClone(golden.instrument);
  instrument.instrument.spec.not_a_field = 1;
  assert.throws(
    () => priceInstrument(instrument, golden.market.data, AS_OF, MODEL),
    (error) => error.kind === 'validation' && /not_a_field/.test(error.message)
  );
});

test('structured inputs reject values JSON cannot represent', () => {
  const nan = structuredClone(golden.instrument);
  nan.instrument.spec.strike = Number.NaN;
  assert.throws(
    () => priceInstrument(nan, golden.market.data, AS_OF, MODEL),
    invalidType('instrumentJson')
  );
  const handle = new core.Currency('USD');
  assert.throws(
    () => priceInstrument(handle, golden.market.data, AS_OF, MODEL),
    invalidType('instrumentJson')
  );
  const nested = structuredClone(golden.instrument);
  nested.instrument.spec.notional.currency = handle;
  assert.throws(
    () => priceInstrument(nested, golden.market.data, AS_OF, MODEL),
    invalidType('instrumentJson')
  );
  assert.throws(
    () => priceInstrument(golden.instrument, golden.market.data, AS_OF, MODEL, ['npv', 7]),
    invalidType('metrics[1]')
  );
  assert.throws(
    () => priceInstrument(42, golden.market.data, AS_OF),
    invalidType('instrumentJson')
  );
  assert.throws(
    () => priceInstrument(golden.instrument, golden.market.data, 20260430),
    invalidType('asOf')
  );
});

test('typed arrays and BigInt become exact JSON numbers', () => {
  const market = structuredClone(golden.market.data);
  const surface = market.surfaces.find((s) => Array.isArray(s.expiries));
  assert.ok(surface, 'golden market carries a gridded vol surface');
  const plain = priceInstrument(golden.instrument, market, AS_OF, MODEL);
  surface.expiries = Float64Array.from(surface.expiries);
  const typed = priceInstrument(golden.instrument, market, AS_OF, MODEL);
  assert.deepEqual(stable(typed), stable(plain));

  const bigintMarket = structuredClone(golden.market.data);
  bigintMarket.fx.config.cache_capacity = 256n;
  assert.deepEqual(
    stable(priceInstrument(golden.instrument, bigintMarket, AS_OF, MODEL)),
    stable(plain)
  );
});

test('rejected arguments do not grow linear memory or poison the instance', () => {
  const run = () => {
    for (let i = 0; i < 2000; i += 1) {
      assert.throws(() => new core.Currency({}));
      assert.throws(() => priceInstrument(i, '{}', AS_OF));
    }
  };
  run();
  const before = wasm.memory.buffer.byteLength;
  run();
  assert.equal(wasm.memory.buffer.byteLength, before);
  assert.equal(new core.Currency('USD').code, 'USD');
});

test('narrow integers reject out-of-range values instead of wrapping', () => {
  assert.throws(() => core.createDate(2025, 257, 1), invalidType('month'));
  assert.throws(() => core.createDate(2025, 1.9, 15), invalidType('month'));
  assert.throws(() => core.createDate('2025', 1, 1), invalidType('year'));
  assert.throws(() => new core.DayCountContext().withBusBasis(65_788), invalidType('busBasis'));
  assert.deepEqual(Array.from(core.dateFromEpochDays(core.createDate(2025, 1, 15))), [2025, 1, 15]);
});

test('booleans are not coerced by truthiness', () => {
  const args = [100, 100, 0.05, 0, 0.2, 1];
  assert.ok(models.bsPrice(...args, true) > models.bsPrice(...args, false));
  assert.throws(() => models.bsPrice(...args), invalidType('isCall'));
  assert.throws(() => models.bsPrice(...args, 'true'), invalidType('isCall'));
  assert.throws(() => models.bsPrice(...args, 1), invalidType('isCall'));
});

test('counts reject negatives, fractions and oversized values before Rust sizes anything', () => {
  const args = [100, 100, 0.05, 0, 0.2, 1, true];
  assert.ok(models.bsCosPrice(...args, 256) > 0);
  assert.throws(() => models.bsCosPrice(...args, -1), invalidType('nTerms'));
  assert.throws(() => models.bsCosPrice(...args, 2.5), invalidType('nTerms'));
  assert.throws(() => models.bsCosPrice(...args, 1_000_000), /num_terms must be in 1\.\.=65536/);
  const asian = [100, 100, 0.05, 0, 0.2, 1];
  assert.throws(() => models.asianOptionPrice(...asian, -1), invalidType('numFixings'));
  assert.throws(() => models.asianOptionPrice(...asian, 0), /num_fixings must be positive/);
});

test('u64 seeds accept exact BigInt or safe integers and reject wrapping', () => {
  const model = models.credit.mertonModelJson(100, 0.2, 80, 0.05);
  const paths = (seed) => models.credit.mertonSimulatePathsJson(model, 2, 2, 1, seed, false);
  assert.equal(paths(7n), paths(7));
  assert.throws(() => paths(-1n), invalidType('seed'));
  assert.throws(() => paths(2n ** 64n + 7n), invalidType('seed'));
  assert.throws(() => paths('7'), invalidType('seed'));
  assert.throws(
    () => models.credit.mertonSimulatePathsJson(model, 2, 2, 1, 7n, 'no'),
    invalidType('antithetic')
  );
});

test('optional integers reject fractions (buildScenarioSpec priority)', () => {
  assert.equal(scenarios.buildScenarioSpec('x', [], undefined, undefined, 3).priority, 3);
  assert.throws(
    () => scenarios.buildScenarioSpec('x', [], undefined, undefined, 1.5),
    invalidType('priority')
  );
  assert.throws(
    () => scenarios.buildScenarioSpec('x', [], undefined, undefined, 2 ** 31),
    invalidType('priority')
  );
});

test('string arrays must be arrays of strings (portfolio metrics)', () => {
  const spec = {
    id: 'empty',
    name: 'Empty',
    base_currency: 'USD',
    as_of: AS_OF,
    entities: {},
    positions: [],
  };
  assert.equal(typeof portfolio.valuePortfolio(spec, golden.market.data, false, []), 'object');
  for (const bad of ['dv01', 5, {}, new Set(['theta'])]) {
    assert.throws(
      () => portfolio.valuePortfolio(spec, golden.market.data, false, bad),
      invalidType('metrics')
    );
  }
});

test('Rust types reject unknown keys and non-finite numbers in object inputs', () => {
  const options = { id: 'F', tenor: 0.25, baseDate: '2025-01-01', knots: [0, 0.03, 1, 0.035] };
  assert.equal(new core.ForwardCurve(options).id, 'F');
  assert.equal(
    new core.ForwardCurve({ ...options, knots: Float64Array.from(options.knots) }).id,
    'F'
  );
  assert.throws(
    () => new core.ForwardCurve({ ...options, dayCont: 'act_360' }),
    (error) => error.kind === 'validation' && /dayCont/.test(error.message)
  );
  assert.throws(
    () => new valuations.fx.FxForward({ contract_rate: Number.NaN }),
    invalidType('spec')
  );
});

test('producer results feed consumers directly as objects', () => {
  const model = {
    id: 'm',
    periods: [{ id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: false }],
    nodes: {
      revenue: { node_id: 'revenue', node_type: 'value', values: { '2025Q1': 100 } },
    },
    schema_version: 1,
  };
  const results = statements.evaluateModel(model);
  assert.deepEqual(
    statements_analytics.creditAssessment(results, '2025Q1'),
    statements_analytics.creditAssessment(JSON.stringify(results), '2025Q1')
  );
});

test('typed model parameters keep deny_unknown_fields (sviImpliedVol)', () => {
  const params = { a: 0.02, b: 0.1, rho: -0.3, m: 0.0, sigma: 0.2 };
  assert.ok(models.volatility.sviImpliedVol(params, 0, 1) > 0);
  assert.throws(
    () => models.volatility.sviImpliedVol({ ...params, sigmma: 0.2 }, 0, 1),
    (error) => error.kind === 'validation' && /sigmma/.test(error.message)
  );
});

test('capacities reject negative and fractional values instead of wrapping', () => {
  new portfolio.InstrumentArtifactCache(8).free();
  assert.throws(() => new portfolio.InstrumentArtifactCache(-1), invalidType('capacity'));
  assert.throws(() => new portfolio.InstrumentArtifactCache(1.5), invalidType('capacity'));
});
