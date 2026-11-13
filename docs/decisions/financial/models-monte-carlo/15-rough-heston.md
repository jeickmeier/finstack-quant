# MODELS-MONTE-CARLO-15: Rough Heston Volterra-history hybrid discretization

- Decision status: **Accepted — complete checked rough Heston paths and pricing package, 2026-11-11**.
- Implementation status: The hybrid Volterra scheme and equity route exist. Checked grid binding, variance updates and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent kernel, projection and option-price examples reviewed. Runtime, host and pricing qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked rough Heston paths and pricing

1. **Select the fractional square-root variance law and its domain.**
   Set `alpha=H+1/2` and `K(u)=u^(alpha-1)/Gamma(alpha)`.
   Under the declared pricing measure, convolve drift
   `kappa*(theta-V)` and diffusion `sigma_v*sqrt(V)` with
   this kernel from origin, with initial variance `v0`.
   Keep typed `0<H<1/2`, finite positive `v0`, `kappa`,
   `theta` and `sigma_v`, finite `-1<rho<1` and positive
   finite spot, including subnormals. Both correlation signs
   are eligible. Use a checked immutable rough parameter
   record and validate strict restoration. Keep fractional
   `kappa` and `sigma_v` in model-years to power `-alpha`
   when variance rates are per model year. Apply no classical
   Feller or mean-reversion-to-leverage admission test.
   [Affine Volterra law](../../../REFERENCES.md#rough-heston-hybrid-paths-and-projection),
   [accepted Fourier domain](../models-analytic/rough-heston-riccati.md#accepted-decision-for-checked-rough-heston-pricing).

2. **Bind the model, carry and complete history to one actual grid.**
   Use the exact stored zero-origin knots `t_0,...,t_n` and
   widths `h_j=t_(j+1)-t_j`, including on irregular grids.
   Prepare the dedicated `rough_heston_hybrid` for the complete
   immutable model, typed Hurst and actual knots. A same-count
   grid or a changed model requires new preparation. Require
   finite signed flat rates with `C_j=(r-q)*h_j` or a checked
   cumulative carry increment at the actual endpoints. Bind
   the declared clocks and coverage. Keep carry distinct from
   the payoff settlement discount. The path starts from its
   qualified origin with empty history. An admitted origin-only
   raw grid returns its origin state without draws. Remove
   regenerated grids, reconstructed receiving times and hidden
   substeps or partial-history restart.
   [Accepted grids](../core-numerics/time-grids.md),
   [accepted carry](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).

3. **Use checked normalized kernel weights on actual cells.**
   At receiving node `k` and cell `j<k`, let
   `A=t_k-t_j`, `B=t_k-t_(j+1)`. The drift weight is
   `D_kj=(A^alpha-B^alpha)/Gamma(1+alpha)` and the shared
   normal load is `Q_kj=D_kj/sqrt(h_j)`. At the newest cell
   `j=k-1`, use `B=0` exactly. For older cells evaluate
   the power divided difference using the actual width,
   `log1p` and `expm1` for close lags, preserving positive
   `B` near unit relative width. Retain the original H and
   `beta=1/2-H` so rounded `alpha` cannot select a limiting
   law. Use the accepted positive log-Gamma kernel. Carry
   scale through complete required products, with one kernel
   normalization. Propagate unresolved required weights.
   [Accepted special functions](../core-numerics/distribution-functions.md),
   [kernel references](../../../REFERENCES.md#rough-heston-hybrid-paths-and-projection).

4. **Sample the newest kernel integral jointly with Brownian leverage.**
   For newest cell `j`, use
   `R_j=h_j^H*beta/[Gamma(1+alpha)*sqrt(2H)]` for the
   independent residual. The two loads `Q_(j+1)j` and `R_j`
   give unit-integrand noise variance
   `h_j^(2H)/[2H*Gamma(alpha)^2]` and covariance `D_(j+1)j`
   with `Delta W_j=sqrt(h_j)*Z_var`. Evaluate the residual
   directly without subtracting rounded variances or
   recovering `2H` from rounded `alpha`. Older cells retain
   only their projection onto the same `Z_var`. This preserves
   their kernel/Brownian covariance and approximates their
   full stochastic kernel contribution. Reuse checked cell
   arithmetic from record 13 where applicable. Rough Heston
   applies path-dependent amplitudes and its own Gamma
   normalization inside the convolution.
   [Accepted fractional arithmetic](13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers).

5. **Use full chronological drift and noise history.** At the
   left node `j`, form `d_j=kappa*(theta-V_j)` and
   `a_j=sigma_v*sqrt(V_j)` from the admitted projected
   variance. At node `k=j+1` compute
   `V_hat_k=v0+sum_(m<k)[D_km*d_m+Q_km*a_m*Z_var,m]
   +R_j*a_j*Z_bridge,j`. Freeze each integrand on its own
   cell and retain every cell. Earlier bridge shocks can
   affect later integrands through their projected states.
   Only the current newest bridge enters the convolution
   directly. Use deterministic checked signed accumulation
   in scale. Bind history to a checked integer interval
   index and fresh state per leg. Remove the floating-point
   step counter and NaN sentinel.

6. **Select explicit positive-part projection.** Set
   `V_k=max(V_hat_k,0)` only after resolving the complete
   provisional result and its sign. A negative provisional
   variance is a valid outcome of the discrete scheme.
   Publish exact zero and use the projected value in later
   coefficients and spot steps. At zero, the cell has
   variance drift `kappa*theta` and no new variance noise.
   Prior history remains active, so zero can be followed by
   positive variance. Reject invalid opening state and
   unresolved numerical results before projection. Remove
   input clamping, variance floors above zero, redraws and
   path dropping. Projection changes finite-grid moments
   and can create a substantial zero atom.
   [Projection and convergence context](../../../REFERENCES.md#rough-heston-hybrid-paths-and-projection).

7. **Advance spot using the adapted left variance.** Set
   `I_j=h_j*V_j`,
   `Z_s=rho*Z_var+sqrt((1-rho)*(1+rho))*Z_independent` and
   `S_(j+1)=S_j*exp(C_j-I_j/2+sqrt(I_j)*Z_s)`. Use the
   actual cell width and checked scaled exponential product.
   At left variance zero the spot step is carry-only. The
   newly computed variance first affects the next cell.
   Preserve ordered normals `(Z_independent,Z_var,Z_bridge)`
   in every positive cell and negate all three on an
   antithetic leg. Apply correlation once inside the scheme.
   Commit both coordinates and the history position together,
   or fail with the estimator, leg, cell and operation.
   [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

8. **Apply the resource guard through every caller.** Retain
   the existing inclusive `8,000` actual interval limit in
   the shared plan. Preflight triangular entry counts, all
   coefficient tables, residual and history buffers, capture,
   byte layouts, concurrent storage and aggregate work
   across legs using checked arithmetic and fallible
   reservation. Report quadratic setup and history work per
   path. The ceiling is a resource admission rule. The caller
   qualifies feasible work and accuracy within it. Remove
   the warning-only split between low-level and raw routes.
   Fail before draws on inadmissible resources. No memory
   window or alternate approximation on failure.

9. **Use direct European estimators with fixed-grid moment limits.**
   Adapted log-Euler gives `E[S_(j+1)|F_j]=S_j*exp(C_j)`
   under ideal Gaussian draws. For a fixed finite admitted
   grid put `R=1+||Z||`, with `Z` the full Gaussian vector.
   The projected recursion bounds `V_j` by a finite constant
   times `R^d_j`, where `d_0=0` and `d_(j+1)=1+d_j/2<2`.
   The absolute terminal log spot has the same subquadratic
   growth bound with exponent `d_n`. Gaussian tails therefore
   give finite terminal spot power moments on that fixed grid.
   Use direct call and put observations, with complete
   antithetic pair means when selected and the accepted
   fixed-budget inference. Qualify actual tails and precision.
   The bound is grid-dependent. Continuum moment explosions,
   refinement-uniform inference, finite normal support and
   rounded range are separate questions.
   [Continuum moment references](../../../REFERENCES.md#rough-heston-hybrid-paths-and-projection).

10. **Use classical and deterministic laws as qualification limits.**
    The formal `H` approach to `1/2` on a fixed grid gives a
    signed Euler accumulator, with vanishing newest residual:
    `V_hat_(j+1)=V_hat_j+d_j*h_j+a_j*sqrt(h_j)*Z_var,j`.
    The coefficients still use `V_j=max(V_hat_j,0)`. The
    stochastic domain stays open at that boundary. For
    vanishing `sigma_v`, compare grid
    convergence with deterministic fractional relaxation
    `theta+(v0-theta)*E_(alpha,1)(-kappa*t^alpha)` and
    integrated variance `theta*T+(v0-theta)*T*E_(alpha,2)(-kappa*T^alpha)`.
    The projected left-grid method approximates that area.
    Use exact endpoint and zero-volatility values in independent
    references. Preserve separate rough Fourier, classical
    QE-M and rough Monte Carlo admissions and methods.
    [Accepted Fourier limit references](../models-analytic/rough-heston-riccati.md#independent-bounded-reference-checks).

11. **Reconcile the existing raw and host contracts.** Require
    explicit `rough_heston_hybrid` selection. Use the model
    block's `v0` once, with only initial spot in the raw
    request. Build internal `[spot,v0]` and publish checked
    spot and projected variance at actual nodes. Retire
    duplicate bitwise initial-variance checks and generic
    Markov stepping. Reconcile existing Rust, Python and WASM
    raw and pricing paths, model state, strict wire schemas,
    declarations, errors and parity. Keep kernel preparation
    and stepping Rust-owned.
    [Raw simulation owner](28-capture-and-simulation-contracts.md).

12. **Reconcile the equity pricing owner.** Preserve dated
    exercise and settlement resolution and live European
    eligibility without future cash dividends. Resolve only
    the active model and market dependencies. Share the
    qualified rough market scalars on the overlap with Fourier,
    with separate numerical controls and admission. Use one
    actual grid and explicit estimator count, seed, pairing
    and execution selection under records 01 and 02. Apply
    the qualified native payoff discount once. Preserve signed
    quantity, currency and truthful sampled zero uncertainty.
    Market fit, settlement, model selection and risk acceptance
    stay with the product owner.
    [Equity rough-volatility adapters](../valuations-equity-and-composition/11-rough-volatility-adapters.md).

13. **Qualify the scheme, estimator and pricing use independently.**
    Check kernel and leverage moments, same-count binding
    failures, boundary projection, adapted spot, replay and
    resource behavior. Use independent one- and two-cell
    Gaussian references and compare refined-grid prices and
    fractional means with separately qualified finite-H
    continuum evidence. Measure projection bias, numerical
    and tail failures and grid error separately from sampling
    uncertainty. Price and monitoring accuracy remain
    product-specific. Existing MC/Fourier agreement is
    cross-method evidence within the workspace.

## Observed behavior and limitations

The current scheme rebuilds receiving times and uses separate
grid construction in raw dispatch and the equity pricer. The
generic engine checks step count alone. The implementation
projects variance with `max` and uses a floating-point step
counter. Raw dispatch defaults to the hybrid and duplicates
the initial variance in state. The equity route uses a fixed
uniform budget and a separate pricing loop configuration.
Tests locate model moments, zero projection, history reset
and an ignored ATM Fourier comparison. Those comparisons
give limited price evidence for the existing grid law.

## Independent examples and qualification

| Example | Expected result or limitation |
| --- | --- |
| `H=.25`, unit newest cell, unit diffusion amplitude | The shared load is `1.08806525213`, residual `.38468915908`, and noise variance `1.33187174201`. At node 2 the old load for `[0,1]` is `.74183508803`. |
| `H=.25`, older cell `[0,2^-60]` at node `1` | The two individually rounded lags coincide. The actual width gives a drift weight about `7.078096261e-19` and shared load `7.600047990e-10`. |
| `H=nextDown(.5)` and newest width `1` | `alpha` rounds to one. The newest residual remains positive, about `5.551115123e-17`. |
| Stored interval `[2^-53,1+2^-52]` | Its width rounds to one. Reconstructing the endpoint loses a representable increment. Kernel geometry uses the stored receiving time. |
| `H=.25,kappa=1,theta=v0=.04,sigma_v=.2`, unit intervals, first `Z_var=-1` and first bridge zero | The first provisional variance is `-.00352261009` and projects to zero. The next variance is `.05384920656` from retained history and current drift. The corresponding first-step zero atom is about `.19310789`; projection raises the mean from `.04` to `.04492777`. |
| Formal `H=1/2` reference, `kappa=1,theta=v0=.04,sigma_v=.5`, two unit cells with first `Z_var=-1` | The signed variance accumulator goes from `-.06` to `-.02`. Both published variances are zero. The signed history survives projection at the first node. |
| Two cells `[0,.5,1]`, `H=.25,kappa=1,theta=v0=.04,sigma_v=.2,rho=-.7,S0=100,K=102`, total carry `.03`, discount `.97` | Conditional Gaussian quadrature gives put PV `7.40352430588` and direct call PV `8.41761409937`. Grid parity holds to quadrature precision. |
| Deterministic reference `H=.25,v0=.09,theta=.04,kappa=.5,T=1` | Continuum integrated variance is about `.07757589337`. The formal zero-volatility left-grid area is `.08191290663` on two equal cells and `.07760344830` on 256 cells. |

## Evidence

Existing source and selected tests were inspected.
[Process](../../../../finstack-quant/models/src/monte_carlo/process/rough_heston.rs),
[hybrid and inline tests](../../../../finstack-quant/models/src/monte_carlo/discretization/rough_heston.rs),
[raw dispatch](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
[equity MC route](../../../../finstack-quant/valuations/equity/src/equity_option/rough_heston_mc_pricer.rs),
[market scalars](../../../../finstack-quant/valuations/equity/src/equity_option/rough_heston_market.rs),
[Python raw route](../../../../finstack-quant-py/src/bindings/models/monte_carlo/engine.rs),
[WASM raw route](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs).

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
