/**
 * Cross-host golden for `analytics.Performance.lookbackReturns` with no date.
 *
 * The Python twin is `finstack-quant-py/tests/test_lookback_default_parity.py`:
 * both hosts build the same pinned panel and compare to the same result.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade, plain } from './statements_parity_support.mjs';

const PINNED = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/fixtures/lookback_default.json', import.meta.url)
  )
);

test('lookbackReturns defaults to the date pinned for both hosts', () => {
  const panel = facade.analytics.Performance.fromReturns(
    PINNED.dates,
    PINNED.returns,
    ['asset'],
    null,
    'daily'
  );
  try {
    assert.deepEqual(plain(panel.lookbackReturns()), PINNED.lookback);
    assert.equal(panel.lookbackReturns(null).ref_date, PINNED.dates.at(-1));
  } finally {
    panel.free();
  }
});
