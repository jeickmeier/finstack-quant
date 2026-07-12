# Consolidation Plan B02: Make range-accrual model selection `ModelKey`-only

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F31
- **Risk tier:** Tier 4 — changes which pricing model is selected
- **Estimated net LOC:** -10 to +30
- **Dependencies:** None
- **Branch:** `codex/simplicity-b02-range-accrual-model-selection`
- **Commit subject:** `fix(valuations): select range-accrual model explicitly`
- **Parallel / merge safety:** Safe beside B01 and B03–B17. Conflicts with B12/B16 if those waves edit range-accrual pricing; land B02 first.

## Scope

Remove the implicit rule that the presence of `mc_seed_scenario` selects Monte Carlo pricing. The default range-accrual path remains analytic, while Monte Carlo is selected only by the explicit registered `ModelKey`; the seed remains a deterministic input to the MC pricer.

### Exact files

- `finstack-quant/valuations/src/instruments/rates/range_accrual/pricer.rs`

### Non-goals

- No change to either analytic or Monte Carlo formulas.
- No change to seed generation, RNG implementation, model registration, or public override field names.
- No removal of the MC seed override.

## Invariants

- The same explicit model key always selects the same pricer regardless of seed presence.
- An explicit MC key with the same seed remains deterministic.
- The default model remains analytic and ignores MC-only configuration.

## Implementation steps

1. Remove the seed-dependent branch from the shared/default PV calculation.
2. Keep seed consumption inside the registered Monte Carlo pricer only.
3. Route model choice solely through the existing registry/model-key mechanism.
4. Add a matrix test covering default/MC key crossed with absent/present seed.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations range_accrual --lib
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

No schema or symbol change. This deliberately corrects behavior for users who supplied a seed without selecting the MC model; add a Python behavioral regression only if that route is already exposed in runtime tests.

## Rollback

Revert the single PR. Existing serialized overrides remain readable because no wire field changes.

## Done criteria

- Seed presence cannot select a model.
- Explicit MC selection remains deterministic.
- Analytic and MC golden expectations pass.
- Full verification is green.

## Targeted re-audit acceptance

`rtk rg -n 'mc_seed_scenario' finstack-quant/valuations/src/instruments/rates/range_accrual/pricer.rs` shows seed use only inside the MC path, never in default model selection.
