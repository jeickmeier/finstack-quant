/**
 * Cross-host golden for `valuations.instruments.priceDriverFields`.
 *
 * The Python twin is `finstack-quant-py/tests/test_price_driver_fields_parity.py`:
 * both hosts compare to the same pinned list.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade } from './statements_parity_support.mjs';

const PINNED = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/fixtures/price_driver_fields.json', import.meta.url)
  )
);

test('priceDriverFields matches the list pinned for both hosts', () => {
  assert.deepEqual(facade.valuations.instruments.priceDriverFields(), PINNED);
});
