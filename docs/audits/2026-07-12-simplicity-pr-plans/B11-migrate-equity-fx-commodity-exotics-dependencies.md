# Consolidation Plan B11: Migrate commodity, equity, exotics, and FX dependency providers

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F11 provider half-migration, wave 1
- **Risk tier:** Tier 3 — broad public trait migration
- **Estimated net LOC:** -100 to +250
- **Dependencies:** B08, B09, B10
- **Branch:** `codex/simplicity-b11-market-dependencies-wave-one`
- **Commit subject:** `refactor(valuations): migrate market dependencies wave one`
- **Parallel / merge safety:** Domain-disjoint from B12 and may be developed in parallel, but both must land before B13. Conflicts with B15 in the same instrument files; land dependency migration before override migration.

## Scope

Convert every commodity, equity, exotics, and FX provider that still implements legacy dependency traits or partial canonical dependencies to one complete, lossless `market_dependencies()` implementation. Remove the legacy provider impls in the same atomic PR.

### Exact fileset

At the B11 parent commit, record and migrate the complete output of:

```sh
rtk rg -l 'curve_dependencies|equity_dependencies|market_dependencies' \
  finstack-quant/valuations/src/instruments/commodity \
  finstack-quant/valuations/src/instruments/equity \
  finstack-quant/valuations/src/instruments/exotics \
  finstack-quant/valuations/src/instruments/fx
```

The captured list is the PR's exact fileset. This domain wave is an explicit exception to the 1–5-file target. It must merge as one green, complete migration; no domain may be left with dual provider implementations. Internal commits may be grouped by domain but may not be merged separately.

### Non-goals

- No rates, fixed-income, or credit-derivatives provider changes.
- No pricing-override migration.
- No pricing, calibration, or instrument-schema changes.

## Invariants

- Each instrument reports the same or a strictly more complete dependency set.
- Equity/exotics volatility dependencies preserve reference strike.
- FX pair orientation, curve roles, correlation IDs, and commodity forward-curve roles remain unchanged.
- Dependency sets remain deterministic and duplicate-free.

## Implementation steps

1. Freeze the fileset with the command above in the PR description.
2. Migrate commodity providers and add dependency-set tests.
3. Migrate equity and exotics providers, preserving volatility strike metadata.
4. Migrate FX providers, preserving domestic/foreign curve roles and pair orientation.
5. Delete legacy impl blocks from every migrated provider.
6. Re-run the fileset search and resolve every remaining legacy provider in these domains before merge.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations commodity --lib
rtk cargo test -p finstack-quant-valuations equity --lib
rtk cargo test -p finstack-quant-valuations exotics --lib
rtk cargo test -p finstack-quant-valuations fx --lib
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

No host API or serde change is intended. Dependency metadata must not create a binding-owned policy layer. Run existing behavioral parity through `all-test`; do not edit stubs or parity contracts unless canonical dependencies are already an exposed surface.

## Rollback

Revert the complete wave before B13. Do not revert individual domain commits from a merged wave because consumers then see mixed dependency semantics.

## Done criteria

- Every selected provider has one canonical dependency implementation.
- No legacy provider impl remains in the four domains.
- Representative dependency-set and risk-metric tests pass.
- Full verification is green at the atomic PR head.

## Targeted re-audit acceptance

```sh
rtk rg -n 'impl (CurveDependencies|EquityDependencies)|fn (curve_dependencies|equity_dependencies)' \
  finstack-quant/valuations/src/instruments/commodity \
  finstack-quant/valuations/src/instruments/equity \
  finstack-quant/valuations/src/instruments/exotics \
  finstack-quant/valuations/src/instruments/fx
```

The command returns no production matches.
