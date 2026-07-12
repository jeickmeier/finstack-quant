# Consolidation Plan: E08 — Make the parity contract authoritative for repeated export manifests

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-e08-centralize-binding-export-contract`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Make the parity contract authoritative for repeated export manifests

**Tier:** 2/3 (contract tooling)  
**Estimated net LOC:** −40 to −100  
**Addresses:** F33 and binding-drift follow-up  
**Depends on:** A11, A12, E01-E07

**Files/filesets:**
- `finstack-quant-py/parity_contract.toml`
- `finstack-quant-py/tests/parity/test_contract_topology.py`
- `finstack-quant-py/src/bindings/cashflows/mod.rs`
- `finstack-quant-py/finstack_quant/cashflows/__init__.py`
- `finstack-quant-py/finstack_quant/cashflows/__init__.pyi`
- `finstack-quant-wasm/exports/cashflows.js`

**Scope:** Use one authoritative contract list to generate or validate cashflow exports; remove repeated hard-coded test lists. Classify core plumbing modules as intentionally excluded rather than perpetually `missing`.

**Non-goals:** Do not dynamically discover runtime exports or broaden the documented WASM core subset.

**Invariants touched:** Explicit exports, documented WASM subset, triplet naming.

## Implementation

1. Update the post-A12 canonical cashflow symbol list in the parity contract.
2. Extend existing generation/check tooling rather than adding an independent generator.
3. Make Python registration/package/stub and JS facade lists derive from or be checked against that source.
4. Replace duplicate topology literals and classify intentional core exclusions.
5. Run topology and full parity suites.

## Tests to add or update

- Parity contract topology, namespace `__all__`, stub, JS facade, and generated-artifact checks.

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

**Bindings/parity/serde impact:** Python/WASM manifests and parity only.

**Parallel and merge safety:** Final binding cleanup; implement after all earlier binding-surface PRs and merge alone.

**Rollback:** Revert generator/check and manifest changes together.

## Done when

- One authoritative export inventory; no repeated seven-symbol cashflow test manifest and no ambiguous `missing` plumbing entries.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.
