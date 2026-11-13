# MODELS-ANALYTIC-08: Heston affine characteristic function and parameter domain

**Decision status:** Accepted — complete checked Heston probability contour and affine logarithm package, 2026-11-10.

**Implementation status:** Observed. Reconciliation covers the affine evaluator, shared parameters and existing consumers below.

**Validation status:** Source, primary references and existing price fixtures inspected. Independent bounded calculations cover selected transforms and failure cases. Production and complete-path qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for the checked Heston transform

Accepted on **2026-11-10** after review of all 22 decisions.

1. **Select the pricing-measure Heston law.** Use
   dS/S=(r−q)dt+sqrt(v)dW_S and
   dv=kappa(theta−v)dt+sigma_v sqrt(v)dW_v, with Brownian correlation rho.
   The variance parameters are already qualified in the pricing measure.
   Initial and long-run variance v0 and theta use instantaneous annualized
   variance units, and kappa is per model year. The coefficient sigma_v
   multiplying sqrt(v) has square-root variance-level units per square root
   of a model year. Calibration and physical-to-pricing parameter
   selection remain with their owners.

2. **Use the carry-normalized log coordinate.** For the model's deterministic
   forward F=S0 exp((r−q)T), let X_T=log(S_T/F). The cash-measure contour P2
   uses E[exp(iuX_T)]. The share contour P1 uses E[exp(X_T)exp(iuX_T)],
   the tilt under the dividend-reinvested share numeraire. The stock
   first moment is finite and normalized to one under this law, including
   kappa−rho sigma_v<=0. No spot, strike, carry or discount enters the
   normalized evaluator. Actual pricing restores the required phase and scale.

3. **Admit only the two required contours.** Use a crate-private typed
   cash/share selector and finite real frequency u of either sign on each.
   These correspond to the cash transform at z=u and z=u−i, respectively.
   Remove the integer selector and implicit P2 default. General complex
   arguments and damping moments require their own maturity-specific
   admission. Feller status and a finite formula value supply no such
   moment test.

4. **Select explicit parameter boundaries.** Require finite v0>=0,
   kappa>0, theta>0, sigma_v>=0 and −1<rho<1, with finite T>=0
   in a compatible model-year clock. Admit initial variance zero and the
   exact deterministic-variance branch. Keep kappa=0, theta=0 and
   correlation endpoints outside this selected profile. Validate every
   relevant input before shortcuts. No perturbation moves a boundary
   value into the supported domain.

5. **Make shared model parameters invariant-preserving.** Retain one
   immutable validated HestonParams with checked construction and
   read-only access. Remove the economic Default and public field mutation.
   Require explicit fields and validate on restoration, rejecting missing,
   duplicate and unknown fields. Move current literals and direct mutation
   through checked reconstruction.

6. **Keep market parameters in the pricing wrapper.** Retain validated
   HestonPricingParams with finite signed r and q and the admitted model
   block. Construct it from those rates and the checked model. Preserve
   the existing strict flat wire fields for the pricing request.
   The normalized transform reads the model block and T only. Dates,
   carry, discount and source selection remain with the price owner.

7. **Use Feller as an explicit inclusive diagnostic.** Report whether
   2 kappa theta >= sigma_v², including equality, with a range-safe
   exact comparison of represented inputs. The diagnostic concerns
   accessibility of zero from a positive variance start. It supplies
   no simulation or Fourier accuracy certificate. Remove constructor
   warnings and the test-only require_feller wrapper. The standalone
   Rust and host predicate requires finite kappa>0, theta>=0 and
   sigma_v>=0 and returns a checked Boolean. Share its exact comparator
   with CIR after CIR's own admission, including its theta=0 cases.
   Numerical overflow supplies no positive diagnostic.

8. **Define the exact origin and singleton.** After admission, u=0 or T=0
   yields normalized characteristic value one and affine logarithm zero
   on either contour. A zero variance start at T>0 still uses the selected
   law. The origin supplies no price or settlement shortcut.

9. **Handle exact zero vol-of-vol as the same model limit.** With
   sigma_v=0 the variance path is deterministic. If I is its integrated
   variance, the affine logarithm is (−u²/2+i a_j u) I, with a_1=+1/2
   and a_2=−1/2. This is exact for sigma_v=0. Any positive sigma_v
   uses the stochastic affine law.

10. **Preserve the two integrated variance weights.** For x=kappa T,
    I=T[v0 A(x)+theta B(x)], where A(x)=−expm1(−x)/x and
    B(x)=(x+expm1(−x))/x, with limits A(0)=1 and B(0)=0.
    Evaluate the nonnegative weights directly and keep their scales
    through multiplication, especially for small x. For positive
    sigma_v the same expression is expected integrated cash-measure
    variance. Retire the unused public average-variance helper and keep
    this arithmetic private. The expectation supplies no Gaussian
    replacement for stochastic variance.

11. **Define the affine logarithm by the Riccati law.** For contour j set
    b_1=kappa−rho sigma_v, b_2=kappa, a_j as above,
    Q=−u²+2i a_j u and B=b_j−i rho sigma_v u.
    With D(0)=A_c(0)=0, solve
    D′=Q/2−B D+sigma_v² D²/2 and A_c′=kappa theta D.
    The target is L=A_c(T)+D(T)v0. The same constant kappa theta
    enters both contours even when b_1 is zero or negative.

12. **Use the regular principal root on the actual domain.** Set
    d=sqrt(B²−sigma_v² Q), choosing positive real part in the regular
    case. The real part of the discriminant is
    b_j²+sigma_v²(1−rho²)u², strictly positive for sigma_v>0
    and u≠0 in the selected domain. Compute its scaled real and
    imaginary terms without losing the positive part to cancellation.
    Zero frequency and exact zero vol-of-vol already have their
    specified branches.

13. **Evaluate the decaying affine form without the g ratio.** Put
    J=−expm1(−dT)/d and Z=1+(B−d)J/2, equivalently
    Z=exp(−dT)+(B+d)J/2. Then
    D(T)=Q J/(2Z) and
    A_c(T)=kappa theta [(B−d)T−2 Log(Z)]/sigma_v².
    Use stable equivalent denominators and rationalize a cancelling
    B−d or B+d using their product sigma_v² Q when the other factor
    is resolved. This avoids the artificial g pole near the share
    origin. Remove the fixed 1e−8 relative denominator cutoff.

14. **Preserve the exact little-trap logarithm branch.** Use the
    principal Log(Z) of the decaying formulation on the admitted
    contours, consistent with the continuous Riccati branch from
    T=0. The argument is the full ratio Z, rather than a difference
    of two independently wrapped principal logs. Do not assume
    |g|<1, select an arbitrary branch or track rotations between
    quadrature nodes. Unresolved branch geometry raises a computation error.

15. **Preserve short-time and small-vol-of-vol terms.** Use scaled
    time variables, complex expm1 and stable log1p remainders. With
    W=(B−d)J/2, the A_c numerator is equivalently
    (B−d)(T−J)+2[W−Log(1+W)]. Evaluate the small remainders in
    their scaled form. Use the resolved full branch for a small
    denominator, including the alternative Z in point 13. Retain
    positive stochastic vol-of-vol and refuse unresolved arithmetic
    rather than substituting the deterministic formula.

16. **Use the true near-origin slopes as reference.** Let
    H_b=∫_0^T exp(−b_j s) ds and
    G_b=∫_0^T (T−s)exp(−b_j s) ds. The derivative of L at u=0 is
    i a_j (v0 H_b+kappa theta G_b). For b_j=0 the weights are
    T and T²/2. The same finite-horizon definition applies to negative
    b_1. The price owner determines how to evaluate a Fourier endpoint.

17. **Return one checked affine logarithm.** Use a typed internal
    `heston_log_cf` returning Result<Complex64>. The result is L
    from the Riccati law, with finite components and nonpositive
    real part, rather than a wrapped logarithm of a rounded
    characteristic value. Remove HestonCfStatus, corrupt zero
    sentinels and the exponent-700 guard. Invalid inputs use
    validation errors. Unresolved range, phase or bound violations
    use computation errors without projection or clamping.

18. **Keep resolved component and downstream scales distinct.** At
    materialization exp(L) has modulus at most one and opposite real
    frequencies are conjugates. Preserve representable components.
    A complex value can round to zero when both final components
    underflow. Squared-norm underflow supplies no such evidence.
    Keep the checked logarithm available through phase and weighting
    as needed. A standalone rounded zero alone cannot authorize
    omission of a weighted quadrature contribution.

19. **Reconcile both actual Fourier paths.** Use the same checked
    logarithm in cached strip and direct node evaluation. Propagate
    failure instead of counting corrupted zeros or skipping nonfinite
    contributions. Apply the price owner's phase using log(F/K) with
    the established sign. Tail, endpoint integration, grid, exact
    deterministic price evaluation and price acceptance remain with
    [record 09](heston-fourier-integration.md).

20. **Reconcile shared parameter consumers with explicit domains.**
    Use checked state at the actual equity market resolution, PDE,
    classical MC and rough-Heston callers. Preserve strictly positive
    v0 and sigma_v at classical MC pricing and path simulation, PDE
    and rough Heston until those owners independently qualify broader
    inputs. Add owner admission wherever the old constructor supplied
    those checks. Discretization and calibration choices remain with
    their owners. Restore the embedded rough model through validated
    construction. The classical Monte Carlo owner has selected the two
    zero-input limits for its checked implementation, with runtime and
    product qualification pending.
    [Accepted classical Heston QE-M](../models-monte-carlo/09-heston-qe.md#accepted-decision-for-checked-heston-qe-m-paths).

21. **Update existing hosts without exposing the internal transform.**
    Propagate the admitted fields and validation through the Rust,
    Python and WASM price and diagnostic surfaces, schema, parity and
    examples. The price owner retains public Fourier method settings
    and payoff selection. CIR and MC callers retain their own
    validated process inputs.

22. **Qualify the formula and its consumers separately.** Compare
    independent Riccati solutions, normalization and conjugacy,
    both share-drift signs and zero, Feller equality and non-Feller
    parameters, exact and small positive vol-of-vol, initial
    variance zero, branch continuity and numerical range. Compare
    independently generated price evidence under the price record.
    Test actual parameter restoration and algorithm admission in
    the existing hosts and owners.

## Independent bounded reference checks

- With kappa=0.125, theta=1, sigma_v=0.5, rho=0.5, v0=0.04
  and T=1, Feller is exactly at equality while b_1=−0.125.
  The share transform is about 1+5.39e−14 i at u=1e−12.
  The old g denominator is about 1e−12 in magnitude and trips
  its relative 1e−8 gate.
- With v0=theta=800, kappa=1, sigma_v=2^-40, rho=0 and T=u=1,
  the normalized magnitude is about exp(−400). Its binary64
  components fit while their squared norm rounds to zero.
- With kappa=1, theta=2^200, sigma_v=1, rho=0.5, v0=1,
  T=2^-100 and u=2, the cash logarithm is about −1−0.5 i.
  The leading term in the affine constant is second order in T
  before multiplying by kappa theta. Subtracting rounded first-order
  terms can erase it even when the separate expm1 calls are stable.
- In the deterministic variance example v0=1, theta=2^100,
  kappa=2^-100 and T=1, I is about 1.5. The observed
  theta+(v0−theta)A expression rounds to zero when A rounds to one.
- With kappa=2^1023, theta=2^-1023 and sigma_v=2, the exact
  Feller comparison is 2<4. Multiplying 2 kappa first
  overflows and the unchecked comparison reports true.
- With kappa=theta=sigma_v=1 and rho=0, Feller holds. The
  second stock moment has Riccati equation D′=1−D+D²/2
  and explosion time 3π/2. The two selected contours remain
  finite at T=5. This distinguishes higher-moment admission
  from the selected transform and Feller diagnostic.
- For v0=0.04, kappa=1.2, theta=0.07, sigma_v=0.8, rho=−0.65,
  T=1.7 and u=2.5, an independent Riccati integration gives centered
  P1 about 0.838316830407+0.126196705081 i and P2 about
  0.782882070479+0.019203976927 i.

## Observed scope and input domain

Heston variance diffusion with positive v0,κ,θ,σv and −1<ρ<1. Fourier parameters accept finite r,q; constructor/serde validate, but public fields can be mutated.

## Observed choice and rationale

The two probability characteristic functions use a little-Heston-trap form, stable algebra for small vol-of-vol and series for small complex arguments. Feller violation warns but does not reject; callers may explicitly require Feller.

## Observed algorithm

For P1 choose u=1/2,b=κ−ρσv and for P2 u=−1/2,b=κ. Compute
d=√[(iρσvφ−b)²−σv²(−φ²+2iuφ)] and denominator b−iρσvφ+d.
Express the difference and g through division. Evolve with exp(−dT) and
use fifth-order expm1/log1p-related series when norms<1e−4. Exponent
C+Dv0+iφ ln S gives the CF. φ=0 returns one. A relative denominator
threshold 1e−8, nonfinite intermediate or real exponent>700 reports
Overflow with zero value. A zero squared norm reports Underflow and
returns zero. `deterministic_avg_variance` integrates mean-reverting
expected variance and uses a second-order small-κT series below 1e−6.

## Observed selection and controls

`heston_pj_characteristic_function` selects j=1 or otherwise P2; it does not independently validate all scalar inputs. Pricing wrappers validate before calling it. Defaults v0=θ=.04,κ=2,σv=.3,ρ=−.5. `require_feller` is opt-in, with condition delegated to the shared MC process helper.

## Observed conventions

v0 and θ are annualized variances, κ inverse years, σv variance-process diffusion parameter, ρ Brownian correlation. P1 and P2 belong to different measures and are not the same generic log-return CF used by COS.

## Observed limitations, failure and fallback

Exact σv=0 is rejected by parameter validation although the stable formula supports a limiting sequence. Public CF invalid j silently selects P2. Overflow/underflow status must be honored; zero returned on overflow is not an economic value.

## Alternatives and tradeoffs

Original Heston complex-log forms and alternative branch tracking are not selected. Deterministic-average variance is a helper, not an automatic Black fallback in the Fourier pricer.

## Implementation and validation evidence

[heston.rs](../../../../finstack-quant/models/src/volatility/heston.rs) and [params.rs](../../../../finstack-quant/models/src/closed_form/heston/params.rs). [tests.rs](../../../../finstack-quant/models/src/closed_form/heston/tests.rs) checks zero frequency, limits and corruption; [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) has independent Heston price fixtures. References: [Heston](../../../REFERENCES.md#heston-1993), [Little Heston Trap](../../../REFERENCES.md#albrecher-2007-little-heston-trap).

The live [strip and direct Fourier code](../../../../finstack-quant/models/src/closed_form/heston/strip_pricer.rs)
consumes the current status and zero sentinel. The
[dated parameter resolver](../../../../finstack-quant/valuations/core/src/instruments/pricing/heston_market.rs)
reads the five model scalars. The
[Feller helper](../../../../finstack-quant/models/src/monte_carlo/process/heston.rs)
is shared by CIR and host diagnostics. The
[rough MC restore path](../../../../finstack-quant/models/src/monte_carlo/process/rough_heston.rs)
currently constructs the model block directly. Primary analysis of the
logarithm branch and moment domain is indexed in the
[Heston transform references](../../../REFERENCES.md#heston-transform-branch-and-moments).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
