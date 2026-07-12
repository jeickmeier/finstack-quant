# C05 — Finish Mortgage Mortality Ownership Migration

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F5, H7
- **Tier:** 4
- **Estimated net LOC:** -120 to -220
- **Dependencies:** C04
- **Branch:** `codex/simplify-c05-finish-mortality-migration`
- **Parallel / merge safety:** sequential after C04. Conflicts with MBS prepayment and structured-credit assumptions/default-model work only.

## Exact files

- `finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/prepayment.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/types/mod.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/pricing/stochastic/default/copula_based.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/assumptions.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/utils/rates.rs`

## Scope

- Delete MBS’s CPR/SMM copies and use the checked core kernel.
- Move structured-credit static/base-rate conversion to checked validation paths; use explicit clamped adapters only for deliberately bounded stochastic outcomes.
- Route CDR/MDR through the same core annual/monthly mortality kernel.
- Make structured-credit PSA/SDA functions parameterize C03’s convention types from the assumptions registry rather than reimplementing segments.
- Remove unused reverse-conversion copies and legacy generic clamping helpers.

## Non-goals

- Do not remove or rename structured-credit assumption JSON fields; they remain legitimate configurable parameters.
- Do not change default/prepayment model serde variants, deal templates, risk bump sizes, or stochastic calibration data.

## Implementation steps

1. Replace MBS local functions and keep its public behavior checked.
2. Update `types/mod.rs` so validated static inputs use the checked kernel; propagate existing validation errors rather than silently clamping.
3. Give stochastic default bounding an explicit `cdr_to_mdr_clamped` boundary in the copula model.
4. Construct `PsaConvention`/`SdaConvention` from registry values in `assumptions.rs` and delegate curve evaluation to core.
5. Remove duplicate power formulas and obsolete reverse helpers from structured-credit rates.

## Behavior / golden tests

- MBS and cashflows remain identical for valid and invalid conversion matrices.
- Static structured-credit invalid rates reject during validation.
- Deliberately shocked stochastic rates preserve prior clamped results through explicitly named adapters.
- PSA/SDA month-boundary goldens and registry override tests remain unchanged.

## Focused verification

```bash
rtk cargo test -p finstack-quant-valuations mbs_passthrough::prepayment
rtk cargo test -p finstack-quant-valuations structured_credit
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

- No Python/WASM symbol or parity change.
- Existing assumption and model serde shapes remain identical; embedded registry values remain configurable inputs, while core owns the curve mathematics.

## Rollback

- Revert this PR; C03/C04 can remain because their APIs are additive and compatible.

## Done criteria

- One core formula owns CPR/SMM and CDR/MDR.
- One core parameterized implementation owns PSA/SDA segments.
- No public or generically named structured-credit rate function silently clamps invalid inputs.

## Targeted re-audit acceptance

```bash
rtk rg -n "powf\\(1\\.0 / 12\\.0\\)|pub fn (cpr_to_smm|smm_to_cpr|cdr_to_mdr|mdr_to_cdr)" finstack-quant/valuations/src/instruments/fixed_income
```

Any remaining domain aliases must be thin delegates with explicit checked/clamped policy and no duplicate computation.
