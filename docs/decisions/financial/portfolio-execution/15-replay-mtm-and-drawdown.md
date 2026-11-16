# PORTFOLIO-EXECUTION-015 — Snapshot replay, surviving-date P&L and MTM drawdown

- **Decision status:** Accepted 2026-11-16 — complete fixed-book replay, explicit modes, dated failures, strict attribution endpoints, interval MTM, drawdown and restore package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent skipped-date, MTM, drawdown precision, tie and availability examples verified. Runtime pricing, attribution, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio replay and drawdown

1. **Replay the fixed book on the supplied dates.** Require a nonempty,
   strictly ascending timeline of immutable dated markets. Preserve the
   supplied dates and use each snapshot's valuation cut. Claims of historical
   point-in-time replay require verified information cutoffs, fixing
   availability, FX coverage and holding eligibility. A later-selected book
   over earlier markets is a fixed-book backcast. Reject malformed input
   before valuation and require the selected model's qualified sources.
   [Publication-aware eligibility](../conventions/0045-financial-clocks-and-date-boundaries.md#publication-aware-fixing-eligibility-without-a-universal-cutoff).

2. **Keep output mode independent of visible risk.** Use `Valuations`, `Mtm`
   and `FullAttribution`. Require an explicit visible metric menu and risk
   policy. An empty menu requests PV. `FullAttribution` also requires an
   explicit attribution method. Remove implicit DV01 and attribution-driven
   widening. Every mode retains the observed-mark summary. `Mtm` adds
   interval and cumulative changes, and `FullAttribution` adds attribution.
   [Valuation profiles](03-valuation-profiles-and-failures.md#accepted-decision-for-checked-portfolio-valuation-profiles-and-failures).

3. **Make skipped dates and failure order explicit.** Strict replay reports
   the first failed initial valuation in timeline order. Explicit best effort
   skips whole failed initial snapshots and preserves exact requested,
   successful and skipped coverage with bounded typed diagnostics. All-failed
   errors carry that coverage directly. Finish initial snapshot selection
   before processing survivor intervals in order. Structural input,
   required endpoint, attribution and arithmetic failures abort either mode.

4. **Qualify attribution endpoints without changing the visible replay.**
   Reuse only a matching strict profile on the same immutable market and date,
   ordered holdings, model, settings and FX basis. Otherwise prepare the
   required endpoint separately through the shared executor. Use at most eight
   new endpoint dates per wave, including the opening endpoint. Carry shared
   endpoints forward. A single survivor requires no attribution endpoints.
   Preserve the attribution owner's coverage and invalid flags.
   [Selective reuse](05-dependencies-and-selective-reuse.md#accepted-decision-for-checked-portfolio-dependencies-and-selective-repricing)
   and [attribution endpoint owner](../portfolio-attribution/08-portfolio-pnl-endpoints.md).

5. **Measure MTM across actual surviving intervals.** Use `interval_mtm_pnl`
   for the checked Money difference from the previous survivor. Compute
   cumulative change directly from the first survivor. Publish the actual
   interval dates. The first step has no interval or cumulative P&L. The
   summary uses last less first, with zero for a single surviving mark.
   Cash-inclusive attribution remains a separate result on its owning basis.

6. **Select monetary and relative drawdowns independently.** Track the
   contemporaneous high-water mark from exact Money marks. Rank absolute
   declines and positive-peak ratios before any binary64 projection. Retain
   the earliest exact peak and decline ties. Publish the selected monetary
   decline in checked Money and the selected relative result as a decimal
   fraction. With no positive peak the relative result is absent. Defined
   zeros carry no event dates. Negative NAV after a positive peak can exceed
   a 100 percent decline. All results describe the observed marks.
   [Money precision](../core-conventions/012-money-decimal-precision.md).

7. **Use one bounded Rust contract for timelines and results.** Restore each
   market through its owner. Validate dates, coverage, holding order,
   currencies, mode-specific fields and derived arithmetic before publication.
   Preserve immutable source references and selected controls for audit and
   replay. Preserve exact Money, interval dates and missing values in the
   existing hosts. Use a Rust-owned JSON codec for large nested integers and
   explicit numeric chart projections.
   [Bounded restoration](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics).

## Independent calculation checks

- Marks of 105 and 100 around a skipped date give interval and total MTM
  of `−5`. An eligible coupon of 5 makes the separate cash-inclusive change
  zero.
- In `[100, 50, 1000, 900]` the monetary maximum is 100 on the last pair
  and the relative maximum is 50 percent on the first pair. In `[3, 2.7,
  10, 9]` the two exact 10 percent declines keep the earlier pair.
- Marks `9007199254740993` and `9007199254740992` have monetary drawdown 1
  even though both project to the same binary64 value. A move from 100 to
  `−20` has a relative drawdown of 120 percent. Marks `−10` and `−15` have
  monetary drawdown 5 and no relative result.

## Observed choice and scope at the audit revision

Replay holds portfolio positions fixed and values supplied market snapshots on their stated dates. It reports changes in marked value; those MTM differences exclude cash paid between dates. Full-attribution mode additionally delegates to a cash-inclusive attribution reducer. These quantities must remain distinct, especially across coupon payments or maturity.

## Timeline and valuation stages

A timeline must be nonempty and strictly ascending, rejecting duplicate or descending dates; it is not sorted or resampled for the caller. JSON snapshots pair each date with a market. Snapshot curve bases/fixings are interpreted by the underlying models rather than reconciled by timeline construction.

All snapshots are evaluated through one shared plan using requested valuation options. `PvOnly`, `PvAndPnl` and `FullAttribution` select output work; `ReplayMode::PvOnly` does **not** replace a standard metric menu with `Only([])`. Default valuation options still request strict DV01. For metrics-based full attribution, Standard/StandardPlus menus are augmented with the attribution defaults. Explicit `Only` menus remain unchanged in the visible snapshot valuation.

Strict replay returns the first snapshot failure in timeline order. Best effort records and drops failed **whole snapshots**, keeping subsequent successful dates. If all fail, the function errors without a result; its message suggests inspecting skipped dates even though no `ReplayResult` is returned in that case. This is a diagnostic inconsistency, not evidence of a hidden partial result.

## Attribution endpoint reuse

For full attribution, obtain the method-specific strict endpoint profile. Reuse a snapshot valuation with matching portfolio state identity/base currency and exact profile; for augmented metrics-based Standard/Plus results, a complete metric superset can also satisfy the endpoint. Otherwise evaluate strict endpoints in bounded batches of up to eight new dates; the initial batch can include the first date as an additional endpoint. Carry the previous endpoint between batches to avoid repeating overlaps.

Attribution connects the previous **surviving** market/date to the next surviving one and delegates to [portfolio P&L endpoint reduction](../portfolio-attribution/08-portfolio-pnl-endpoints.md). Best-effort policy only governs initial snapshot pricing. Endpoint retries, attribution errors and Money arithmetic failures remain fatal, including when the visible snapshot used best-effort metric fallback.

## MTM formulas and summary

For successful values `V_0,...,V_n`, the first step has no daily/cumulative P&L. Subsequent steps report `V_i - V_(i-1)` and `V_i - V_0` with checked Money subtraction. A gap of several days still uses the field `daily_mtm_pnl`; it is the interval between surviving observations, not a daily-normalized return.

The summary always computes end-minus-start MTM, even in PV-only mode. Scan a running high-water mark `H_i = max(V_0,...,V_i)`. Dollar drawdown is `max(H_i - V_i)`. Percentage drawdown is separately selected from `(H_i - V_i)/H_i` only when `H_i>0`; these two maxima can have different peak/trough dates. If no positive peak exists, percentage is zero with absent percentage dates. Strict `>` comparisons retain the first occurrence of tied new peaks or maximum declines. Dollar dates initialize to the first observation even when no drawdown occurs. Values are sampled marks, not an intraperiod path.

The live scan projects exact marks to binary64 before comparing or
subtracting them. That can lose both amount precision and the true order
of peak or trough candidates.

## Alternatives

A self-financing total-return replay with reinvested cash, changing holdings or external flows is not implemented here. Filling failed dates, resampling, or normalizing interval P&L would introduce new estimation assumptions. Existing MTM labeling and cash-inclusive attribution separation should be preserved.

## Evidence

[replay.rs](../../../../finstack-quant/portfolio/src/replay.rs) and [evaluation plan](../../../../finstack-quant/portfolio/src/evaluation/plan.rs). Inspected [replay tests](../../../../finstack-quant/portfolio/tests/replay.rs) showing January 1→3 after a failed January 2, exact visible `Only([])` with separate strict metrics, and propagated strict retry errors under best effort. Inline synthetic-value tests independently exercise contemporaneous peaks, different dollar/percentage maxima, nonpositive peaks and total-P&L overflow. Product attribution is separately qualified; no historical backtest was run.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
