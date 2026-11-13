# MODELS-MONTE-CARLO-16: Cheyette rough-volatility rate state and deterministic shift

- Decision status: **Accepted — complete checked rough Cheyette paths package, 2026-11-11**.
- Implementation status: The legacy Euler step and raw simulation route exist. Checked frozen-amplitude stepping, curve inputs and host reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent transition, covariance and curve examples reviewed. Runtime, host, grid-convergence and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked rough Cheyette paths

1. **Select the Cheyette state and the rough domain.** In model years use
   `dx=(y-kappa*x)dt+sigma*dW_rate`, `dy=(sigma^2-2*kappa*y)dt`
   and `r=x+phi(t)`. Require finite `kappa>=0` in inverse model years,
   typed `0<H<1/2`, finite `eta>=0` in inverse model years to the
   power `H` and `rho in [-1,1]` for the underlying Brownian drivers.
   Admit signed finite `x` in decimal annual-rate units and finite
   `y>=0` in squared rate units. `y` is the weighted auxiliary
   variance and convexity state. `sigma^2` is the instantaneous
   variance rate. The pair `[x,y]` omits fractional history, which
   remains part of the simulation law. Reject a nonrough Hurst input
   even in deterministic limits. [Cheyette state equations](../../../REFERENCES.md#cheyette-state-and-rough-volatility-extension).

2. **Use a model-owned base volatility amplitude.** Require a nonempty
   schedule with an explicit sampling mode and finite nonnegative
   amplitudes in decimal rate units per square-root model year. Preserve
   both point-linear interpolation with flat tails and right-continuous
   ending-interval sampling from zero with a flat final tail. Point
   knots are finite, nonnegative and strictly increasing. Interval ends
   are finite, positive and strictly increasing. At a shared interval
   boundary, select the next interval. A singleton is a valid constant.
   Check finite nonnegative queries, preserve literal zero and keep
   exact knots. Remove the variance-curve carrier and its wire shape
   from this role. Preserve the amplitude ordinates and square only
   after sampling. For example, `0.005` is a 50 bp amplitude whose
   square is `0.000025`. The midpoint of amplitudes `0.01` and `0.03`
   has squared amplitude `0.0004`. Interpolating their squared knots
   would give `0.0005`.
   [Accepted forward-variance boundary](../core-market/forward-variance.md#accepted-decision-nonnegative-variance-sources-and-explicit-model-time-semantics).

3. **Keep the signed deterministic shift explicit.** Require nonempty,
   finite, signed `phi` levels in decimal annual-rate units, with
   finite, strictly increasing nonnegative model-time knots. Accept a
   singleton, exact knot values, linear interior evaluation and flat
   tails. Use the checked scalar interpolation contract and remove the
   credit-line helper dependency. Project `short_rate=x+phi(t)` through
   one checked calculation at the requested stored node, including the
   origin, and allow negative short rates. Publish the raw factor order
   `[x,y]` consistently in path results and metadata. The derived
   short rate has its own name and units.
   [Accepted scalar interpolation](../core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics),
   [accepted credit-line package](11-credit-line.md#accepted-decision-for-checked-revolving-credit-paths).

4. **Bind the full model to the actual grid and clock.** Require explicit
   initial `[x0,y0]` and a common model-year axis from zero for the
   grid, amplitude, shift and rough driver. Bind preparation and reuse
   to the immutable model, complete stored grid, clock and Hurst value.
   Use each actual adjacent width and original node time. Require a
   matching stored knot at every strict-interior change of an
   ending-interval amplitude. A repeated level, point-linear knot or
   shift knot needs no forced split. Reject a missing break, an
   incompatible plan or an invalid clock. Do not infer a day count,
   snap events or regenerate intervals. A fresh scenario has zero
   fractional history, including when the initial factors are nonzero.
   The origin-only grid returns the validated initial state without
   draws. Conditioned historical restart needs its own history contract.
   [Accepted time grid](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots).

5. **Freeze the adapted, normalized amplitude on each cell.** At
   `a=t_i` use the accepted actual-grid RL level `Y_a` and its sampled
   variance `v_a`. Set
   `sigma_a=sigma_base(a)*exp(eta*Y_a/2-eta^2*v_a/4)`.
   The amplitude is known at the left node before any current-cell
   draws. Under ideal Gaussians,
   `E[sigma_a^2]=sigma_base(a)^2`. The mean amplitude itself is
   `sigma_base(a)*exp(-eta^2*v_a/8)`. Use sampled `v_a` rather than
   the continuum power when they differ. At the origin or `eta=0`
   use the base exactly. At zero base use zero after input validation
   without evaluating the rough exponential. Retain the fractional
   history across zero cells and the reserved draw layout at all limits.
   [Accepted fractional noise](13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers).

6. **Integrate both drifts and the frozen rate variance.** For `h=b-a>0`
   set `z=kappa*h`, `E=exp(-z)`,
   `B=h*exprel(-z)` and `B2=h*exprel(-2*z)`, where
   `exprel(-u)=-expm1(-u)/u` with value one at zero. Use
   `y_b=E^2*y_a+sigma_a^2*B2` and
   `m_x=E*x_a+E*B*y_a+sigma_a^2*B^2/2`. Add centered rate noise
   of conditional variance `sigma_a^2*B2` to `m_x`. This gives the
   exact frozen-amplitude transition for the two state coordinates
   considered without the new RL level. It keeps `y_b>=0` on
   every admitted cell. At `kappa=0`, `E=1` and `B=B2=h`.
   With zero amplitude the coupled deterministic drift still applies.

7. **Preserve the Brownian increment covariance with three draws.**
   Define `q=B/sqrt(h*B2)=sqrt(tanh(z/2)/(z/2))`, with `q=1`
   at zero. Use
   `w=sqrt((1-abs(rho))*(1+abs(rho))+rho^2*(1-q^2))` and add
   `sigma_a*sqrt(B2)*(rho*q*Z_drive+w*Z_independent)` to `m_x`.
   Its conditional covariance with the actual driver increment
   `sqrt(h)*Z_drive` is `sigma_a*rho*B`. The newest-cell fractional
   residual `Z_near` remains inside the RL generator. This projection
   preserves covariance with the ordinary Brownian increment that
   represents the cell in later RL rows. It omits the frozen OU
   kernel's residual covariance with `Z_near`. The exact within-cell
   joint law agrees when `kappa=0`, `rho=0` or `sigma_a=0`.
   With `abs(rho)=1` and `kappa*h>0`, the independent coordinate
   represents the OU kernel residual after projection onto the
   increment. That coordinate does not introduce another physical
   Brownian driver at perfect correlation. The full rough law and its
   within-cell leverage are grid approximations.

8. **Keep the step fallible over its declared numerical domain.** Use
   stable scaled decay and exponential-relative calculations. Preserve
   scale through `sigma_a^2*B2`, `sigma_a^2*B^2`, `E^2*y_a`,
   `E*B*y_a` and the complete rough exponent. Near zero `z`,
   `sqrt(1-q^2)~z/sqrt(12)`. Compute the residual stably before
   `q` rounds to one. Use checked signed sums for `x` and the
   requested rate projection. Retain representable subnormals. Report
   invalid or unrepresentable required outputs, including a strictly
   positive required amplitude or `y_b` lost to zero. Preserve exact
   zero limits and avoid evaluating irrelevant zero-weight terms.
   Do not clamp state, add jitter, impose a coarse-grid floor or
   substitute a different model. Commit checked state together and
   return no partial path or requested projection on failure.
   [Accepted stable Gaussian covariance](12-schwartz-smith.md#accepted-decision-for-checked-schwartzsmith-paths-and-expectations).

9. **Reuse the accepted RL plan, draw order and limits.** Use the
   chronological Philox triples `(Z_drive,Z_near,Z_independent)`
   on the exact actual grid, including reserved draws at model
   limits. Apply leverage once. A paired leg uses fresh state and
   work and negates all three coordinates. Raw capture has no payoff
   draws. Reuse the checked setup, aggregate generation, storage and
   allocation preflight, including the 64,000,000-cell triangular
   preparation ceiling. Preserve canonical node times and checked
   result shapes. Attach path, leg and cell or node context to
   failures. Never shorten the fractional history to fit a request.
   [Accepted fractional noise](13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers),
   [accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

10. **Reconcile the existing raw Rust, Python and WASM route.** Keep
    the `cheyette_rough` process tag and select the explicit
    `cheyette_rough_frozen_amplitude` scheme. Remove the Cheyette
    Euler and generic default scheme routes, injectable noise
    slots, zero drift/diffusion placeholders and generic `fbm`
    selector. Construct or restore immutable parameters and schedules
    through the same strict checks. Reject missing and unknown
    fields, mismatched curve shapes, unsupported schemes and the old
    variance-curve wire input. Use one Rust path law, publish the
    same factor shape and names on all hosts, and reconcile the
    existing schema, bindings, declarations, examples and parity ledger.
    The low-level driver and step preparation remain Rust-owned.

11. **Keep pricing, fitting and discounting at their financial owners.**
    The supplied shift defines the raw scenario rate. A qualified
    curve owner can use `phi=f(0,t)` with `x0=y0=0` and a coherent
    curve and clock. Keep this package on the existing raw simulation
    surface. A product owner must specify the dated curve fit, pricing
    numéraire, stochastic bank account and any bond projection, along
    with a joint rate-integral law, actual monitoring and settlement
    rules and payoff-specific moment and grid qualification. Ideal fixed
    finite grids give finite polynomial state moments. Exponential rate
    and discount moments require separate proof for PV and standard
    error claims.

12. **Qualify path and financial results independently.** Check exact
    frozen drift, variance and Brownian covariance, deterministic
    piecewise-constant partitions, exact breaks, signed projections,
    zero and extreme correlation limits, stable residuals and
    checked restoration. Verify the independent newest-cell covariance
    discrepancy and measure its behavior under refinement on uniform
    and irregular grids, including positive and negative leverage.
    Qualify sampled rough covariance, tails and state convergence with
    references independent of the production transition. Check seeded
    and paired replay, resource failures and host parity. A later
    pricing approval needs its own curve fit, discount, product and
    estimator evidence. Endpoint and sample-moment checks alone
    cannot close those gates.

## Observed behavior and limitations

The current implementation requires positive `kappa` and `eta` and only warns
when `H>=1/2`. It accepts a `ForwardVarianceCurve` for `sigma_base` and consumes
its ordinates directly as amplitudes. The deterministic shift is validated on
construction and sampled through the credit-line interpolation helper.
The Euler step uses the left fractional level with continuum compensation
`eta^2*t^(2H)/4` in the amplitude exponent, then adds the next fractional
increment. It combines the driving and independent normals for the rate
shock. The available fractional generators differ in covariance. The process
uses a separate shift for projected short rates, while raw dispatch publishes
`[x,y]` and its process metadata says `[rate,variance]`. The observed
source supplies no Cheyette valuation product or discount-curve calibration.
[Process](../../../../finstack-quant/models/src/monte_carlo/process/cheyette_rough.rs),
[stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/cheyette_rough.rs),
[raw dispatch](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[non-Markov tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests/non_markov.rs).

## Independent review evidence

- For `kappa=h=1`, `x_a=0.02`, `y_a=0.04` and `sigma_a=0.1`,
  the observed Euler formula produces `y_b=-0.03`. The frozen step
  gives `y_b=0.009736734913281444`, conditional mean
  `0.01865723714529067` and conditional variance
  `0.004323323583816937` for `x_b`. At `kappa=0`,
  `h=0.5`, `x_a=-0.01` and the same `y_a` and amplitude,
  `y_b=0.045` and `m_x=0.01125`. An independent constant-amplitude
  check on unequal widths `0.2`, `0.25` and `1.25` reproduces the
  single-cell state means and rate variance for total width `1.7`
  at zero and positive mean reversion.
- For `kappa=h=1`, `H=0.25`, `rho=0.8` and `sigma_a=0.1`,
  the continuous frozen OU cross-covariance with the newest-cell RL
  contribution is `sigma_a*rho*sqrt(2H)*integral_0^h exp(-kappa*u)*u^(H-1/2) du`.
  The selected cross-covariance is
  `sigma_a*rho*B*sqrt(2H)*h^(H-1/2)/(H+1/2)`. The values are
  `0.05129544295309` and `0.04767751825868` respectively. Both
  preserve covariance with the actual Brownian increment. At `z=2^-54`
  the perfect-correlation residual is about `1.60247e-17` when
  binary64 `q` rounds to one.
- A signed shift with knots `(1,-0.02)` and `(3,0.04)`
  gives `phi(0)=-0.02`, `phi(2)=0.01` and `phi(4)=0.04`.
  With `x(2)=-0.03`, the projected rate is `-0.02`.
- With `kappa=x0=y0=phi=0` and a constant amplitude on a single
  width `h`, the exact integrated rate has mean `sigma^2*h^3/6`
  and variance `sigma^2*h^3/3`. Its expected discount is one.
  Endpoint trapezoidal discounting instead has mean
  `exp(-sigma^2*h^3/8)` even with exact endpoints.
- Primary references establish the separable Cheyette state and
  the hybrid Volterra construction. The combination of this RL law,
  left amplitude freezing and the endpoint covariance projection is
  a model decision here. Existing tests inspect legacy interpolation,
  left-node stepping, compensation and seeded raw replay. The
  independent examples check formulas and limits. Runtime and pricing
  benchmarks for the proposal are pending.
  [Primary references](../../../REFERENCES.md#cheyette-state-and-rough-volatility-extension).

## Follow-up tracking

Adoption questions and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
