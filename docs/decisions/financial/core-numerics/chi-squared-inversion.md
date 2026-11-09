# CORE-NUMERICS-15: Chi-squared quantiles through logarithmic tail inversion

- Decision status: **Accepted — complete chi-squared-inversion package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded/exact references and read-only kernel/caller reviews were completed on 2026-11-08; these do not certify the production quantile or a copula model.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/distributions.rs::chi_squared_quantile`: the central chi-squared law with real positive degrees of freedom, dimensionless inputs/output, and Gamma(df/2,scale2) relation. The accepted contract is below; the current runtime endpoint, numerical ceiling, rounding and residual behavior is retained separately as observed evidence.

## Accepted decision: checked chi-squared log-tail inversion

Accepted on **2026-11-08** after whole-record review. Runtime,
caller reconciliation and independent numerical/statistical/financial qualification
remain pending; source-observed behavior is retained below.

1. **Retain one checked central-law quantile.** Keep
   chi_squared_quantile(p,df)->Result<f64>, with F(x)=P(a,x/2), a=df/2 and P/Q
   regularized lower/upper incomplete Gamma. df is a dimensionless real shape
   parameter, not an integer observation count or an inferred financial degree
   of freedom. No Option, public distribution/CDF class, state, method selector,
   noncentral law, generalized scale or inverse approximation family.

2. **Adopt strict closed probability endpoints.** Require finite p in[0,1] and
   finite df>0 before any shortcut. Exact p0 returns canonical+0 and p1 returns
   +infinity. NaN, infinities/outside probabilities and invalid df are input
   errors rather than saturated limits. Endpoint identities do not evaluate
   incomplete Gamma or the interior numerical ceiling. No confidence default,
   epsilon probability snap or owner-open-uniform policy is inferred.

3. **Keep an explicit interior evaluation domain and positive-output rule.**
   For0<p<1 retain df<=1e8 as the fixed numerical profile, not a statistical
   restriction, approximation switch or proof of bounded work/accuracy. A larger
   interior df is a supported-domain error. Require positive finite returned q;
   no interior0 or infinity. If df/2 cannot be represented as a positive shape,
   fail computation instead of silently replacing the distribution by shape0.
   Remove the half-subnormal zero-return and nearest-rounded-quantile claims.
   Valid mathematical inputs can fail when this numerical profile cannot resolve
   their requested returned-value tail criterion.

4. **Use direct private logarithmic Gamma tails with three fixed regions.**
   Carry log(z)=log(x)-log2, including virtual positive z=x/2 below the smallest
   representable value; an underflowed materialized z is not a zero CDF identity.
   For a<=1,z<=1 use the preserved correction series
   P(a,z)=exp(a*log(z)-logGamma1p(a))*(1+a*S),
   S=sum_(k>=1)((-z)^k/(k!*(a+k))). Evaluate V=S/z with first term
   v1=-1/(a+1) and successive ratio -z*(a+k)/((k+1)*(a+k+1)); use compensated
   V and a relative-term test against V before restoring the deferred correction
   a*z*V inside log1p. This remains defined when virtual positive z materializes0:
   a negligible restored correction may round away, without a0/0 stopping test
   or a claim that z or the CDF is mathematically0.
   For a>1,z<=a use the positive lower series T0=1,
   Tk=Tk-1*z/(a+k), with logP=a*logz-z-logGamma1p(a)+log(sum T).
   Otherwise use the existing pinned-statrs upper continued-fraction recurrence
   with checked power-of-two rescaling, returning logQ from its positive ratio
   and log prefactor. These are same-function regional evaluations, not model
   approximations or a fallback chosen after failure. Keep helpers private and
   remove the old unchecked shifted-mass/dependency route.

5. **Bound every iterative tail evaluation.** Each selected series or fraction
   has at most100,000 updates and a fixed1e-15 operational stopping proxy: absolute
   last-term/absolute accumulated-sum for the scaled V correction series, positive
   last-term/sum for the lower series, or relative change of positive successive
   continued-fraction ratios. Test on the final permitted update; require valid
   finite required state and stable counters/progress. Use scaled comparisons,
   compensated sums and rescaling before overflow, not raw squared/underflowed
   ratios. Exhaustion, stagnation before the criterion, invalid denominators/
   ratios or unresolved required arithmetic is computation/convergence failure.
   The cap and proxy do not establish a true truncation/rounding bound. No
   unbounded dependency loop, returned last iterate or near-zero shape/x cutoff.

6. **Preserve the small-shape Gamma increment.** For0<a<=1/8 evaluate private
   logGamma1p(a)=a*(-EulerGamma+sum_(k=2..16)((-1)^k*zeta(k)*a^(k-1)/k)),
   using fixed independently verified nearest-binary64 coefficients and checked
   polynomial arithmetic. The omitted exact-series tail is below6e-17 at1/8,
   and its magnitude divided by a is below5e-16 throughout the region; these
   control truncation only. For larger a where a<16, use the accepted12 checked
   positive-real log-Gamma at1+a. Do not first round1+a to1 for a tiny increment,
   or interpret the resulting zero as exact normalization. No public logGamma1p
   API, coefficient-generation framework or all-range precision guarantee.

7. **Avoid large-shape prefactor cancellation.** For a>=16 select the five-term
   Stirling correction
   C=1/(12a)-1/(360a³)+1/(1260a⁵)-1/(1680a⁷)+1/(1188a⁹),
   using inverse powers. Its next-term bound691/(360360*a¹¹) is below1.1e-16
   at16. Let r=(z-a)/a where that geometry is preserved. The lower-series
   log prefactor is a*log1pmx(r)-.5*(log(2*pi)+log(a))-C; the upper fraction's
   prefactor adds log(a). For |r|<=.5 use
   log1pmx(r)=-r²*sum_(j=0..63)((-r)^j/(j+2)) with checked compensated summation
   and final scaling. Outside this region retain log(z)-log(a) and z-a directly
   in the equivalent expression; do not use log1p of a ratio rounded to-1.
   These same-function normalization profiles avoid subtracting separate O(a*log a)
   quantities to obtain an O(1) tail. Truncation bounds do not certify total log
   error, and an in-range result alone is not adequate arithmetic evidence.

8. **Compute requested tails without clipping or erased complements.** Combine
   logarithmic prefactors/results directly, without exponentiating a tiny mass
   merely to take its logarithm. When a complement is needed, use the preserved
   finite L=log(tail)<=0 with stable log1mexp: log(-expm1(L)) near0 and
   log1p(-exp(L)) away from0 (switch at-log2). A complement lost because L rounded
   to0 cannot be recovered by expm1: error if that requested tail is unresolved.
   Legitimate final log of a probability near1 may round to0 when its lost
   complement is not requested. Positive computed log mass, NaN, undeclared
   infinite log tail or invalid required correction/fraction state errors; no
   min(logmass,0), probability floor, silent saturation or swallowed dependency
   error. General normal/Student-t definitions in accepted12 remain unchanged.

9. **Keep a finite analytic bracket and explicit owner controls.** Let eta be
   the smallest positive binary64 value. Use log-coordinate bounds log(eta) and
   log(U), U=df+64*sqrt(df)+128, formed with checked conservative geometry.
   Gamma-MGF Chernoff gives Pr[X>=df+16*sqrt(df)+128]<=exp(-64)<2^-53, which is smaller
   than every available1-p for interior binary64 p. The wider U is therefore
   a conservative real-arithmetic bracket for fractional as well as integer df;
   this derivation does not certify rounded evaluator signs. Evaluate endpoints
   fallibly and require actual finite bracketing signs. If the lower endpoint
   already lies at/above the root, eta is a candidate only if its returned-value
   gate below passes; otherwise fail range/resolution. No below-eta/half-eta
   search, domain expansion, guess, Wilson–Hilferty or normal approximation.

10. **Reuse accepted05 Brent with truthful callbacks/work.** Use the one
    canonical fallible Brent kernel with explicit log-coordinate tolerance1e-13
    and max_iterations256, without generic solver defaults or private copies.
    For p<=.5 residual is logP(a,x/2)-log(p); otherwise it is
    log(1-p)-logQ(a,x/2), using log1p(-p) for the supplied upper probability.
    Both residuals increase with x. Propagate the first tail error and require
    finite residuals; remove the +/-1000 clip and every NaN/error-to-value adapter.
    Solver endpoint/coordinate termination is not probability acceptance.
    Count actual solver and kernel work; the finite composition of their bounds
    is not a wall-clock promise or a new public diagnostics/options surface.

11. **Gate the actual restored output, not a latent log root.** Restore q from
    the accepted log-coordinate candidate; require positive finite q and recompute
    the same requested log-tail residual at log(actual returned q). Require its
    finite absolute magnitude<=1e-8, including an eta boundary candidate. This
    is a fixed numerical admission rule distinct from the solver's coordinate
    tolerance, true tail accuracy and quantile-relative error. If binary64
    quantization cannot satisfy it, return computation/range failure rather than
    zero, a forced nearest value, a relaxed gate or a latent-root-only success.
    Do not promise nearest-subnormal rounding, universal returned-tail accuracy,
    correct rounding or success for every finite input in the numerical domain.

12. **Keep error categories and output claims truthful.** Invalid probability,
    df and unsupported interior df domain are input errors. Tail/normalization,
    transformed-shape, work/progress, bracket, solve, restoration and returned-
    gate failures are computation/convergence errors with stage/p/df context.
    Exact p1 is the only successful infinity; exact p0 the only successful zero.
    No panic, successful NaN, private catch/fallback, mean approximation or
    partial result. Preserve independent kernel/cell/error qualification rather
    than claiming dependency reuse or the1e8 cap establishes production accuracy.

13. **Preserve Rust-only scope and actual model ownership.** Keep the existing
    scalar core API intentionally unbound in Python/WASM; no new class, schema,
    host/default/seed or generated surface. The actual Student-t copula caller
    supplies an open uniform, computes W=chi_squared_quantile(U,nu)/nu and requires
    finite positive W, propagating failures. Its nu>2, normalization, shared-factor
    timing, RNG order/conditioning and portfolio/default meanings remain owner
    policy; no redraw, W1 or Gaussian replacement. This is a central law, not
    an exact noncentral CIR transition; current CIR QE discretization is separate.

14. **Qualify kernels, returned cells and consumers independently.** Cover
    df2 exponential/df4 closed-form tails, independently bounded fractional and
    large-df references, small-shape/normalizer/prefactor/region seams, virtual
    half arguments, upper-near-one probabilities, subnormal q and impossible
    returned gates, all endpoints/invalid parameters, cap/work/progress/error
    boundaries and actual copula propagation. Distinguish shared-statrs checks,
    shared-log-Gamma quadrature, inverse roundtrips and latent-root residuals
    from independent references and returned-output acceptance. This review
    changes no runtime algorithm, tests, fixtures, bindings or dependencies.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Invalid p/df, including endpoint requests | Input error before identities; no saturation. |
| p0 /p1 with finite df>0 | +0 /+infinity without interior Gamma evaluation or df ceiling. |
| Interior df>1e8 | Supported-domain error, not an approximation switch. |
| Interior df/2 rounds0 or no positive q can pass the gate | Computation/range error, no interior zero. |
| df2,p.5 /p=1-2^-53 | References2log2 /106log2; returned positive q must pass its own gate. |
| df2,p=eta | Reference quantile just above2eta; positive2eta can satisfy the selected gate. |
| df4,p=eta | Reference about6.28691113881e-162, preserving the log lower tail. |
| df1,p=2^-537 | True quantile about(pi/2)eta rounds2eta, but its returned log-tail residual about.120782 violates1e-8: computation/resolution error. |
| df1,p=2^-538 | True quantile beloweta/2; no selected interior0 success. |
| Tiny a with1+a rounding1 | Preserve logGamma1p's original increment or error; never assume zero normalization. |
| Large a, z near a | Use stable prefactors; a finite result from cancellation is not evidence of accuracy. |
| Lost requested complement or positive computed log mass | Computation error, no clamp/expm1 recovery claim. |
| Kernel/solver budget or returned-output gate fails | Computation/convergence error without normal/mean/redraw fallback. |
| Student-t mixing caller | Preserve positive finite W/error propagation and owner open-uniform/nu policy. |

## Observed choice and rationale

Solve for log(x), comparing log lower-tail mass for p≤0.5 and log survival for the upper tail. This avoids absolute probability tolerances that would erase tiny tails and avoids negative iterates. The numerical ceiling is an implementation domain restriction, not a financial distribution choice.

## Observed algorithm

1. Validate p and df; p=0 returns zero. Set a=df/2; if rounding makes a=0, return zero. Compute log Gamma(a+1), requiring finiteness.
2. With y=x/2, evaluate log lower CDF using the recurrence `P(a,y)=exp(a log(y)−y−logGamma(a+1))+P(a+1,y)`. Combine the two positive terms by log-sum-exp; use the first term alone when shifted mass underflows, and cap the log result at 0.
3. Lower-tail residual is `log P(a,x/2)−log(p)`. Upper-tail residual is `log(1−p)−log Q(a,x/2)`; for y≤1 compute survival by `−expm1(log P)`, otherwise use statrs' regularized upper Gamma. Clamp residual to [-1000,1000].
4. Lower bracket is `log(min_subnormal)−log(2)`. If residual there is nonnegative, return zero as rounded-underflow output. Upper bracket is `log(df+64 sqrt(df)+128)`.
5. Brent solves in log-space with x tolerance 1e-13 and max 256 iterations. Exponentiate; require finite result and finite final log-tail residual≤1e-8 in magnitude at the latent log root, not recomputed at the restored output. Solver failures are wrapped as validation errors.

## Observed selection and controls

All controls are internal and fixed. No inverse-CDF approximation is selected by df. The shared [Brent record](brent-root.md) governs its bracket-width stopping, but this caller adds a residual postcheck.

## Observed limitations, failure and fallback

p=1 and NaN/out-of-range p error. Very small quantiles may return zero; this is explicit representability handling, not an atom at zero in the continuous distribution. A valid mathematical quantile outside the guarded numerical domain errors. Accuracy still depends on statrs incomplete-Gamma evaluation and finite log residual clamping.

## Observed alternatives and tradeoffs

Direct x-space inversion, Wilson–Hilferty approximations and dependency inverse CDFs are not used. Log inversion costs repeated special-function evaluations but preserves relative tail information. It cannot express probabilities closer to one than f64 can represent.

## Evidence

- **Observed source and located tests.** [Implementation and inline tests](../../../../finstack-quant/core/src/math/distributions.rs) select the current open upper endpoint, df ceiling, shifted lower-tail correction, clipped residual and latent-log-root postcheck. `chi_squared_quantile_matches_exponential_closed_form_in_both_tails` uses df2, x=-2log1p(-p),1e-11 relative tolerance down to p1e-300. df4 lower-tail tests use a separately coded Taylor series (1e-10 relative mass). Fractional-df tests use transformed midpoint quadrature but share log-Gamma; upper-tail checks share statrs. These roles are distinct. Tests were inspected/located, not executed, and do not validate the accepted closed endpoint, bounded kernels, no-interior-zero or restored-output acceptance rules.
- **Primary definitions and fixed evaluation profiles.** [NIST special-function references](../../../REFERENCES.md#nist-special-functions) identify the chi-squared Gamma relation, positive/alternating lower series, upper continued fraction, log-Gamma increment series and Stirling remainder. The three regions,16/64-term profiles,100,000 update cap,1e-15 stopping proxies and1e8 interior ceiling are selected project evaluation policies; the references do not certify their floating accuracy. [Pinned statrs Gamma source](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/function/gamma.rs) has unbounded loops, approximate-zero branches, ordinary complement subtraction and prefactor-underflow shortcuts, justifying the narrow private checked logarithmic adaptation. Preserve actual source/license provenance; no implementation is copied into runtime in this review.
- **Bracket derivation.** [Laurent–Massart2000](../../../REFERENCES.md#laurent-massart-2000-chi-squared) gives the primary integer-dimension concentration reference. Extending its upper bound to real df>0 here follows directly from the Gamma MGF, not from a claim that the paper stated the fractional case: Pr[X>=df+2sqrt(df*t)+2t]<=exp(-t). At t64 this is below2^-53, and the current coefficient64 gives a wider bracket than16sqrt(df)+128. This real-arithmetic argument motivates the bracket; fallible endpoint/sign checks and numerical qualification remain necessary. [Accepted05 Brent](brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search) and the [Brent reference](../../../REFERENCES.md#brent-1973) own solver behavior separately from this returned-quantile gate.
- **Independent bounded closed-form references.** On2026-11-08, exact-rational logarithm/alternating-series bounds and high-precision Decimal calculations, independently of production/statrs, gave df2 quantiles2.00000000000000005011818367042e-300 at float64(1e-300),2.00000000000099995977329592514e-12 at float64(1e-12),2log2 at.5, and106log2=73.4736011393542027982266048746 at1-2^-53. At p=eta the true df2 quantile is just above2eta. For df4, exact-rational CDF bounds with quantile interval width below1e-59 relative gave6.28691113881051475563806269122e-162 at eta,2.82842712474619013304228498252e-150 at float64(1e-300), and2.82842845808038764475059602170e-6 at float64(1e-12). These are bounded reference examples, not production runs, new committed fixtures or broad-range certificates.
- **Returned-grid limitation.** For df1,p=2^-537 the true quantile is about(pi/2)*eta and rounds to2eta, whose returned smaller-tail log residual is about.12078223763524522235. At p=2^-538 the true quantile lies beloweta/2. Thus latent-root convergence and even nearest rounding cannot imply the selected1e-8 returned-output gate; these inputs must error under this accepted contract. The mathematical proof that df=eta has all interior quantiles beloweta/2 explains the old shortcut but does not select zero as a successful interior output. The accepted contract deliberately chooses a stricter numerical result contract without a rounding-cell framework.
- **Normalization/range targets.** Direct lnGamma(round(1+a)) can lose an entire small-a increment; for df4e-19 and p=nextDown(1), omitting -EulerGamma*a can move an otherwise representable quantile by a factor near exp(EulerGamma). At large a, raw a*logz-z-lnGamma terms cancel quantities near1e9, so a finite answer alone proves no1e-8 log-tail accuracy. Independent rational truncation bounds support the selected Gamma1p and five-term Stirling margins; coefficient tables, floating seam/prefactor errors and returned-tail acceptance still require production qualification.
- **Actual Rust-only consumer.** The [parity contract](../../../../finstack-quant-py/parity_contract.toml) intentionally leaves distributions Rust-only. [Student-t copula mixing](../../../../finstack-quant/models/src/correlation/copula/student_t.rs) supplies an open uniform, propagates core errors, divides the quantile by nu and requires positive finite W. [Portfolio loss](../../../../finstack-quant/models/src/correlation/portfolio_loss.rs) and [per-name defaults](../../../../finstack-quant/models/src/credit/pool/default/per_name.rs) preserve the failure path; no redraw, W1 or Gaussian replacement is observed. The copula's nu>2 and draw/correlation conventions remain separately owned. [Current CIR QE](../../../../finstack-quant/models/src/monte_carlo/discretization/qe_cir.rs) does not call this central quantile; the helper's broad CIR example is not evidence of an exact transition implementation.
- **Qualification limit.** Read-only independent mathematical/kernel/caller reviews assessed the complete package. No Rust suite, binding build, sampler, generator or financial experiment ran. Formula/bound examples, shared dependency checks, inverse consistency, computational admission and financial-model fitness remain distinct.

Tests do not exhaust the continuous df/probability domain, even where a special-case reference is independent.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
