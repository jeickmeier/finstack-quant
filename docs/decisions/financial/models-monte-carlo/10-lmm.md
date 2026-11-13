# MODELS-MONTE-CARLO-10: Displaced terminal-measure LMM predictor-corrector

- Decision status: **Accepted — complete checked displaced LMM and terminal-bond package, 2026-11-11**.
- Implementation status: The displaced LMM predictor-corrector and terminal-measure pricing routes exist. Checked transitions, bond ratios and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent arithmetic and integration examples reviewed. Runtime, host and product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked terminal-measure LMM paths

1. **Define a discrete-tenor, single-curve model.** Evolve `n>=1`
   simple forward rates `F_i` for `[T_i,T_(i+1)]` under the
   `T_n` terminal-bond measure. The same modeled bond family
   defines the forwards and terminal numeraire. Require finite
   strictly increasing tenor knots `0<=T_0<...<T_n` on one
   declared model-year clock, with reset at `T_i`. Require
   positive finite contractual accrual fractions `tau_i`,
   independent of the tenor-coordinate differences. Forward rates
   and per-rate signed displacements `d_i` are finite decimals.
   A forward with `T_i=0` starts at its supplied fixing and
   is frozen from origin. The product supplies any actual fixing.
   The model admits raw horizons through `T_n`, inclusive.
   [Model references](../../../REFERENCES.md#displaced-lmm-and-terminal-bond-pricing).

2. **Keep the entire displaced support inside the accrual domain.**
   Set `X_i=F_i+d_i` and `B_i=1+tau_i*F_i`. Require `X_i>0`
   and `B_i>0`, and admit a displacement only if the exact
   product `tau_i*d_i<=1`, including equality. Check the
   sign of the margin without first rounding that product to
   one. Then `B_i=(1-tau_i*d_i)+tau_i*X_i` stays positive
   throughout the positive shifted support. At equality the
   drift weight `q_i=tau_i*X_i/B_i` is exactly one. Signed
   displacements are allowed within this domain. No minimum
   positive denominator or displaced-level floor.

3. **Require immutable, explicitly shaped factor data.** Retain
   an explicit `K=2` or `K=3` with exactly `K` finite signed
   entries in each of `n` loading rows per regime. A loading
   is per square root of model year. Zero rows and dependent
   columns are valid. Derive `n` from the initial forwards and
   check all other lengths, allocation dimensions and restored
   state through one validated model constructor. For `m`
   strictly increasing breakpoints in `(0,T_n)`, require
   `m+1` loading matrices. The first matrix starts at zero,
   each breakpoint activates the next matrix and the last
   matrix covers the remaining horizon. An empty breakpoint
   vector specifies constant loadings. Remove the ignored third
   lane of a two-factor input. No correlation fit or default
   loading shape is implied.

4. **Bind fixings and loadings to exact grid nodes.** A grid
   contains every fixing and loading break within its horizon.
   Step over actual stored endpoints `[a,b]` and width `h=b-a`.
   Both predictor and corrector use the loading regime at `a`
   and the same alive set `T_i>a`. A rate fixing at `b`
   evolves on the incoming step, is observed at `b`, and is
   held exactly from that node onward. A loading change at `b`
   first affects the outgoing step. Reject a strict interior
   crossing, even by one representable time increment. Bind
   prepared work to the complete immutable model and actual
   knot sequence. Direct and prepared paths use the same rule.
   [Accepted time grids](../core-numerics/time-grids.md).

5. **Use one dedicated scheme and a fixed draw layout.** Select
   the displaced log predictor-corrector. Each positive interval
   uses `K` checked independent standard normals in loading-column
   order, even for zero loadings or fully frozen states. The
   loadings already determine covariance. Reject an additional
   correlation transform or a generic scheme pairing. Both
   stages reuse the same normals, and antithetic legs negate all
   `K` normals in the corresponding slots. An origin-only raw
   grid validates the model and consumes no draws.
   [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

6. **Average terminal-measure proportional drift.** With interval
   loading `ell_i=sqrt(h)*lambda_i(a)`, set
   `D_i(X)=-sum_(j>i) q_j(X)*(ell_i dot ell_j)` and
   `J_i=ell_i dot z-(ell_i dot ell_i)/2`. Predict all alive
   shifted rates with `X_i^p=X_i*exp(J_i+D_i(X))`. From that
   complete predictor recompute `D_i(X^p)` under the original
   interval coefficients. Correct from the original state with
   `X_i'=X_i*exp(J_i+(D_i(X)+D_i(X^p))/2)`. Frozen rates
   are carried through unchanged. Evaluate the proportional
   drift directly, with a deterministic checked accumulation.
   The empty terminal-rate drift is exactly zero.
   [Drift and predictor-corrector references](../../../REFERENCES.md#displaced-lmm-and-terminal-bond-pricing).

7. **Validate stages and publish only a complete step.** Preserve
   scale in the accrual weight, covariance and shock products,
   drift averaging and exponential update. Evaluate a small
   undisplaced rate change with the equivalent
   `F_i+X_i*expm1(log_increment)` when needed to preserve it.
   The predictor may retain its shifted representation. At
   publication check the actual rounded `F_i'` for finiteness
   and strict support, including the case where it rounds onto
   `-d_i`. Propagate invalid shape, knot, draw, state, resource
   and numerical errors with stage, leg, step and coordinate as
   applicable. No skipped term, invalid-predictor drift fallback,
   absorbed boundary, NaN sentinel, dropped path or hidden retry.

8. **State what is exact in the finite-step scheme.** In ideal
   arithmetic the terminal forward follows its exact shifted
   lognormal law until fixing, with the supplied piecewise
   loadings. Zero loading gives exact identity for that rate.
   With interacting rates the state-dependent drift approximation
   can bias both deflated bond moments and option values. Finite
   normal support and rounded arithmetic add separate limitations.
   [Discretization references](../../../REFERENCES.md#displaced-lmm-and-terminal-bond-pricing).

9. **Expose checked tenor-bond ratios with explicit maturities.**
   For `0<=k<=l<=n` and `t<=T_k`, compute
   `P(t,T_l)/P(t,T_k)=product_(j=k..l-1) 1/B_j` from the
   admitted state, with exact identity for `k=l`. Preserve
   range through scaled products or stable sums of log factors,
   including accurate near-unit factors and cancellation in
   `1+tau_i*F_i`. Reject an unresolved or unrepresentable
   requested result. Ratios may exceed one. At `t=T_k` the
   denominator bond is one and the product includes the just-fixed
   rate when `l>k`. At `t<T_k` an absolute bond price also needs
   `P(t,T_k)` from an owner-qualified stub model. Retire
   `lmm_numeraire` and `lmm_first_alive` as generic state keys.
   Dynamics use the first `T_i>t`, while a tenor-bond projection
   can start at `T_i=t`. At `T_n` the terminal bond is one.

10. **Keep terminal-measure discounting with the pricing owner.**
    A payoff valued at `t` contributes `H_t/P(t,T_n)` in
    terminal units. Multiply the expectation by the same curve's
    positive `P(0,T_n)` once. The co-terminal Bermudan caller
    binds dated exercises to exact tenor indices, uses the shared
    checked transitions and tenor-bond arithmetic, and propagates
    projection or annuity failures. For a payment `H_i` fixed
    at `T_i` and paid at `T_(i+1)`, the terminal-unit amount
    at fixing is `H_i*P(T_i,T_(i+1))/P(T_i,T_n)`. For a
    co-terminal payer swap at `T_k`, let `G_j=P(T_k,T_j)/P(T_k,T_n)`.
    The unit-notional value in terminal units is
    `G_k-1-strike*sum_(j=k..n-1) tau_j*G_(j+1)`.
    The product supplies the payoff sign and exercise rule.
    Keep generic scalar pricing limited
    to caller-qualified terminal-equivalent payoffs. Contractual
    schedule, exercise, stopping policy, loading and displacement
    construction, calibration and product tolerances retain their
    owning records. This package admits no generic off-tenor
    interpolation, multicurve discounting or stochastic short-rate
    projection.
    [Bermudan owner](../valuations-rates/09-lmm-bermudans.md),
    [calibration owner](../calibration-rate-models/09-lmm-rebonato-calibration.md).

11. **Reconcile existing raw simulation and transport.** Require
    explicit `lmm_predictor_corrector` selection. Use the model's
    `initial_forwards` once, removing the duplicate raw initial
    vector. Publish undisplaced `forward_i` coordinates at actual
    knots, including frozen fixings. Reconcile the existing Rust,
    Python and WASM raw routes, schema, state, declarations,
    parity and error propagation. Keep the low-level scheme and
    checked bond arithmetic Rust-owned.
    [Raw simulation owner](28-capture-and-simulation-contracts.md).

12. **Qualify the law and actual prices independently.** Check
    support and rounding boundaries, two- and three-factor steps,
    deterministic, frozen and terminal-rate cases, exact fixing
    and loading endpoints, bond-ratio orientation, failure and
    replay through existing routes. Compare terminal and
    nonterminal caplets to displaced Black on the applicable
    payment measure with the proper terminal-measure weights.
    Check earlier deflated bond moments, multi-forward European
    prices and product convergence under explicit grid refinement.
    Keep grid bias separate from sampling error and exercise-policy
    bias. Terminal-forward and shared-kernel replay checks alone
    cannot establish these properties.

## Observed behavior and limitations

The current code defaults to the dedicated predictor-corrector in
raw dispatch. It uses two or three columns of three-wide loading
rows and asks for the initial forwards twice in the raw request.
The grid validator requires exact interior knots. The direct step
reconstructs its endpoint, permits crossings within a floating-point
tolerance and writes NaNs on recognized crossings. The drift skips
small accrual denominators, leaves invalid displaced states unchanged
and assigns zero corrected drift to a nonpositive predictor. The
initial checks alone allow displacements whose support crosses an
accrual denominator pole.

The process publishes a relative bond product as `lmm_numeraire`
and reports the first included period as `lmm_first_alive`, which
differs from the evolution index at fixing equality. The Bermudan
caller snaps exercise times, uses a separate period tolerance and
maps failed annuity or numeraire calculations to zero. The generic
scalar engine does not use the pathwise bond product automatically.

## Independent examples and qualification

| Example | Expected behavior under the proposal |
| --- | --- |
| `tau=d=1,F=-1+2^-53` | `B=2^-53` and `q=1` exactly. This valid drift contribution survives the old `1e-15` skip threshold. |
| `tau=1-2^-53,d=1+2^-52` | The rounded product is one, but the exact product exceeds one by `2^-53-2^-105`. Reject the shift. |
| `h=.25,z=[.6,-.4],F=[-.01,.03,.05],d=[.025,.02,.01],tau=[.48,.53,.49]` with all rates alive and loading rows `[[.4,.1],[.2,-.1],[.3,.2]]` | The corrected rates are approximately `[-.00879522943350,.03381118025289,.05205955957427]`. The last rate equals its predictor. |
| Terminal forward `F=.03,d=.01`, `h=2^-1074`, loading `[2^537,0]` and zero shocks | The interval loading is `[1,0]`. The updated forward is about `.0142612263885`, even though the square of the unscaled loading overflows. |
| Stored interval `[.25,nextUp(.5)]` with fixing or loading break `.5` | Reject the interior crossing. The incoming interval `[.25,.5]` is valid. |
| Deterministic `P(0,T0)=.95,P(0,T1)=.91,P(0,T2)=.86` and `tau=[.49,.51]` | The terminal bond at `T0` is `.86/.95`, including the rate fixed there. Just after `T0`, `.86/.91` is relative to `T1`. At `T0` a unit-notional payer with strike `.07` gives PV `.028085` using either direct discounting or terminal-bond deflation. A caplet at the same strike, fixed at `T0` and paid at `T1`, has PV `.008787`. |
| One ideal-normal step of width one before both fixings with `F=[1,1]`, `d=[0,0]`, `tau=[1,1]` and both loading rows `[1,0]` | The initial deflated first bond is `4`. Direct normal integration gives a one-step expected deflated first bond of about `4.01818227`, although the terminal forward has the exact lognormal step. |

## Evidence

Located tests cover terminal caplet pricing, freezing, the terminal
forward mean, loading endpoints and production-loop equality.
[Process and drift](../../../../finstack-quant/models/src/monte_carlo/process/lmm.rs),
[predictor-corrector and tests](../../../../finstack-quant/models/src/monte_carlo/discretization/lmm_predictor_corrector.rs),
[raw simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests/non_markov.rs),
[Bermudan numeraire tests](../../../../finstack-quant/valuations/rates/src/swaption/pricing/tests/numeraire.rs).

The existing dispatch, Bermudan stepping and calibration inputs were
traced through the current Rust and host surfaces.
[Raw dispatch](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[Bermudan adapter](../../../../finstack-quant/valuations/rates/src/swaption/lmm_pricer.rs),
[Bermudan kernel](../../../../finstack-quant/valuations/rates/src/swaption/pricing/lmm_bermudan.rs),
[calibration](../../../../finstack-quant/calibration/src/lmm.rs),
[Python](../../../../finstack-quant-py/src/bindings/models/monte_carlo/engine.rs),
[WASM](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs).

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
