/**
 * Every binding error is a structured `FinstackError` with a `kind`, never a
 * bare string. These cases used to throw string primitives.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { models, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const structured = (kind) => (error) => {
  assert.ok(error instanceof Error, `expected an Error, got ${typeof error}: ${error}`);
  assert.equal(error.name, 'FinstackError');
  assert.equal(error.kind, kind);
  return true;
};

test('listedProductCatalog rejects an unknown exchange with a validation error', () => {
  assert.throws(() => valuations.market.listedProductCatalog('mx'), structured('validation'));
});

test('typed instrument fromJson rejects a different instrument type', () => {
  const termLoan = valuations.instruments.TermLoan.example().toJson();
  assert.throws(
    () => valuations.instruments.Bond.fromJson(termLoan),
    (error) => {
      structured('validation')(error);
      assert.match(error.message, /expected instrument type "bond", found 'term_loan'/);
      return true;
    }
  );
});

test('volatility argument errors are structured', () => {
  const volatility = models.volatility;
  assert.throws(() => new volatility.SabrCalibrator().withShift('bad'), structured('validation'));
  assert.throws(() => volatility.sviImpliedVol({}, 0, 1), structured('validation'));
  assert.throws(
    () => volatility.convertAtmVolatility(0.2, 'bogus', 'normal', 0.03, 1),
    structured('validation')
  );
});
