# MODELS-ANALYTIC-24: Rate model clocks, curve fitting and conditional forward projection

**Decision status:** Accepted (complete package, 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent date, scalar and Gaussian checks were executed for the accepted formulas. Production pricing, calibration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked model clocks, curve fit and term projection

1. **Use an explicit dated model origin.** A dated model instance uses
   ACT/365F from the origin supplied by its owner. For a date d, model
   time is the signed integer difference in calendar days divided by
   365. Live model queries require nonnegative model time. Keep signed
   date differences for eligibility and history at the caller. The
   source curve base, fixing cutoff and coupon year fraction have
   separate roles. Rate parameters and numerical steps use model years.
   [ADR 0045](../conventions/0045-financial-clocks-and-date-boundaries.md)

2. **Preserve exact dated coordinates.** Map a dated request directly
   from its date to the source coordinate. A numerical request equal
   to that date's canonical binary64 model time denotes the same date.
   Use checked date arithmetic and exact equality with that time.
   Multiplication by 365 can estimate a day bracket. Require the
   canonical adjacent knots to establish the actual bracket.

3. **Use each source's complete native clock.** Evaluate its signed
   year fraction from its own base with its stored day count and
   required context, including the resolved Bus/252 calendar. Keep
   the source's explicit signed-time and tail policy. A model origin
   before a source base is eligible when the source admits the signed
   query. Propagate missing context, unavailable coverage and
   unsupported coordinates. Do not substitute a default context or
   clamp a negative native coordinate to the curve origin.
   [CORE-MARKET-01](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction)

4. **Interpolate the clock on fractional model days.** Between
   consecutive canonical model-day knots `t_d<t_{d+1}`, use weight
   `(t-t_d)/(t_{d+1}-t_d)` to interpolate their native coordinates,
   then evaluate the source at the mapped time. Preserve nondecreasing
   native coordinates, including plateaus. Reject a decreasing
   native segment or an unavailable date bracket. A factor query at
   an exact date needs only that date's mapping. Fractional time
   remains numeric and does not manufacture a fixing or event date.

5. **Rebase discount factors at the model origin.** For native source
   factor `D_s` and mapped coordinate `c(t)`, set
   `D(t)=D_s(c(t))/D_s(c(0))`. Qualify the anchor and both
   absolute endpoint factors as required by CORE-MARKET-01. Return
   exact D(0)=1 after admission. Use checked relative factors
   for factor requests and stable differences of positive-factor
   logs for log requests. A log-ratio request can avoid a detached
   unrepresentable relative factor. Inception ratios at two model
   times reuse the original source endpoints and cancel the anchor.

6. **Differentiate the actual mapped discount function.** Compute
   `f0(t)=-d log D(t)/dt` by composing the native curve's
   checked one-sided log-factor derivative with the selected clock
   slope. Take the right limit at model t=0 and the left limit at
   positive model breakpoints. Use the true approached side of the
   complete source evaluator, including native knots, transforms
   and tails. Require the selected daily bracket to be nondecreasing.
   A flat daily clock segment gives zero composed forward after
   factor admission. A rolled origin on a native knot can
   require its right limit even though the source's standalone
   derivative reports its left limit.

7. **Use only the capabilities a result requires.** A fitted short-rate
   law needs a source whose mapped log discount is absolutely continuous
   on the admitted intervals. A short-rate value additionally requires
   the actual composed derivative. A factor-only calculation needs its
   checked factors. Refuse an unsupported derivative, inadmissible tail,
   bad date, invalid parameter or unresolved range using the appropriate
   domain or computation error. Remove the finite-difference epsilon
   recipe and error-to-NaN callbacks from this fitted path.

8. **Bind one fitted model to the active discount source.** Use the
   immutable checked Gaussian parameters of
   [record 23](hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures)
   with the same origin, source and coefficient schedule for discount
   fit and conditional projection. Numeric-only setups must supply a
   qualified curve on the declared common model clock. Keep exact
   kappa=0 and zero-volatility cases. A different origin, source or
   coefficient state requires fresh preparation.

9. **Fit the curve with an exact deterministic shift.** Use the
   centered state `x(0)=0`, `dx=-kappa*x*dt+sigma(t)*dW` and
   `r(t)=x(t)+phi(t)`, where `phi(t)=f0(t)+J(0,t)` and J
   is record 23's state and integrated-state covariance. Initialize
   the reported short rate from the discount curve's right forward
   at the origin. Keep deterministic short-rate jumps at admitted
   curve or clock kinks. The centered state stays continuous.

10. **Use the conditional integrated-state variance.** Define
    `Q(a,b)=integral_a^b sigma(u)^2 B(u,b)^2 du`. Evaluate
    the scheduled contributions through the shared checked loading
    and stable local integrals. Preserve exact zero-duration and
    zero-noise cases and the kappa=0 polynomial limit. Use
    nonnegative segment contributions and retain scale through the
    complete requested result.

11. **Integrate the deterministic shift directly.** With
    `B=B(a,b)` and `0<=a<=b`, use
    `integral_a^b phi(t)dt = -log(D(b)/D(a)) + B J(0,a)
    + (B^2 V(0,a)+Q(a,b))/2`. The variance terms follow by
    splitting integrated-state noise at a. They avoid subtracting
    nearly equal cumulative variances. From the origin, Gaussian
    expected discount equals D(b) in the model. The curve's
    point-forward convention at a kink leaves the integral unchanged.

12. **Remove the monthly mean-level fit from curve-driven paths.**
    Use the centered Gaussian transition with record 23's exact
    conditional variance. Derive the deterministic shift or its
    integral on the actual requested support. Zero horizon needs no
    synthetic future fit point. No implicit month extension,
    theta fit, or division by kappa is needed. Explicit mean-level
    OU models have their own input law. Record 25 owns the
    stochastic bank-account integration, path grid and estimator
    decisions.
    [OU process record](../models-monte-carlo/07-hull-white.md)

13. **Expose conditional discount bonds on their proper coordinate.**
    With centered state `x_a`, use
    `log P_d(a,b)=log(D(b)/D(a))-B(a,b)(x_a+J(0,a))
    -B(a,b)^2 V(0,a)/2`. This is record 23's short-rate law
    after substitution. Evaluate the centered form directly for
    centered paths. A supplied short-rate coordinate follows record
    23. Return one at identical dates after required admission.
    Keep bond evaluation separate from index rate reconstruction.

14. **Use the actual index period.** Prepare a live in-advance
    observation with `0<=f<=S<E` and finite positive contractual
    index accrual `tau`, where f is fixing, S effective and E
    maturity. A fixing before S is eligible. Product owners
    resolve the index tenor, calendar and value-date rules and
    qualify any contract outside this ordering. A coupon accrual,
    payment date or nominal tenor cannot supply the index interval
    or its year fraction.

15. **Resolve one explicit projection role.** Obtain the inception
    gross index accumulation `G_p0=P_p(S)/P_p(E)` from the
    qualified index-specific factor source. Use the discount factor
    ratio only for an explicitly selected, product-supported
    single-curve role. Keep projection factors distinct from tradable
    discount bonds. Follow CORE-MARKET-02's actual endpoint, source
    clock and strict factor admission. Retire point fixed-tenor
    lookups and implicit discount fallback from this reconstruction.
    [CORE-MARKET-02](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates)

16. **Use the deterministic multiplicative basis of record 23.**
    For each qualified period, the basis is G_p0 divided by the
    inception discount gross `D(S)/D(E)`. At fixing, multiply
    the conditional bond ratio `P_d(f,S)/P_d(f,E)` by
    that basis. This preserves positive gross accumulation and
    the same Gaussian loading used for analytic term caplets.
    Remove the generic additive simple-rate offset in this path.
    The selected law models deterministic basis.

17. **Evaluate the combined path gross in log scale.** With
    `L=exp(-kappa*(S-f))*B(S,E)`, use
    `log G(f)=log G_p0+L[x_f+J(0,f)
    +(B(f,E)+B(f,S))*V(0,f)/2]`. Compute L directly by
    the factored difference. Retain scale through the combined
    log and final output. This form avoids detached bond
    ratios and cancellation of the inception discount gross.
    At inception with `x_0=0`, it recovers the index gross.

18. **Check the requested simple rate.** Return the annual decimal
    `expm1(log G(f))/tau` using stable final scaling and
    CORE-MARKET-02's returned-rate positive-gross gate. Admit
    signed rates within the projected gross domain. Permit a
    representable complete rate when an unrequested intermediate
    gross or path bond is out of range. Reject a projected rate
    rounded onto the zero-gross boundary. Preserve true zero and
    propagate numerical failures without tenor, date or gross floors.

19. **Distinguish a path fixing from a payment-measure expectation.**
    The gross fixing in point 17 has mean G_p0 under the
    accrual-end forward measure. Sample the actual fixing on each
    eligible path and apply the path cash discount at the
    payment time. Record 23 supplies the analytic
    payment-measure adjustment when computing an expectation
    directly. Do not also apply that adjustment to the path
    fixing. Curve coverage through index maturity can extend
    beyond the final required path observation.

20. **Keep known fixings literal.** An eligible known or explicitly
    supplied deterministic fixing is a checked finite signed rate,
    distinct from a stochastic period and from a discount bond.
    Use its value directly. Remove the flat-rate bond-coefficient
    substitute and its denominator floor. Keep information cutoff,
    missing history, realized state, contractual spread or gearing,
    coupon accrual and cash payment with the product owner.
    [CORE-MARKET-12](../core-market/scalar-series-and-fixings.md)

21. **Select complete parameters together.** The product or
    calibration adapter supplies one checked kappa and exactly
    one scalar sigma or schedule as a coherent selection.
    An explicit parameter override is complete. The fitted
    alternative must resolve the qualified unitless representation
    using record 23's family keys. Reject partial overrides,
    contradictory active forms, monetary values and invalid
    selected state. Do not blend unrelated persisted and explicit
    inputs or fall through on an invalid selected source.

22. **Publish and restore complete fitted state.** Associate the
    fitted pair with its origin, selected form and applicable
    curve, index and fit provenance. Publish or replace the active
    fitted representation atomically, so a stale schedule and
    a new scalar fit cannot silently share kappa. Restoration
    enforces the same parameter, schedule, role and provenance
    validation. Owner policies still select which fits may be used.

23. **Preserve the original sigma support.** A dated fit retains its
    actual fit origin and exact dated support, with the first
    dated knot at that origin. A synthetic numeric schedule
    retains model-time knots and is used on its declared origin
    and clock. A different origin needs a separately qualified
    schedule. Never manufacture dated knots by rounding 365
    times a numeric knot. Treat a persisted dated schedule as
    exact ordered unitless step data under record 23, without
    generic time-series interpolation.

24. **Materialize a qualified dated schedule on the active origin.**
    At the same or a later owner-approved origin, use the latest
    effective knot on or before that date as sigma at t=0 and
    retain subsequent dates at their exact ACT/365F model times.
    A knot at the origin supplies the new value. The final
    sigma extends flat. An origin before fit coverage fails.
    The discount curve base does not move the schedule. The
    product or calibration fit policy governs reuse after rolling.

25. **Keep valuation scenarios explicit.** With parameters held
    fixed, rebuild the deterministic shift for the bumped
    discount source and qualify projection roles for that scenario.
    A sigma bump keeps the active schedule form and knot times.
    Refit only through an explicit calibration/replay policy.
    Product and calibration owners choose the fit or scenario
    policy and qualify its inputs.

26. **Reconcile actual valuation and calibration paths.** Route
    callable range accrual, Snowball, TARN, revolving credit,
    MBS OAS and Bermudan paths through the common dated state
    and the applicable projection or bond operation. Keep spot
    used for contractual bounds separate from the initial short
    rate. Reconcile scalar and scheduled cap/floor pricing and
    risk, and direct and envelope calibration. Keep product
    exercise, settlement and unsupported-product choices with
    their owners.

27. **Reconcile existing host surfaces.** The existing dated
    pricing and calibration paths in Python and WASM use the
    same Rust origin, curve roles, fitting and checked errors.
    Update associated declarations, state/schema and parity
    where those paths expose a changed contract. Clock and
    projection preparation remain Rust helpers.

28. **Qualify dated and numerical behavior independently.** Cover
    differing source origins, leap years, Bus/252 plateaus,
    native and model kinks, strict tails, canonical dated
    coordinates and fractional queries. Compare the deterministic
    fit and conditional bond with independent Gaussian values.
    Exercise fixing lag, different index and coupon accrual,
    distinct discount/projection clocks, signed known fixings,
    no active fit, rolled sigma and actual valuation, risk,
    calibration and host paths. Model identities apply on
    admitted curve support, subject to checked finite precision.
    [Rate-clock and projection references](../../../REFERENCES.md#hullwhite-rate-clock-curve-fit-and-term-projection)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Three days on ACT/365F | Binary64 `(3/365)*365` is 2.9999999999999996. Exact canonical time lookup selects the third date. |
| Source base 2023-12-29, model origin 2023-12-31, ACT/ACT ISDA source at a continuous 3.6% | At 1.5 model days the rebased factor is about 0.9998522004585734. At the January 1 kink the left and right model forwards are 3.6% and about 3.590164%. |
| Deterministic forward at 2.5% through 1/48 year and 5.5% thereafter, kappa=0.05 | Matching the short-rate mean at the monthly endpoint gives a discount factor of 0.9966713508883. The source and exact shifted law give 0.9960494905415. |
| Scheduled sigma (0.006, 0.022, 0.011, 0.016) at (0, 0.4, 1.7, 3.1), kappa=0.18, interval [2,6] | Independent split quadrature agrees with the factored integrated-state variance difference, about 0.008990866955. |
| kappa=0.07, sigma=0.025, f=2, S=f+2/365, E=S+91/365, tau=91/360, x_f=0.01, initial index rate 3.5% | The original affine bond ratio with basis and the centered formula both give 4.6087477245%. The accrual-end forward-measure expected gross is 1.008847222222. |
| Origin January 10, curve base January 20, sigma switch February 10, kappa=0.05, sigma 0.01 then 0.02 | The switch is at 31/365. Variance to that date is about 8.45719e-6. Moving the switch to 21/365 would give about 1.66651e-5. |

The date, Decimal and quadrature checks support the accepted
semantics and formulas. Production and complete-path qualification
remain pending.

## Observed implementation and evidence

### Scope and input domain

Curve-driven HW1F setup and term forwards. Model time is actual calendar days/365 from valuation date; source discount curve can have its own base date/day-count.

### Choice and rationale

Convert model time through dates before querying/rebasing the discount curve. Conditional affine bond ratios derive simple forwards; a deterministic additive spread can represent projection basis.

### Algorithm

Map t to365t calendar days, query source year fraction at floor date and interpolate linearly to next day for fractional day; reject nonfinite/negative/out-of-date-range t. Model DF=Psource(mapped t)/Psource(mapped0), requiring finite positive values. Estimate f0(t)=−central difference ln P at h=clamp(.001t,1e−6,1e−3) if t>h; near origin use−ln P(h)/h. `prepare_hw1f_params` supplies rebased discount callback to shared theta calibrator on monthly knots i/12 with ceil(12horizon) count (at least1); initial rate=f0(0). `period_coeffs` gets B and ln A at t=max(fixing,0),T=t+max(τ,0); conditional P=exp(ln A−Br), simple forward=(1/P−1)/τ+spread, or0 ifτ≤0. Deterministic curve_forward uses P(t)/P(t+τ).

### Selection and controls

Explicit HullWhiteCalibrationParams and discount curve/as_of/horizon. Monthly theta spacing is internal, not exposed in these helpers. `from_flat_rate` creates B=0 and ln A=−ln(max(1+rateτ,MIN_POSITIVE)); additive spread changes simple forward only, not discount_factor. CouponEvent carries accrual, discount factor, conditional coefficients and needs_path_sample flag.

### Conventions

Calendar model time and contractual coupon accrual are distinct. Source DF base date is rebased to valuation date. Projection from the modeled discount curve plus additive spread is deterministic-basis treatment, not independent stochastic forward dynamics.

### Limitations, failure and fallback

Negative finite fixing/tenor are clamped in period_coeffs, whereas curve_forward requires positive tenor. Flat-rate denominator is floored, silently modifying extreme rate×accrual inputs. Origin finite-difference formula assumes normalized P(0)=1, satisfied by ModelDiscountCurve. Theta fitting details live in the shared OU process implementation beyond this file slice.

### Alternatives and tradeoffs

An explicit discount/forward multi-factor model or exact analytic curve derivatives would change assumptions. Current date mapping preserves curve conventions while monthly theta introduces its own approximation.

### Implementation and validation evidence

[clock.rs](../../../../finstack-quant/models/src/rates/clock.rs), [curve.rs](../../../../finstack-quant/models/src/rates/hw1f/curve.rs), [coupon_event.rs](../../../../finstack-quant/models/src/rates/hw1f/coupon_event.rs) and [hull_white.rs](../../../../finstack-quant/models/src/rates/hull_white.rs). Inline flat-curve/curve-rebasing and invalid-DF tests locate mapping/formula evidence; no date-clock accuracy claim beyond inspected implementation is made. References: [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-ird), [Interest-rate modeling](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling).

The source-clock adapter uses a default day-count context. The
[source discount trait](../../../../finstack-quant/core/src/market_data/traits.rs)
already supplies a context. The
[OU mean fit](../../../../finstack-quant/models/src/monte_carlo/process/ou.rs)
matches sampled means. Its one-knot case adds a one-year fit
interval, so a request shorter than one month can sample beyond
its horizon. The
[Bermudan](../../../../finstack-quant/valuations/rates/src/swaption/bermudan.rs)
and [MBS OAS](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/mc_oas.rs)
paths also prepare or consume theta. The
[revolver](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_generator.rs)
supplies an explicit simulation origin.

[Callable range accrual](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/pricer.rs),
[Snowball](../../../../finstack-quant/valuations/exotics/src/exotics/snowball/pricer.rs)
and [TARN](../../../../finstack-quant/valuations/exotics/src/exotics/tarn/pricer.rs)
use nominal tenor in rate reconstruction. The
[cap/floor parameter resolver](../../../../finstack-quant/valuations/rates/src/cap_floor/hw_pricer.rs)
can move the sigma anchor to the discount curve base, while the
[fit publisher](../../../../finstack-quant/calibration/src/step_runtime.rs)
rounds numeric support into dates. The
[direct fit adapter](../../../../finstack-quant/calibration/src/hull_white/curves.rs)
and existing [Python](../../../../finstack-quant-py/src/bindings/models/rates/hull_white.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/rates/hull_white.rs)
cap/floor paths also require clock and role reconciliation.

## Follow-up tracking

Implementation and validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
