# Consolidation Plan B12: Migrate rates, fixed-income, and credit-derivatives dependency providers

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F11 provider half-migration, wave 2
- **Risk tier:** Tier 3 — broad public trait migration
- **Estimated net LOC:** -150 to +250
- **Dependencies:** B02, B07, B09, B10
- **Branch:** `codex/simplicity-b12-market-dependencies-wave-two`
- **Commit subject:** `refactor(valuations): migrate market dependencies wave two`
- **Parallel / merge safety:** Domain-disjoint from B11 and may be developed in parallel, but both must land before B13. Conflicts with B16 and B18 in rates/fixed-income files; land this wave first.

## Scope

Convert every rates, fixed-income, and credit-derivatives provider that still implements legacy dependency traits or partial canonical dependencies to one complete, lossless `market_dependencies()` implementation. Remove legacy provider impls in the same atomic PR.

### Exact fileset

At the B12 parent commit, record and migrate the complete output of:

```sh
rtk rg -l 'curve_dependencies|equity_dependencies|market_dependencies' \
  finstack-quant/valuations/src/instruments/rates \
  finstack-quant/valuations/src/instruments/fixed_income \
  finstack-quant/valuations/src/instruments/credit_derivatives
```

The captured list is the PR's exact fileset. This domain wave is an explicit exception to the 1–5-file target. It must merge as one green, complete migration; internal vertical commits may not be merged separately.

### Non-goals

- No commodity, equity, exotics, or FX provider changes.
- No pricing-override migration.
- No curve-role redesign, pricing change, or instrument-schema change.

## Invariants

- Discount, forecast, hazard, recovery, inflation, repo, and collateral curve roles remain distinct and unchanged.
- Optional dependencies remain optional; required dependencies remain required.
- Dependency ordering and deduplication are deterministic.
- Risk factor selection and fully qualified metric keys remain stable.

## Implementation steps

1. Freeze the exact fileset in the PR description.
2. Migrate rates providers by complete instrument vertical, testing discount/forecast roles.
3. Migrate fixed-income providers, testing repo, hazard, recovery, and optional curve behavior.
4. Migrate credit-derivatives providers, testing index/basket/tranche dependency completeness.
5. Delete legacy impl blocks from every migrated provider.
6. Re-run the search and resolve every legacy provider in these domains before merge.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations rates --lib
rtk cargo test -p finstack-quant-valuations fixed_income --lib
rtk cargo test -p finstack-quant-valuations credit_derivatives --lib
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

No host API or wire change is intended. Do not move dependency policy into Python or WASM. Existing risk-metric parity must remain green through `all-test` and `python-build`.

## Rollback

Revert the complete wave before B13. Do not partially revert a merged vertical migration.

## Done criteria

- Every selected provider has one canonical dependency implementation.
- No legacy provider impl remains in the three domains.
- Curve-role and dependency-set regression tests pass.
- Full verification is green at the atomic PR head.

## Targeted re-audit acceptance

```sh
rtk rg -n 'impl (CurveDependencies|EquityDependencies)|fn (curve_dependencies|equity_dependencies)' \
  finstack-quant/valuations/src/instruments/rates \
  finstack-quant/valuations/src/instruments/fixed_income \
  finstack-quant/valuations/src/instruments/credit_derivatives
```

The command returns no production matches.
