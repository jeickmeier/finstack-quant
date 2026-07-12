# C07 — Delegate Valuations Option Formulas to Core

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F18; closes H12 after C06
- **Tier:** 4
- **Estimated net LOC:** approximately -350 to -500, deletion-dominated
- **Dependencies:** C06
- **Branch:** `codex/simplify-c07-delegate-valuations-option-formulas`
- **Parallel / merge safety:** sequential after C06. Can merge in parallel with the C04/C05 and C08/C09 chains. Conflicts broadly with valuations closed-form/volatility model edits.

## Exact files

- `finstack-quant/valuations/src/models/volatility/black.rs`
- `finstack-quant/valuations/src/models/volatility/normal.rs`
- `finstack-quant/valuations/src/models/closed_form/vanilla.rs`
- `finstack-quant/valuations/src/models/closed_form/asian.rs`

## Scope

- Retain valuations’ public functions as thin compatibility/policy facades while deleting duplicate mathematical bodies.
- Delegate spot and forward d1/d2, Black-76 call/put, Bachelier price/distance, and Black-Scholes price to C06 core kernels.
- Map raw core Greeks to existing `BsGreeks` units: vega/rhos per 1% and theta per configured day basis.
- Delegate only the exact fixed-strike, equally spaced discrete geometric-Asian overlap; preserve distinct variants.

## Non-goals

- Do not rename public Rust, Python, or WASM valuation functions.
- Do not change checked-host validation, implied-vol solvers, Greek units, discount/annuity/notional scaling, or specialized Asian models.

## Implementation steps

1. Replace `volatility/black.rs` calculations with core calls and retain re-export compatibility.
2. Make `normal.rs::bachelier_price` dispatch call/put and apply annuity around core unit-annuity values.
3. Make `vanilla.rs` dispatch option type, retain checked wrappers, and scale core raw Greeks exactly once.
4. Make overlapping Asian call/put wrappers delegate to core; keep DF overrides and nonuniform/floating/arithmetic routines local.
5. Delete now-unused local state, CDF/PDF, d1/d2, and formula helpers.

## Behavior / golden tests

- C01/C06 cross-implementation tests become single-owner regression tests without tolerance changes.
- Python/WASM `bs_price` and `bs_greeks` outputs remain identical, including host error behavior.
- Instrument-level Black, normal, and Asian pricing tests retain PV and Greek units.

## Focused verification

```bash
rtk cargo test -p finstack-quant-valuations models::closed_form
rtk cargo test -p finstack-quant-valuations models::volatility
rtk cargo test -p finstack-quant-valuations --test sanity_invariants
rtk cargo test -p finstack-quant-wasm valuations::analytic
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

- Python/WASM symbols, argument order, return DTOs, `index.d.ts`, Python stubs, and parity-contract entries remain unchanged.
- Bindings continue to call checked valuations facades; no financial dispatch moves into host code.
- No serde impact.

## Rollback

- Revert C07 to restore local formulas while retaining additive C06 core APIs. Revert C06 only after C07.

## Done criteria

- Valuations contains policy/scaling adapters, not duplicate probability or pricing kernels.
- Every overlapping function delegates to core, and specialized non-overlaps remain clearly documented.

## Targeted re-audit acceptance

- Inspect the four files and confirm overlapping functions contain dispatch/scaling only.
- Re-run F18’s search and find one computational owner for Black-76, Bachelier, Black-Scholes, d1/d2, Greeks, and the exact geometric-Asian overlap.
