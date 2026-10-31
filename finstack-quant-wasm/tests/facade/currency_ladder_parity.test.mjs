/**
 * Cross-host golden for `valuations.instruments.calendarYearLadderByCurrency`.
 *
 * The Python twin is `finstack-quant-py/tests/test_currency_ladder_parity.py`:
 * both hosts read the same pinned envelope and compare to the same ladders.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade, plain } from './statements_parity_support.mjs';

const PINNED = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/fixtures/currency_ladder.json', import.meta.url)
  )
);

test('calendarYearLadderByCurrency matches the ladders pinned for both hosts', () => {
  const actual = facade.valuations.instruments.calendarYearLadderByCurrency(PINNED.envelope);
  assert.deepEqual(plain(actual), PINNED.ladders);
});
