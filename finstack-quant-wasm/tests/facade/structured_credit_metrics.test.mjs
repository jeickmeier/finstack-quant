import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function fixture() {
  const f = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_structured_credit.json',
        import.meta.url
      )
    )
  );
  const spec = f.instrument.instrument.spec;
  spec.prepayment_spec = { cpr: 0, curve: 'constant' };
  spec.default_spec = { cdr: 0, curve: 'constant' };
  return f;
}

for (const cdr of [0, 0.9]) {
  test(`seasoned accrued uses opening face before ${cdr} projected defaults`, () => {
    const f = fixture();
    f.instrument.instrument.spec.default_spec.cdr = cdr;
    const result = valuations.instruments.priceInstrument(
      JSON.stringify(f.instrument),
      JSON.stringify(f.market),
      '2024-02-15',
      'discounting',
      ['accrued']
    );
    assert.ok(Math.abs(result.measures.accrued - 750_000) < 1e-6);
  });
}

test('effective CPR/CDR use the configured model', () => {
  const f = fixture();
  const spec = f.instrument.instrument.spec;
  spec.prepayment_spec.cpr = 0.36;
  spec.default_spec.cdr = 0.07;
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(f.instrument),
    JSON.stringify(f.market),
    f.as_of,
    'discounting',
    ['cpr', 'cdr']
  );
  assert.ok(Math.abs(result.measures.cpr - 0.36) < 1e-12);
  assert.ok(Math.abs(result.measures.cdr - 0.07) < 1e-12);
});

test('recovery sensitivity bumps the credit-model recovery', () => {
  const f = fixture();
  const spec = f.instrument.instrument.spec;
  spec.default_spec.cdr = 0.03;
  spec.recovery_spec.rate = 0.55;
  const recovery01 = valuations.instruments.priceInstrument(
    JSON.stringify(f.instrument),
    JSON.stringify(f.market),
    f.as_of,
    'discounting',
    ['recovery01']
  ).measures.recovery01;
  assert.ok(Math.abs(recovery01) > 1);
});

test('a deal call adds the to-call twins next to the to-maturity metrics', () => {
  const f = fixture();
  const spec = f.instrument.instrument.spec;
  const asOf = '2024-02-15';
  const id = spec.tranches.tranches[0].id;
  const maturityDeal = valuations.instruments.StructuredCredit.fromJson(
    JSON.stringify(f.instrument)
  );
  let toMaturity;
  try {
    toMaturity = valuations.instruments.trancheMetrics(
      maturityDeal,
      id,
      JSON.stringify(f.market),
      asOf
    );
  } finally {
    maturityDeal.free();
  }
  assert.equal(toMaturity.wal_to_call, undefined);
  assert.equal(toMaturity.z_spread_to_call_bp, undefined);

  spec.call_assumption = { date: '2026-02-15', price_pct: 100.0, scope: 'deal' };
  const callDeal = valuations.instruments.StructuredCredit.fromJson(JSON.stringify(f.instrument));
  try {
    const toCall = valuations.instruments.trancheMetrics(
      callDeal,
      id,
      JSON.stringify(f.market),
      asOf
    );
    // The to-maturity figures are projected without the call and stay put.
    assert.ok(Math.abs(toCall.wal - toMaturity.wal) < 1e-9);
    assert.equal(typeof toCall.wal_to_call, 'number');
    assert.ok(toCall.wal_to_call < toCall.wal);
    assert.equal(typeof toCall.z_spread_to_call_bp, 'number');
    assert.ok(Number.isFinite(toCall.z_spread_to_call_bp));
  } finally {
    callDeal.free();
  }
});

test('clean and dirty targets share coupon-crossing settlement', () => {
  const f = fixture();
  const spec = f.instrument.instrument.spec;
  spec.quote_settlement_date = '2024-05-15';
  const asOf = '2024-02-15';
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(f.instrument),
    JSON.stringify(f.market),
    asOf,
    'discounting',
    ['accrued', 'clean_price', 'dirty_price']
  );
  const accrued = (100_000_000 * 0.06 * 44) / 360;
  assert.ok(Math.abs(result.measures.accrued - accrued) < 1e-6);
  const clean = result.measures.clean_price;
  const dirty = result.measures.dirty_price * 1_000_000;
  assert.ok(Math.abs(clean * 1_000_000 + accrued - dirty) < 1e-6);
  const id = spec.tranches.tranches[0].id;
  const deal = valuations.instruments.StructuredCredit.fromJson(JSON.stringify(f.instrument));
  try {
    const metrics = valuations.instruments.trancheMetrics(
      deal,
      id,
      JSON.stringify(f.market),
      asOf,
      clean
    );
    assert.ok(Math.abs(metrics.z_spread_bp) < 1e-6);
    assert.ok(Math.abs(metrics.spread_duration + metrics.cs01 / (dirty * 1e-4)) < 1e-10);
    assert.ok(Math.abs(metrics.accrued - accrued) < 1e-6);
    assert.equal(typeof metrics.ytm, 'number');
    assert.equal(metrics.metrics, undefined);
    const withRegistry = valuations.instruments.trancheMetrics(
      deal,
      id,
      JSON.stringify(f.market),
      asOf,
      clean,
      ['dv01']
    );
    assert.deepEqual(Object.keys(withRegistry.metrics), ['dv01']);
    assert.ok(Number.isFinite(withRegistry.metrics.dv01));
    const config = {
      num_paths: 1,
      stochastic_rates: false,
      stochastic_credit: false,
      hw1f_mean_reversion: 0.05,
      hw1f_sigma: 0.01,
      prepay_beta: 7,
      credit_loading: 0.3,
      seed: 42,
      tolerance: 1e-10,
    };
    const oas = valuations.instruments.trancheOas(
      deal,
      id,
      JSON.stringify(f.market),
      asOf,
      clean,
      JSON.stringify(config)
    );
    assert.ok(Math.abs(oas.oas) < 1e-8);
    assert.ok(Math.abs(oas.model_price - clean) < 1e-8);
  } finally {
    deal.free();
  }
  for (const quote of [{ quoted_clean_price_pct: clean }, { quoted_dirty_price_currency: dirty }]) {
    spec.instrument_pricing_overrides = { market_quotes: quote };
    const priced = valuations.instruments.priceInstrument(
      JSON.stringify(f.instrument),
      JSON.stringify(f.market),
      asOf,
      'discounting',
      ['z_spread', 'cs01', 'spread_duration']
    );
    assert.ok(Math.abs(priced.measures.z_spread) < 1e-8);
    assert.ok(
      Math.abs(priced.measures.spread_duration + priced.measures.cs01 / (dirty * 1e-4)) < 1e-10
    );
  }
});

test('structured credit errors preserve typed classification', () => {
  const f = fixture();
  const tranche = f.instrument.instrument.spec.tranches.tranches[0].id;
  const grid = JSON.stringify({ cprs: [0], cdrs: [0], severities: [0.4] });
  const check = (kind) => (error) => {
    assert.equal(error.name, 'FinstackError');
    assert.equal(error.kind, kind);
    assert.equal(typeof error.message, 'string');
    return true;
  };
  assert.throws(() => valuations.instruments.StructuredCredit.fromJson('{'), check('validation'));
  const deal = valuations.instruments.StructuredCredit.fromJson(JSON.stringify(f.instrument));
  try {
    assert.throws(
      () =>
        valuations.instruments.trancheScenarioTable(
          deal,
          tranche,
          JSON.stringify({ ...f.market, curves: [] }),
          f.as_of,
          grid
        ),
      check('not_found')
    );
  } finally {
    deal.free();
  }
});

test('tranche analytics borrow a reusable deal across successful and failed calls', () => {
  const f = fixture();
  const tranche = f.instrument.instrument.spec.tranches.tranches[0].id;
  const market = JSON.stringify(f.market);
  const grid = JSON.stringify({ cprs: [0], cdrs: [0], severities: [0.4] });
  const deal = valuations.instruments.StructuredCredit.fromJson(JSON.stringify(f.instrument));
  try {
    const id = deal.id;
    const before = deal.toJson();
    const first = valuations.instruments.trancheScenarioTable(deal, tranche, market, f.as_of, grid);
    assert.equal(first.cells.length, 1);
    assert.equal(deal.id, id);
    assert.equal(deal.toJson(), before);

    assert.throws(
      () => valuations.instruments.trancheScenarioTable(deal, 'missing', market, f.as_of, grid),
      (error) => {
        assert.equal(error.name, 'FinstackError');
        assert.equal(error.kind, 'not_found');
        return true;
      }
    );
    assert.equal(deal.id, id);
    assert.equal(deal.toJson(), before);
    assert.deepEqual(
      valuations.instruments.trancheScenarioTable(deal, tranche, market, f.as_of, grid),
      first
    );
  } finally {
    deal.free();
  }
});
