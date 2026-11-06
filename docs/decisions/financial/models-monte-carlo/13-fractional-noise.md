# MODELS-MONTE-CARLO-13: Fractional Brownian and Riemann–Liouville noise generation

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in rng/fbm.rs, rng/volterra.rs, simulate/fractional.rs and engine/fractional.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Three explicit noise choices exist: exact-grid fractional Brownian increment covariance via Cholesky, windowed conditional fBM approximation, and a uniform-grid Riemann–Liouville (RL) Volterra hybrid driver. They are different stochastic covariance constructions, not interchangeable implementations of one model. `FbmSpec` defaults to RL Volterra; it applies only to rough Bergomi and rough Cheyette. Hurst validity is delegated to `HurstExponent`.

## Algorithm and controls

Cholesky forms the full increment covariance from the fBM kernel, factors it, and multiplies L by independent normals. It accepts irregular increasing nonnegative grids; failure of positive-definite factorization errors. Windowed fBM exactly generates the first b increments, then uses `w=C_prev^-1 cov`, conditional mean w·previous b increments and variance `var_i-cov·w`. Default b=floor(sqrt(n)) clamped 10–50, capped by n. Negative conditional variance is floored at zero, with a warning if less than −1e-9*max(|var_i|,1).

RL uses `Y(t)=sqrt(2H) integral_0^t (t-s)^(H-1/2)dW(s)`. Two normals per step jointly generate the Brownian increment and exact newest-cell kernel integral using their 2×2 covariance. Older cells use interval-average kernel weights `(m^(H+1/2)-(m-1)^(H+1/2))/(H+1/2)*dt^(H-1/2)`. Sum the near integral and all older weighted Brownian increments, then difference consecutive levels. This is O(n²) work per path; the far-field approximation is not exact RL covariance. The true driving Brownian normal is preserved separately for leverage correlation.

The simulation adapter bounds Cholesky n² or window n*b storage by 64 million values, requires uniform RL grids, injects fractional increments at factor 1 and driving normals at factor 2, and mirrors both for antithetics. Low-level fractional path stepping validates buffer sizes, pairing and expected step count, then resets history per path.

## Failure behavior and alternatives

Windowing truncates long-memory dependence; its compensated variance need not equal t^(2H). Switching RL to stationary-increment fBM changes covariance even when marginal level variances match. For fBM alternatives the adapter uses the input innovation as a leverage driver, not an RL Brownian driver. Full covariance, FFT/hybrid acceleration and Markovian lifts are meaningful alternatives; only the named generators exist here. Low-level injected-noise helpers trust more caller state than `simulate_paths`.

## Evidence

[fBM](../../../../finstack-quant/models/src/monte_carlo/rng/fbm.rs), [RL kernel](../../../../finstack-quant/models/src/monte_carlo/rng/volterra.rs), [selection/budgets](../../../../finstack-quant/models/src/monte_carlo/simulate/fractional.rs), [injection](../../../../finstack-quant/models/src/monte_carlo/engine/fractional.rs). Tests locate covariance checks, exact near-field moments, H=1/2 Brownian reduction, finite-memory comparison, and deterministic generation. [Non-Markov tests](../../../../finstack-quant/models/src/monte_carlo/simulate/non_markov_tests.rs) explicitly exempt short-window Cheyette compensation. [Mandelbrot–Van Ness](../../../REFERENCES.md#mandelbrot-van-ness-1968), [Bennedsen–Lunde–Pakkanen](../../../REFERENCES.md#bennedsen-lunde-pakkanen-2017) and [McCrickerd–Pakkanen](../../../REFERENCES.md#mccrickerd-pakkanen-2018) provide distinct reference families.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
