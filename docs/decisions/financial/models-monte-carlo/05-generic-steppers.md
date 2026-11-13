# MODELS-MONTE-CARLO-05: Euler, log-Euler, proportional Milstein and Gaussian processes

- Decision status: **Accepted — complete coordinatewise Euler and exact Gaussian steps package, 2026-11-11**.
- Implementation status: Existing generic steppers and additive processes observed. The accepted checked Euler and exact multi-OU contracts require implementation and caller reconciliation.
- Validation status: Source, selected tests and primary references inspected. Independent Euler, Brownian and multi-OU arithmetic checked. Runtime, host and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked coordinatewise Euler and exact Gaussian steps

1. **Retain coordinatewise Euler for explicitly compatible Itô processes.**
   For each of `n>0` ordered states, require one finite drift, one finite
   diagonal diffusion loading and one corresponding Gaussian innovation.
   The signed loading multiplies its own innovation. Each coefficient
   can depend on the whole state, but the loading contract must be
   explicitly coordinatewise. Equal state and innovation counts alone
   do not establish diagonal diffusion. Require the actual model/scheme
   pairing before draws. Full diffusion matrices, injected noise and
   dedicated schemes require their own routes. Use one checked
   engine-owned correlation application where a matrix is explicitly
   selected. [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [Euler references](../../../REFERENCES.md#coordinatewise-euler-and-exact-gaussian-transitions).

2. **Evaluate and commit a complete Euler step.** At the exact stored
   left knot evaluate both coefficient sets against the same unchanged
   left state. For actual adjacent width `h>0`, compute
   `x_i' = x_i + mu_i*h + b_i*sqrt(h)*z_i`. Validate state, step,
   workspace, callback output shape and completeness, and finite
   inputs. Propagate coefficient failures. Evaluate each complete signed
   sum in checked scale, preserving representable results through
   avoidable intermediate overflow, underflow and cancellation. Admit
   genuine zero and signed states when compatible with the model.
   Validate every candidate against the process domain before committing
   any coordinate. On failure, preserve the input state and discard
   work output. No positivity floor, redraw or adaptive time refinement.
   Weak/strong order, stability and price error require the actual
   coefficient, time grid and payoff assumptions.
   [Canonical time grid](../core-numerics/time-grids.md).

3. **Retire the blanket log-Euler and proportional Milstein paths.**
   Retain the accepted typed left-node local-volatility log-Euler
   operation. Remove the generic floored-state division and the
   proportional Milstein branch, its absolute small-state cutoff and
   the obsolete proportional-diffusion marker after exact GBM reads
   direct coefficients. The current Milstein pairings are single and
   multiasset GBM, which now select exact transitions under record 04.
   Remove their obsolete scheme selections and the Milstein wire tag.
   Reconcile tests, benches, examples and host declarations.
   [Accepted GBM and local volatility](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).

4. **Use the exact additive step for Brownian motion.** For single or
   ordered multi-coordinate Brownian motion, require immutable checked
   nonempty parameter shapes, signed finite drift `mu_i` in state
   units per model year and finite `sigma_i>=0` in state units per
   square root model year. States are signed finite quantities.
   Reuse the additive update in clause 2 with constant coefficients.
   This gives the exact ideal Gaussian increment at the stored knots.
   Select independence or a checked driver factor `F_d` using the
   accepted strict correlation admission and explicit pivot tolerance.
   Apply it once. With `R_eff=F_d*F_d^T`, the ideal conditional
   covariance is `sigma_i*sigma_j*R_eff[i,j]*h`. Zero scale is
   deterministic. Preserve ordered draw addresses and explicit state
   projection, with no generic spot or variance aliases.
   [Core correlation factor contract](../core-numerics/matrix-factorization.md).

5. **Select an exact constant-coefficient multi-OU transition.** Require
   nonempty equally sized checked vectors of finite `kappa_i>=0` per
   model year, signed finite `theta_i` in state units and finite
   `sigma_i>=0` in state units per square root model year. Reject
   negative speeds on this mean-reverting route. At width `h`, the
   conditional mean is
   `exp(-kappa_i*h)*x_i + (1-exp(-kappa_i*h))*theta_i`.
   Use checked exponential and `expm1` terms and a scaled convex
   combination. Resolve consequential weighted terms before rounding.
   At zero speed the mean is the input state and the stochastic part
   is driftless Brownian. At zero scale the transition is deterministic.
   Validate all admitted inputs even for these limits. Replace
   multi-OU's current Euler dispatch with this exact law.
   [Exact OU references](../../../REFERENCES.md#coordinatewise-euler-and-exact-gaussian-transitions).

6. **Integrate OU covariance over each actual step.** Use the effective
   instantaneous driver covariance `R_eff=F_d*F_d^T`, or identity
   for independent drivers. For `c>=0`, set
   `I(c,h)=-expm1(-c*h)/c` for `c>0` and `I(0,h)=h`.
   The ideal transition covariance is
   `Q[i,j]=sigma_i*sigma_j*R_eff[i,j]*I(kappa_i+kappa_j,h)`.
   Compute stable marginal roots and normalized transition entries
   using the actual width and checked complete scale. Avoid forcing
   the unscaled covariance, products of variances or the speed sum
   to fit when the requested factor and step are resolvable. Distinguish
   a structural zero from a lost required positive scale.
   Distinct mean-reversion speeds can change correlation and increase
   transition rank. The input driver matrix remains instantaneous
   correlation in the contract and metadata.

7. **Prepare and apply one OU transition transform.** For heterogeneous
   correlated active coordinates, factor the normalized covariance with
   the same explicit core pivot tolerance. Set its active diagonal to one
   by definition. Restore per-coordinate scales
   `sigma_i*sqrt(R_eff[i,i]*I(2*kappa_i,h))`. Preserve exact zero
   stochastic rows and original innovation positions. For independence,
   use the separate marginal roots. When all stochastic coordinates
   share one speed, reuse `F_d` with the common time scale and the
   coordinate scales. This includes the zero-speed Brownian reduction.
   Bind preparation to the admitted process and stored grid. Reuse
   factors for exactly equal actual widths and check factor work,
   storage and allocation sizes before drawing. The OU scheme owns
   the transform and receives raw innovations. Use no engine
   pre-correlation. Apply the chosen factor once, add the checked mean
   and publish only a complete finite state. Preserve one declared
   draw slot per coordinate, even when a scale is zero. No automatic
   jitter, clipping or Euler fallback. A positive pivot tolerance can
   change the effective covariance. Qualify its numerical error
   separately from exactness of the ideal Gaussian law.
   [Accepted factor and truncation rules](../core-numerics/matrix-factorization.md).

8. **Reconcile existing simulation and model-owner routes.** Keep signed
   Gaussian states and explicit Brownian and exact multi-OU identities
   through Rust and existing Python/WASM raw simulation. Replace obsolete
   Euler aliases for these Gaussian process routes with truthful scheme
   selection and metadata. Reconcile validated state restoration,
   schemas, error mapping, examples and replay. Keep low-level steppers
   and coefficient callbacks Rust-only. Hull–White, CIR/CIR++,
   Heston and Schwartz–Smith retain ownership of existing Euler
   eligibility and their domain and convergence rules. LMM, rough and
   other dedicated models retain their own pairing admission. Generic
   Euler supplies no new model fallback, discount or pricing convention.
   [Hull–White](07-hull-white.md), [CIR/QE](08-cir-qe.md),
   [Heston](09-heston-qe.md), [LMM](10-lmm.md),
   [Schwartz–Smith](12-schwartz-smith.md), [simulation owner](28-capture-and-simulation-contracts.md).

## Observed choice and domain

The generic diagonal steppers are alternatives selected explicitly, with `simulate_paths` restricting valid process/scheme combinations. Additive Brownian motion uses constant drift mu and diffusion sigma; multi-OU uses independent-coordinate drifts `kappa_i(theta_i-x_i)` with optional driver correlation. Brownian sigmas are finite/nonnegative; multi-OU kappas/thetas must be finite but kappas may be negative or zero.

## Observed algorithm and controls

Euler evaluates coefficients at the left endpoint and applies `x_i += mu_i dt + b_i sqrt(dt) z_i`. For constant additive coefficients this is the exact increment; multi-OU default is Euler, not an exact OU transition. Log-Euler uses `x_safe=max(x,MIN_POSITIVE)`, divides drift/diffusion by it, and multiplies by `exp((mu/x_safe-(b/x_safe)²/2)dt+(b/x_safe)sqrt(dt)z)`. Milstein is restricted by `ProportionalDiffusion`, adds `0.5 b b' (z²-1)dt`, and estimates `b'=b/x` only when |x|>1e-10, else zero. The engine supplies correlated normals; these loops assume coordinatewise diffusion, not arbitrary matrix diffusion. LMM therefore requires a dedicated scheme.

## Observed conventions and failure behavior

Mu has state units per year, additive sigma has state units per square
root year, and kappa has inverse-year units. The Euler implementation
consumes one scalar loading per coordinate despite broad diffusion
claims in its documentation. CIR/Heston coefficient evaluation
truncates negative variance before the square root, and an Euler step
can still produce negative variance. The multi-OU route accepts
negative mean-reversion speeds and uses Euler. The generic log-Euler
and Milstein paths apply the state cutoffs described above.

## Independent examples and qualification

| Example | Expected behavior under the accepted contract |
| --- | --- |
| Euler `x=-2^1023, mu=2^1023, h=2, b=0` | Complete result is `+2^1023` despite overflow of the isolated drift product. |
| Euler `x=0, mu=b=eta, h=0.25, z=1` with `eta=2^-1074` | The exact sum is `0.75*eta` and rounds to `eta`. Both increments separately round to zero. |
| Observed Milstein with `x=2^-40`, relative sigma `0.5`, `mu=0, h=1, z=0` | The genuine Milstein polynomial gives `(7/8)*x`. The observed cutoff yields `x`. The accepted contract retires this path. |
| Two Brownian coordinates, scales `(2,3)`, `h=0.25`, driver rho `-0.5` | Conditional covariance off-diagonal `-0.75`, with variances `1` and `2.25`. |
| Deterministic OU, `x=1,theta=0,kappa=3,h=1` | Exact mean `exp(-3)≈0.0497871`. The observed Euler step gives `-2`. |
| OU speeds `(0,1)`, unit diffusion, perfectly correlated driver and `h=1` | Ideal Q has entries `Q11=1`, `Q12=1-exp(-1)≈0.63212056`, `Q22=(1-exp(-2))/2≈0.43233236`. The determinant is positive and step correlation is about `0.96137106`. |

Independently qualify coefficient callbacks and signed finite updates,
exact stored widths, original factor order, zero-speed and zero-scale
reductions, near-equal and unequal speeds, rank-deficient drivers,
mixed scales, factor error and resource admission. Check multi-step
Gaussian moments against the continuous law. Qualify existing host
state and caller behavior, and keep any pricing or grid-convergence
claims within the responsible model's evidence.

## Evidence

Existing simulation tests compare Brownian moments and correlation to
direct formulas and compare the multi-OU mean to the discrete Euler
recursion. The accepted exact multi-OU transition is pending.
[Euler and log-Euler](../../../../finstack-quant/models/src/monte_carlo/discretization/euler.rs),
[Milstein](../../../../finstack-quant/models/src/monte_carlo/discretization/milstein.rs),
[Brownian](../../../../finstack-quant/models/src/monte_carlo/process/brownian.rs),
[multi-OU](../../../../finstack-quant/models/src/monte_carlo/process/multi_ou.rs),
[simulation dispatch](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs),
[Python simulation](../../../../finstack-quant-py/src/bindings/models/monte_carlo/engine.rs),
[WASM simulation](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
