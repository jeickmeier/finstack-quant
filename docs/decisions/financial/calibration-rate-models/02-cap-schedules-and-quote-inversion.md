# CALIBRATION-RATE-MODELS-002: Scheduled cap/floor market prices, HW repricing and implied-normal inversion

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

A cap/floor quote is a flat Normal volatility applied to a strip of live term-index coupons. Market and HW model values use the same fixing, accrual and payment schedule. The source explicitly excludes the first spot-start coupon and reuses the production scheduled HW caplet kernel so fitting and valuation share contractual timing. Shared-kernel consistency is distinct from independent correctness.

## Input domain and schedule

All scheduled times/accruals must be finite. Each period has `0 < fixing ≤ start < end`, `payment ≥ start` and positive contractual accrual. Fixings increase strictly; accrual periods may have gaps but cannot overlap beyond `1e-10`. The final accrual end must match quote maturity within `1e-10`. Payment can precede the unadjusted accrual end after calendar adjustment. Empty live schedules are rejected. This representation excludes compounded overnight options.

Without a contractual schedule, set `n=max(round(maturity×payments_per_year),1)` and `δ=maturity/n`; retain periods `i=1,…,n−1`, with fixing/start `iδ`, end/payment `(i+1)δ` and accrual δ. The first period is excluded, and `n=1` therefore fails the no-live-caplet gate. This divides the whole maturity evenly; it does not construct a contractual final stub. Supplied schedule count must equal quote count.

## Ordered algorithm

1. Derive each projected simple forward from the projection factors: `F_i=(P_f(start_i)/P_f(end_i)−1)/α_i`. Nonpositive/nonfinite projection factors yield NaN rather than a floor.
2. Price the market strip as `Σ α_i D(payment_i) Bachelier(F_i,K,σ_N,fixing_i)`, selecting call for cap and put for floor. The strip's quote vega is the corresponding sum of analytical Normal vegas at the actual strike and quoted volatility.
3. Reprice each identical period with `hw1f_term_caplet_price_from_dfs_with_model`, using discount/payment factors, both projection factors, fixing/start/end/payment times, accrual, strike and cap/floor side. Projection/discount basis is deterministic; payment-measure adjustment is retained by the [HW kernel](../models-analytic/hull-white-analytic-kernels.md).
4. For reporting, invert the resulting model strip value back to one flat Normal volatility. Reject negative/nonfinite target price. Start upper vol at `0.01` and double while its market price is below target and the current bound is below `100`; the final doubling can exceed 100. Solve `[0,upper]` with Brent tolerance `1e-14` and the shared default iteration budget. This reconstructs quote error instead of reporting a first-order vega approximation.

## Selection and controls

Scalar fixed-κ and two-parameter fits use this preparation and inversion. Both validate aggregate market vega against `1e-8` before branching; this restriction applies even though the fixed-κ objective is unweighted price SSE. The piecewise bootstrap uses the same prices/inversion but does not apply that vega gate. Required final `fit_tolerance` is in decimal Normal-volatility units and is independent of this inversion tolerance.

## Limitations, failure and fallback

This is a term-index cap method, not an overnight-cap calibration. The market schedule validator permits gaps, payment before unadjusted end and arbitrary positive accruals, so contractual correctness depends on the caller. The model kernel rejects invalid prices/domains; quote inversion propagates unavailable brackets or solver failures. The finite upper-bracket search is a numerical control, not a universal range guarantee. Tiny/near-intrinsic quote inversion remains conditioned by strip vega.

## Alternatives and tradeoffs

A simple forward-rate Normal-vol proxy would be cheaper but is not the selected HW price; exact scheduled bond-ratio dynamics retain accrual and delay effects. Joint smile fitting or shifted/lognormal cap quotation would require an explicit new objective/domain. No automatic quotation conversion occurs.

## Implementation and evidence

[cap_schedule.rs](../../../../finstack-quant/calibration/src/hull_white/cap_schedule.rs) and [pricing.rs](../../../../finstack-quant/calibration/src/hull_white/pricing.rs) own schedule validation, market pricing and inversion; [cap_floor.rs](../../../../finstack-quant/calibration/src/hull_white/cap_floor.rs) calls them. [cap_schedule_tests.rs](../../../../finstack-quant/calibration/src/hull_white/cap_schedule/tests.rs) exercises ACT/360 accrual, early fixing, delayed/early payment, stubs, invalid schedules and aligned schedule sorting. Its ACT/360 case includes fixed numerical references, but the inspected file does not establish their generator provenance. Repricing also reuses the production caplet kernel.

[tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) checks spot-caplet exclusion, one-period rejection and cap/floor parity. Parity is an invariant; synthetic recovery is a same-kernel round trip. All evidence was inspected, not run.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
