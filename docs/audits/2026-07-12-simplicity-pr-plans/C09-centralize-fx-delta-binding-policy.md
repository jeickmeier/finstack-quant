# C09 — Centralize FX Delta Construction and Pillar Validation

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F20, H5
- **Tier:** 4
- **Estimated net LOC:** -60 to -120
- **Dependencies:** C08
- **Branch:** `codex/simplify-c09-centralize-fx-delta-binding-policy`
- **Parallel / merge safety:** sequential after C08. Can merge in parallel with C02, C04/C05, and C06/C07 chains. Conflicts with core/Python/WASM FX-delta binding work.

## Exact files

- `finstack-quant/core/src/market_data/surfaces/fx_delta_vol_surface.rs`
- `finstack-quant/core/tests/market_data/surfaces/fx_delta_vol_tests.rs`
- `finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs`
- `finstack-quant-wasm/src/api/core/market_data.rs`

## Scope

- Make Rust `pillar_vols` itself return `Result` and remove the panicking accessor/temporary `try_` name.
- Make Python and WASM constructors use C08’s builder, eliminating duplicated optional-10d match branches.
- Remove binding-owned pillar bounds checks and map the Rust error centrally.
- Remove `FxDeltaVolSurface::with_10d`; keep `new` as the short common constructor and the builder as the optional-wing path.

## Non-goals

- Do not change Python/WASM constructor signatures, Python exception class, WASM error transport, JS/TS names, quote ordering, interpolation, or serde.
- Do not add a second host constructor for the Rust builder.

## Implementation steps

1. Rename the checked Rust accessor to canonical `pillar_vols` and update Rust callers/tests.
2. Construct the Rust builder in each binding, set 10-delta wings only when supplied, and call `build` once.
3. Delete Python/WASM paired-option match branches and manual index guards.
4. Remove `with_10d` after all current callers use the builder.
5. Verify unchanged declaration/contract surfaces rather than editing them gratuitously.

## Behavior / golden tests

- Python out-of-range access remains `IndexError` through centralized Rust-to-Python mapping.
- WASM out-of-range access remains an error `JsValue`.
- Python and WASM mixed 10-delta arguments retain current rejection behavior.
- Rust valid pillars and all C01/C08 grid goldens remain exact.
- Verification-only surfaces: `finstack-quant-py/parity_contract.toml`, `finstack-quant-wasm/index.d.ts`, `finstack-quant-wasm/tests/dts_contract.rs`, `finstack-quant-wasm/tests/wasm_core_market_data.rs`, and `finstack-quant-py/tests/test_fx_delta_vol_surface.py`.

## Focused verification

```bash
rtk cargo test -p finstack-quant-core --test market_data fx_delta_vol
rtk cargo test -p finstack-quant-wasm --test wasm_core_market_data fx_delta_vol_surface
rtk cargo test -p finstack-quant-wasm --test dts_contract
rtk uv run pytest finstack-quant-py/tests/test_fx_delta_vol_surface.py
rtk uv run pytest finstack-quant-py/tests/parity
```

## Full verification

```bash
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Binding / parity / serde impact

- Python/WASM public signatures and export names stay unchanged; only constructor and validation ownership move to Rust.
- `finstack-quant-py/parity_contract.toml` pins, Python stubs, JS exports, and `index.d.ts` must require no edits. Any generated diff indicates accidental public drift.
- `FxDeltaVolSurface` and `MarketContextState` serde shapes remain unchanged.
- Rust removal of `with_10d` and the infallible pillar signature is a deliberate semver-visible cleanup.

## Rollback

- Revert C09 to restore binding branches and the old accessor while leaving C08’s additive builder/checking support available. Revert C08 only afterward.

## Done criteria

- Rust owns optional-wing validation and pillar bounds validation.
- Python/WASM bindings contain conversion and error mapping only.
- Host signatures, parity pins, stubs, declarations, and serde remain stable.

## Targeted re-audit acceptance

```bash
rtk rg -n "with_10d|expiry_idx >=|\\(Some\\(rr\\), Some\\(bf\\)\\)" finstack-quant/core/src/market_data/surfaces/fx_delta_vol_surface.rs finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs finstack-quant-wasm/src/api/core/market_data.rs
```

The command must return no production policy copies; `pillar_vols` must have a fallible Rust signature and both bindings must delegate to it.
