# MODELS-MONTE-CARLO-11: Bounded utilization, short-rate and CIR spread joint process

- Decision status: **Accepted — complete checked revolving-credit paths package, 2026-11-11**.
- Implementation status: Dedicated joint paths and shared component kernels exist. Checked stepping, explicit modes and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent arithmetic and integration examples reviewed. Runtime, host and product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked revolving-credit paths

1. **Require one dedicated checked joint transition.** The
   three coordinates are utilization `U` in `[0,1]`, a signed
   decimal rate level with a declared role, and nonnegative
   decimal credit spread `s`. The caller declares the measure
   and purpose of the joint simulation. Validate the immutable model,
   mode-specific initial state, correlation selection, grid
   and work requirements before drawing. The scheme reads
   one complete opening state and commits the complete
   receiving state after all components pass. Reject generic
   Euler and incompatible process/scheme or correlation
   pairings. Low-level inputs and stepping remain Rust-owned.
   [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

2. **Bind clocks, preparation and observations to actual knots.**
   Use actual endpoints `a<b`, `h=b-a` and the model's declared
   clock. Bind prepared coefficients and the factor to the
   admitted model and the complete stored grid. The dated
   facility uses its actual simulation anchor and ACT/365F
   model time, with contractual accrual and each market source
   clock mapped separately. Keep requested product events
   on their exact knots. Use an origin-only grid when no
   interval is needed. Remove the dummy step, reconstruction
   of the right endpoint and hidden refinement in the kernel.
   Direct and prepared paths use the same laws and checks.
   [Accepted time grids](../core-numerics/time-grids.md),
   [accepted model and source clocks](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection).

3. **Select the left-node utilization target explicitly.** Require
   finite `theta` in `[0,1]` and a finite signed sensitivity
   `beta`. Zero sensitivity selects the constant target `theta`.
   Nonzero sensitivity requires a strictly positive initial
   spread `s_ref` at the run anchor. Freeze that reference for
   the run and use `theta_a=clip(theta+beta*(s_a-s_ref)/s_ref,0,1)`
   from the opening spread. At `s_a=s_ref` the target is exactly
   `theta`. A subsequent zero spread gives a relative change
   of minus one. Evaluate the scaled expression before applying
   the target cap. Reject an invalid reference before drawing.
   The receiving spread first affects the next interval.

4. **Select the clamped Gaussian utilization law.** Require
   finite `kappa_U>0` and `sigma_U>=0`. Set `x=kappa_U*h`,
   `d=exp(-x)`, `g=-expm1(-x)` and
   `E(y)=-expm1(-y)/y` with `E(0)=1`. Use
   `m=d*U_a+g*theta_a`, `nu=sigma_U*sqrt(h*E(2*x))` and
   `U_b=clip(m+nu*Z_U,0,1)`. Compute the two weights and
   complete noise loading in scale. The Gaussian proposal
   has the exact frozen-target OU marginal in ideal arithmetic.
   Clamping defines a discrete transition with boundary atoms
   and grid-dependent moments. Literal zero volatility keeps
   deterministic reversion and spread response. Retire both
   callers' tiny-volatility freezes. The product owns any
   contractual fixed-utilization rule.
   [Gaussian and utilization references](../../../REFERENCES.md#revolving-credit-utilization-and-joint-paths).

5. **Select the rate law independently of volatility.** A fixed
   rate is the supplied signed level. A deterministic forward
   schedule uses checked linear interpolation with closed
   coverage, finite signed values and strictly increasing
   finite knots, evaluated at the actual receiving endpoint.
   The dated caller prepares index observations on the native
   forward-curve clock, then binds them to exact model nodes.
   An explicit-level Hull–White mode uses record 07's checked
   endpoint law and its supplied initial rate. A dated fitted
   Hull–White mode uses the accepted centered state and curve
   shift, starting from `x_0=0`. Keep the selected law through
   its exact zero-volatility limit. Retire the curve-to-theta
   fit, fitted initial-rate override, scalar offset across
   unlike clocks and implicit flat or zero-rate fallback.
   [Accepted Hull–White endpoints](07-hull-white.md#accepted-decision-for-checked-hullwhite-endpoints-and-curve-fit-boundaries),
   [accepted curve fit](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection),
   [accepted interpolation](../core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics).

6. **Separate literal constant spread from checked CIR.** In
   constant mode the finite nonnegative input, including zero,
   is the spread at every node. In CIR mode use record 08's
   checked QE transition and its fixed regime rule. Allow the
   same zero and deterministic cases, with Feller as a
   diagnostic. Zero CIR volatility still permits mean reversion.
   Remove the synthetic CIR for constant spread and all floors
   on CIR inputs, state and output. Market-anchored mapping,
   hazard, LGD and fit qualification remain product choices.
   [Accepted CIR](08-cir-qe.md#accepted-decision-for-checked-cir-and-cir-quadratic-exponential-paths).

7. **Apply one correlation of latent step normals.** Select
   identity or an explicit strict `3×3` correlation in
   `[utilization, rate, credit_spread]` order. Use the accepted
   checked PSD factor with zero pivot tolerance, without
   repair or rank truncation of positive directions. Reject
   unresolved factorization or application. The scheme alone
   applies the factor. Assign three independent ordered normal
   slots on every positive interval, including deterministic
   coordinates and rank-deficient cases. An antithetic leg
   negates all assigned slots. Origin-only paths draw nothing.
   The matrix couples standardized OU/Hull–White endpoint
   noises and the latent normal used by QE. Clamping and the
   QE transform can change realized transition correlation.
   Unequal kernels can change exact endpoint correlations under
   continuous-time Brownian coupling. This scheme specifies
   a finite-step coupling.
   [Accepted correlation factor](../core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission),
   [accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

8. **Use the joint rate integral for dated fitted discounting.**
   Use the accepted Hull–White conditional Gaussian factor
   for the rate endpoint and its integral. The correlated rate
   normal is `Z_rate` in both loads, giving endpoint noise
   `s_r*Z_rate` and integral noise `l_r*Z_rate+r_r*Z_bank`.
   In this pricing composition reserve a fourth, independent
   normal after the three endpoint slots for `Z_bank`, even
   on a deterministic interval, and negate it on the antithetic
   leg. The integral residual is independent of utilization and
   spread normals conditional on the correlated rate normal.
   Accumulate the accepted shift and Gaussian integral on
   actual intervals and discount once at the owner's cash dates.
   Remove trapezoidal bank reconstruction from endpoint captures.
   This composition preserves the fitted Hull–White marginal
   and declares the cross-factor coupling to qualify.
   [Accepted joint Hull–White paths](../models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling).

9. **Publish checked and honestly named results.** Reject
   invalid opening utilization, nonfinite rate, negative
   spread, or inconsistent deterministic levels. Preserve
   scale in complete updates and apply saturation only where
   the utilization law defines it. Reject unresolved arithmetic,
   missing schedule coverage and draw failures with estimator,
   leg, interval and component context. Return no partial
   trajectory or estimate. Publish utilization under its own
   name, spread as `credit_spread`, and identify the rate as
   fixed, forward or Hull–White short rate. Remove the generic
   spot alias. Only an actual Hull–White short rate receives
   that label. State endpoints alone supply neither the bank
   account nor an integrated hazard.

10. **Reconcile the standalone and pool callers.** Migrate the
    revolver to the checked joint law and real anchor. Use
    checked shared utilization and spread kernels in the
    structured-credit pool while preserving its own systematic
    shock construction and two-slot layout. Keep contractual
    commitment, LC-cap event ordering and feedback, coupon and
    fixing projection, survival and default, funding, recovery,
    waterfall, and draw-option and loss counterfactuals with
    their owners. A matched seed fixes draw assignment, while
    a changed spread or link can change the utilization path.
    [Standalone revolver](../valuations-other-fixed-income/11-revolver-monte-carlo-credit.md),
    [risk and counterfactuals](../valuations-other-fixed-income/12-revolver-quotes-risk.md),
    [structured-credit revolver](../valuations-structured-credit/16-revolver-utilization-and-option-cost.md).

11. **Carry the contract through existing product surfaces.**
    Propagate the resolved model, clock, draw layout, rate role
    and errors through the existing Rust, Python and WASM
    revolver pricing, path and configuration surfaces. Reconcile
    affected state, schema, declarations, docs and parity there.
    Keep product selection of model inputs and calibrated or
    scenario values explicit. Retain the Rust-only low-level
    process without adding a generic raw simulator variant.
    Reconcile the shared interpolation helper's rough-Cheyette
    consumer under its own model contract.
    [Rough-Cheyette owner](16-rough-cheyette.md).

12. **Qualify the step law and economics separately.** Check
    ideal clamped moments and boundary mass, old-spread and
    exact-zero limits, CIR support, latent-normal and joint-bank
    moments, source clocks, replay and failures. Measure grid
    dependence of utilization, spread coupling and product PV
    independently of sampling error. Empirical drawdown evidence
    motivates testing linkage. Its coefficient, pricing measure,
    scenario stability and product acceptance need separate
    evidence. [Utilization references](../../../REFERENCES.md#revolving-credit-utilization-and-joint-paths).

## Observed behavior and limitations

The dedicated step applies correlated normals, uses the opening
spread for the capped target, clamps an OU draw, updates fixed,
explicit-mean Hull–White or linearly interpolated rate, and calls
CIR QE. Generic Euler can bypass coupling, endpoint saturation
and deterministic forward updates. The dedicated path ignores a
factor-application error and mutates state component by component.
The forward helper has flat tails, empty and mismatched input
fallbacks and a reconstructed right endpoint.

The standalone and pool callers freeze utilization below a
volatility threshold. The standalone caller maps constant spread
to a CIR surrogate, switches rate law at zero Hull–White
volatility, and discounts from captured endpoint trapezoids.
The pool already keeps constant spread literal and supplies its
own systematic and idiosyncratic shocks.

## Independent examples and qualification

| Example | Result under the proposal |
| --- | --- |
| `kappa_U=ln(2),h=1,U_a=.4,theta=.25,beta=.2,s_ref=.02,s_a=.04,sigma_U=0` | The target is `.45` and utilization becomes `.425`. At the reference spread it becomes `.325`. A zero receiving spread does not change either step. |
| Unlinked utilization with `kappa_U=ln(2),h=1,U_a=.2,theta=0` and one-step proposal SD `.4` | Clamped mass is about `.40129367` at zero and `.01222447` at one. Mean utilization is about `.21284404`, rising to `.23081568` on two equal half-steps with the same model parameters. |
| `theta=.25,beta=2^-1074,s_ref=2^-1000,s_a=2^72` | The complete target rounds to `.5` even though the isolated spread ratio overflows. |
| `h=1,kappa_U=2^1023,sigma_U=2^512,theta=.5,Z_U=.25` | The scaled noise is one and utilization becomes `.75`. Forming `2*kappa_U` first would overflow. |
| Literal constant spread `.04` | It stays `.04` for every draw. The observed standalone CIR surrogate has a one-year spread SD of about `.0001990`. |
| Ideal QE quadratic draw with `m=1,psi=1.5` and latent `Z` | `s_b=(1+Z)^2/2` has correlation `sqrt(2/3)` with `Z`, even with perfect latent-normal linkage. |
| Constant-sigma Ho–Lee limit with `h=sigma=1` and latent `corr(Z_U,Z_rate)=.6` | Endpoint variance is one, integral variance is `1/3`, and their covariance is `1/2`. The integral noise covariance with `Z_U` is `.3`. |

## Evidence

Located tests cover the unclipped OU mean, spread-linked target,
CIR tails and the old curve-clock theta integration.
[Process](../../../../finstack-quant/models/src/monte_carlo/process/credit_line.rs),
[dedicated stepping](../../../../finstack-quant/models/src/monte_carlo/discretization/credit_line.rs).

Existing production and host paths were traced.
[Standalone paths](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_generator.rs),
[standalone discounting](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_pricing.rs),
[pool paths](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_paths.rs),
[Python](../../../../finstack-quant-py/src/bindings/valuations/typed_revolving_credit/revolving_credit.rs),
[WASM](../../../../finstack-quant-wasm/src/api/valuations/typed/fixed_income.rs).

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
