# MODELS-ANALYTIC-07: Quanto drift and domestic discounting

**Decision status:** Accepted — complete checked fixed-conversion quanto pricing package with explicit FX orientation, 2026-11-10.

**Implementation status:** Observed. Reconciliation covers the scalar functions, existing host price call and dated quanto consumers below.

**Validation status:** Source, primary references and existing golden evidence inspected. Independent bounded calculations cover selected formulas and failure cases. Production and complete product qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for fixed-conversion quanto premiums

Accepted on **2026-11-10** after review of all 20 decisions.

1. **Select the fixed-conversion European law.** The underlying is a
   positive foreign asset spot, with a foreign-price strike in the same
   units. The contract pays fixed-conversion call or put intrinsic on the
   asset fixing. The scalar prices its lognormal terminal marginal in
   the payout-currency measure with constant asset and FX volatilities
   and correlation over the fixing horizon.

2. **Use one checked prepared request.** Select
   `quanto_option_price(params: &QuantoPriceParams) -> Result<f64>`.
   Require spot, strike, time, foreign_log_carry, payment_df, volatility,
   fx_volatility, correlation and option_type. Use the existing call/put
   enum. Retire the separate raw quanto_price and its duplicate admission
   path. No defaults or additional public risk operations are selected.

3. **Validate the complete domain.** Require finite positive S, K and
   payout discount D_d, finite signed total foreign log carry B_f, finite
   nonnegative T and annual relative volatilities sigma_S and sigma_X,
   and finite rho in [−1,1]. Allow D_d>1 and both correlation endpoints.
   Validate every field before a singular or zero-covariance branch.
   T is an explicitly compatible fixing and variance coordinate in model years.

4. **Fix the FX quote orientation.** Let C_f be asset currency and C_d
   payout currency. X is C_d per unit of C_f, with base C_f and quote C_d
   under the [accepted ordered FX convention](../core-conventions/011-fx-date-and-quote-conventions.md).
   Rho is correlation between the instantaneous asset log-return shock and
   the log-return shock of that X, on the same model clock.
   Prepared scalars assume this orientation. The dated owner qualifies it.

5. **Specify reciprocal orientation explicitly.** The exact reciprocal
   stochastic quote has the opposite log-return shock and the same relative
   volatility. Transform correlation by sign reversal once when a
   qualified reciprocal input is expressed on the selected orientation.
   This identity applies to the same modeled FX process. Qualification of
   a reciprocal smile, strike coordinate or observed source remains with
   its owner. The scalar performs no quote or contract conversion inference.

6. **Separate prepared foreign carry and payout discount.** B_f is the
   total log growth of the unadjusted foreign forward relative to S across
   the fixing interval. In the flat model it is (r_f−q)T, where q is the
   asset's continuous foreign dividend yield. For compatible deterministic
   curves, the owner supplies the integrated foreign financing less yield.
   D_d is the independently qualified deterministic factor to actual cash
   payment. Avoid a dated factor-to-rate-to-factor round trip. The
   [financial clock decision](../conventions/0045-financial-clocks-and-date-boundaries.md)
   governs the dated inputs.

7. **Apply the signed covariance correction once.** In the flat
   two-currency law, X has payout-measure drift r_d−r_f and the
   domestic value X S of the foreign asset has ex-yield drift r_d−q.
   The Itô cross term in X S is +rho sigma_S sigma_X. Thus asset carry
   in the payout measure is r_f−q−rho sigma_S sigma_X.
   The corresponding total correction is
   C=rho sigma_S sigma_X T, giving B_q=B_f−C and F_q=S exp(B_q).
   Add the signed adjustment −C once. Constant covariance over the common
   interval defines this record. Time-varying coefficients need their
   pointwise integrated covariance under a separately qualified model.

8. **Price the resulting terminal Black law.** Set w=sigma_S sqrt(T)
   and x=log(S/K)+B_q. For regular inputs use d1=x/w+w/2 and d2=x/w−w/2.
   The unconverted scalar is D_d times
   F_q Φ(d1)−K Φ(d2) for a call or K Φ(−d2)−F_q Φ(−d1) for a put.
   This describes the law. Use stable same-law positive and tail
   expressions for evaluation.

9. **Make the zero-time state coherent.** At T=0 require B_f=0 over
   the same fixing interval. The asset state is S and the scalar is
   D_d times spot intrinsic. A separately prepared payment discount can
   differ from one. Zero model time supplies no dated fixing or payment fact.
   A live clock collapse with nonzero carry needs an explicit owner treatment.

10. **Distinguish zero asset volatility from zero covariance.** When
    sigma_S=0, the foreign carry produces deterministic S exp(B_f)
    and the scalar is its discounted intrinsic. When sigma_X=0 or
    rho=0, the adjustment is exactly zero and the remaining stochastic
    asset law still uses B_f and D_d. Positive w lost in an intermediate
    calculation does not trigger a deterministic branch.

11. **Preserve scaled covariance and log moneyness.** Evaluate
    C=rho (sigma_S sqrt(T))(sigma_X sqrt(T)) or its equivalent scaled
    product without requiring the annual covariance, variances or
    separately represented factors to fit. Retain the signed carry
    difference and original S/K geometry. Avoid raw S/K, sigma_S²,
    d2=d1−w and premature rounding of an adjusted forward. Unresolved
    input geometry or arithmetic raises a computation error.

12. **Evaluate in the requested normalized discounted scale.** Reuse
    checked private log-forward, Gaussian-tail, positive-payoff and
    discount scaling primitives. Keep D_d in the full result even if F_q
    or the undiscounted premium would overflow or underflow. Return a
    finite nonnegative scalar. A known-positive requested result lost
    to zero or unresolved cancellation errors. Remove max-zero clipping
    and NaN sentinels.

13. **Keep contractual conversion and quantity with the product.**
    The scalar is denominated in foreign payoff units per underlying unit
    after payout-measure discounting. With contractual fixed kappa
    in C_d/C_f and quantity Q, the payout-currency product PV is
    Q kappa times that scalar. Use the contractual conversion once,
    independently of the live FX used by sources. The requested scalar
    itself must fit. A later quantity or conversion supplies no range
    extension to this API. The existing
    [dated product decision](../valuations-fx-and-exotics/quanto-drift.md)
    owns how the contractual multiplier is specified. The observed
    notional/strike fallback remains a product review item.

14. **Use the model's parity, bounds and direction checks.** In scalar
    units call minus put is D_d(F_q−K). Each premium is at least
    discounted forward intrinsic. Call is at most D_d F_q and put at most
    D_d K. The regular model gives strictly positive premiums. Higher
    oriented correlation lowers call and raises put with other inputs
    fixed when T and both volatilities are positive. At zero covariance
    ordinary spot BSM is recovered when carry and discount are also
    matched. Use these as independent checks.

15. **Separate fixed-input model checks from dated risk.** For regular
    inputs let H=D_d eta F_q Φ(eta d1), where eta is +1 for call and −1
    for put. H is the derivative with respect to log F_q. Correlation
    sensitivity is −sigma_S sigma_X T H and FX-volatility sensitivity
    is −rho sigma_S T H. Asset-volatility sensitivity also changes
    covariance, giving D_d F_q φ(d1) sqrt(T)−rho sigma_X T H. Its sign
    can vary. These are scalar checks in native units. The product
    owns quote bumps, clock, curve and smile rebuilding, position
    scale, correlation endpoint risk and report units.

16. **Reconcile dated market admission.** Validate asset-price currency
    and units before numeric extraction. Qualify domestic payment and
    foreign carry roles, common variance clocks, asset and FX quote
    conventions and oriented correlation. Require distinct ordered
    asset and payout currencies and actual pair authority for FX sources.
    Identifier spelling alone supplies no
    orientation evidence. Where the product selects FX ATM-forward vol,
    query the qualified forward in that same quote and strike coordinate.
    The owner retains spot dates, curve/funding basis, smile selection
    and interpolation or extrapolation policy.

17. **Reconcile actual fixing, payment and scale.** Resolve the required
    eligible asset fixing and outstanding cash with the dated owner.
    Current spot and date comparisons supply no automatic settlement
    fact. Resolve a known claim before unnecessary live market queries.
    Validate the contractual payoff scale and its arithmetic through
    payout-currency Money. Retain actual settlement and scope choices
    for the product review.

18. **Keep this terminal law separate from other claims.** Share checked
    covariance arithmetic with the existing equity-future and
    range-accrual owners where their law and orientation agree. They
    retain their own horizons, market inputs and payoff models. The
    adjusted one-factor asset marginal suffices for this fixed-conversion
    European payoff. Joint FX paths and stochastic conversion require
    their own contracts. This record selects the analytic price.

19. **Propagate the checked scalar contract across hosts.** Use the
    same explicit request and units in Rust, Python and WASM. Reconcile
    existing re-exports, stubs, declarations, schemas, parity and
    examples. Reject missing, duplicate and unknown request fields at
    the applicable boundary. Hosts map validation and computation
    errors without duplicating financial logic or conversion.

20. **Qualify the formula and actual consumers separately.** Check
    independent nonzero-correlation prices with unequal rates, both FX
    directions, exact zero cases, parity, scalar risk signs, scale and
    extreme covariance. Verify dated quote selection, fixing, settlement,
    contractual sizing, and host results separately. The existing
    QuantLib golden case covers one matched constant-coefficient payoff.
    Mixed clocks and smile behavior require their own evidence.

## Independent bounded reference checks

- With S=100, K=105, T=1.25, r_d=0.03, r_f=0.05, q=0.01,
  sigma_S=0.24, sigma_X=0.15 and rho=0.6, B_f=0.05, C=0.027 and
  F_q≈102.32665395472. Independent Decimal and payoff integration agree.
  A fixed conversion of 1.3 gives per-underlying-unit call/put values
  12.24326772793 / 15.59070531161. Quantity 7 gives about 85.70287410 /
  109.13493718 in payout currency. Using rho=−0.6 on the same quote
  gives call/put 16.19177438430 / 12.43004754815. This is the result of
  using the reciprocal quote's correlation without reorientation.
  Zero correlation retains the foreign carry and gives
  14.12028102929 / 13.96112034790 after conversion.
- The native QuantLib fixture has payout PV 81.84076455808031. A
  separate normal-CDF calculation using the fixture terms gives
  81.84076455808011. Its fixed conversion is 0.01 and quantity is
  1000. This checks the formula and literal conversion for the matched
  flat case.
- With sigma_S=sigma_X=2^520, T=2^-1040, rho=0.5, S=K=D_d=1 and
  B_f=0, both total standard deviations are one and C=0.5. The call is
  about 0.144610075925 even though the annual volatility product
  overflows binary64.
- With rho=2^-1000, sigma_S=2^-100, sigma_X=2^1000, T=1,
  S=K=2^100, B_f=0 and D_d=1, C=w=2^-100. A premature
  rho sigma_S multiplication rounds to zero. Independent tail scaling
  gives call/put about 0.083315470588 / 1.083315470588.
- With sigma_S=0, B_f=700, S=1e100, K=1 and D_d=exp(−700),
  the deterministic forward exceeds binary64 range and the discounted
  call is about 1e100.
- With S=K=100, T=D_d=1, B_f=0, sigma_S=sigma_X=2 and rho=0.8,
  the call sensitivity to asset volatility is about −0.4303689210
  per decimal unit. Independent centered differences agree with the
  derivative including the covariance term.

## Observed scope and input domain

Fixed-conversion European quanto calls/puts with asset/FX volatility, correlation and domestic/foreign continuous rates. Checked dispatch requires asset/FX vols≥0 and correlation in [−1,1].

## Observed choice and rationale

Apply the domestic-measure quanto drift correction −ρ σasset σFX to foreign asset carry and discount domestically. Source documents the drift sign; approval of FX quotation orientation and contractual mapping is not established here.

## Observed algorithm

Build adjusted forward Fq=S exp[(rf−q−ρσassetσFX)T]. Use Black call/put on Fq,K with asset σ and multiply exp(−rdT). At zero asset vol calculate discounted adjusted-forward intrinsic; at expiry spot intrinsic. Nonfinite results return NaN; finite prices floor at zero.

## Observed selection and controls

`quanto_option_price` validates and calls the raw `quanto_price`; the latter verifies only finiteness before branches. FX conversion multiplier is absent: the caller must apply any fixed contract conversion/notional.

## Observed conventions

Spot and strike are asset-price units; rd is payout-currency discount, rf is underlying-currency rate, σFX is relative annual FX volatility and ρ correlates asset return with the FX return orientation assumed by the correction.

## Observed limitations, failure and fallback

Constant rates, vols and correlation; no quanto smile, stochastic FX/asset covariance or stochastic discounting. Raw API accepts out-of-range correlation or negative volatility if finite. Contract orientation cannot be deduced from scalar arguments.

## Alternatives and tradeoffs

Joint asset/FX simulation can represent richer covariance and path features but is not this method. Zero correlation removes only the drift adjustment and retains the two-currency discount/carry split.

## Implementation and validation evidence

[quanto.rs](../../../../finstack-quant/models/src/closed_form/quanto.rs) and
[dispatch.rs](../../../../finstack-quant/models/src/closed_form/dispatch.rs)
show the raw and checked routes. The dated pricer currently calls the raw
route, derives annual rates from the two dated factors and applies the
contract multiplier afterward. It uses live FX for the FX-volatility lookup.
[Dated pricer](../../../../finstack-quant/valuations/fx/src/quanto_option/pricer.rs)

The existing [QuantLib golden fixture](../../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/quanto_option/nky_usd_quanto_call_1y_quantlib.json)
and [generator](../../../../scripts/golden/quantlib/fx_exotics.py)
use native QuantoEuropeanEngine with matched flat rates and constant
volatilities. The flat FX surface provides no independent strike-orientation
or smile evidence. Existing vanilla-limit, parity and correlation checks
are additional implementation evidence. See the
[quanto formula references](../../../REFERENCES.md#fixed-conversion-quanto-pricing).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
