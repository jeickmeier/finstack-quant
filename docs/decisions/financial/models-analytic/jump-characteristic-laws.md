# MODELS-ANALYTIC-11: Black, Merton jump and variance-gamma characteristic laws

**Decision status:** Accepted — complete checked, forward-normalized Black, Merton and variance-gamma law package for COS, 2026-11-10.

**Implementation status:** Observed. Runtime, consumer and schema reconciliation remain open.

**Validation status:** Source, primary papers and existing tests inspected. Standalone exact, high-precision and conditional-law checks cover selected cases. Production numerical and complete-path qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked characteristic laws

The complete 24-decision package was accepted on 2026-11-10.

1. **Keep one private built-in law interface for COS.** Prepare the
   selected Black–Scholes, Merton or variance-gamma law once per scalar
   request. It supplies checked support, cumulant geometry, centered log
   moments, real-frequency centered transforms and a modulus envelope to
   [record 10](cos-inversion.md#accepted-decision-for-checked-cos-pricing).
   COS consumes c1, c2 and c4. The c3 formulas specify the law for
   qualification without requiring a third cumulant for a price.
   Retire the public `CharacteristicFunction` trait, raw CF parameter
   structs and public `Cumulants`. The named scalar price calls remain
   the production entry points.

2. **Keep units and clocks explicit.** T is nonnegative finite elapsed
   model years. Volatilities are decimal standard deviations per square
   root model year, except Merton jump standard deviation, which is per
   jump. Merton intensity is arrivals per model year. Variance-gamma
   sigma and theta are the Brownian volatility per square root of
   gamma-time years and log drift per gamma-time year, respectively.
   Nu is in model years, with E[G_T]=T and Var(G_T)=nu T.
   Inputs describe the already selected pricing law. The scalar price
   owns deterministic carry and discount inputs. Prepared laws contain
   no r, q, spot, strike or inferred market data.

3. **Use the two selected probability laws.** Under the cash measure Q,
   X=log(S_T/F) with E_Q exp(X)=1. Set s=0, eta=+1 for the cash put law
   and s=1, eta=−1 for the share-reflected call law. Under the selected
   measure use Y=eta X with density exp(s X) relative to Q. For the cash
   log moment K_X, the selected log moment is K_Y(z)=K_X(s+eta z)
   for admitted real z. Both satisfy K_Y(0)=K_Y(1)=0 by the same
   martingale identity. Use these algebraic identities and the selected
   law directly, without an arbitrary complex-frequency API or extracting
   a real component at −i.

4. **Admit Black diffusion with an exact zero.** Require finite
   sigma>=0. V=sigma² T is total variance. Preserve positive V in scaled
   form even when standalone sigma² overflows or underflows.

5. **Admit the explicit Merton jump inputs.** Require finite sigma,
   lambda and sigma_jump, all nonnegative, and finite signed mu_jump.
   The Brownian motion, Poisson count and i.i.d. log jumps are independent.
   The jump multiplier is exp(J), with J normal of mean mu_jump and
   variance j=sigma_jump², including fixed log jumps at j=0. Finite
   primitive inputs specify a mathematical moment law. A standalone
   exp(mu_jump+j/2), annual compensator or tilted intensity need not
   fit binary64 when the required scaled expression can be resolved.

6. **Admit variance-gamma on its strict martingale domain.** Require
   finite positive sigma and nu, and finite signed theta. With
   D(z)=1−nu theta z−nu sigma² z²/2, require mathematical D(1)>0.
   Evaluate the sign from the original inputs with checked scaled or
   compensated arithmetic. Reject a proven zero or negative value.
   Return a computation error when the sign cannot be resolved. Use
   no tolerance band, clipping or substituted denominator.

7. **Select singular laws only from exact inputs.** Validate the
   primitive inputs and variance-gamma domain before the T=0 point
   mass at X=0. At positive T, Black sigma=0 is the same point mass.
   Merton lambda=0 or both mu_jump=0 and sigma_jump=0 reduces to
   Black before evaluating inactive jump arithmetic. Positive random
   scales retain their stochastic law. The nu→0 Black limit qualifies
   variance-gamma, while nu=0 and sigma=0 are outside its selected domain.

8. **Use the exact Black normalized laws.** K_X(z)=V z(z−1)/2.
   Cash and reflected share both give Y normal with mean −V/2 and
   variance V. Their third and fourth cumulants vanish. For centered
   Y, the real log moment is V p²/2 and the log characteristic
   function is −V u²/2. Every finite real p is admitted. For V>0
   the support is the entire real line with no atom.

9. **Use the compensated Merton cash law.** Write Lambda=lambda T,
   m=mu_jump, b=m+j/2 and V=sigma² T. Then
   K_X(z)=V z(z−1)/2+Lambda[expm1(m z+j z²/2)−z expm1(b)].
   This includes the Brownian Itô drift −V/2 and the jump
   compensator −Lambda expm1(b). Correct the existing printed
   drift and preserve the distinction between a log jump and
   its expected relative price change.

10. **Transform the whole Merton jump law under the share measure.**
    For the selected Y use Lambda_s=Lambda exp(s b),
    m_s=eta(m+s j), b_s=m_s+j/2=eta b, with V and j unchanged.
    Substitute Lambda_s, m_s and b_s into the compensated Merton
    expression in decision 9. The share law changes both Poisson
    mass and Gaussian jump mean before reflection. Keep small or
    large positive masses in scale or log form until their use.

11. **Compute Merton cumulants from the selected law.** With
    R(x)=expm1(x)−x evaluated as a stable remainder, use
    c1=−V/2−Lambda_s[j/2+R(b_s)],
    c2=V+Lambda_s(m_s²+j),
    c3=Lambda_s m_s(m_s²+3j), and
    c4=Lambda_s(m_s⁴+6m_s²j+3j²). Use the stable c1 form
    to preserve the drift when the compensation and mean jump
    contributions nearly cancel. Preserve nonnegative c2 and c4.

12. **Use a nonnegative centered Merton log moment.** Every finite
    real p is in the mathematical domain. For
    K_c(p)=log E[exp(p(Y−c1))], use
    K_c(p)=(V+Lambda_s j)p²/2+Lambda_s R(m_s p+jp²/2).
    Evaluate positive products and remainders in scale and supply
    outward bounds for the actual COS moment probes.

13. **Use the centered Merton log characteristic function.** For
    finite real u set a=−j u²/2 and d=m_s u. Its real part is
    −V u²/2+Lambda_s[expm1(a)−2 exp(a) sin²(d/2)]. Its phase is
    Lambda_s[exp(a)(sin(d)−d)+d expm1(a)]. Use stable small
    remainders, checked scaling and qualified trigonometric range
    reduction. The real part is nonpositive by construction.

14. **Preserve the exact Merton support and atoms.** With positive
    V, the law has full real support and no atoms. At V=0 with
    active jumps, the no-jump location is d0=−Lambda_s expm1(b_s)
    and its probability is exp(−Lambda_s). With j>0 the rest is a
    continuous mixture with full real support. With j=0 and
    nonzero m_s the support is the one-sided lattice d0+n m_s,
    n=0,1,2,..., including its endpoint. Determine support from
    exact structural inputs. Small probabilities and small
    positive diffusion carry their original support meaning.

15. **Use the compensated variance-gamma cash law.** For positive
    T, G has Gamma shape alpha=T/nu and scale nu. Conditional on
    G, the uncorrected log increment is normal with mean theta G
    and variance sigma² G. The forward-normalized cash log moment
    is K_X(z)=alpha[z log D(1)−log D(z)] on its admitted domain.
    The positive-time law has full real support and no atom.

16. **Use the selected variance-gamma moment domain.** Write
    d_s=D(s), A=eta nu(theta+s sigma²)/d_s and B=nu sigma²/(2d_s).
    Then D(s+eta z)/d_s=1−A z−B z². The selected positive-time
    real moment domain is strictly 1−A p−B p²>0. The moment
    diverges at its endpoints and beyond. Check the sign in
    selected coordinates without losing p in a rounded s+eta p.
    A proven outside-domain probe may be halved by COS. An
    unresolved sign or failed admitted computation propagates.

17. **Compute variance-gamma cumulants in the selected coordinates.**
    Use c1=alpha[eta log D(1)+A], c2=alpha(A²+2B),
    c3=alpha(2A³+6AB) and c4=alpha(6A⁴+24A²B+12B²).
    For small A+B, the equivalent mean is
    alpha[log1pmx(−(A+B))−B], where log1pmx(x)=log1p(x)−x
    is evaluated stably. Preserve original log geometry near
    the moment boundary or at extreme ratios.

18. **Use a nonnegative centered variance-gamma log moment.** For
    an admitted real p use
    K_c(p)=alpha[B p²−log1pmx(−A p−B p²)]. Evaluate the
    remainder stably and supply outward bounds. Keep B p²,
    alpha and moment-domain geometry in the needed scale.

19. **Fix the variance-gamma logarithm branch and phase.** For
    finite real u set x=B u², d=A u and y=d/(1+x). The selected
    denominator has real part 1+x>0. Use its principal logarithm.
    The centered log characteristic function has real part
    −alpha[log1p(x)+log1p(y²)/2] and phase
    alpha[(atan(y)−y)−d x/(1+x)]. Use stable remainders and
    checked scaled ratios, including near the origin. Keep
    log attenuation separate from phase through reconstruction.

20. **Supply the model-specific COS envelopes.** For Black and
    Merton use H(u)=exp(−V u²/2), including H=1 at zero
    diffusion. For variance-gamma use H(u)=(1+B u²)^−alpha.
    These are nonincreasing in |u| and bound the centered
    characteristic-function modulus. Supply the selected
    scaled coefficients for record 10's conservative arithmetic.

21. **Preserve exact origins and fail explicitly at numerical limits.**
    Validate inputs before returning the exact zero log transform
    at u=0 or p=0, or at an admitted point mass. Preserve conjugacy
    at real frequencies and usable nonzero geometry and attenuation
    through the weighted COS term. Distinguish a justified numerical
    zero component from lost positive magnitude. Use checked
    arithmetic and qualified argument reduction. Return a computation
    error for unresolved required geometry, phase, enclosure or
    range. No NaN, clipping, moment floor or zero-value recovery.
    These rules give no universal correct-rounding guarantee.

22. **Keep Rust canonical across the actual consumers.** Use one
    private preparation route with no unchecked public or serialized
    raw-law state. Remove the standalone raw CF schema registrations
    and generated types. Reconcile the COS benchmark, scalar wrappers,
    strict market/config wire contracts, Python/WASM exports and
    declarations, parity ledger and both pricing notebooks with the
    accepted COS record. Hosts perform conversion and propagate Rust
    validation and computation errors. Add no host transform API.

23. **Keep the law and pricing owner boundaries explicit.** This
    record owns pricing-law admission, transforms, moments and support.
    Record 10 owns payoffs, tolerance, spectral work, convergence
    and premium acceptance. Heston and rough Heston use their own
    transform and integration paths. Calibration, dated model-time
    and carry preparation, path simulation and market parameter
    sourcing retain their existing owners.

24. **Qualify the selected laws and actual COS route.** Compare both
    contours against independent Gaussian, conditional Poisson–Gaussian
    and Gamma-mixture or bilateral Gamma references. Check the strict
    variance-gamma moment edges, exact zero cases, rare and small jumps,
    cancellation and mixed-scale inputs, cumulants, conjugacy, support
    and modulus envelopes. Qualify outward moment bounds and the
    resulting scalar price and error behavior across existing hosts.
    Current same-kernel differencing and serde tests supply limited
    evidence for these contracts.

## Independent bounded reference checks

- Black with sigma=2^-600 and T=2^500 has V=2^-700. With
  sigma=2^600 and T=2^-500 it has V=2^700. In both cases the
  complete variance fits while the binary64 sigma² intermediate
  loses its range.
- For Merton with T=1, r=q=0, sigma=sigma_jump=0, lambda=1e34 and
  mu_jump=1e−18, high-precision cash values give c1 near −0.005,
  c2 near 0.01 and K_c(0.6) near 0.0018. The current `exp(x)−1`
  loses the compensator at that jump size, giving a mean near
  1e16. Using `expm1` alone still leaves a cancellation in c1.
  At lambda=1e−300 and mu_jump=750, the compensator is about
  5.25849e25 despite standalone exponential overflow.
- With T=1.25, sigma=0.2, lambda=0.35, mu_jump=−0.03 and
  sigma_jump=0.08, a conditional Gaussian Poisson sum gives
  cash variance 0.05319375 and reflected-share variance
  0.0529631829873. The centered transforms at u=2.5 are
  approximately 0.846945568621+0.000575817333i and
  0.847545658657−0.000433853545i, respectively.
- For variance-gamma with nu=sigma=1 and theta the binary64
  predecessor of 0.5, exact D(1)=2^-54. The current evaluation
  rounds it to zero. Theta=0.5 is the actual excluded boundary.
  With sigma=2^512, nu=T=2^-1023 and theta=−2^1023, D(1)=1,
  c2=3 and c4=42 despite standalone sigma² overflow.
- With variance-gamma nu=T=1, sigma=0.5 and theta=−0.25, the
  Gamma factors are exponential. The cash scales are 1/4 and
  1/2. The reflected-share scales are 1/3 and 1/3. The selected
  moment domains are (−2,4) and (−3,3). Their variances are
  5/16 and 2/9, and fourth cumulants 51/128 and 4/27.
  Independent bounded payoff integration gives 128/729 in
  both selected measures at S=K=1 and r=q=0.

## Observed scope and input domain

Risk-neutral log-return characteristic laws feeding COS. BS stores finite-domain responsibility in its wrapper; Merton validates nonnegative diffusion/jump intensity/jump std; VG requires positive σ,ν and finite rates/θ.

## Observed choice and rationale

BS Gaussian diffusion, compound-Poisson lognormal jumps, and gamma-time-changed Brownian returns are explicit choices. Their drift compensators preserve the intended forward moment; no calibration is embedded here.

## Observed algorithm

BS exponent is iu(r−q−σ²/2)T−σ²u²T/2. Merton adds λT[exp(iuμJ−σJ²u²/2)−1] and subtracts iuλ[exp(μJ+σJ²/2)−1]T; c1 includes λμJ, c2=σ²T+λT(σJ²+μJ²), c3/c4 are λT times third/fourth raw jump moments. VG φ=exp{iu(r−q+ω)T−(T/ν)ln[1−iuθν+σ²νu²/2]}, ω=ln(1−θν−σ²ν/2)/ν. VG cumulants include c1=(r−q+ω+θ)T, c2=(σ²+νθ²)T and analytic higher moments. These cumulants determine the COS window, not just report statistics.

## Observed selection and controls

Merton wrapper calls validate, including finite compensator. VG constructor/validate requires martingale argument>0. Public struct literals and derived serde can bypass validation; CF methods themselves mostly propagate invalid arithmetic. VG ω returns NaN for a nonpositive argument.

## Observed conventions

λ is jumps/year; μJ and σJ describe log jumps, not arithmetic mean/std. ν is gamma-time variance parameter and θ Brownian drift under time change. Cumulants are of log returns and carry annual-time scaling.

## Observed limitations, failure and fallback

Merton doc formula omits −σ²/2 in its printed drift while implementation includes it. Degenerate zero-jump/diffusion inputs may validate in Merton but later fail COS window construction. No jumps in rates/carry are modeled.

## Alternatives and tradeoffs

The three laws are supported alternatives through distinct wrapper functions. Other Lévy laws require an explicit characteristic-function implementation; no model is inferred from data.

## Implementation and validation evidence

[black_scholes.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/black_scholes.rs), [merton.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/merton.rs), [variance_gamma.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/variance_gamma.rs) and [mod.rs](../../../../finstack-quant/models/src/fourier/characteristic_function/mod.rs). Module tests compare analytic cumulants to finite differences and λ→0 Merton to BS; these are identity/limit checks. References: [Merton jumps](../../../REFERENCES.md#merton-1976-jump), [Variance gamma](../../../REFERENCES.md#madan-carr-chang-1998).

Actual uses include the [COS engine](../../../../finstack-quant/models/src/fourier/cos.rs),
[Fourier benchmark](../../../../finstack-quant/models/benches/fourier_pricing.rs)
and [schema registry](../../../../finstack-quant/models/src/schema.rs).
The [Python](../../../../finstack-quant-py/src/bindings/models/fourier.rs) and
[WASM](../../../../finstack-quant-wasm/src/api/models/fourier.rs) modules
expose the scalar pricing entries. The
[Fourier pricing notebook](../../../../finstack-quant-py/examples/notebooks/02_pricing/instruments/fourier_pricing.ipynb)
and [equity options notebook](../../../../finstack-quant-py/examples/notebooks/02_pricing/instruments/equity_and_options.ipynb)
use those entries. See the [model-law references](../../../REFERENCES.md#normalized-jump-characteristic-laws).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
