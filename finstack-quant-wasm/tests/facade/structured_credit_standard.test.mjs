import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function standardParams() {
  const example = valuations.instruments.StructuredCredit.example();
  const { id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id } =
    example.toDict();
  example.free();
  return { id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id };
}

for (const family of ['abs', 'clo', 'cmbs', 'rmbs']) {
  test(`newStandard selects the ${family} profile`, () => {
    const params = standardParams();
    params.pool.deal_type = family;
    const deal = valuations.instruments.StructuredCredit.newStandard(family, params);
    assert.equal(deal.toDict().deal_type, family);
    assert.equal(deal.toDict().calendar_id, 'nyse');
    deal.free();
  });
}

test('newStandard accepts a null construction calendar and rejects unsupported profiles', () => {
  const params = standardParams();
  params.calendar_id = null;
  const deal = valuations.instruments.StructuredCredit.newStandard('clo', params);
  assert.equal(Object.hasOwn(deal.toDict(), 'calendar_id'), false);
  deal.free();
  for (const family of ['cbo', 'auto', 'card']) {
    assert.throws(() => valuations.instruments.StructuredCredit.newStandard(family, params));
  }
});
