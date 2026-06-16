# Quant Review — `finstack-quant-cashflows` + bindings (re-review)

**Date:** 2026-06-13
**Scope:** `finstack-quant/cashflows` crate and its Python/WASM bindings.
**Method:** Independent verification-and-extension pass over the *committed* crate (the 2026-06-09 remediation is now landed in `10dab412d`, with `2b7c805f0` for B1/B2/M6). Six parallel deep-review agents over distinct subsystems (date/schedule generation, accrual engine, floating-rate/fixings, aggregation/DataFrame, specs/emission/pipeline, bindings/json), followed by personal source-level verification of every headline claim.

## Verdict

The crate is in materially better shape than the 2026-06-09 baseline. The two Blockers and the bulk of the 15 Majors are correctly fixed; I re-verified **B1/B2/M6/M7/M9/M10/M2/M14/M11/M12/M4/M5** and the NaN/sign/atomicity guards against current source and they hold. The remaining risk concentrates where the remediation did not fully reach: the accrual engine still mixes year-fraction bases for **ICMA stubs**, and two binding-facing config types never received `deny_unknown_fields`. The findings below are the residual and net-new issues; each was personally traced unless marked *(reported)*.

## Findings

### Major

**1 — ICMA stub coupons under-accrue (~10%) in the common builder path.**
`finstack-quant/cashflows/src/accrual.rs:516-530` and `:578-599`.

Coupon emission stamps the **quasi-coupon** year-fraction onto stub flows — `finstack-quant/cashflows/src/builder/emission/coupons.rs:300-309` computes `yf` with `coupon_period: (!is_stub).then_some(...)`, i.e. `None` for stubs, so the builder's `accrual_factor` uses core's frequency-based quasi-coupon grid. The accrual engine then takes that stamped `accrual_factor` as `total_yf` (`accrual.rs:516-517`) but computes `dc_elapsed` with `coupon_period: Some((p.start, p.end))` (`accrual.rs:582`) — the **stub-as-its-own-reference** basis. Core returns `(days/period_days) · months_until(start,end)/12` for that branch (`finstack-quant/core/src/dates/daycount.rs:928-934`), so the two numbers live on different bases that do **not** cancel. The M1 rescale at `accrual.rs:590` only triggers when `dc_elapsed > total_yf`; for a short first coupon `dc_elapsed` (≈5/12) stays *below* `total_yf` (≈0.4613), so it never rescales.

*Worked example:* issue 2025-01-15, first semi-annual coupon 2025-07-01 → on the day before payment, linear AI = `C × 0.41667/0.4613 ≈ 0.903·C`. Accrued reaches only ~90% of the coupon and then jumps discontinuously at payment.

*Impact:* wrong accrued interest / dirty price on the first or last coupon of any ICMA bond with a stub — the dominant new-issue case. This is the residual of the original M1 (basis mixing), now isolated to stubs.

*Fix:* make the elapsed fraction self-consistent — `elapsed = total_yf × dc_elapsed/dc_total` **always** (both `dc_*` under the same context, so the reference-period basis cancels), instead of only in the `dc_elapsed > total_yf` branch; or carry the regular/stub flag onto `CouponBucket` and pass `coupon_period: None` for stubs (mirroring `periods.rs:68`).

**2 — Strict-serde invariant violated on two public binding inputs.**
`finstack-quant/cashflows/src/accrual.rs:204` (`AccrualConfig`), `:150` (`ExCouponRule`).

Both are deserialized directly by `accrued_interest_json` (the Python/WASM bridge) yet neither carries `#[serde(deny_unknown_fields)]`. A misspelled `method`, `ex_coupon`, or `days_before_coupon` key is silently ignored and the financially-significant default is used. This is **provably live**: `finstack-quant-py/tests/test_cashflows.py:163` passes `"strict_issue_date": True` — a field deleted from Rust in `d8981646f` — and the test still asserts success only because the key is silently dropped.

*Impact:* a typo'd ex-coupon/method config produces wrong accrued with no error; direct breach of the project's strict-serde cross-cutting invariant on a binding surface. Residual of M13.

*Fix:* add `deny_unknown_fields` to both types and update the `test_cashflows.py` fixture (the test will otherwise fail — which is the point). `CashFlow` and `TenorDe` in core are also missing it (Minor below).

### Moderate

**3 — Term-rate seasoning boundary keyed on `accrual_start`, not the fixing/reset date.**
`finstack-quant/cashflows/src/builder/emission/coupons.rs:826`.

`let projected = if accrual_start < fwd.base_date() && resolved_fixing.is_some()` decides "use the realized fixing vs re-project," but the fixing is looked up at `reset_date` (`coupons.rs:831`), and a term reset fixes T-2 *before* `accrual_start`. A live coupon whose `accrual_start == base` (or lands within `reset_lag` business days after base) has an already-published fixing yet falls through to projection off today's curve. Term SOFR loans (the dominant US loan convention) hit this at period rolls.

*Impact:* the current/most-material coupon of a seasoned term FRN/loan re-projects instead of reading its published fixing — wrong current coupon and accrued by the reset-lag rate drift. Residual sliver of M3 (the overnight path already keys per-observation correctly).

*Fix:* gate on the fixing date: `if reset_date < fwd.base_date() && resolved_fixing.is_some()`.

**4 — Index floor/cap applied to the period-compounded rate, not per daily fixing.**
`finstack-quant/cashflows/src/builder/emission/coupons.rs:776-788`.

`compute_overnight_rate` produces the compounded index, then `calculate_floating_rate` applies floor/cap/gearing/spread once to that period rate. The ARRC/LSTA convention for floored SOFR loans floors **each daily fixing** before compounding (`compound(max(SOFRᵢ, f))`), which is convex and ≥ the period-floor result.

*Impact:* silently understates floored compounded-SOFR coupons whenever daily fixings straddle the floor; the convention is unavailable and the deviation undocumented. Not addressed by the remediation.

*Fix:* thread the index floor/cap into the daily sampler so each rate is floored before compounding; apply only all-in floor/cap, gearing, spread to the compounded result. Make daily-flooring selectable since the period-floor convention also exists.

**5 — DataFrame credit-adjusted PV diverges from the canonical credit-PV path.**
`finstack-quant/cashflows/src/builder/dataframe.rs:606-616` vs `finstack-quant/cashflows/src/aggregation.rs:501-534`.

With a hazard curve, the DataFrame multiplies `df·sp` onto *every* cash kind, while `credit_adjusted_period_pv` zeroes `DefaultedNotional`, discounts `Recovery`/`AccruedOnDefault` at `df` with **no** survival factor, and adds the `r·(1−sp)` recovery term on surviving principal. The class docstring claims parity with the credit-adjusted PV path.

*Impact:* the two public PV exports of the same credit schedule don't reconcile — the DataFrame down-weights realized recoveries by `sp`, leaves defaulted notional in PV, and omits recovery-on-survival.

*Fix:* route DataFrame PV through the same per-kind logic, or drop the parity claim and document the "survival-only" view explicitly.

**6 — DataFrame export skips the sort/period validation that aggregation enforces.**
`finstack-quant/cashflows/src/builder/dataframe.rs:502-551`.

`to_period_dataframe` walks `&self.flows` with a forward-only `period_cursor` that never resets, with no `validate_periods` call and no sortedness guard. `CashFlowSchedule.flows` is a public field; aggregation paths either sort internally or hit a `debug_assert` and hard-reject unsorted/overlapping/duplicate periods via `validate_periods`.

*Impact:* unsorted flows, or overlapping/unsorted caller periods, silently drop rows from the table (understating totals) where the aggregation path errors loudly — the two exports disagree.

*Fix:* call `validate_periods(periods)?` and sort a local flow index (or `debug_assert` sortedness) at the top of the DataFrame builder.

**7 — Ex-coupon window can span (or precede) the whole period → full negative coupon.**
`finstack-quant/cashflows/src/accrual.rs:606-610`.

`days_before_coupon` is validated only against 366, never against the actual period length. For a short coupon (e.g. a 1-month stub) with `days_before_coupon ≥ period length`, `ex_date < inputs.start`, so `as_of >= ex_date` holds for the entire period; at `as_of == start`, AI = `−coupon_total` (a full negative coupon).

*Impact:* dirty price understated by up to a full coupon for a mis-configured/short-period ex rule.

*Fix:* clamp/validate the ex-date to the active period: `let ex_date = ex.ex_date(inputs.end)?.max(inputs.start);` or reject `ex_date < start`.

**8 — Amortization round-trips through f64, reintroducing the drift the Decimal accumulator was added to prevent.** *(reported)*
`finstack-quant/cashflows/src/builder/pipeline.rs:100-113`.

`BuildState.outstanding` is `Decimal` specifically to avoid f64 drift on long-dated amortizers, but `emit_amortization` converts to f64, mutates in f64, and converts the delta back. Coupons and fees correctly stay in Decimal; only amortization round-trips, so the documented >1bp drift over 600+ periods can re-enter and break exact balance conservation in Decimal mode.

*Fix:* port `emit_amortization_on` to Decimal (carry the step/custom/linear targets as Decimal).

**9 — `build_periods` rates entry point bypasses the duplicate-payment-date guard.** *(reported)*
`finstack-quant/cashflows/src/builder/periods.rs:147-230`.

The dup-payment-date check (the prior Moderate fix) lives in `index_period_schedule` on the cashflow-compiler path; `build_periods`/`build_single_period` (used by valuations rates instruments) never call it, so two periods adjusting to the same payment date emit without error.

*Fix:* run the same collision check in `enrich_period_schedule` so both entry points behave identically.

### Minor

- **No schema_version / rounding-context stamp** in the wire format — still open from M13 (`finstack-quant/cashflows/src/json.rs:153`).
- **`CashFlow` (core) and `TenorDe` (core) lack `deny_unknown_fields`** — transitively inbound via the bridge (`finstack-quant/core/src/cashflow/primitives.rs:317`, `finstack-quant/core/src/dates/tenor.rs:162`); required fields still error, so only additive junk is tolerated.
- **Non-finite builder `accrual_factor` (`+inf`) → silent zero AI** — the `> 0.0` check passes `+inf`; AI then computes to 0 instead of erroring (`accrual.rs:398`, `:517`). Add `is_finite()`.
- **Issue-dated `StepRemaining`/`CustomPrincipal` amortization silently dropped** — the loop filters `d > issue` but init doesn't process issue-dated amortization (`orchestrator.rs:509`); outstanding overstated for life.
- **Overnight forward-tenor sampling basis** taken from the curve day-count, not `overnight_basis` (`coupons.rs:432`) — sub-0.1bp inconsistency.
- **DataFrame initial-funding detection has no once-only guard** (`dataframe.rs:509`) — two funding-sized draws on the anchor date are both skipped.
- **`nth_tenor` integer multiply unguarded against i32 overflow + no anchor-count cap** (`finstack-quant/core/src/dates/schedule_gen.rs:226`) — robustness only; parsed/deserialized tenors are capped.
- **Pre-first-fixing non-business days weighted to the following fixing** rather than the ISDA preceding fixing (`coupons.rs:538`) — documented deviation; bounded to a window opening on a non-business day.
- **Python `__all__` registration order not alphabetized** (`finstack-quant-py/src/bindings/cashflows/mod.rs:172`) — disagrees with the stubs; project standard says sorted.

## Confirmed remediated (re-verified against committed source)

- **B1** — schedule anchors now computed as a single `add_months(±k·n)` from an unclamped seed (`schedule_gen.rs:215-235`); no roll-day drift through short months.
- **B2** — `LongFront` now merges the residual stub into the first regular period (`schedule_gen.rs:306-335`).
- **M6** — EOM snaps only interior anchors; effective/termination dates emitted verbatim.
- **M7** — `ScheduleParams.adjust_accrual_dates` plumbed; swap presets true, bond presets false.
- **M9** — redemption dated on BDC-adjusted maturity via the principal leg's calendar.
- **M10** — zero-count `Tenor` rejected at deserialization, parse, and generation entry.
- **M2** — date loop filters `d > issue`; pre-issue events emitted once at init; init/loop emission mutually exclusive.
- **M14** — `PointInTime` fees read the period-start balance.
- **M11/M12** — half-open bucketing everywhere; pre-base PV zeroed in all paths; balance replay hoisted; funding detection via `meta.issue_date`.
- **M4** — lockout uses ISDA 2021 `daily_rates[n − lockout]`.
- **M5** — empty overnight fixing windows error.
- NaN/inf default/prepayment guards, `add_principal_event` sign/kind validation, multi-event atomicity, negative-balance warn, and `FeeTier::from_bps` finiteness checks all present.

## Open Questions or Assumptions

1. **Severity of #1 (ICMA stub accrued):** rated Major because accrued is wrong on the dominant new-issue case, but the price error self-corrects at payment and accrued is a small fraction of price. If repo/clean-dirty/settlement workflows lean heavily on accrued, it is closer to a Blocker.
2. **Daily-floor convention (#4):** is the period-floor behavior intentional for compounded legs, or should the ARRC/LSTA daily-floor be the default? It is currently silent either way.
3. **DataFrame credit PV (#5):** is the DataFrame meant to be a true credit-adjusted PV (match aggregation) or a survival-only diagnostic view? The docstring asserts the former; the math does the latter.
4. **`emit_default_on`/`emit_prepayment_on`** are public but not wired into the build orchestrator (only valuations/tests call them), so scheduled-amort vs default vs prepay ordering is the consumer's responsibility. Confirm that is the intended contract.

## Quant Notes

- The clean structural fix for #1 is to never compute an accrued *fraction* from one basis against a denominator from another. `AI = C · yf(start, as_of) / yf(start, end)` with one shared `DayCountContext` is correct for every convention (ICMA reference period and 30/360 curvature both cancel); the current `elapsed/total_yf` only cancels for regular periods.
- For #3, the governing references are the ARRC Term SOFR conventions and ISDA 2021 §7 — the decision rule is "has the rate for this period already fixed?", which is a statement about the reset/fixing date, never the accrual start.
- For #4, the NY Fed publishes SOFR averages/index values that make a clean golden fixture to pin daily-vs-period flooring; a single low-rate period with sub-floor fixings separates the two conventions by several bp.
