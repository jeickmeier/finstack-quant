# Consolidation Plan B04: Move `MarketContextSplit` into test support

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F30
- **Risk tier:** Tier 3 — public Rust helper removal
- **Estimated net LOC:** -30 to -80 production LOC
- **Dependencies:** None
- **Branch:** `codex/simplicity-b04-market-context-test-support`
- **Commit subject:** `refactor(valuations): move market split helper to tests`
- **Parallel / merge safety:** Independent of B01–B03 and B05–B18; low conflict risk.

## Scope

Remove the public, production `MarketContextSplit` helper that is used only by integration-test support. Re-home the minimal conversion logic in `valuations/tests/support/test_utils.rs`; keep production validation of rejected legacy/v2 inputs intact.

### Exact files

- `finstack-quant/valuations/src/calibration/api/market_datum.rs`
- `finstack-quant/valuations/tests/support/test_utils.rs`

### Non-goals

- No calibration-market-datum schema change.
- No relaxation of `validate.rs` rejection behavior.
- No new public replacement type.

## Invariants

- Integration tests construct identical market contexts.
- Production calibration input validation remains unchanged.
- Test-only convenience code is not exported by the library.

## Implementation steps

1. Copy only the needed split/conversion behavior into a private test-support function.
2. Switch integration-test callers to that function.
3. Delete `MarketContextSplit`, its production impls, exports, and production-only tests.
4. Retain tests for the underlying supported market-datum behavior.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations calibration
rtk cargo test -p finstack-quant-valuations --tests
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

No expected Python/WASM or serde change. If the symbol appears in generated documentation only, update generated output through the normal documentation pipeline rather than retaining an alias.

## Rollback

Revert the PR; no stored-data or schema migration is involved.

## Done criteria

- Production code exports no `MarketContextSplit`.
- Test support remains private and behaviorally equivalent.
- Calibration validation tests remain green.
- Full verification is green.

## Targeted re-audit acceptance

`rtk rg -n 'MarketContextSplit' finstack-quant/valuations/src` returns no matches; any remaining match is confined to private test support or historical audit documentation.
