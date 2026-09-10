import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { attribution } from '../../index.js';

await init({ module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)) });
const input = JSON.parse(readFileSync(new URL('../../../finstack-quant/valuations/tests/fixtures/production_convertible.json', import.meta.url)));

test('Taylor and metrics attribution count risky discount moves only as credit', () => {
  const fixture = JSON.parse(readFileSync(new URL('../../../finstack-quant/attribution/tests/fixtures/production_convertible_credit.json', import.meta.url)));
  for (const method of ['metrics_based', { taylor: {} }]) {
    const params = new attribution.AttributionParams(JSON.stringify(fixture.instrument), JSON.stringify(fixture.market_t0), JSON.stringify(fixture.market_t1), fixture.as_of_t0, fixture.as_of_t1, JSON.stringify(method), undefined, false);
    const result = attribution.attributePnl(params);
    assert.ok(Number(result.rates_curves_pnl.amount) === 0);
    assert.ok(Number(result.credit_curves_pnl.amount) < 0);
    assert.ok(Math.abs(Number(result.credit_curves_pnl.amount)) > 5 * Math.abs(Number(result.residual.amount)));
  }
});

test('metrics attribution reports gross carry and a separate funding overlay', () => {
  const fixture = JSON.parse(readFileSync(new URL('../../../finstack-quant/attribution/tests/fixtures/production_gross_carry.json', import.meta.url)));
  const bond = fixture.instrument;
  const market = fixture.market;
  const config = { metrics: ['theta', 'carry_total', 'coupon_income', 'pull_to_par', 'roll_down', 'funding_cost'], tolerance_abs: 0.01, tolerance_pct: 0.001 };
  const params = new attribution.AttributionParams(JSON.stringify(bond), JSON.stringify(market), JSON.stringify(market), '2025-06-17', '2025-06-18', '"metrics_based"', JSON.stringify(config), false);
  const result = attribution.attributePnl(params);
  const amount = (m) => Number(m.amount);
  const detail = result.carry_detail;
  assert.ok(amount(detail.funding_cost) > 0);
  assert.ok(Math.abs(amount(result.carry) - amount(result.total_pnl)) < 0.01);
  assert.ok(Math.abs(amount(result.residual)) < 0.01);
  assert.ok(Math.abs(amount(detail.total) - amount(detail.coupon_income.total) - amount(detail.pull_to_par) - amount(detail.roll_down.total)) < 0.01);
});

test('scalar volatility moves stay out of generic scalar attribution', () => {
  const instrument = structuredClone(input.instrument);
  instrument.instrument.spec.call_put = null;
  instrument.instrument.spec.conversion.ratio = 10;
  const opening = structuredClone(input.market);
  opening.prices.AAPL = { unitless: 80 };
  opening.prices['AAPL-VOL'] = { unitless: 0.25 };
  const closing = structuredClone(opening);
  closing.prices['AAPL-VOL'] = { unitless: 0.26 };
  for (const method of ['parallel', { waterfall: attribution.defaultWaterfallOrder() }]) {
    const params = new attribution.AttributionParams(JSON.stringify(instrument), JSON.stringify(opening), JSON.stringify(closing), input.as_of, input.as_of, JSON.stringify(method), undefined, false);
    const result = attribution.attributePnl(params);
    assert.ok(Number(result.total_pnl.amount) > 0.01);
    assert.ok(Math.abs(Number(result.vol_pnl.amount) - Number(result.total_pnl.amount)) < 0.01);
    assert.ok(Math.abs(Number(result.market_scalars_pnl.amount)) < 0.01);
  }
});
