# CORE-NUMERICS-17: PCG normal draws, Brownian bridge ordering and Poisson counts

- Decision status: **Accepted — complete RNG/bridge/Poisson package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent arithmetic/conditional-Gaussian references and read-only RNG/bridge/count/caller reviews were completed on2026-11-08; these do not certify production random streams, numerical range or financial models.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/random.rs`, `random/brownian_bridge.rs`, and `random/poisson.rs`: seeded pseudorandom uniforms/normals, Brownian paths on increasing time grids, and Poisson quantile counts. Generic process and product selection remains outside this slice.

## Accepted decision: checked random draws, bridge and Poisson counts

Accepted on **2026-11-08** after whole-record review. Runtime/caller reconciliation and independent numerical/statistical/
financial qualification remain pending; source-observed behavior is retained below.

1. **Keep the three mechanisms explicit and separate.** PCG supplies random
   words/uniforms and cached normals; the immutable Brownian bridge transforms
   supplied shocks into time levels; Poisson quantile maps one supplied uniform
   to an integer count. None automatically enables the others, chooses a process,
   calendar, intensity conversion, product payoff or QMC error model. Retain
   existing dependencies and Rust-only core scope, with no new public options/
   state/serialization/replay framework or algorithm registry.

2. **Select one explicit PCG constructor and transparent stream mapping.** Use
   Pcg64Rng::new(seed:u64,stream:u64)->Self and the existing rand_pcg Pcg64
   (XSL-RR128/64), passing state=seed as u128 and sequence=stream as u128 to its
   constructor. The pinned library forms increment=2*stream+1 and its documented
   initialization. Every u64 stream selects a distinct odd increment. Remove
   new_with_stream, seed_from_u64 expansion and custom stream/state bit mixing;
   no alias or old seeded-sequence compatibility. Correct current claims about
   SplitMix64 and double-shifted stream increments. No implicit seed or stream.

3. **Fix the uniform grid and qualify stream/replay claims.** Keep
   RandomNumberGenerator::uniform(&mut self)->f64 with finite output in[0,1).
   PCG consumes one raw u64 word, takes its top53 bits k and returns k*2^-53;
   there is no floor, rejection or hidden draw. The PCG transition has period
   2^128 per selected odd increment; different streams/seeds do not prove
   independence, non-overlap or distinct finite output prefixes. Reproducibility
   requires the selected implementation/dependency/build/math profile and exact
   ordered calls, not a seed alone. Retain private state/cache and Clone/Debug; a clone
   copies the same state/spare and is not an independent stream.

4. **Select one checked reflected Box–Muller transform.** Rename the canonical
   helper to box_muller(u1,u2)->Result<(f64,f64)>, validate both finite uniforms
   in[0,1), then use r=sqrt(-2*log1p(-u1)), theta=2*pi*u2 and one sin_cos pair,
   returning (r*cos(theta),r*sin(theta)). This is the same ideal continuous-normal
   construction through1-U1, with fixed inputs and no redraw or numerical floor.
   u1=0 gives radius0; reject1 rather than clip it. For binary64 u1<1 the ideal
   radius is at most sqrt(106log2), about8.57167; remove the old37-sigma claims.
   A finite uniform grid produces discrete pairs/endpoint atoms, not exact
   continuous independent normals or guaranteed extreme-risk coverage. Require
   finite outputs and truthful computation failure, without an unchecked twin.

5. **Make general-normal requests fallible and explicit.** Change the trait and
   PCG normal(mean,std_dev)->Result<f64>. Require finite mean and finite
   std_dev>=0 before consuming state or cache. Exact std0 returns mean without
   draws/cache changes, an explicit sampling point-mass identity distinct from
   accepted12's positive-scale density/CDF domain. For positive scale obtain one
   standard normal, restore with fused std_dev*z+mean, and require finite final
   output; a separate overflowing product must not lose a finite fused result.
   Parameter errors are input/domain failures and unresolved output/range errors
   are computation failures. Uniform remains the infallible primitive with its
   stated output/statistical contract; no parallel checked/unchecked RNG families.

6. **Define normal caching and failure consumption.** With no spare, consume
   exactly two uniform trait calls, return the first checked standard variate
   and cache the second STANDARD variate; scale it with each call's own parameters.
   A spare consumes no uniforms, and intervening uniform calls leave it intact.
   Invalid parameters/std0 consume no standard variate. A valid positive-scale
   request consumes its selected standard variate even if final restoration fails;
   a newly generated companion remains cached. No rollback, reseed, replacement
   or retry. The generic trait requires well-distributed source values; numeric
   checks, Send/Sync and finite results cannot certify statistical independence
   or bound an arbitrary implementation's internal work.

7. **Retain a simple checked bridge topology with a resource limit.** Keep
   BrownianBridge as private immutable step/index topology, with
   new(num_steps)->Result<Self> and fixed inclusive0<=num_steps<=100,000.
   Check n+1/bytes and use fallible reservation; no unchecked/unbounded constructor
   or new owned-grid/cache class. n0 is valid. Build indices using
   left+(right-left)/2, with bounded recursion/iteration. This is a resource
   policy, not a statistical time-resolution recommendation or a TimeGrid default.

8. **Validate the whole bridge request before writes.** construct_path keeps
   explicit z,w_out,times inputs: exactly n finite supplied shocks, n+1 output
   cells and n+1 finite strictly increasing times, with t0 exactly0. No minimum
   spacing epsilon, sorting, gap filling or implicit uniform grid. n0 requires
   z=[],times=[0],w_out.len()==1 and returns[0]. Caller times use one consistent
   clock/unit, with Brownian levels in sqrt(time) units; financial year fractions
   are not inferred. Shocks target independent standard normals, whose statistical
   validity is a caller assumption; no clipping or hidden random draws.

9. **Pin terminal-first depth-first INDEX ordering.** z0 drives the terminal
   level; recurse on the left midpoint subtree before the right, using integer
   index midpoints on any admitted grid/step count. For n8 the full shock order
   is[8,4,2,1,3,6,5,7]. Retain this one mapping, not a new breadth-first or
   variance-priority option. It does not globally order conditional variance,
   even on uniform grids. Remove unconditional effective-dimension, convergence-
   rate, variance-reduction or barrier-hitting improvement claims.

10. **Evaluate the conditional Gaussian law in requested-output form.** Set
    W0=0 and Wn=sqrt(tn)*z0. For known l,r and midpoint m, let a=tm-tl>0,
    b=tr-tm>0 and span=tr-tl. The conditional mean is
    (b*Wl+a*Wr)/span and SD=sqrt(a*b/span). Compute stable weighted endpoint
    contributions without overflowing Wr-Wl or erasing a tiny weight that a
    large endpoint rescues. Compute SD directly, e.g.
    sqrt(min(a,b))*sqrt(max(a,b)/span), without requiring a*b or the variance
    itself fit. Combine mean+SD*z using fused/scaled arithmetic before final
    rounding. All required levels must be finite; no negative-variance floor,
    silent NaN or successful partial path. Unresolved arithmetic errors.

11. **State bridge scratch and downstream precision limits.** Invalid shape,
    grid/shock or resource admission leaves output values unchanged. Once
    computation starts, later failure may leave output unspecified and callers
    must discard it; no full transactional copy. Output is chronological Brownian
    LEVELS. Increment and standardized-increment consumers must use the same
    actual adjacent knots and evaluate their complete requested quantities before
    avoidable subtraction/product/division loss. Rounded levels alone cannot
    recover every representable mathematical increment: retain needed transform
    information or fail if the owner's requested result is unresolved. Do not
    promise universal rescue or add an unneeded new public increment API here.
    TimeGrid construction/dt/event policy remains with record20.

12. **Define one architecture-independent Poisson quantile domain.** Rename
    the canonical function to poisson_quantile(lambda,u)->Result<u64>, with finite
    lambda>=0 and finite u in[0,1). Validate both before lambda0/u0 identities,
    which return0; u1 errors even for lambda0. Lambda is the expected count for
    the caller's interval, not an annual rate automatically multiplied by dt.
    The count domain is0..=2^53-1, so k+1 is exactly representable as binary64;
    no usize-dependent saturation, floating count rounding or sentinel. This
    output/evaluation limit does not certify numerical success for every lambda.

13. **Use one shared checked log-tail count predicate.** Remove the<30/200
    sequential branch and direct statrs Poisson CDF path. Reuse accepted15's
    private bounded log-Gamma kernels/normalizers, with their work/progress/error
    contracts; do not apply the chi-squared inverse's df ceiling as a Poisson
    lambda ceiling. F(k)=Q(k+1,lambda), SF(k)=P(k+1,lambda). Compare logF to log(u)
    for u<=.5 and logSF to log1p(-u) above. The k0 logF=-lambda identity may be
    evaluated directly with its stable complementary form. No underflowed ordinary
    probability or near-one subtraction, residual clipping, normal/Cornish–Fisher
    approximation, new Gamma copy or different-law fallback.

14. **Select one bounded integer bracket/binary search.** Evaluate the predicate
    at0. If false, start hi1, double with checked arithmetic and cap at K=2^53-1
    until the predicate is true; a false predicate at K is a count-range failure.
    Maintain a cached false lo/true hi and bisect by lo+(hi-lo)/2 until adjacent.
    At most54 positive upper probes,52 bisections and the0probe are needed,
    hence107 predicate probes; reuse final adjacent cached predicates rather
    than hide additional scans. Each Gamma evaluation keeps its100,000-update
    bound, with failure on unresolved state/progress. This is a finite work
    profile, not a universal wall-clock or all-range-success guarantee.

15. **Require both neighboring discrete inverse inequalities.** Return the
    smallest numerical k with F(k)>=u and k0 or F(k-1)<u; equivalently for upper
    tails SF(k)<=1-u<SF(k-1). Validate the final adjacent states, without a
    continuous-root tolerance, epsilon probability snap or interpolation of counts.
    Known unresolved boundary comparisons, invalid/nonmonotone tail results,
    kernel failure and count overflow are computation/convergence failures,
    not an arbitrary tie choice. Computed inequalities/independent qualification
    are not an all-input exact-count/correct-rounding certificate.

16. **Reconcile all fallible callers and preserve Rust-only surfaces.** Generic
    PCG/RNG/Box–Muller/bridge/Poisson remain intentionally unbound; do not add
    Python/WASM classes/options or duplicate wrappers. Propagate normal and
    transform errors through accepted14 sampling, statement/covenant/toggle/
    Merton call chains and the distinct Philox RandomStream fill path, without
    unwraps, NaN/zero/false, deterministic mean or skipped-path replacements.
    Checked Poisson u64-to-usize/allocation admission belongs to count consumers;
    compute no unused antithetic count when that output is disabled. Propagate
    bridge construction/path failures through the selected Sobol caller. Keep existing
    model-host errors/docs/examples and actual numerical fixtures coherent in
    the future runtime slice, with no successful partial model result.

17. **Keep actual stream, clock and financial/QMC choices with owners.** Models
    supply and disclose their chosen stream IDs; this generic constructor does
    not select product seed/stream defaults, statistical independence or path
    assignment. Philox counter/split/open-uniform policy and Sobol scramble/
    dimension/replicate policy are separate, as are model bridge flags, single-
    factor limits, standardized increments, actual knots, barrier-crossing and
    process discretization. Merton owns lambda*dt, antithetic endpoint/coupling
    and jump-vector/resource acceptance; its analytical Poisson-mixture cutoff
    is a different calculation. No global TimeGrid/uniform-helper or monitoring
    adoption, financial retry, convergence/price/risk certificate or replay
    framework is implied by these primitives.

18. **Qualify all three mechanisms independently before production claims.**
    Require primary/pinned known-answer PCG mapping/word/grid references, ordered
    mixed-call/cache/clone/std0/error cases, reflected-transform endpoints/finite-
    grid/statistical diagnostics, exact bridge order and conditional-covariance/
    extreme-range/scratch references, and independent Poisson adjacent-CDF tails/
    count/work/range cases. Separate same-kernel comparisons, sampled moments,
    deterministic references, stream independence, QMC performance and financial
    acceptance. This review changes no runtime, tests, fixtures, dependencies,
    generated host surface or model policy.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Explicit PCG seed/stream | One direct pinned-library mapping; no seed expansion, stream-bit mixing or implicit default. |
| Uniform raw word0 /u64MAX | Grid0 /1-2^-53, one raw word each. |
| box_muller u1=0 /u1=1 or malformed input | Finite zero-radius pair /input error, no floor or redraw. |
| normal with invalid parameters /std0 | Error with no state consumption /exact mean with cache/draws unchanged. |
| mean=-MAX,std=MAX,z=1.5 | Fused result MAX/2, not overflow of the separate product. |
| Failed positive-scale normal restoration | Standard variate consumed; companion remains cached; no retry/rollback. |
| Bridge n0 /n100,001 | [0] for valid empty request /resource-domain error. |
| times[0,1,4],shocks[z0,z1] | Levels[0,(z0+sqrt3*z1)/2,2*z0] and the stated ideal Gaussian covariance. |
| times[0,eta,2eta],shocks[0,1] | Midpoint2^-537/sqrt2 despite unrepresentable conditional variance eta/2. |
| times[0,eta,4],shocks[2^999,0] | Midpoint2^-76; no erased tiny interpolation weight. |
| Bridge finite target with overflowing raw endpoint difference/noise product | Preserve complete conditional result or fail computation, never successful infinity. |
| Rounded level0 whose standardized increment could be nonzero | Owner needs retained transform information or truthful failure; no universal recovery from rounded levels. |
| Poisson lambda0/uvalid or u0/lambdavalid | Count0 after full input validation; u1 is always invalid. |
| Poisson lambda1,u.5/.9/.99 | Reference counts1/2/4, each with adjacent inequalities. |
| Poisson lambda740,u=eta | Count0 via logF(0)=-740; do not erase the subnormal zero-count mass. |
| Poisson quantile beyond K or unresolved Gamma/adjacent predicate | Computation/range failure, no saturation, approximate count or retry. |

## Observed choice and rationale

PCG64 is the supplied pseudorandom generator; Box–Muller produces two normals per two uniforms and caches one. Brownian bridge changes the mapping of supplied normals to path time levels in terminal-first depth-first index order; this does not establish global variance priority or QMC improvement. Poisson inversion preserves a single-uniform count mapping across low/high intensities. These are independently selected mechanisms, not automatically enabled together.

## Observed algorithm

1. `Pcg64Rng::new(seed)` delegates to `seed_from_u64`. `new_with_stream(seed,stream)` supplies state `(stream<<64)|seed` and sequence argument `(stream<<1)|1` to `Pcg64::new`.
2. Uniform uses rand's f64 generation. Normal consumes a cached standard normal if present, otherwise draws U1,U2 and calculates `r=sqrt(−2 log(clamp(U1,MIN_POSITIVE,1−EPSILON)))`, angle=2pi U2, pair `(r cos(angle),r sin(angle))`; second normal is cached. Requested mean/std multiply only at output.
3. A bridge precomputes midpoint indices recursively, terminal first. Set W0=0, Wn=sqrt(tn) z0. For midpoint m between known l,r, conditional mean is `Wl+(tm−tl)/(tr−tl)(Wr−Wl)` and variance `(tm−tl)(tr−tm)/(tr−tl)`. Use successive normals in depth-first left/right midpoint order.
4. Poisson validates finite lambda≥0 and u∈[0,1). Zero lambda/u returns count 0. For lambda<30, accumulate P0=exp(−lambda) and recurrence `Pk=Pk−1 lambda/k` through at most 200 counts.
5. If low-intensity accumulation does not reach u, or lambda≥30, use statrs CDF for u≤0.5 or survival for u>0.5. Double an integer upper bound from 1 until its CDF reaches u, then binary search the smallest satisfying count; detect usize/range overflow and invalid dependency probabilities.

## Observed selection, controls and conventions

Seeds and stream IDs are explicit. Bridge construction fixes number of steps; supplied times include t0=0, use consistent year units, and must be finite strictly increasing. Bridge output is Brownian levels, so increments require subtraction. Poisson lambda is expected count for the caller's interval, not an annual intensity automatically multiplied by dt.

## Observed limitations, failure and fallback

Normal parameters and Box–Muller uniforms are not validated beyond U1 clamping. Bridge validates dimensions/times but not shock finiteness. Poisson uses exact-distribution numerical inversion rather than a normal fallback; the low-intensity recursion is only its fast branch. PCG stream independence is not proven by different seeds or the deterministic stream construction. Side effects from normal caching make draw-order changes alter subsequent paths.

## Observed alternatives and tradeoffs

Direct increments versus bridge ordering and PCG versus the separate [Sobol API](sobol-sequences.md) are explicit caller choices. Inverse normal mapping is used by Sobol, not PCG. A generic RNG trait does not promise quasi-Monte Carlo error estimates.

## Evidence

- **Observed source and located tests.** [RNG](../../../../finstack-quant/core/src/math/random.rs), [bridge](../../../../finstack-quant/core/src/math/random/brownian_bridge.rs) and [Poisson](../../../../finstack-quant/core/src/math/random/poisson.rs) contain the current dual seeding paths/normal floors, infallible transforms, unchecked shock/output arithmetic and sequential/dependency count branches. `test_pcg64_rng_normal_statistics` and uniform cases are sampled-moment checks; seed/stream examples distinguish some sampled sequences, not independence. `test_brownian_bridge_irregular_grid_construction` checks supplied conditional values; `precomputed_brackets_match_per_path_lookup_bit_for_bit` compares two versions of the same mapping. Poisson `quantile_brackets_cdf_on_both_sides_of_normal_approximation_threshold` uses shared-statrs inequalities around29.9–100; its name is historical and no normal approximation remains. Tests were inspected/located, not executed, and do not qualify the selected profiles.
- **Pinned PCG source correction.** [Cargo.lock](../../../../Cargo.lock) pins rand_pcg0.9.0/rand_core0.9.5. Their inspected sources identify Pcg64 as XSL-RR128/64; the dependency itself shifts its sequence argument to form an odd increment. Thus the current wrapper's stream mapping produces4*stream+3, not its claimed2*stream+1. The current default seed expansion uses PCG32, not SplitMix64. The proposal removes both competing paths in favor of the direct explicit mapping; it does not claim an old-seed compatibility or empirical independence result. The [PCG author reference](../../../REFERENCES.md#o-neill-2014-pcg) and pinned implementation roles remain distinct from project known-answer/stream qualification.
- **Transform and bridge provenance.** [Box–Muller](../../../REFERENCES.md#box-muller-1958), the [author-hosted Devroye derivation](../../../REFERENCES.md#devroye-1986-random-variates), and the [conditional Brownian bridge reference](../../../REFERENCES.md#brownian-bridge-conditional-law) support the ideal laws. The reflected half-open/fixed-grid/cache/error/DFS/resource choices are explicit project profiles, not a source guarantee of exact continuous normals, global variance ordering or QMC rates. The bridge file ends in its tests; no trailing runtime section was missed. Uniform TimeGrid constructors/dt consistency/event merging belong to unreviewed record20 rather than another bridge constructor.
- **Independent bridge references.** Exact conditional-Gaussian algebra gives levels[0,(z0+sqrt3*z1)/2,2z0] on times[0,1,4], with level covariance[[1,1],[1,4]] and standardized increments[(z0+sqrt3*z1)/2,(sqrt3*z0-z1)/2], an ideal orthogonal transform. Exact power-of-two/rational cases give midpoint2^-537/sqrt2 on[0,eta,2eta], midpoint2^-76 on[0,eta,4] with terminal shock2^999, and midpoint3*2^1022 on[0,8,16] with shocks[-2^1021,2^1023], despite unrepresentable variance/weight/noise intermediates. On[0,1,2,3,4] with shocks[MAX/2,-MAX,0,0], all levels[0,-MAX/4,-MAX/2,MAX/4,MAX] fit although a raw endpoint difference overflows. These are bounded mathematical references, not production path execution.
- **Requested-output limits.** On[0,8,16] with shocks[MAX/4,-MAX/2], the final raw increment1.5MAX cannot fit while its standardized value3MAX/(4sqrt2) does. Conversely a one-step grid ending at eta with shock eta has a level rounding to0 although its standardized increment is eta. Rearranging arithmetic cannot recover information already erased in the level view; consumer retention/error policy is required. Reflected Box–Muller has ideal binary64-input radius bound sqrt(106log2), and FMA can rescue mean=-MAX,std=MAX,z=1.5 to MAX/2. Finite-grid and these arithmetic facts do not certify delivered normal/financial tails or every representable target.
- **Independent count/search references.** The [Poisson definition and Gamma identities](../../../REFERENCES.md#poisson-count-quantile) distinguish the mathematical smallest-count inverse from dependency rounding policies. High-precision defining-PMF sums give lambda1,u.5/.9/.99 counts1/2/4. At lambda740, logF0=-740 exceeds log(eta), so ueta has quantile0 despite ordinary prefactor cutoffs. The fixed count ceiling gives54 positive upper probes and at most52 bisections plus the0probe, with adjacent predicates cached. Exact grid/mapping/count arithmetic and bounded path/Poisson reference calculations on2026-11-08 use no project RNG, sampler or kernel calls; predicate/kernel accuracy and all-boundary count acceptance still require independent production qualification.
- **Actual fallible caller and scope boundaries.** The [parity contract](../../../../finstack-quant-py/parity_contract.toml) intentionally keeps these core random APIs unbound. [Philox](../../../../finstack-quant/models/src/monte_carlo/rng/philox.rs) calls the shared transform through its separate RandomStream fill/cache path; checked errors must propagate without a sentinel. [Accepted14 sampling](beta-gamma-sampling.md#accepted-decision-checked-beta-and-gamma-sampling), [statements](../../../../finstack-quant/statements/src/forecast/statistical.rs), [covenant MC](../../../../finstack-quant/covenants/src/forward.rs), [toggle MC](../../../../finstack-quant/models/src/credit/toggle_exercise.rs) and [Merton simulation](../../../../finstack-quant/models/src/credit/merton/simulation.rs) need the complete fallible-normal chain, including currently plain-returning helpers. Merton converts annual intensity/time and currently computes an unused antithetic count even when disabled; its endpoint/coupling policy is not adopted here. [Selected Sobol path conversion](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs) uses bridge levels and standardizes using its time data; preserve actual-knot consistency and range/error semantics without adopting record18/20 defaults or continuous barrier monitoring.
- **Qualification limit.** Independent read-only mathematics, source/API and caller reviews assessed the complete package. No production random stream, suite, binding build, fixture generator, statistical experiment or financial benchmark ran. [Monte Carlo context](../../../REFERENCES.md#glasserman-2004-monte-carlo) does not turn finite reference examples into stream-independence, convergence, range or risk certificates.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
