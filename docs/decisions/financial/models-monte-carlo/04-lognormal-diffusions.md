# MODELS-MONTE-CARLO-04: GBM, curve-implied interval coefficients and local volatility

- Decision status: **Accepted — complete GBM and local-volatility paths package, 2026-11-11**.
- Implementation status: Existing GBM, curve bootstrap and local-volatility routes observed. The accepted checked contracts and caller reconciliation are pending.
- Validation status: Source, selected tests and primary references inspected. Independent transition, carry, bootstrap and local-volatility examples checked. Runtime, host and pricing qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked GBM and local-volatility paths

1. **Validate multiplicative-diffusion inputs.** Require immutable checked
   coefficients and finite positive spot at origin and each node, including
   positive subnormals. Rates and proportional yields are signed finite
   continuously compounded decimals per model year. Relative volatility is
   finite and nonnegative per square root model year. The caller owns the
   clock, measure, asset order and carry basis. Validate shapes, positive
   actual intervals, coefficient coverage and finite consumed innovations.
   Use the typed exact scheme for each GBM route. Retire generic Euler,
   log-Euler aliases and Milstein selections on these routes. Generic
   steppers and dividend jumps have separate model contracts.
   [Generic steppers](05-generic-steppers.md), [dividend jumps](06-cash-dividend-jumps.md).

2. **Use one checked exact lognormal transition.** For integrated carry
   `A`, integrated variance `V` and standard normal innovation `z`, use
   `S_next=S*exp(A-V/2+sqrt(V)*z)`. Flat inputs have `A=(r-q)*dt` and
   amplitude `sigma*sqrt(dt)`. Evaluate the complete exponent and final
   spot in checked scaled arithmetic. An isolated rate difference, square,
   product or exponential need not fit if the requested output is resolvable.
   Preserve the exact identity for zero exponent. Zero volatility leaves
   deterministic carry and preserves required draw addresses. A positive
   result rounded to zero, overflow or unresolved output is a computation
   error. No clipping, floor or replacement draw. Exactness applies to the
   ideal Gaussian transition at model knots under deterministic coefficients.
   Finite random grids and contractual monitoring have separate error limits.
   [Exact transition reference](../../../REFERENCES.md#checked-lognormal-and-local-volatility-paths).

3. **Make cumulative carry explicit and bounded.** Single-asset GBM selects
   flat carry or a checked piecewise-linear cumulative carry schedule with
   constant volatility. Require at least two finite strictly ordered knots,
   starting at zero, and coverage through the run horizon. Signed cumulative
   values and additive offsets are valid. Integrate the represented schedule
   between exact stored endpoints, including across interior carry knots.
   Use checked segment contributions to preserve increments erased by
   subtraction of separately rounded cumulative queries. Reject queries
   outside coverage. Remove implicit endpoint-slope extension and any
   scheduled path that reverts to flat carry. Reconcile shared schedule
   consumers, including Heston, with their owners. Dated sources and
   contractual observations retain their respective authority.
   [Canonical time grid](../core-numerics/time-grids.md),
   [accepted Sobol pricing](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics),
   [Heston owner](09-heston-qe.md).

4. **Use direct per-asset coefficients and apply correlation once.**
   Multiasset exact GBM takes nonempty ordered flat per-asset coefficients
   and one shock per asset. Remove recovery by division by spot and
   signed-unit probes for subnormals. Select independent innovations or
   one checked engine-owned factor with explicit pivot tolerance. Preserve
   original asset and innovation order. With `a_i=sigma_i*sqrt(dt)` and
   effective factor `F`, the ideal log-increment covariance is
   `a_i*a_j*(F F^T)[i,j]`. Project ordered spot values explicitly.
   A scalar spot alias requires a declared mapping to its asset.
   [Accepted engine correlation](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [core matrix factor contract](../core-numerics/matrix-factorization.md).

5. **Bind piecewise coefficients to exact grid knots.** Require a nonempty
   validated immutable schedule starting at zero, with finite strictly
   increasing positive interval end times and checked integrated carry and
   nonnegative variance for each interval. Include every interior coefficient
   break within the run as an exact grid knot and cover the last simulation
   knot. A step can end on a break or subdivide an interval. Departing an
   exact interior break selects the new interval. For a substep of width
   `h` in an interval of width `H`, use `A*h/H` and `V*h/H` in the
   shared transition. Reject crossing an unrepresented break and stepping
   beyond coverage. Remove last-interval extrapolation, skipped checkpoints,
   near-date deduplication and the absolute spacing floor. Include an
   identical physical coefficient checkpoint once. Distinct coefficient
   dates that collapse to one model time error. Contractual event
   multiplicity and settlement timing stay with the payoff owner.

6. **Resolve the active dated discount and ATM volatility sources.** For
   valuation date `v`, require ordered future dates `d_i` and positive
   model times `t_i`. Set `D_0=1` and obtain each finite positive `D_i`
   using the curve's relative factor from `v` to `d_i` on its own clock.
   Allow factors above one and negative implied rates. With initial spot
   `S0` and constant signed proportional yield `q`, use ATM strike
   `F_i=S0*exp(-q*t_i)/D_i`. Require an admitted unshifted Black surface
   in absolute strike units and a qualified calibration origin and source
   clock. Query `F_i` and the source expiry for `d_i` with the declared
   interpolation and strict support. Remove automatic coordinate clamping
   and implicit clock or quote conversion. An explicit scalar volatility
   in model units bypasses the unused surface.
   [Accepted volatility clocks](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility),
   [volatility-source evaluation](../models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization).

7. **Bootstrap integrated carry and forward variance.** Set `W_0=0` and
   require every annual volatility finite and nonnegative before squaring.
   For each surface checkpoint use `W_i=sigma_i^2*tau_i`, with `tau_i`
   elapsed time from `v` to `d_i` on the source clock. A model-unit scalar
   uses `t_i` instead. Form `V_i=W_i-W_(i-1)` and
   `A_i=log(D_(i-1)/D_i)-q*(t_i-t_(i-1))`. Use checked scaled variance
   differences and a stable log ratio without requiring the DF quotient to
   fit. Allow exact zero variance increments and reject decreases or
   unresolved positive variance. Publish the complete schedule only after
   all checks and lookups succeed. The ideal law matches the sampled ATM
   forward and total variance at those expiries. Smile, surface arbitrage,
   variance swap levels and path-dependent prices require separate evidence.
   [Time-dependent volatility and calendar conditions](../../../REFERENCES.md#checked-lognormal-and-local-volatility-paths).

8. **Require a qualified local-volatility grid and explicit support.**
   Use the accepted instantaneous relative-sigma grid with a single-factor
   constant proportional-carry process on its qualified model clock.
   Require compatible underlying, units and carry. An extracted grid must
   agree with the process forward basis `S0*exp((r-q)*t)` at its recorded
   source horizons under the owning qualification. A direct numeric grid
   uses its declared model units. Select strict support or flat local
   coordinate boundaries. Strict time-zero use requires a zero row. Flat
   time extension uses the first row for earlier times, including zero.
   Query at the stored left knot and current positive spot. Preserve zero
   sigma. The implied-volatility horizon conversion does not convert
   instantaneous diffusion or carry to another clock.
   [Accepted Dupire and local-grid contract](../models-analytic/dupire-local-volatility.md#accepted-decision-for-checked-dupire-extraction-and-local-volatility-grids).

9. **Select checked left-node log-Euler for local volatility.** Use the
   shared positive update with `A=(r-q)*dt` and amplitude
   `sigma_local(t_left,S_left)*sqrt(dt)`. Use the relative coefficient
   directly. Remove division by a floored spot, spot Euler and unsupported
   exact GBM or proportional Milstein pairings for this process. Propagate
   lookup and arithmetic failures without clipping, resampling or changing
   schemes. Validate consumed draws even at zero sigma. A flat local grid
   recovers the same ideal transition as flat GBM with matching inputs and
   shocks. State-dependent log-Euler freezes coefficients per step and
   requires independent grid-convergence and repricing evidence.
   [Local volatility and simulation references](../../../REFERENCES.md#checked-lognormal-and-local-volatility-paths).

10. **Reconcile live product, pricing and simulation routes.** Use the same
    checked state and scheme selection through Rust and existing Python/WASM
    pricing and simulation. Reconcile checked schemas, restoration, errors
    and replay metadata. Admit zero vol in plain exact GBM pricing without
    weakening density-based Greek or other owner-specific eligibility.
    Reconcile dated Asian, lookback, barrier and range-accrual carry and
    autocallable and cliquet coefficient checkpoints. Preserve the product's
    fixing and payment rules, one engine discount, and any accepted fixed-run
    precision admission. Keep low-level processes and the bootstrap
    Rust-owned. Monitoring and payoff and model-error budgets retain their
    respective owners.
    [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
    [simulation contracts](28-capture-and-simulation-contracts.md).

## Observed choice and input domain

Single/multiasset GBM uses `dS=(r-q)S dt+sigma S dW`; finite decimal rates/yields and nonnegative annualized volatility are accepted by checked parameters. Local-volatility diffusion replaces sigma by a supplied `LocalVolSurface.value(t,S)`, keeping constant r−q drift. Surface construction/Dupire extraction is delegated to the volatility domain. Convenience APIs require strictly positive volatility, even though generic GBM supports zero.

## Observed algorithm and selection

Exact GBM multiplies spot by `exp(integrated_drift - sigma² dt/2 + sigma sqrt(dt) z)`. An optional cumulative drift schedule is linearly interpolated and linearly extrapolated using endpoint slopes; exact stepping takes its endpoint difference. Generic `drift()` still uses constant r−q, so choosing Euler/Milstein ignores this optional schedule. Multiasset exact stepping recovers proportional drift/diffusion coefficients by dividing by state; zero remains absorbing; subnormal states are probed at signed unit level to avoid underflow. Correlation is applied once by the engine.

Piecewise GBM chooses the interval after a knot on equality and extrapolates the last interval. Its exact step freezes that interval through the whole step: callers must align all coefficient boundaries. `bootstrap_forward_gbm` starts at time zero/DF=1, skips nonincreasing checkpoints, computes `r_i=ln(DF_prev/DF_i)/dt`, queries sigma at `(T,S0 exp(-qT)/DF(T))`, differences total variances `sigma(T)²T`, and sets forward sigma to the square root of incremental variance/dt. Nonpositive/nonfinite DFs, dt≤1e-6 and decreasing total variance error.

Default path-simulation selection is exact GBM/multi-GBM, and log-Euler local vol; Euler is an explicit local-vol alternative and Milstein is rejected there. Exact lognormal transitions avoid time-step distribution error for constant coefficients, but monitoring remains grid-dependent.

## Observed limitations

Piecewise structs expose unvalidated vectors, the bootstrap does not
independently check callback volatility finiteness and sign, and an
empty or skipped schedule can leave no intervals. The valuation adapter
can pass model time to the volatility source and silently clamp the query.
Exact scheduled stepping reconstructs the right endpoint as `t+dt`.
The raw simulation guard excludes positive subnormals, while nonfinite
post-step checks can allow underflow to zero. The local-volatility
log-Euler loop recovers its relative coefficients through a spot floor.

## Independent examples and qualification

| Example | Expected behavior under the accepted contract |
| --- | --- |
| `sigma=2^520, dt=2^-1040, z=0, A=0` | Amplitude is one and exponent is `-1/2` despite an overflowing standalone sigma square. |
| `S=2^-1000` with exponent `1000`, or `S=2^1000` with exponent `-1000` | Final spots are about `1.83859569658e133` and `5.43893364845e-134`. The standalone exponentials overflow and underflow respectively. |
| Carry knots `[0,1]`, values `[2^53,2^53+2]` | Carry over `[0,0.5]` is one. The separately rounded cumulative endpoint values subtract to zero. |
| `sigma=(0.2,0.3), dt=0.25, rho=-0.5` | Ideal off-diagonal log-increment covariance is `-0.0075`. |
| `S0=100,q=0.03,t=(1,2),D=(0.98,0.95),sigma=(0.20,0.25)` on one clock | `W=(0.04,0.125)`. The second variance increment is `0.085` and its equivalent annual sigma is about `0.291547595`. ATM strikes are about `99.0250544` and `99.1331088`. |
| `t=(1,4),sigma=(0.25,0.125)`, or a future expiry 30 days away with source ACT/365F and model ACT/360 | First case gives a genuine zero second variance increment. In the second, source sigma `0.20` gives total variance about `0.00328767123` and model-equivalent sigma about `0.198625413`. |
| Local sigma `1`, carry zero, `S=100,dt=1,z=-2` | Log-Euler gives about `8.20849986`. The observed spot-Euler option returns `-100`. |

Independently qualify checked shapes, clocks and exact knots, coefficient
breaks and subdivisions, complete scale and cancellation arithmetic,
source support and callback errors, zero-vol limits and multiasset
covariance. For local volatility, compare strict/flat boundaries,
flat-grid reductions, independent vanilla benchmarks and grid refinement
on state-dependent surfaces. Qualify dated products and host paths with
their own tolerances.

## Evidence

Existing tests cover drift interpolation, interval selection, small
states, GBM log moments and flat or skewed local-volatility comparisons.
These tests document observed runtime expectations.
[GBM](../../../../finstack-quant/models/src/monte_carlo/process/gbm.rs),
[piecewise bootstrap](../../../../finstack-quant/models/src/monte_carlo/process/piecewise_gbm.rs),
[exact schemes](../../../../finstack-quant/models/src/monte_carlo/discretization/exact.rs),
[local volatility](../../../../finstack-quant/models/src/monte_carlo/process/local_vol.rs),
[raw simulation](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs),
[dated equity adapter](../../../../finstack-quant/valuations/equity/src/piecewise_gbm.rs),
[volatility resolution](../../../../finstack-quant/valuations/core/src/instruments/common_impl/vol_resolution.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
