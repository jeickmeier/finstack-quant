# MODELS-MONTE-CARLO-13: Fractional Brownian and Riemann–Liouville noise generation

- Decision status: **Accepted — complete checked fractional-noise and Volterra driver package, 2026-11-11**.
- Implementation status: The fBM and Volterra generators and their consumers exist. Checked generation, sampled-variance normalization and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent covariance, conditioning and grid examples reviewed. Runtime, host and pricing qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked fractional noise and Volterra drivers

1. **Distinguish the two stochastic laws and type Hurst.** Use
   the accepted `HurstExponent` with finite `0<H<1`. Standard
   unit-normalized fractional Brownian motion (fBM) has
   stationary increments. The zero-origin Riemann–Liouville
   (RL) driver has level variance `t^(2H)` in the continuum
   and generally nonstationary increments. A model declares
   its clock and its eligible Hurst range. Literal `H=1/2`
   is the Brownian case for both laws. Remove the generic
   generator contract that erases their different outputs.
   [Accepted fBM covariance](../core-numerics/fractional-covariance.md#accepted-decision-checked-fractional-increment-covariance).

2. **Retain a standalone checked exact-grid fBM sampler.**
   Use the accepted checked increment covariance on a nonempty
   strictly increasing finite nonnegative knot sequence, which
   may begin above zero. A singleton validates and gives no
   draws. Use the chronological lower Cholesky factor with
   strictly positive pivots and one independent normal per
   interval. Preserve time order, so an increment has no
   dependence on later input normals. At exact `H=1/2`, use
   independent `sqrt(h_j)*Z_j` with actual widths and no dense
   factor. Propagate covariance and factorization failures.
   Remove the windowed conditional sampler, its automatic
   window, the Schur-complement floor and automatic fallbacks.
   Keep the standalone sampler Rust-only. Its Cholesky normals
   define a grid-specific coupling if reused and carry no RL
   Brownian-driver identity.
   [Accepted matrix factors](../core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission),
   [fBM and hybrid references](../../../REFERENCES.md#fractional-noise-and-actual-grid-volterra-projection).

3. **Use one RL hybrid profile on the actual grid.** Target
   `Y(t)=sqrt(2H)*integral_0^t (t-s)^(H-1/2) dW(s)`.
   Prepare on the exact zero-origin grid `t_0,...,t_n` with
   actual widths `h_j=t_(j+1)-t_j`. At each receiving node,
   keep the exact newest-cell integral and project each older
   cell onto that cell's Brownian increment using the average
   kernel. Use all cells since origin. The formula covers
   irregular grids. A canonical uniform layout still uses
   actual widths and lags. The model owner qualifies grid
   eligibility and accuracy. `[0]` gives only the zero level
   and variance, with no draws. Prepare no nominal `T/n`
   replacement grid or hidden substeps.
   [Accepted time grids](../core-numerics/time-grids.md),
   [hybrid projection references](../../../REFERENCES.md#fractional-noise-and-actual-grid-volterra-projection).

4. **Keep the exact newest-cell covariance without subtraction.**
   For `p=H+1/2` and `gamma=H-1/2`, use the ordered
   independent normals `(Z_j,U_j)` and `Delta W_j=sqrt(h_j)*Z_j`.
   The normalized newest-cell integral is `a_j*Z_j+b_j*U_j`,
   with `a_j=h_j^H*sqrt(2H)/p` and
   `b_j=h_j^H*abs(gamma)/p`. These give variance `h_j^(2H)`
   and the kernel integral as covariance with `Delta W_j`.
   Compute the residual directly. Avoid subtracting nearly
   equal variances or recovering `2H` from `2*gamma+1`.
   At `H=1/2` generate Brownian levels directly from actual
   increments, preserving the unused residual slot.

5. **Use checked older-cell divided differences.** For node `k`
   and `j<k-1`, let `A=t_k-t_j`, `B=t_k-t_(j+1)`. The load on
   `Z_j` is `c_kj=sqrt(2H)*(A^p-B^p)/(p*sqrt(h_j))`.
   Use the known width and a stable power divided difference
   with `log1p` and `expm1` for close lags. Near a unit
   relative width preserve the actual positive `B`. Retain
   scale through the complete coefficient, including when
   the two rounded lags coincide. Validate the coefficient
   before use. Reject unresolved required values without
   replacing them with zero, jitter or another noise law.

6. **Publish levels and the matching node variances.** At node
   `k>0`, return `Y_k=a_(k-1)*Z_(k-1)+b_(k-1)*U_(k-1)
   +sum_(j<k-1)c_kj*Z_j` directly. Use deterministic checked
   scaled sums. Derive `v_k` by summing the squared loadings
   of that same row. In ideal arithmetic
   `v_k=h_(k-1)^(2H)+sum_(j<k-1)c_kj^2`. The row dot
   products give the sampled covariance, and the joint
   covariance with every past `Delta W_j` is the true
   kernel integral on that cell. Older-cell averaging can
   reduce variance and change level covariance. The driver
   supplies checked variance or its scaled contribution to
   compensation, preserving origin zero and the Brownian
   identity `v_k=t_k` in ideal arithmetic.

7. **Use the sampled variance for Gaussian normalization.**
   A caller that targets a unit-mean multiplier uses
   `exp(eta*Y_k-eta^2*v_k/2)` on the sampled grid. The
   identity holds under ideal Gaussian draws. Substituting
   the continuum `t_k^(2H)` when the variances differ
   changes the mean. Use the actual left-node level and
   variance for a left-point model step. They depend only
   on previous intervals. Expose the actual Brownian
   `Z_j` separately for that interval's leverage. A model
   owns its correlation coefficient and its base variance,
   volatility amplitude and payoff dynamics.

8. **Bind generation and stepping to one validated plan.**
   Bind each plan and its generated values to the complete
   stored knots, Hurst value and noise law. Use one checked
   Rust path interface for the named raw and pricing routes.
   Give the model the left-node level and matching variance,
   interval driving normal and independent model normal.
   Remove arbitrary injection indices, optional leverage
   buffers, dummy normals and the difference/reaccumulate
   cycle. Require exact shapes and finite inputs, reset
   state, work and payoff per leg, and check the actual
   receiving state and event time. A same-size different
   grid or Hurst value is incompatible. Propagate errors
   with stage, estimator, leg and node or cell context.
   Publish no partial output or successful estimate on failure.
   [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

9. **Pin the named model draw layout.** For each independent
   estimator's Philox substream, generate chronological
   triples `(Z_drive,Z_near,Z_independent)`, including
   reserved slots at exact limits. Keep any predeclared
   payoff uniforms after that normal block, in actual
   node order. Raw capture consumes no payoff draws.
   Negate every normal and complement eligible uniforms
   for an antithetic leg, using fresh state and work.
   Apply model leverage once, with no engine correlation.
   Keep raw and pricing path replay aligned for the same
   effective model, grid and normal layout. Different
   grids can assign draws to different intervals.
   [Accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

10. **Apply shared resource checks before setup and generation.**
    Use the existing `64,000,000` logical-cell ceiling for
    `n*n` dense fBM preparation and `n*(n+1)/2` shared-normal
    coefficients in the RL triangle. Account separately for
    residuals, covariance and factor storage, path/capture
    buffers and concurrency. Use checked work counts and byte
    layouts and fallible reservation. Preflight setup and
    aggregate generation counts across all legs before draws.
    Dense fBM costs cubic factor setup and quadratic sampling.
    The actual-grid RL plan costs quadratic setup and sampling,
    with a reusable triangular table. Literal half-Hurst
    paths use linear work/storage and ordinary buffer limits.
    The cell ceiling controls prepared size. Product owners
    qualify aggregate work for their declared grid and counts.
    Fail an inadmissible request without shortening its history.

11. **Reconcile the named model and host routes.** Use RL for
    existing rough Bergomi and rough Cheyette paths. Remove
    `FbmSpec`, the generic raw `fbm` selector and fBM
    alternatives from those models' existing wire and parity
    surfaces. Migrate the raw Rust/Python/WASM routes and
    rough Bergomi equity pricer together. Use the accepted
    explicit estimator/stream controls and checked statistics.
    Rough Heston keeps its own stochastic Volterra history
    and normalization. Keep shared low-level generators
    Rust-only and reconcile generated schema and declarations
    through the existing model hosts.
    [Rough Bergomi](14-rough-bergomi.md), [rough Heston](15-rough-heston.md),
    [rough Cheyette](16-rough-cheyette.md).

12. **Qualify the generator and financial model separately.**
    Verify exact fBM covariance, near-field and Brownian cross
    moments, sampled RL covariance and variance compensation,
    irregular and nominally uniform grids, exact half and
    adjacent Hurst cases, replay, resource and failure behavior.
    Compare RL to the continuum kernel under refinement.
    Finite normal support and rounded coefficients have separate
    limits. Qualify rough-model parameters, grid bias, prices,
    tail sampling and monitoring under records 14–16 and their
    product owners.

## Observed behavior and limitations

The current raw selector defaults to nominally uniform RL and
accepts exact or windowed stationary fBM as alternatives for
rough Bergomi and rough Cheyette. The window drops older
conditioning information and floors its Schur residual. The
RL generator reconstructs `T/n`, subtracts rounded near-field
variances and emits differences of levels. The current consumers
sum those differences and compensate with the continuum variance.
Their fBM alternatives reuse a Cholesky innovation for leverage.

The public injection helper accepts index and buffer combinations
without identifying the complete model grid or noise law. The raw
and priced paths have distinct stepping and stream assignment.

## Independent examples and qualification

| Example | Expected result or limitation |
| --- | --- |
| Stationary fBM at `H=.25`, unit steps on `[0,1,2,3]` | Lag-two increment covariance is about `-.04818816`. Window one gives `+.08578644` and terminal level variance `2` in place of `sqrt(3)`. |
| RL at `H=.25` on `[0,1,2]` | The newest-cell loads are about `.94280904` and `.33333333`. The older shared load at node 2 is `.64280045`. Sampled terminal variance is `1.41319241` versus continuum `sqrt(2)`. With `eta=3`, continuum compensation gives mean multiplier `.99541537` and sampled compensation gives ideal mean `1`. |
| RL at `H=.1` on `[0,.75,1]` | Sampled terminal variance is about `.9940908814`. The continuum variance is `1`. |
| RL at `H=nextDown(.5)`, width `1` | The correct newest-cell residual is about `5.551115123e-17`. The observed Schur subtraction gives about `1.49e-8`. At exactly half the residual is zero. |
| RL at `H=.25` on `[0,2^-60,1]` | The first cell's older load at terminal time is about `6.585445080e-10`. Rounded individual lags coincide, so a direct power difference loses the term. |
| Canonical `uniform(1,3)` grid | The last stored width is `.33333333333333337`. Use it for both the Brownian increment and newest kernel, even though the layout is recognized as uniform. |

## Evidence

Located tests cover covariance, exact near-field moments, Brownian
reduction, finite-memory comparison and loop replay. The non-Markov
tests exempt short-window Cheyette compensation. The current replay
fixture exercises the shared helper with manually split streams.
[fBM](../../../../finstack-quant/models/src/monte_carlo/rng/fbm.rs),
[RL kernel](../../../../finstack-quant/models/src/monte_carlo/rng/volterra.rs),
[selection and budgets](../../../../finstack-quant/models/src/monte_carlo/simulate/fractional.rs),
[injection](../../../../finstack-quant/models/src/monte_carlo/engine/fractional.rs),
[non-Markov tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests/non_markov.rs),
[equity pricing loop](../../../../finstack-quant/valuations/equity/src/equity_option/rough_bergomi_mc_pricer.rs),
[Python parity](../../../../finstack-quant-py/tests/test_models_wasm_parity.py),
[WASM parity](../../../../finstack-quant-wasm/tests/facade/models_parity.test.mjs).

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
