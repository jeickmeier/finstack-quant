# CMS spread conditional lognormal integration

- ID: VALUATIONS-RATES-17-CMS-SPREAD
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

The CMS spread pricer is registered as `StaticReplication`, but its actual algorithm is one-dimensional Gaussian integration of a conditional two-lognormal payoff. It does not replicate an arbitrary pair of smiles. It accepts unshifted Black **flat-strike** marginal surfaces and an explicit Gaussian correlation in `[-1,1]`; SABR and nonflat smiles are rejected.

## Algorithm and controls

1. Resolve both CMS reference swap forwards, historical fixings and conventions as in [CMS conventions](11-cms-convexity.md). If payment is settled, return zero. Past expiry uses observed long/short CMS fixings and deterministic spread intrinsic; expiry exactly today still resolves forward/vol inputs before intrinsic.
2. For each future leg, obtain ATM Black vol, require positive finite forward/vol, and add the same annuity-weight convexity correction used by the approximate CMS pricer. Validate adjusted forward positive. Flatness checks strike-node vols at the requested expiry to `1e-12`; tenor-axis surfaces are naturally strike-flat. Positive vols below `1e-8` are floored to `1e-8`.
3. Treat adjusted rates as lognormal means. With standard normal z for the long rate, `L=mean_L*exp(s_L*z-s_L^2/2)`. The short conditional mean is `mean_S*exp(s_S*rho*z-(s_S*rho)^2/2)`, and conditional log standard deviation is `s_S*sqrt(1-rho^2)`.
4. Conditional on L, a spread call is a Black put on short rate with strike `L-K`; the spread put is a Black call. When `L-K<=0`, use zero for the spread call and `mean_short-(L-K)` for the put.
5. Integrate this conditional payoff with a fixed 20-point Gauss–Hermite rule and multiply by notional and payment discount. No separate coupon-accrual multiplier is applied. Nonfinite/negative expected payoffs error.

## Selection, risk and limitations

The default model and only local pricing registration are StaticReplication; quadrature order is a private default with no public numerical override. Registered curve risks are combined and triangular bucketed DV01; the result also reports both forwards, both convexity adjustments, adjusted spread and correlation. The implied joint distribution uses the supplied Gaussian correlation without calibration. Marginal means are approximated payment-measure means, and covariance/smile-consistent payment-measure mapping is absent. Correlation endpoints reduce conditional volatility to zero.

## Alternatives and tradeoffs

Kirk, normal-spread pricing, Monte Carlo with joint dynamics, and smile-mapped copula integration are unimplemented alternatives. Conditional Black integration removes the inner payoff kink and lowers integration dimension, but this dimensional reduction does not establish fixed-order accuracy near correlation endpoints or deep strikes.

## Evidence

- [Spread pricer and inline tests](../../../../finstack-quant/valuations/rates/src/cms_spread_option/pricer.rs), [validation and selection](../../../../finstack-quant/valuations/rates/src/cms_spread_option/types.rs), [risk registration](../../../../finstack-quant/valuations/rates/src/cms_spread_option/metrics.rs): observed implementation and local behavioral evidence.
- [Black](../../../REFERENCES.md#black-1976), [Hagan CMS](../../../REFERENCES.md#hagan-2003-cms-convexity), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo): relevant constituent methods. A canonical reference and independent validation for this precise combination were not established.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
