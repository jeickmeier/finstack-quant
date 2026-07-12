# C04 — Make Structured Prepayment Clamping Explicit

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F5, H7 (prepayment half)
- **Tier:** 4
- **Estimated net LOC:** -20 to +40
- **Dependencies:** C03
- **Branch:** `codex/simplify-c04-explicit-structured-prepayment-clamping`
- **Parallel / merge safety:** sequential after C03 and before C05. Conflicts with structured-credit stochastic-prepayment work, but not with C02, C06/C07, or C08/C09.

## Exact files

- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/utils/rates.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/pricing/stochastic/prepayment/richard_roll.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/pricing/stochastic/prepayment/factor_correlated.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/pricing/stochastic/prepayment/regime_switching.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/pricing/stochastic/prepayment/spec.rs`

## Scope

- Replace the local CPR/SMM computation with the C03 checked core kernel.
- Give stochastic shock bounding an explicit name such as `cpr_to_smm_clamped`; clamp once at the modeling boundary, then call the checked kernel.
- Migrate every stochastic-prepayment caller to the explicit policy name.

## Non-goals

- Preserve current stochastic outputs; this slice does not change shocks, burnout, regimes, correlations, or RNG behavior.
- Do not migrate MBS, static structured-credit types, CDR/MDR, PSA/SDA assumptions, or public exports; C05 owns those.

## Implementation steps

1. Add a crate-private explicit clamped adapter delegating to `annual_mortality_to_monthly`.
2. Keep deliberate `.clamp(0.0, 1.0)` at one named stochastic boundary rather than inside the generic rate function.
3. Update Richard-Roll, factor-correlated, regime-switching, and stochastic-spec callers.
4. Remove duplicated CPR arithmetic from `utils/rates.rs` while retaining temporary compatibility delegates needed by C05.

## Behavior / golden tests

- C01’s valid conversion matrix remains exact.
- Shocked CPR below 0 still produces SMM 0; above 1 still produces SMM 1, now through an explicitly named path.
- Base/shocked stochastic prepayment outputs remain unchanged for fixed fixtures.

## Focused verification

```bash
rtk cargo test -p finstack-quant-valuations structured_credit::pricing::stochastic::prepayment
rtk cargo test -p finstack-quant-valuations structured_credit::utils::rates
```

## Full verification

```bash
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Binding / parity / serde impact

- No Python/WASM symbol, parity-contract, or stub change.
- No serde change; stochastic model specifications retain their existing fields and values.

## Rollback

- Revert the PR to restore the local clamping function; no persisted state changes.

## Done criteria

- Every stochastic prepayment clamp is visible at the call boundary by name.
- No stochastic caller invokes a generic function that silently clamps invalid CPR.

## Targeted re-audit acceptance

```bash
rtk rg -n "\\bcpr_to_smm\\(" finstack-quant/valuations/src/instruments/fixed_income/structured_credit/pricing/stochastic/prepayment
```

All production hits must use the explicit clamped adapter or the checked core kernel.
