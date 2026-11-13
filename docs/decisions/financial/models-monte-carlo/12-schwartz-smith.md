# MODELS-MONTE-CARLO-12: Schwartz–Smith joint Gaussian transition and futures expectation

- Decision status: **Accepted — complete checked Schwartz–Smith paths and expectations package, 2026-11-11**.
- Implementation status: The exact Gaussian transition and commodity pricing caller exist. Checked state, scaled covariance and expectation, and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent arithmetic examples reviewed. Runtime, host and product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Schwartz–Smith paths and expectations

1. **Use one explicit two-factor law.** In a declared commodity
   quote unit and model-year clock, let `log S=X+Y`, where `X`
   is the short deviation and `Y` the long log level. Use
   `dX=(-kappa*X-lambda_x)dt+sigma_x*dW_x` and
   `dY=mu_y*dt+sigma_y*dW_y` with instantaneous Brownian
   correlation `rho`. Require finite `kappa>=0`, both finite
   volatilities `>=0`, finite `rho` in `[-1,1]`, and explicit
   finite signed `lambda_x` and effective `mu_y`. Kappa is per
   model year, drifts are log units per model year and
   volatilities are log units per square root of model year.
   Admit zero speeds and volatilities as finite-horizon limits.
   Validate and restore one immutable parameter value, including
   explicit zero risk shift on the wire.
   [Schwartz and Smith](../../../REFERENCES.md#schwartz-smith-2000).

2. **Require an explicit initial factor split.** Admit exactly
   two finite coordinates `[X0,Y0]`. Keep the checked
   `from_spot` convenience with a required finite `X0` and
   a finite positive `S0`. Set `L0=log(S0)` and choose
   the rounded `Y0=L0-X0`. Require the checked rounded sum
   `X0+Y0` to recover the binary64 log anchor `L0`. Refuse
   an unresolved split. Spot is derived from the stored factors,
   with ordinary log/exp rounding. Remove the hidden `X0=0`
   default and the unchecked constructor. A given spot admits
   different short-factor allocations and forward expectations.

3. **Use one checked exact transition on the actual grid.**
   Require the dedicated `exact_schwartz_smith` scheme. Bind
   prepared coefficients to the complete immutable model and
   exact stored knots, using actual adjacent widths `h=b-a`.
   Direct and prepared evaluation use the same formulas. An
   admitted origin-only grid returns the checked initial state
   without a draw. Remove generic Euler, implicit dispatch and
   the first-step cache with a mismatched-grid fallback.
   [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [accepted grids](../core-numerics/time-grids.md).

4. **Use exact means and the integrated covariance.** Put
   `u=kappa*h`, `E(v)=-expm1(-v)/v` with `E(0)=1`.
   The conditional means are `m_x=exp(-u)*X-lambda_x*h*E(u)`
   and `m_y=Y+mu_y*h`. The variances and covariance are
   `V_x=sigma_x^2*h*E(2*u)`, `V_y=sigma_y^2*h` and
   `C_xy=rho*sigma_x*sigma_y*h*E(u)`. At `kappa=0`, the
   short factor is Brownian with drift `-lambda_x`. A zero
   volatility removes the respective noise and preserves
   its conditional mean. Keep complete sums and products in
   scale, including `h*E(u)` at small or large `u`.
   [Joint Gaussian references](../../../REFERENCES.md#schwartz-smith-joint-transition-and-spot-expectation).

5. **Preserve the orthogonal noise at nearly singular inputs.**
   Define `q=E(u)/sqrt(E(2*u))=sqrt(tanh(u/2)/(u/2))`
   with `q(0)=1`, and
   `w=sqrt((1-abs(rho))*(1+abs(rho))+rho^2*(1-q^2))`.
   Use X-first loadings `a_x=sigma_x*sqrt(h*E(2*u))`,
   `b_y=rho*sigma_y*sqrt(h)*q`, and
   `c_y=sigma_y*sqrt(h)*w`. Add `a_x*Z_x` to `m_x` and
   `b_y*Z_x+c_y*Z_y` to `m_y`. Preserve `sqrt(1-q^2)`
   directly in scale, using its small-`u` expansion beginning
   `u*sqrt(1/12-u^2/120+17*u^4/20160)` where applicable.
   The factor has rank two when `h,kappa,sigma_x,sigma_y`
   are all positive, including at `rho=+1` or `rho=-1`.
   Use exact structural zero loadings in degenerate cases.
   Remove correlation clamps and factor repair.
   [Shared residual arithmetic](../models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling).

6. **Assign two raw normal slots per positive step.** Use
   independent standard normals in order `(Z_x,Z_y)` and
   correlate only through the scheme's integrated loadings.
   Preserve the triangular orientation and both slots when
   either factor is deterministic or the factor has rank one.
   Antithetic legs negate both normals. Reject a second
   correlation transform, bad shapes, and failed draws.
   [Accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

7. **Share a checked spot projection and define its limits.**
   Raw state stores the two rounded factors. A requested spot
   is the positive checked `exp(X+Y)` from those factors,
   with positive subnormals allowed. Check the complete sum
   and scale of the exponential. Raw factor capture requires
   no spot projection. Reject unresolved required outputs,
   overflow and positive spot rounded to zero. Publish a
   complete step or return contextual errors with interval,
   leg and operation. Very large opposite factors can lose
   log-spot detail on materialization. The ideal transition
   and expectation do not certify that rounded path regime.

8. **Use one checked conditional spot expectation.** For an
   admitted current state and finite `tau>=0` in model years,
   evaluate `log E[S_tau]=m_x+m_y+V_log/2` using the same
   mean and loading laws at `h=tau`. Use
   `V_log=(a_x+b_y)^2+c_y^2`. Form the common loading as
   `sqrt(h)*q*[(sigma_x+rho*sigma_y)-sigma_x*g/2]`, where
   `g=-expm1(-u)`, and preserve the complete signed expression.
   This retains positive variance when rounded opposing
   loadings appear to cancel. Evaluate the combined mean and
   half variance in scale. A zero horizon returns the current
   log spot or checked spot, as requested. Keep a log view for
   checked arithmetic and an exponentiated level view. Replace
   the unconditional `futures_price` label with `expected_spot`.

9. **State the measure and quote semantics.** The kernel
   returns the undiscounted conditional spot mean under the
   supplied drifts. The caller supplies the measure and its
   provenance. The original physical convention has
   `lambda_x=0`; the pricing law subtracts the constant
   short drift shift at unchanged `kappa` and takes `mu_y`
   already adjusted. Do not infer the measure from either
   coefficient or infer that a degenerate supplied law comes
   from an equivalent change of measure. The expectation
   can be interpreted as a model futures price under the
   qualified risk-neutral futures convention. A forward
   generally uses a discount-weighted expectation and agrees
   under deterministic discounting or independence of
   spot and discount. Market fit and curve provenance require
   separate evidence.
   [Measure and quote references](../../../REFERENCES.md#schwartz-smith-joint-transition-and-spot-expectation).

10. **Reconcile the commodity spot-option owner.** Use the
    checked transition and log expectation. Where the owner
    selects a single-horizon forward pin, solve directly for
    the effective `mu_y` from the positive target's logarithm
    less the expectation exponent excluding long drift,
    divided by the positive horizon. Check the resulting pin
    with the owner's price accuracy criterion. This avoids
    intermediate level ratios and a rounded drift adjustment.
    The owner selects the state split, quote source, model
    clock, pin, parameters, discount, settlement and risk.
    Matching one expiry imposes one mean constraint. Curve
    calibration and risk-premium identification stay separate.
    [Commodity spot-option owner](../valuations-fx-and-exotics/commodity-spot-option.md).

11. **Reconcile existing raw and pricing routes.** Keep one
    raw `[X0,Y0]` vector and the `x`, `y` factor outputs at
    actual nodes in Rust, Python and WASM. Validate raw wire
    state, parameters, explicit scheme and ranges canonically.
    Retire NaN fill values for missing coordinates. Reconcile
    schemas, declarations, Rust errors, parity and replay
    metadata across the existing raw and product routes.
    Keep low-level state construction, transition and
    expectation Rust-owned.
    [Raw simulation owner](28-capture-and-simulation-contracts.md).

12. **Qualify the law and the financial use independently.**
    Check means, covariance, joint loading orientation,
    exact zero limits and near-singular ranges, nonuniform
    partition moments, projection, replay and failures.
    The ideal Gaussian law at a given knot is invariant under
    step partition. Pathwise replay across different grids,
    finite precision, finite normal support and between-node
    payoff monitoring have separate limits. Test a European
    terminal payoff against an independent lognormal price
    using full two-factor terminal variance and the qualified
    discount and forward. Assess calibrated market fit and
    product accuracy under their own controls.

## Observed behavior and limitations

The live code validates strictly positive speed and volatilities
at its checked parameter constructor, defaults `lambda_x` to
zero and accepts publicly mutable parameters and unchecked
factor state. Its spot constructor defaults the short factor
to zero. Raw dispatch defaults to the exact scheme and permits
Euler. The exact scheme caches the first width, reconstructs
the endpoint correlation and clamps it. Computing its residual
from that rounded value can erase nonzero orthogonal noise.
The unchecked futures function accepts invalid maturities and
can lose a positive log variance when opposing covariance terms
cancel. The option caller uses the spot constructor's default
split and adjusts long drift through a ratio of price levels.

## Independent examples and qualification

| Example | Expected behavior under the proposal |
| --- | --- |
| `kappa=ln(2),h=1,sigma_x=.30,sigma_y=.20,rho=-.65,X=.12,Y=4.30,lambda_x=.02,mu_y=.04` | Means `.0455730495911` and `4.34`, variances `.0486909576300` and `.04`, covariance `-.0281325532973`, and expected spot `81.5964678686`. Splitting the interval at `.25` preserves the ideal terminal moments. |
| `kappa=2^-54,h=1,sigma_x=sigma_y=2^54,rho=-1` with zero state and drifts | Common log loading is about `-.5`, orthogonal loading `.2886751346`, and log variance `1/3` to the shown precision. Expected spot is about `1.1813604129`. At exactly `kappa=0` the log variance is zero. With `Z_x=1,Z_y=0`, separately rounded factors in the positive-`kappa` case sum to zero while the ideal log spot is about `-.5`. |
| `kappa=h=2^1023,sigma_x=2^512,sigma_y=2^511,rho=1` with zero state and drifts, `Z_x=1,Z_y=0` | Both complete increments round to one even though `kappa*h` and `2*kappa` individually overflow binary64. |
| `S0=100,X0=1e20` | Rounded factors reconstruct log spot zero. The checked spot constructor rejects the split. |
| Zero volatilities and drifts, `kappa=ln(2),h=1` and `S0=100` | Changing `X0` from `0` to `.2`, with the same spot, lowers the log expectation by `.1`. |
| `T=kappa=1,S0=100,X0=0,lambda_x=0,sigma_x=.3,sigma_y=.15,rho=.3` with a target of `103` | Full terminal log variance is `.07847716734`. Direct log pin gives `mu_y=-.00967978143`. A unit European call at strike `100` and discount `.97` has lognormal benchmark `12.48293125517`. |

## Evidence

Existing tests cover Gaussian means and covariance, raw field names,
partition moments and commodity pricing. Some comparisons reuse
production formulas. [Process and expectation](../../../../finstack-quant/models/src/monte_carlo/process/schwartz_smith.rs),
[transition](../../../../finstack-quant/models/src/monte_carlo/discretization/schwartz_smith.rs),
[simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs),
[partition tests](../../../../finstack-quant/models/tests/lookback_commodity_regressions.rs),
[commodity option tests](../../../../finstack-quant/valuations/exotics/tests/commodity_spot_option/mc_pricing.rs).

Existing production and host routes were traced.
[Raw dispatch](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[commodity option](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/types.rs),
[Python](../../../../finstack-quant-py/src/bindings/models/monte_carlo/engine.rs),
[WASM](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs).

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
