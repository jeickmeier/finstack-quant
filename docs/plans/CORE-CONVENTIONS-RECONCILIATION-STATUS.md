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
| 11 | C1 | **C1a and C1b committed** (`C1a:`, `C1b:` in git log) | C1a: workspace rust-lint; rust-test 16346; core doctests 286; gen-write; python-test 14003; wasm-test 1086 | 004 entry: fallible membership/counting recorded | C1 split: C1a = fallible `HolidayCalendar` trait + propagation; C1b = engine semantics (nominal-year gating, reach, checked arithmetic, ordinals, missing-table errors, cache) |
| 12 | C2 | committed (`C2:` in git log); partial | core lint; core tests 2226; gen-write; gen-check; py test_core_parity 134, parity 6067 | Calendar-input admission entry rewritten to the remaining work | Done: strict JSON admission + explicit controls (28 files) + ID validation + CSV widths. Not done: `CalendarMetadata` purpose/ranges/edition extension (004 #8), generator-level semantic checks, independent table ranges |
| 13 | H2 | committed (`H2:` in git log) | workspace lint; rust-test (9 fixture fixes re-run: exotics 314, core count test); gen-write; python-test 14003; wasm-test 1086; ui-check | 004/005 entry updated (cme/cnbe removed; NERC settlement use rejected); record 005 partly reconciled | Commodity example uses `usny` as an illustrative contract calendar; NERC purpose restriction is enforced at the commodity forward only (metadata purposes pending) |
| 14 | C3 | committed (`C3:` in git log); partial | gen-write; workspace rust-test 16350; python-test 14003; wasm-test 1086; ui-check; py test_runtime_docs 3684 | 006 entry rewritten to the remaining work; record 006 partly reconciled | Done: no `BusinessDayConvention` default (enum, cashflow schedule/fee specs, statements `SwapConventions`, Python/WASM tenor rolls), empty `+` joint members rejected, Modified rolls compare year+month. Not done: resolver fallbacks (with 011 FX callers), 100-day cap, composite coverage; product-level defaults (see Needs review) |
| 15 | C4 | committed (`C4:` in git log) | core lint; core tests 2235 + doctests 286; gen-write; workspace rust-test 16359; python-test 14003; wasm-test 1086 | 006 entry: search items moved to reconciled | Cap and `max_days` removed; Modified forward search stays in the original year/month then reverses from the original date; Nearest alternates sides and stops on proof; `Unadjusted` returns before classifying; tests: 120-day closure, fully closed month, same month a year later, unneeded next-month/preceding data, needed missing data, `i32::MIN` |
| 16 | D1 | committed (`D1:` in git log) | lint core/analytics; core tests 2237 + doctests 287; analytics 262; gen-write; workspace rust-test 16361; python-test 14003; wasm-test 1086; ui-check | 001 part of the actual-day/coupon-context entry rewritten to qualification only; evidence-prose entry: Bus252 prose and 4.16(d)/(e) citations done | New public `DayCount::validate_context` (structure, then requirements) runs before every equal-date zero; analytics CAGR guard delegates to it; `to_ctx` revalidates public fields; Bus252 `0 <= B <= D` (`Error::Internal` for a defective calendar count); ISDA integer years + two partials; summation-order test replaced by validation/date-range tests |
| 17 | D2 | committed (`D2:` in git log) | workspace rust-lint; core tests 2236 + doctests 287; fixed-income 1253; gen-write; workspace rust-test 16360; python-test 14003; wasm-test 1086; ui-check | 002 part of the actual-day/coupon-context entry: ACT/365L items recorded as landed; record 002 partly reconciled | Coupon-end-year rule for all frequencies; frequency no longer required or used (bond `act365l_year_fraction`/`act365l_flow_times` lose the parameter); containment checked before equal-date zero; A009/ISDA 2021 4.6.1(ix) citations in source, Python, WASM, notebook and golden fixture. Values that moved (366 to 365, or 365 to 366): 2 golden cases, 4 core integration tests, 6 product tests (each the 2023-12 to 2025-01-01 annual long-front case) |
| 18 | D3 | committed (`D3:` in git log) | workspace rust-lint; gen-write; workspace rust-test 16360; core doctests 285; python-test 13992; python-lint; wasm-test 1085; wasm-lint; ui-check; notebooks schedule_building and day_count_conventions | 30/360 entry rewritten as Evidence (Reconciliation 57→56, Evidence 67→68); **day-count evidence-prose entry deleted** (totals 594→593, Observed 170→169, Core conventions 20→19); record 003 reconciled; README/financial README/central follow-ups/INVARIANTS updated | New `DayCount::Thirty360Isda` (`30_360_isda`); `end_is_termination_date: Option<bool>` (omitted = unspecified), required by 30E/360 ISDA incl. equal dates; signed reversal keeps it; exact `i64` numerators; raw `days_30_360`/`days_30e_360_isda`/`Thirty360Convention` removed from Rust, Python, WASM, parity ledger, notebooks and docs-site. **[data]** fixture regenerated by running QuantLib 1.43 (`core/tests/golden/generate_daycount_quantlib.py`, 37 cases: 26 QuantLib, 11 hand ICMA/ACT365L, status provisional); QuantLib agreed with every accepted formula. Overnight projection inputs carry `end_is_termination_date` |
| 19 | E1 | committed (`E1:` in git log) | workspace rust-lint; gen-write; workspace rust-test 16361; core doctests 285; python-test 13993; python-lint; wasm-test 1085; wasm-lint; ui-check | 014/007 entry: 007 core arithmetic recorded, remaining narrowed; 002 tenor-bridge item marked landed; record 007 partly reconciled | Representation limits (D u32, W u32/7, M i32, Y i32/12; 200-year cap gone); `to_years` → `to_years_approx` in Rust, Python and WASM (`toYearsApprox`); exact `to_days_approx`; `payments_per_year -> Result` rejecting D/W (about 20 callers propagate); Hull-White swaption cashflows pick their payoff annuity at construction and look up the nominal frequency only for cash par-yield (O5); `date_from_epoch_days -> Result`; WASM `toDaysApprox` returns an exact number; `to_years_with_context` takes the caller's context (Python `ctx=`, WASM trailing optional `ctx`) and errors on an end before the origin |
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
- **Phase C** (after C4 `96d1d0a2ec`): `mise run all` **pass** on the second
  run. The first run failed only
  `test_portfolio_gil_release.py::test_sensitivity_result_conversion_and_decomposition_release_gil`
  (a background-thread progress timing probe under full-gate load). It passed
  3/3 in isolation and the full rerun passed (Python 14003).
- **Phase D** (after D3 `6645922848`): `mise run all` **pass** on the second
  run. The first run failed only `rust-check-schema-residue`: a D1 test named a
  loop variable `dc` (a retired abbreviated identifier). Fixed in the separate
  commit "D1 follow-up: rename a test variable..."; the rerun passed (Python
  13992, nextest 16360).

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
- **C3 scope boundary.** Removing `BusinessDayConvention::default` exposed
  `valuations::serde_defaults::bdc_modified_following` and two
  variance-swap observation defaults. These are product-level instrument
  defaults across many instruments; whether each product's wire form
  should require the convention is a product-owner choice, so they were
  left in place and are recorded as remaining in the 006 register entry.
  The core enum, the cashflow schedule/fee specs, the statements
  `SwapConventions` and the host tenor rolls no longer default.
- **D1 representation choices.** The equal-date requirement check covers
  presence of required fields plus the ICMA reference-grid and frequency-unit
  checks the analytics guard used to make; ACT/365L accrual containment in
  its `coupon_period` stays in the kernel (002/D2 territory). A calendar
  returning a business-day count outside `0..=elapsed days` is reported as
  `Error::Internal` (a calendar contract violation, not a caller input). The
  ACT/ACT ICMA kernels keep their own equal-date shortcuts until the 002
  slices.
- **D3 termination status at product boundaries.** Record 003 #4 forbids
  inferring 30E/360 ISDA termination status. Schedule-built contexts
  (cashflow periods, LSMC replay, statements residual schedules) pass their
  known `Some(period.end_is_termination_date)`. The IRS overnight leg passes
  `Some(is final period)`. `Tenor::to_years_with_context` and the other
  overnight-projection callers (TRS, revolver, basis and cross-currency swaps,
  cap/floor, IR futures, benches) have no schedule fact and pass `None`, so a
  30E/360 ISDA day count on those paths now errors instead of silently using
  `false`. Plumbing their termination facts is product work (the 003 entry
  keeps product mapping with the product records).
- **D3 wire form.** Following 003 #4 ("omission meaning unspecified"), an
  omitted `end_is_termination_date` in `DayCountContextState` JSON now loads as
  `None`; previously the key was required as a `bool`. The core test asserting
  required presence was changed to assert the omission rule.
- **E1 tenor-bridge callers.** The seven scenario curve-clock callers of
  `to_years_with_context` previously had a frequency and coupon period invented
  from the tenor. They now pass the empty `DayCountContext`, so a curve whose
  day count needs context (ICMA, ACT/365L, Bus/252, 30E/360 ISDA) errors there
  rather than measuring with invented context. The endpoint calendar is no
  longer reused as the Bus/252 counting calendar (record 007 #6 keeps the
  roles separate).
- **A1 not closed.** The 020 register entry stays open as Evidence. Equality,
  adjacent and already-beyond observations, window/history behavior, and
  settlement/discount boundaries at each consumer were not independently
  qualified in this slice.
