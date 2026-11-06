/**
 * Cross-host golden for `attribution.pnlAttributionPctOfTotalByBucket`.
 *
 * The Python twin is `finstack-quant-py/tests/test_pnl_bucket_share_parity.py`:
 * both hosts read the same pinned attribution and compare to the same records.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade, plain } from './statements_parity_support.mjs';

const read = (path) =>
  readFileSync(new URL(`../../../finstack-quant-py/tests/${path}`, import.meta.url), 'utf8');

test('pnlAttributionPctOfTotalByBucket matches the records pinned for both hosts', () => {
  const actual = facade.attribution.pnlAttributionPctOfTotalByBucket(
    read('data/attribution_bond.json')
  );
  assert.deepEqual(plain(actual), JSON.parse(read('fixtures/pnl_bucket_shares.json')));
});
