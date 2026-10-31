/**
 * Cross-host golden for `models.factor.risk.evaluateRiskBudgetFromDecomposition`.
 *
 * The Python twin is
 * `finstack-quant-py/tests/test_risk_budget_from_decomposition_parity.py`: both
 * hosts read the same pinned decomposition and compare to the same result.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade, plain } from './statements_parity_support.mjs';

const PINNED = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant-py/tests/fixtures/risk_budget_from_decomposition.json',
      import.meta.url
    )
  )
);

test('evaluateRiskBudgetFromDecomposition matches the result pinned for both hosts', () => {
  const actual = facade.models.factor.risk.evaluateRiskBudgetFromDecomposition(
    PINNED.decomposition,
    PINNED.targets,
    1.1
  );
  assert.deepEqual(plain(actual), PINNED.result);
});
