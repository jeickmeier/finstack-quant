# CALIBRATION-RATE-MODELS-005: Contractual swaption payoff under the Gaussian exercise-date measure

**Decision status:** Accepted 2026-11-15 — complete contractual swaption payoff, Gaussian integration and pricing acceptance package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent delayed-payment, tilted-tail and missed-exercise calculations verified. Production pricing, market references, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for checked contractual Gaussian swaption integration

1. **Use the prepared dated physical payer contract.** Bind the expressly
   shared curve, exact exercise cut, actual schedules and computed ATM
   strike. Price authoritatively known coupons as literal signed cash on
   their actual payments. For unresolved term coupons require
   `e ≤ f ≤ Sx < Ex` and `P ≥ Sx`, with the admitted event phases
   and eligible early or delayed payment. Reject an unresolved reset
   before exercise. Keep contractual Gaussian integration for this
   profile, including zero-lag schedules.
   [Accepted ATM preparation](03-atm-swaption-quotes.md#accepted-decision-for-exact-atm-swaption-targets-and-native-quote-reconstruction) ·
   [Accepted dated payoff](../valuations-rates/08-hull-white-swaptions.md#accepted-decision-for-checked-hull-white-swaption-exercise-and-risk).

2. **Select the overnight coupon law explicitly.** Exact telescoping
   requires fully future contiguous reciprocal-bond resets, fixing at
   interval start, exact benchmark accrual weights and payment at the
   last interval end. Use the resulting difference of the start and end
   bonds and pay the simple spread outside compounding. The continuous
   approximation requires an explicit election and a qualified
   unseasoned, unshifted interval. Use its own payment adjustment and
   actual payment date. Partially realized or transformed overnight
   schedules require a separately qualified payoff law.
   [Accepted overnight laws](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures).

3. **Build the signed cash terms under the exercise-date measure.**
   For a term coupon let `A` be contractual coupon accrual and `αx`
   benchmark accrual. The positive gross term's expected discounted
   magnitude is `(A/αx) D(v,P) G_P`, with the whole-life
   payment-adjusted gross projection `G_P` from the accepted
   term law. Its loading is `q_Ex − q_Sx − q_P`, with
   `q_t = B(e,t) sqrt(V(v,e))`. Use the valuation-to-fixing
   covariance once. The subtraction `−(A/αx) D(v,P)` and
   signed spread use loading `−q_P`. Keep the fixed leg and
   known coupons on their actual payment loadings.
   [Accepted payment-measure kernels](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures) ·
   [Henrard's exercise-date formulation](https://arxiv.org/abs/0901.1776).

4. **Take the positive part after combining all cash terms.** Integrate
   `[Σ_j s_j M_j φ(z−b_j)]⁺`, where `M_j` is expected
   discounted magnitude and `b_j` the Gaussian loading. This
   incorporates density and discounting once. Use checked signed
   arithmetic and reject unresolved cancellation. At genuine zero
   state variance, evaluate the complete discounted deterministic
   payoff directly.

5. **Bound truncation and quadrature separately.** Require an explicit
   finite positive extent `L`, initial panel width, refinement depth,
   aggregate evaluation limit and separate absolute allowances per
   unit notional. Bound omitted payoff by
   `Σ_j M_j [Φ(−L−b_j) + Φ(b_j−L)]` with checked Gaussian
   tails. Integrate the admitted interval with fallible adaptive
   order-16 Gauss–Legendre. Treat coarse-versus-refined differences
   as operational estimates.
   [Accepted quadrature](../core-numerics/quadrature.md#accepted-decision-checked-quadrature-budgets-and-measures).

6. **Check each panel with entirely zero sampled payoff.** Use a
   conservative whole-panel upper bound. Accept a zero panel if
   the bound proves zero or the possible missed contribution fits
   the interior allowance. Otherwise refine within budget or fail.
   Keep missed-payoff bounds separate from the estimator in the
   evidence and charge them to the interior allowance. An unresolved
   zero total fails precision before quote inversion.

7. **Report pricing evidence before native fit acceptance.** Preserve
   the selected coupon and approximation laws, tail and missed-payoff
   bounds, quadrature estimate, actual work and typed failures. Pass
   the admitted premium to the original-convention inverse and
   apply its native quote gate before ranking candidates.
   [Accepted ATM quote reconstruction](03-atm-swaption-quotes.md#accepted-decision-for-exact-atm-swaption-targets-and-native-quote-reconstruction).

## Independent calculation checks

- In the Ho–Lee limit with sigma `.1`, exercise `1`, fixing
  `1.5`, benchmark start and end `1.75` and `2.25`,
  and payment `2.5`, take benchmark and payment discount
  factors `.98`, `.97` and `.965`, coupon accrual `.2`,
  benchmark accrual `.5` and spread `.001`. With matching
  fixed accrual and payment at the prepared ATM rate, direct
  Gaussian integration and the two-term analytical price give
  `.0074110667502` per unit notional. Omitting payment
  covariance gives `.0077781529544`.
- A unit-mass Gaussian term tilted to loading `12` loses
  approximately half its mass on `[−12,12]`. Weighted terms
  determine the tail allowance.
- The signed exponential stress payoff
  `10^6 exp(−2z)[cosh(.0001)−cosh(z−.5)]` is positive only
  on `(.4999,.5001)`. All order-16 nodes on `[0,1]`
  and its two halves miss that interval. Independent integration
  on the positive support gives about `8.6345064e-8` per
  unit notional.

## Observed choice and rationale at the audit revision

Contractual HW swaption calibration integrates the positive value of the full floating-minus-fixed swap under the exercise-date forward measure. Source rationale is to preserve floating fixing/payment lags and unshifted overnight compounding instead of telescoping the floating leg into two bonds. The default plan engine supplies contractual schedules and therefore selects this method; it does not call synthetic Jamshidian solely because dates have zero lag.

## Observed input domain

One-factor constant κ/σ, positive expiry, a discount-factor function, ATM strike and the validated [contractual schedule](03-atm-swaption-quotes.md). Single-curve simple term or unshifted compounded overnight coupons are supported; observation lookbacks/shifts/cutoffs are absent. Factor reads must be finite and positive. This private kernel relies on caller validation for schedule consistency. It allows a zero state variance deterministic branch for its mathematical kernel/test domain, even though the two-parameter calibration box excludes zero σ.

## Observed ordered algorithm and equations

1. Set state variance `V=σ² B(2κ;0,T)` and standard deviation √V; reject nonfinite/negative V.
2. Express time-0-weighted cashflows as signed exponentials `s_j exp(a_j+b_j z)` of one standard normal state. For a floating period define `B_s=B(κ;T,start)`, `B_e`, and `B_p` similarly. Its positive term has `a=ln D(start)+ln D(payment)−ln D(end)−V(B_s²+B_p²−B_e²)/2+ln adjustment` and `b=−(B_s+B_p−B_e)√V`; its negative term has `a=ln D(payment)−VB_p²/2`, `b=−B_p√V`.
3. Obtain the conditional delayed-coupon adjustment from the shared HW kernel using fixing/start/end/payment times and the compounded flag. Fixed-leg terms have sign `−sign(K)`, `a=ln(|K|αD(payment))−VB_p²/2` and `b=−B_p√V`; omit them at K=0. Reject nonfinite coefficients.
4. If V=0, return the positive part of the signed deterministic sum. Otherwise integrate `max(Σ s_j exp(a_j+b_jz),0)φ(z)`. Discounting is already embedded in coefficients; there is no additional external expiry discount multiplier.
5. Set integration bound `L=max|b_j|+12`. Bound omitted absolute tails by `Σ exp(a_j+b_j²/2)[Φ(−L−b_j)+Φ(b_j−L)]`. Reject nonfinite tail bound, bound above `1e-13`, or L above 1024.
6. Divide `[-L,L]` into `ceil(2L)` panels of width no greater than one. Integrate each with adaptive order-16 Gauss–Legendre, per-panel absolute budget `0.9e-12/panels`, depth 20. Nonfinite integrands or integration errors propagate; sum the panel values.

## Observed controls, limitations and alternatives

The nominal absolute per-unit-notional price budget is `1e-12`, split 10% to an analytic absolute-tail bound and 90% to quadrature estimates. It is not scaled to instrument notional or exposed as a calibration option. The tail bound encloses exponentially tilted centers; finite-state truncation alone is not the acceptance test. Unit-width panels reduce the risk of missing a narrow exercise region but do not by themselves prove every kink is found by the adaptive estimator. Cancellation among large signed terms remains a conditioning concern.

Synthetic Jamshidian is a cheaper implemented alternative with different schedule assumptions. Numerical roots followed by analytic exponential-region integrals, or adaptive tolerance tied to quote vega, are unimplemented alternatives requiring their own error and complexity decisions.

## Implementation and evidence

[contractual_swaption.rs](../../../../finstack-quant/calibration/src/hull_white/contractual_swaption.rs), called through [targets.rs](../../../../finstack-quant/calibration/src/hull_white/targets.rs). The underlying delayed-coupon covariance formula is documented in [HW analytic kernels](../models-analytic/hull-white-analytic-kernels.md); adaptive integration is in [core quadrature](../core-numerics/quadrature.md).

Inline `one_coupon_swaption_matches_independent_log_ratio_option` compares against an analytical log-ratio option whose whole-life covariance is assembled from 30,000 midpoint Brownian-loading steps. It independently constructs that covariance but shares B/CDF helpers; it is a special-case reference, not a vendor benchmark or full multi-coupon proof. The zero-variance test checks deterministic delayed-payment cashflows; a timing test in [swaption.rs](../../../../finstack-quant/calibration/src/hull_white/swaption.rs) compares zero-lag integration to Jamshidian. Evidence was read, not run. Background: [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-ird).

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
