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
  spec.prepayment_spec = { cpr: 0, curve: null };
  spec.default_spec = { cdr: 0, curve: null };
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

test('effective CPR/CDR use the configured model and explicit overrides', () => {
  const f = fixture();
  const spec = f.instrument.instrument.spec;
  spec.prepayment_spec.cpr = 0.36;
  spec.default_spec.cdr = 0.07;
  for (const override of [false, true]) {
    if (override) Object.assign(spec.behavior_overrides, { cpr_annual: 0.18, cdr_annual: 0.03 });
    const result = valuations.instruments.priceInstrument(
      JSON.stringify(f.instrument),
      JSON.stringify(f.market),
      f.as_of,
      'discounting',
      ['cpr', 'cdr']
    );
    assert.ok(Math.abs(result.measures.cpr - (override ? 0.18 : 0.36)) < 1e-12);
    assert.ok(Math.abs(result.measures.cdr - (override ? 0.03 : 0.07)) < 1e-12);
  }
});

test('recovery sensitivity bumps the active override', () => {
  const f = fixture();
  const spec = f.instrument.instrument.spec;
  spec.default_spec.cdr = 0.03;
  spec.behavior_overrides.recovery_rate = 0.55;
  const value = () =>
    valuations.instruments.priceInstrument(
      JSON.stringify(f.instrument),
      JSON.stringify(f.market),
      f.as_of,
      'discounting',
      ['recovery_01']
    ).measures.recovery_01;
  const override = value();
  spec.behavior_overrides.recovery_rate = null;
  spec.recovery_spec.rate = 0.55;
  const canonical = value();
  assert.ok(Math.abs(override - canonical) < 1e-8);
  assert.ok(Math.abs(override) > 1);
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
  const metrics = valuations.instruments.structuredCreditTrancheMetrics(
    JSON.stringify(f.instrument),
    id,
    JSON.stringify(f.market),
    asOf,
    clean
  );
  assert.ok(Math.abs(metrics.z_spread_bp) < 1e-6);
  assert.ok(Math.abs(metrics.spread_duration + metrics.cs01 / (dirty * 1e-4)) < 1e-10);
  const config = {
    num_paths: 1,
    stochastic_rates: false,
    stochastic_credit: false,
    hw_kappa: 0.05,
    hw_sigma: 0.01,
    prepay_beta: 7,
    credit_loading: 0.3,
    seed: 42,
    tolerance: 1e-10,
  };
  const oas = valuations.instruments.structuredCreditTrancheOas(
    JSON.stringify(f.instrument),
    id,
    clean,
    JSON.stringify(f.market),
    asOf,
    JSON.stringify(config)
  );
  assert.ok(Math.abs(oas.oas) < 1e-8);
  assert.ok(Math.abs(oas.model_price - clean) < 1e-8);
  for (const quote of [{ quoted_clean_price: clean }, { quoted_dirty_price_currency: dirty }]) {
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
