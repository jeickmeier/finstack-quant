# MODELS-AND-VALUATIONS-CREDIT-09: Copula thresholds, factor conditioning and numerical integration

- Decision status: **Accepted — complete checked copula laws and latent factors package, 2026-11-12**.
- Implementation status: Current behavior observed in the source below. The accepted contract requires Rust, host and product reconciliation.
- Validation status: Independent distribution, quadrature and factor-coordinate examples evaluated. Production, host and product qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked copula laws and latent factors

1. **Select a checked copula and preserve marginal probability.** Require
   explicit Gaussian, Student-t, random factor loading (RFL), or two-factor
   Gaussian selection. Remove the generic Gaussian default and clamping
   constructors. Admit finite marginal PD `p` and latent correlation `ρ`
   in `[0,1]`. Use one canonical preparation of the marginal and the
   family-specific threshold. Interior PD uses the checked normal or
   Student-t inverse. The prepared endpoints `p=0` and `p=1` yield literal
   default probabilities and decisions after validating applicable state.
   A threshold query may represent these endpoints as `−∞` and `+∞`.
   The conditional and event APIs take the marginal or its prepared value,
   preserving exact independence and endpoint meaning across hosts. Use
   complete family-typed factor inputs, with checked shapes, ranges and
   arithmetic. Remove the unchecked parallel operations, probability floors,
   `±10` CDF argument clipping and missing-factor substitutes. Conditional
   probabilities and simulated events share the same latent geometry and
   inclusive threshold. The caller supplies the period and probability basis.
   [Li](https://www.ressources-actuarielles.net/EXT/ISFA/1226.nsf/0/34e84cb615c8b4eac12575fe006a9759/%24FILE/li.defaultcorrelation.pdf),
   [checked distribution functions](../core-numerics/distribution-functions.md),
   [period default resolution](07-pool-defaults.md).

2. **Use the one-factor Gaussian law at its full domain.** With finite
   standard normal factor `Z`, threshold `c=Φ⁻¹(p)` and independent
   standard normal residual `ε`, the latent is `sqrt(ρ) Z+sqrt(1−ρ) ε`.
   Conditional PD is `Φ((c−sqrt(ρ) Z)/sqrt(1−ρ))` for interior `ρ`.
   At `ρ=0` return the exact marginal in conditional mode. At `ρ=1` use
   the inclusive event `Z<=c` with no residual draw. Evaluate scaled
   comparisons and normal tails from the complete geometry. A legitimately
   rounded probability endpoint can result from a finite tail. Unresolved
   arithmetic fails rather than producing a substitute probability.

3. **Keep both Student-t conditioning inputs explicit.** Require finite
   degrees of freedom `2<ν<=1e8`, consistent with the accepted pool and
   mixing profiles. Use threshold `c=t_ν⁻¹(p)`, common normal `Z` and
   positive finite common scale `W=χ²_ν/ν`, independent of `Z` and the
   residuals. Generate `W` from the checked chi-squared inverse on an
   open uniform. The latent is `(sqrt(ρ) Z+sqrt(1−ρ) ε)/sqrt(W)`.
   Compare its numerator with `c sqrt(W)` using checked scaled arithmetic.
   Conditional PD uses the normal CDF of
   `(c sqrt(W)−sqrt(ρ) Z)/sqrt(1−ρ)`, with the inclusive deterministic
   branch at `ρ=1`. A representable event decision can succeed even if a
   separately requested latent level is out of range. At `ρ=0` the shared
   scale still induces dependence. Retire the overloaded single-coordinate
   `Z/sqrt(W)` route. Reuse the same common scale across names, and across
   antithetic partners under the accepted pool replay contract.
   [Demarta and McNeil](https://onlinelibrary.wiley.com/doi/10.1111/j.1751-5823.2005.tb00254.x),
   [checked chi-squared mixing](../core-numerics/chi-squared-inversion.md).

4. **Calibrate the shared random loading with a checked residual.** Retain
   the project law `B=clip(μ+σ η,0,1)`, with `σ` in `[0,.5]` and one common
   standard normal loading shock `η`, independent of `Z` and all residuals.
   Solve for location `μ` so that `E[B²]=ρ` for the common-loading pair.
   Exact `ρ=0` and `ρ=1` select constant loadings. At `σ=0` use the Gaussian
   law with the original `ρ`. For interior `ρ` and positive `σ` retain
   the finite bracket `[−40σ,1+40σ]`, check its signs, and use checked
   Brent with explicit positive coordinate tolerance and iteration limit.
   Use stable clipped normal moments. For `ρ<=.5` solve using `log E[B²]`
   against `log(ρ)`. For `ρ>.5` use `log E[1−B²]` against `log1p(−ρ)`.
   For Brent, map the signed log difference `D` to the finite residual
   `sign(D)*[−expm1(−abs(D))]`. An endpoint whose moment is established
   below numerical range can use the corresponding limiting residual.
   The complementary moment can be evaluated as `2E[1−B]−E[(1−B)²]`
   from its own clipped-normal moments. The monotone objectives are
   mathematically determined by the respective integrals of
   `2t Φ((μ−t)/σ)` and `2t Φ((t−μ)/σ)` on `[0,1]`. Check the absolute
   log residual on the returned location against `1e−8`. Store one
   immutable preparation per admitted correlation and volatility and
   propagate unresolved tails, bracket or convergence failures. Remove
   the mutable last-correlation cache and NaN fallback. Matching this
   moment matches latent correlation under the stated law. The published
   Andersen–Sidenius model motivates random loading. This project's shared
   independent clipped-normal law has its own specification and a loading
   distribution that is independent of the market factor.
   [Andersen and Sidenius](https://www.maths.univ-evry.fr/pages_perso/crepey/Credit/gaussextensionsAndersenSilenius.pdf),
   [checked Brent](../core-numerics/brent-root.md).

5. **Use the same prepared loading for RFL probabilities and events.**
   Share one `η` across names when the loading is random, using the
   checked normal inverse on an open uniform for the pool draw. Conditional
   PD and default events use the latent `BZ+sqrt((1−B)(1+B)) ε` and the
   normal threshold. At an exact zero loading the conditional probability
   is the marginal. An actual unit-loading atom uses `Z<=c`. Resolve the
   clipping comparison before rounding an interior `B` to an endpoint,
   retaining a usable residual scale or returning an error. A deterministic
   loading requires no loading draw. Remove the two-sigma stress proxy from
   the copula API. A calibrated loading moment by itself establishes no
   default-indicator correlation or tranche fit.

6. **Make the shared two-factor Gaussian split explicit.** Rename the
   generic `MultiFactor` copula to `TwoFactorGaussian`. Require sector
   share `s` in `[0,1]` and explicit finite independent standard normal
   global and sector realizations `Z_g,Z_s`. The common component is
   `sqrt(ρ(1−s)) Z_g+sqrt(ρ s) Z_s`. Residual variance is `1−ρ`, derived
   from admitted `ρ` before the rounded weights. The conditional normal
   and the latent event use the same component and exact zero-residual
   handling. Remove the implicit 60/40 split and single-factor projection.
   This kernel shares both common coordinates across its modeled names.
   Sector membership, different sector shocks and arbitrary exposure
   loading vectors require their own authored model and caller. The
   CREDIT-07 single-systematic route refuses this two-factor geometry.
   [Authored pool dependence](08-prepayment-and-pool-correlation.md),
   [portfolio exposure loadings](10-portfolio-loss-and-correlated-recovery.md).

7. **Report the actual tail dependence of the specified pair.** Validate
   the family and correlation before returning the common lower and upper
   coefficient of these symmetric laws. Gaussian and shared two-factor
   Gaussian give zero for `ρ<1` and one for `ρ=1`. For Student-t use
   `2 t_(ν+1)(−sqrt((ν+1)(1−ρ)/(1+ρ)))` for `ρ<1` and exact one at
   `ρ=1`. For random RFL the coefficient is the unit-loading mass
   `Φ((μ−1)/σ)`. Conditional Gaussian components below unit loading
   contribute zero to the limit. Constant RFL uses the Gaussian result.
   The result describes the specified pair sharing the applicable factors.
   It is fallible and admits no clipped input or NaN sentinel.
   [Demarta and McNeil](https://onlinelibrary.wiley.com/doi/10.1111/j.1751-5823.2005.tb00254.x).

8. **Use one checked fixed quadrature path for scalar and paired expectations.**
   Select supported Hermite orders explicitly from `5,7,10,15,20` for
   normal axes. Integrate Gaussian and constant RFL over `Z`, random RFL
   over independent `η,Z`, and two-factor Gaussian over `Z_g,Z_s`.
   Student-t integrates independent `W,Z`, with an explicit Laguerre
   order in `10..=64`. These are the outer-to-inner traversal orders.
   Validate the tensor count and work before callbacks, and traverse sorted
   nodes. Use one fallible callback invocation
   per node, including paired requests, with independent checked weighted
   compensated totals for each component. Stop at the first error or
   nonfinite sample. Apply the normalized measure weights once and check
   finite results. Report the selected rule and evaluated node count.
   Fixed quadrature approximates the factor expectation. Order agreement
   and polynomial exactness alone do not qualify a tail or discontinuity.
   [Checked quadrature](../core-numerics/quadrature.md).

9. **Prepare Student-t mixing quadrature only when requested.** Write
   `k=ν/2`, so `W` has Gamma shape `k` and scale `1/k`. In `W` coordinates
   the symmetric Jacobi matrix has diagonal `1+2i/k` and off-diagonal
   `sqrt((i/k)*(1+(i−1)/k))` between `i−1` and `i` for `i>=1`. Use the
   accepted bounded symmetric eigensolver with explicit controls. Use
   its sorted eigenvalues as nodes and squared first eigenvector
   components as probability weights. Require finite positive, strictly
   increasing nodes and finite positive weights. Check weighted mass
   and mean against one within `1e−12` and centered variance against
   `2/ν` within `1e−8` relative error. These are numerical admission checks.
   Remove order clamping and eager eigensolving in threshold and simulation
   construction. Reject unusable eigenpairs or lost weights rather than
   dropping nodes or changing rules. The weights include the Gamma
   measure, so apply no additional density multiplier.
   [Golub and Welsch](https://doi.org/10.1090/S0025-5718-69-99647-1),
   [checked symmetric eigensolving](../core-numerics/matrix-factorization.md).

10. **Keep one checked whole-vector latent-factor operation.** Require
    explicit finite nonnegative scales, ordered labels and validated
    standardized correlation. For one factor apply its scale. For the
    prepayment and credit pair use the CREDIT-08 credit-first innovation
    order and named result coordinates, with exact signed correlation
    endpoints and each supplied scale applied to its own output. For an
    authored matrix, use the accepted `PsdFactor` with explicit numerical
    PSD tolerance and preserve both original variable and innovation
    order. Apply `diag(scales) F z` with checked scaling, dot products and
    full input validation, including when a scale is zero. A zero-scaled
    output has undefined Pearson correlation. The standardized matrix
    remains available. For a loading vector `β` on correlated factors,
    the systematic variance uses `βᵀRβ` on standardized coordinates.
    Remove the diagonal-only contribution, implicit one-factor defaults
    and RMBS/CLO presets. Static scales describe the draw transform.
    Mean reversion, time units and actual innovation processes belong to
    the product. Preserve explicit persistence when switching factor
    coupling. [Checked matrix factors](../core-numerics/matrix-factorization.md),
    [product factor coupling](08-prepayment-and-pool-correlation.md).

11. **Reconcile the checked kernels with their live consumers.** Use
    validated immutable state on direct and restored inputs. Reconcile the
    existing Python and WASM copula and factor handles, declarations,
    schemas, parity and product payloads with the Rust authority. Keep
    quadrature callbacks in Rust. Pool pricing uses one prepared period
    law under CREDIT-07. Portfolio loss preserves its authored per-name
    loading vectors and shared scale where selected under CREDIT-10.
    Tranche valuation propagates threshold and integration errors and
    passes the selected law through risk. Tranche selection, factor-rule
    convergence, known discontinuities, truncation and conditional loss
    budgets remain with the tranche owner. For adaptive normal integration,
    apply the checked numerical budgets and account for omitted factor
    mass under the actual payoff bound. Known discontinuities include
    unit-correlation thresholds, RFL loading breakpoints and zero-residual
    two-factor thresholds. Reconcile explicit product configuration before
    removing implicit dispatch paths. [Tranche model selection](0031-tranche-loss-model-selection.md),
    [tranche loss](17-tranche-loss-distribution.md), [tranche risk](19-tranche-risk.md).

## Observed behavior

- Generic selection defaults to Gaussian. Raw conditionals can replace
  malformed state with an unconditional probability or substitute a
  factor. The checked host methods accept a finite raw threshold. Several
  Gaussian conditional routes clip CDF arguments and report tail dependence
  zero at perfect correlation. Student-t also accepts a collapsed one-slot
  systematic variate. RFL uses a mutable location cache and can return NaN.
- Student-t builds its mixing quadrature at construction with an unbounded
  eigensolve and clamped order. Generic paired expectations can evaluate
  callbacks twice, while the Student-t pair shares nodes but accumulates
  before Hermite normalization. Tranche routes select adaptive normal
  integration or fixed Student-t quadrature and can turn threshold errors
  into NaN. The generic two-factor law fixes its sector share at `.40`.
- Generic latent-factor state contains default and calibration presets.
  Multi-factor application can discard a factor error and scale unchecked.
  The structured-credit caller uses single-factor mean reversion and the
  two-factor correlation, with zero mean reversion under the current
  two-factor path. Its standardized factors do not use the generic scale
  fields. Portfolio loss's per-name loading path is a distinct geometry.

## Evidence

Owners: [Copula implementations](../../../../finstack-quant/models/src/correlation/copula) and [latent_factor.rs](../../../../finstack-quant/models/src/correlation/latent_factor.rs). Inline tests locate marginal, integration and tail-dependence checks; these do not establish independent quadrature convergence. References: [Li](../../../REFERENCES.md#li-2000-gaussian-copula), [t copula](../../../REFERENCES.md#demarta-mcneil-2005-t-copula), [RFL](../../../REFERENCES.md#andersen-sidenius-2005-rfl).

### Independent review evidence

- At Gaussian conditional score `−11`, the normal tail is about
  `1.91066e−28`. Clipping to `−10` gives `7.61985e−24`. For Student-t
  at `ν=3,ρ=.5`, the analytic tail coefficient is `.3125`. With
  `ν=3,ρ=0,c=−1` and collapsed systematic variate zero, the one-name
  conditional PD is `5/32`. The two-name probability is about
  `.0315869782` versus `.0244140625` from multiplying those marginals.
- With RFL `ρ=.30,σ=.20`, independent moment calculations give
  `μ≈.5109263521` and unit-loading mass `.0072352156`. At `μ=.75,σ=.5`
  and `η=nextDown(.5)`, the exact interior loading is `1−2^−55`. A
  rounded unit branch gives probability one at `Z=c=0`, while the
  interior law gives `.5`. At `ρ=nextDown(1),σ=.5`, locations near
  `4.9577453` and `4.9877453` round to the same ordinary moment as the
  target, with smaller-tail log residuals about `+.245` and `−.246`.
  At two-factor `ρ=1,s=.05`, deriving the
  residual from rounded weights can invent a positive residual.
- The 20-node Hermite rule gives about `.0019633931` for the Gaussian
  step at `−3.1`, where the exact probability is `.0009676032`. All
  admitted Hermite orders miss the step at `−8`, with positive exact
  mass. For `ν=4,ρ=0` a t marginal of `.005` gives `.0048739950`
  at Laguerre order 20 and `.0049938616` at order 64. Separate generated
  rule checks covered mass, mean and centered variance at selected
  orders and degrees of freedom. Applying normalized weights first
  also preserves a large finite constant callback result erased by
  intermediate overflow in the current raw paired sum.
- For labelled correlation with pair coefficients `.9,.1,.2` and
  pivot order `(1,3,2)`, the original second innovation gives
  `[0,.4216370214,0]` under the both-axis permutation. A row-only
  permutation gives `[0,.1105541597,.9949874371]` despite reproducing
  the same covariance. These are fixed-coordinate reference results.
- Production qualification still requires range, calibrated tails,
  integration and factor work limits, seeded paths and host wire checks,
  and independent tranche and portfolio error studies.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
