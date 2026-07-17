# Consolidation Plan: D05 — Remove the unpriceable InstrumentType::Loan fossil

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-d05-remove-unpriceable-loan-kind`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Remove the unpriceable InstrumentType::Loan fossil

**Tier:** 4 (serde-sensitive public change)
**Estimated net LOC:** −10 to −40
**Addresses:** F32
**Depends on:** B03 recommended

**Files/filesets:**
- `finstack-quant/valuations/src/pricer/keys.rs`
- `finstack-quant/valuations/src/instruments/exotics/basket/metrics/constituent_delta.rs`
- `finstack-quant/valuations/tests/instruments/common/pricer/registry.rs`
- `finstack-quant-py/parity_contract.toml`

**Scope:** Remove the runtime `Loan` variant; parse legacy `"loan"` as `TermLoan` and make scenario/asset classification use the real priceable type.

**Non-goals:** Do not rename `TermLoan`, change registry dispatch, or drop the accepted legacy input string.

**Invariants touched:** Serde compatibility and pricer dispatch identity.

## Implementation

1. Lock parsing and serialization behavior for `loan` and `term_loan`.
2. Redirect classification/filter branches to `TermLoan`.
3. Delete `InstrumentType::Loan` and its display arm.
4. Update parity/contract metadata if the enum is host-visible.

## Tests to add or update

- InstrumentType parse/round-trip tests and term-loan registry pricing smoke tests.

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

**Bindings/parity/serde impact:** Parity contract only if host-visible; run full binding stack.

**Parallel and merge safety:** Can run beside most plans, but not B-domain enum work touching `instruments/mod.rs` or pricer keys.

**Rollback:** Revert the variant deletion and parser mapping together.

## Done when

- Every accepted loan label resolves to the priceable `TermLoan`; no `Loan` runtime variant remains.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
