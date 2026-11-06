# Normalized smile replication for CMS options and swaps

- ID: VALUATIONS-RATES-12-CMS-REPLICATION
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

`StaticReplication` prices CMS options using normalized annuity weighting of a Black swaption smile. CMS swaps derive the expected rate and embedded cap/floor options from the same kernel. Positive reference forward and a strike-axis Black-lognormal surface are required; no Normal or shifted-lognormal conversion is inferred. This is a supported alternative to the default [convexity approximation](11-cms-convexity.md), with an explicitly finite numerical tail.

## Algorithm and controls

Let `w` be the payment-date annuity weight and let `O(K)` be the out-of-money Black put below F, call at/above F, using clamped surface volatility at K. Normalize derivatives by `w(F)`. For signed option payoff `g(r)=max(sign*(r-K),0)`, compute a numerator containing intrinsic, the strike kink `w(K)/w(F)*O(K)` when K is positive, and the integral of the weighted-payoff second derivative times O. On the in-the-money half-line the smooth second derivative is `sign*(2*w'(r)+(r-K)*w''(r))`. The denominator is `1+integral(w''(r)/w(F)*O(r)dr)`. Return `D_pay*numerator/denominator`.

Integrate in log strike `x=ln(r/F)` with Jacobian r. Bounds are `±[12*sigma_max*sqrt(T)+(2+abs(delay))*sigma_max^2*T]`, where sigma_max scans the whole surface. Split at zero, every surface strike in range and the payoff strike. Each interval uses adaptive 16-node Gauss–Legendre, maximum depth 16, and total tolerance `1e-11` divided across intervals. Width above 500/nonfinite errors. Nonpositive time or zero maximum surface vol returns intrinsic. Require finite positive normalization and finite value no lower than `-1e-10`; clamp smaller negative roundoff to zero.

CMS option sums these optionlets with N and contractual accruals. CMS swap recovers `E[CMS]=F+(call(F)-put(F))/D_pay`. Because each replicated optionlet is already discounted, coupon rate is `E[CMS]+spread-call(cap-spread)/D_pay+put(floor-spread)/D_pay` for the configured cap/floor. Multiply the resulting rate by `N*alpha*D_pay` and net funding. Past fixings still use observed coupons; future vol smile is not used to revalue known observations.

## Selection, conventions and failure behavior

Registry explicitly selects StaticReplication; it is not an automatic repair when the approximation fails. Surface extrapolation clamps to boundary vol, so remote tail behavior is flat-vol extrapolation within the integration range. Payment delay is signed ACT/365F from reference swap start. Call/put strike may fall outside the positive integration domain; the kink contribution then is omitted consistently with the supported positive-rate underlying.

## Alternatives and tradeoffs

A larger adaptive tail with error estimation, arbitrage-free surface extrapolation, or a stochastic term-structure model could provide different error controls; none is locally selected. Normal CMS replication would require a different domain/weight treatment. The normalized expectation controls the numeraire weight's mass but does not by itself prove the input smile is arbitrage-free.

## Evidence

- [Replication kernel](../../../../finstack-quant/valuations/rates/src/cms_option/replication_pricer.rs), [weight derivatives](../../../../finstack-quant/valuations/rates/src/cms_common.rs), [swap replication](../../../../finstack-quant/valuations/rates/src/cms_swap/pricer.rs).
- [Replication sign tests](../../../../finstack-quant/valuations/rates/tests/cms_option/replication_sign.rs), [pricing and seasoned-coupon tests](../../../../finstack-quant/valuations/rates/tests/cms_option/test_pricing.rs), and inline swap cap/floor/parity tests: numerical and invariant evidence, not independent market-wide qualification.
- [Hagan CMS](../../../REFERENCES.md#hagan-2003-cms-convexity), [Black](../../../REFERENCES.md#black-1976), [numerical integration reference](../../../REFERENCES.md#press-numerical-recipes).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
