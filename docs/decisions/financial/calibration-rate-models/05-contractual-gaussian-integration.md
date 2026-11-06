# CALIBRATION-RATE-MODELS-005: Contractual swaption payoff under the Gaussian exercise-date measure

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

Contractual HW swaption calibration integrates the positive value of the full floating-minus-fixed swap under the exercise-date forward measure. Source rationale is to preserve floating fixing/payment lags and unshifted overnight compounding instead of telescoping the floating leg into two bonds. The default plan engine supplies contractual schedules and therefore selects this method; it does not call synthetic Jamshidian solely because dates have zero lag.

## Input domain

One-factor constant κ/σ, positive expiry, a discount-factor function, ATM strike and the validated [contractual schedule](03-atm-swaption-quotes.md). Single-curve simple term or unshifted compounded overnight coupons are supported; observation lookbacks/shifts/cutoffs are absent. Factor reads must be finite and positive. This private kernel relies on caller validation for schedule consistency. It allows a zero state variance deterministic branch for its mathematical kernel/test domain, even though the two-parameter calibration box excludes zero σ.

## Ordered algorithm and equations

1. Set state variance `V=σ² B(2κ;0,T)` and standard deviation √V; reject nonfinite/negative V.
2. Express time-0-weighted cashflows as signed exponentials `s_j exp(a_j+b_j z)` of one standard normal state. For a floating period define `B_s=B(κ;T,start)`, `B_e`, and `B_p` similarly. Its positive term has `a=ln D(start)+ln D(payment)−ln D(end)−V(B_s²+B_p²−B_e²)/2+ln adjustment` and `b=−(B_s+B_p−B_e)√V`; its negative term has `a=ln D(payment)−VB_p²/2`, `b=−B_p√V`.
3. Obtain the conditional delayed-coupon adjustment from the shared HW kernel using fixing/start/end/payment times and the compounded flag. Fixed-leg terms have sign `−sign(K)`, `a=ln(|K|αD(payment))−VB_p²/2` and `b=−B_p√V`; omit them at K=0. Reject nonfinite coefficients.
4. If V=0, return the positive part of the signed deterministic sum. Otherwise integrate `max(Σ s_j exp(a_j+b_jz),0)φ(z)`. Discounting is already embedded in coefficients; there is no additional external expiry discount multiplier.
5. Set integration bound `L=max|b_j|+12`. Bound omitted absolute tails by `Σ exp(a_j+b_j²/2)[Φ(−L−b_j)+Φ(b_j−L)]`. Reject nonfinite tail bound, bound above `1e-13`, or L above 1024.
6. Divide `[-L,L]` into `ceil(2L)` panels of width no greater than one. Integrate each with adaptive order-16 Gauss–Legendre, per-panel absolute budget `0.9e-12/panels`, depth 20. Nonfinite integrands or integration errors propagate; sum the panel values.

## Controls, limitations and alternatives

The nominal absolute per-unit-notional price budget is `1e-12`, split 10% to an analytic absolute-tail bound and 90% to quadrature estimates. It is not scaled to instrument notional or exposed as a calibration option. The tail bound encloses exponentially tilted centers; finite-state truncation alone is not the acceptance test. Unit-width panels reduce the risk of missing a narrow exercise region but do not by themselves prove every kink is found by the adaptive estimator. Cancellation among large signed terms remains a conditioning concern.

Synthetic Jamshidian is a cheaper implemented alternative with different schedule assumptions. Numerical roots followed by analytic exponential-region integrals, or adaptive tolerance tied to quote vega, are unimplemented alternatives requiring their own error and complexity decisions.

## Implementation and evidence

[contractual_swaption.rs](../../../../finstack-quant/calibration/src/hull_white/contractual_swaption.rs), called through [targets.rs](../../../../finstack-quant/calibration/src/hull_white/targets.rs). The underlying delayed-coupon covariance formula is documented in [HW analytic kernels](../models-analytic/hull-white-analytic-kernels.md); adaptive integration is in [core quadrature](../core-numerics/quadrature.md).

Inline `one_coupon_swaption_matches_independent_log_ratio_option` compares against an analytical log-ratio option whose whole-life covariance is assembled from 30,000 midpoint Brownian-loading steps. It independently constructs that covariance but shares B/CDF helpers; it is a special-case reference, not a vendor benchmark or full multi-coupon proof. The zero-variance test checks deterministic delayed-payment cashflows; a timing test in [swaption.rs](../../../../finstack-quant/calibration/src/hull_white/swaption.rs) compares zero-lag integration to Jamshidian. Evidence was read, not run. Background: [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-ird).

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
