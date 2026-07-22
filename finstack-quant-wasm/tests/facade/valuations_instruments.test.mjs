/**
 * Typed Bond / TermLoan facade smoke tests for `valuations.instruments`.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = join(__dirname, '..', '..', 'pkg');
const WASM_BG = join(PKG_DIR, 'finstack_quant_wasm_bg.wasm');

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

const facade = await import('../../index.js');
const init = facade.default;
const { core, valuations } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

test('instruments namespace exposes typed Bond and TermLoan classes', () => {
  assert.equal(typeof valuations.instruments.Bond, 'function');
  assert.equal(typeof valuations.instruments.TermLoan, 'function');
});

test('Bond.fixed constructs, round-trips through tagged JSON', () => {
  const usd = new core.Currency('USD');
  const bond = valuations.instruments.Bond.fixed(
    'BOND-1',
    new core.Money(1_000_000, usd),
    new core.Rate(0.05),
    '2024-01-01',
    '2034-01-01',
    'USD-OIS'
  );
  assert.equal(bond.id, 'BOND-1');
  const json = bond.toJson();
  const payload = JSON.parse(json);
  assert.equal(payload.type, 'bond');
  assert.equal(payload.spec.id, 'BOND-1');
  const roundTripped = valuations.instruments.Bond.fromJson(json);
  assert.equal(roundTripped.toJson(), json);
});

test('Bond.fromJson rejects malformed JSON and wrong instrument types', () => {
  assert.throws(() => valuations.instruments.Bond.fromJson('{not valid json'));
  const loanJson = valuations.instruments.TermLoan.example().toJson();
  assert.throws(() => valuations.instruments.Bond.fromJson(loanJson));
});

test('TermLoan.example round-trips through tagged JSON', () => {
  const loan = valuations.instruments.TermLoan.example();
  assert.equal(loan.id, 'TERM-LOAN-USD-5Y');
  const json = loan.toJson();
  const payload = JSON.parse(json);
  assert.equal(payload.type, 'term_loan');
  assert.equal(valuations.instruments.TermLoan.fromJson(json).toJson(), json);
});
