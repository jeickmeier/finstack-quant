# Consolidation Plan B18: Normalize swaption underlier at the serde boundary

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F25
- **Risk tier:** Tier 4 — serde-sensitive instrument-state normalization
- **Estimated net LOC:** -30 to +120
- **Dependencies:** B12, B16, B17
- **Branch:** `codex/simplicity-b18-swaption-underlier`
- **Commit subject:** `refactor(valuations): normalize swaption underlier`
- **Parallel / merge safety:** Final Cluster B implementation slice. Conflicts with rates dependency/override waves and swaption model work; rebase after B12/B16/B17 and land alone.

## Scope

Make fully constructed fixed and floating swap legs the sole runtime underlier representation for `Swaption`. Continue accepting legacy scalar frequency fields at the serde boundary, normalize them immediately into complete legs, and remove runtime fallback logic and dual-state invariant checks.

### Exact files

- `finstack-quant/valuations/src/instruments/rates/swaption/types/swaption.rs`
- `finstack-quant/valuations/src/instruments/rates/swaption/types/bermudan.rs`
- `finstack-quant/valuations/src/instruments/rates/swaption/metrics/implied_vol.rs`
- `finstack-quant/valuations/tests/instruments/swaption/common.rs`
- `finstack-quant/valuations/tests/calibration/swaption_vol.rs`

### Non-goals

- No swaption payoff, settlement, exercise, annuity, implied-volatility, or curve-selection change.
- No removal of legacy wire fields until a separately approved schema version change.
- No second runtime legacy-fields struct stored alongside the legs.

## Invariants

- Legacy scalar payloads and full-leg payloads normalize to equal runtime instruments.
- Conflicting dual representations fail deterministically at deserialization.
- All pricers and metrics consume the same normalized legs.
- Canonical serialization emits one unambiguous underlier representation.

## Implementation steps

1. Add a private wire representation in `swaption.rs` that accepts legacy frequencies or complete legs.
2. Validate conflicts and normalize both accepted forms into required fixed/floating runtime legs.
3. Remove optional runtime legs, scalar runtime fields, fallback leg construction, and both-or-neither invariant branches.
4. Update Bermudan, implied-volatility, calibration, and common test literals to build complete legs.
5. Add legacy-input, canonical-output, equality, conflict-rejection, and pricing-regression tests.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations swaption --lib
rtk cargo test -p finstack-quant-valuations swaption --tests
rtk cargo test -p finstack-quant-valuations --test calibration swaption_vol
rtk mise run rust-check-schemas
rtk uv run pytest finstack-quant-py/tests/test_valuations_pricing.py -k swaption -q
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

Legacy JSON fields remain accepted in Rust, including existing Python fixtures using `fixed_freq` and `float_freq`; canonical output uses complete legs. Binding constructors/stubs should expose the normalized canonical shape while compatibility input stays in the Rust serde adapter.

## Rollback

Revert the PR. Canonical full-leg payloads remain readable by the pre-PR code, which already accepts optional full legs.

## Done criteria

- Runtime `Swaption` stores required complete legs only.
- No scalar-field fallback exists in pricing or metrics.
- Legacy payloads remain accepted and canonicalize predictably.
- Pricing/calibration regressions and full verification are green.

## Targeted re-audit acceptance

```sh
rtk rg -n 'fixed_freq|float_freq|underlying_fixed_leg: Option|underlying_float_leg: Option' \
  finstack-quant/valuations/src/instruments/rates/swaption
```

Matches are confined to the private serde wire adapter and compatibility tests; no runtime/pricer fallback remains. Re-run the Cluster B F1/F9/F11/F17/F19/F25/F30/F31 and H1/H12 searches and require all planned acceptance conditions to pass.
