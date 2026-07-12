# C08 — Make the FX Delta Builder Construct Its Named Surface

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F20; prepares H5
- **Tier:** 4
- **Estimated net LOC:** +60 to +140
- **Dependencies:** C01
- **Branch:** `codex/simplify-c08-canonical-fx-delta-builder`
- **Parallel / merge safety:** may run in parallel with C02, C03, and C06 after C01. C09 depends on it. Conflicts with core FX-delta surface construction work.

## Exact files

- `finstack-quant/core/src/market_data/surfaces/delta_vol_surface.rs`
- `finstack-quant/core/src/market_data/surfaces/fx_delta_vol_surface.rs`
- `finstack-quant/core/src/market_data/surfaces/mod.rs`
- `finstack-quant/core/tests/market_data/surfaces/fx_delta_vol_tests.rs`

## Scope

- Make public `FxDeltaVolSurfaceBuilder` collect expiries, ATM, 25-delta wings, and optional paired 10-delta wings, and return `FxDeltaVolSurface`.
- Rename/make private the existing spot/rates/delta-to-strike grid converter; expose generic conversion only through `FxDeltaVolSurface::to_vol_surface`.
- Have `new` and temporarily retained `with_10d` delegate to the canonical builder.
- Add a fallible `try_pillar_vols` alongside the existing accessor so C09 can migrate bindings without an intermediate break.

## Non-goals

- Do not change delta convention, wing recovery, strike ordering, interpolation, serde shape, or Python/WASM constructor signatures.
- Do not remove the old panicking accessor or host branches yet; C09 owns that cutover.

## Implementation steps

1. Separate quote-object construction from strike-grid conversion by type and visibility.
2. Implement paired optional 10-delta validation once in the public builder.
3. Route constructors through the builder and `to_vol_surface` through the private converter.
4. Add checked pillar indexing returning the existing core error type.
5. Update Rust tests/docs that previously expected the misleading builder to return `VolSurface`.

## Behavior / golden tests

- C01 25-delta/10-delta quote and grid goldens remain exact.
- Mixed 10-delta inputs fail in Rust builder validation.
- `try_pillar_vols` matches the old accessor for valid indices and returns `Err` for an invalid index.
- Serde round trips remain byte/field compatible.

## Focused verification

```bash
rtk cargo test -p finstack-quant-core --test market_data fx_delta_vol
rtk cargo test -p finstack-quant-core market_data::surfaces::fx_delta_vol_surface
rtk cargo test -p finstack-quant-core --test serde fx_delta_vol_surface
```

## Full verification

```bash
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Binding / parity / serde impact

- Python/WASM bindings continue using existing constructors in this slice; host symbols and parity pins do not change.
- `FxDeltaVolSurface` serde fields and `MarketContextState` remain unchanged.
- Reusing the public builder name for the named type is a deliberate Rust semver change; the old generic converter becomes private.

## Rollback

- Revert before C09, or revert C09 first. Serialized surfaces require no migration.

## Done criteria

- The public builder’s name and return type agree.
- Generic strike-grid conversion has one private implementation reachable from the named surface.
- A non-panicking pillar API exists for C09.

## Targeted re-audit acceptance

```bash
rtk rg -n "struct FxDeltaVolSurfaceBuilder|fn build\\(|struct .*Grid.*Builder|try_pillar_vols" finstack-quant/core/src/market_data/surfaces
```

Confirm the public builder builds `FxDeltaVolSurface` and the generic-grid converter is not publicly re-exported.
