# C06 — Make Core Canonical for Option Kernels

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F18, H12
- **Tier:** 4
- **Estimated net LOC:** +190 to +260
- **Dependencies:** C01
- **Branch:** `codex/simplify-c06-canonical-core-option-kernels`
- **Parallel / merge safety:** may run in parallel with C02, C03, and C08 after C01. C07 depends on this API. Conflicts with core volatility-pricing work.

## Exact files

- `finstack-quant/core/src/math/volatility/pricing/black.rs`
- `finstack-quant/core/src/math/volatility/pricing/bachelier.rs`
- `finstack-quant/core/src/math/volatility/pricing/mod.rs`
- `finstack-quant/core/src/math/volatility/mod.rs`
- `finstack-quant/valuations/tests/sanity_invariants/test_cross_impl_parity.rs`

## Scope

- Adjudicate H12 in core: exact-ATM degenerate Black-76 uses `d1=d2=0`, call delta `0.5`, and put delta `-0.5`; strict ITM/OTM degeneracies retain `±∞` and digital limits.
- Add canonical Black-76 d1/d2 pair accessors and canonical Bachelier standardized-distance access.
- Add raw Black-Scholes/Garman-Kohlhagen Greeks in core with explicit units: vega/rhos per unit parameter and theta per year.
- Add the exact fixed-strike discrete geometric-Asian put counterpart to core’s existing call kernel.

## Non-goals

- Do not redirect valuations callers yet; C07 owns delegation and deletion.
- Do not merge arithmetic, floating-strike, nonuniform-fixing, Monte Carlo, barrier, or instrument scaling logic.
- Do not change option prices at zero time/volatility or invalid-domain checked behavior beyond the documented H12 delta convention.

## Implementation steps

1. Centralize Black state creation and expose d1/d2 without recomputation.
2. Implement the exact-ATM half-delta convention consistently in d1/d2 and delta helpers.
3. Expose Bachelier `d` from the existing private state.
4. Add a core raw-Greeks DTO/function with unit documentation.
5. Add geometric-Asian put using put-call parity or the shared adjusted state.
6. Update C01’s H12 characterization from divergence to the adjudicated convention.

## Behavior / golden tests

- QuantLib ordinary-domain price goldens remain unchanged.
- Exact-ATM zero-vol/zero-time tests assert half delta; ITM/OTM limits remain 1/0 for calls.
- Greeks match finite differences with raw-unit scaling.
- Geometric-Asian call/put parity and existing call golden remain unchanged.

## Focused verification

```bash
rtk cargo test -p finstack-quant-core volatility::pricing
rtk cargo test -p finstack-quant-core --test golden_tests vol_models
rtk cargo test -p finstack-quant-valuations --test sanity_invariants
```

## Full verification

```bash
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Binding / parity / serde impact

- New core primitives are Rust canonical kernels, not new host-language calculation names.
- Python/WASM valuations APIs, stubs, `index.d.ts`, and parity pins stay unchanged until and after C07 because their facades retain existing names.
- No serde impact; the raw-Greeks DTO need not derive serde.

## Rollback

- Revert before C07, or revert C07 first if already landed. No persisted data migration.

## Done criteria

- Core can supply every exact overlapping Black-76/Bachelier/spot-Black/geometric-Asian primitive needed by C07.
- H12 has one documented, tested convention.

## Targeted re-audit acceptance

- Verify `d1_black76(F,F,0,T) == 0`, `black_delta_call(F,F,0,T) == 0.5`, and the same convention for zero expiry.
- Confirm the new core Greek units are stated at every public boundary and require no valuations-specific type.
