# Consolidation Plan B17: Delete full-bag pricing overrides and mirrored builders

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F17 final deletion
- **Risk tier:** Tier 4 — public API deletion with wire-compatibility requirements
- **Estimated net LOC:** -250 to -450
- **Dependencies:** B15, B16
- **Branch:** `codex/simplicity-b17-delete-pricing-overrides-bag`
- **Commit subject:** `refactor(valuations): remove runtime pricing override bag`
- **Parallel / merge safety:** Must follow both complete override waves. Conflicts with B13 in `instrument.rs` and with B18 only if swaption still refers to the bag; land B17 first.

## Scope

Delete the runtime `PricingOverrides` catch-all, its mirrored fluent builders, full-bag trait hooks, temporary adapters, and root exports. Retain only focused runtime override types plus the minimal private wire adapter required to accept stable legacy JSON.

### Exact files

- `finstack-quant/valuations/src/instruments/pricing_overrides.rs`
- `finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs`
- `finstack-quant/valuations/src/instruments/common_impl/helpers.rs`
- `finstack-quant/valuations/src/metrics/core/traits.rs`
- `finstack-quant/valuations/src/instruments/mod.rs`

### Non-goals

- No provider migration; B15/B16 must already be complete.
- No removal of focused override builders.
- No legacy JSON break and no binding-owned compatibility parser.

## Invariants

- Existing serialized instrument fixtures still deserialize.
- Canonical serialized output and default behavior remain stable.
- Focused model, quote, metric, and scenario owners remain distinct.
- No all-fields runtime object survives under a new name.

## Implementation steps

1. Prove B15/B16 acceptance searches are clean.
2. Delete `PricingOverrides` runtime storage and mirrored forwarding builders.
3. Remove full-bag immutable/mutable trait hooks and temporary focused-accessor defaults.
4. Keep the private wire representation only at the serde boundary and convert directly to focused fields.
5. Remove exports, docs, examples, and tests that promote the catch-all runtime API.
6. Add negative compile/search assertions where practical to prevent reintroduction.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations pricing_overrides --lib
rtk cargo test -p finstack-quant-valuations --lib
rtk mise run rust-check-schemas
rtk uv run pytest finstack-quant-py/tests/parity -q
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

This intentionally removes a catch-all Rust API. Python/WASM must expose the focused canonical types consistently or retain only a facade that directly delegates without owning schema/policy. Legacy JSON acceptance remains mandatory and is verified by Rust schema tests and Python fixtures.

## Rollback

Revert B17 alone while B14–B16 remain landed; temporary full-bag API and adapters are restored around the focused provider storage.

## Done criteria

- No runtime catch-all override type, full-bag hook, or mirrored builder remains.
- Focused accessors are the only runtime override contract.
- Legacy JSON and binding parity are green.
- Full verification is green.

## Targeted re-audit acceptance

```sh
rtk rg -n 'pub struct PricingOverrides|fn pricing_overrides(_mut)?|impl PricingOverrides|PricingOverrides::' \
  finstack-quant/valuations/src
```

The command returns no production runtime API matches; a private `PricingOverridesWire` serde-boundary name is acceptable only inside `instruments/pricing_overrides.rs`.
