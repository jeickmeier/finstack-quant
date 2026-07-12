# C01 — Lock Canonical Numerical Boundaries

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F2, F5, F18, F20; H5, H7, H12
- **Tier:** 4 — behavior characterization only
- **Estimated net LOC:** +180 to +240 test lines
- **Dependencies:** none
- **Branch:** `codex/simplify-c01-lock-numerical-boundaries`
- **Parallel / merge safety:** land first. Later Cluster C slices depend on these locks. It touches only tests or `#[cfg(test)]` blocks, but C02, C06, and C08/C09 will subsequently edit three of the same files.

## Exact files

- `finstack-quant/valuations/src/models/volatility/sabr/tests.rs`
- `finstack-quant-py/tests/test_valuations_new_bindings.py`
- `finstack-quant-wasm/src/api/valuations/sabr.rs` (`#[cfg(test)]` only)
- `finstack-quant/valuations/tests/sanity_invariants/test_cross_impl_parity.rs`
- `finstack-quant/core/tests/market_data/surfaces/fx_delta_vol_tests.rs`

## Scope

- Replace derivative-route-only SABR equivalence coverage with canonical calibrator golden cases for unshifted, auto-shifted, and ATM-pinned smiles.
- Pin Python and WASM SABR length-error category and a message fragment common to the Rust error.
- Add valid-domain CPR/SMM, CDR/MDR, Black-76, Black-Scholes, Bachelier, and geometric-Asian cross-implementation matrices.
- Explicitly characterize the current structured-credit clamping divergence, degenerate ATM Black-76 d1 divergence, and Rust FX pillar panic.
- Pin FX 25-delta/10-delta construction and generic-grid conversion outputs.

## Non-goals

- No production behavior, public symbol, serde shape, or binding signature changes.
- Do not adjudicate H7, H12, or H5 in this slice; mark the tests that later slices intentionally update.

## Implementation steps

1. Record deterministic SABR parameter and fitted-vol tolerances rather than optimizer iteration counts.
2. Move lasting coverage away from `calibrate_*_with_derivatives`; C02 must be able to delete that family without losing numerical locks.
3. Add a table-driven rate-conversion matrix including 0, ordinary rates, near-1 rates, 1, NaN, infinities, and out-of-range values.
4. Add option parity cases for ordinary moneyness, zero time, zero volatility, and exact ATM; document the current H12 disagreement in the test name.
5. Use `catch_unwind` only to characterize the current Rust `pillar_vols` failure; C09 replaces this assertion with `Result::Err`.

## Behavior / golden tests

- SABR shifted/unshifted fitted vols remain within existing calibration tolerances.
- Python and WASM invalid smile lengths return host errors before and after validation moves into Rust.
- Valid mortality conversions agree across all copies; invalid structured-credit inputs currently clamp while checked owners reject.
- Core and valuations option kernels agree away from H12.
- FX quote ordering, recovered wing vols, and strike-grid values are fixed independently of constructor plumbing.

## Focused verification

```bash
rtk cargo test -p finstack-quant-valuations sabr
rtk cargo test -p finstack-quant-valuations --test sanity_invariants
rtk cargo test -p finstack-quant-core --test market_data fx_delta_vol
rtk cargo test -p finstack-quant-wasm sabr
rtk uv run pytest finstack-quant-py/tests/test_valuations_new_bindings.py
```

## Full verification

```bash
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Binding / parity / serde impact

- Python and WASM SABR surfaces are test-only changes.
- `finstack-quant-py/parity_contract.toml`, generated stubs, `index.d.ts`, and serde payloads remain unchanged.

## Rollback

- Revert the PR; no production state or compatibility migration is involved.

## Done criteria

- All four duplicate-kernel groups have deterministic ordinary-domain locks.
- H5, H7, and H12 are visible as named, intentional characterization cases.
- No lasting SABR golden depends on the derivative calibrator being retained.

## Targeted re-audit acceptance

- Re-run the audit’s F2/F5/F18/F20 examples and confirm every behavior-changing follow-up has a pre-existing lock to update rather than inventing expected values during canonicalization.
