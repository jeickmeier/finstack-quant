# Consolidation Plan: D12 — Make Money construction uniformly fallible

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-d12-make-money-construction-fallible`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Make Money construction uniformly fallible

**Tier:** 4 (Decimal/public API-sensitive)  
**Estimated net LOC:** −50 to −150 net; very high mechanical churn  
**Addresses:** F22  
**Depends on:** A03-A05 recommended first

**Files/filesets:**
- `finstack-quant/core/src/money/types.rs`
- `finstack-quant/core/src/money/mod.rs`
- `finstack-quant-py/src/bindings/core/money.rs`
- `finstack-quant-wasm/src/api/core/money.rs`
- `All workspace `Money::new` and `new_with_config` call sites, fixtures, docs, benches, and generated schemas`

**Scope:** Make the short public constructor checked, remove panicking constructor shadows, and reserve explicit crate-private `_unchecked` construction only for trusted constants after call-site audit.

**Non-goals:** Do not convert Decimal arithmetic to f64, change scale/rounding, currency checks, or serde representation.

**Invariants touched:** Decimal equality, rounding context, currency safety, serde, parity.

## Implementation

1. Characterize finite/non-finite and configured-rounding behavior.
2. Define the final checked constructor names and private trusted path.
3. Mechanically migrate production, tests, docs, and both bindings.
4. Delete panicking shadows and run stale-call-site/API searches.
5. Compare Decimal goldens serially and in the normal test configuration.

## Tests to add or update

- Core money rounding/FX tests; cashflow/valuation/portfolio monetary goldens; Python/WASM Money error mapping.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run python-build -- --release
rtk mise run python-lint
rtk mise run python-test
rtk mise run wasm-build
rtk mise run wasm-lint
rtk mise run wasm-test
rtk uv run pytest finstack-quant-py/tests/parity -x
```

**Bindings/parity/serde impact:** Both hosts touched. Compile-atomic large-PR exception because the public signature changes across the workspace.

**Parallel and merge safety:** Do not run with A03-A05 or financial-kernel PRs that add `Money` literals.

**Rollback:** Atomic workspace revert.

## Done when

- All external f64 construction returns `Result`; no panicking `Money` constructor remains.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
