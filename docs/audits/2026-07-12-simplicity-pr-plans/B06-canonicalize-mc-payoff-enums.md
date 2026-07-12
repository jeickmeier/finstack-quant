# Consolidation Plan B06: Reuse instrument payoff enums in Monte Carlo pricers

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F9 (autocall and cliquet payoff duplication)
- **Risk tier:** Tier 2 — source-compatible type consolidation
- **Estimated net LOC:** -20 to -60
- **Dependencies:** None
- **Branch:** `codex/simplicity-b06-mc-payoff-enums`
- **Commit subject:** `refactor(valuations): reuse canonical payoff enums`
- **Parallel / merge safety:** Safe beside B01–B05, B07, and B08. Conflicts with B11/B15 in the same equity modules; land first.

## Scope

Use each instrument's canonical payoff enum directly in its Monte Carlo module and remove the duplicate MC enum plus conversion match. Preserve old MC module paths only as direct re-exports of the canonical type where source compatibility is useful.

### Exact files

- `finstack-quant/valuations/src/instruments/equity/autocallable/monte_carlo.rs`
- `finstack-quant/valuations/src/instruments/equity/autocallable/pricer.rs`
- `finstack-quant/valuations/src/instruments/equity/cliquet_option/monte_carlo.rs`
- `finstack-quant/valuations/src/instruments/equity/cliquet_option/pricer.rs`

### Non-goals

- No payoff-formula, simulation, path-generation, or enum-variant rename.
- No new generic payoff abstraction shared between unrelated instruments.

## Invariants

- Every existing payoff variant produces identical path payoffs.
- Serde and binding ownership remains with the instrument-domain enum.
- No runtime conversion is needed between instrument and MC payoff types.

## Implementation steps

1. Import or directly re-export the instrument-domain enum from each MC module.
2. Delete the MC-local enum definitions.
3. Remove conversion matches from both pricers.
4. Pass canonical values directly to MC configuration/payoff construction.
5. Add exhaustive per-variant regression tests if current tests do not cover every variant.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations autocallable --lib
rtk cargo test -p finstack-quant-valuations cliquet_option --lib
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

No host or wire change is intended. Direct re-exports must point to the same type identity; do not introduce wrapper types to preserve paths.

## Rollback

Revert the PR; no serialized data migration is involved.

## Done criteria

- One payoff enum exists per instrument concept.
- Conversion matches are removed.
- MC results remain unchanged for every variant.
- Full verification is green.

## Targeted re-audit acceptance

Searches in both `monte_carlo.rs` files find no local payoff `enum` definitions, and the corresponding pricers contain no payoff-type conversion match.
