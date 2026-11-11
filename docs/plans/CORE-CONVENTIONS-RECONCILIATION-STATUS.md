# Core conventions reconciliation: run status

**Plan:** [CORE-CONVENTIONS-RECONCILIATION-PLAN.md](CORE-CONVENTIONS-RECONCILIATION-PLAN.md) (frozen).
**Branch:** `refactor/core-conventions-reconciliation`, cut from local `master` at `c5c5a0a56e`.
**Worktree:** `.claude/worktrees/core-conventions`.
**Started:** 2026-11-08.

## Summary

**Run complete (2026-11-09). Final gate: `mise run all` pass** at HEAD
(nextest 16358 passed, Python 13988 passed / 112 skipped). Nothing pushed or
merged; everything is on `refactor/core-conventions-reconciliation`, cut from
local `master` `c5c5a0a56e`.

**Committed slices (21 slices, 22 code/doc commits plus status commits):**

| Slice | Commit | Scope |
| --- | --- | --- |
| A1 | `b6f0dd8d61` | barrier rebate timing (020) |
| A2 | `c808334fd3` | IMM option-expiry helpers removed (009) |
| A3 | `bb0fb7b027` | explicit SIFMA class, checked lookups (010) |
| A3b | `b7248fccbf` | SIFMA agency/term classification removed; TBA class required (010, O2) |
| B1 | `c94d13be41` | Rate/Bps units, admission, precision (018) |
| B1b | `49da53a95e` | fractional-bp term-loan step-ups (018, O12) |
| B2a | `69c2dc0d3a` | exact decimal grammar and canonical decimal wire (012) |
| B2b | see `B2b:` in git log | shortest-text float admission; `Money::from_model_value` (012 #4, D1) |
| C1a | `a078dd7301` | fallible holiday membership/counting (004) |
| C1b | `8f24022396` | holiday engine year gating, reach, data errors (004) |
| C2 | `a59aaebe60` | strict calendar-input admission (004); partial |
| H2 | `92c4c64a5a` | cme/cnbe retired; commodity contract calendar (005) |
| C3 | `5e6e2d0586` | no default BDC; strict joint IDs (006); partial |
| C4 | `96d1d0a2ec` | uncapped minimal business-day searches (006) |
| D1 | `848317a970` (+ `c671891da1`) | day-count context validation before zero (001) |
| D2 | `a5023cbec1` | ACT/365L coupon-end-year rule (002) |
| D3 | `6645922848` | 30/360 package; QuantLib-regenerated fixture (003) |
| E1 | `7dbae3e236` | tenor limits, `to_years_approx`, context bridge (007) |
| E2a | `17103b7ba5` | completed months; roll calendar rule (007) |
| E2b | `4c5cb90980` | structured-credit month meanings stated (007, O13; docs) |
| E3a | `28530ab1ef` | schedules: no policies, warnings or repair (008 #12) |
| E3b-1 | `cd07652242` | strict EOM/IMM anchors, final-date ceiling (008 #5, #6, #11; 009 #10) |
| E3b-2 | `f1959df015` | private validated `Schedule` state (008 #13) |
| E3b-3 | `E3b-3:` | `GridMode`, complete `ScheduleSpec`, explicit builder (008 #2, #8, #14) |
| G2 | `63645b0593` | CreditRating ladder, partial order (019) |

Status-only commits: `b6182cf840`, the interim summary, and this one.

**Not done, with reasons:**
- **Blocked (owner decision, since resolved; see Owner decisions):** B2b
  (committed after D1); `from_years` removal in E2 (`ForwardCurve`
  carries no contractual tenor).
- **Not executed (scope):** E2c, the O6 required quote `Compounding` on Bond,
  TermLoan, RevolvingCredit and ABF (about 1,000 construction sites).
- **Not attempted (run budget):** E3b (plan L; about 700 builder sites),
  G3 (plan M/L; classify validation and embedded-data retirement are coupled
  and the embedded `sp` scale violates the representative rule), F1 (plan M;
  about 200 sites), F2–F4 (F1 chain), F5 (plan L), H1, H3, H4, H5 (data
  slices).
- **Skipped (dependency):** B3, B4, B5, G1, G4, F6, F7, F8, G5, H6 (B2b
  chain); E4, E4b, E5 (E3b); E6 (G5 and CASHFLOWS-001).

**Register changes:** deleted the 018 Rate/Bps entry (B1b) and the
day-count evidence-prose entry (D3); rewritten as Evidence: the 020 barrier
entry (A1) and the 003 30/360 entry (D3); narrowed in place to the remaining
work: 004/005 calendar-input and holiday entries, 006 rolling, 001/002
actual-day/coupon-context, 007/014 tenor/periods, 008 schedules, 010 SIFMA,
012 money, 019 ratings. Totals: 595 → 593 items; Core conventions 21 → 19.

**Needs review:** the section at the end of this file (A1 representation,
B2 split, C3 product defaults, D1 choices, D3 product termination status and
wire omission, E1 scenario curve clocks, E2 split/blocker/scenario roll,
E2c and E3b scope, G2 `indexmap` dependency, G3 coupling, and A1 not closed).

**Phase gates:** A, B, C, D and E passed (C and D on the second run, with the
cause logged under Phase gates); the final gate passed. Phases F and G did
not complete (G2 only), so the final gate stands for them.

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
| 7 | B2 | **B2a committed** (`B2a:` in git log); **B2b committed after owner decision D1** (`B2b:` in git log) | B2a: workspace rust-test (16346 incl. 2 re-pins), gen-write, python-test 14003, wasm-test 1086, ui-check. B2b: workspace rust-test 16360 (again after the sweep) + rust-test --doctest; gen-write + gen-check; python-test 13988; wasm-test-built 1085 after a full wasm-test build; ui-check 1527; dual pins and single re-pins checked in a linux/amd64 rust:1.97.1 container | 012 entry: B2a items recorded; B2b: item 4 recorded as reconciled, blocker sentence removed; record 012 status updated | B2b: shortest-text `f64_to_decimal`; `Money::from_model_value` for pricer outputs (125 failure-driven sites, then a 216-site classified sweep; follow-up commit); re-pins and test changes listed under Needs review |
| 8 | B3 | committed after owner decision D7 (`B3:` in git log) | rust-test 16,368 pass; doctests pass; python-test 13,994 pass / 112 skip; wasm-test 1,085 pass; ui-check 1,527 pass; gen-write moved no golden/fixture; Linux x86_64 container: all test binaries pass except the 3 known golden-runner container artifacts (read-only mount, no git); the core doctest run hung under Rosetta emulation and was stopped (doctests pass natively) | 012 entry: arithmetic/rate adapters recorded as reconciled; record 012 status updated | Exact-to-minor-unit sums (D7); Decimal-scalar mul/div with underflow errors and `_f64` adapters; direct `ratio`; Decimal-rate `convert_at_rate` + `convert_at_rate_f64`; Python/WASM twins; see Needs review |
| 9 | B4 | committed (`B4:` in git log) | rust-test 16,372 pass; doctests pass (last slice to run them; now nightly-tier only by owner request); python-test 13,997 pass / 112 skip; wasm-test 1,087 pass; ui-check 1,526 pass + 1 load timeout (contracts.test.ts lazy-module case) that passed on rerun (91/91); gen-write regenerated schemas/declarations/UI fixtures (default stamps lose `"mode": "bankers"`, label rename only otherwise); Linux container run skipped by owner request | 012 entry: rounding/config/mode rename/display bounds recorded as reconciled; record 012 status updated | Explicit `Money::round(scale, mode)`; `MidpointAwayFromZero` without alias; no default mode, optional policy/stamp mode; override-only ingest; fallible `from_decimal_with_config`; formatting cap 28 and sign-free zero; Python `round` replaces `__round__` (O10), WASM `round`; see Needs review |
| 10 | B5 | committed (`B5:` in git log) | rust-test 16,374 pass; generator tests 23 pass; python-test 14,002 pass / 112 skip; wasm-test 1,087 pass; ui-check 1,526 pass + the same contracts.test.ts lazy-module load timeout as B4, passing on rerun (91/91); gen-write moved only reorders (UI fixtures, gallery data); doctests and the Linux run skipped by owner request | 012 entry: currency identity recorded as reconciled; record 012 status updated | Position discriminants + metadata table; unique-only `TryFrom<u16>`; generator admits shared numerics; alphabetic `Ord` (O10) for maps and SIMM; Python hash by code; see Needs review |
| 11 | C1 | **C1a and C1b committed** (`C1a:`, `C1b:` in git log) | C1a: workspace rust-lint; rust-test 16346; core doctests 286; gen-write; python-test 14003; wasm-test 1086 | 004 entry: fallible membership/counting recorded | C1 split: C1a = fallible `HolidayCalendar` trait + propagation; C1b = engine semantics (nominal-year gating, reach, checked arithmetic, ordinals, missing-table errors, cache) |
| 12 | C2 | committed (`C2:` in git log); partial | core lint; core tests 2226; gen-write; gen-check; py test_core_parity 134, parity 6067 | Calendar-input admission entry rewritten to the remaining work | Done: strict JSON admission + explicit controls (28 files) + ID validation + CSV widths. Not done: `CalendarMetadata` purpose/ranges/edition extension (004 #8), generator-level semantic checks, independent table ranges |
| 13 | H2 | committed (`H2:` in git log) | workspace lint; rust-test (9 fixture fixes re-run: exotics 314, core count test); gen-write; python-test 14003; wasm-test 1086; ui-check | 004/005 entry updated (cme/cnbe removed; NERC settlement use rejected); record 005 partly reconciled | Commodity example uses `usny` as an illustrative contract calendar; NERC purpose restriction is enforced at the commodity forward only (metadata purposes pending) |
| 14 | C3 | committed (`C3:` in git log); partial | gen-write; workspace rust-test 16350; python-test 14003; wasm-test 1086; ui-check; py test_runtime_docs 3684 | 006 entry rewritten to the remaining work; record 006 partly reconciled | Done: no `BusinessDayConvention` default (enum, cashflow schedule/fee specs, statements `SwapConventions`, Python/WASM tenor rolls), empty `+` joint members rejected, Modified rolls compare year+month. Not done: resolver fallbacks (with 011 FX callers), 100-day cap, composite coverage; product-level defaults (see Needs review) |
| 15 | C4 | committed (`C4:` in git log) | core lint; core tests 2235 + doctests 286; gen-write; workspace rust-test 16359; python-test 14003; wasm-test 1086 | 006 entry: search items moved to reconciled | Cap and `max_days` removed; Modified forward search stays in the original year/month then reverses from the original date; Nearest alternates sides and stops on proof; `Unadjusted` returns before classifying; tests: 120-day closure, fully closed month, same month a year later, unneeded next-month/preceding data, needed missing data, `i32::MIN` |
| 16 | D1 | committed (`D1:` in git log) | lint core/analytics; core tests 2237 + doctests 287; analytics 262; gen-write; workspace rust-test 16361; python-test 14003; wasm-test 1086; ui-check | 001 part of the actual-day/coupon-context entry rewritten to qualification only; evidence-prose entry: Bus252 prose and 4.16(d)/(e) citations done | New public `DayCount::validate_context` (structure, then requirements) runs before every equal-date zero; analytics CAGR guard delegates to it; `to_ctx` revalidates public fields; Bus252 `0 <= B <= D` (`Error::Internal` for a defective calendar count); ISDA integer years + two partials; summation-order test replaced by validation/date-range tests |
| 17 | D2 | committed (`D2:` in git log) | workspace rust-lint; core tests 2236 + doctests 287; fixed-income 1253; gen-write; workspace rust-test 16360; python-test 14003; wasm-test 1086; ui-check | 002 part of the actual-day/coupon-context entry: ACT/365L items recorded as landed; record 002 partly reconciled | Coupon-end-year rule for all frequencies; frequency no longer required or used (bond `act365l_year_fraction`/`act365l_flow_times` lose the parameter); containment checked before equal-date zero; A009/ISDA 2021 4.6.1(ix) citations in source, Python, WASM, notebook and golden fixture. Values that moved (366 to 365, or 365 to 366): 2 golden cases, 4 core integration tests, 6 product tests (each the 2023-12 to 2025-01-01 annual long-front case) |
| 18 | D3 | committed (`D3:` in git log) | workspace rust-lint; gen-write; workspace rust-test 16360; core doctests 285; python-test 13992; python-lint; wasm-test 1085; wasm-lint; ui-check; notebooks schedule_building and day_count_conventions | 30/360 entry rewritten as Evidence (Reconciliation 57→56, Evidence 67→68); **day-count evidence-prose entry deleted** (totals 594→593, Observed 170→169, Core conventions 20→19); record 003 reconciled; README/financial README/central follow-ups/INVARIANTS updated | New `DayCount::Thirty360Isda` (`30_360_isda`); `end_is_termination_date: Option<bool>` (omitted = unspecified), required by 30E/360 ISDA incl. equal dates; signed reversal keeps it; exact `i64` numerators; raw `days_30_360`/`days_30e_360_isda`/`Thirty360Convention` removed from Rust, Python, WASM, parity ledger, notebooks and docs-site. **[data]** fixture regenerated by running QuantLib 1.43 (`core/tests/golden/generate_daycount_quantlib.py`, 37 cases: 26 QuantLib, 11 hand ICMA/ACT365L, status provisional); QuantLib agreed with every accepted formula. Overnight projection inputs carry `end_is_termination_date` |
| 19 | E1 | committed (`E1:` in git log) | workspace rust-lint; gen-write; workspace rust-test 16361; core doctests 285; python-test 13993; python-lint; wasm-test 1085; wasm-lint; ui-check | 014/007 entry: 007 core arithmetic recorded, remaining narrowed; 002 tenor-bridge item marked landed; record 007 partly reconciled | Representation limits (D u32, W u32/7, M i32, Y i32/12; 200-year cap gone); `to_years` → `to_years_approx` in Rust, Python and WASM (`toYearsApprox`); exact `to_days_approx`; `payments_per_year -> Result` rejecting D/W (about 20 callers propagate); Hull-White swaption cashflows pick their payoff annuity at construction and look up the nominal frequency only for cash par-yield (O5); `date_from_epoch_days -> Result`; WASM `toDaysApprox` returns an exact number; `to_years_with_context` takes the caller's context (Python `ctx=`, WASM trailing optional `ctx`) and errors on an end before the origin |
| 20 | E2 | **E2a committed** (`E2a:` in git log); **E2c (O6 compounding) not executed (scope)**; `from_years` removal **blocked (owner decision)** | E2a: workspace rust-lint; gen-write; workspace rust-test; core doctests; python-test; python-lint; wasm-test; wasm-lint; ui-check | 014/007 entry: `months_until` and roll rule recorded; remaining narrowed | E2a: `months_until -> Result<u32>` (greatest `k` with `add_months(k) <= end`; reversed errors) in core, Python and WASM; structured-credit sites moved to a named site-local `ClampedMonths::clamped_months_until` keeping the pre-007 count (E2b assigns meanings); bond future uses the checked core measure (day-1 start, identical values); `Tenor::add_to_date` errors without a calendar unless `Unadjusted` |
| 21 | E2b | committed (`E2b:` in git log); documentation only | docs (no code change) | 014/007 entry links the stated meanings | All 16 structured-credit sites have a stated meaning (seasoning, remaining term, simulation index/grid length, lock-out months, domain bound) in the structured-credit README. None migrated: no accepted record selects a measure, and O13 forbids assuming completed months. All stay on `ClampedMonths::clamped_months_until` |
| 22 | E3 | committed (`E3a:`, `E3b-1:`, `E3b-2:`, `E3b-3:` in git log) | E3b: lint, gen-check, rust-test, python-test, wasm-test, ui-check (slice-gate per commit; after gen-write); notebooks schedule_building, dates_calendars_schedules, credit_factor_hierarchy | 008 entry narrowed to CDS mode (E4) and the CDS premium calendar regime (E4b); record 008 and 009 #10 status updated | E3b-1 strict EOM/IMM anchors and final-date ceiling, IMM searches removed; E3b-2 private validated `Schedule`; E3b-3 `GridMode`, complete `ScheduleSpec`, explicit builder, hosts; see Needs review |
| 23 | E4 | not started (dep E3 met) | | | |
| 24 | E4b | skipped (dep E4) | | | O7 decision not recorded in this run |
| 25 | G1 | committed (`G1:` in git log) | workspace rust-lint; gen-write; workspace rust-test 16,366 (16,365 + the mid-coupon make-whole re-expectation, re-run 19/19); python-test 14,008 / 112 skip; wasm-test 1,087; ui-check 1,527; doctests and the Linux run skipped by owner request | 017 entry: runtime reconciliation recorded (qualification remains); context-free-XIRR entry removed; record 017 status updated | Guess-free `irr`/`xirr`, fixed ACT/365F clock, canonical per-date net, full log bracket via fallible shared `solve_in_bracket`, one acceptance gate; `YieldBasis` (ACT/365F, ACT/360) with explicit loan `yield_basis` (O11); bond floor `day_count` removed; MC path IRR removed; see Needs review |
| 26 | G2 | committed (`G2:` in git log) | workspace rust-lint; gen-write; workspace rust-test 16358; core doctests 285; python-test 13988; python-lint; wasm-test 1085; wasm-lint; ui-check | 019 entry: CreditRating part recorded; record 019 partly reconciled | `D` = 21 and `NR` off the ladder; no `Ord`, hand-written `PartialOrd` (`NR` incomparable with rated values, equal to itself); `ladder_position()`; `notches_to`/`to_moodys_string -> Result` (`NR` arithmetic and Moody's `D` error); finite canonical/Moody's/status alias tables replace suffix stripping; the models rating-factor table and the structured-credit `rating_haircuts` move from `BTreeMap` to `IndexMap` (ladder order / wire order); Python ordering of `NR` against a rated value raises `TypeError` |
| 27 | G3 | not attempted (run budget; plan sized M/L) | | | See Needs review: classify validation and embedded-data retirement must land together |
| 28 | G4 | committed (`G4:` in git log) | workspace rust-lint; gen-write (nothing moved); workspace rust-test 16,371; python-test 14,008 / 112 skip; wasm-test 1,087; ui-check 1,527; doctests and the Linux run skipped by owner request | 016 part of the 015/016 entry: runtime reconciliation recorded (qualification and product review remain); record 016 status updated | `npv`/`npv_amounts(as_of, flows, include_today)`; validate-before-select, no unused source query, `df_between_dates`, exact Money and checked scalar paths; shared `NeumaierAccumulator::checked_add`/`checked_total`; `flat_discount_factor` underflow error; schedule helpers take `include_today`; private trade-NPV copy removed; see Needs review |
| 29 | F1 | not attempted (run budget; plan sized M) | | | About 200 sites in about 50 files (fiscal presets 48+ in 19 files, `PeriodId::annual` 72 in 20, `next_fiscal`/`prev_fiscal` 39, plus host presets) |
| 30 | F2 | not attempted (run budget; dep F1) | | | |
| 31 | F3 | not attempted (run budget; dep F2) | | | |
| 32 | F4 | not attempted (run budget; dep F3) | | | |
| 33 | F5 | not attempted (run budget; plan sized L) | | | |
| 34 | F6 | committed (`F6:` in git log) | lint, rust-test, python-test, wasm-test, all-audit (slice-gate; after gen-write) | 013 entry: config/policy items recorded as reconciled; record 013 status updated; STATEMENTS-CAPITAL-09 observed text updated | Required `pivot_currency: Option<Currency>`; no FX config/matrix/policy defaults; explicit pivots at ~165 Rust and ~60 host sites (`None` unless a deliberate hub); `PeriodAverage` removed; required host `rate` policy; calibration `fx: Option<FxConfig>`; statements `PeriodEnd` fallback removed; see Needs review |
| 35 | F7 | committed (`F7:` in git log) | lint, gen-check, rust-test, python-test, wasm-test, ui-check (slice-gate; after gen-write) | 013 entry: resolution items recorded as reconciled, state/shock items remain for F8; record 013 status updated | Pin > global > provider; identity quotes rejected; literal ordered cache; NotFound-only routing with preserved leg causes; checked revisions; one-call limit for revisionless providers; single fallible `FxProvider::snapshot` hook with validated capture; see Needs review |
| 36 | F8 | committed (`F8:` in git log) | lint, gen-check, rust-test, python-test, wasm-test, ui-check (slice-gate; after gen-write) | 013 entry and the non-atomic-merge entry removed (598 items; Observed 156, Reconciliation 92; Core conventions 17); record 013 status and core-conventions inventory updated | `load_from_state` removed; fresh `try_from_state` with repeated-key rejection; frozen quote-only `with_bumped_rate(reference, b)`; `MarketBump::FxPct.policy`; validated `FxRateResult` deserialization; see Needs review |
| 37 | G5 | in progress (G5-1, G5-2 committed) | lint, rust-test, python-test, wasm-test, ui-check (slice-gate; after gen-write) | | Kinds (`Floating`, `ProtectionPayment`); PIK/write-down producers fill explicit deltas and PIK readers read them; cash-only calendar-year ladders; see Needs review |
| 38 | E5 | not started (dep E3 met) | | | |
| 39 | E6 | skipped (dep G5, CASHFLOWS-001) | | | |
| 40 | H1 | not attempted (run budget) | | | Data slice; needs primary sources fetched in-run |
| 41 | H3 | not attempted (run budget) | | | Data slice |
| 42 | H4 | not attempted (run budget) | | | Data slice |
| 43 | H5 | not attempted (run budget) | | | Data slice |
| 44 | H6 | not started (dep B5 met) | | | |

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
- **Phase E** (after E3a `28530ab1ef`, run after G2 `63645b0593`; E4/E4b/E5
  skipped): `mise run all` **pass** (Python 13988).

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

## Owner decisions after the run (2026-11-09)

Resolved by the owner after the run; these unblock or settle the items
logged under Needs review.

| # | Item | Decision |
| --- | --- | --- |
| D1 | B2b: sub-1e-28 model values entering `Money` | Keep the strict float-to-Decimal converter for inputs; add one named pricer-only adapter (for example `Money::from_model_value`) that rounds explicitly to Decimal's 28-digit scale, half-even. Unblocks B2b and its chain (B3–B5, G1, G4, F6–F8, G5, H6). |
| D2 | `from_years` removal: source of contractual tenor | `ForwardCurve` stores its index tenor as a `Tenor` (core market-data wire change); callers use it, taking a year value from `to_years_approx` only where a clock is needed. |
| D3 | G3: scorecard rating-scale source | `CreditScorecardExtension::new(config, &registry)`; `rating_scale` required, no `sp` default; the selected scale is resolved once and snapshotted. Embedded sp/moodys data and `embedded_registry()` are removed; tests and examples supply explicit registry JSON. |
| D4 | E2c: quote `Compounding` value in existing fixtures | Hybrid: market-sourced goldens and examples state their actual market quote convention (re-pinned where it differs from payment frequency, each listed in the commit); synthetic unit fixtures state `Periodic(payment frequency)`. |
| D5 | C3: valuations product BDC defaults | Remove `serde_defaults::bdc_modified_following` and the variance-swap observation defaults; fixtures write `"modified_following"` explicitly. |
| D6 | D3: 30E/360 ISDA termination status on product paths | Products plumb `Some(is final period)` from their own schedules (as the IRS overnight leg does); the tenor bridge keeps requiring the caller's `DayCountContext`. |
| D7 | B3: Money sums whose exact value exceeds Decimal's 28-29 digits | Exact to the minor unit: an unrepresentable exact sum rounds once, half-even, at the greatest fitting scale only when every digit at or above the currency's minor unit is kept; otherwise it errors (`1e28 + 0.01` USD still errors). Strict exact-or-error (record 012 #6 as written) broke 458 tests, because sums of full-precision bounded products need one digit more than Decimal holds. Decided 2026-11-09. |

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
- **B2b (after owner decision D1).** `f64_to_decimal` now admits exactly
  Rust's shortest round-trip text and rejects anything outside the Decimal
  domain. That includes more than 28 fractional digits, such as a
  17-significant-digit `1.23e-15`, not only sub-`1e-28` values. Pricer
  outputs go through `Money::from_model_value`, which rounds half-even at
  scale 28.
  - **Which sites use the adapter.** The first commit converted the 125
    sites that failing tests reached: the structured-credit pricing and
    simulation engine, `pricing_methods.rs`, the CDS tranche engine, and
    single sites in Black equity options, CMS replication, cap/floor,
    swaption, revolving-credit path pricing, inflation-linked bond and
    `Position::scale_value`. A follow-up sweep classified all 292 remaining
    non-test `Money::new` sites in `valuations/**/src`, `models`,
    `calibration` and `cashflows`, and moved 216 model outputs to the adapter
    (PVs, price × quantity, MC/tree/PDE/closed-form results, projected
    floating coupons, computed accruals, bumped prices, composite history).
  - **Kept strict as contractual or caller inputs:** notionals and index
    factors, contractual upfronts, fees and OID, amortization and sweep
    amounts, term-loan commitment fees, settlement cash, schema/test-support
    literals, and the cashflows crate's contractual coupons, fees and
    redemptions.
  - **Ambiguous, kept strict:** `commodity_forward` `contractual_invoice_amount`
    (falls back to the market forward when no contract price is set),
    `commodity_swap` `classified_flow` (shared by fixed and projected
    floating legs), `cashflows` `aggregation::sum_by_period` (shared by
    nominal and PV sums), `structured_credit` pool purchase price and the
    `dollar_roll` trade cash. Each can error on a sub-`1e-12` full-mantissa
    projection; splitting them by branch is an owner call.
  - **Not swept:** `statements`, `statements-analytics`, `portfolio`,
    `margin`, `attribution` and `scenarios` (about 120 sites). These are
    Money arithmetic written as `Money::new(m.amount() * k)` (scale, ratio,
    residual). B3 does not touch that pattern, so a computed sub-`1e-12`
    full-mantissa residue there still errors; moving them to Money
    arithmetic or the adapter is unscheduled work.
  - No per-family deep-out-of-the-money probe tests were added beyond the
    two proptest seeds proptest recorded (equity option, swaption).
- **B2b test changes. Each is a test weakening or reframing for owner review:**
  - `capital_structure` `finite_inputs_cannot_produce_an_infinite_ratio`
    now expects the `Decimal conversion overflow` error. An infinite ratio
    cannot reach `Money`, and the test still proves no infinity escapes.
  - **Weakening:** the structured-credit trap test's
    (`production_waterfall_trap_cure_rebreach_conserves_interest`) released
    trap moved from `== 0.0` to `abs() < 1e-9`. The residue is now about
    1e-10 of float noise; the old exact zero came from lossy conversion.
  - **Weakening:** the Python Asian/European one-fixing mean equality is now
    `approx(abs=1e-14)`, the file's stderr tolerance. The two reducers'
    means differ by two ulps.
  - **Weakening:** the UI portfolio host-parity check compares the wasm32
    worker with the native-Python values within the parity tests' 1e-9
    relative, not exactly.
  - **Reframed (same claim):**
    - The portfolio CIP tests compute `1.10 * (0.99 / 0.95)` in the
      production order.
    - The WASM mixed-node forecast expects `100 * (1 + 0.1)`.
    - The UI valuations workbench finds the headline Money among elements
      titled with the PV, because the one-row cashflow ladder now prints the
      same exact PV.
  - **Re-pins:**
    - 10 recorded wire-key PVs
    - the callable-loan OAS price
    - the LMM Bermudan pin
    - the term-loan coupon bits
    - 15 bump-routing pins
    - the dual macOS/Linux pins: GBM Greeks, LSMC, Asian, barrier PDE,
      term-loan tree and callable loan
    - about 25 UI literal pins, at the last digits

    The term-loan tree's short-rate pin now differs by one ulp between hosts.
  - **Regenerated** with each file's documented command:
    - the valuations, statements, margin and cashflows cross-host goldens
    - the portfolio `host_parity.json` (`UPDATE_HOST_PARITY=1`)
    - the UI fixtures

    Values move at the ulp level, except finite-difference outputs (up to
    about 1e-11 relative: equity volga, factor stress) and 10
    structured-credit waterfall residuals, which went from exact 0 to about
    ±1e-10. A small negative residual is safe: undistributed interest
    passes through `snap_de_minimis`, and cash conservation uses a
    tolerance.
- **B2b platform evidence.**
  - **Linux:** the full Rust workspace (bindings excluded), run in a local
    `rust:1.97.1` linux/amd64 container, passes apart from three
    container-only golden-runner failures (read-only mount, no git).
  - **H2 doctests fixed here:** it also exposed two commodity-forward
    doctests broken since H2, which lacked `settlement_calendar_id`. macOS
    `rust-test` skips doctests.
  - **Not run on Linux:** Python and the UI. The Python exact-string
    goldens were regenerated on macOS, so CI on Linux may still show native
    libm drift there. The wasm32 facade, with libm independent of the host,
    matches them.
  - **WASM gate:** the WASM figure (1085) is from `wasm-test-built`, after
    one full `wasm-test` build.
- **F8.**
  - **Shock as a state transform.** `with_bumped_rate` captures the state,
    rewrites it as data and restores a fresh quote-only matrix, rather than
    wrapping providers. This makes every shocked matrix frozen, capturable and
    re-shockable; `BumpedFxProvider` and `ResolvedPairProvider` were deleted.
  - **Pivot-derived targets are stored, not routed.** Where the pair is only
    reachable through the pivot, each orientation's unshocked route result is
    shocked and written at provider authority for the global scope (probed on a
    date outside every captured scope) and for every date/policy scope. The
    shocked matrix therefore reports `triangulated == false` for that pair: it
    no longer routes it. Record 013 #6 defines the flag as "this matrix used
    its pivot fallback", so this was taken as correct.
  - **One-orientation stores.** Where a store/scope held only one orientation
    of the target, the other is written beside it from its own unshocked
    reciprocal, so each orientation is "its own baseline / f" exactly (bit
    equality is asserted) rather than the reciprocal of the shocked opposite.
  - **Live providers can no longer be shocked.** Capture is required, so FX
    delta, FX01, cross-factor bumps, `MarketBump::FxPct` and scenario FX
    shocks error over a computed provider. Tests that relied on live
    flow-through (provider updates reaching an existing shock) now assert
    freezing; tests that only needed fixed rates moved to `SimpleFxProvider`
    or scoped quotes (the term-structure property is kept with a date pin).
    The FX swap metrics fixtures and the attribution QuantLib FX parity market
    used hand-written fixed-rate providers; they now hold the same EUR/USD
    quotes in a `SimpleFxProvider` (their unused stubs were deleted).
  - **`MarketBump::FxPct.policy`.** Required, because the bump already carries
    its reference date. All producers (portfolio delta engine, FX01, cross
    factor bumper, scenario adapter, valuations FX bump helper) state
    `CashflowDate`, the policy their lookups already use, so no value moved.
    `OperationSpec::MarketFxPct` has no date or policy; scenario authors cannot
    choose the reference policy.
  - **Batch duplicates.** Bulk `set_quotes` (matrix and `SimpleFxProvider`)
    now rejects a repeated ordered pair instead of last-row-wins; host
    `from_dict` inputs that spell one pair two ways (`eurusd`, `EUR/USD`) now
    error.
  - **Follow-up closure.** The plan says F8 closes the 013 entry and the
    non-atomic-merge entry, so both were removed. The 013 entry also listed
    independent qualification (average counterexamples, provider races,
    snapshot/shock replay); that remains pending in record 013's validation
    status, with no follow-up entry of its own.
  - **Bench fix.** `core/benches/fx_matrix.rs` (outside the gate) asserted a
    triangulated cross over a matrix the F6 sweep had given pivot `None`; it
    is a deliberate USD hub and now uses `Some(USD)`. The pre-existing
    `schedule_generation` bench compile error (`Schedule.warnings`) is
    unrelated and untouched.
- **F7.**
  - **Scope taken strictly.** The plan's F7 bullets name pinned-before-global,
    ordered literal cache keys, identity rejection, NotFound-only routing,
    a checked revision counter and fallible snapshot hooks. Record 013 item 8
    (a revisionless provider may answer only one call per resolution attempt)
    and item 9 (validate captured state before returning it; capture needs
    coherent generations) are not in any phase bullet, and F8 "closes the 013
    entry", so they were implemented here.
  - **Revisionless providers.** A resolution that needs a second provider call
    (a pivot route after the missing probe) now errors when the provider
    reports no revision; wholly matrix-held legs after a single missing probe
    still route. Several immutable test providers gained `get_revision() ->
    Some(0)` (the record's sanctioned value for immutable providers).
  - **Snapshot hooks merged.** `snapshot_quotes` and `snapshot_pinned_quotes`
    became one `FxProvider::snapshot() -> Result<Option<FxProviderSnapshot>>`:
    `Ok(None)` (default) is unsupported, `Some(empty)` is a complete empty
    provider. One call keeps the two stores coherent; no error variant for
    "unsupported" exists, so `Option` carries it. Capture now also requires a
    provider revision (previously only non-empty snapshots did).
  - **Behaviour flips encoded in tests.** Pinned-before-global reverses the
    old global-first order: core tests (`pinned_quote_outranks_pair_global_in_either_orientation`,
    `pinned_fixing_precedes_global_source_in_routes_and_reciprocals`, two
    bumped-snapshot authority checks), the Python
    `test_fx_pinned_fixing_precedes_global_quote_in_either_orientation` and
    the Node "FX pinned fixing precedes the global quote" test now assert the
    record's order. Overflowing shocks now fail at capture instead of at
    serialization. A live provider's bumped matrix no longer captures as an
    empty provider snapshot; it errors (the lifecycle test asserts that).
  - **Route context.** A non-NotFound leg error propagates with its own
    category but without added route context: the error type has no wrapper
    that keeps the category, and relabelling would change it. Logged rather
    than inventing a variant.
  - **Shock snapshots.** `ResolvedPairProvider`/`BumpedFxProvider` snapshot
    lookups now propagate invalid reciprocals and invalid crosses instead of
    silently omitting them, and their scoped lookups use pin before global.
    The live-provider shock itself is unchanged (F8).
  - **Scenario FX preview.** The scenarios FX adapter captures the matrix
    state to read its pivot and global quotes, so an FX shock over a matrix
    whose provider cannot be captured (a live custom provider) now errors at
    preview instead of proceeding. Its two tests used a NotFound-only custom
    provider and now use an empty `SimpleFxProvider` (same answers). F8 makes
    every shock need a frozen baseline anyway.
- **F6.**
  - **Pivot assignment (O9).** Every matrix now states its pivot. Mechanical
    sweep: `None` everywhere, then `Some(USD)` only where a test's assertions
    route a cross through USD (14 core FX tests, the portfolio sensitivity
    helper `test_fx_market` and `triangulated_fx_matrix`, one PeriodAverage
    test since deleted). Explicit test configs kept their exact meaning
    (`enable_triangulation: false` -> `None`, otherwise the given pivot).
    JSON fixtures were rewritten the same way (`true` keeps the pivot, `false`
    becomes `null`; about 160 files, text-edited so formatting is unchanged).
    Tests that pass under `None` without routing were not reviewed one by one
    for a "passes for the wrong reason" error assertion.
  - **One routing test deadlocked instead of failing.**
    `fx_quote_update_prevents_inflight_cross_from_repopulating_cache` blocks
    until a cross-rate fetch through the pivot starts; under `None` it waited
    forever (about 48 minutes) until stopped. It now has its USD pivot, and the
    owner had every test runner cap tests at 60 s (commit `d8889521ca`).
  - **Calibration FX config.** `CalibrationConfig.fx` is `Option<FxConfig>`
    (omitted key = no FX choice). Building FX from spot quotes without it is
    an error naming the missing `fx` section; a present section requires
    `pivot_currency` (`null` allowed) and rejects `enable_triangulation`.
    `CalibrationConfig::default()` therefore carries no FX choice.
  - **Hosts.** Python `FxMatrix(pivot_currency)` /
    `from_dict(quotes, pivot_currency)` and WASM `new FxMatrix(pivotCurrency)`
    / `fromDict(quotes, pivotCurrency)` take a required routing choice (WASM
    rejects `undefined`, accepts `null`). Python `FxMatrix.rate` and WASM
    `rate` require a policy; the facade's default-policy patch and the
    zero-argument `FxConversionPolicy` constructor are gone. Existing host
    callers pass `None`/`null` and `"cashflow_date"` (the former default).
  - **Implicit policies made explicit.** 130 `Default::default()` /
    `FxConversionPolicy::default()` policy arguments became
    `FxConversionPolicy::CashflowDate` (the former default), so no value moved.
  - **PeriodAverage.** No production caller. Tests that used it only as a
    second distinct policy now use `PeriodEnd`; the matrix test about
    averaging was deleted (013 #2: no averaging point query).
  - **Statements.** STATEMENTS-CAPITAL-09 (Draft) recorded "omitted policy
    selects PeriodEnd"; O9 removes that fallback. Strictest workable reading:
    `CapitalStructureSpec.fx_policy` stays optional, a cross-currency
    conversion without it errors, same-currency totals need none. The builder
    prose that claimed `CashflowDate` converts on each flow date was corrected
    (every aggregate converts on the period-end snapshot date).
  - **Canonical hash.** The canonical calibration fixture lost its
    `enable_triangulation` key, so its recorded `content_hash` changed.
    The pinned WASM content hash of `examples/market_bootstrap/01_usd_discount.json`
    also moved (the envelope file is unchanged; the defaulted `fx` section is
    no longer serialized); Python computes the same new hash.
  - **Tests rewritten because the default they checked is gone.** Core
    `fx_query_defaults_policy...` now asserts a missing `policy` errors; the
    wasm32 `fx_matrix_rate_defaults_policy_to_cashflow_date` became
    `fx_matrix_requires_an_explicit_routing_choice` (`undefined` rejected,
    `null`/`"USD"` accepted). Two statements tests now set `fx_policy:
    PeriodEnd` (one would otherwise hit the new missing-policy error before the
    missing-FX-matrix error it checks). Host routing tests (3 Python, 2 Node)
    got a USD pivot; one Node FX state fixture got `pivot_currency: null`.
- **G4.**
  - **`include_today` choices preserve current behaviour.** No product
    review of same-day cash was done (016 #2 makes it a product decision).
    Every caller of the old exclusion now passes `false` explicitly: the
    term-loan, PE fund, asset-backed facility and structured-credit pricers,
    the valuations `schedule_pv`/`schedule_pv_raw` holder views (deposit and
    inflation-linked bond) and tests. The deposit's raw trade NPV, which
    already kept the T+0 inception exchange through a private loop, passes
    `true`. The plan's "about 20 product callers" predates the helpers that
    now centralise them; only these sites call the core NPV.
  - **Private NPV copy removed.** `schedule_trade_pv_raw` summed
    `amount * df` with an unchecked accumulator and no validation (016 #1
    forbids a parallel engine). It is deleted; the deposit uses
    `schedule_pv_raw(..., true)`. The only behavioural difference: an empty
    schedule without a notional now errors instead of returning 0.
  - **Error messages moved to the source boundary.** Invalid valuation-date
    and payment-date factors now report `df_between_dates`'s "Invalid
    discount factor on 'from'/'to' date" (016 #5: the source owns those
    checks). Two tests were updated to the new message; both still require
    an error.
  - **Shared numeric checks.** `NeumaierAccumulator` gains `checked_add` and
    `checked_total` instead of an NPV-private checked copy (016 #7).
  - **Not done.** Doctests were not run (owner request), including the new
    NPV, `flat_discount_factor` and `checked_add` examples. No independent
    qualification of 016's numerical targets.
- **G1.**
  - **Shared Brent (partial CORE-NUMERICS-05).** `solve_in_bracket` now
    takes `FnMut(f64) -> Result<f64>` and propagates objective errors (05
    #2). Its 12 other callers wrap their objective in `Ok(...)` unchanged;
    moving them onto genuine error propagation, deleting `solve_fallible`
    and the rest of record 05 belong to that record's slice.
  - **One commit, not G1a/G1b.** The interim plan to derive the loan basis
    from the accrual day count failed two tests on a 30/360 loan, so the
    explicit field landed in the same commit.
  - **Loan `yield_basis` is a required field (O11).** Term loans and
    revolvers carry `yield_basis: YieldBasis` (`act_365f` | `act_360`),
    separate from `day_count`; YTM, YTC/YTW and the EIR use it, and EIR
    accretion uses its clock (actual days over 365 or 360). Every builder
    chain, struct literal, JSON fixture, Python dict, notebook and host
    builder (Python/WASM `yield_basis` setters and getters) was given the
    basis matching its accrual day count; ACT/360 and ACT/365F loans keep
    their values (equal growth). Accruals with no exact conversion (the
    30/360 test loan, an ACT/365L revolver fixture) quote on ACT/365F, so
    their yields moved: the two make-whole YTC tests now expect
    `G^(365/181) - 1` and `G^(365/90) - 1` instead of the 30/360
    `G^2 - 1` and `G^4 - 1`.
  - **Real estate and PE beyond O11.** O11 names loans and bonds only.
    Real-estate (unlevered/levered) IRR and PE LP IRR keep their configured
    `day_count` as the quote basis, but only ACT/365F or ACT/360 (named
    error otherwise; every fixture and default is ACT/365F). The PE waterfall
    validates this up front, so `calculate_lp_irr_to_date`'s `.ok()` cannot
    hide a basis error.
  - **Swallowed solver failures left as owner policy.** Bond XIRR-to-worst
    skips non-converging exit paths (documented; candidate-selection policy
    is the open "collateral exercise policies" register entry), and
    structured-credit equity IRR and PE LP IRR-to-date map failures to
    "unavailable" (`.ok()`). Record 017 #12 asks for owner-documented
    meanings; these were not changed.
  - **Retired-behaviour tests replaced.** Tests pinning a guess, a
    day-count argument (ACT/360 vs ACT/365F XIRR, 30/360 netting) or the
    removed bench group were replaced by record 017's boundary rows
    (leap-day clock, zero spacing, origin shift, both directions, signed
    zero, explicit failures). `xirr` with a 99% loss now must converge
    without a hint (it was allowed to fail).
  - **Value moves.** Structured-credit equity IRR in the cross-host golden
    moved in the 15th digit (new solver path); the bond floor, portfolio and
    structured-credit paths already used ACT/365F.
  - **Not done.** Doctests were not run (owner request; nightly tier),
    which includes the new `irr`/`xirr`/`solve_in_bracket`/`YieldBasis`
    examples. No independent qualification of 017's numerical targets.
- **B5.**
  - **"Python `__index__` on `Currency`" (plan line, no further detail).**
    `Currency` had no `__index__`. Strictest reading under 012 #12 (identity
    is the alphabetic code; `numeric()` is metadata, not a cast): Python
    `Currency` gets no integer protocol (`operator.index`/`int()` raise
    `TypeError`, now pinned by `tests/test_currency_identity.py`), and
    `__hash__` moved from the numeric code to the alphabetic code. If the
    plan meant to *add* `__index__`, that would reintroduce a numeric cast
    and needs an owner decision.
  - **Order changes are deliberate (O10).** Currency-keyed output now sorts
    by code: same-date multi-currency `dated_flows`, SIMM `ir_*`/`fx_*`
    rows, the portfolio margin JSON, FX quote snapshots, the UI XCCY ladder
    and FRTB/SIMM rows. The SIMM FX-vega risk-factor label for an
    unordered pair is now `EUR/USD` (was `USD/EUR`). Tests pinning the old
    numeric order were updated to the new order; no value changed.
  - **Generator test.** `test_duplicate_numeric_currency_rejected` pinned
    the opposite of #14 (it rejected a numeric code shared by distinct
    alphabetic codes). It is replaced by a duplicate-alphabetic rejection
    test and a shared-numeric (ANG/XCG 532) ambiguity test.
  - **FX observed-cache order independence is provider-conditional.**
    `FxMatrix` asks the provider in the requested direction ("the provider
    remains direction-sensitive") and stores the pair canonically by
    `Currency` order. A cache warmed in either direction stores the same
    value only when the provider's canonical-side quote is the exact
    reciprocal of its other side. The test provider (`ReciprocalFx`) derived
    EUR/USD from USD/EUR, which matched the old numeric canonical side; it
    now quotes USD/EUR and derives EUR/USD so its derived side is again the
    canonical one. The property under test is unchanged. Making the cache
    order-independent for every provider (query the canonical direction)
    is an FX design change for record 013, not done here.
  - **Golden regeneration.** `margin_wasm_parity.json` and the
    `market_context` case of `core_wasm_parity.json` were regenerated and
    contain only reorders. The regeneration also rewrote three
    `discount_curve_queries` values in their 16th digit (pre-existing
    drift, inside the test's tolerance, unrelated to B5); those three were
    restored to the committed values.
  - **Schemas unchanged.** The generated enum keeps its schema-facing doc
    text, so JSON schemas and declarations did not move.
- **B4.**
  - **Wire shape of an unselected mode (strictest reading of #8/#9).**
    `RoundingPolicy.mode` and the stamped `RoundingContext.mode` are
    `Option<RoundingMode>` and are omitted from JSON when unselected, not
    written as `null`. Every default result stamp therefore loses
    `"mode": "bankers"`; generated schemas, declarations and UI fixtures were
    regenerated, and old payloads carrying a mode still parse.
  - **Incomplete policy is detected at use, not at load.** A config with an
    ingest scale override and no mode deserializes; configured construction
    for that currency then errors (`FinstackConfig::ingest_rounding`). Output
    overrides need no mode, because output scale is a presentation control.
  - **Host shapes.**
    - Python `FinstackConfig.rounding_mode` and `ingest_scale()` return
      `None` when unselected; WASM `roundingMode` and `ingestScale()` return
      `undefined`.
    - Python `round(scale, mode)` takes an `int` (not `bool`); a negative
      scale raises `ValueError` from the binding, before Rust. WASM rejects a
      negative scale as `TypeError` (its integer reader).
    - `round(money)` now raises `TypeError` (no `__round__`).
  - **Negated zero.** `Money::checked_neg` now goes through
    `from_decimal`'s zero normalization (#1), so `-Money(0)` stores and
    displays the unsigned zero. Python `test_negated_zero_display_matches_wasm`
    and the WASM `checkedNeg` test had pinned `USD -0.00`; both now expect
    `USD 0.00` per #10. This updates two pinned tests to the accepted rule.
  - **Hand-maintained goldens.** `host_parity.json`, `covenants_parity.json`,
    `valuations_parity.golden.json` and the UI `scenario-pnl.json` dropped the
    default `"mode": "bankers"` stamp (only stamps directly preceding
    `ingest_scale_by_currency` were edited); no other value moved.
  - **UI.** The stamp badge and `formatMoney` already treat a stamp without
    a mode as unavailable and show the raw amount; no implicit mode was
    added there.
  - **Out of scope, unchanged.** CORE-NUMERICS-23 (`rounding-and-config.md`)
    item 1 restates these prerequisites; its own items (removing
    `ZeroKind`/`ToleranceConfig`, `rounding_applied`, version 2 snapshots) are
    not part of this plan and were not touched.
- **B3 (after owner decision D7).**
  - **Qualification first.** rust_decimal 1.43's `checked_*` was tested
    against an exact BigInt reference on 200k random pairs and 600k exact
    half-even ties.
    - add/sub silently round about 30% of inexact sums (`1e28 + 0.01`
      returns `1e28`), so the slow path is ours.
    - mul/div match the record's greatest-scale half-even rule wherever a
      value is returned; the only gap was returning 0 on nonzero underflow,
      which now errors.
    - The reference test lives in `money/rounding.rs`.
  - **D7.** Strict exact-or-error sums (#6 as written) broke 458 tests:
    `24889.265874428517229867135231 + 990070.2447286517` needs 30 digits.
    Sums now round only below the minor unit. This is an owner amendment of
    an accepted item.
  - **API.**
    - `Money::checked_div(Money) -> f64` is renamed `ratio`.
    - `checked_mul`/`checked_div` now take a Decimal scalar.
    - `convert_at_rate` takes a Decimal; f64 callers moved to
      `convert_at_rate_f64` (core `MarketContext` conversion and tests).
    - The same-currency shortcut in `MarketContext::convert_money` still
      returns before any rate is read, so it cannot trip the
      exact-unit-rate rule.
    - Python: `convert_at_rate` takes `Decimal`/`int`/`str`, and a `float`
      raises `TypeError` naming `convert_at_rate_f64`. The operators take
      exact `int`/`Decimal` scalars and reject `bool`.
    - WASM: `convertAtRate` takes a decimal string; `convertAtRateF64`,
      `checkedMul` and `checkedDiv` are added.
  - **Results.** Arithmetic results pass through `from_decimal`'s zero
    normalization. No golden moved.
  - **Not done:**
    - Correctly rounded `amount()` (#5): `as_f64` is one ulp off for about
      10% of values, and fixing it re-pins nearly everything. It needs its
      own slice.
    - The ~65 production `checked_mul_f64`/`checked_div_f64` callers
      (quantities, discount factors, accrual fractions, attribution scale
      factors) were not reclassified. Their only new failure is a product
      below 5e-29 in magnitude.
    - No before/after performance timing. The add/sub fast path is
      unchanged; the BigInt slow path runs only when the dependency rescaled.
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
- **E2 split.** O6 requires a new required quote `Compounding` on Bond,
  TermLoan, RevolvingCredit and ABF: no instrument carries one today (the
  existing `compounding: None` fields are overnight floating-rate
  compounding). That is a schema and fixture change across every construction
  site of those instruments, so it is its own slice, **E2c**, and
  `compounding_frequency` stays until E2c removes its last caller.
- **E2c not executed (scope).** O6 is unambiguous, but a required quote
  `Compounding` on each instrument touches every construction site. Counted on
  this branch: Bond `Bond::fixed(` 365 sites in 152 files, `Bond::builder(`
  130 in 47, `Bond::floating(` 33 in 20, `Bond {` literals 253 in 99;
  TermLoan builder 86 in 29 and literals 32 in 20; RevolvingCredit builder
  118 in 33 and literals 45 in 21; AssetBackedFacility 12 sites; and 81 JSON
  fixtures naming these instruments. Most sites pass a frequency variable, so
  the value cannot be inserted by script. `compounding_frequency` stays until
  E2c removes its 8 callers. Two notes for whoever executes it: check first
  that `df_base * Compounding::Periodic(m).df_from_rate(z, t)` reproduces
  today's `z_spread_discount_factor(df_base, t, z, m)` so the field replaces
  the `f64` ratio directly; and setting `Periodic(n)` from each fixture's
  payment frequency to pin existing goldens would be an implementer's choice,
  not the record's.
- **E2 `from_years` blocked.** Record 007 #9 says callers keep actual tenor
  metadata instead of rebuilding a tenor from years. Every remaining caller
  (scenario forward-rate bindings, bond ASW float-leg frequency, bond annuity
  ASW, structured-credit floating period ends) rebuilds it from
  `ForwardCurve::tenor() -> f64` or a stored year fraction, and the curve
  carries no contractual tenor. Removing `from_years` needs either a
  `ForwardCurve` tenor field (core market-data wire change, outside 007) or a
  per-product methodology choice. Left in place; owner decision.
- **E2a scenario roll.** `update_rate_from_binding` took an optional calendar
  and always asked for Modified Following. It now rolls Modified Following
  with a calendar and `Unadjusted` without one (previously the convention was
  silently ignored without a calendar; values are unchanged).
- **E3b (run in three gated commits, on the owner's instruction to run E3b before G5).**
  - **Split.** E3b-1 strict anchors on the old API, E3b-2 private `Schedule`
    state, E3b-3 the explicit spec and builder. No instrument, golden or
    pricing result moved in any of them.
  - **Strict anchors (E3b-1).** EOM and quarterly-IMM grids now reject an
    off-grid regular anchor instead of forcing a mid-month anchor onto
    month-ends or searching for the next IMM date and adding an extra stub.
    25 unit tests, 3 Python tests, 1 Node test, docs and the schedule notebook
    used off-grid anchors; they now use on-grid anchors (for example
    2025-03-19, Jan 31) and assert the off-grid case errors. In the semi-annual
    IMM leg test the old off-cycle maturity produced an undeclared final stub;
    with an on-cycle maturity only the declared front stub remains.
  - **Defaults stated, not invented.** Test, bench, doc and notebook chains
    gained the former defaults explicitly (monthly, `StubKind::None`,
    `GridMode::Natural`, unadjusted), so no date moved. Production sites were
    edited by hand: the cashflow period generator, variance observation dates
    and two bond annuity legs take only the raw grid from the core schedule and
    adjust dates themselves, so they state `.unadjusted()` with their own grid.
    A first sweep's pattern also matched `CashFlowSchedule::builder` and
    `DividendSchedule::builder` and touched one production line; all 38
    affected files were restored from `HEAD` and a corrected sweep rerun.
  - **CDS premium calendar.** The CDS premium schedule applies its business-day
    convention only when the leg names a calendar. Making a convention without
    a calendar an error broke real inputs (the production CDS fixture carries
    `modified_following` with a null calendar), and that policy belongs to the
    CDS date regime (O7, E4b), not to 008's generic builder. The site now states
    `.unadjusted()` explicitly when there is no calendar; E4b should decide
    whether such inputs are accepted.
  - **Semantics that changed.** `Schedule::builder(start, end)` errors unless
    `start < end` (equal dates previously failed only at build). Grid
    selection never resets frequency or stub; `imm()` used to set quarterly and
    `short_back`. CDS IMM mode with a non-natural grid is an error in either
    call order (there is no last-call-wins between them). An IMM roll rule with
    the EOM flag in a cashflow leg remains an error.
  - **Not executed.** `StubKind` keeps its `Default` (about ten instrument and
    spec structs derive `Default` through it); removing it is outside E3's
    bullets. `cds_imm()` still sets quarterly/`short_back` until E4 removes CDS
    mode.
  - **Flaky test.** `test_from_materialization_releases_gil_for_parse_and_build`
    (portfolio materialization, a one-second thread wait) failed twice under
    gate load and passed on rerun; it is unrelated to schedules.
  - **Bench.** `core/benches/schedule_generation.rs` asserted the removed
    `Schedule.warnings` (an E3a leftover) and built standard IMM from an
    off-cycle start; both fixed. Benches are outside the gate.
- **G5 (run in gated commits after E3b).**
  - **G5-1 kinds.** `FloatReset` became `Floating`; CDS-style protection
    rows get `ProtectionPayment`; `is_coupon` / `is_repayment_like` added and
    `is_cash_settlement` made exhaustive. No values moved.
  - **G5-2 explicit deltas, cash unchanged.** Fixed and floating PIK emission
    now sets `principal_delta` to the capitalized amount, and structured-credit
    and asset-backed-facility `DefaultedNotional` rows set a negative
    `principal_delta` plus `principal_date`. Every PIK reader (coupon periods,
    statements PIK expense and capitalization, residual rebuild, term-loan call
    claims, bond LSMC accrual claims, structured-credit balance replay and
    period buckets) reads the delta and errors when it is missing. PIK rows
    still carry the same cash amount until G5-3 sets it to zero.
  - **Write-down sign.** The old fallback derived a defaulted-notional
    balance change of `+amount` (an increase) when no delta was set; the
    explicit delta is `-amount`. The balance-path readers that handled the
    kind took the correct sign by kind, so no structured-credit test or golden
    moved; the UI scenario-table fixtures gained the explicit fields.
  - **Cash-only ladders (owner decision "Cash only").** Calendar-year ladders
    (aggregation, `CashFlowSchedule`, instrument envelopes, Python and WASM)
    skip rows that are not `is_cash_settlement`. The cashflows UI fixture's
    PIK note ladder moved: its 2024 principal changed from -9,536,346.67 to
    -10,000,000 and the PIK-only years dropped out. Portfolio date netting sums
    cash `amount`, so it becomes cash-only once G5-3 zeroes non-cash rows;
    its JSON form takes opaque kind labels and was not filtered by kind.
  - **Fixtures.** Four hand-built PIK rows in tests (cashflows normalize and
    performance accrual, statements effective-date PIK, bond seasoned PIK)
    gained the explicit delta. The cross-host cashflows parity golden
    (`finstack-quant-wasm/tests/facade/golden/cashflows_parity.json`) was
    regenerated with its documented command; it only gained the PIK rows'
    `principal_delta` (equal to their cash amounts), no value moved.
- **Dependency added (G2).** `indexmap` (already a workspace dependency, used
  by core and structured credit) was added to `finstack-quant-models` so the
  rating-factor table keeps a deterministic order once `CreditRating` lost
  `Ord`.
- **G3 not attempted (run budget).** Record 019 #7 and #9 couple the new
  `classify` representative rule with retiring the embedded sp/moodys data:
  the embedded `sp` scale already violates the rule (AA+ representative 95 is
  not below AAA's minimum 95), and #7 forbids repairing those numbers, so the
  validation cannot ship without removing `embedded_registry()` and having
  scorecards resolve one snapshot from a supplied registry. Counted
  rating-scale sites: Python binding 95 and stub 79, WASM binding 69, the
  statements-analytics scorecard extension 12, schema examples 4, host tests
  about 50, two notebooks (one, `registry_defaults_and_overrides.ipynb`, is
  about the embedded defaults being removed). The scorecard API has two
  defensible registry-supply shapes (explicit registry argument versus the
  config extension), which is a design choice for the owner.
- **A1 not closed.** The 020 register entry stays open as Evidence. Equality,
  adjacent and already-beyond observations, window/history behavior, and
  settlement/discount boundaries at each consumer were not independently
  qualified in this slice.
