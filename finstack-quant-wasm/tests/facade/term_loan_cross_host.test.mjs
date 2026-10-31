/**
 * Cross-host golden for term-loan outputs bound through both hosts.
 *
 * The Python twin is `finstack-quant-py/tests/test_term_loan_cross_host.py`:
 * both hosts price the shared example term loan on the shared market and compare
 * to the same pinned values.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade } from './statements_parity_support.mjs';

const INSTRUMENT = readFileSync(
  new URL(
    '../../../finstack-quant/valuations/tests/instruments/json_examples/term_loan.json',
    import.meta.url
  ),
  'utf8'
);
const PINNED = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/fixtures/term_loan_cross_host.json', import.meta.url)
  )
);
const REL = 1e-12;

test('oid_eir_rate matches the value pinned for both hosts', () => {
  const result = facade.valuations.instruments.priceInstrument(
    INSTRUMENT,
    JSON.stringify(PINNED.market),
    PINNED.as_of,
    PINNED.model,
    ['oid_eir_rate']
  );
  const actual = result.measures.oid_eir_rate;
  assert.ok(
    Math.abs(actual - PINNED.oid_eir_rate) <= REL * Math.abs(PINNED.oid_eir_rate),
    `${actual}`
  );
});
