# CALIBRATION-RATE-MODELS-004: Synthetic swaption Jamshidian decomposition and safeguarded exercise root

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

Synthetic equal-period ATM payer swaptions use an affine coupon-bond decomposition. The implemented rationale is to reduce one-factor exercise to a critical short rate and a sum of bond puts. This is the direct synthetic calibration path, not the engine's contractual-schedule integration.

## Input domain and cashflows

The caller supplies positive κ and σ, expiry, curve factors, forward par rate and synthetic frequency/tenor. The prepared kernel receives sorted fixed-bond `(payment_time,amount)` cashflows and a swap-start time at/after expiry. Production synthetic preparation sets swap start to expiry, pays `Fδ` each period, adds principal 1 at maturity, sorts by time and aggregates only bit-identical duplicate times. A test-only convenience wrapper appears before the production prepared kernel; the latter is included in this audit.

## Ordered algorithm

1. Compute the shared affine `B(T,t)` and `ln A(T,t)` coefficients for each bond and the swap-start bond.
2. Solve `g(r)=Σ c_i exp[ln A_i−ln A_s−(B_i−B_s)r]−1=0`. Start at the finite-difference instantaneous forward at expiry. Evaluate analytic g′ and its termwise absolute-magnitude sum.
3. Attempt at most 50 Newton steps. Fall back if g′ is nonfinite or `|g′|≤1e-10×max(Σ|term_i|,MIN_POSITIVE)`, or if `g/g′` is nonfinite or exceeds 5 in absolute rate units. A step below `1e-12` signals Newton convergence; there is no separate final g-residual gate.
4. If Newton did not converge, use shared Brent with tolerance `1e-12` and bracket bounds centered at the original forward with half-width `max(0.5,5σ√T)`. Failure to bracket/solve propagates; a nonfinite final root errors.
5. Set each bond-ratio strike `K_i=exp[ln A_i−ln A_s−(B_i−B_s)r*]`, and ratio volatility to the absolute difference between payment-bond and start-bond option volatilities. Require the start/payment discount factors positive/finite.
6. Sum `c_i[K_i D(start)Φ(−d₂)−D(payment_i)Φ(−d₁)]`. Below bond-volatility `1e-15`, use intrinsic instead. Individual negative put values are floored to zero while NaNs propagate. Reject nonfinite total, then floor total at zero.

## Conventions and controls

Values are per unit notional; κ is inverse years and σ absolute annual rate volatility. Newton's maximum step 5 means a 500-percentage-point rate change. Root tolerances, derivative guard and bracket heuristic are fixed numerical policies, not fit acceptance. Shared HW affine/forward derivatives are documented [separately](../models-analytic/hull-white-analytic-kernels.md); Brent internals belong to [core numerics](../core-numerics/brent-root.md).

## Limitations, failure and alternatives

Normal quotes can produce a negative par rate and hence negative intermediate fixed-bond coupons. The kernel does not impose nonnegative cashflows or verify a unique exercise region. The decomposition's applicability across those signed-coupon cases requires independent qualification; this inspection does not claim a reproduced misprice. Step-size-only root convergence and final price floors can hide some numerical residual behavior and need a measured envelope.

Contractual Gaussian integration is already implemented for explicitly supplied schedules and can retain signed payoffs without the zero-lag telescoping representation. It is selected by schedule presence, not as an automatic failure fallback. Trees or direct numerical payoff integration for all synthetic inputs would change speed and error controls.

## Implementation and evidence

[swaption.rs](../../../../finstack-quant/calibration/src/hull_white/swaption.rs) owns cashflows, root safeguards and decomposition. Inline `synthetic_prepared_cashflows_preserve_reference_price` fixes a numerical baseline without establishing external generator provenance. `explicit_zero_lag_schedule_reduces_to_synthetic_price` compares the two in-repo methods. [tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) contains positivity, σ-monotonicity and extreme-root/fallback cases; these are domain regressions, not a global uniqueness proof. Formulation references: [Jamshidian](../../../REFERENCES.md#jamshidian-1989-bond-option) and [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-ird). None was executed or externally requalified here.

Source commentary that HW construction removes the need for numerical arbitrage checks is broader than the implemented individual-price validation; it does not certify the computed surface or decomposition in every numerical regime.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
