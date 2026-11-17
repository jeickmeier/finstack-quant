# Valuations & Bindings Quant Audit Remediation Plan (rewritten against source)

- Rewritten 2026-11-16 against the working tree on `master` (`8f0beb709d` plus uncommitted docs changes).
- Supersedes the 8-slice draft of the same name. Finding numbers 1–19 are kept from that draft.
- The audit that produced the findings is not in `docs/audits/`. Every status below comes from reading current source, not from running tests.

## What changed from the draft

1. **Stale findings are dropped or narrowed.** Several were already fixed or described code that does not exist.
2. **Every slice is vertical.** A slice that changes a wire type, constructor or metric unit carries its Python and WASM bindings, `.pyi`, parity ledger and `mise run gen-write` in the same commit. The draft deferred all of that to a final slice, which leaves the binding crates uncompilable in between.
3. **Slice 0 reproduces before anything is fixed.** Findings I could not confirm by reading get a failing test first; a finding with no failing test is closed. For a convention item the expected value must cite an accepted record or an external reference, so the test does not just encode the draft's opinion.
4. **Accepted decision records win.** Where a finding contradicts an accepted record, the code change is blocked until the record owner amends the record. The plan does not rewrite records to fit code.

## Finding status

Status key: **Open** = confirmed in source. **Narrowed** = real, but smaller or elsewhere than the draft said. **Repro** = not confirmed by reading; needs a failing test in Slice 0. **Decision** = contradicts an accepted record or a documented deliberate choice. **Closed** = already fixed or not present.

| # | Finding | Status | Evidence in current source |
|---|---|---|---|
| 1 | FX spot/delivery dates | Narrowed | `FxOption` has no `delivery_date` or spot lag. `FxForward` already has `base_calendar_id` / `quote_calendar_id`, and `CORE-CONVENTIONS-011` records the joint-calendar spot rule as runtime-reconciled on 2026-11-13. Only the `FxOption` part is open; the `FxForward` part is Repro. |
| 2 | Same-day cash default | Open, redesigned | Trait default is `true` (`traits/instrument.rs:740`). Ten impls in nine files override it, all to `false` (bond, IRS, cap/floor, convertible, ILB, CMS swap/option, both inflation swaps, XCCY). No credit, FX, exotics, equity or structured-credit impl overrides it. Flipping the default would silently change every unlisted impl. `CORE-CONVENTIONS-016` #2 makes same-day cash a product decision. |
| 3 | Carry uses constant-tenor roll | Decision | `VALUATIONS-FRAMEWORK-13` item 4 (accepted 2026-11-14): carry holds the opening dated market; theta conditions at the closing date; elapsed days are not added to a source's reference. The draft's change contradicts that item. Current code holds the opening market in carry (`carry_decomposition.rs:156`) and conditions in theta (`theta.rs:176`); the record's own implementation status is still "reconciliation pending", so I am not claiming the code is fully reconciled. No Finding 3 change unless the record is amended. |
| 4 | Composite FX and non-additive metrics | Repro | `CompositeInstrument::market_dependencies` already adds an FX pair to the reporting currency. The metric classification gap is not confirmed. |
| 5a | Par-quote bucketed DV01 | Open | `QuoteBump::TenorsBp` bumps the single closest quote (`recalibration/rates.rs:820`), not a partition-of-unity weight. |
| 5b | CDS bucketed DV01 without hazard replay | Open | `cds/metrics/mod.rs:113` registers `BucketedDv01` on the plain triangular calculator; only parallel DV01 uses `rate_bumped_market_with_hazard_replay`. |
| 5c | Quoted bond `Dv01` alignment | Repro | `bond/metrics/dv01.rs:80` already routes through `with_bond_risk_view`. Likely already done. |
| 5d | Seasoned-curve key-rate anchoring | Repro | Triangular bumps are built in curve time (`dv01.rs:450`). Sign inversion on `base_date < as_of` is not confirmed. |
| 6 | ICMA reference period, Street final period | Decision | `icma_reference_period` is already threaded through the solver. The single-cashflow branch compounds for `Street` and has a separate simple-yield `Moosmuller` variant (`ytm_solver.rs:86`), so final-period simple yield under `Street` is a convention choice, not a bug to reproduce. |
| 7 | Ex-coupon holder vs settlement quote | Closed pending repro | `bond/pricing/settlement.rs` already separates the holder value from the settlement quote, with a test that the cum-coupon holder value includes the detached coupon. |
| 8 | OAS `1e12` sentinel | Closed | No sentinel in `oas.rs`. `BrentSolver::solve` takes a fallible closure and propagates the first error. `solve_fallible` does not exist. |
| 9 | OIS accrual adjustment, same-day fixing | Decision + Repro | Accrual-adjust flags are already plumbed through `irs/cashflow.rs`; whether `from_conventions` sets them for overnight indices is Repro. Same-day fixing: `overnight.rs:660` values at start of day, before the same-day fixing is published, and a test asserts that. `conventions/0045` says same-day fixing selection follows an explicitly declared phase and introduces no default same-day policy, so consuming an `as_of` fixing is a Decision. |
| 10 | CDS first coupon and clean upfront | Decision + Repro | `dates[0] = cds.premium_leg.start` (`cds/pricing/metrics.rs:82`) is commented as deliberate ("the explicit premium start remains the contractual first accrual boundary"). The CDS date regime was reworked in the conventions plan (E4b-4 committed, five-date resolver still open, two slow-tier CDS goldens failing). Changing the first boundary belongs to that work, not this plan. Upfront filter `dt >= as_of` is confirmed at `metrics.rs:515` and `engine.rs:479`. Cash-settlement discounting of clean accrued is Repro. |
| 11 | Option Greeks: rho units, zero vol, 30/360 | 11b Closed; rest Repro | `FxOptionGreeks` is `pub(crate)` and not bound in Python or WASM. Its rho is per 1%, and `fx_option/types.rs:548` plus the `fx_digital_option` twin divide by 100 to publish `rho_bp` per bp, so the published unit is already correct. Zero-vol NaN, the rho day-count Jacobian and the 30/360 expiry case are Repro. |
| 12 | Python/WASM pricing options, stub kind | Narrowed + Repro | `stub_kind_from_py` maps `None` to `StubKind::ShortFront`, and its doc comment says so; the draft's `StubKind::None` claim is wrong. Whether `None` should be rejected is a small API call, not a silent bug. Missing `config` / `overrides` / `market_history` on typed `metric` / `greeks` is Repro. |
| 13 | `zero_rate_from_df` validation order | Open | Signature is `(df, t, context)`. `t <= 0.0` returns `Ok(0.0)` before `df` is validated (`helpers.rs:153`). |
| 14 | Basket NAV, delta, weight risk | Open + Repro | Spot value subtracts one day of `expense_ratio / days_in_year` (`basket/pricer.rs:57`, `:101`). Whether that drag is wrong for spot NAV needs a cited convention. The single-constituent weight-risk case and the duplicate-ticker overwrite in `constituent_delta.rs` are Repro. |
| 15 | Credit-adjusted cashflow export | Open + Repro | Every `Err` becomes a `cashflow_trace_unavailable` trace step (`cashflow_export.rs:388`): Open. `prev_sp` advances on every row (`:646`), so a second row on the same date gets zero conditional default probability; that may be intended (the default mass is counted once per date), so it is Repro against the reconciliation with `Instrument::value`. |
| 16 | Missing curves, bond CS01, bucket labels | Narrowed + Repro | `collect_curves` only fails when `curves.is_empty()` (`dv01.rs:280`), so a partly missing set passes. The `{:.0}y` label pattern is not in `cs01_direct_hazard.rs` or `cs01_z_spread.rs`: Closed. Bond CS01 central bump and bucket additivity are Repro. |
| 17a | `FxDeltaVolSurface` / `VolCube` in risk | Narrowed + Repro | Pricing already resolves delta surfaces through `resolve_fx_sigma`. No reference to either type exists under `core/src/metrics`, so vega, cross-gamma and VaR coverage is the open part; confirm with a test. |
| 17b | Cent-rounded finite differences | Narrowed | `finite_difference.rs` and `option_greeks.rs` already use raw values. Still rounded: `fx/src/shared.rs:381–491` and `cds/metrics/recovery01.rs:46` (check the index, tranche, option and structured-credit twins). |
| 18 | Historical VaR | Narrowed + Repro | `window_days` is on `MarketHistory`, not `VarConfig`; it is a constructor argument, bound in Python with a getter and repr, and not read by any calculation. Fixed `STANDARD_BUCKETS_YEARS` grid confirmed at `market_history.rs:338` and `:498`. Hazard replay trigger and Taylor precompute are Repro. |
| 19a | Non-finite measures reach JSON | Open | `ValuationResult::with_measures` and the `measures` map have no finiteness check. `measures` is a `pub` field, so a setter check alone does not close it. |
| 19b | WASM untrusted market JSON, member parity | Open, relocated | Unbounded `serde_json::from_str` of the market is at `wasm/src/api/valuations/pricing.rs:45`, `:809` and `typed/mod.rs:235`, not `market.rs`. `PROPERTY_CHECKED_PREFIXES` covers only models and cashflows. Five Python members are absent from `facade-surface.json`: `lender_cashflows`, `net_proceeds_fraction`, `total_size`, `is_non_deferrable`, `payment_priority`. Some may be plain-data `literal` members that must not get a WASM function. |
| 19c | `CoverageRules.borrowing_base`, `ccc_bucket` doc | Open + Repro | `PyCoverageRules` has no `borrowing_base`. The `ccc_bucket` unit wording is Repro. |

## Decisions needed before the affected slice starts

| Item | Question | Default if unanswered |
|---|---|---|
| Finding 3 | Amend `VALUATIONS-FRAMEWORK-13` to a constant-tenor carry experiment, or keep the accepted record? | Keep the record; Finding 3 is closed. |
| Finding 6 (Street) | Should `Street` switch to simple yield inside the final coupon period? | Keep compounding; drop this part. |
| Finding 9 (fixing) | Should a fixing published for `as_of` be consumed, against the start-of-day rule? | Keep start-of-day; drop this part. |
| Finding 10 (first coupon) | Should the standard regime keep the previous roll date as the first accrual boundary? | Leave it to the conventions plan's CDS date work; drop this part here. |
| Finding 12 (stub) | Reject `None`, or keep the documented `ShortFront` default? | Keep the default. |
| Finding 14 (drag) | Is one day of expense drag wrong in spot basket NAV? | Keep it until a convention is cited. |

## Slices

Each slice ends with scoped lint and tests for the crates touched, then `mise run gen-write` when a wire type, binding or facade changed, then `mise run slice-gate`, then its commit. `mise run all` runs once at the end. Doctests stay in the nightly tier.

### Slice 0 — Reproduce

- Write one test per Repro item in the table, in the owning crate's test tree, marked with the finding number. For a convention item, the expected value cites an accepted record or an external reference.
- A Repro item whose test passes on current source is closed and removed from the later slices.
- No production code changes and no commit. `#[ignore]` is the slow suite here (`--slow` runs the ignored tests), so a failing test cannot be parked behind it. Each failing test lands in the slice that fixes it; update the status table in this file with the outcome.

### Slice 1 — Input and result validation (13, 19a)

Crates: `finstack-quant-valuations-core`.

- **13:** In `zero_rate_from_df(df, t, context)`, validate `t` finite and non-negative and `df` finite and positive before the `t == 0.0` early return. Keep the signature and the `context` in the error text.
- **19a:** Reject non-finite values where measures enter a `ValuationResult`, with a `Validation` error naming the `MetricId`. `measures` is a `pub` field, so first list every writer; either make the field private behind a checked setter or validate once at the serialization boundary. Follow the existing cross-binding return contract for non-finite numbers (serde writes infinity as `null` and cannot read it back) rather than inventing a second rule. Update the Python/WASM result constructors in this slice.

### Slice 2 — Same-day cash policy made explicit (2, upfront part of 10)

Crates: all `valuations-*` family crates, `finstack-quant-valuations`.

- Remove the default body of `Instrument::includes_valuation_date_cashflows` so every impl must state its policy, and write an explicit `true` on each of the roughly 95 impls that inherit it today. This commit changes no numbers.
- Then, one instrument family per commit, add a roll-conservation test (`PV(t0) == PV(t1) + PeriodCash(t0, t1)` on a zero-rate market across a payment date). Where it fails, the flag disagrees with the pricer's `>` / `>=` comparison; flip the flag. Period-cash and carry metrics read this flag (`period_cash.rs:91`, `:103`, `:126`), so each flip moves those numbers and needs its goldens regenerated in the same commit.
- CDS, CDS index and CDS tranche dated upfront: move `dt >= as_of` to `dt > as_of` only if the CDS record confirms strict ex-cash; cite the record section in the commit.

### Slice 3 — Finite differences and cashflow export (15, 17b)

Crates: `finstack-quant-valuations-core`, `-fx`, `-credit`, `-structured-credit`.

- **17b:** Replace `instrument.value(..)?.amount()` with the raw reprice in `fx/src/shared.rs` and the `Recovery01` calculators. `finite_difference.rs` and `option_greeks.rs` need no change.
- **15:** Update `prev_sp` only when the row date advances. Return an empty schedule for a matured instrument whose target PV is within tolerance. Return `Err` for a reconciliation residual above tolerance on a non-empty schedule; keep the trace step only for the model routes that have no dated schedule.

### Slice 4 — Rate and credit bucketed risk (5a, 5b, 16 missing-curves, and 5c/5d/16-CS01 if reproduced)

Crates: `finstack-quant-calibration`, `finstack-quant-valuations-core`, `-rates`, `-credit`, `-fixed-income`.

- **5a:** Replace the closest-quote selection in `QuoteBump::TenorsBp` with triangular partition-of-unity weights over the bucket grid, flat on both wings. Skip buckets with zero weight on every quote. Test: sum of buckets equals parallel DV01 within 1e-3 relative.
- **5b:** Give single-name CDS a bucketed DV01 that replays the hazard bootstrap per bucket, reusing `rate_bumped_market_with_hazard_replay`. Check how index, tranche and CDS option register `BucketedDv01` before extending it to them.
- **16:** Fail in `collect_curves` whenever any required curve is missing. The single-curve OIS `Pv01` case only if reproduced.
- **5d**, **5c**, bond CS01: only if Slice 0 reproduced them. For 5d, anchor the triangular bump at `as_of` on a seasoned curve; name the new curve-transform method in this slice rather than assuming one exists.

### Slice 5 — FX option dates and Greeks (1, 11)

Crates: `finstack-quant-valuations-fx`, `-equity`, `finstack-quant-models`, `finstack-quant-py`, `finstack-quant-wasm`, `finstack-quant-ui` fixtures.

- **1:** Add `delivery_date`, a spot lag and the two calendar ids to `FxOption`, using the field names the FX forward family already uses (`settlement_days`, `base_calendar_id`, `quote_calendar_id`) rather than a new `spot_settlement_days`. Validate `delivery_date >= expiry` and require both calendars when the lag is positive. Reuse the joint-calendar spot rule that `FxForward` already owns under `CORE-CONVENTIONS-011`; do not add a second implementation. Forward over spot-to-delivery, discount to delivery, volatility time to expiry.
- **11:** zero-vol Greeks, the day-count Jacobian on analytical rho, and the 30/360 expiry case, each only if reproduced.
- Same commit: `PyFxOption` constructor, getters and `.pyi` doctests; WASM `FxOption` builder and JSDoc; instrument JSON schema; parity ledger; `mise run gen-write` for `index.d.ts`, `facade-surface.json` and UI fixtures.

### Slice 6 — Basket, composite and historical VaR (14, 18, and 4/17a if reproduced)

Crates: `finstack-quant-valuations-core`, `finstack-quant-valuations`, `finstack-quant-py`, `finstack-quant-wasm`.

- **14:** Validate `days_in_year > 0` and a finite non-negative `expense_ratio`. Remove the one-day expense drag from spot value only if the Finding 14 decision says so. Single-constituent weight risk and duplicate-ticker delta only if reproduced.
- **18:** Remove `window_days` from `MarketHistory` (constructor, serde, schema example, Python constructor, getter, repr and `.pyi`, WASM wrapper) in one commit. Building key-rate shocks from each scenario's own sorted tenors instead of `STANDARD_BUCKETS_YEARS` only if a sparse-grid test shows a dead zone.
- **4**, **17a**, the VaR hazard-replay trigger and the Taylor precompute: only if reproduced.

### Slice 7 — Binding-only gaps (19b, 19c, 12 if reproduced)

Crates: `finstack-quant-py`, `finstack-quant-wasm`.

- **19b:** Route the three WASM parse sites through `MarketContext::from_state_slice` with `LoadLimits::default()`.
- **19b:** Classify the five missing members first. A member that only reads a stored field is `literal` and gets a ledger exclusion, not a WASM function; `payment_priority` and `is_non_deferrable` look like that. A member that computes in Rust (`total_size`, `net_proceeds_fraction`, `lender_cashflows` are candidates) gets a WASM twin.
- **19b:** Add `"finstack_quant.valuations."` to `PROPERTY_CHECKED_PREFIXES` and record the `literal` exclusions the test then reports. Do this last in the slice; the backlog ceiling may only shrink.
- **19c:** Add `borrowing_base` to `PyCoverageRules` with its `.pyi` entry and WASM twin.
- **12:** `config`, `overrides` and `market_history` on typed `metric` / `greeks`, if reproduced.

### Final gate

- `mise run gen-write`
- `mise run all-fmt`
- `mise run all`

## Not verified in this rewrite

- No test was run; every status is from reading source.
- The Repro rows are open questions, not confirmed defects.
- The decision records for CDS upfront timing, FX option delivery, same-day cash (`CORE-CONVENTIONS-016`) and fixing eligibility (`conventions/0045`) were grepped, not read end to end; Slices 2 and 5 must cite the governing section before changing numbers.
- `docs/plans/CORE-CONVENTIONS-RECONCILIATION-STATUS.md` tracks in-flight FX and CDS date work that overlaps Findings 1, 9 and 10; check it before starting Slices 2 and 5.
