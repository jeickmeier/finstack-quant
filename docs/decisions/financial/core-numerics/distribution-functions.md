# CORE-NUMERICS-12: Normal and Student-t functions and logarithmic Gaussian tails

- Decision status: **Accepted — complete checked special-function/distribution package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded high-precision reference calculations and read-only source/host/caller reviews were completed on 2026-11-08; these do not certify the production implementation or a financial model.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/special_functions.rs`: error function, log-Gamma, standard/general normal density/CDF/inverse, log normal CDF, and centered unit-scale Student-t CDF/inverse. The code supplies scalar functions, not a copula or financial model.

## Accepted decision: checked special functions and distribution domains

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host reconciliation and independent numerical/statistical/financial
qualification remain pending; source-observed behavior is retained below.

1. **Select one checked scalar surface for all ten functions.** Retain erf,
   ln_gamma, norm_cdf, norm_pdf, log_norm_cdf, standard_normal_inv_cdf,
   norm_cdf_with_params, norm_pdf_with_params, student_t_cdf and student_t_inv_cdf
   as canonical `Result<f64>` functions. No Option availability, parallel unchecked/
   NaN family, distribution-object hierarchy, controls class or host-only
   computation. The later scoped Rust beta-quantile entry is specified in the
   [accepted amendment](#accepted-amendment-checked-rust-beta-quantile).
   Use existing statrs algorithms where they meet the
   selected contract; the narrowly checked Student-t adaptation below addresses
   verified missing failure/work semantics rather than adding another method.

2. **Separate invalid inputs from legitimate extended-real variates.** Reject
   NaN in every input. Forward erf/normal/Student-t evaluation admits variate
   x=+/-infinity with explicit mathematical limits: erf=+/-1, CDF=0/1, PDF=0,
   log_norm_cdf=-infinity/0. Finite location and finite positive scale/df inputs
   are validated before any endpoint, median or other identity return. Genuine
   identities include erf(0)=0, standard/centered CDF(0)=.5 and quantile(.5)=0.
   Signed zero may be canonicalized where it has no distinct mathematical role.

3. **Use a closed, strict inverse-probability domain.** Require finite p in[0,1]
   for normal/Student-t inverses. Exact p0/p1 maps to -/+infinity after complete
   parameter validation. Outside probabilities, infinities and NaN are input
   errors, not saturated limits. Interior probabilities require a finite quantile;
   nonfinite reconstruction or a fabricated median at p!=.5 is a computation/
   range error. No epsilon probability clamp, endpoint nudge or application
   confidence restriction such as p>.5. Normal inversion retains the existing
   Boost-derived erfc_inv profile, not an AS241 or exact-roundtrip claim.

4. **Define erf and positive-real log-Gamma explicitly.** Erf retains its existing
   kernel, with finite result in[-1,1]; finite arguments may round to endpoint
   values. ln_gamma means real ln(Gamma(x)) for finite x>0 only: zero, negatives
   and infinities error, without log-absolute/complex continuation. The result can
   be negative on(1,2); retain exact zeros at1,2. Use same-function positive
   recurrence lnGamma(x)=lnGamma(1+x)-ln(x) on0<x<.5 to avoid tiny sine/reflection
   geometry, keeping a safely shifted argument where needed. Overflow of the
   requested log-Gamma is a computation/range failure, not an infinity sentinel.

5. **Retain standard-normal density/CDF without false strict-range claims.**
   Phi is the standard normal CDF and phi=exp(-x²/2)/sqrt(2*pi). Keep the existing
   normal-CDF dependency and direct density profile with checked arithmetic.
   CDF lies in[0,1], PDF is finite nonnegative, and ordinary final binary64 tail
   underflow/rounding to0/1 is allowed. Erf endpoints have the same representation
   distinction. Such values do not certify exact zero/full probability. Avoid
   premature products that destroy a representable requested quantity; no
   arbitrary tail cutoff, probability floor, clamp or approximation switch.

6. **Evaluate parameterized normals in requested-output units.** Require finite
   mean and finite std_dev>0; std_dev is standard deviation in the same units as
   x/mean, not variance. No zero-scale point mass. Evaluate z=(x-mean)/std_dev
   using safe scaled/difference arithmetic, without demanding an overflowing
   raw difference or other unrequested intermediate fit. Compute PDF with the
   Jacobian in the final log/exponent scaling rather than first underflowing
   phi(z) then dividing by a small std_dev. Return representable subnormal
   densities, allow genuine final density underflow0, and error on unrepresentable
   large density. Extended x limits still require all finite parameters valid.

7. **Retain a precise logarithmic Gaussian-tail profile.** For x>=0 use
   log1p(-Phi(-x)); for -10<x<0 use log(Phi(x)); for x<=-10 set a=-x and use
   -a²/2-log(a)-log(2*pi)/2+log(S), with S starting at1 and next term
   -term*(2k-1)/a². Keep at most99 updates, stop before term magnitudes cease
   decreasing, or after adding a term no larger than EPSILON*abs(S). Check a
   positive finite sum and stable reciprocal-square/quadratic/log assembly;
   negligible correction underflow is not a false probability identity. The
   positive-real erfc first-omitted-term bound controls exact-series truncation,
   not total floating error. Finite x returns a finite nonpositive log probability,
   allowing final rounding to0 in the positive tail; unrepresentable negative
   log magnitude errors. Reserve -infinity for explicit x=-infinity. No error
   estimate/tolerance option or replacement by log of an underflowed ordinary CDF.

8. **Keep centered unit-scale Student-t for every finite df>0.** Scale1 is not
   variance1; variance exists only for df>2 and equals df/(df-2). Do not impose
   that moment threshold on this mathematical distribution. For finite x use the
   same Student-t function, with lower-side tail .5*I_h(df/2,.5),
   h=df/(df+x²), and symmetry for the other side. Preserve small complementary
   beta arguments, finite df/2 and requested tail information with safe ratios,
   logarithms/exponents and symmetry as needed. Do not let x²/df overflow or
   premature addition/endpoint rounding fabricate a median or erase a
   representable tail. Legitimate final CDF rounding to.5 for tiny nonzero x is
   permitted; it does not justify losing meaningful intermediate displacement.
   If required geometry cannot be resolved, return a computation error. No
   large-df normal substitution, arbitrary df ceiling, location/variance
   normalization, or additional public Student-t log-tail/survival family.

9. **Select narrowly bounded private incomplete-beta machinery.** The pinned
   statrs0.18 incomplete-beta path has no convergence failure on exhausting its
   continued fraction, and inverse beta has unbounded nested loops. Use one
   private checked adaptation of those existing formulas/coefficients, preserving
   source/license provenance and sharing it across these Student-t operations;
   remove the old direct unchecked Student-t route. Keep the existing modified-
   Lentz continued-fraction profile, at most140 iterations and its
   abs(update-1)<=2^-53 stopping test (statrs F64_PREC, not Rust EPSILON).
   Reaching140 without passing that test is
   a computation/convergence error, not success with the last iterate. Numerical
   denominator protection is part of this kernel, not financial probability
   clipping. Replace approximate beta endpoint shortcuts with exact endpoints
   backed by preserved transformed geometry. Use stable log prefactors and
   checked finite/domain arithmetic; known log-Gamma cancellation/endpoint loss
   must not be accepted merely because the final probability is in[0,1]. In
   particular, use a stable same-function log-Gamma ratio or reject unresolved
   geometry when a+.5 rounds to a; finite individual log-Gamma values do not
   make their naive difference a trustworthy beta normalization.

   Retain the existing AS64/AS109 inverse-beta profile with one fixed private
   **1024-work-step budget**, consumed by every outer-refinement body and every
   nested damping/backtracking body. Each beta evaluation itself has the140
   limit above. Count before executing a body; no hidden unbounded loop. Use
   equivalent stable logarithms/sign/comparison arithmetic instead of squared
   tiny probabilities/residuals; keep the existing initial-iterate seed limits
   as numerical heuristics, never as an input-probability clamp.

   Make the inherited inverse stopping profile explicit. After its internal
   symmetry transformation, let a,b be the positive shapes, r the smaller beta
   target, y the current interior iterate and q=(I_y(a,b)-r)/beta_pdf(y) its Newton
   correction. Set e=trunc(-5/a²-r^(-.2)-13), acu=10^max(e,-30), and initialize
   previous correction0, stored adjustment-square bound v1 and last adjustment
   square1. When correction signs reverse or either is zero, update
   v=max(last_adjustment²,1e-30). Damped trials use adjustment=q/3^k and must have
   adjustment²<v and y-adjustment in[0,1]. An admissible trial stops only when
   v<=acu or q²<=acu; otherwise require an interior next iterate before continuing.
   Evaluate these conditions with stable scaled/log/sign arithmetic, not lost
   squares or overflowing integer casts. Unchanged state before a stopping test
   passes, exhaustion or invalid/nonfinite required state errors. A stopped beta
   endpoint still has to satisfy the Student-t reconstruction rules below. The
   1e-30 constant is this algorithm's step heuristic, not a probability floor,
   global accuracy target or qualified tail guarantee.

   This is one owned bounded adaptation, not an optional
   patched-dependency/Brent/normal fallback or public algorithm registry.

10. **Restore Student-t quantiles without squaring a representable answer.**
    Reflect p about.5 using the smaller supplied tail, solve the same beta
    inverse and restore signed sqrt(df*(1-y)/y) using checked exponent/ratio
    arithmetic. Preserve y/complement information when possible; do not require
    an unrepresentable squared quantile to fit. If interior y/complement has been
    lost to0/1 or required reconstruction is unresolved, error before division,
    panic or a fabricated0/infinity. The bounded algorithm's stopping test is
    not an independent probability-residual, ULP or inverse-accuracy certificate;
    independent qualification must cover both probabilities and quantile values.

11. **Make output/error and precision claims truthful.** Input/domain failures
    are distinct from calculation/range/convergence failures with function/stage
    context. No successful NaN, undeclared infinity, blanket min/max/clamp, catch-
    panic fallback or partial result. Validate each function's stated range;
    range checks alone are not accuracy evidence. Fixed internal numerical
    switches/budgets are disclosed profiles, not caller-selected financial
    defaults. Remove unsupported SIMD, universal tail accuracy, exact inverse
    roundtrip and cross-platform/compiler bit-identity claims. No all-finite-input
    success, correct-rounding or global absolute/relative/ULP bound is selected.

12. **Reconcile existing scalar host APIs together.** Keep Rust/Python names and
    WASM camelCase twins, scalar inputs/results and existing namespace/parity.
    All ten canonical fallible paths use centralized errors: Python ValueError
    for input/domain and RuntimeError for computation; corresponding structured
    WASM categories. Update stubs/runtime docs/declarations/examples, including
    allowed infinity results, rounded endpoint ranges, strict probability errors
    and scale/df units. No host default, class, state/serialization or second
    computational implementation.

13. **Keep financial conventions and actual consumers with their owners.**
    Reconcile core/regression CI and Student-t copula adapters that currently
    suppress errors as NaN, and probability/quantile call chains that assume raw
    scalar results. Preserve accepted09 inference availability/explicit-alpha
    and accepted analytics policies. Copula PD0/1 can legitimately map to infinite
    thresholds; its df>2/variance normalization and RNG open-interval/endpoint
    policy are separate from core df>0. Do not adopt private PD clipping, add a
    normal substitute or reinterpret product delta/confidence/default/expiry
    rules. Preserve barrier/lookback/Merton log-reflection compositions so an
    underflowed ordinary CDF does not erase a representable composed claim;
    financial outputs/error budgets and model acceptance remain separately owned.

14. **Qualify the full selected profile independently before production claims.**
    Require independent ordinary/seam/tail and closed-form/high-precision
    references, all input/parameter/endpoint/error combinations, subnormal and
    near-median quantiles, wide/small-scale normal transforms, requested-output
    restoration, tiny/large df and beta work/progress failures. Distinguish a
    converged kernel from accuracy, reference examples from production execution,
    dependency roundtrips from independence, and probability accuracy from
    financial price/risk/inference fitness. This review changes no runtime code,
    test fixture, generator or dependency.

## Accepted amendment: checked Rust beta quantile

Accepted on **2026-11-12** with [MODELS-AND-VALUATIONS-CREDIT-06](../models-and-valuations-credit/06-recovery-and-lgd.md#accepted-decision-for-checked-exposure-loss-and-recovery-allocations).
Expose one checked Rust beta quantile entry for `BetaRecovery`, using the
same bounded incomplete-beta and inverse-beta machinery specified above.
Require finite positive shapes and finite `p` in `[0,1]` with exact endpoint
results. For interior `p` require `0<x<1` and verify the actual returned
`x` against the smaller supplied tail, using `log(p)` on the lower side and
`log1p(−p)` on the upper side. Require an absolute log residual at most
`1e−8`. Unresolved values, exhausted work and failed residuals error. This
adds no host beta API or public incomplete-beta routines. The Student-t
contract is governed by its own gates above. Reconcile and qualify the
shared implementation with CREDIT-06.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| NaN variate, invalid scale/mean/df, or inverse p outside[0,1] | Input/domain error; no NaN/saturation fallback. |
| Valid forward x=-/+infinity | Declared distribution limits after full parameter validation. |
| Normal/Student-t inverse p0/p1 | -/+infinity; invalid df still errors first. |
| Valid centered x0 / inverse p.5 | Exact .5 /0, including positive df whose half would underflow on a nontrivial path. |
| ln_gamma(1),ln_gamma(2),ln_gamma(1.5) | 0,0 and a finite negative value respectively. |
| Tiny positive log-Gamma argument | Same-function positive recurrence; no unexamined tiny sine/reflection loss. |
| General normal x=MAX,mean=-MAX,std_dev=MAX | z2, CDF about.97724986805 and PDF about3.00334720460e-310; no raw-difference saturation. |
| General normal x=40*2^-1000,mean0,std_dev=2^-1000 | Density about1.56790666841e-47 despite standalone phi(40) underflow. |
| log_norm_cdf(-10),(-40),(-100) | About-53.23128515051,-804.60844201375,-5005.52420869421, preserving representable log tails. |
| log_norm_cdf(-1.5e154) / finite input with unrepresentable negative log | Finite value about-1.125e308 / computation-range error; no premature x² overflow. |
| log_norm_cdf(40), ordinary far-tail PDF/CDF or finite erf saturation | Final representational0/0-or-1/+/-1 may be valid; never infer exact mass from rounding. |
| Student-t df1, inverse p.75 / df2,CDF x1 | Quantile1 / .5+1/(2sqrt3), subject to qualified floating error. |
| Student-t df1,CDF x=-float64(1e200) | Tail about3.18309886184e-201, or truthful unresolved computation error; never0 merely because x² overflowed. |
| Student-t df=2^60,x1 | Do not fabricate median .5 from rounded beta geometry; resolve the same function or error. |
| Student-t df=2^-40,inverse p.75 | True quantile exceeds MAX: computation/range error, not an endpoint infinity. |
| Positive df/2 or interior beta geometry lost to0/1 | Computation error on the nontrivial path before dependency panic/false endpoint. |
| Continued fraction/inverse work budget exhausted or unresolved stagnation | Computation/convergence error, never a silently returned last iterate. |

## Observed choice and rationale

Most distribution functions delegate to the existing `statrs` dependency. Normal density is evaluated directly. Log normal CDF has a special negative-tail expansion to retain representable log probabilities after the CDF itself underflows. No normal approximation replaces large-df Student-t calls.

## Observed algorithm

1. `erf`, `ln_gamma`, normal CDF/inverse, and Student-t CDF/inverse call statrs. A lazily initialized N(0,1) is shared. General normal functions standardize `(x−mean)/std_dev`, and PDF also divides by std_dev.
2. Normal PDF is `exp(−x²/2)/sqrt(2pi)` with a stored normalization constant.
3. For log normal CDF, x≥0 uses `log1p(−Phi(−x))`; −10<x<0 uses `log(Phi(x))`. At x≤−10 use `−x²/2−log(−x)−log(2pi)/2+log(S)`, with asymptotic sum S starting at 1 and successive term `−term*(2k−1)/x²`. Stop before terms increase, at epsilon relative to the sum, or after at most 99 updates.
4. Normal and Student-t inverse calls explicitly map p≤0 to −infinity, p≥1 to infinity, and NaN to NaN. Student-t validates finite df>0 first.

## Observed selection, controls and conventions

No numerical tolerance is exposed. General normal methods require finite positive standard deviation but do not independently reject nonfinite x or mean. Student-t scale is one, not variance one: finite variance, where defined, differs from that scale. Consumers must apply any variance normalization themselves.

## Observed limitations, failure and fallback

Out-of-domain finite p saturates rather than errors. Student-t invalid df errors; NaN x/p returns Ok(NaN). `ln_gamma` and erf inherit dependency domain behavior. Parameterized normals directly form the difference/quotient and standard density before its scale adjustment. Student-t inherits beta argument, normalization, convergence and reconstruction limitations from the pinned dependency; finite supplied inputs alone do not establish valid transformed arithmetic. The logarithmic tail uses an asymptotic series with a fixed switch at −10; it does not return an error estimate. No checked financial input domain is inferred.

## Observed alternatives and tradeoffs

Direct versus logarithmic CDF use is explicit; multiplying an underflowed CDF cannot recover information retained by log-space evaluation. A normal cutoff for large-df Student-t would change tail behavior and is absent. Fully log-domain Student-t tails are not supplied by these wrappers.

## Evidence

- **Observed source and located tests.** [Implementation and inline tests](../../../../finstack-quant/core/src/math/special_functions.rs) expose all ten functions and their currently mixed fallibility, raw parameterized standardization, saturating inverses and log-tail profile. `logarithmic_normal_cdf_preserves_underflowed_tails` uses three tabulated values with2e-12 absolute log tolerance; generation/provenance is not linked in that test. Existing normal/Student-t roundtrips share dependency kernels and are consistency evidence. Boundary/invalid-df tests locate the old saturation/NaN/error contracts; they were not executed and do not validate the accepted replacements.
- **Pinned dependency and bounded-adaptation reason.** [Cargo.lock](../../../../Cargo.lock) pins statrs0.18.0. Its [normal source](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/distribution/normal.rs), [Student-t source](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/distribution/students_t.rs) and [beta source](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/function/beta.rs) identify the existing profiles. Beta continued-fraction exhaustion returns its final iterate, while inverse beta uses unbounded nested loops and unflagged stopping/stagnation. These missing checked hooks justify the narrowly owned bounded adaptation, rather than a second public solver or an assumption that preflight proves termination. Its existing seed/denominator protections and stopping heuristics are numerical devices, not financial probability policy or true-error guarantees. Preserve source/license attribution; this review copies no implementation into runtime.
- **Static range/geometry targets.** The source's raw general-normal subtraction can overflow for x=MAX,mean=-MAX,std_dev=MAX despite exact z2; its standard PDF can underflow before a small scale rescues a representable final density. The pinned Student-t path forms x² and df/(df+x²), df/2, naive log-Gamma differences and a squared quantile ratio: these can overflow, erase meaningful beta displacement, lose a tiny shape or destroy an otherwise representable answer. A rounded h1 or finite in-range result is not independent correctness evidence. The proposal requires safe same-function evaluation or truthful computation failure, not an unverified normal approximation or arbitrary df cutoff.
- **Independent bounded high-precision references.** On2026-11-08, standard-library Decimal calculations at150–170 digits used Machin-series pi, the [Mills continued fraction](https://dlmf.nist.gov/7.9) and [bounded erfc expansion](https://dlmf.nist.gov/7.12.E1), independently of production/statrs calls. They gave logPhi(-10)=-53.2312851505124705783470273541312, logPhi(-40)=-804.608442013753788166606832918610 and logPhi(-100)=-5005.52420869420508862630245733003. Reported reference-bracket widths were below2.2e-79,1.2e-99,6.5e-104 respectively. This establishes bounded reference examples and identifies the previously unlinked tail values; it does not establish production error at the switch or over a whole domain.
- **Independent transform/closed-form references.** The same calculations gave Phi(2)=.977249868051820792799717362833467 and phi(2)/MAX=3.00334720460081132151386499896e-310, plus phi(40)/2^-1000=1.56790666841285025073943968625e-47. [Cauchy identities](https://www.itl.nist.gov/div898/handbook/eda/section3/eda3663.htm) give the actual binary64-input tail F1(-float64(1e200))=3.18309886183790681172013950420e-201 and Q1(.75)=1. Student-t F2(1)=.5+1/(2sqrt3)=.788675134594812882254574390251. The [Student-t density/beta definition](https://stat.ethz.ch/R-manual/R-patched/library/stats/html/TDist.html) also gives F_df(MAX)-.5 <= (df/2)*(1+log(MAX/sqrt(df))) <3.296e-10 for exact df=2^-40, so its .75 quantile exceeds MAX. These bounded calculations are reference evidence, not newly run production tests or committed fixtures.
- **Primary function provenance.** [NIST special-function references](../../../REFERENCES.md#nist-special-functions) identify the error/Gamma definitions, same-function Gamma recurrence and real erfc truncation bound; the latter controls truncation rather than total floating-point error. The [pinned implementation reference](../../../REFERENCES.md#statrs-018-special-functions) distinguishes the actual Boost-derived normal inverse and AS64/AS109 Student-t path from unrelated algorithms. [Numerical Recipes](../../../REFERENCES.md#press-numerical-recipes) remains general context, not certification of these particular wrappers.
- **Actual host and consumer reconciliation.** [Python](../../../../finstack-quant-py/src/bindings/core/math/special_functions.rs), [WASM](../../../../finstack-quant-wasm/src/api/core/math.rs) and the [parity ledger](../../../../finstack-quant-py/parity_contract.toml) retain ten matching scalar operations. Current raw scalar paths and endpoint/NaN/strict-interior/accuracy examples need coordinated changes through existing centralized error mapping. [Core CI](../../../../finstack-quant/core/src/math/stats.rs), [analytics regression CI](../../../../finstack-quant/analytics/src/benchmark.rs) and the [Student-t copula adapter](../../../../finstack-quant/models/src/correlation/copula/student_t.rs) suppress errors as NaN; accepted09/analytics policies and owner-qualified failures must remain authoritative. [Copula threshold selection](../../../../finstack-quant/models/src/correlation/copula/mod.rs) legitimately uses probability endpoints while retaining its own df policy. [Barrier](../../../../finstack-quant/models/src/closed_form/barrier.rs), [lookback](../../../../finstack-quant/models/src/closed_form/lookback.rs) and [Merton](../../../../finstack-quant/models/src/credit/merton/default_probability.rs) demonstrate why log probabilities must be retained through reflected products. Their payoff, clipping/default and financial-error policies are not adopted by this numerical package.

Existing tests do not demonstrate every representable input.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
