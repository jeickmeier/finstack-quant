/**
 * Cross-host check that a full-grid sensitivity result has no tornado.
 *
 * The Python twin is `finstack-quant-py/tests/test_tornado_full_grid_parity.py`:
 * both read the same pinned result; Python gets `None`, JavaScript `null`.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade } from './statements_parity_support.mjs';

const PINNED = readFileSync(
  new URL('../../../finstack-quant-py/tests/fixtures/tornado_full_grid.json', import.meta.url),
  'utf8'
);

test('generateTornadoEntries returns null for a full-grid result', () => {
  assert.equal(
    facade.statements_analytics.generateTornadoEntries(PINNED, 'profit', '2025Q2'),
    null
  );
});
