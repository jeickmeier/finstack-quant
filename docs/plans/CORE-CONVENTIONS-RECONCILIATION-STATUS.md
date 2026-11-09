# Core conventions reconciliation: run status

**Plan:** [CORE-CONVENTIONS-RECONCILIATION-PLAN.md](CORE-CONVENTIONS-RECONCILIATION-PLAN.md) (frozen).
**Branch:** `refactor/core-conventions-reconciliation`, cut from local `master` at `c5c5a0a56e`.
**Worktree:** `.claude/worktrees/core-conventions`.
**Started:** 2026-11-08.

## Summary

Run in progress.

## Baseline

`mise run all` at `c5c5a0a56e`: **pass** (second attempt). The first attempt
failed only in `wasm-fmt`/`wasm-lint` because the fresh worktree had no
`node_modules` (`prettier: command not found`). After `npm ci` in
`finstack-quant-wasm`, `finstack-quant-ui` and `docs-site`, the full gate passed
(Python 14017 passed / 112 skipped; nextest 16347 tests). No pre-existing failures.

## Slices

| # | Slice | Status | Gates | Register | Notes |
|---|---|---|---|---|---|
| 1 | A1 | committed (`A1:` in git log) | lint core/models/exotics/fx/valuations; tests core 2228, models 2077, exotics 314, fx 251, valuations 2453; gen-write; py-build + test_production_barrier_ndf 7; wasm-test 1090 + dts | 020 entry rewritten as Evidence (consumer qualification remains); Reconciliation 59→58, Evidence 66→67 | `rebate_timing: Option<PayoutTiming>` validated with the rebate via new `models::closed_form::validate_rebate_timing`; `expired_barrier_payoff` takes `Option<(f64, PayoutTiming)>` |
| 2 | A2 | committed (`A2:` in git log) | lint core/py/wasm; core tests; gen-write; py-build + test_dates_imm 29, test_core_wasm_parity 85, parity 6071; wasm-test | Roll-date entry trimmed (option-expiry action done); record 009 item 7 marked reconciled | Removed from core, Python, stubs, WASM, facade, parity, notebook, docs-site lesson and tracks |
| 3 | A3 | committed (`A3:` in git log) | lint core/fixed-income/py/wasm; tests core 2219, fixed-income 1252; gen-write; py test_dates_imm 30, test_core_wasm_parity 83, test_core_parity 133, test_runtime_docs 3684, parity 6069; wasm 1086 | SIFMA entry rewritten to the remaining work (A3b, data pin/admission H5, qualification); record 010 partly reconciled | Missing cells are `InputError::NotFound` (Python `KeyError`, JS `not_found`); out-of-range years are validation errors checked first |
| 4 | A3b | committed (`A3b:` in git log) | lint core/fixed-income/valuations/py/wasm; tests core 2218, fixed-income 1252, valuations 2453 (after gen-write); py test_dates_imm, test_core_parity, test_runtime_docs, parity 6067, test_production_audit 28; wasm 1086 | SIFMA entry: agency/term classification done; remaining data pin/admission (H5) and qualification | `settlement_class` is a required field on AgencyTba and DollarRoll (O2); hand fixture production_mortgage.json given class `a` (FNMA 30Y) |
| 5 | B1 | committed (`B1:` in git log) | workspace rust-lint; core tests 2218 + 286 doctests; gen-write; py test_core_parity 134, members 6, ergonomics 48, calibration boundaries 45, core_wasm_parity 83, runtime_docs 3684, parity 6067; wasm 1086 | 018 entry narrowed to the term-loan step-up (B1b) | Only bindings consumed the changed API outside core; a CDS quote test now compares against the single binary64 `as_bp` view instead of noise-free decimal shifting (018 #6) |
| 6 | B1b | committed (`B1b:` in git log) | lint fixed-income; tests fixed-income 1253, covenants 115 | **018 entry deleted**; totals 595→594, Reconciliation 58→57, Core conventions 21→20; record 018, README, INVARIANT_MAP M23–M26, INVARIANTS, financial README, central follow-ups updated | Step-up computed fully before mutation; negative/non-finite increases rejected |
| 7 | B2 | **B2a committed** (`B2a:` in git log); **B2b blocked (owner decision)** | B2a: workspace rust-test (16346 incl. 2 re-pins), gen-write, python-test 14003, wasm-test 1086, ui-check | 012 entry: B2a items recorded; item 4 marked blocked | B2b (shortest-text float admission) breaks 55 tests on sub-1e-28 model values; see Needs review |
| 8 | B3 | skipped (dep B2b) | | | |
| 9 | B4 | skipped (dep B3) | | | |
| 10 | B5 | skipped (dep B4) | | | |
| 11 | C1 | **C1a committed** (`C1a:` in git log); C1b in progress | C1a: workspace rust-lint; rust-test 16346; core doctests 286; gen-write; python-test 14003; wasm-test 1086 | 004 entry: fallible membership/counting recorded | C1 split: C1a = fallible `HolidayCalendar` trait + propagation; C1b = engine semantics (nominal-year gating, reach, checked arithmetic, ordinals, missing-table errors, cache) |
| 12 | C2 | pending | | | |
| 13 | H2 | pending | | | |
| 14 | C3 | pending | | | |
| 15 | C4 | pending | | | |
| 16 | D1 | pending | | | |
| 17 | D2 | pending | | | |
| 18 | D3 | pending | | | |
| 19 | E1 | pending | | | |
| 20 | E2 | pending | | | |
| 21 | E2b | pending | | | |
| 22 | E3 | pending | | | |
| 23 | E4 | pending | | | |
| 24 | E4b | pending | | | |
| 25 | G1 | skipped (dep B3) | | | |
| 26 | G2 | pending | | | |
| 27 | G3 | pending | | | |
| 28 | G4 | skipped (dep B3) | | | |
| 29 | F1 | pending | | | |
| 30 | F2 | pending | | | |
| 31 | F3 | pending | | | |
| 32 | F4 | pending | | | |
| 33 | F5 | pending | | | |
| 34 | F6 | skipped (dep B3) | | | |
| 35 | F7 | skipped (dep F6) | | | |
| 36 | F8 | skipped (dep F7) | | | |
| 37 | G5 | skipped (dep G4) | | | |
| 38 | E5 | pending | | | |
| 39 | E6 | skipped (dep G5, CASHFLOWS-001) | | | |
| 40 | H1 | pending | | | |
| 41 | H3 | pending | | | |
| 42 | H4 | pending | | | |
| 43 | H5 | pending | | | |
| 44 | H6 | skipped (dep B5) | | | |

## Phase gates

- **Phase A** (after A3b `b7248fccbf`): `mise run all` **pass**.
- **Phase B** (after B2a `69c2dc0d3a`; B3–B5 skipped): `mise run all` **pass**.

## Judgement log

- **Setup precondition.** The other session's register edits were committed to
  local `master` in `c5c5a0a56e` before the branch was cut. The main
  checkout still has one uncommitted file,
  `docs/decisions/financial/cashflows/05-coupon-cash-and-pik.md`. That is a
  cashflows record that no slice in the queue edits, so it is not one of the
  register files the precondition protects, and the run proceeds. **Rule for
  the rest of the run:** do not edit that file. If a slice needs to change
  it, mark the slice `blocked (concurrent edit)`.

- **Tooling.** In this worktree, git must be invoked as
  `/usr/bin/git -C <worktree>`, because the rtk-wrapped git is refused by the
  isolation hook. The commit pre-commit hook runs the full workspace
  `rust-lint` and acts as an extra per-slice gate. `mise run python-test-file`
  accepts exactly one path.

## Needs review

- **A1 representation choice.** Record 020 #2 says no timing may be invented
  for cash that does not exist. So `rebate_timing` became
  `Option<PayoutTiming>`, required exactly when `rebate` is set, rather than an
  always-required field. Absence on the wire means "no rebate". Knock-in
  `AtHit` is rejected in product validation (O1). The analytic helpers return
  their existing NaN sentinel for it instead of repricing at expiry.
- **B2 split; B2b blocked (owner decision needed).** Record 012 #4 requires
  every `f64 -> Decimal` adapter to use shortest round-trip text and to
  reject any value outside the Decimal domain, including nonzero `1e-29`.
  Implemented as specified, it broke 55 workspace tests. Most of them are
  pricers that place a negligible model value (deep out-of-the-money option
  PVs, Monte Carlo/tranche expected losses, CMS replication, structured-credit
  cash flows) into `Money` and now get `ConversionOverflow`. The rest are
  bit-pinned values that moved: shortest-text admission differs from
  `Decimal::try_from(f64)`. No record says what a pricer should do with a
  sub-`1e-28` model value: error, an owner-selected financial rounding
  stage before `Money`, or computing in `f64` until a final boundary. So the
  converter change is held back as **B2b** and the run kept going with
  **B2a** (exact text grammar, exact `DecimalWire`, exact Python `int`,
  zero normalization). Slices that build on the float converter
  (B3 named f64 adapters onward) are affected; see their rows.
- **A1 not closed.** The 020 register entry stays open as Evidence. Equality,
  adjacent and already-beyond observations, window/history behavior, and
  settlement/discount boundaries at each consumer were not independently
  qualified in this slice.
