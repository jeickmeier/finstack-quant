# MODELS-MONTE-CARLO-09: Heston QE variance, integrated-variance approximation and martingale correction

- Decision status: **Accepted — complete checked Heston QE-M package, 2026-11-11**.
- Implementation status: Existing Heston QE spot and variance paths observed. Checked QE-M, exact deterministic limits and caller reconciliation are pending.
- Validation status: Source, selected tests and primary references inspected. Independent moment, pole, small-volatility, deterministic-variance and carry examples checked. Runtime, host and pricing qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Heston QE-M paths

1. **Use the checked classical Heston model domain.** Require finite
   `kappa>0`, `theta>0`, `sigma_v>=0`, `v0>=0`,
   `-1<rho<1`, and finite signed carry inputs on the declared
   pricing-measure clock. Spot is strictly positive and finite,
   including representable subnormals. Step variance is finite and
   nonnegative. Zero starting variance with positive immigration
   uses the selected law. Negative state errors without truncation.
   Reuse immutable model validation and the inclusive Feller
   diagnostic without a Feller gate or warning. This package widens
   the classical MC owner's domain to zero initial variance and zero
   vol-of-vol, with independent evidence required for those limits.
   PDE and rough Heston retain their own admission.
   [Accepted Heston model](../models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform).

2. **Select one checked QE-M profile.** Use record 08's checked
   square-root conditional moments and QE variance regime with
   threshold `3/2` and equality in the quadratic branch. Use the
   same regime for the draw and the spot correction. Retire the
   unused Heston threshold override, generic Heston Euler and
   automatic scheme substitution. Require the dedicated scheme
   in generic pricing and raw simulation. Preserve explicit
   calibration and product model choice at their owners.
   [Accepted shared QE](08-cir-qe.md#accepted-decision-for-checked-cir-and-cir-quadratic-exponential-paths).

3. **Bind carry and preparation to the actual grid.** For the
   stored endpoints `a<b`, use actual width `h=b-a` and
   carry increment `C=(r-q)*h` or the difference from one
   qualified piecewise-linear cumulative carry schedule. Use
   record 04's checked segment evaluation and full coverage.
   Reject out-of-support schedules and incompatible bindings.
   Prepare shared work for the complete admitted model and exact
   stored knots. Direct and prepared stepping use the same
   arithmetic. Do not reconstruct endpoints as `a+h`, replace
   scheduled carry with flat carry or add hidden substeps.
   [Accepted carry](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
   [time grid](../core-numerics/time-grids.md).

4. **Own correlation and ordered draws inside the scheme.** Each
   positive interval takes two checked independent standard-normal
   innovations in order `(z_spot,z_var)`. The variance draw uses
   `z_var`. The first slot supplies the orthogonal spot noise in
   the stochastic branch. Compute `w=(1-rho)*(1+rho)` and use
   rho in the leverage term. No engine pre-correlation or clamp
   of rho, w or variance. Validate both slots at deterministic
   limits and preserve their assignment. An eligible antithetic
   partner negates both normals. An admitted origin-only raw grid
   uses no draw. Keep assignment fixed across QE branches.
   [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [accepted draw profile](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

5. **Use one trapezoidal integrated-variance approximation.** For
   positive vol-of-vol, set `I=h*(v+v_next)/2`, using the
   checked selected variance draw. Preserve required scale
   through the average, time product and spot noise. When both
   endpoints are zero, the chosen approximation gives exact zero
   for that step. Remove the mean-reversion-adjusted alternative,
   `exact_variance` and the method selector. Trapezoidal `I`
   is a finite-step approximation of the variance integral.
   [Andersen QE-M and exact-simulation references](../../../REFERENCES.md#checked-heston-qe-m-spot-paths).

6. **Normalize the spot by the log moment of the same variance law.**
   With `c2=h/2`, set
   `A=(rho/sigma_v)*(1+kappa*c2)-rho^2*c2/2`.
   Let `m` be the exact conditional mean in record 08 and
   `K=log E[exp(A*(V_next-m)) | v]` for the selected QE law.
   Use the log growth
   `C+A*(v_next-m)-K-w*I/2+sqrt(w*I)*z_spot`.
   This equals the uncentered normalized expression using
   `A*v_next-log E[exp(A*V_next)|v]`. At rho zero, the
   normalized leverage term is exactly zero after admission.
   With ideal independent normal innovations and a finite moment,
   conditioning on `V_next` gives unit conditional mean for
   the compensated orthogonal factor. The full conditional spot
   mean under this discretization is therefore `S*exp(C)`.
   [Andersen QE-M](../../../REFERENCES.md#andersen-2008-heston-qe).

7. **Preserve centered leverage at small positive vol-of-vol.** In the
   quadratic regime, denote the QE scale by `a_q` and put
   `g=2*A*a_q`, `D=1-g`. Form the sample deviation directly as
   `a_q*(z_var^2-1)+2*sqrt(a_q*(m-a_q))*z_var`.
   For `D>0`, the centered log moment is
   `2*A^2*a_q*(m-a_q)/D+(-log1p(-g)-g)/2`.
   Evaluate the logarithmic remainder and complete products in scale.
   Subtraction of rounded variance endpoints or separately rounded
   `A*m` and log moments can erase the spot leverage. For the
   mixture put `g=A*B`, `D=1-g` and use record 08's positive
   weight `q=m/B`. With `L=0` at the atom and
   `L=log(q)-log(normal_survival(z_var))` in the positive tail,
   the centered sample term is `g*(L-q)` and the centered log
   moment is `log((1-q)+q/D)-q*g`. Near zero evaluate that
   moment as `q*g^2/D+log1pmx(q*g/D)`, where `log1pmx(x)`
   means `log1p(x)-x` evaluated without cancellation. Retain
   scale and use stable log-sum arithmetic for extreme ratios.
   Form the final positive spot with the checked scaled
   exponential product and commit both state coordinates only
   after successful evaluation. No small-positive-sigma cutoff.
   [Accepted scaled spot update](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).

8. **Fail a run on a divergent moment or unresolved arithmetic.**
   Use the mathematical domains of the same QE law, including
   `1-2*A*a_q>0` in the quadratic branch and `1-A*B>0`
   in the exponential branch with positive scale `B`.
   Compare in scale. Distinguish a true pole or divergence from
   a numerical computation failure. Report the affected estimator,
   leg, node and regime through the existing failure contract.
   Remove the uncorrected fallback, clipping, retries, silent
   refinement and partial successful estimates. A new grid is
   a new explicitly selected run.
   [Accepted run failures](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

9. **Use the exact deterministic-variance law at literal zero vol-of-vol.**
   Put `d=exp(-kappa*h)` and
   `E(z)=-expm1(-z)/z` with `E(0)=1`.
   Use `v_next=v*d+theta*(1-d)` and
   `I_det=h*[v*E(kappa*h)+theta*(1-E(kappa*h))]`.
   Evaluate both weights and the complete positive contributions
   with stable small-product arithmetic. The log spot update is
   `C-I_det/2+sqrt(I_det)*z_spot` with full spot loading.
   Preserve the declared second normal slot. The zero branch
   uses the same deterministic limit as the analytic owner.
   Every positive vol-of-vol uses stochastic QE-M. A fixed
   coarse-grid stochastic approximation can have a different
   limit or pathwise coupling as vol-of-vol tends to zero.
   [Accepted analytic deterministic variance](../models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform).

10. **Reconcile actual pricing, raw and host routes.** Use the
    model block's `v0` as the sole initial variance. Raw Heston
    takes the initial spot and constructs the internal
    `[spot,v0]` state, returning checked spot and variance at
    the actual knots. Remove the duplicate raw initial variance
    field. Reconcile the direct Rust/Python/WASM Heston pricer
    with explicit path count, grid or step selection, seed,
    pairing, execution and currency under record 01. Keep
    the discount factor and carry qualified by the caller.
    Migrate equity vanilla, Heston Asian and Heston barrier
    routes. Select one qualified dated carry representation
    per operation so fixing multipliers and scheduled carry
    cannot both apply the same adjustment. Preserve product
    fixing, settlement and monitoring authority. Reconcile
    existing state, schemas, declarations, errors and parity.
    Low-level scheme and regime operations stay Rust-owned.
    [Raw simulation owner](28-capture-and-simulation-contracts.md),
    [Asian owner](17-vanilla-and-asian.md), [barrier owner](18-barrier-monitoring.md).

11. **Qualify the simulated law and product accuracy independently.**
    Verify ideal conditional first moments in both regimes,
    zero and tiny positive vol-of-vol, initial variance zero,
    Feller-failing cases, close poles, signed leverage and
    carried forwards on actual grids. Distinguish finite normal
    support and floating error from the ideal QE-M first-moment
    result. Check weak and path-dependent error on refined
    grids against independent Fourier or exact-simulation
    references where applicable. Product owners qualify
    monitoring, risk and price tolerances. The martingale
    correction fixes a conditional stock moment and leaves
    integrated variance and the joint transition approximate.
    [Heston and Broadie–Kaya references](../../../REFERENCES.md#checked-heston-qe-m-spot-paths).

## Observed choice and domain

Heston carries spot and instantaneous variance, with correlated spot/variance Brownian drivers. Checked model inputs come from `HestonPricingParams`: finite decimal r/q, strictly positive finite kappa/theta/sigma_v/v0, and rho strictly between -1 and 1; generic process constructors trust the supplied object. Variance v/theta/v0 is annualized volatility squared, kappa is inverse years and time is a year fraction. Default and convenience paths use QE with threshold 1.5 and trapezoidal integrated variance. Explicit Euler uses engine correlation and full-truncation coefficient evaluation. Feller is an inclusive diagnostic, not an enforcement gate.

## Observed algorithm and selection

1. Sample v_next from the shared [QE kernel](08-cir-qe.md) using z[1]. Use independent z[0] for the orthogonal spot innovation; the scheme marks correlation internal.
2. Approximate integrated variance as I=c0+c1*v+c2*v_next. Trapezoidal uses (0,dt/2,dt/2). `MeanReversionAdjusted` uses c1=c2=(1-exp(-kappa dt))/(2kappa), c0=theta*dt−2theta*c1, reverting to trapezoidal for |kappa dt|<1e-8. `exact_variance()` selects this approximation; it is not an exact integrated CIR path sample.
3. Clamp rho to [-1,1] and I at zero. Define Apre=−c2/2+(rho/sigma_v)(kappa*c2+1), A=Apre+(1-rho²)c2/2. If the QE distribution MGF at A is finite and positive, set K0*=−ln(MGF)−(1-rho²)(c0+c1*v)/2 and log-spot increment to drift_increment+K0*+Apre*v_next+sqrt((1-rho²)I)z[0].
4. If that MGF does not exist, silently use the uncorrected approximation `drift_increment-I/2+(rho/sigma_v)(v_next-v-kappa*theta*dt+kappa*I)+orthogonal_noise`. For |sigma_v|≤1e-10 use the deterministic-variance lognormal step with full spot noise. Exact/QE drift increments may use the optional cumulative drift schedule; Euler drift ignores that schedule.

## Observed limitations

The moment-failure fallback changes the spot law without
reporting the change. At a true pole the uncorrected formula
has the same divergent tilt in the ideal conditional stock
mean. Finite-grid draws can still produce finite paths.
The current zero-vol-of-vol branch uses the selected
approximate endpoint-based integral, and the small-sigma
threshold also routes positive vol-of-vol to that branch.

## Independent examples and qualification

| Example | Expected behavior under the proposal |
| --- | --- |
| Exponential regime with `kappa=sigma_v=1,theta=0.01,v=0,rho=0.5,h=20` | `psi=50`, `A=4.25` and `A*B≈1.083749998`. This is a true moment divergence. The run errors. |
| Quadratic regime with `kappa=sigma_v=theta=v=1,rho=0.9,h=12` | The model satisfies Feller. `psi≈0.5`, `A=3.87` and `2*A*a_q≈1.037`. The run errors at the quadratic moment pole. |
| Exponential regime with `kappa=sigma_v=1,theta=0.25,v=0,rho=0.5,h=ln(2)`, zero carry and `z_spot=0` | `psi=2`, `q=2/3` and `B=3/16`. The atom-inclusive moment is about `1.089292760`. From spot 100, `z_var=-1` gives zero variance and spot `91.80268490`. `z_var=0` gives variance `0.05394038858` and spot `94.31231158`. |
| `kappa=h=1,theta=v=0.04,sigma_v=1e-100,rho=-0.7,z_var=1,z_spot=0` and zero carry | Rounded `m` and `v_next` both equal `0.04`, but the centered leverage term is about `-0.1380791693`. From spot 100 the checked log update gives about `85.40102373`. |
| `sigma_v=0,kappa=ln(2),h=1,theta=0.08,v=0,C=0.03,z_spot=1,rho=0.8` | Exact deterministic variance gives `v_next=0.04` and `I_det≈0.02229219836`. Spot 100 advances to about `118.31254484` using the full loading. At this fixed step the positive-sigma QE-M log-variance limit is about `0.02394228375`. |
| Stored interval `a=2^-53,b=1+2^-52` with carry knots `[0,1,b]` and cumulative values `[0,0,1]` | The actual carry increment is one. Reconstructing the endpoint by `a+(b-a)` gives time one and loses the increment. |

Independently compare both QE regimes and their ideal conditional stock
moment, verify exact and small-positive volatility limits and confirm
that the pole and range failures reach every existing caller. Qualify
actual paths and product prices on model-specific grids before sign-off.

## Evidence

Existing tests cover conditional martingale behavior, branch
positivity and comparisons between production methods. The live
equity, Asian and barrier routes use the trapezoidal profile.
[Heston process](../../../../finstack-quant/models/src/monte_carlo/process/heston.rs),
[QE spot update](../../../../finstack-quant/models/src/monte_carlo/discretization/qe_heston.rs),
[convenience pricing](../../../../finstack-quant/models/src/monte_carlo/pricer/heston.rs),
[raw simulation](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[equity vanilla](../../../../finstack-quant/valuations/equity/src/equity_option/heston_mc_pricer.rs),
[Asian](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/heston_mc_pricer.rs),
[barrier](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/heston_mc_pricer.rs),
[Python](../../../../finstack-quant-py/src/bindings/models/monte_carlo/engine.rs),
[WASM](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs).

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
