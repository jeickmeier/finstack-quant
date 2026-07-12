# Consolidation Plan B07: Collapse legacy GNMA into GNMA II

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F9 (GNMA/GNMA II duplication)
- **Risk tier:** Tier 4 — serde-sensitive public enum consolidation
- **Estimated net LOC:** -10 to +20
- **Dependencies:** None
- **Branch:** `codex/simplicity-b07-gnma-ii`
- **Commit subject:** `refactor(valuations): canonicalize GNMA II agency`
- **Parallel / merge safety:** Safe beside B01–B06 and B08–B11. Conflicts with B12/B16 in fixed-income files; land first.

## Scope

Remove the legacy `Gnma` runtime variant and use `GnmaII` as the single canonical agency variant. Continue accepting legacy serialized `GNMA` through a serde alias while emitting the canonical `GNMA_II` spelling. Consolidate payment-delay and TBA-code branches.

### Exact files

- `finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/types.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/delay.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/tba/types.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/tba/pricer.rs`

### Non-goals

- No change to GNMA II settlement, delay, coupon, pool, or TBA pricing conventions.
- No attempt to combine other agency variants.
- No retained parallel Rust variant for source compatibility.

## Invariants

- Legacy payloads containing `GNMA` deserialize as `GnmaII`.
- Serialization emits only the canonical spelling.
- Payment dates, delay-day behavior, and TBA classification remain numerically unchanged.

## Implementation steps

1. Add the legacy serde alias to `GnmaII`.
2. Delete `Gnma` and merge its match arms into `GnmaII` behavior.
3. Update TBA construction and pricing matches to the canonical variant.
4. Replace tests that compare separate variants with legacy-deserialization and canonical-roundtrip tests.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations mbs_passthrough --lib
rtk cargo test -p finstack-quant-valuations tba --lib
rtk mise run rust-check-schemas
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

Rust source users must move from `Gnma` to `GnmaII`. Serialized legacy input remains compatible; canonical output changes only for values previously represented as legacy `GNMA`. Binding enums and stubs must expose one GNMA II member, with no duplicate host member.

## Rollback

Revert the PR. Payloads written as `GNMA_II` remain readable by the pre-PR code.

## Done criteria

- One GNMA runtime variant remains.
- Legacy input and canonical output roundtrip tests pass.
- Agency timing and TBA pricing tests are unchanged.
- Full verification is green.

## Targeted re-audit acceptance

`rtk rg -n '\bGnma\b|"GNMA"' finstack-quant/valuations/src/instruments/{fixed_income/mbs_passthrough,fixed_income/tba}` finds only the intentional serde alias and migration tests.
