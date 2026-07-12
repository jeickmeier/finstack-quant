# Consolidation Plan B05: Canonicalize `Position`

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F9 (`Position` duplication)
- **Risk tier:** Tier 3 — public Rust type consolidation
- **Estimated net LOC:** -20 to +40
- **Dependencies:** B03
- **Branch:** `codex/simplicity-b05-canonical-position`
- **Commit subject:** `refactor(valuations): canonicalize position type`
- **Parallel / merge safety:** Safe beside B01, B02, B04, B06–B08 after B03. Conflicts with dependency/override waves touching commodity-forward or IR-future types; land before B11/B12/B15/B16.

## Scope

Create one runtime `Position` enum used by commodity forwards, IR futures, and bond futures. Preserve useful semantics (`sign`, default, and accepted buy/buyer/sell/seller serde spellings) while removing the duplicate IR-future runtime enum. Old module paths may re-export the canonical type, but may not define a parallel enum or conversion layer.

### Exact files

- `finstack-quant/valuations/src/instruments/position.rs`
- `finstack-quant/valuations/src/instruments/mod.rs`
- `finstack-quant/valuations/src/instruments/common_impl/parameters/market.rs`
- `finstack-quant/valuations/src/instruments/rates/ir_future/types.rs`
- `finstack-quant/valuations/src/instruments/commodity/commodity_forward/types.rs`

### Non-goals

- No new long/short quantity abstraction.
- No change to instrument payoff sign conventions.
- No second enum retained behind a type with identical variants.

## Invariants

- Long/buy positions retain positive sign; short/sell positions retain negative sign.
- Existing accepted serde aliases remain accepted and canonical serialization remains stable.
- Bond futures continue to use the same canonical runtime type through their existing re-export chain.

## Implementation steps

1. Add the canonical enum and its docs, default, serde aliases, and `sign` method.
2. Change commodity-forward and IR-future fields/imports to the canonical type.
3. Delete both old enum definitions.
4. Retain source-path compatibility only through direct `pub use` re-exports of the same type.
5. Add identity/type and serde tests proving there is one runtime type.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations ir_future --lib
rtk cargo test -p finstack-quant-valuations commodity_forward --lib
rtk cargo test -p finstack-quant-valuations bond_future --lib
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

Host-language names and accepted payloads must remain unchanged. Run parity through the full gate; update parity metadata only if it records the canonical Rust source path, never to expose a second host type.

## Rollback

Revert the PR. No data rewrite is needed because serde spelling remains stable.

## Done criteria

- Exactly one `enum Position` exists in valuations production code.
- All participating instruments store that exact type.
- Compatibility paths, if retained, are pure re-exports.
- Full verification is green.

## Targeted re-audit acceptance

`rtk rg -n 'enum Position' finstack-quant/valuations/src` returns exactly one production definition, and no `From`/`Into` bridge exists between position enums.
