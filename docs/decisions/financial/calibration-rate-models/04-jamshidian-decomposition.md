# CALIBRATION-RATE-MODELS-004: Synthetic swaption Jamshidian decomposition and safeguarded exercise root

**Decision status:** Accepted 2026-11-15 — complete synthetic Jamshidian exercise, pricing and acceptance package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent signed-coupon, exercise residual and small-positive-variance calculations verified. Production pricing, market references, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for checked synthetic Jamshidian decomposition

1. **Select the exact synthetic payer profile.** Use the regular
   unlagged shared-curve schedule with exercise equal to swap start
   and strike equal to prepared par `F`. At frequency `m`, construct
   preterminal coupon-bond weights `c_i = F/m` and terminal
   weight `c_n = 1 + F/m`. Dated contractual swaps use the separately
   selected Gaussian integration profile.
   [Accepted ATM preparation](03-atm-swaption-quotes.md#accepted-decision-for-exact-atm-swaption-targets-and-native-quote-reconstruction) ·
   [Jamshidian's bond option paper](https://doi.org/10.1111/j.1540-6261.1989.tb02413.x).

2. **Admit the regular negative-ATM coupon pattern.** The par
   identity gives strictly positive terminal weight
   `c_n = (D_e + Σ_{i<n} D_i)/Σ_i D_i`. For nonnegative par
   the coupon-bond sum decreases with the state. For negative
   par, every earlier coupon is negative and the final bond's
   loading is strictly greatest. Dividing the exercise residual
   by that bond gives a strictly decreasing expression with
   one crossing: `c_n − Σ_{i<n}|c_i|R_i/R_n − 1/R_n`.
   Every component put exercises on the same side of that
   crossing. Preserve the complete terminal weight and loading
   differences in checked arithmetic. Other signed cashflow
   patterns require separate qualification.
   [Henrard's one-factor exercise formulation](https://arxiv.org/abs/0901.1776).

3. **Solve on the standardized expiry-forward Gaussian state.**
   Let `q_i = B(e,t_i) sqrt(V(0,e))` be the total bond-option
   log standard deviation. With standardized state `z`, use
   `log R_i(z) = log(D_i/D_e) − q_i²/2 − q_i z`.
   Require an explicit finite bracket, positive state-coordinate
   tolerance and nonzero update budget. Use fallible Brent
   with checked sign-preserving arithmetic. Retire the
   instantaneous-forward seed, Newton path and heuristic
   fallback bracket.
   [Accepted affine bond law](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures) ·
   [Accepted Brent contract](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search).

4. **Gate the exercise residual at the returned state.** Form
   each strike `K_i = R_i(z_*)` from that state and require
   `C_* = Σ_i c_i K_i > 0`. Use an explicit positive absolute
   PV allowance per unit notional for `D_e |C_* − 1|`. The
   common-state put sum prices the basket at strike `C_*`,
   so this bounds the root's contribution to price error.
   Keep coordinate stopping, strike arithmetic, bond option
   arithmetic and this root-price allowance separate.

5. **Price each bond put on its original discounted legs.**
   Use `D_i` and `K_i D_e` with total log standard deviation
   `q_i`. Preserve root-derived strike geometry. Accumulate
   signed weighted premiums in checked scale, applying embedded
   discounting once. Reject unresolved cancellation or an
   invalid final price. Retire individual and total price clipping.
   [Accepted bond option pricing](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures).

6. **Preserve exact deterministic and Ho–Lee limits.** At genuine
   zero state variance price discounted intrinsic directly.
   Preserve exact zero mean reversion through the checked Ho–Lee
   kernel. At positive variance use the stochastic law at
   every supported scale. Retire the small-volatility intrinsic
   shortcut.

7. **Keep payoff and quote-fit acceptance separate.** Preserve
   the actual method, root evidence, work and typed failures.
   Pass the admitted premium to the original-convention quote
   reconstruction and apply its native gate before ranking
   calibration candidates.
   [Accepted ATM quote reconstruction](03-atm-swaption-quotes.md#accepted-decision-for-exact-atm-swaption-targets-and-native-quote-reconstruction).

## Independent calculation checks

- In the Ho–Lee limit with expiry one, annual payments at years
  two and three, sigma `.02` and discount factors `.99`, `1`
  and `1.01`, the par rate is `−2/201`. Signed puts and
  direct Gaussian payoff integration agree at about
  `.015876447385453` PV per unit notional.
- With one payment, expiry factor `.98` and ATM total log
  standard deviation `5e-16`, the premium is about `1.954817e-16`
  per unit notional. The observed intrinsic shortcut loses it.
- A one-bond exercise equation `exp(−x − .005) − 1` on
  the shifted rate coordinate has root `−.005`. Bracket
  `[−.00501,−.00490]` fits coordinate tolerance `.0002`,
  while both endpoint exercise residuals exceed `1e-8`.

## Observed choice and rationale at the audit revision

Synthetic equal-period ATM payer swaptions use an affine coupon-bond decomposition. The implemented rationale is to reduce one-factor exercise to a critical short rate and a sum of bond puts. This is the direct synthetic calibration path, not the engine's contractual-schedule integration.

## Observed input domain and cashflows

The caller supplies positive κ and σ, expiry, curve factors, forward par rate and synthetic frequency/tenor. The prepared kernel receives sorted fixed-bond `(payment_time,amount)` cashflows and a swap-start time at/after expiry. Production synthetic preparation sets swap start to expiry, pays `Fδ` each period, adds principal 1 at maturity, sorts by time and aggregates only bit-identical duplicate times. A test-only convenience wrapper appears before the production prepared kernel; the latter is included in this audit.

## Observed ordered algorithm

1. Compute the shared affine `B(T,t)` and `ln A(T,t)` coefficients for each bond and the swap-start bond.
2. Solve `g(r)=Σ c_i exp[ln A_i−ln A_s−(B_i−B_s)r]−1=0`. Start at the finite-difference instantaneous forward at expiry. Evaluate analytic g′ and its termwise absolute-magnitude sum.
3. Attempt at most 50 Newton steps. Fall back if g′ is nonfinite or `|g′|≤1e-10×max(Σ|term_i|,MIN_POSITIVE)`, or if `g/g′` is nonfinite or exceeds 5 in absolute rate units. A step below `1e-12` signals Newton convergence; there is no separate final g-residual gate.
4. If Newton did not converge, use shared Brent with tolerance `1e-12` and bracket bounds centered at the original forward with half-width `max(0.5,5σ√T)`. Failure to bracket/solve propagates; a nonfinite final root errors.
5. Set each bond-ratio strike `K_i=exp[ln A_i−ln A_s−(B_i−B_s)r*]`, and ratio volatility to the absolute difference between payment-bond and start-bond option volatilities. Require the start/payment discount factors positive/finite.
6. Sum `c_i[K_i D(start)Φ(−d₂)−D(payment_i)Φ(−d₁)]`. Below bond-volatility `1e-15`, use intrinsic instead. Individual negative put values are floored to zero while NaNs propagate. Reject nonfinite total, then floor total at zero.

## Observed conventions and controls

Values are per unit notional; κ is inverse years and σ absolute annual rate volatility. Newton's maximum step 5 means a 500-percentage-point rate change. Root tolerances, derivative guard and bracket heuristic are fixed numerical policies, not fit acceptance. Shared HW affine/forward derivatives are documented [separately](../models-analytic/hull-white-analytic-kernels.md); Brent internals belong to [core numerics](../core-numerics/brent-root.md).

## Observed limitations, failure and alternatives

Normal quotes can produce a negative par rate and hence negative intermediate fixed-bond coupons. The kernel does not impose nonnegative cashflows or verify a unique exercise region. The decomposition's applicability across those signed-coupon cases requires independent qualification; this inspection does not claim a reproduced misprice. Step-size-only root convergence and final price floors can hide some numerical residual behavior and need a measured envelope.

Contractual Gaussian integration is already implemented for explicitly supplied schedules and can retain signed payoffs without the zero-lag telescoping representation. It is selected by schedule presence, not as an automatic failure fallback. Trees or direct numerical payoff integration for all synthetic inputs would change speed and error controls.

## Implementation and evidence

[swaption.rs](../../../../finstack-quant/calibration/src/hull_white/swaption.rs) owns cashflows, root safeguards and decomposition. Inline `synthetic_prepared_cashflows_preserve_reference_price` fixes a numerical baseline without establishing external generator provenance. `explicit_zero_lag_schedule_reduces_to_synthetic_price` compares the two in-repo methods. [tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) contains positivity, σ-monotonicity and extreme-root/fallback cases; these are domain regressions, not a global uniqueness proof. Formulation references: [Jamshidian](../../../REFERENCES.md#jamshidian-1989-bond-option) and [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-ird). None was executed or externally requalified here.

Source commentary that HW construction removes the need for numerical arbitrage checks is broader than the implemented individual-price validation; it does not certify the computed surface or decomposition in every numerical regime.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
