# MODELS-MONTE-CARLO-07: Hull–White exact transition and curve-fitted theta levels

- Decision status: **Accepted — complete Hull–White endpoint and curve-fit package, 2026-11-11**.
- Implementation status: Existing endpoint and curve-to-theta routes observed. The accepted checked endpoint stepping and dated caller reconciliation are pending.
- Validation status: Source, selected tests and primary references inspected. Independent endpoint, range, exact-grid and curve-discount examples checked. Runtime, host and pricing qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Hull–White endpoints and curve-fit boundaries

1. **Select the explicit mean-level model for raw short-rate paths.**
   For caller-supplied levels use
   `dr=kappa*(theta(t)-r)*dt+sigma(t)*dW` on one declared model clock.
   Require finite `kappa>=0` per model year, finite signed initial and
   evolved decimal rates, and finite nonnegative absolute sigma per
   square root model year. Theta is a signed short-rate level in decimal
   units. Use the typed exact endpoint scheme and retire the raw Euler
   pairing for this model. With exact zero kappa the level forcing
   vanishes and the raw rate follows driftless Brownian motion with
   the supplied sigma schedule. Zero sigma gives deterministic
   evolution. Neither limit bypasses parameter and state admission.
   [Accepted Euler ownership](05-generic-steppers.md#accepted-decision-for-checked-coordinatewise-euler-and-exact-gaussian-steps),
   [Gaussian model references](../../../REFERENCES.md#hullwhite-endpoint-paths-and-curve-fit-boundaries).

2. **Use canonical signed theta and nonnegative sigma schedules.**
   Require immutable validated nonempty equal-length times and levels
   for theta. Times start at zero, are finite and strictly increasing.
   A constant is a single origin entry. Each level applies from its
   start knot, right-continuously, and the final level has an explicit
   flat tail for finite future times. Require finite signed levels
   and checked nonnegative time queries. Use the accepted nonnegative
   coefficient schedule for sigma. Retire the dual start/end theta
   knot interpretations, unweighted theta averaging and implicit
   extra final interval. No sorting, missing-origin synthesis,
   knot coalescing or generic defaults.
   [Piecewise sigma and integration](../core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration).

3. **Integrate mean-level forcing over exact stored endpoints.** For
   stored adjacent times `a<b` and `h=b-a`, set
   `A=exp(-kappa*h)`. For each theta segment `[l,u]` intersecting
   the step in positive length, add
   `theta(l)*exp(-kappa*(b-u))*(-expm1(-kappa*(u-l)))`
   to the conditional mean `A*r_a`. Integrate across every interior
   theta knot using its represented geometry. A knot exactly at
   the receiving time introduces no new forcing in that step.
   Combine the scaled signed terms and noise into the complete
   requested rate. Preserve consequential weights and products even
   if a detached `kappa*h`, decay or forcing term would round away.
   Require a finite checked result and propagate unresolved arithmetic.
   [Canonical grid](../core-numerics/time-grids.md).

4. **Share the checked endpoint-noise law.** Use the accepted state
   variance `V(a,b)=integral_a^b sigma(u)^2*exp(-2*kappa*(b-u)) du`.
   For a constant-sigma interval of width `h`, the endpoint loading
   is `sigma*sqrt(h*E(2*kappa*h))`, with
   `E(z)=-expm1(-z)/z` and `E(0)=1`. Use the same scaled
   scheduled contributions as the endpoint marginal of the joint
   rates driver. Apply the loading to the interval's one standard
   normal. Preserve a positive representable loading even if a
   standalone variance or product cannot fit. Successful zero
   noise comes from a genuinely inactive sigma schedule. Remove
   the first-order small-argument approximation and its hardcoded
   threshold, along with the
   variance-to-NaN then `max(0)` path. A variance requested as
   an output has its own checked representability.
   [Analytic variance](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures),
   [joint endpoint marginal](../models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling).

5. **Prepare one checked endpoint step sequence per process and grid.**
   Freeze deterministic coefficients for the complete admitted
   process and exact stored knots, checking shapes and work sizes
   before draws. Rebuild when either changes. Direct and prepared
   stepping use the same law on the actual endpoints. Retire
   first-step cache and alternate arithmetic. Internal coefficient
   knots are integrated in preparation. They introduce neither
   observations nor random draws. The raw path consumes one checked
   normal per positive actual interval in order, including reserved
   slots on zero-noise intervals. An admitted origin-only grid
   has no step or draw. Publish the signed `short_rate` coordinate.
   Remove the generic spot alias. Preserve explicit observation
   and replay metadata through existing simulation transport.
   [Accepted path state](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [Philox draw provenance](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

6. **Remove the curve-to-theta fit from dated paths.** Retire both
   curve-to-theta helpers and their finite-difference forward,
   monthly schedule, empty fit, and guessed final interval paths.
   Use the accepted dated centered process `x(0)=0`,
   `dx=-kappa*x*dt+sigma(t)*dW` and
   `r(t)=x(t)+phi(t)` with `phi(t)=f0(t)+J(0,t)`.
   Bind the origin, ACT/365F model time, each curve's native clock,
   rebased discount function and active sigma schedule under record 24.
   Use the actual qualified one-sided derivative for a requested
   short rate, right at the origin and left at positive breakpoints.
   Preserve deterministic short-rate jumps from the curve shift.
   The fitted origin rate is the curve's right forward. Remove the
   independent initial-rate override. Fitted kappa-zero and
   zero-volatility paths retain the selected curve shift.
   [Accepted curve fit and clocks](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection).

7. **Keep bank integration and cash with the joint pricing driver.**
   Raw endpoint output is a short rate. Dated stochastic discounting
   uses record 25's joint Gaussian endpoint and rate integral,
   exact shift integral and dated payments. Apply that discount
   once. Remove fitted-path trapezoid integration and any inference
   of an exact bank account from raw rate captures. An owner using
   an explicitly deterministic scalar discount must qualify that
   measure and payoff representation under record 01. A scalar
   exact endpoint marginal alone supplies no exact coupled rate,
   utilization or spread transition.
   [Accepted joint paths and discounting](../models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling),
   [joint credit-line owner](11-credit-line.md).

8. **Reconcile the actual dated and raw callers.** Migrate the shared
   rates helper and the Bermudan swaption's direct fit to the
   accepted curve-bound preparation. Reconcile callable range
   accrual, Snowball, TARN, MBS MC OAS and revolving-credit paths,
   including the separate MBS rate step and credit-line variance
   sentinel callers. Preserve the product's fixing, payment,
   exercise and linked-factor semantics. Revolving credit chooses
   its rate law explicitly and keeps it through its zero-volatility
   limit. Reconcile the existing Rust, Python and WASM raw
   `hull_white_1f` simulation contract with checked immutable
   parameter restoration, resolved scheme and exact grid metadata.
   Keep low-level processes and prepared steps Rust-owned.
   Dated calibration, lattices, projection and product risk
   remain with their existing owners.

9. **Qualify endpoint, fitted curve and product error separately.**
   The explicit-level endpoint is the exact Gaussian marginal of
   its supplied coefficients at the admitted grid nodes in ideal
   arithmetic. For curve-driven pricing, test conditional and
   origin discount replay against the independently sourced curve
   under the accepted shift and joint law. Cover exact coefficient
   breaks, small positive widths, zero limits, signed rates and
   unresolved range. Product owners set price, risk and path
   tolerances and qualify exercise and other coupled approximations.
   A match of short-rate means at sampled boundaries supplies no
   discount replay test. [Gaussian references](../../../REFERENCES.md#hullwhite-endpoint-paths-and-curve-fit-boundaries).

## Observed choice and input domain

The short rate follows `dr=kappa(theta(t)-r)dt+sigma(t)dW`, with
positive kappa, scheduled nonnegative volatility validated by shared
Hull–White parameters, and finite signed theta levels. Theta is a
mean-reversion level. Times are years and rates are decimals. The
raw dispatcher defaults to the exact transition and also accepts
explicit Euler.

## Observed algorithm and embedded curve fitting

Exact mean is `r exp(-kappa dt)+integral kappa theta(u) exp(-kappa(t+dt-u))du`; each theta segment is integrated with exponential weights. Variance integrates piecewise sigma² with the squared exponential kernel. Constant sigma uses `sigma sqrt((1-exp(-2kappa dt))/(2kappa))`, with a first-order small-kappa-dt expansion below 1e-8. Grid-first-step constants are cached; unequal dt recomputes them.

`calibrate_theta_from_curve` estimates instantaneous forwards via shared finite differences of positive DFs. It constructs the curve shift alpha(t): constant sigma uses `f(0,t)+sigma²(1-exp(-kappa t))²/(2kappa²)`; piecewise sigma uses the difference of single/double exponential variance integrals divided by kappa. Each theta level solves `alpha_end=alpha_start exp(-kappa width)+theta(1-exp(-kappa width))`. Positive-first knots are interval ends; zero-first knots are interval starts and an extra last interval width is extrapolated (one year if only one knot). Empty constant-sigma theta times return one constant initial forward level.

## Observed failure behavior

Failed theta-fit forward samples error. The low-level
`sigma_variance_for_step` can return NaN, which the exact step
can suppress with `.max(0)` before the square root. The
empty-theta shortcut uses the initial forward level and omits
the time-dependent convexity shift. The low-level process
publishes a short-rate endpoint and currently adds a spot alias.
The existing dated rates driver integrates endpoint rates by
trapezoid. Its reconciliation is already accepted in record 25.

## Independent examples and qualification

| Example | Expected behavior under the accepted contract |
| --- | --- |
| `kappa=ln(2)`, times `[0,1,2]`, theta `(-0.02,0.06)` and sigma `(0.01,0.02)` with knots `[0,1]`, opening rate `0.04` | One step over both coefficient segments and two-step composition give mean `0.035`, variance about `0.00022992952214` and endpoint SD about `0.01516342712`. |
| `kappa=2^-600,h=2^-600,theta=2^800,r0=sigma=0` | The mean contribution is approximately `2^-400`. A detached product `kappa*h` rounds to zero. |
| `kappa=0,h=1,sigma=1e-200`, or `kappa=2^1023,h=1,sigma=2^512` | The endpoint loading is `1e-200` in the first case and rounds to one in the second. The detached variance underflows in the first case, and detached `2*kappa` and `sigma^2` overflow in the second. |
| Stored `a=2^-53,b=1+2^-52`, theta moves from zero to `2^52` at time `1`, `kappa=1,r_a=sigma=0` | The theta contribution is just under one. The reconstructed endpoint `a+(b-a)=1` erases it. |
| Zero-volatility curve with `f0(t)=0.01+0.04*t` over `[0,1]`, `kappa=1` | A single fitted theta of about `0.07327906827` matches both endpoint means but gives exact integrated discount about `0.96726858794`. The qualified curve shift gives the input factor `exp(-0.03)≈0.97044553355`. |

Independently qualify multi-interval moments, subnormal loadings,
deterministic and kappa-zero paths, signed theta, exact stored endpoints,
curve forward jumps, normalized curve replay, and host state and error
handling. Qualify each migrated pricing product on its actual dates,
cash law and risk or exercise requirements.

## Evidence

Existing tests cover flat-curve shift, forward jumps, signed and
scheduled coefficients, and exact endpoint step composition.
The live dated callers also use the monthly curve-to-theta helper
and a separate direct Bermudan fit.
[Theta process and fitting](../../../../finstack-quant/models/src/monte_carlo/process/ou.rs),
[exact stepping](../../../../finstack-quant/models/src/monte_carlo/discretization/exact_hw1f.rs),
[raw simulation](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[simulation moments](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs),
[rates curve helper](../../../../finstack-quant/models/src/rates/hw1f/curve.rs),
[dated rates driver](../../../../finstack-quant/models/src/rates/hw1f/mc.rs),
[credit-line step](../../../../finstack-quant/models/src/monte_carlo/discretization/credit_line.rs),
[MBS MC OAS](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/mc_oas.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
