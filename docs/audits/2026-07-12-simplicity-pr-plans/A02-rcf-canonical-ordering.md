# Consolidation Plan A02: Use Canonical Same-Date Ordering for Revolving Credit

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F4.
- Risk tier: Tier 4 — same-date ordering can change outstanding balances and cashflow amounts.
- Estimated net change: -20 to +40 LOC.
- Dependencies: none.
- Suggested branch: `codex/a02-rcf-canonical-ordering`.
- Parallel and merge safety: safe beside every slice except A03/A04 if they edit canonical schedule sorting simultaneously. Merge before A04 so metadata sorting has one established order.
- Atomicity: normal three-file slice; no compile-atomic exception.

## Exact Files

- `finstack-quant/cashflows/src/builder/schedule.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/revolving_credit/cashflow_engine.rs`
- `finstack-quant/valuations/tests/instruments/revolving_credit/cashflows.rs`

## Scope

- Delete the revolving-credit-local cashflow rank/comparator.
- Use the cashflows crate's canonical date-plus-kind ordering at both revolving-credit sort sites.
- Expose only the smallest sorting helper needed by valuations if current visibility is too narrow.
- Pin economically significant same-date ordering with a regression test.

## Non-Goals

- No change to event-generation formulas, utilization logic, fee rates, or PIK calculation.
- No global redefinition of canonical kind precedence unless a new regression demonstrates the existing canonical order is economically wrong.
- No schedule metadata redesign; that belongs to A03/A04.

## Implementation Steps

1. Add or expose one canonical `sort_flows`/comparator entry point in `schedule.rs` without duplicating its rank table.
2. Replace both local `sort_by_key` blocks in `cashflow_engine.rs` with that helper.
3. Remove the local `cf_rank` function and comments describing the divergent order.
4. Add a focused fixture containing same-date PIK, amortization, fee, draw/repayment, and final notional flows.
5. Assert both final order and outstanding-balance consequences.

## Tests to Add or Update

- Regression for same-date PIK before principal reduction where required by the canonical order.
- Regression covering fee and repayment rows on the same date.
- Existing revolving-credit cashflow and pricing tests must remain numerically unchanged except where the prior divergent ordering was wrong.

## Full Verification

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run goldens-test
```

## Binding, Parity, and Serde Impact

- Python/WASM: no surface change; serialized flow order may change for the affected same-date RCF case.
- Parity contract: no topology change.
- Serde/schema: no shape change.
- Golden impact: any changed RCF output must be reviewed as an intentional ordering correction, not blindly accepted.

## Rollback

Revert the helper visibility change, the two call-site replacements, and the regression together.

## Done Criteria

- `cashflow_engine.rs` contains no local cashflow kind-rank table.
- Every RCF sort uses the cashflows canonical helper.
- The same-date economic regression passes and documents the expected balance path.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` on the RCF engine and canonical schedule sorter. Accept only when exactly one kind-order policy remains and no local comparator, post-sort repair, or second ordering table exists.
