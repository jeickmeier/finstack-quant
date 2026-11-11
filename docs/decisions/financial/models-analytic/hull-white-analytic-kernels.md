# MODELS-ANALYTIC-23: Hull–White affine bonds, caplets and covariance corrections

**Decision status:** Proposed (complete package, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent scalar, high-precision and Gaussian checks support the proposed distinctions. Production and end-to-end qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Proposed decision for checked Hull–White analytic kernels and payment measures

1. **Use the one-factor Gaussian short-rate law.** The centered state
   follows `dx = -kappa x dt + sigma(t) dW` with deterministic
   curve-fitting shift in the short rate. Use constant finite
   kappa≥0 and finite nonnegative scalar or scheduled sigma. Exact
   kappa=0 is the Ho–Lee limit. Zero sigma is a deterministic model.
   Calibration bounds and fitted-parameter selection remain at
   their owners.

2. **Use one immutable checked pricing state.** Build scalar sigma as
   the constant case of the same validated scheduled model. Use the
   accepted nonnegative, right-continuous, flat-final coefficient
   schedule in CORE-NUMERICS-21. Apply the same validation on
   restoration. Remove public mutable model fields, unchecked
   parameter paths and generic model defaults. A compact fit record
   can retain fitted values under its calibration contract.

3. **Require an explicit common model clock and curve roles.** Model
   times are finite elapsed years on one qualified origin and clock.
   Kappa is in inverse model years and sigma is absolute decimal
   short-rate volatility per square root model year. Contractual
   accrual tau is a separate positive fraction. Discount and
   projection inputs have distinct roles. Record 24 owns dated
   clock mapping, instantaneous-forward sampling and fitted-state
   selection.

4. **Use checked affine loading.** For 0≤a≤b set
   `B(a,b) = integral_a^b exp(-kappa(u-a)) du`.
   Compute it using the stable expm1 ratio at the complete product
   kappa(b−a). Exact zero duration gives zero and exact kappa=0
   gives b−a. Validate the whole requested domain before an
   identity. Remove thresholds on kappa alone.

5. **Use factored loading differences.** For a≤b≤c evaluate
   `B(a,c)-B(a,b) = exp(-kappa(b-a)) B(b,c)`.
   For a signed difference choose the ordering before forming the
   positive magnitude. Preserve the complete result when long
   loadings separately round to the same number.

6. **Use the exact conditional state-noise integral.** Define
   `V(a,b) = integral_a^b sigma(u)² exp(-2 kappa(b-u)) du`.
   Use CORE-NUMERICS-21 for scheduled segments and the constant
   schedule through the same calculation. At kappa=0 this is
   the sum of sigma-squared durations. A zero interval or
   zero supported sigma gives exact zero. Retain scale for
   downstream loading, variance and square-root products.

7. **Check the complete covariance domain.** Conditional on the
   state at v, the covariance for times t,u≥v is
   `V(v,min(t,u)) exp(-kappa |t-u|)`. Origin-based queries
   use v=0. Validate both times before a zero identity.
   Restore the complete covariance in scale and propagate
   unavailable positive or nonfinite requested results.

8. **Share the state and integrated-state covariance.** Use
   `J(a,b) = integral_a^b sigma(u)² B(u,b) exp(-kappa(b-u)) du`,
   which equals the covariance of the noise in x_b and in the
   short-rate integral over that interval conditional on x_a.
   For a constant-sigma segment [l,r] inside it, the contribution
   is `sigma² (B(l,b)² - B(r,b)²)/2`. Evaluate as a factored
   difference with the common loading kernel. Reuse this
   moment for payment shifts and futures.

9. **Use the scheduled affine discount-bond law.** With qualified
   initial discount factors and signed instantaneous f0(t), use
   `ln A(t,T) = log(P0(T)/P0(t)) + B(t,T) f0(t)
   - B(t,T)² V(0,t)/2`. Require positive finite initial
   factors with the proper normalization and clock. Admit
   factors above one and signed rates.

10. **Evaluate conditional bonds on the original rate coordinate.**
    Use `ln P(t,T) = log(P0(T)/P0(t)) + B(t,T)(f0(t)-r_t)
    - B(t,T)² V(0,t)/2`. Use a checked log ratio and scale
    through the final requested value. At t=T the bond price
    is exactly one and log A is zero. Initial-curve fit uses
    the qualified initial state r_0=f0(0). The curve adapter
    supplies f0(t) under record 24.

11. **Use total bond-option log standard deviation.** With
    valuation v, expiry e and bond maturity m, require
    0≤v≤e≤m and use `q = B(e,m) sqrt(V(v,e))`.
    Preserve q in scale through its use in pricing. This is
    a total log standard deviation. A genuine zero loading
    or variance yields zero.

12. **Price the bond option on its original discounted legs.**
    Admit finite positive qualified factors D_e and D_m and a
    finite positive bond-price strike K. Use the Black law with
    total q on legs `A = D_m` and `H = K D_e`. The result
    is PV per unit bond face. Parity is A−H. Use the checked
    Black geometry of records 02–03 with scaled legs and OTM
    tails, preserving the complete requested price even when
    a detached ratio or product would lose range.

13. **Use exact deterministic option limits.** At genuine q=0
    evaluate original-leg intrinsic. A positive q uses the
    same Black law at every supported scale. Remove small-q
    intrinsic shortcuts, zero clipping and NaN sentinels.
    A positive requested time value lost by the calculation
    must be resolved or produce a computation error.

14. **Admit explicit term caplet periods.** Require
    0≤v≤f≤S<E, payment P≥S, finite tau>0 and finite signed
    rate strike K. Here f is fixing and S,E are accrual
    endpoints. Preserve early payment S≤P<E as an admitted
    case. Product owners qualify actual period dates, known
    fixings and settled cash, including same-day information.

15. **State the deterministic projection assumption.** Let
    `G_0 = P_p(v,S)/P_p(v,E)` from qualified positive
    projection factors. The modeled fixing accumulation is
    the discount bond ratio at fixing times a deterministic
    multiplicative basis. Its basis factor is G_0 divided
    by the initial discount bond ratio. The model uses the
    same Gaussian loading for both, with separate payment
    discounting `P_d(v,P)`.

16. **Use stable fixing and payment loadings.** Set
    `L = exp(-kappa(S-f)) B(S,E)` and
    `D = B(f,P) - B(f,E)`, using the signed factored
    difference. Under the payment-date measure use
    `G_P = G_0 exp(-V(v,f) L D)` and total log standard
    deviation `q = L sqrt(V(v,f))`. The multiplier is
    exactly one at P=E. It increases the mean for P<E and
    decreases it for P>E when variance is positive.

17. **Price the cap or floor on gross accumulation.** Form the
    exact strike `H = 1 + tau K`. For H>0 use a Black call
    for the caplet or put for the floorlet on (G_P,H,q).
    For H≤0 the caplet is linear with expectation G_P−H
    and the floorlet is zero. Use exact intrinsic at genuine
    q=0. Apply P_d(v,P) once. Retain the complete scaled
    premium, including close strikes and small positive q.

18. **Keep cap/floor aggregation on explicit periods.** Use the
    same checked period law for constant or scheduled sigma
    and for calibration and valuation. Use ordered checked
    accumulation of PV per unit notional with an exact empty
    sum after admission. Remove the legacy implicit
    fixing-equals-start and payment-equals-end tuple.
    Product owners handle quantity, cash state and payment
    inclusion.

19. **Expose a checked term payment adjustment.** Reuse the
    exact multiplier from the term caplet law for an
    admitted term-index coupon. Preserve signed early or
    delayed payment and the same-day identity. Share
    checked logarithmic arithmetic with pricing so the
    complete price does not depend on a detached multiplier
    fitting binary64.

20. **Make continuous compounded accumulation explicit.** For
    the model payoff from continuous accumulation
    `exp(integral_S^E r(u) du)`, require
    0≤v≤S<E≤P. Its payment log multiplier is
    `-B(E,P) [B(S,E) exp(-kappa(E-S)) V(v,S) + J(S,E)]`.
    Multiply the qualified end-measure accumulation mean
    by its exponential. No fixing input is needed.
    Payment at E has exact multiplier one.

21. **Keep contractual overnight schedules with their owners.**
    Replace the generic compounded boolean with the actual
    term or continuous accumulation operation. Continuous
    accumulation is a model for the stated integral and a
    mesh limit of reciprocal-discount resets. A contractual
    discrete coupon, including lookback or cutoff, requires
    a qualified schedule model or an explicitly selected
    continuous approximation. Reconcile the existing swaption
    coupon adapters and overnight cap/floor owner.

22. **Select the exact standalone simple-rate futures bias.**
    Use fixing at accrual start S, 0≤v≤S<E, one admitted
    Gaussian model and explicit contractual tau>0. Assume
    the futures-implied simple rate is the money-market
    measure expectation for the same ratio underlying the
    accrual-end forward. Admit finite signed futures rate R_f
    with positive gross accumulation 1+tau R_f.

23. **Use the checked quote-dependent adjustment.** Set
    `b = B(S,E)` and
    `z = b [J(v,S) + b V(v,S)]`.
    Return the nonnegative rate adjustment
    `(1+tau R_f) (-expm1(-z)) / tau` to subtract from R_f.
    Keep the complete scaled product. Exact zero z gives zero.
    The current z/tau helper is the low-rate, small-z limit.
    Actual term and overnight futures selection remains with
    VALUATIONS-RATES-10.

24. **Return a price-equivalent term caplet ATM Normal quote.**
    Require positive time to fixing. From the same term
    period law return ATM strike `(G_P-1)/tau` and annual
    Normal sigma
    `(G_P/tau) sqrt(2 pi/(f-v)) erf(q/(2 sqrt(2)))`.
    Use the checked ATM premium law of record 22 with
    complete scaling. The common payment discount cancels.
    Preserve zero from exact q=0. Retire the proxy that
    drops the gross accumulation level and substitutes
    contractual accrual for model duration.

25. **Preserve explicit parameter identifiers and active inputs.**
    Keep separate generic HW and cap/floor fitted keys,
    including the cap/floor schedule key. Owners qualify the
    curve and instrument association and choose the actual
    active fitted or explicit state. Numerical kernels take
    admitted parameters and make no market lookup.

26. **Propagate checked arithmetic through actual callers.**
    Reconcile term cap/floor pricing and fit, bond and tree
    consumers, swaption coupon adjustments and conditional
    projection adapters. Preserve underlying errors and the
    actual chosen law. Record 24 owns dated mapping and
    record 25 owns simulation discounting and estimator
    controls. Neither follows from affine fit alone.

27. **Reconcile the existing host contracts.** Update Python
    and WASM model constructors, scheduled state, bond price,
    cap/floor periods, exact futures and ATM Normal quote
    paths, and associated schema, stubs, declarations, facade
    and parity. Rust owns the formulas. Keep model unit
    outputs and product money handling distinct.

28. **Qualify the stated domain independently.** Cover signed
    rates, exact Ho–Lee and deterministic limits, irregular
    sigma segments, tiny positive option volatility and
    extreme discounted legs. Test early/end/late payment,
    exact nonpositive accumulation strikes, continuous versus
    discrete coupons, quote-dependent futures bias and
    actual ATM price replay. Qualify dated and host paths
    separately. See the
    [Hull–White analytic references](../../../REFERENCES.md#checked-hullwhite-analytic-kernels-and-payment-measures).

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| kappa=1e−12, duration 1e12 | The state variance time factor is about 0.43233236 times the duration. A kappa-only Ho–Lee shortcut uses the whole duration. |
| kappa=0, sigma 1e−200, unit noise interval and bond loading | Standalone variance would round to zero. The total bond log standard deviation is representable at 1e−200. |
| kappa=0.18, sigma knots (0,0.4,1.7,3.1) with sigmas (0.006,0.022,0.011,0.016), t=2,T=6, flat f0=−0.003, r_t=0.014 | The conditional discount bond is about 0.96225337199. The affine expression agrees with the independent conditional integrated-rate Gaussian law. |
| kappa=0.07, sigma 0.025, fixing/start/end=(2,2.02,2.52), payment at 2.50/2.52/2.72 | The term payment multipliers are about 1.000010324, 1 and 0.999897559. |
| Same kappa and sigma, continuous period S=2.02,E=2.52,P=2.72,v=0.5, compared with two equal discrete reset intervals | The continuous payment log correction is about −0.00009561814. The discrete correction is about −0.00008806059. |
| kappa=0.03, sigma 0.015, S=5,E=5.25,tau=0.25, futures rate 0.06 | The exact bias is about 0.00269634, giving forward 0.05730366. The leading adjustment is about 0.00265738. |
| kappa=0.05, sigma 0.012, f=S=4,E=P=5,tau=1,G_P=1.04 | The ATM Normal quote is about 0.01105123. The log-ratio standard-deviation proxy is about 0.01062638. |

These checks compare independent analytic, integrated Gaussian and
scalar formulas. Production pricing, calibration and host qualification
remain pending.

## Observed implementation and evidence

### Scope and input domain

One-factor Gaussian rate kernels with constant κ and scalar/piecewise-constant σ. Scalar calibration params require κ,σ>0; scheduled model requires κ>0 and delegates schedule validation to core PiecewiseConstantCurve. Some standalone kernels permit κ=0.

## Choice and rationale

Affine bond coefficients and integrated Gaussian variance price zero-coupon options and caplets; deterministic multi-curve ratios and payment-measure covariance adjust projection. Futures convexity helper is a leading low-rate/small-variance approximation, not the full quote-dependent bias.

## Algorithm

Bκ(t,T)=−expm1[−κ(T−t)]/κ with exact zero-product limit T−t. State variance is ∫σ²(u)exp[−2κ(t−u)]du; covariance=Var(earlier)exp[−κ|Δt|]. Bond vol=B(expiry,maturity)√integrated variance. ln A=ln P0(T)−ln P0(t)+B f0(t)−σ²(1−exp(−2κt))B²/(4κ), with Ho–Lee limit. ZCB option applies Black to P0(maturity),K P0(expiry),total bond vol,unit time; <1e−15 vol uses intrinsic. Simple caplet uses bond strike1/(1+τK), opposite bond option direction and multiplier (Pd(pay)/Pf(pay))(1+τK). If1+τK≤0, cap is linear and floor zero. Scheduled term caplet prices ratio Pf(start)/Pf(end) adjusted by exp[−Var(xfix)×ratio_loading×payment_loading]; ratio loading=B(fix,end)−B(fix,start), payment loading=B(fix,pay)−B(fix,end), total ratio vol=abs(bondvol(end)−bondvol(start)). Discount by Pd(pay). Cap/floor sums period prices.

## Selection and controls

Futures adjustment=.5σ²(Bsettle,end/τ) × (B0,2settle×Bsettle,end+B0,settle²). Delayed compounded coupon multiplier=exp[−Bend,pay×(Bstart,end exp(−κΔ)Var(xstart)+.5σ²Bstart,end²)]. Term multiplier uses fixing-state variance times differences of B. Approximate normal caplet vol=σ B0,τ/τ ×√[(1−exp(−2κtfix))/(2κtfix)], with small-κ limit. Market scalar-key helpers distinguish generic HW from cap/floor calibration, and expose a cap/floor σ-schedule key.

## Conventions

κ inverse years, σ absolute decimal short-rate units/√year, τ contractual accrual fraction distinct from model-year intervals; bond vol is total log standard deviation. Pf projects and Pd discounts. No stochastic basis factor is modeled.

## Limitations, failure and fallback

Low-level scalar helpers have unequal validation; bad arithmetic may return NaN or clamped intrinsic while Result variants validate schedules/times. Payment time is allowed between start/end; beyond-end is not mandatory. `state_covariance` returns zero when earlier time≤0 before complete time-domain checking. Convexity approximation must not be represented as exact futures bias.

## Alternatives and tradeoffs

Constant and piecewise σ variants are supported. Full quote-dependent futures convexity and stochastic multi-curve basis are outside these helper formulas.

## Implementation and validation evidence

[hull_white.rs](../../../../finstack-quant/models/src/rates/hull_white.rs). `delayed_term_caplet_matches_independent_payment_measure_integral` integrates Gaussian loading covariance independently; scalar/schedule equivalence and put/call parity are internal invariants. `convexity_matches_quantlib_low_rate_small_variance_expansion` distinguishes approximation from full bias. References: [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-ird), [Brigo–Mercurio](../../../REFERENCES.md#brigo-mercurio-2006-interest-rate-models).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
