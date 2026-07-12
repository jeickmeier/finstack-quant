# Consolidation Plan A01: Make CashFlowBuilder Order-Independent

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F3; H4.
- Risk tier: Tier 3 — public Rust builder semantics change, with no intended numerical change.
- Estimated net change: +140 to +240 LOC.
- Dependencies: none.
- Suggested branch: `codex/a01-builder-order-independent`.
- Parallel and merge safety: safe beside A02, A03, A05, A06, and A10. Merge before A08 and A09. Expect conflicts in `coupon_api.rs` and `compiler.rs` if A08 or A09 starts first.
- Atomicity: normal five-file slice; no compile-atomic exception.

## Exact Files

- `finstack-quant/cashflows/src/builder/principal.rs`
- `finstack-quant/cashflows/src/builder/coupon_api.rs`
- `finstack-quant/cashflows/src/builder/compiler.rs`
- `finstack-quant/cashflows/src/builder/orchestrator.rs`
- `finstack-quant/cashflows/tests/cashflows/builder/principal_events.rs`

## Scope

- Stop `principal()` from clearing a previously recorded builder error.
- Make `amortization()` valid before or after `principal()` by retaining the requested amortization until compilation.
- Represent full-horizon coupon operations as unresolved full-horizon instructions and resolve their issue/maturity window once in `compile_plan()`.
- Make the common full-horizon methods (`fixed_cf`, `floating_cf`, `step_up_cf`, and horizon-dependent convenience programs) produce the same plan regardless of whether `principal()` was called first.
- Preserve one sticky first error until the terminal build returns it.

## Non-Goals

- No signature changes to public builder methods.
- No changes to explicit window validation or overlapping-window precedence.
- No coupon-spec serde changes; those belong to A08.
- No JSON bridge expansion; that belongs to A09.

## Implementation Steps

1. Add one internal representation for a full-horizon window/instruction instead of reading `issue` and `maturity` inside fluent methods.
2. Store an amortization request independently from principal setup and combine them when compiling the canonical `Notional`.
3. Remove `pending_error = None` from `principal()` and ensure later successful calls never erase the first error.
4. Resolve every deferred horizon against the final validated issue/maturity in `compile_plan()`; retain current validation for explicit bounded windows.
5. Delete now-redundant `issue_maturity_or_record_error` branches and update builder documentation to describe order independence and sticky errors.

## Tests to Add or Update

- Add permutation tests proving `principal().amortization()` equals `amortization().principal()`.
- Add permutation tests for fixed, floating, step-up, fixed-to-float, and margin-step-up full-horizon programs.
- Add a regression proving an invalid coupon/program call followed by `principal()` still fails with the original error.
- Retain tests proving explicit windows outside the final principal horizon fail at build time.

## Full Verification

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
```

## Binding, Parity, and Serde Impact

- Python/WASM: none; the bound JSON functions retain their signatures.
- Parity contract: no change.
- Serde/schema: no change.
- Behavioral impact: Rust and JSON callers may now order full-horizon instructions before principal without silent loss; invalid chains remain errors.

## Rollback

Revert the single slice. No persisted data migration or compatibility shim is introduced.

## Done Criteria

- No builder method clears `pending_error`.
- No full-horizon method reads issue/maturity during the fluent call.
- `amortization()` is never a silent no-op solely because principal has not been supplied yet.
- All order-permutation and sticky-error regressions pass.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` against `builder/principal.rs`, `builder/coupon_api.rs`, and `builder/orchestrator.rs`. Accept only when it finds one deferred-plan path, no principal-first temporal requirement for full-horizon operations, no swallowed error, and no replacement parallel builder API.
