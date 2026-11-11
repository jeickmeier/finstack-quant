# MODELS-ANALYTIC-18: Raw SVI evaluation, bounded transformations and fit acceptance

**Decision status:** Accepted (complete package adopted 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent scalar and exact-arithmetic examples support the proposed distinctions. Production, market and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked raw SVI and slice fit acceptance

All 26 decisions were accepted on **2026-11-10**. Implementation reconciliation
and independent complete-path qualification remain open.

1. **Select raw SVI for one expiry.** Use total Black variance
   `w(k) = a + b (rho x + R)`, with `k = log(K/F)`, `x = k-m`
   and `R = sqrt(x^2+s^2)`. The existing wire field `sigma` is the
   positive width s in log-moneyness. The annual Black quote is
   `sqrt(w/T)`. Each fit has one explicit positive forward and
   expiry. The dated source owner qualifies their basis and clock.

2. **Use one immutable validated parameter state.** Keep finite
   a, b, rho, m and s, with b≥0, s>0 and −1<rho<1. Route Rust
   construction and strict state restoration through the same
   checks. Retain the five named fields and reject missing, unknown
   and duplicate fields. Remove mutable and unchecked construction.

3. **Check the exact global minimum.** For b>0 let
   `q = sqrt((1-rho)(1+rho))`. The minimum is
   `w_min = a + b s q` at `k = m-rho s/q`. Require w_min≥0.
   For negative a, compare a² with the complete
   `b² s² (1-rho)(1+rho)` on represented inputs without tolerance
   slack or overflowing squares. b=0 requires a≥0 and gives
   constant variance a at every finite k. The minimum can be zero.

4. **Apply the asymmetric wing boundaries.** Require left slope
   `b(1-rho) ≤ 2` and right slope `b(1+rho) < 2`, comparing
   the complete expressions on represented inputs. For raw SVI
   right equality gives d₊→0 and a unit forward call tending to 0.5.
   The strict right condition gives the required vanishing call
   wing. Left equality remains admissible for raw evaluation and
   can allow an atom at zero in an otherwise valid price law.
   These are shape and tail rules.

5. **Use checked original log-forward moneyness.** A numeric k
   must be finite. A strike-based caller requires finite positive
   K and F and computes the same stable log ratio without first
   dividing K by F. Preserve close-level geometry and reject
   unresolved coordinates. Absolute strike, log-moneyness and the
   offset x each keep their own meaning.

6. **Evaluate total variance in stable complete form.** Use scaled
   differences and hypot geometry, including a potentially
   overflowing raw x or R. Near the minimum set `L = R+rho x`
   and use
   `w = w_min + b (x+rho R)^2 / (L+s q)`.
   Preserve the complete minimum margin and the squared deviation
   before final rounding. Stable evaluation can use the equivalent
   noncancelling expression for L from the wing slope and
   `s²/(R+|x|)`. A genuine zero returns zero. A lost positive
   or nonfinite requested variance is a computation error.

7. **Evaluate annual implied volatility directly from the scaled law.**
   Validate finite T>0. Return sqrt(w/T) with complete-result scaling,
   even if w or w/T by itself would be outside binary64 range. Preserve
   exact zero variance and the constant b=0 branch. Use no expiry
   epsilon or variance floor. Range checks apply to the requested
   output.

8. **Use the analytic derivatives in k.** Use
   `w' = b (rho + x/R)` and `w'' = b s²/R³` with stable signed
   differences and scaled ratios, preserving cancellation near the
   minimum and small curvature in the wings. The derivative geometry
   uses x. The density formula uses the original k.

9. **Return a checked local Durrleman factor.** For w>0 use
   `g = (1-k w'/(2w))² - (w'²/4)(1/w+1/4) + w''/2`.
   Keep scales through the complete signed result, including large
   cancelling terms. Propagate arithmetic failure. The factor is
   dimensionless and its sign agrees with the implied call price's
   second strike derivative. Flat positive variance has g=1. At w=0
   the formula is undefined and the checked operation errors.

10. **Use an explicit checked finite scan.** Supply at least two finite,
    strictly increasing represented log-moneyness points and a finite
    nonnegative dimensionless tolerance. Evaluate each point in order
    and report the checked minimum g, its coordinate and sample count.
    Report a breach when that computed minimum is below minus the
    tolerance. On ties select the first point.
    Reject invalid controls and propagate every undefined or failed g.
    Remove the invalid-grid-to-success behavior and implicit grid creation.

11. **Limit density claims to their domain.** A valid scan reports
    only its sampled coordinates and tolerance. Continuous butterfly
    freedom for the positive-variance slice requires g≥0 everywhere
    together with the right-tail condition. Calendar freedom across
    expiries and diagnostics for a sampled strike grid have their own
    owners. Keep the standalone scan Rust-only.

12. **Admit complete single-expiry fit observations.** Require at least
    five paired finite positive strikes and annual Black vols, with
    explicit finite positive F and T. Sort complete pairs by strike for
    the solver, retaining quote identity and original order for errors.
    Reject duplicate strikes and distinct strikes that collapse to the
    same represented log-moneyness. Five observations do not establish
    parameter identification.

13. **Retain the unweighted total-variance objective.** Compute each
    positive representable market target `w_i = T sigma_i²` with
    complete scaling. Minimize the norm of `w(k_i)-w_i` in total
    variance units. Use the checked model law and stable residual
    geometry. Every quote contributes once. The native volatility
    acceptance below is independent of the objective's units.

14. **Require explicit fit controls and one interior seed.** Validate a
    supplied raw seed with positive minimum variance and two strictly
    positive slopes below 2. Require the accepted LM residual and step
    tolerances and attempt limit, an absolute annual-volatility error
    cap, and the explicit density scan and tolerance. LM residual
    tolerance uses the total-variance norm and step tolerance uses
    Euclidean transformed coordinates. The vol cap is finite and
    nonnegative. Validate the scan before solving and require its
    endpoints to cover the original quote coordinates. Remove fixed ATM
    heuristics, solver defaults and the alternate solver entry point.

15. **Transform into a smooth interior for LM.** Map coordinates to
    `w_min = exp(u)`, `s = exp(v)`, `L = 2 logistic(l)`
    and `H = 2 logistic(h)` for the left and right slopes, with m
    free. Reconstruct `b=(L+H)/2`, `rho=(H-L)/(H+L)` and
    `a=w_min-b s sqrt((1-rho)(1+rho))`. Encode the seed with the
    matching log and logit coordinates. Keep complete scales and verify
    the actual represented raw tuple retains strictly positive minimum
    variance and both slopes in (0,2). Reject collapsed or saturated
    boundaries.

16. **Use the accepted checked Levenberg–Marquardt solve.** Use one
    caller-seeded solve with the central numerical Jacobian and
    augmented pivoted QR from [CORE-NUMERICS-06](../core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes).
    Validate each transformed
    callback candidate and all residuals, including perturbed coordinates.
    Propagate hard callback, range and solver failures. Apply the
    actual attempt budget and check shape, allocation and work counts.

17. **Recognize exact equal-volatility observations.** After validating
    the full request, if all input sigmas are equal use the analytic
    constant slice with a equal to the checked market total variance,
    b=rho=m=0 and s=1. Report the analytic path and zero LM attempts.
    Apply the same quote and density acceptance. Unequal sigmas use
    the selected LM path.

18. **Judge the final represented candidate.** Rebuild the candidate
    through canonical parameter checks and recompute the actual
    objective. A finite candidate from a step, damping or attempt
    termination remains eligible for the two acceptance checks.
    Preserve its literal status and work. A termination flag alone
    does not determine financial acceptance.

19. **Require exact-unit quote replay.** Re-evaluate annual Black sigma
    from the returned parameters at every original observation with the
    original forward and expiry. Require every absolute sigma error
    to be at most the supplied cap. Report signed errors in input
    order, maximum absolute error and stable annual-vol RMSE. Remove
    the variance-derived approximate RMSE gate and its near-zero
    fallback.

20. **Require the finite density gate before returning a fit.** The
    prevalidated scan covers all observed log-moneyness. Scan the exact
    union of supplied points and all original quote
    coordinates using the returned candidate. Require all checked g
    values to be at least minus the declared tolerance. Propagate
    invalid geometry or a failed scan as an error. A sampled breach
    rejects the fit even when every quote error is within budget.

21. **Return the actual fit result and evidence.** Return the immutable
    parameters with forward, expiry, supplied controls and seed,
    literal solve or analytic status, actual work, objective norm,
    native quote errors and finite scan result. Preserve the sampled
    support and tolerance for replay. Retain causes and failed
    acceptance diagnostics through existing error channels where
    available. The fit result describes a bounded local solve.

22. **Keep dated equity orchestration at its owner.** Reconcile the
    existing per-expiry equity SVI target with this checked model fit.
    Carry its actual solver, quote and scan outcomes in fitted-slice
    reports. Replace the duplicate scalar density loop with the checked
    scan when the target elects its own additional diagnostics.
    CALIBRATION-TARGETS-009/012 own dated forwards, clocks and
    per-expiry orchestration.

23. **Keep sampled-surface acceptance separate.** Materialization
    evaluates native w at each original slice's own forward and
    checked log-moneyness. Use the published strike grid to replay
    original live quotes under the target's distinct vol tolerance
    and publication policy. CALIBRATION-TARGETS-010/012 and
    CALIBRATION-FRAMEWORK-017 own cross-expiry interpolation, tails,
    validation disposition and publication. The model fit grants
    acceptance at the replayed quotes and evaluated scan points.

24. **Reconcile actual hosts and state.** Update Python and WASM
    scalar evaluation, checked constructor/restoration and existing
    fit entry points and results. Carry fitted-slice reports through
    the calibration schema and its UI consumers. Align stubs,
    declarations, facade, parity and examples. Rust owns the
    calculations.

25. **Qualify the actual limits and error semantics.** Distinguish
    invalid shape, coordinates and controls from computation and
    failed financial acceptance. A finite scan and a local fit carry
    no continuous, calendar or materialized-surface certificate.
    Preserve separate downstream validation and pricing eligibility.

26. **Require independent and complete-path evidence.** Cover raw
    parameter and tail boundaries, stable minimum, annualization
    range, analytic derivative and density, sampled misses,
    represented fit and seed boundaries, native quote error at
    different expiries, scan rejection, actual work and published
    surface replay. Qualify solver, pricing and host behavior
    independently. See the
    [raw SVI references](../../../REFERENCES.md#raw-svi-admission-density-and-calibration).

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| T=0.01, observed sigma 0.20 and model sigma 0.21 | Total variances are 0.000400 and 0.000441. The current proxy gives 0.001025 while actual annual sigma error is 0.01. |
| Raw a=0, b=1, rho=m=0, s=0.05 | Minimum variance is 0.05 and both wing slopes are 1. Exact-decimal inputs give g(0.12)=−22662/28561≈−0.79345961. A scan at 0 and 10 passes while missing that violation. |
| Raw a=−1, b=s=1, rho=m=0, k=2⁻⁴⁰ | The stable positive variance rounds to 2⁻⁸¹. Direct subtraction of the rounded square root and 1 gives zero. |
| Flat w=1e300, T=1e−300 | Annual sigma is approximately 1e300, although w/T overflows binary64. |
| a=s=T=binary64 MAX, b=1, rho=m=k=0 | Standalone w exceeds binary64 range. Annual sigma is √2. |
| b=1 and rho=nextDown(1) | Exact right slope is 2−2⁻⁵³. Direct binary64 addition rounds 1+rho to 2. |
| T=1, F=100, raw (a,b,rho,m,s)=(0.04,0.4,−0.4,0,0.2), sampled at K=80 and 120 | Source ATM sigma≈0.34641016. The sampled TotalVariance grid returns ≈0.39663956 at K=100. |

These standalone checks exercise the formula distinctions and an illustrative
published-grid bracket. Production fit, pricing and host paths remain pending.

## Observed implementation and evidence

### Scope and input domain

Single-expiry raw SVI w(k)=a+b[ρ(k−m)+√((k−m)²+s²)], k=ln(K/F); ≥5 finite positive strike/vol quotes for calibration.

### Choice and rationale

Fit unweighted total-variance residuals by one-start LM, then validate basic parameter/moment bounds. Butterfly diagnosis exists separately and is not required by the fitter.

### Algorithm

Validation requires b≥0,s>0,|ρ|<1, finite parameters, minimum variance a+bs√(1−ρ²)≥−1e−14 and Lee slope b(1+|ρ|)≤2+1e−12. IV=√(w/T), requiring finite k,T>0,w≥0. Density g=(1−kw′/(2w))²−(w′²/4)(1/w+1/4)+w″/2; w≤0 returns −∞. Uniform requested-grid scan reports worst g<−tol; invalid scan with n<2 or nonincreasing limits returns None. Fit transforms b=exp(x),ρ=tanh(x),s=exp(x), leaves a,m free. Seed a=nearest-ATM variance,b=.1,ρ=m=0,s=.1. After LM, validate parameters and reject approximate quote error above.005, calculated as RMSE(w)/(2√mean(w)) (or RMSE(w) near zero).

### Selection and controls

Default LM tolerance1e−12 and300 iterations; alternate solver accepted explicitly. No multistart or hard butterfly/calendar constraints. Returned iteration count is solver count. Acceptance does not inspect termination reason separately from parameter/RMSE checks.

### Conventions

Raw w is total variance, not annual variance; s is width in log-moneyness. Observed approximate “vol RMSE” omits √T in denominator, so it estimates standard-deviation error rather than annualized-vol error.

### Limitations, failure and fallback

Minimum-variance and Lee bounds are not sufficient for butterfly freedom. Calibration can return an interior butterfly violation because g is not called. Fixed .005 acceptance depends on maturity in annual-vol terms. Grid scan can report no violation for an invalid/too-small scan.

### Alternatives and tradeoffs

Constrained SSVI or arbitrage-penalized multistart are unimplemented alternatives. Existing analytic density scan is diagnostic, not surface calibration.

### Implementation and validation evidence

[svi.rs](../../../../finstack-quant/models/src/volatility/svi.rs). Synthetic fit recovery/noisy-fit rejection tests establish objective behavior; `durrleman_g_matches_finite_difference_density` checks the derivative formula. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) uses50-digit mpmath formula values, not independently calibrated market parameters. References: [SVI](../../../REFERENCES.md#gatheral-2004-svi), [SVI arbitrage](../../../REFERENCES.md#gatheral-jacquier-2014-svi).

The equity target fits each expiry, separately scans a fixed density grid
when enabled, and replays the original quotes on the published strike
grid. Its current materializer converts raw SVI to annual sigma before
squaring back to total variance. The host fit returns only parameters.
See the [equity target](../../../../finstack-quant/calibration/src/targets/svi.rs),
[fitted-slice reports](../../../../finstack-quant/calibration/src/report.rs),
[Python binding](../../../../finstack-quant-py/src/bindings/models/volatility.rs)
and [WASM binding](../../../../finstack-quant-wasm/src/api/models/volatility.rs).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
