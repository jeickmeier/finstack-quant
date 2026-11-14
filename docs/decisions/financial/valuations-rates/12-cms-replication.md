# VALUATIONS-RATES-12-CMS-REPLICATION: Normalized smile replication for CMS options and swaps

- ID: VALUATIONS-RATES-12-CMS-REPLICATION
- Decision status: **Accepted — complete checked CMS smile replication and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, model and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked CMS smile replication and risk

1. **Select replication and prepare the actual CMS claims.** Explicitly
   select `StaticReplication` for CMS options or swaps. Use the exact
   benchmark, reference IRS par rate `F`, actual dated CMS and funding
   periods, payment roles and publication and entitlement cuts from
   [record 11](11-cms-convexity.md#accepted-decision-for-checked-cms-convexity-pricing-and-risk).
   A known fixing keeps its literal signed cash payoff and requires no
   live volatility, strike support or reference projection. Settled or
   otherwise deterministic claims need only their active inputs.
   A live projection requires an admitted elapsed interval to fixing
   strictly greater than zero, including a qualified intraday clock
   for an unpublished same-day observation. Date equality and zero
   quote volatility do not establish actual fixing authority. Require
   `F>0` on this profile. No live Normal, shifted Black or negative-rate
   alternative is implied; remove the misleading fallback guidance.
   Failure on either CMS pricing route cannot choose the other route.

2. **Bind one dated, fixed-expiry reference smile.** Require an exact
   typed unshifted Black swaption strike source or qualified materialized
   fixed-tenor strike slice, matching the CMS fixing, actual reference
   swap convention, currency, tenor, collateral basis and quote clock.
   Resolve the actual expiry strictly within admitted source support;
   evaluate at the source's original absolute strike nodes and use
   only its qualified expiry evaluation. A stored fixed expiry must
   match exactly. Preserve native annual quote sigma and any admitted
   same-interval variance bridge, quote origin and smile association.
   No implicit clock roll, clamp, cube-to-strike substitution, or
   evaluation at an unrelated cash or accrual date. A selected strike
   strip is an explicit quote set; use all nodes of that selected
   source strip and check each quote and ordered positive strike,
   with no silent removal of bad nodes.
   Convert each quote to a checked unit call price on the same
   unadjusted reference forward, strike and variance. Use one common
   reference annuity basis for the strip. If the reference and
   payment discount laws differ, require an explicitly qualified
   collateral and numeraire mapping. Normalizing the payment weight
   does not qualify an unrelated discount or swaption source.
   [Volatility grids](../core-market/volatility-grids-and-shocks.md),
   [volatility-source evaluation](../models-analytic/volatility-source-interpolation.md)
   and [Black price law](../models-analytic/forward-normal-shifted.md).

3. **Elect a finite-support call-price law with explicit wings.** For
   each admitted live strip require finite authored support endpoints
   `0<L<F<U`, with every quoted strike strictly in `(L,U)`. Retain
   the endpoints' identity, rate units and qualification. Add the
   call-price anchors `(L,F−L)` and `(U,0)`. Interpolate linearly in
   unit call price between consecutive anchors and quote nodes, with
   `C(K)=F−K` at or below `L` and `C(K)=0` at or above `U`.
   This is a CMS pricing law; it does not alter the generic volatility
   source's interpolation. Require original call bounds and ordered
   adjacent secants in `[−1,0]`, nondecreasing across the strip
   including the exterior slopes `−1,0`. Do not clip, sort away
   failures, refit nodes, or move anchors to pass. Use checked
   cross-products or equivalent scale-preserving comparisons on
   original coordinates, retaining call intrinsic and OTM time value
   separately where a rounded price would erase important geometry.
   This selects a complete finite-support law across strikes for one
   expiry. The support is an economic input, not an inferred bound on
   market rates or a claim about cross-expiry arbitrage.
   [Davis and Hobson on option-price consistency](https://warwick.ac.uk/fac/sci/statistics/staff/academic-research/hobson/publications/range.pdf)
   and [Henrard on CMS wing sensitivity](https://quant.opengamma.io/Smile-Extrapolation-OpenGamma.pdf).

4. **Recover and validate one annuity-measure distribution.** Put
   probability at each anchor and quoted strike equal to its
   right slope minus its left slope. Exact equality can give zero
   mass. Verify nonnegative masses, unit total and first moment
   `F` in checked arithmetic, preserving small consequential slope
   jumps and their weighted contributions. These identities follow
   from the admitted chords and anchors; a numerical check must
   bound rounding on the original admitted nodes. Propagate
   unresolved sign, weight or range errors rather than accepting a
   fixed absolute floor, dropping positive mass or renormalizing
   inconsistent inputs. If every admitted active nodal sigma is
   exactly zero, use the point mass at `F` after validating terms,
   quote scope and support. The chord across an unstored intrinsic
   kink would otherwise create spurious time value. Neither a
   rounded-away positive premium nor a zero quote on another expiry
   establishes this degeneracy. The atom distribution is exact for
   this selected piecewise linear price law.

5. **Normalize the dated payment weight over that distribution.**
   Reuse record 11's dated flat-yield proxy, with positive integer
   `m`, actual fixed accruals and ACT/365F payment times
   `α_j,u_j`, and signed ACT/365F delay `δ` from reference start
   to CMS payment. With `q(r)=1+r/m`, use
   `A_flat(r)=Σ_j α_j q(r)^(−m u_j)` and
   `w(r)=q(r)^(−mδ)/A_flat(r)`. The curve annuity that supplies
   `F` is separate from this payment proxy. Use positive scaled
   weights `h_i=w(r_i)/w(F)`, or an equivalent common internal
   scaling. Form `Z=Σ_i p_i h_i` and calculate every admitted
   payoff as `Σ_i p_i h_i g(r_i)/Z`. These are undiscounted
   payment-measure model amounts with one shared normalization.
   Require a resolved positive normalizer, and hold adequate scale
   through signed sums, cancellation and the final requested native
   cash result. An internal unscaled factor need not fit separately.
   Remove the log-strike width heuristic, implicit flat-vol wings,
   derivative-weight quadrature budgets and negative-result clamp.
   Preflight actual strike, payment and payoff work against the
   checked source and schedule limits and an explicit aggregate
   work limit before allocation;
   preserve ordered checked accumulation and reject invalid,
   unsupported or unresolved output. There is no omitted-tail
   integral under this model because the support is part of the law.
   That does not establish market wing accuracy or an exact
   term-structure payment measure.
   [Hagan's CMS measure change and replication](https://www.deriscope.com/docs/Hagan_Convexity_Conundrums.pdf).

6. **Use the same law for the coupon, option and embedded collar.**
   Compute the CMS mean `M` directly with `g(r)=r`. Price a
   call or put with its actual signed strike payoff, including
   strikes outside quoted nodes. Below `L` the payoff is linear
   for a call and zero for a put; above `U` the roles reverse.
   The same identities hold at the respective endpoints. Do not
   manufacture an off-grid volatility quote or actual fixing.
   Compute a CMS coupon by applying spread once and applying
   the contractual floor and cap to `r+s` at each state. Preserve
   `M+s−C_pay(cap−s)+P_pay(floor−s)` for explanation and
   reconciliation. Use the bounded coupon directly when separate
   subtractions could lose a material residual. Equal bounds give
   fixed cash. Check applicable option bounds, collar bounds and
   `C_pay(K)−P_pay(K)=M−K` on the common normalized basis;
   no clipping or unexplained residual plug. The same ATM
   identity can verify the mean without pricing two discounted
   optionlets and dividing their difference by the payment factor.
   Apply each period's notional, contractual accrual and actual
   dated payment factor once. Net independently prepared funding
   on the holder's signs and eligibility. Known cash follows
   record 11, including signed rates and entitled same-day claims.

7. **Reprice this law on each declared risk axis.** Apply record
   11's exact typed curve risk and selected-measure volatility
   stencils, including `dv01`, `pv01`, `bucketed_dv01` and
   where admitted native Black vega, volga and
   `cross_gamma_rates_vol`. Bind the active quote scope, source
   clock, requested steps, roles and held-input policy. Hold the
   authored support strikes fixed for direct rate and quote-vol
   experiments; rebuild the source-qualified forward, node call
   prices, masses and payment weights as needed at every point.
   Keep the same strike strip membership and exact market factor
   identities, with curve replay on a separately authorized basis.
   A failed strip or support check invalidates that endpoint,
   including failure at a discrete-law boundary. Do not change
   the model, shrink the step or repair the smile to obtain a
   Greek. Use the shared one-sided vega rule only at its declared
   native sigma domain boundary with a valid full up experiment.
   An option currently outside support can lose option exposure
   without losing all modeled rate/discount channels; prove zero
   on the complete requested axis. Keep
   `convexity_adjustment_risk` on the record 11 approximation.
   A comparison with constant payment weights, if later needed,
   is a separate unadopted measure. The model rate diagnostic
   `M−F` can use a centered numerator to preserve cancellation.
   [Shared option risk](../valuations-framework/option-sensitivities.md)
   and [mixed risk](../valuations-framework/cross-factor-risk.md).

8. **Keep the selected method through reports and public routes.**
   Reuse the matching prepared rows for PV, underlying and
   payment-mean diagnostics, risk and admitted cashflow export.
   Report asserted support, dated source, evaluation, active roles,
   selected method, actual steps and status. Expected CMS or
   embedded option cash is model output; known CMS and funding
   amounts retain fixing and payment authority. If the selected
   result has no qualified complete decomposition, return
   unavailable. No default approximation export or cash placeholder.
   A horizon crossing a fixing needs the separate qualified
   observation and holder ledger. Reconcile Rust entry points and
   registry, existing Python/WASM envelopes, schemas, declarations,
   and parity. Keep internal distribution state validated and
   Rust-owned. Qualify quote/anchor admission, deep strike
   and small mass arithmetic, signed delay, uneven reference
   schedules, known and live cuts, payment parity, scenario-side
   failures, and host reports. Independently measure sensitivity
   to support and strike resolution against qualified models and
   market fixtures, including analytic full-support comparisons.
   Exact summation for this law does not certify the chosen
   support, collateral proxy or empirical accuracy.
   [Cashflow export](../valuations-framework/cashflow-export.md),
   [holder horizon](../valuations-framework/carry-theta-breakeven.md).

## Observed behavior

### Choice, scope and rationale

`StaticReplication` prices CMS options using normalized annuity weighting of a Black swaption smile. CMS swaps derive the expected rate and embedded cap/floor options from the same kernel. Positive reference forward and a strike-axis Black-lognormal surface are required; no Normal or shifted-lognormal conversion is inferred. This is a supported alternative to the default [convexity approximation](11-cms-convexity.md), with an explicitly finite numerical tail.

### Algorithm and controls

Let `w` be the payment-date annuity weight and let `O(K)` be the out-of-money Black put below F, call at/above F, using clamped surface volatility at K. Normalize derivatives by `w(F)`. For signed option payoff `g(r)=max(sign*(r-K),0)`, compute a numerator containing intrinsic, the strike kink `w(K)/w(F)*O(K)` when K is positive, and the integral of the weighted-payoff second derivative times O. On the in-the-money half-line the smooth second derivative is `sign*(2*w'(r)+(r-K)*w''(r))`. The denominator is `1+integral(w''(r)/w(F)*O(r)dr)`. Return `D_pay*numerator/denominator`.

Integrate in log strike `x=ln(r/F)` with Jacobian r. Bounds are `±[12*sigma_max*sqrt(T)+(2+abs(delay))*sigma_max^2*T]`, where sigma_max scans the whole surface. Split at zero, every surface strike in range and the payoff strike. Each interval uses adaptive 16-node Gauss–Legendre, maximum depth 16, and total tolerance `1e-11` divided across intervals. Width above 500/nonfinite errors. Nonpositive time or zero maximum surface vol returns intrinsic. Require finite positive normalization and finite value no lower than `-1e-10`; clamp smaller negative roundoff to zero.

CMS option sums these optionlets with N and contractual accruals. CMS swap recovers `E[CMS]=F+(call(F)-put(F))/D_pay`. Because each replicated optionlet is already discounted, coupon rate is `E[CMS]+spread-call(cap-spread)/D_pay+put(floor-spread)/D_pay` for the configured cap/floor. Multiply the resulting rate by `N*alpha*D_pay` and net funding. Past fixings still use observed coupons; future vol smile is not used to revalue known observations.

### Selection, conventions and failure behavior

Registry explicitly selects StaticReplication; it is not an automatic repair when the approximation fails. Surface extrapolation clamps to boundary vol, so remote tail behavior is flat-vol extrapolation within the integration range. Payment delay is signed ACT/365F from reference swap start. Call/put strike may fall outside the positive integration domain; the kink contribution then is omitted consistently with the supported positive-rate underlying.

### Alternatives and tradeoffs

A larger adaptive tail with error estimation, arbitrage-free surface extrapolation, or a stochastic term-structure model could provide different error controls; none is locally selected. Normal CMS replication would require a different domain/weight treatment. The normalized expectation controls the numeraire weight's mass but does not by itself prove the input smile is arbitrage-free.

## Evidence

- [Replication kernel](../../../../finstack-quant/valuations/rates/src/cms_option/replication_pricer.rs), [weight derivatives](../../../../finstack-quant/valuations/rates/src/cms_common.rs), [swap replication](../../../../finstack-quant/valuations/rates/src/cms_swap/pricer.rs).
- [Replication sign tests](../../../../finstack-quant/valuations/rates/tests/cms_option/replication_sign.rs), [pricing and seasoned-coupon tests](../../../../finstack-quant/valuations/rates/tests/cms_option/test_pricing.rs), and inline swap cap/floor/parity tests were located or inspected, not executed. They exercise the existing integration method and do not qualify the accepted price law.
- [Hagan CMS](../../../REFERENCES.md#hagan-2003-cms-convexity), [Black](../../../REFERENCES.md#black-1976), [numerical integration reference](../../../REFERENCES.md#press-numerical-recipes).

### Independent review evidence

- With `F=.05`, support `[.02,.08]` and ATM call `.015`,
  both secants are `−.5`, giving masses one half at each endpoint.
  With one reference payment `α=.5,u=.5`, `m=2,δ=1`,
  the normalized weight ratio is `h(r)=2.05/(2+r)`. Then
  `Z=42025/42016`, `M=254/5125`, call at `.05` is
  `303/20500` and put is `78/5125`; their difference is
  exactly `M−.05`. The call PV is about `7,168.53658537`
  on one million notional, accrual `.5` and payment factor `.97`.
- At `F=.04`, keep the same unit call quotes at
  `(.025,.01625),(.04,.005),(.06,.0025)`. Support
  `[.02,.08]` gives masses `1/4,5/8,1/8` at
  `.02,.04,.08`. Support `[.015,.10]` gives masses
  `1/8,1/8,5/8,1/16,1/16` at `.015,.025,.04,.06,.10`.
  Both recover exactly the same quotes and forward. For
  `w=(1+r)/(2+r)`, their payment means are about
  `.04014006615` and `.04016533973`. The payment call at
  `.085` goes from zero to about `.00096333356529`.
- At `F=.05`, with support `[.01,.10]` and original
  quote pairs `(.03,.021),(.05,.010),(.07,.008)`, the calls
  meet their individual bounds and are decreasing. The last
  two interior-to-boundary slopes are `−.1,−4/15`; the
  negative jump requires rejection. A nodal price-bound check
  alone would admit this inconsistent strip.
- For `F=.04`, equal annuity masses at `.02,.06` and
  `w=1+r`, payment probabilities are `51/104,53/104` and
  the normalized mean is `.0403846153846`. With spread
  `.005`, floor `.03` and cap `.05`, the expected coupon
  is `.0401923076923`, worth `19,694.2307692` for notional
  one million, accrual `.5`, and payment factor `.98`.
- A separate analytic full-support Black comparison at `F=.04`,
  `σ=.20,T=2` with the same `w=1+r` has mean
  `(F+F² exp(.08))/(1+F)≈.0401281339503`. Using truncated
  lognormal moments, the same spread and collar have expected
  coupon `.0425606208890` and PV `20,854.7042356` on
  the same cash scale. This is a comparison under a different
  distribution; precise computation of a finite-support price
  does not imply agreement with a continuous model.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
