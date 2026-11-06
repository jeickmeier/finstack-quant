# MODELS-MONTE-CARLO-01: Engine execution, discounting and independent estimators

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in engine/, traits.rs, estimate.rs, registry.rs and convenience.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

The generic engine composes a caller-selected process, scheme and payoff, evaluates undiscounted payoff units, and applies one scalar discount factor. This is not a stochastic-numeraire pricing engine. Year fractions, decimal rates/volatilities, and the currency tag come from callers. Model selection is explicit; convenience European pricing fixes exact GBM, while the Heston convenience fixes QE. Adoption of these defaults is pending.

## Algorithm and controls

1. Validate 2–10,000,000 independent estimators, a nonempty grid, state dimension, nonnegative finite discount factor, positive chunk size, payoff maximum event index, model-specific grid, and dedicated scheme identity. Generic pricing rejects injected-noise processes and quasi-random streams.
2. Prepare the scheme and Cholesky-factor process correlation unless the scheme declares internal correlation. Reset payoff, state and workspace per path; populate state and call the payoff at time zero, then after every step. Uniform draws are supplied only when requested by the payoff.
3. Use `rng.split(path_id)` for splittable streams. Serial nonsplittable streams advance one cloned stream. Parallel execution requires splitting. Serial/parallel reductions use the same ordered chunks; default chunk size is `(num_paths/64).clamp(100,10000)`.
4. Antithetic mode runs `(z,-z)` and `(u,1-u)`, records the pair mean as one independent estimator, and reports twice as many simulated paths. Price paths are discounted before online moment updates. Nonfinite final discounted payoffs error.
5. `Estimate::from_stats` delegates sample mean, standard deviation, standard error and 95% Student-t interval to core `OnlineStats`. Optional serial stopping checks CI half-width after 5,000 samples and stops at the first strict improvement below target. It is an optional-stopping estimator, not fixed-sample coverage.

The [registry asset](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json) fixes Rust engine parallel=true/antithetic=false; European and path-dependent counts=100,000, seed=42; convenience parallel=false, 252 steps; LSMC antithetic=true. Currency defaults to USD only in convenience resolution. Registry schema validation checks positive counts; the engine imposes stronger minimum/maximum limits later.

## Failure behavior and alternatives

The generic engine checks payoff finiteness, not every state component at every step; `simulate_paths` has a stronger state check. Process `drift`/`diffusion` methods and public parameter structs are not universally self-validating. Default state mapping labels coordinate zero as spot and coordinate one as variance, so custom processes must override it. Capture and antithetics are rejected together; parallel auto-stop is rejected. Captured payoff snapshots require `value()` to be valid before maturity. Pathwise discounting, sequential confidence sequences and randomized-QMC uncertainty are distinct alternatives; the latter is implemented in [record 03](03-randomized-qmc.md).

## Evidence

[Engine](../../../../finstack-quant/models/src/monte_carlo/engine/pricing.rs), [path event loop](../../../../finstack-quant/models/src/monte_carlo/engine/simulation.rs), [traits](../../../../finstack-quant/models/src/monte_carlo/traits.rs), [estimates](../../../../finstack-quant/models/src/monte_carlo/estimate.rs), [European](../../../../finstack-quant/models/src/monte_carlo/pricer/european.rs), [Heston entry](../../../../finstack-quant/models/src/monte_carlo/pricer/heston.rs). [Engine tests](../../../../finstack-quant/models/src/monte_carlo/engine/tests.rs) include `test_serial_vs_parallel_bit_identical_for_varying_payoff`, thread-pool determinism, antithetic counts, incompatible configuration rejection and the 5,000-sample stop. These establish implementation contracts, not external price accuracy. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) and [Welford](../../../REFERENCES.md#welford-1962) supply canonical background; no adoption approval is inferred.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
