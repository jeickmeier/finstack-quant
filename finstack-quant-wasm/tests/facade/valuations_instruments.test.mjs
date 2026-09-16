/**
 * Typed Bond / TermLoan / RevolvingCredit facade smoke tests for `valuations.instruments`.
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

test('instruments namespace exposes typed Bond, TermLoan and RevolvingCredit classes', () => {
  assert.equal(typeof valuations.instruments.Bond, 'function');
  assert.equal(typeof valuations.instruments.TermLoan, 'function');
  assert.equal(typeof valuations.instruments.RevolvingCredit, 'function');
});

test('RevolvingCredit.example round-trips through the canonical envelope', () => {
  const facility = valuations.instruments.RevolvingCredit.example();
  assert.equal(facility.id, 'RCF-USD-3Y');
  const json = facility.toJson();
  assert.equal(JSON.parse(json).instrument.type, 'revolving_credit');
  assert.equal(valuations.instruments.RevolvingCredit.fromJson(json).toJson(), json);
  assert.throws(() => valuations.instruments.RevolvingCredit.fromJson('{not json'));
});

test('Bond.fixed constructs, round-trips through the canonical envelope', () => {
  const usd = new core.Currency('USD');
  const bond = valuations.instruments.Bond.fixed(
    'BOND-1',
    new core.Money(1_000_000, usd),
    new core.Rate(0.05),
    '2024-01-01',
    '2034-01-01',
    'none',
    'USD-OIS'
  );
  assert.equal(bond.id, 'BOND-1');
  const json = bond.toJson();
  const payload = JSON.parse(json);
  assert.equal(payload.schema, 'finstack_quant.instrument/1');
  assert.equal(payload.instrument.type, 'bond');
  assert.equal(payload.instrument.spec.id, 'BOND-1');
  assert.equal(payload.instrument.spec.cashflow_spec.fixed.stub, 'none');
  const roundTripped = valuations.instruments.Bond.fromJson(json);
  assert.equal(roundTripped.toJson(), json);
  assert.throws(() => valuations.instruments.Bond.fromJson(JSON.stringify(payload.instrument)));
  assert.throws(() =>
    valuations.instruments.Bond.fixed(
      'BAD-STUB',
      new core.Money(1_000_000, usd),
      new core.Rate(0.05),
      '2024-01-01',
      '2034-01-01',
      'not_a_stub',
      'USD-OIS'
    )
  );
});

test('Bond.fromJson rejects malformed JSON and wrong instrument types', () => {
  assert.throws(() => valuations.instruments.Bond.fromJson('{not valid json'));
  const loanJson = valuations.instruments.TermLoan.example().toJson();
  assert.throws(() => valuations.instruments.Bond.fromJson(loanJson));
});

test('TermLoan.example round-trips through the canonical envelope', () => {
  const loan = valuations.instruments.TermLoan.example();
  assert.equal(loan.id, 'TERM-LOAN-USD-5Y');
  const json = loan.toJson();
  const payload = JSON.parse(json);
  assert.equal(payload.schema, 'finstack_quant.instrument/1');
  assert.equal(payload.instrument.type, 'term_loan');
  assert.equal(valuations.instruments.TermLoan.fromJson(json).toJson(), json);
});

test('listedProductCatalog exposes exact venue-filtered valuation routes', () => {
  const montreal = valuations.market.listedProductCatalog('montreal');
  assert.ok(montreal.length > 0);
  assert.ok(montreal.every((row) => row.exchange === 'montreal'));
  assert.ok(montreal.some((row) => row.symbols.includes('CRA')));
  assert.ok(montreal.some((row) => row.instrument_type === 'interest_rate_future'));
  assert.ok(
    valuations.market
      .listedProductCatalog('sgx')
      .some((row) => row.instrument_type === 'commodity_future')
  );
  const optionRoutes = new Set(
    valuations.market
      .listedProductCatalog()
      .filter((row) => row.product_kind === 'option_on_future')
      .map((row) => row.instrument_type)
  );
  assert.deepEqual(
    optionRoutes,
    new Set([
      'commodity_future_option',
      'equity_future_option',
      'fx_future_option',
      'interest_rate_future_option',
      'volatility_index_future_option',
    ])
  );
  assert.throws(() => valuations.market.listedProductCatalog('mx'));
});
