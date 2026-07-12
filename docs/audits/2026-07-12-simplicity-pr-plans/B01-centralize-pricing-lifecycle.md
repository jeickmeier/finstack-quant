# Consolidation Plan B01: Centralize validation and pricing lifecycle

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F19, H1; H12 interaction only
- **Risk tier:** Tier 4 — pricing control-flow and validation behavior
- **Estimated net LOC:** +40 to +120
- **Dependencies:** None
- **Branch:** `codex/simplicity-b01-pricing-lifecycle`
- **Commit subject:** `refactor(valuations): centralize pricing lifecycle`
- **Parallel / merge safety:** Safe beside B02–B08. Conflicts with B09, B14, and B17 in `instrument.rs` and `helpers.rs`; land B01 first.

## Scope

Create one pricing lifecycle used by `Instrument::value`, base/raw value helpers, generic pricing helpers, registry dispatch, and `price_with_metrics`: validate the instrument, resolve the model, compute the base result, apply scenario handling, then attach metrics. A direct registry call must no longer bypass instrument validation.

### Exact files

- `finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs`
- `finstack-quant/valuations/src/pricer/registry.rs`
- `finstack-quant/valuations/src/instruments/common_impl/pricing/generic.rs`
- `finstack-quant/valuations/src/instruments/common_impl/helpers.rs`

### Non-goals

- No pricing-formula changes.
- No choice between the competing degenerate-ATM Black-76 conventions in H12.
- No registry-key, metric-name, scenario, or public result-shape changes.

## Invariants

- Every public pricing entry point validates before model lookup or market access.
- Validation errors and model-resolution errors preserve their existing error categories.
- Base value, raw base value, scenario value, and metric attachment retain their current meanings.
- Existing zero-volatility and expiry-boundary outputs are pinned, not reinterpreted.

## Implementation steps

1. Extract the smallest internal lifecycle helper around validation, registry resolution, base computation, scenario application, and metric attachment.
2. Delegate the trait entry points and generic wrappers to that lifecycle.
3. Make direct registry dispatch call the same validated path or require an explicitly validated internal token/helper.
4. Remove overlapping lifecycle fragments once all callers delegate.
5. Add regression tests for invalid instruments through every public entry point and for unchanged base/raw/scenario results.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations --lib
rtk cargo test -p finstack-quant-valuations --test instruments
```

Add tests proving direct registry and `price_with_metrics` reject the same malformed instrument as `value`, and pin current t=0/zero-volatility behavior relevant to H12.

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

No intended Python, WASM, parity-contract, or serde-shape change. Behavioral tests in bindings must continue to observe the same successful prices and the same invalid-input rejection.

## Rollback

Revert this PR as one commit. No data migration is involved; the old entry-point implementations are restored together.

## Done criteria

- All public pricing routes share one ordered lifecycle.
- No direct registry route can price an invalid instrument.
- Successful prices and metric keys are unchanged.
- Full verification is green.

## Targeted re-audit acceptance

- `price_with_metrics` contains no independent validation-bypassing pipeline.
- `registry.rs` exposes no public path to `price_dyn` before validation.
- Searches for duplicated validate/resolve/scenario/metrics sequences find only the canonical lifecycle and test scaffolding.
