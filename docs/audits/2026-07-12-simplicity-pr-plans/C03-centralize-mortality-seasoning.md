# C03 — Centralize Mortality and Seasoning Kernels

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F5; prepares H7
- **Tier:** 4
- **Estimated net LOC:** +80 to +160
- **Dependencies:** C01
- **Branch:** `codex/simplify-c03-centralize-mortality-seasoning`
- **Parallel / merge safety:** may run in parallel with C02, C06, and C08. C04 and C05 depend on this public core kernel.

## Exact files

- `finstack-quant/core/src/credit/mortality.rs` (new)
- `finstack-quant/core/src/credit/mod.rs`
- `finstack-quant/cashflows/src/builder/credit_rates.rs`
- `finstack-quant/cashflows/src/builder/specs/prepayment.rs`
- `finstack-quant/cashflows/src/builder/specs/default.rs`

## Scope

- Add one checked annual-to-monthly mortality transform and inverse in core, usable for both CPR/SMM and CDR/MDR.
- Use `ln_1p`/`exp_m1` forms for stability near zero and validate finite unit-interval inputs.
- Add parameterized PSA and SDA convention values/functions in core; standard defaults remain 30 months/6% CPR and 30-month peak/60-month plateau/120-month decline/0.60%-to-0.03% CDR.
- Make cashflows’ existing public CPR/SMM functions thin compatibility delegates and make prepayment/default specs use the core conventions.

## Non-goals

- Do not yet migrate MBS or structured-credit callers.
- Do not change cashflow public names, JSON/serde variants, speed-multiplier semantics, or invalid-input policy.

## Implementation steps

1. Implement `annual_mortality_to_monthly` and `monthly_mortality_to_annual` with shared validation.
2. Implement `PsaConvention` and `SdaConvention` as parameter objects rather than scattered constants.
3. Preserve exact boundary behavior at 0 and 1 and return errors for NaN, infinities, and rates outside `[0, 1]`.
4. Delegate `cashflows::builder::{cpr_to_smm,smm_to_cpr}` to core.
5. Replace hard-coded PSA/SDA segment arithmetic in the two cashflow specs with the standard convention methods.

## Behavior / golden tests

- Use C01 conversion and month-boundary tables unchanged.
- Add core round trips at 0, ordinary rates, near-1, and 1.
- Assert PSA months 0/1/30/31 and SDA months 0/30/60/61/120/121 retain prior values.

## Focused verification

```bash
rtk cargo test -p finstack-quant-core mortality
rtk cargo test -p finstack-quant-cashflows credit_rates
rtk cargo test -p finstack-quant-cashflows prepayment
rtk cargo test -p finstack-quant-cashflows default
```

## Full verification

```bash
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Binding / parity / serde impact

- The new core kernel is Rust-only infrastructure; do not add Python/WASM aliases for CPR/CDR terminology.
- Existing cashflow JSON and host surfaces remain unchanged.
- No serde field or enum representation changes. If structural parity detects the new core module, classify it as an intentional Rust numerical kernel rather than adding transport-only wrappers.

## Rollback

- Revert the PR; cashflow delegates return to their prior local formulas without wire migration.

## Done criteria

- Core contains the only checked annual/monthly formula and the only PSA/SDA segment formulas used by cashflows.
- Cashflow compatibility functions contain no independent exponentiation or validation policy.

## Targeted re-audit acceptance

```bash
rtk rg -n "powf\\(1\\.0 / 12\\.0\\)|0\\.06.*30|0\\.006|0\\.0003" finstack-quant/cashflows/src/builder
```

Any remaining hits must be docs/tests or constructors, not computational branches.
