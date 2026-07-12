# Consolidation Plan B08: Canonicalize barrier direction and activation type

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F9 (four barrier-type definitions)
- **Risk tier:** Tier 3 — cross-crate public type consolidation
- **Estimated net LOC:** -50 to +80
- **Dependencies:** None
- **Branch:** `codex/simplicity-b08-canonical-barrier-type`
- **Commit subject:** `refactor(core): canonicalize barrier type`
- **Parallel / merge safety:** Safe beside B01–B07. Conflicts with B11/B15 and barrier-model work; land before those waves.

## Scope

Introduce one canonical four-state barrier type in core and use that exact runtime type in Monte Carlo payoff code, instrument definitions, closed-form pricing, and tree state. Normalize variant names to `UpAndIn`, `UpAndOut`, `DownAndIn`, and `DownAndOut`; temporary associated-constant spelling shims are allowed only within this PR if needed to keep the migration atomic.

### Exact files

- `finstack-quant/core/src/types/barrier.rs` (new)
- `finstack-quant/core/src/types/mod.rs`
- `finstack-quant/monte_carlo/src/payoff/barrier.rs`
- `finstack-quant/valuations/src/instruments/exotics/barrier_option/types.rs`
- `finstack-quant/valuations/src/models/closed_form/barrier.rs`
- `finstack-quant/valuations/src/models/trees/tree_framework/node_state.rs`

This six-file cross-crate slice is an atomic exception to the 1–5-file target: leaving any one duplicate in place preserves the conversion graph the PR is intended to delete.

### Non-goals

- No barrier payoff, monitoring, rebate, touching-condition, Heston, PDE, or tree-algorithm change.
- No generic option-style hierarchy.
- No second runtime enum retained by a compatibility alias.

## Invariants

- Every old variant maps one-to-one to the same direction and in/out activation.
- Barrier-touch equality semantics remain unchanged in every model.
- Python/WASM names and serialized spellings remain stable unless the existing contract explicitly uses canonical Rust-derived names.

## Implementation steps

1. Add the documented canonical core enum with existing serde semantics.
2. Replace the four enum definitions with direct imports/re-exports of the core type.
3. Remove all type-to-type barrier conversion impls and match adapters.
4. Update internal variant spellings without altering comparison conditions.
5. Add a cross-model variant matrix proving identical classification and payoff behavior.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-core barrier
rtk cargo test -p finstack-quant-monte-carlo barrier
rtk cargo test -p finstack-quant-valuations barrier --lib
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

Preserve host-language enum names and wire values. If canonical ownership changes generated documentation or type paths, update exports/stubs without adding wrapper enums; run the structural parity suite through `all-test`.

## Rollback

Revert the whole PR. Do not roll back a subset because the shared type crosses three crates.

## Done criteria

- One production barrier enum exists across core, Monte Carlo, and valuations.
- No barrier-enum conversion graph remains.
- Cross-model variant tests and full verification are green.

## Targeted re-audit acceptance

`rtk rg -n 'enum BarrierType|enum BarrierDirection' finstack-quant/{core,monte_carlo,valuations}/src` finds exactly the canonical core definition, and searches find no `From` implementation translating barrier kinds.
