# Core conventions reconciliation plan

**Created:** 2026-11-08 at HEAD `21c81ab9cb`.
**Covers:** the 21 entries under "Core conventions" in
[financial/FOLLOW_UPS.md](../decisions/financial/FOLLOW_UPS.md#core-conventions)
and their accepted records
[001–020](../decisions/financial/core-conventions/README.md).

## Ground rules

- **No decisions are reopened.** Each slice implements the record's
  `#accepted-decision-…` section. Where a record leaves something to a product
  owner, the slice follows the matching row in
  [Owner resolutions](#owner-resolutions). Nothing here is guessed.
- **Removals have no aliases, defaults, or fallbacks** (AGENTS.md).
- **A slice is always a full triplet:** Rust, then Python (`src/bindings`,
  `.pyi`), then WASM (`src/api`, `exports/**`), then
  `parity_contract.toml`, then `mise run gen-write`. Schemas, fixtures and UI
  generated files are part of the slice.
- **Every slice carries its own qualification tests** (track c). These are
  the boundary rows the record lists. A record's existing golden digest is a
  consistency check only and does not count.
- **Data and provenance refresh (track b) is separate work.** It needs pinned
  external sources: URL, edition, retrieval date and hash. These slices are
  marked **[data]**.
- **Gates per slice:** `mise run rust-lint-crate -- <pkg>` and
  `mise run rust-test-crate -- <pkg>` for each touched crate,
  `mise run python-test-file` for the touched bindings, and
  `mise run wasm-test-built`, plus `mise run gen-write` so generated files
  are current. Once per phase, run the full `mise run all` gate.
- **Commit after every slice.** This keeps review to one slice per commit:
  - **One slice, one commit.** Never combine two slices in a commit. Never
    start the next slice while the previous one is uncommitted.
  - **Self-contained.** The commit includes its own regenerated schemas,
    fixtures, `.pyi`, `index.d.ts` and parity ledger changes, so it builds
    and passes the per-slice gates on its own.
  - **Register update included.** The commit carries its follow-up register
    edit: the entry deleted or rewritten, as described under
    [Closing register entries](#closing-register-entries). There are no
    separate "docs catch-up" commits.
  - **Message format:**
    `<slice id>: <summary> (CORE-CONVENTIONS-<record>)`, for example
    `A1: require explicit barrier rebate timing (CORE-CONVENTIONS-020)`.
    The body lists what was removed, the consumers changed, the tests added,
    and any owner question left open.
  - **Mixed slices.** If a slice turns out to need both mechanical caller
    churn and semantic changes, split it into two consecutive commits under
    the same slice id (`B2a`, `B2b`), each passing the gates.
  - **Gate failures.** A full `mise run all` failure at the end of a phase is
    fixed in its own commit, before the next phase starts.
  - **Shared files.** Stage only this slice's files. Another session may have
    uncommitted edits in the register files, so stage by hunk rather than
    adding whole files.
- **Baseline:** none of the 20 records has runtime work at HEAD. Commit
  `c5f2212130` ("Refactor barrier classification") changed docs only. Since
  the audit revision `7e82fd03ad`, the only change under `core/src/dates` is
  the `FiscalConfig` schemars ranges. Every finding below was re-checked
  against HEAD.

## Closing register entries

The user asked that fixed items be removed from the follow-up list. Follow the
precedent set by `df804d4059` ("Implement accepted analytics decisions").

1. **Delete** a register entry from `docs/decisions/financial/FOLLOW_UPS.md`
   in the commit that completes its last named action. That includes runtime
   work, the qualification tests it lists, and any **[data]** refresh it
   names. Do not strike it through or mark it done.
2. When a slice completes only part of an entry, **rewrite the entry in
   place** so it states only what remains.
3. In the same commit, update the register's bookkeeping:
   - The header total (currently 593 items across 33 scopes).
   - The per-label tallies (Observed, Investigate, Decision, Reconciliation,
     Evidence).
   - The scope-index row count for "Core conventions".
   - When the section empties, remove both the section and its index row,
     and decrement the scope count.
4. Update these files in the same commit:
   - **`core-conventions/README.md`:** replace "Implementation status is
     Observed …" with the reconciled statement for the records that are now
     closed. The precedent wording is: accepted sections state the current
     contract, and the former discrepancies have been removed from the
     follow-up registers.
   - **`core-conventions/coverage.json`:** refresh the source ledger and
     fingerprints for the files that changed.
   - **The record itself:** the "Observed" inventory lines for the
     reconciled behavior describe current code.
   - **Cross-cutting docs:** `financial/README.md`,
     `financial/AUDIT_BOUNDARY.md`, `financial/SOURCE_COVERAGE.md`, the
     central `docs/decisions/FOLLOW_UPS.md` core-conventions paragraph, and
     `INVARIANT_MAP.md` / `INVARIANTS.md` rows where they exist.
5. **Precondition for go:** the main checkout has uncommitted edits by another
   session to these same register files. Those edits must be committed to
   local `master` before the run branch is cut. Otherwise the run's closure
   edits would conflict with them at merge time. At setup, check
   `git status docs/decisions INVARIANTS.md` in the main checkout. If it is
   still dirty, log `blocked (uncommitted register edits)` and stop before
   slice 1. Never commit or discard another session's edits.

## Entry-to-slice map

| Register entry (Core conventions) | Slices | Closes after |
|---|---|---|
| Accepted shared barrier classification and timing (020) | A1 | A1 |
| Accepted SIFMA published-date, class and coverage (010) | A3, A3b (owner), H5 [data] | H5 |
| Accepted rolling/joint-calendar and roll-date (006, 009) | A2, C3, C4, E5 | E5 |
| Accepted Rate/Bps units, domains and precision (018) | B1 | B1 (term-loan step-up goes to its owner) |
| Accepted monetary precision, rounding and currency identity (012) | B2–B5, H6 [data] | H6 |
| Accepted holiday engine and jurisdiction calendars (004, 005) | C1, C2, H1–H4 [data] | H4 |
| Accepted calendar-input admission contract | C2 | C2 |
| Astronomical calendar generation and source provenance | H4 [data] | H4 |
| Accepted actual-day and coupon-context contracts (001, 002) | D1, D2, E6 | E6 |
| Accepted 30/360 family and interval boundary (003) | D3 | D3 |
| Day-count evidence and complexity prose need correction | D1, D3 | D3 |
| Context-dependent day counts exposed by context-free XIRR | G1 | G1 |
| Accepted schedule grid, event and failure contract (008) | E3, E4 | E4 |
| Accepted reporting/fiscal periods and LTM (014, plus 007 caller part) | E1, E2, F1–F4 | F4 |
| Accepted FX quote metadata and value-date boundaries (011) | F5 | F5 |
| Accepted FX point resolution, authority, state and shocks (013) | F6–F8 | F8 |
| FX state loading is a non-atomic merge (observation) | F8 | F8 |
| Accepted IRR/XIRR stream, clock, solver (017) | G1 | G1 |
| Accepted ratings and scorecards (019) | G2, G3 | G3 |
| Accepted cashflow representation and NPV boundaries (015, 016) | G4, G5 | G5 |
| **Decision:** CDS product boundaries and maturity composition | E4b (resolution O7, recorded in the owning record first) | E4b |

---

## Phase A: isolated quick wins (no upstream dependencies)

**A1 · 020 barrier timing (S)**
- Remove `Default`/`#[default]` from `BarrierType` and `PayoutTiming`
  (`core/src/types/barrier.rs:5,10,65,70`).
- Remove the `#[builder(default)] #[serde(default)]` on `rebate_timing` in:
  - `valuations/exotics/src/exotics/barrier_option/types.rs:50`
  - `valuations/fx/src/fx_barrier_option/types.rs:47`
- Fix the `Default::default()` callers: `barrier_option/pricer.rs:513,1029`,
  `pde_pricer.rs:695`, `benches/fd_greeks.rs:102`, `option_pricing.rs:94`.
- Stop the knock-in override: `models/src/closed_form/barrier.rs:564` sends
  every knock-in to the expiry rebate. Per O1, product validation rejects
  knock-in plus `AtHit` for the current no-hit rebate. The shared enums stay
  as they are; no new product type hierarchy is added.
- Docs: remove "side of spot" and "market standard". State that
  `expired_barrier_payoff` covers the expiry slice only.
- Regenerate the 10 schemas that carry `"default":"at_hit"`.
- Tests: wire tests for all three enums, missing `rebate_timing` in
  Rust/JSON/Py/WASM, and knock-in plus AtHit.

**A2 · 009 IMM option-expiry removal (S)**
- Delete `imm_option_expiry` and `next_imm_option_expiry` (`imm.rs:368,401`)
  and their re-exports.
- Delete them from every surface:
  - Python: `bindings/core/dates/imm.rs`, `_imm.pyi`, `__init__.pyi`.
  - WASM: `api/core/imm.rs:209-248`, `exports/core.js:200`.
  - Parity: `parity_contract.toml:116`.
  - Tests: `core_wasm_parity.json`, `test_dates_imm.py`,
    `core_parity.test.mjs`.
  - Docs: the notebook `futures_and_listed_options.ipynb` and the docs-site
    V1 futures page.
- Tests: equality and year transitions for the remaining lattice helpers.

**A3 · 010 SIFMA API (M)**
- Remove `Default` from `SifmaSettlementClass` (`imm.rs:445`) and the host
  defaults:
  - Python `unwrap_or_default` (`sifma.rs:111`).
  - WASM `sifmaSettlementDateDefaultClass` and the optional class in
    `exports/core.js:111-133`.
- `sifma_settlement_date` returns `Result` instead of `Option`.
- `next_sifma_settlement` takes a class, returns `Result`, and checks only the
  current and next month.
- Delete `estimated_sifma_settlement_date` (37 hits, including
  `facade-surface.json`). The TBA consumer maps the new `Result`.
- Replace the wall-clock 12-month test (`imm.rs:1052`) with fixed coverage,
  gap and equality cases.

**A3b · 010 `from_agency_term` and TBA class inference (S, resolution O2)**
- Per O2, `settlement_class` is required on both **TBA** and **DollarRoll**,
  which shares the same inference path.
- Delete `from_agency_term`, the agency/term inference branch at
  `tba/types.rs:298-311`, and the catch-all class.
- Remove the `agency_tba.schema.json:241` description and its generated
  UI/TS copies, and the matching DollarRoll schema text.
- Tests: missing class is an error in Rust, JSON, Python and WASM, for both
  products.

## Phase B: foundational value types

These must land before Phases D–G. The workspace must compile after each
slice.

**B1 · 018 Rate/Bps (M)**
- In `core/src/types/rates.rs`:
  - Normalize zero in every constructor and operation.
  - Remove `scale_decimal`.
  - `from_bp(f64) -> Result`.
  - `as_bp -> Result<f64>`; delete `as_bp_f64`.
  - `as_percent -> Result`.
  - Strict lexer for `FromStr`.
  - Scalar validation in `checked_mul`/`checked_div`.
  - `Bps::checked_div` errors on a remainder.
  - Shortest round-trip `Display`.
- Call sites: under 100 (core 9/12/21/18, py 5, wasm 4).
- Hosts: Python and WASM take an f64 bp and reject bools.
- Tests: the full 018 boundary table.
- **B1b · term-loan step-up (S, resolution O12):** in a separate commit
  after B1, the step-up at `term_loan/types.rs:942-944` keeps finite,
  nonnegative fractional-bp increases through checked Decimal arithmetic.
  Remove the `.round() as i64` whole-bp cast. Do not substitute
  `Bps::try_new`; whole-bp eligibility applies only where a contract states
  it.

**B2 · 012 decimal parser, float converter and wire (M)**
- `parse_decimal` owns the grammar and normalizes before the range test.
- `f64_to_decimal` uses shortest round-trip text, then the exact parser.
- `DecimalWire` goes through `parse_decimal` and emits normalized output.
- Python `int` is converted exactly.
- Signatures do not change. Behavior changes under roughly 3,900
  `Money::new` sites, so run the full `mise run all`.

**B3 · 012 Money arithmetic (M)**
- Exact-or-error `repr_add`/`repr_sub`.
- Canonical Decimal-scalar mul/div with named f64 adapters; nonzero underflow
  is an error.
- Direct binary64 ratio.
- `convert_at_rate`: canonical strictly positive Decimal rate, and same
  currency requires exactly 1.

**B4 · 012 rounding and config (M)**
- `round(scale, mode)` with scale 0..=28 or an error.
- Rename `AwayFromZero` to `MidpointAwayFromZero` with no alias; this touches
  48 JSON files.
- No default rounding mode for financial rounding.
- An absent ingest override means no rounding.
- `from_decimal_with_config` returns `Result`.
- Cap `MAX_DECIMALS` at 28.
- Per O10, remove the Python `Money.__round__` (`money.rs:359`) and its
  `.pyi` entry. Python exposes `round(scale, mode)` instead.

**B5 · 012 currency identity (M/L)**
- The generator emits non-numeric discriminants and a metadata table.
- Numeric lookup is unique-only (`TryFrom<u16>`).
- Per O10, `Currency`'s `Ord` becomes alphabetic code order. Currency-keyed
  maps and output are then deterministic and do not depend on the numeric
  codes. The margin SIMM sort (`simm_types.rs:343`) uses the same order.
- Python `__index__` on `Currency`.
- Adding XCG/ZWG is **[data]** work in H6.

## Phase C: calendar engine

**C1 · 004 fallible engine (L, keystone)**
- `HolidayCalendar::is_holiday`, `is_business_day` and `count_business_days`
  return `Result`. This touches 153 call lines in 22 files and three trait
  impls, plus the custom calendar in `daycount/context.rs:246`.
- Gate once on the nominal year inside `materialize_year`.
- Derive the candidate reach from the rule instead of `year±1`.
- Checked `Duration` arithmetic (`rule.rs:421-455`).
- Errors for invalid ordinals, impossible dates and missing lunar/equinox
  tables.
- No partial caching.
- Year cache: reject reversed ranges; prefix and scan results must agree.
- Hosts map the new errors.

**C2 · 004 admission and metadata (M)**
- Generator `deny_unknown_fields`, applied recursively.
- Required fields: `observed` (including `none`), `ignore_weekends`, `offset`.
- ID validation: lowercase ASCII, at most 32 bytes, unique under case folding.
- Rewrite all 28 JSON files to include the explicit fields.
- CSV readers check exact row widths. Table ranges are declared separately
  from the cache bounds.
- `CalendarMetadata` gains purpose, ranges, edition and evidence basis, in
  every host.
- This closes the calendar-input admission entry.

**C3 · 006 conventions and composites (M)**
- Remove `BusinessDayConvention` `Default` and its 17 uses, including schema
  defaults.
- Remove `resolve_calendar(None)` returning weekends-only, and the
  missing-leg drop in `resolve_joint_calendar`.
- Strict `joint_members` parsing.
- Composites validate every member before reducing.
- `adjust` compares year and month, not month alone.
- Coordinate with F5: three of the call sites are in `dates/fx.rs`.

**C4 · 006 searches (M, needs C1)**
- Remove `MAX_BUSINESS_DAY_SEARCH_DAYS` and the `AdjustmentFailed.max_days`
  payload.
- Bound the Nearest search.
- `add_business_days` uses the fallible uncapped step.
- Tests: fully closed months, `i32::MIN`, Modified across a year end.

## Phase D: day counts (needs B1 and C1)

**D1 · 001 boundary (M)**
- Validate structure, then the selected convention's requirements, then
  return the equal-date zero.
- Bus252: check `0 <= B <= D`.
- ISDA: integer complete years plus at most two partial fractions; remove the
  summation-order test.
- Remove the inner equal-date shortcuts and the analytics duplicate guard
  (`return_based.rs:56-96`).
- Swap the 4.16(d)/(e) citations, correct the Bus252 scan prose, and remove
  the "50-year" claim. Regenerate the day-count schema descriptions.
- Per O4, keep the `DayCountContext` representation and its public fields.
  Validation runs centrally in `year_fraction`/`signed_year_fraction`:
  first structure, then the selected convention's requirements, then the
  equal-date shortcut. The 107 struct literals are unchanged.

**D2 · ACT/365L end-year rule (S/M)**
- Always use the coupon-end year; remove the frequency requirement.
- Fix the A008 and ICMA citations.
- Update two golden values (365 becomes 366) and about 12 tests.

**D3 · 003 30/360 package (L)**
- Add `DayCount::Thirty360Isda` (label `30_360_isda`; flip the
  retired-alias test).
- `end_is_termination_date: Option<bool>` (about 330 hits, re-grepped). ThirtyE360Isda
  requires `Some`.
- Remove the raw `days_30_*` exports and their Python/WASM wrappers and parity
  rows. The golden harness compares `year_fraction*360` instead.
- Signed reversal keeps the termination status of the chronological later
  endpoint.
- Checked i32 arithmetic.
- Correct the alias prose.
- **[data]** Regenerate the QuantLib fixture independently, with the variant
  and termination inputs recorded. Drop the unverified `certified` flag and
  the wrong "both EOM" text.
- This closes the 003 entry and the evidence-prose entry.

## Phase E: tenor, schedules, ICMA

**E1 · 007 core arithmetic (S)**
- Representation-based count limits and checked casts in `add_to_date`.
- Rename `to_years` to `to_years_approx` (27 callers).
- Exact `to_days_approx`.
- `payments_per_year` rejects D/W. Per O5, the swaption par-yield
  cash-annuity method (`swaption/pricing/hw_cashflows.rs:46`) rejects D/W
  payment tenors, and only when that settlement method needs the nominal
  frequency. Physical settlement keeps using the actual scheduled cashflows.
  Make the lookup conditional on the method.
- `date_from_epoch_days` returns `Result`.
- Remove the WASM `i32` narrowing.
- Tenor bridge (007 #6, 002 #8): `to_years_with_context` takes the caller's
  `DayCountContext` instead of inventing frequency and coupon period, and
  errors when the adjusted end falls before the origin. This covers 7
  scenario callers plus the Python and WASM bindings. It is not blocked by
  CASHFLOWS-001; only the ICMA kernel in E6 is.

**E2 · 007 removals and rolling (M; resolutions O6, O13)**
- Delete `compounding_frequency`. Per O6, each instrument gains an
  independent, required `Compounding` quote convention, shared by pricing,
  quote inversion and sensitivities, and never derived from payment tenor.
  This replaces eight calls: bond Z-spread, three term-loan paths, three
  revolver paths and ABF CS01.
  - Examples and fixtures supply the convention explicitly.
  - Re-grep the call inventory before starting.
- Delete `from_years`. The ASW and scenario callers keep the actual tenor
  metadata instead of rebuilding a tenor from years.
- `months_until` returns `Result<u32>`. The core helper and its host
  bindings change in this slice. Per O13, the roughly 15 structured-credit
  consumers move to **E2b**.
- **E2b · structured-credit `months_until` consumers (M, separate commit).**
  - Completed calendar months do not automatically define seasoning,
    simulation indices or payment counts.
  - Before replacing the helper at each site (`oas.rs:162,322`,
    `cmbs.rs:47,60`, `pool.rs:919,1116`, `pool_flows.rs:748,761` and the
    rest), state which meaning that site needs.
  - Write the meaning into the structured-credit record and the commit body.
  - Sites whose meaning is not settled stay on a site-local, named helper
    and are listed in the commit body. They do not silently adopt
    completed months.
- `add_to_date` requires a convention, and only `Unadjusted` may have no
  calendar. Remove the host defaults.

**E3 · 008 schedule core (L)**
- Introduce `GridMode {Natural, EndOfMonth, QuarterlyImm}` in place of the
  booleans.
- An explicit `ScheduleSpec` with no defaults; `start < end`.
- Strict EOM and IMM anchors: no `.ok()` replacement search, no implicit
  stub.
- Remove `ScheduleErrorPolicy`, `ScheduleWarning`, `GracefulEmpty`, and the
  dedup helpers.
- The 100,000 ceiling counts final dates.
- Private validated `Schedule` state.
- Expose the nominal reference (anchor, frequency, roll).
- Hosts, schemas, parity and benches.
- Then remove `imm_on_or_after`/`_before` (009 #10).

**E4 · 008 CDS-mode removal (M)**
- Remove `with_cds_imm`, `cds_imm()` and the `prev_cds_date` start
  substitution.
- Remove the cashflows `effective_frequency`/`effective_stub` overrides and
  the `.ok()?` in `icma_coupon_period`.
- Replace the CDS-option `unwrap_or(as_of)` (`cds_option/types.rs:530`) and
  the tranche `contractual_start_date(as_of).unwrap_or(as_of)`
  (`sensitivities.rs:302`) with errors.
- **E4b · CDS contractual date regime (L, resolution O7).**
  - **Record the decision first.** O7 is a new decision, not
    reconciliation. The slice's first commit records it as accepted in the
    owning records: `calibration-framework/07-cds-materialization.md` and
    the CDS product record. It also rewrites the register's CDS Decision
    entry.
  - Remove the generic `RollRule::CdsImm` and its cashflows overrides.
  - The CDS product resolves five distinct dates: premium accrual,
    protection/step-in, upfront settlement, maturity, and default payment.
    Stub selection and endpoint inclusion belong to the selected regime.
  - Fix the pricer contradiction: `metrics.rs:40` requires `ShortFront`
    while the schedule machinery substitutes `ShortBack`.
  - Post-2015 single-name quoting is a named, explicitly selected regime.
    `calibration/src/build/cds.rs:51` is qualified against the official
    fixtures: 2015-12-22 + 3M gives 2016-03-20, and + 9M gives 2016-09-20.
    Today the 3M case produces a date already in the past.
  - Explicit contractual maturity dates stay literal. Other regimes need
    their own selected rules; there is no fallback.
  - Update the roughly 25 schemas and calibration examples 05 and 12 that
    carry `"cds_imm"`.

**E5 · 006/009 closure**
- Qualification sweep for the roll-date entry once C3, C4 and E3 have landed.

**E6 · 002 ICMA contained-period kernel (L)**
- Blocked on CASHFLOWS-001 long-stub composition. Removing the grid walk
  first would break long-stub bonds that today rely on
  `cashflows/src/builder/periods.rs:244`.
- Remove the frequency-only grid, the rounding inference and the 512 cap.
- Implement the contained-period formula with validation.
- Rewrite the 7 ICMA golden cases. The tenor bridge already landed in E1.

## Phase F: periods, LTM and FX

**F1 · 014 grammar and ordering (M)**
- Explicit `PeriodKind` rank and a structural total `Ord`.
- Case-insensitive signed grammar; reject Gregorian `FY`.
- Fallible `annual`.
- Context-aware `shift`; remove `next_fiscal`/`prev_fiscal`.
- Remove `FiscalConfig::default` and the country presets.
- The analytics FYTD path keeps an explicit `FiscalConfig::calendar_year()`
  when the fiscal configuration is omitted. Per O8, that January 1 rule is
  already accepted in ANALYTICS-003, so it becomes an explicit call rather
  than a `Default`.

**F2 · 014 state (M)**
- Make `PeriodCalendar` public and add it to `PeriodPlan`.
- Private validated `Period`/`PeriodPlan` state.
- Update the schemas.

**F3 · 014 LTM (M)**
- `ltm_window` takes a validated plan and requires the target to be present.
- Weekly is rejected; daily requires full calendar-day coverage.
- Errors propagate, which fixes the NaN mask in statements
  `formula_aggregates.rs:276` and the `.ok()?` in statements-analytics
  `reports.rs:299`.

**F4 · 014 `annualization_factor` (S/M)**
- Rename and remove `prior_observation_date`.
- Core slot counts stop using the statistical factor.
- Per O8, statements `annualize()`, `growth_rate` and rollup require
  explicit annualization factors, growth lags and rollup methods.
  - Remove the `periods_per_year` defaults (`formula_dispatch.rs:376`,
    `formula_timeseries.rs:218`, `rollup.rs:341,349`).
  - Rollup distinguishes flow sums, closing balances and recomputed ratios.
  - Dated CAGR stays on ACT/365F (ADR 0008).

**F5 · 011 FX quote and date (L)**
- Validated `(base, quote, pip_size)`; remove `FxQuoteConvention` and the
  `usd_quotation`/`settlement_days` ranking.
- Remove `fx_standard_settlement_days` and `fx_spot_date`.
- Add `fx_forward_date(spot, tenor, bdc, preserve_business_eom, calendar)`.
- Per O9, products prefer actual value dates. Where a date must be
  derived, the product owns the explicit rule.
  - NDF fixing and settlement stay separate.
  - XCCY legs are dated independently.
  - These rules apply to the five production callers: FX spot, forward,
    swap, NDF and XCCY.

**F6 · 013 config and policy (L)**
- Required `pivot_currency: Option<Currency>` and a required policy.
- Remove `PeriodAverage` and every FX `Default`.
- About 170 `FxMatrix::new` sites move mechanically to an explicit pivot.
- Regenerate the 156 `enable_triangulation` and 42 `period_average` JSON
  files.
- Per O9, every matrix supplies its pivot explicitly. Tests and fixtures
  use `Some(USD)` only where the dataset is deliberately a USD hub.
  Otherwise they use `None`, or the dataset's actual hub.
- Remove the statements `PeriodEnd` fallback
  (`capital_structure_runtime.rs:323`). The period-close aggregate
  conversion is already accepted in STATEMENTS-CAPITAL-09, so this is
  reconciliation work.

**F7 · 013 resolution (M)**
- Pinned before global.
- Ordered literal cache keys; reject stored identity quotes.
- Route only on `NotFound` and preserve the cause.
- Checked revision counter.
- Fallible snapshot hooks.

**F8 · 013 state and shocks (M)**
- Delete `load_from_state`; atomic fresh `try_from_state` with duplicate
  rejection.
- `with_bumped_rate(FxQuery, b)` on a frozen baseline with per-orientation
  baselines.
- Three callers: `ops_bump.rs:221`, `scenarios/src/adapters/fx.rs:25`,
  `valuations/fx/src/shared.rs:405`.
- This closes the 013 entry and the non-atomic-merge entry.

## Phase G: cashflows, NPV, IRR, ratings (needs B and D1)

**G1 · 017 IRR/XIRR (M)**
- Add a fallible fixed-bracket entry to the shared Brent
  (`math/solver.rs:454`) together with the core-numerics 05 owner.
- `xirr(flows)` uses fixed ACT/365F; remove the guess parameter.
- One canonical per-date net.
- The `!(finite && |r| <= 1e-8)` gate applies on every return path.
- Integer-offset periodic IRR.
- Reconcile the 19 consumer files.
- Per O11, loans separate the yield basis from coupon accrual.
  - Support explicit ACT/365F and ACT/360 annual-effective quotes, using
    the mathematically equivalent growth conversion (`loan_quotes.rs:84,415`).
  - EIR accretion uses the matching clock.
- Bond `Xirr` return floors are defined on ACT/365F only. Remove the
  `spec.day_count` override (`bond/types/return_floor.rs:62,310`).
- Remove the MC `SimulatedPath.irr` and its schema field.
- This closes the 017 entry and the context-free-XIRR entry.

**G2 · 019a CreditRating (M)**
- `D = 21` with NR off the ladder.
- Hand-written `PartialOrd` with no `Ord`.
- `notches_to` and `to_moodys_string` return `Result`.
- Finite alias tables replace `parse_credit_rating`.
- Re-key three `BTreeMap<CreditRating,_>`.
- Per O11 and the accepted 019 contract, Python ordering of NR against a
  rated value raises `TypeError`. WASM throws the mapped error.

**G3 · 019b registry and scorecards (M/L)**
- Remove `UnknownScalePolicy`, `default_score`/`default_scale_id`, the
  embedded rating-scale static/fallback, and `rating_scales.v2.json`.
  `embedded_registry.rs` itself stays because other loaders share it.
- Private validated scales with `classify`.
- Scorecards:
  - Resolve one snapshot.
  - Remove the `sp` default.
  - Remove the last-row fallback.
  - Error on unknown labels.
- Update hosts and parity.

**G4 · 016 NPV (M, needs B3)**
- Explicit `as_of` and `include_today`.
- Rename `npv_amounts_with_curve` to `npv_amounts`.
- Scalar empty returns 0, with no unused curve query.
- Use `df_between_dates`.
- Checked Neumaier.
- `flat_discount_factor` rejects underflow.
- About 20 product callers choose `include_today` explicitly.

**G5 · 015 CashFlow semantics (L, last)**
- Zero-cash PIK and DefaultedNotional.
- Rename `FloatReset` to `Floating` and add `ProtectionPayment`: 53 Rust
  files, 24 schemas.
- Rename `is_coupon`/`is_repayment_like`.
- An atomic principal delta and date.
- Remove the principal inference and opening-funding heuristics in
  `cashflows/src/builder/schedule.rs`.
- `DayCountContextState` in `CashFlowAccrual`.
- Private validated construction (56 `CashFlow::new` sites).
- CDS protection kinds, margin repo mapping, export fidelity.
- This closes the 015/016 entry.

## Phase H: data refresh and provenance [data]

Each item pins its source with URL, edition, retrieval date and hash next to
its CSV or JSON. Each regenerates the digest and adds exact closure and
open-date tests.

- **H1** Frankfurt/Hesse `defr` and NSW Anzac 2026/2027.
- **H2** Retire `cme`/`cnbe`. Per O3, no universal calendar replaces `cme`.
  - Commodity products (`conventions.rs:346`,
    `commodity_forward/types.rs:391,425`) require the contract's settlement
    calendar wherever dates are calculated.
  - NERC is restricted to labels and may not be used for settlement rolling.
    That restriction is already accepted in 005, so this is reconciliation.
  - H2 needs no external data. It can run right after C2, and does not wait
    for the other H items.
- **H3** SSE annual notices replacing the estimates and bridge. NSE and BSE
  split into separate data. Coverage errors for SGSI outside 2024–2027.
- **H4** Japan citizens'-holiday epochs and equinox published versus
  projected status. Lunar generator pinning and CNY provenance. Remove the
  cross-library claims. HK era bound.
- **H5** SIFMA:
  - Per-year closures replace the nearest-Friday rules and the Good Friday
    recurrence.
  - Pinned MBS settlement edition.
  - `sifma_rows` validated against the Date domain.
- **H6** XCG/ZWG and historical currency metadata against a pinned SIX
  edition.

## Owner resolutions

Resolved 2026-11-08. Rows marked **already accepted** restate an accepted
contract, so the slice is plain reconciliation. Rows marked **new** are
decisions. The slice that implements a new decision records it in the owning
record in the same commit, or in E4b's case in its first commit.

| # | Resolution | Status | Slice |
|---|---|---|---|
| O1 | Barrier: product validation rejects incompatible combinations (knock-in no-hit rebate plus `AtHit`). Shared enums kept; no new product hierarchy. | Within accepted 020 | A1 |
| O2 | TBA and DollarRoll require an explicit `settlement_class`. Remove the agency/term heuristics and the catch-all. | New, selected from 010's owner alternatives | A3b |
| O3 | No universal `cme` replacement: commodity products require the contract's settlement calendar. NERC is labels-only, with no settlement rolling. | NERC already accepted (005); product calendar new | H2 |
| O4 | Keep `DayCountContext` public; validate centrally before calculations and equal-date shortcuts. | Implementation choice within 001 | D1 |
| O5 | Swaption par-yield cash annuity rejects D/W tenors only when that method needs the nominal frequency. Physical settlement uses actual cashflows. | New | E1 |
| O6 | Each instrument gets a required independent `Compounding` quote convention, shared by pricing, inversion and sensitivities and never derived from tenor (8 calls). ASW and scenarios keep actual tenor metadata. | Within accepted 007 separation; per-instrument fields new | E2 |
| O7 | CDS uses a complete named contractual date regime: `CdsImm` removed; the product resolves accrual, protection, settlement, maturity and default-payment dates. Post-2015 single-name maturity is qualified against the official fixtures. Literal maturities stay literal. | **New:** closes the register's CDS Decision entry | E4b |
| O8 | FYTD with omitted fiscal config stays on January 1. Statements require explicit annualization factors, growth lags and rollup methods. CAGR is ACT/365F. | FYTD and CAGR already accepted; statements explicitness new | F1, F4 |
| O9 | FX: actual value dates preferred, with product-owned derivation rules. NDF fixing and settlement separate; XCCY legs independent; explicit pivot per matrix. The statements `PeriodEnd` fallback is removed. | Statements already accepted; product rules new | F5, F6 |
| O10 | Remove Python `Money.__round__` and expose `round(scale, mode)`. Currency-keyed ordered output uses alphabetic code order. | Within accepted 012 | B4, B5 |
| O11 | Loan yield basis is separate from accrual: ACT/365F and ACT/360 annual-effective quotes, with EIR on the matching clock. Bond `Xirr` floors are ACT/365F only, with no override. NR ordering raises `TypeError` in Python. | NR already accepted (019); yield and floor new | G1, G2 |
| O12 | Term-loan step-ups keep finite, nonnegative fractional bp via checked Decimal; remove whole-bp rounding. | New | B1b |
| O13 | Structured-credit `months_until` consumers: state each site's meaning (seasoning, simulation index, payment count) before replacing it. Completed months are not assumed. | Within accepted 007 boundary | E2b |

For B5 and O10: once currency discriminants are decoupled from ISO numerics,
replace the derived `Ord` on `Currency` with alphabetic code order.
`BTreeMap<Currency,_>` serialization then stays deterministic and
independent of the numeric codes. The margin SIMM sort uses the same order.

## Execution order (run queue)

When the user says go, run the slices **in exactly this order, one after
another, with a commit after each**. Each line gives the slice and the slices
it depends on. A slice runs only if every slice it depends on is committed.

| # | Slice | Depends on |
|---|---|---|
| 1 | A1 | — |
| 2 | A2 | — |
| 3 | A3 | — |
| 4 | A3b | A3 |
| 5 | B1 | — |
| 6 | B1b | B1 |
| 7 | B2 | — |
| 8 | B3 | B2 |
| 9 | B4 | B3 |
| 10 | B5 | B4 |
| 11 | C1 | — |
| 12 | C2 | C1 |
| 13 | H2 | C2 |
| 14 | C3 | C1 |
| 15 | C4 | C1, C3 |
| 16 | D1 | B1, C1 |
| 17 | D2 | D1 |
| 18 | D3 | D1 |
| 19 | E1 | D1 |
| 20 | E2 | E1 |
| 21 | E2b | E2 |
| 22 | E3 | C3, C4, E1 |
| 23 | E4 | E3 |
| 24 | E4b | E4 |
| 25 | G1 | B3, D1 |
| 26 | G2 | — |
| 27 | G3 | G2 |
| 28 | G4 | B3 |
| 29 | F1 | E1 |
| 30 | F2 | F1 |
| 31 | F3 | F2 |
| 32 | F4 | F3 |
| 33 | F5 | C3, E1 |
| 34 | F6 | B3, F5 |
| 35 | F7 | F6 |
| 36 | F8 | F7 |
| 37 | G5 | G4, D3, E3 |
| 38 | E5 | C3, C4, E3 |
| 39 | E6 | E3, G5, and CASHFLOWS-001 landed on the base branch |
| 40 | H1 | C2 |
| 41 | H3 | C2 |
| 42 | H4 | C2 |
| 43 | H5 | C2, A3 |
| 44 | H6 | B5 |

Close each register entry in the commit that completes it.

## Unsupervised execution protocol

The run is designed to proceed with no human in the loop. Follow these rules
exactly. Where a situation is not covered, choose the more conservative
action and log it. Do not stop to ask.

### Setup (once)

1. **Isolated worktree.** Create a worktree on a new branch
   `refactor/core-conventions-reconciliation` cut from **local** `master`,
   not `origin/master`. Verify with `git log -1`. This keeps the run away
   from other sessions' uncommitted edits in the main checkout, especially
   in `docs/decisions/**`. Check the precondition in Closing register
   entries, item 5, first. Use the worktree environment fixes in the
   worktree-traps memory: `/usr/bin/git` for chained git, `python-sync`,
   `npm ci` with a scratchpad cache, and `MISE_CEILING_PATHS`.
2. **Baseline.** Run `mise run all` once before any slice and record the
   result in the status file. If the baseline already fails, record the
   failing checks as **pre-existing**. Per-slice gates are then judged
   against the baseline: the slice must not add failures. Do not fix
   pre-existing failures unless they block a slice; if one does, fix it in
   its own commit, labelled `baseline:`.
3. **Status file.** Create `docs/plans/CORE-CONVENTIONS-RECONCILIATION-STATUS.md`.
   It is the run's single source of truth: after a context compaction,
   resume from it, not from memory. It holds:
   - One row per queue slice: status (`pending`, `in_progress`,
     `committed <sha>`, `blocked`, `skipped`, `failed`), the gates run and
     their results, the register entries closed or rewritten, and notes.
   - A log of every judgement call, deviation or ambiguity, with the choice
     made.
   - The status file is updated and committed as part of each slice commit.
     It is not a separate commit.
4. **Do not edit this plan file** during the run. AGENTS.md treats it as
   frozen once execution starts. All progress goes in the status file.

### Per-slice loop

For each queue row, in order:

1. Skip it if a dependency is not `committed`. Mark it `skipped (dep <id>)`
   and move on.
2. Mark it `in_progress`. Re-grep the slice's call-site inventory, since
   counts here are point-in-time, and re-read its record's
   `#accepted-decision-…` section. If HEAD has diverged from the plan's
   description, implement the record's contract, not the plan's wording.
   Log the divergence.
3. Implement the full triplet: Rust, then Python bindings and `.pyi`, then
   WASM and `exports/**`, then `parity_contract.toml`, schemas, fixtures and
   the follow-up register edit. Add the record's qualification tests.
4. Run the gates:
   - `mise run rust-fmt`
   - `mise run rust-lint-crate -- <pkg>` and `mise run rust-test-crate -- <pkg>`
     for each touched crate
   - `mise run gen-write`
   - `mise run python-build`, then `mise run python-test-file` for the
     touched tests
   - `mise run wasm-test` when the WASM surface changed, else
     `mise run wasm-test-built`

   Run anything likely to exceed about 2 minutes in the background. After
   any failed or abandoned run, kill leftover cargo, clippy, nextest or
   maturin processes.
5. **Fix loop.** If a gate fails, diagnose and fix. Allow at most **3 full
   fix attempts** per slice. A fix may not weaken a test, add an
   `#[allow]`, reintroduce a default or fallback, or reopen an accepted
   decision.
6. **Success:** stage only this slice's files, by hunk where a file is
   shared. Commit with the message format from Ground rules, ending with the
   attribution trailer. Mark the row `committed <sha>`.
7. **Failure after 3 attempts:**
   - Discard the slice's uncommitted changes with
     `git restore --staged --worktree .` inside the worktree. That returns
     the tree to the last slice commit; nothing committed is lost.
   - Mark the row `failed`, with the failing command, the error excerpt and
     the attempted fixes.
   - Its dependents will be skipped by step 1. Continue with the next row.
8. **Phase gate.** After the last queue row of each phase letter (A, B, C, D,
   E, F, G) and once at the very end, run the full `mise run all`.
   - Fix any new failure in its own commit, `phase-<letter>: fix …`, with
     the same 3-attempt limit.
   - If it still fails, log it and continue. Later slices are judged
     against the new failure set.

### Blocked-by-design rows (never forced)

- **E6:** runs only if CASHFLOWS-001 long-stub composition is already on the
  base branch. Otherwise mark it `blocked (CASHFLOWS-001)`.
- **H1, H3–H6:** data may be entered only from a primary source that was
  actually retrieved in this run, with its URL, edition, retrieval date and
  SHA-256 recorded next to the data file. Check with WebFetch; standard
  search first.
  - If a source cannot be retrieved or does not contain the needed dates,
    mark the item `blocked (source)` and list what is missing.
  - Never fill dates from memory, secondary sites or estimates. Partially
    sourced data is committed only for the years actually covered, with
    explicit coverage errors outside them.
- **New decisions (A3b, E2, E4b and the other "new" O-rows):** record them in
  the owning record as part of the slice, per the Owner resolutions table.
  The resolutions are the user's authority for this.

### Hard limits during the run

- No `git push`, no PR, no force operations, and no history rewriting of
  existing commits. Everything stays local on the run branch for review.
- Never touch the main checkout's working tree.
- No new dependencies unless a record requires one. Log any that are added.
- Do not change any convention beyond the record and the Owner resolutions.
  If a record is genuinely ambiguous on something the slice cannot avoid,
  take the strictest reading (error rather than default). Log it under
  "Needs review" in the status file.

### End of run

1. Run the final `mise run all` and record the result.
2. Write a summary at the top of the status file:
   - Committed slices with their SHAs.
   - Failed, blocked and skipped rows, with the reason for each.
   - The register entries deleted and those rewritten.
   - The "Needs review" items.
   - The final gate result.
3. Send the status file to the user (SendUserFile, `proactive`) and stop.
   Do not merge the branch.
