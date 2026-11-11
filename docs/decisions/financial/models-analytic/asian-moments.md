# MODELS-ANALYTIC-04: Geometric Asian law and arithmetic lognormal moment matching

**Decision status:** Accepted — complete discrete Asian pricing package with explicit fixing levels and payment discount, 2026-11-10.

**Implementation status:** Observed. Reconciliation covers the scalar functions and the dated Asian option consumers below.

**Validation status:** Source, primary references and existing tests inspected. Independent bounded calculations cover selected formulas and discrepancies. Production and complete product qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for discrete Asian pricing

Accepted on **2026-11-10** after review of all 21 decisions.

The scalar model prices a single European payoff on an equally weighted discrete
average. The prepared fixing levels extend the current scalar surface to cover
the existing seasoned equity and commodity analytic consumers. The dated owners
retain observation eligibility, market sources and payment conventions.

1. **Select one checked price API.** Use
   `asian_option_price(params: &AsianPriceParams) -> Result<f64>` for the
   explicit `AveragingMethod::Geometric` or `AveragingMethod::Arithmetic` choice.
   Reuse the existing call/put and averaging enums. The request has required
   strike, option_type, averaging, volatility, payment_time, payment_df,
   observed_fixings and projected_fixings. Each projected row pairs time with
   its forward. Empty slices represent zero fixings of that kind. At least
   one fixing is required in total.

2. **Use actual discrete slots.** Every supplied level has weight 1/N,
   where N counts observed plus projected rows. Projected times are finite,
   strictly positive and nondecreasing, with every time at or before payment_time.
   Exact ties retain their multiplicity and share the same Brownian shock.
   Accept no sorting, deduplication, tolerance matching or dropped slots.
   Dated owners retain their strictly increasing contractual dates and check
   calendar ordering independently of model-time ties.

3. **Require a prepared law for each projected level.** Each finite positive
   F_i is E[S_i] in the common pricing measure for the supplied deterministic
   payment factor. With annual relative decimal volatility sigma and coherent
   model-year time t_i, select
   S_i = F_i exp(−sigma² t_i/2 + sigma W(t_i)), with one Brownian motion.
   Its log covariance is sigma² min(t_i,t_j). This includes compatible flat
   carry and dated deterministic carry. The source owner establishes the
   measure and forward inputs, including carry and any fixing-date scaling.
   A flat caller can prepare F_i = S exp(b t_i) from its explicit carry b.

4. **Make the payment factor independent.** Require finite payment_time >= 0
   and finite payment_df D > 0, allowing D > 1. Return D times the payoff
   expectation, per unit of quantity. Discount once at the stated payment.
   The model-time horizon checks the grid. It does not identify the drift or
   imply a discount factor. Zero payment time is admissible with a fully
   observed schedule. Owners determine actual payment eligibility, dates,
   contractual timing, currency and quantity.

5. **Validate the complete request before deterministic or support branches.**
   Require finite K > 0 and finite sigma >= 0. Observed levels must be
   finite and strictly positive for geometric averaging. Arithmetic permits
   finite signed and zero observed levels, preserving the commodity contract.
   Future forwards stay positive under both laws. Observed values are supplied
   facts, with no inferred fixing at valuation time and no spot substitution
   for missing history. Refuse a projected row mapped to zero model time.

6. **Retire count and continuous sentinels.** Remove count-based raw leaves,
   the old explicit-time siblings and `DEFAULT_ASIAN_AVERAGING`. Supply the
   actual grid, including explicit iT/N times and corresponding forwards when
   an equal-spacing contract calls for them. The continuous geometric limit
   remains a mathematical reference. The discrete API has no continuous mode
   and no zero-count or arithmetic fallback.

7. **Price the exact geometric marginal.** For observed levels H_j and
   projected forwards F_i, the full geometric average G has Gaussian log mean
   μ = [Σ log H_j + Σ(log F_i − sigma² t_i/2)]/N and log variance
   v = sigma² Σ_ij min(t_i,t_j)/N², summing over future pairs only.
   Price the fixed-strike option by the Black expectation on mean E[G] and
   total log standard deviation sqrt(v). The full N includes past slots, so
   seasoning changes both the expected level and the distribution.

8. **Evaluate geometric variance and mean with positive interval sums.**
   For sorted projected times, let Δ_i = t_i−t_(i−1), t_0 = 0, and let w_i
   be the number of projected rows from i onward divided by N. Then
   v = sigma² Σ Δ_i w_i² and
   log E[G] = [Σ log H_j + Σ log F_i]/N − (sigma²/2) Σ Δ_i w_i(1−w_i).
   Calculate the remaining fraction and its complement from slot counts.
   This preserves the single-fixing cancellation and avoids subtracting
   rounded nearly equal quantities. Evaluate in linear work.

9. **Keep the arithmetic payoff’s known contribution.** Set
   H = Σ H_j/N, Y = Σ S_i/N and K* = K−H, so the full average is H+Y.
   Moment-match Y only, keeping H as a deterministic shift of the payoff.
   This preserves the lower support from known fixings and the payoff scale.
   The equivalent remaining-average formulation would scale the payoff
   by m/N for m projected rows. The Y formulation already includes that scale.

10. **Match the first two future-contribution moments.** For future rows
    let M = Σ F_i/N and a_i = F_i/Σ F_j. The relative variance of Y is
    R = Σ_i a_i(a_i+2 Σ_(j>i) a_j) expm1(sigma² t_i).
    Accumulate the suffix weights and the nonnegative covariance terms in
    linear work. The fitted lognormal has mean M and log variance log1p(R).
    For K*> 0, price the Black expectation for Y against K* and apply D.
    Match future moments in scaled or log coordinates without forming raw M2
    or subtracting M² from a rounded second moment.

11. **Resolve the exact support branches.** When future observations remain
    and K*<= 0, the arithmetic call is D(M−K*) and the put is exactly zero.
    Use the same prepared F_i on both sides of K*= 0. Determine the sign of K*
    from the original inputs with checked arithmetic. An unresolved comparison
    produces a computation error.

12. **Price exact deterministic cases from the average.** With no projected
    fixings use the supplied realized average. At sigma = 0 use F_i for each
    projected level and compute the selected full average. Return the discounted
    intrinsic of that average. With positive sigma, any future row gives a
    positive model variance. Lost variance does not trigger a deterministic
    fallback.

13. **State exact reductions and the arithmetic approximation’s scope.** A
    single projected row, or multiple projected rows sharing one model time,
    makes their future arithmetic contribution proportional to one lognormal.
    The shifted arithmetic expectation is exact in those cases and in the
    deterministic and support branches. With distinct stochastic times the
    two-moment proxy is generally approximate. Its provenance is the lognormal
    baseline of Turnbull–Wakeman. Their full method adds higher-cumulant terms.

14. **Evaluate the requested discounted result in its final units.** Reuse
    the accepted Black geometry and tail calculations where applicable. Keep
    the original mean, strike and discount geometry through scaling and
    cancellation. Preserve representable discounted results when an unrequested
    raw moment, variance, effective strike or unit premium would overflow or
    underflow. The public unit forward premium in record 02 retains its own
    output-range contract.

15. **Reject unresolved numerical results.** Use checked scaled/logarithmic
    positive sums, stable small-variance arithmetic and resolved differences for
    signed known levels and effective strike. Preserve positive standard
    deviation when its square underflows. Return a finite, nonnegative
    requested price or a computation error. Exact zeros follow the payoff
    branches. Numerical loss of a known-positive price raises an error.
    Remove clamping, epsilon variance floors, NaN and synthetic zero outputs.

16. **Use the applicable parity and bounds.** For the geometric law and
    arithmetic fit, C−P = D(E[average]−K) for the same request. Geometric bounds
    use E[G] and K. Arithmetic bounds use M and positive K* in the shifted
    problem, with the exact branches above. A signed known contribution can
    make a put exceed D K. These are model identities and verification
    targets. The arithmetic fit can fall below the geometric call even
    though the true arithmetic call dominates it on positive paths.
    Report the approximation without a cross-method repair or accuracy claim.

17. **Reconcile the equity Asian analytic paths.** Build one qualified vector
    of dated expectations from the actual carry and fixing inputs. Use it
    consistently across the seasoned arithmetic branches. Reuse the geometric
    moment law in the analytic pricer. Validate that an explicitly selected
    analytic engine matches the contract’s averaging method. For the Monte
    Carlo control variate, match the simulated geometric payoff’s history,
    sampling times and multiplicity, drift and fixing multipliers, volatility
    and covariance, strike and direction, payment discount and quantity.
    Share checked moment arithmetic under the MC owner’s time domain, including
    any admitted model-time-zero slot. Reconcile the event mapping with the
    [accepted time-grid contract](../core-numerics/time-grids.md) and preserve
    errors. The estimator, volatility selection, non-GBM pricers and path
    dynamics retain their owners.

18. **Reconcile the commodity analytic paths.** Reuse the same prepared
    moment laws with each dated commodity forward, the actual independent
    payment factor and the existing observed-level domain. Retire the duplicate
    pricing and moment kernels after actual consumers are reconciled.
    Retain positive future levels for this lognormal model. Other commodity
    laws and source selection retain their owners. Describe the actual
    averaging branch in product model-key documentation, including the
    two-moment baseline meaning of the Turnbull–Wakeman label.

19. **Preserve actual fixing and settlement authority.** Validate dated
    eligibility and history coverage, including required valuation-date
    observations under the existing product contracts. A collapsed model clock
    does not settle a claim. Fully observed claims use their contractual
    payoff and payment treatment without a new live spot or vol query.
    Missing observations and unsupported clocks remain errors. Exercise,
    settlement, publication, quantity and Money conversion remain at the owner.

20. **Update the existing host surface as one contract.** Retain one Rust
    entry point and propagate the required request fields and strict averaging
    values to Python and WASM. Reconcile re-exports, stubs, declarations,
    schemas, parity, examples and affected controls. Reject malformed,
    unknown and duplicate request fields before use or key collapse. Hosts
    translate inputs and Rust error kinds. Invalid schedule/domain is a
    validation error. Numerical refusal is a computation error.

21. **Qualify the law and the approximation separately.** Check independent
    geometric and raw double-sum moment references, including seasoned and
    tied grids, tiny volatility, large scales, signed known values, payment
    timing and exact reductions. Compare arithmetic fitted prices with
    independent pricing of the true sum on representative cases, including
    wings and nonflat forwards. Check actual consumer, host and state paths
    and MC control matching separately. Moment or same-kernel comparisons
    do not establish arithmetic option-price error bounds.

## Independent bounded reference checks

- For S = 100, b = 0.03, t = [0.25,1], sigma = 0.2, K = 105 and D = 0.96,
  a Decimal joint-normal calculation gives geometric call/put
  3.722314860302978 / 7.071460657193956. A separate raw double sum and the
  fitted lognormal give arithmetic call/put 3.919744577626791 /
  6.896573566520353. This compares model formulas on one grid.
- With two fixings at times 0.5 and 1, S = 100, zero yield and volatility,
  K = 110 and D = 0.81, date-specific fixing discounts 1 and 0.81 give
  arithmetic call PV 1.40. Inferring flat drift from D instead gives 5.90.
  The difference occurs on the exact deterministic branch.
- With one known fixing 200, one positive future fixing and K = 90, the
  arithmetic put is identically zero since the known contribution is 100.
  Keeping that contribution in the strike preserves the support.
- With S = 1, b = −2, t = [1,2], sigma = 0.5, K = 10 and D = 1, the
  fitted arithmetic call is about 9.67149e−24 and the exact geometric call
  about 2.70097e−23. Positive paths satisfy AM >= GM, so the true arithmetic
  call is at least the geometric call. The fit misses that lower bound.
- A positive sigma of 2^-550 with S = K = D = 1, b = 0 and t = [0.5,1]
  gives an ATM premium of leading order 8.55761e−167 even though binary64
  sigma² rounds to zero. With one fixing and S = K = 2^600, sigma = 0.2,
  t = 1 and D = 0.5, raw M2 exceeds binary64 range while the call fits at
  about 1.65266e179. These references exercise selected intermediate-range
  distinctions.
- With one observed arithmetic fixing −200, one zero-volatility projected
  level 100, K = 90 and D = 1, the put is 140. Its full average is −50 and
  the original K is therefore not a valid upper bound. In a fully observed
  arithmetic case with level −MAX, K = MAX and D = 0.25, the undiscounted put
  is 2 MAX and the requested discounted price is MAX/2.

## Observed scope and input domain

Fixed-strike calls/puts on an unseasoned equal-weight average under constant GBM. Count-based dates are iT/n; explicit fixing-time variants accept positive nondecreasing times, including duplicates.

## Observed choice and rationale

Geometric averaging is priced through its exact lognormal marginal. Arithmetic
averaging uses two-moment lognormal matching. The Python and WASM wrappers use
the arithmetic default constant when averaging is omitted.

## Observed algorithm

For geometric average, μ=ln S+(r−q−σ²/2) mean(ti), variance=σ² Σij min(ti,tj)/n², evaluated with sorted-time weights 2(n−1−i)+1. Its expected level exp(μ+variance/2) is passed to Black with standard deviation √variance and unit time. Count n=0 means continuous geometric average with σ/√3. Arithmetic first moment is mean[S exp((r−q)ti)]. Second moment uses E[Si Sj]=S² exp((r−q)(ti+tj)+σ² min(ti,tj)); a prefix sum reduces the double sum to O(n), with Neumaier accumulation. Match log variance ln(m2/m1²), price Black on m1 and discount. Degenerate variance returns discounted forward intrinsic.

## Observed selection and controls

Explicit-time methods derive one rate −ln(df)/T from df>0. Checked dispatch rejects zero fixings; the raw arithmetic count function instead returns 0 for n=0. Raw geometric n=0 is continuous averaging. Expiry shortcut precedes fixing-list/df validation in explicit-time functions. Arithmetic output clamps to [0,df m1] for calls or [0,df K] for puts.

## Observed conventions

Annual decimal volatility/rates and model-year fixing times; each fixing has equal weight and payment discount is supplied/derived at expiry. No historical fixings or partial accumulated sum are inputs.

## Observed limitations, failure and fallback

Explicit-time validation allows future observations beyond payment expiry.
Equal model times retain separate equal-weight slots. First and second moment
agreement leaves arithmetic tail prices unqualified.

## Alternatives and tradeoffs

Geometric is an implemented alternative. Monte Carlo, Curran/Levy approximations, and explicit seasoned-average formulas would have different costs and domains; these are not automatic substitutes in this dispatch.

## Implementation and validation evidence

[asian.rs](../../../../finstack-quant/models/src/closed_form/asian.rs) and [dispatch.rs](../../../../finstack-quant/models/src/closed_form/dispatch.rs). `test_second_moment_on_algorithm_matches_brute_force` checks O(n) versus double-sum moments; `test_geometric_continuous_limit_matches_discrete_and_kemna_vorst` checks the limiting formula. Arithmetic positivity/parity checks are weaker than independent pricing evidence. References: [Kemna–Vorst](../../../REFERENCES.md#kemna-vorst-1990), [Turnbull–Wakeman](../../../REFERENCES.md#turnbull-wakeman-1991).

The [equity Asian pricer](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs)
uses dated expectations in the nonpositive effective-strike branch and payment-implied
flat carry in the other arithmetic branch. Its geometric analytic and MC-control
paths already carry seasoned state and dated drift. The
[commodity Asian pricer](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_asian_option/pricer.rs)
uses dated forward levels and separate moment implementations.
[Python](../../../../finstack-quant-py/src/bindings/models/closed_form.rs) and
[WASM](../../../../finstack-quant-wasm/src/api/models/closed_form.rs) expose the
count-based dispatch with optional averaging today.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
