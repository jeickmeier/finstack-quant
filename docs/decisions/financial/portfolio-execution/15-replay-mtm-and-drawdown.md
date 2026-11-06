# PORTFOLIO-EXECUTION-015 — Snapshot replay, surviving-date P&L and MTM drawdown

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

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

## Alternatives

A self-financing total-return replay with reinvested cash, changing holdings or external flows is not implemented here. Filling failed dates, resampling, or normalizing interval P&L would introduce new estimation assumptions. Existing MTM labeling and cash-inclusive attribution separation should be preserved.

## Evidence

[replay.rs](../../../../finstack-quant/portfolio/src/replay.rs) and [evaluation plan](../../../../finstack-quant/portfolio/src/evaluation/plan.rs). Inspected [replay tests](../../../../finstack-quant/portfolio/tests/replay.rs) showing January 1→3 after a failed January 2, exact visible `Only([])` with separate strict metrics, and propagated strict retry errors under best effort. Inline synthetic-value tests independently exercise contemporaneous peaks, different dollar/percentage maxima, nonpositive peaks and total-P&L overflow. Product attribution is separately qualified; no historical backtest was run.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
