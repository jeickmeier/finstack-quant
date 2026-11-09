# CORE-NUMERICS-14: Marsaglia–Tsang Gamma sampling and beta ratios

- Decision status: **Accepted — complete beta/Gamma-sampling package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded/exact reference calculations and read-only algorithm/RNG/caller reviews were completed on 2026-11-08; these do not certify the production sampler or recovery model.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/distributions.rs::sample_beta` and private Gamma sampler: positive finite shape parameters and a caller-supplied `RandomNumberGenerator`. Beta outputs lie on [0,1] under the assumed RNG contract; this is a generic distribution, not a chosen recovery/severity model.

## Accepted decision: checked beta and Gamma sampling

Accepted on **2026-11-08** after whole-record review. Runtime,
RNG/caller reconciliation and independent numerical/statistical/financial
qualification remain pending; source-observed behavior is retained below.

1. **Retain one checked beta sampler and private Gamma machinery.** Keep
   sample_beta(rng,alpha,beta)->Result<f64> for dimensionless finite alpha>0 and
   beta>0, targeting the continuous Beta(alpha,beta) law under its statistical
   RNG assumptions. Keep Gamma(shape,scale1) generation private; its selected
   representation is sampled log-Gamma components, not the deterministic
   ln_gamma(shape) special function. No public Gamma/log sampler, method/options
   family, normal/point-mass shortcut, arbitrary shape cutoff or approximation
   registry. Remove the obsolete raw-Gamma ratio and Bernoulli endpoint fallback.

2. **Validate parameters and every consumed RNG value.** Validate both shapes
   before any RNG call. Honor the existing uniform trait contract: finite U in
   [0,1), including0; U1, outside values, NaN and infinities are malformed output.
   Request normal(0,1), requiring a finite returned standard-normal variate.
   Abort immediately on malformed output, with source/draw-stage computation
   context, without clamping, skipping or retrying it. Numeric checks cannot
   establish uniformity, normality or independence: the caller supplies independent
   correctly distributed draws, including across both Gamma paths. Send/Sync,
   a seed and finite outputs do not establish that statistical contract.

3. **Keep the exact uniform special case.** For caller values alpha=beta=1,
   return one checked uniform draw in[0,1), consuming no normals or Gamma work.
   This is an exact distribution identity and declared finite-RNG endpoint
   profile, not an epsilon-based approximately-uniform branch. Generic beta
   results from other paths may round to either0 or1 as specified below.

4. **Retain Marsaglia–Tsang proposals with stable geometry.** For shape a>=1
   set d=a-1/3 and c=(1/sqrt(d))/3, avoiding overflowing9*d. Each attempt consumes
   one finite standard normal X. Evaluate t=c*X and the positive cubic base
   b=1+c*X consistently, using FMA for cancellation-sensitive base admission.
   b<=0 is an ordinary rejected proposal and consumes no acceptance uniform.
   A positive base then consumes one checked acceptance uniform. Preserve small
   displacement through log1p(t) where appropriate, and use the consistent log(b)
   near cancellation; rounded t=-1 alone must not discard a positive FMA base.
   Accepted log-Gamma base components are log(d) and3log(b), with their local
   correction information retained. Do not materialize b³ or d*b³ merely to log
   them, and do not clamp the base/cube to a tiny positive constant.

5. **Specify the exact acceptance law and bounded cancellation profile.** Retain
   the strict squeeze U<1-.0331*X^4, evaluated with checked scaled comparisons
   rather than an overflowing fourth power or a clipped threshold. Otherwise
   accept when log(U)<R, where in real arithmetic
   R=X²/2+d*(1-b³+3log(b))=d*H(t),
   H(t)=3log1p(t)-3t+1.5t²-t³ for t>-1. This is the same mathematical MT law,
   not bitwise equivalence to the unstable original floating evaluation.

   For |t|<=.5 select H=t^4*h,
   h=-3*sum_(j=0..63)((-t)^j/(j+4)), with fixed source-order checked compensated
   accumulation. Restore d*H through ((d*t²)*t²)*h or equivalent checked scaling,
   avoiding t^4 underflow before multiplication by d. The exact omitted h tail
   is below5e-21 throughout this region; this bounds truncation, not total
   floating error. Beyond this region use the same log/base-plus-polynomial
   remainder with stable scaling and cancellation-aware base/log consistency.
   H<=0 in real arithmetic since H'(t)=-3t³/(1+t); unresolved required arithmetic
   or an invalid positive/nonfinite computed H or R errors, rather than becoming a
   proposal rejection or clamped zero. Exact representational underflow of a
   negligible negative remainder is distinct from erasing required information.

6. **Handle valid uniform zero explicitly, without floors or redraws.** Select
   log(0)=-infinity only for an actual checked U0. In MT it accepts any valid
   positive proposal, so an unused unrepresentable acceptance intermediate need
   not be formed. In the shape boost below it tags an exact represented-zero
   Gamma path. There is no zero redraw, reflection or max(U,1e-300), and malformed
   uniforms never receive this treatment. This finite-RNG endpoint convention is
   separate from the ideal continuous-law theorem; arithmetic loss from a
   nonzero U must never be relabelled as exact zero provenance.

7. **Keep the same-law shape boost in deferred logarithmic form.** For0<a<1,
   draw and validate the augmentation uniform first, then draw the base
   Gamma(a+1,1) under the same MT rules and proposal budget, even when the
   augmentation uniform is0. Use log(G_a)=log(G_(a+1))+log(U)/a. This is the
   positive-shape augmentation identity; do not call it an independently selected
   Ahrens–Dieter sampler. Remove powers, explicit1/a and numerical floors. Keep
   positive-U boost quotient information deferred/scaled until the beta logit;
   do not materialize an overflowing negative scalar log and infer an endpoint.
   Rounded shape shifts and all other finite-precision steps require qualification;
   they establish no exact continuous-law or all-shape-success guarantee.

8. **Preserve components until forming the joint log difference.** Draw alpha's
   Gamma path completely before beta's. Retain log-scale/local proposal/boost
   pieces and use stable same-function log ratios, compensated differences and
   checked exponent handling for Delta=log(G_alpha)-log(G_beta). Do not first
   restore two rounded scalar logs: a common huge offset can erase a consequential
   difference even when both scalar logs are finite, and two boost quotients can
   individually exceed range while their difference is usable. Require only
   information needed for the final ratio; a private component representation
   suffices, with no public numeric/state/cache framework or arbitrary precision.
   If required sign/magnitude or cancellation cannot be resolved, error instead
   of assuming equality, a zero Gamma, endpoint or deterministic mean.

9. **Restore the ratio with a stable logistic and explicit zero provenance.**
   For nonzero Gamma paths compute t=exp(-abs(Delta)), s=t/(1+t); return s when
   Delta<0 and1-s otherwise. Preserve the smaller tail directly before the one
   final complement, avoiding an overflowing exponential, a lost1+t increment
   in the large branch, or recovery of a small tail from a rounded near-one
   result. A proven extreme difference may yield genuine final binary640/1;
   do not require every unrequested intermediate/log magnitude fit. A single
   tagged exact-zero Gamma gives0/1 as appropriate; two tagged zero paths error
   because the generated ratio is undefined. No extra uniform/Bernoulli draw,
   alpha+beta, Gamma sum, clipping, endpoint mixture or resampling fallback.

10. **Retain bounded work and explicit RNG consumption.** Each Gamma path has
    at most100,000 MT proposal attempts, including rejected bases; accept on the
    last permitted attempt. Shape augmentation has one additional uniform, not
    another nested rejection budget. Nonshortcut beta can therefore consume at
    most200,000 normal trait calls and200,002 uniform trait calls; a failed first
    Gamma does not start the second. Invalid parameters consume0 calls, and
    Beta(1,1) consumes one checked uniform. Malformed draws/arithmetic failures
    abort rather than count as ordinary rejection and retry. These are sampler
    trait-call/proposal bounds, not bounds on an arbitrary RNG's internal work;
    cached normals also change underlying word/uniform consumption. No public
    attempt preset or automatic retry. Errors leave RNG state advanced; no
    rollback, reseed or seeded-sequence compatibility promise.

11. **Require truthful outputs, errors and precision claims.** Successful beta
    is finite in[0,1]. Parameter failures are input/domain errors; malformed RNG,
    numerical/component/logistic failure and proposal exhaustion are computation/
    convergence errors with available stage/shape/attempt context. No successful
    NaN, undeclared infinity, numeric-floor repair or partial draw. Rounded
    endpoints, squeeze/series profiles and a work bound do not certify the exact
    continuous distribution, correct rounding, global tail/total-variation error,
    all-finite-shape success or portable identical bits. Statistical and arithmetic
    qualification remain separate.

12. **Preserve the actual Rust/host boundary and reconcile the model caller.**
    Generic sample_beta remains Rust-only and Gamma private; do not add bindings,
    result classes, schemas or host-only algorithms. The existing BetaRecovery
    model's seeded Python/WASM operation remains its owner-controlled API and
    propagates canonical input/computation errors through centralized mapping.
    Reconcile finite-positive derived-shape admission in construction/restore:
    tiny positive std_dev can currently square to0 and produce infinite shapes.
    Remove stale statrs/near-unreachable-error claims; keep sampling failure
    propagation and no partial seeded vector or mean replacement. Its quantile,
    calibration and wire/model conventions are not selected by this sampler.

13. **Leave RNG and financial-model choices with their owners.** Record17 owns
    general generators/normal transformation, caches, stream/seed assignment and
    endpoint policy outside this sampler; do not silently change its Box–Muller
    floors or all uniform methods here. Financial owners select recovery/severity
    distributions, means/dispersion, bounds, dependence/correlation, calibration,
    scenario/stream and convergence/tail acceptance. A generic Beta draw neither
    chooses a recovery model nor establishes production risk. Removing the
    numerical Bernoulli fallback does not approve a separately modelled endpoint
    mixture, deterministic recovery or Gaussian substitute.

14. **Require independent deterministic and statistical qualification.** Cover
    scripted RNG identities/draw order/zero/malformed outputs, first/last/budget
    cases, MT squeeze/full/series seams and near-boundary bases, stable small/large
    shape/boost/component cancellation, logistic tails and final endpoint ranges.
    Compare bounded/high-precision algebraic references independently of production
    code; separately test distributional shape/quantiles/tails and multiple seeded
    streams with disclosed sampling uncertainty. Moment sanity alone is not a
    tail or all-shape certificate. Reconcile the real BetaRecovery host path and
    declared errors. This documentation review runs no production sampler or
    statistical experiment and changes no runtime, fixtures or dependencies.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Invalid alpha/beta | Input/domain error before any RNG consumption. |
| alpha=beta=1 | One checked uniform draw, preserving its[0,1) endpoint profile. |
| Uniform1/outside/NaN/infinite, or nonfinite normal | Immediate computation error after consumed source call; no floor or retry. |
| Valid MT base<=0 | Ordinary rejection, no acceptance-uniform draw, within the100,000 budget. |
| Positive FMA base with rounded t=-1 | Preserve consistent base/log information or error; do not reject merely from the rounded product. |
| a=float64(1e64), scripted X=float64(1e16),U=float64(.999) | Mathematical R about-.00925926 is below log(U) about-.00100050: reject; rounded-base cancellation must not fabricate acceptance. |
| Acceptance uniform0 at a valid positive MT proposal | Accept by the exact log0 limit without an extra draw. |
| One augmentation uniform0 / both Gamma paths taggedzero | Corresponding beta endpoint / computation error, never a Bernoulli fallback. |
| alpha=beta=1e308, accepted normals0/uniforms.5 | Intended ratio.5 despite raw Gamma-sum and9d overflow; no alpha/(alpha+beta) fallback. |
| alpha=beta=2^-1000, equal positive augmentation uniforms, base Gamma draws2 and1 | Common boost cancels; intended ratio2/3 rather than.5 from rounded scalar logs. |
| Nonzero-U boost logs outside scalar range | Keep joint quotient/difference information; error if unresolved, never tag as an exact zero draw. |
| Delta=1 /-710 | Logistic about.73105857863 /4.47628622568e-309, without exp710 overflow. |
| Delta=37 | Preserve the small component before complementing; no premature1 from rounded1+exp(-37). |
| Delta=-745 /-746 | Final rounding near eta /0 is permitted; no arbitrary tail threshold. |
| Proposal budget exhausted / numerical failure | Computation/convergence error, RNG remains advanced, no reseed/retry/mean fallback. |
| BetaRecovery tiny positive std_dev yielding nonfinite derived shapes | Model constructor/restore reconciliation; generic sampler still validates its own positive finite shapes. |

## Observed choice and rationale

Beta draws are ratios of independent Gamma draws. Gamma shape≥1 uses Marsaglia–Tsang acceptance/rejection; shape<1 uses shape augmentation and a uniform power. The bounded rejection loop protects callers from pathological RNG behavior. The endpoint fallback is an observed numerical approximation, not an accepted economic convention.

## Observed algorithm

1. Validate alpha,beta finite positive. If both equal 1 exactly, return one uniform draw.
2. Gamma shape a<1 returns `Gamma(a+1)*max(U,1e−300)^(1/a)`.
3. For a≥1 set d=a−1/3 and c=1/sqrt(9d). Draw standard normal X, set v=(1+cX)^3 when its base is positive, and draw U. Accept if `U<1−0.0331 X^4` or `log(max(U,1e−300))<X²/2+d[1−v_safe+log(v_safe)]`, with v_safe=max(v,1e−300). Return d v.
4. After 100,000 unsuccessful attempts, return a validation error. A shape-augmentation call shares that same attempt cap.
5. Draw X~Gamma(alpha), Y~Gamma(beta). If X+Y is finite positive, return X/(X+Y). Otherwise draw a Bernoulli endpoint: return 1 if U<alpha/(alpha+beta), else 0.

## Observed selection, controls and conventions

The RNG and seed are caller-selected; rejection consumption is path-dependent. The Gamma sampler is private. There is no option to disable the endpoint approximation or request a log-Gamma beta ratio. Shapes are dimensionless, and Gamma scale is one.

## Observed limitations, failure and fallback

Tiny shapes can underflow both Gamma draws and trigger the endpoint approximation. Large finite shapes can overflow their sum; alpha+beta in the fallback probability can also overflow, making its computed ratio zero. Thus the fallback is not established as uniformly correct over all accepted finite shapes. RNG outputs are trusted; no uniform/normal validation occurs here.

## Observed alternatives and tradeoffs

A log-domain Gamma ratio or specialized beta sampler could avoid the endpoint approximation but is not implemented. The supplied uniform special case avoids unnecessary Gamma work. Fallback changes distributional behavior; its permitted consumer domain is not adopted.

## Evidence

- **Observed source and located tests.** [Implementation and inline tests](../../../../finstack-quant/core/src/math/distributions.rs) expose the raw Gamma ratio, max(U/v,1e-300) floors, literal9*d, per-Gamma100,000 proposal cap and extra Bernoulli fallback. `gamma_rejection_loop_has_a_hard_limit` locates pathological-RNG bounded-failure coverage. Beta/Gamma moment tests are stochastic sanity checks; the small-shape beta test admits endpoints and checks range/mean without demonstrating that a particular endpoint occurred. These tests were inspected/located, not executed, and do not qualify the selected algorithm/RNG/error changes or all extreme shapes.
- **Primary method and identity provenance.** The [Marsaglia–Tsang2000 entry](../../../REFERENCES.md#marsaglia-tsang-2000-gamma) identifies the paper DOI, institutional abstract and published proposal/squeeze/shape-boost profile. The [author-hosted Devroye chapter](../../../REFERENCES.md#devroye-1986-random-variates) gives the independent-Gamma beta-ratio identity (theorem3.1, printed pp402–403) and positive-shape augmentation (printed p420). The latter is a distributional identity, not grounds to label this implementation an Ahrens–Dieter sampler. These ideal-law references do not prescribe the project's finite-RNG zero policy, budgets, component representation or finite-precision accuracy. [GSL Gamma source](https://raw.githubusercontent.com/ampl/gsl/master/randist/gamma.c) corroborates the equivalent c=(1/3)/sqrt(d) form; it is an implementation crosscheck, not qualification of this beta sampler.
- **Derived cancellation profile.** In real arithmetic, the MT log threshold is d*H(t), with H(t)=3log1p(t)-3t+1.5t²-t³ and derivative -3t³/(1+t). Expanding its remainder gives the selected64-term h profile; the geometric tail bound3*abs(t)^64/(68*(1-abs(t))) is below5e-21 for abs(t)<=.5. This is a derived fixed project evaluation profile, not a claim the primary paper specifies this truncation or that its bound controls total floating error/acceptance bias. Stable base/log consistency and final scaling require independent qualification.
- **Independent deterministic references.** On2026-11-08, standard-library Fraction/Decimal calculations independently checked large equal-shape raw-sum overflow versus ratio.5; the remainder identity/sign and64-term bound at t=+/-.5 and a tiny t; cancelling boost components with intended ratio2/3; and logistic/tail rounding. Additional scripted arithmetic at a=float64(1e64), X=float64(1e16),U=float64(.999) gives R=-.00925925925925925881493439 versus log(U)=-.00100050033358353438921047, a rejection that naive rounded-base cancellation can reverse. This is a deterministic source-derived numerical target, not a claim that the production normal generator reached that scripted X or a production statistical experiment.
- **Tail/component reference limits.** Delta1 gives.731058578630004879251159241822; Delta-710 gives4.47628622567512995608316070229e-309. Delta37 requires the smaller-tail-before-complement calculation to avoid premature1; Delta-745/-746 round to eta/0. For equal shapes2^-1000 and equal positive boost uniforms.5, the common log correction is about-7.42713e300, which erases a scalar log2 offset although the joint ratio can be2/3. For a=2^-10,U=247/512 and accepted augmented Gamma4, the power alone is about.13660149eta and rounds0 while the complete product is about.54640597eta and rounds to eta. These motivate component/deferred arithmetic without promising every finite shape/draw resolves or certifying the rounded continuous distribution.
- **Actual RNG and caller boundaries.** [RandomNumberGenerator and PCG](../../../../finstack-quant/core/src/math/random.rs) specify uniform[0,1); PCG's cached Box–Muller path belongs to record17. Every sampler-consumed value needs checking, but the local call budget cannot bound an arbitrary trait implementation's internals or establish independence. The sole external [BetaRecovery caller](../../../../finstack-quant/models/src/credit/lgd/seniority.rs) forwards core errors and its seeded loop returns no partial successful vector. Its constructor can currently generate infinite shapes after a tiny positive std_dev squares to0; constructor/restore admission and stale statrs/near-unreachable-error docs require owner reconciliation. Existing seeded [Python](../../../../finstack-quant-py/src/bindings/models/credit/lgd.rs) and [WASM](../../../../finstack-quant-wasm/src/api/models/credit/lgd.rs) expose that model operation; the [parity contract](../../../../finstack-quant-py/parity_contract.toml) keeps generic distributions Rust-only. No generic binding or recovery/correlation model is newly selected here.
- **Qualification limit.** Read-only independent reviews assessed the mathematical, RNG/work and host/caller package. No production sampler, seeded statistical suite, generator, binding build or financial experiment ran. [Monte Carlo context](../../../REFERENCES.md#glasserman-2004-monte-carlo) remains general context; production distribution/tail and financial acceptance remain open.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
