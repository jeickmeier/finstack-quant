# MODELS-MONTE-CARLO-01: Engine execution, discounting and independent estimators

- Decision status: **Accepted — complete engine and estimation package, 2026-11-11**.
- Implementation status: Existing runtime observed. The accepted contract still requires implementation and caller reconciliation.
- Validation status: Source, selected tests, primary references and independent arithmetic examples reviewed. Runtime, host parity and production pricing qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Monte Carlo execution and estimation

1. **Scope the primary engine to fixed-budget pseudorandom pricing.** Require
   explicit compatible process, scheme, payoff, initial state, positive-horizon
   grid, currency, deterministic scalar present-value factor, splittable random
   stream with explicit seed/provenance, independent estimator count and
   antithetic selection. The count is an integer from 2 through 10,000,000
   inclusive. Check the positive state dimension, innovation count, shapes,
   mode compatibility and required work/allocation sizes before draws. The
   caller owns the pricing measure and the representation of the final payoff
   as one equivalent settlement amount. Its settlement reference can differ
   from the last grid knot. A stream of differently dated flows needs an
   owner-qualified equivalence to this amount. Stochastic discounting,
   stochastic numeraires, QMC, injected fractional noise and zero-horizon
   pricing use their separately qualified owner routes.

2. **Apply a positive discount once per trajectory.** Require finite
   `D > 0`, accept an explicitly selected `D = 1` and allow `D > 1`.
   Reject every final payoff leg whose Money currency differs from the run
   currency, including a zero amount. Preserve signed amounts and literal zero.
   Convert a binary64 factor once through the checked Decimal adapter,
   multiply each leg under the accepted Money rules, and use the checked
   binary64 view of the discounted amount for moments. A factor or leg
   product outside that contract fails even if subsequent pairing or
   accumulation would yield a representable mean. Convert the completed mean
   and interval endpoints back through the same Money boundary. Fail if the
   conversion loses a required strict interval or cannot represent the
   result. No implicit FX or currency-minor-unit rounding. [Money precision and scalar conversion](../core-conventions/012-money-decimal-precision.md).

3. **Use exact stored knots and checked node-local state.** Bind any
   grid-prepared scheme to the full admitted knot sequence and all model
   dependencies used in its preparation. Validate the payoff's entire
   required event binding against the grid before simulation. Dated events
   must match their exact assigned knots and an index-native event must be
   in bounds. The payoff owner controls order and multiplicity within a
   node, as well as historical fixings and event eligibility. Reset each
   leg's path state, payoff and workspace. Invoke its event callback once
   at the origin and once at every receiving node, using stored
   `t[i+1]` and actual adjacent `dt[i]`. The scheme and payoff can own
   path history explicitly. Require an explicit projection of any
   named observables, with absence distinct from zero and no reuse of a
   prior node's values. Remove the generic first-coordinate spot and
   second-coordinate variance inference. Check the initial and all
   evolved raw coordinates, requested projected observables and consumed
   draws for shape, presence and finiteness. Propagate callback failures
   with node/stage context. Model-specific positivity and boundary
   behavior belong to the admitted process/scheme. [Canonical time grid](../core-numerics/time-grids.md).

4. **Apply each correlation law once.** Declare independent innovations,
   an engine-owned correlation, or a compatible scheme-owned transform.
   For the engine-owned matrix, require exact shape and strict correlation
   admission with a caller-selected finite `0 <= pivot_tolerance < 1`,
   then apply the checked core factor in original factor and innovation
   order. Propagate factor/application errors. A qualified scheme-owned
   transform receives untransformed innovations and validates its own
   correlation inputs. Reject conflicting ownership and unused supplied
   correlations. No implicit jitter or repair. A nonzero pivot tolerance
   can truncate directions under the accepted core contract, and the
   caller owns the resulting covariance error budget. [Matrix and correlation admission](../core-numerics/matrix-factorization.md).

5. **Assign a deterministic stream to each estimator.** In both serial
   and native parallel pricing, split the declared root stream by the
   zero-based independent estimator index. Reject failed splitting,
   nonsplittable streams and quasi-random streams. Remove the serial
   nonsplit branch. Fix the normal draw layout and whether one independent
   payoff uniform is needed at each node, including the origin, before
   running. Each antithetic pair shares this layout and receives the same
   innovations and their negatives, with payoff uniforms paired as
   `u, 1-u`. Require both uniforms strictly inside `(0,1)` after
   rounding, with no clamping or redraw on failure. Keep the generator,
   substream and uniform construction profile in [record 02](02-random-streams.md).

6. **Count and reduce independent estimator units.** Ordinary pricing
   contributes one discounted payoff per unit. Antithetic pricing
   contributes the equal-weight mean of two complete discounted legs.
   Use a checked, range-safe midpoint and reject an unrepresentable
   nonzero result. No dropping failed legs or treating the two legs as
   independent. Report the independent estimator count `N` and the
   raw trajectory count, exactly `N` or `2N` as selected. Use explicit
   names for both quantities in the pricing contract. Antithetic
   coupling provides no unconditional variance-reduction guarantee.
   Preserve the execution-only default chunk size
   `(N / 64).clamp(100, 10000)` with integer division, or accept a
   positive explicit chunk size. Fold estimator indices within each
   chunk in order and merge chunks in ascending order across serial
   and native Rayon execution. For path failures, return the lowest
   failing estimator index, with first leg before reflected leg and
   failure context at the actual operation/node. Failed runs publish
   no partial estimate or captured-path result. [Reproducibility and deterministic Monte Carlo execution](../../standards/0043-deterministic-ordering-and-monte-carlo-execution.md).

7. **Retain an explicitly defined fixed-sample 95% interval.** For
   independent estimator observations `X[i]`, use the checked core
   mean and sample standard deviation `s` with divisor `N-1`.
   Compute `SE = s/sqrt(N)` and
   `h = -t_inv(0.025, N-1) * SE` directly, using the core's explicit
   `alpha = 0.05`. Publish `ci_95 = [mean-h, mean+h]` and preserve
   the computed half-width for any consumer that needs it. A constant
   realized sample yields zero standard error and a point interval.
   A positive uncertainty that underflows or loses a strict endpoint
   is an error. Define relative standard error as `SE / abs(mean)`
   with a checked range at nonzero mean, and unavailable at exact zero.
   Remove the near-zero threshold and infinity sentinel. Exact 95%
   coverage applies to a fixed count of IID, nondegenerate normal
   estimator units. For general IID finite-variance payoffs the
   interval is a large-sample approximation. It describes sampling
   uncertainty conditional on the chosen model, grid and payoff
   representation. Rare-payoff samples can yield a point interval
   while population uncertainty remains positive. [Checked sample statistics](../core-numerics/sample-statistics.md), [estimation references](../../../REFERENCES.md#fixed-budget-monte-carlo-pricing-and-estimation).

8. **Remove generic width-driven stopping.** Execute the entire declared
   budget or fail. Remove the 5,000-estimator warm-up and generic
   `target_ci_half_width` stopping option. Reconcile the piecewise-GBM
   autocallable and cliquet consumers that currently forward the option.
   Where those owners request precision admission, require a positive
   finite tolerance in PV currency units and apply `h <= tolerance`
   after the fixed run, returning a precision failure on a miss.
   A successful gate gives no conditional 95% coverage guarantee for
   estimates selected by width. The callable-bond LSMC post-run gate
   retains its separate owner. Any future sequential inference needs
   its own estimator, coverage decision and evidence. [Callable-bond LSMC policy](../valuations-bonds/07-lsmc-policy.md).

9. **Require explicit controls and truthful results at existing entry points.**
   Remove inherited count, seed, grid/step-count, pairing and USD defaults
   for the covered generic and pricing convenience routes. Keep typed
   GBM and Heston operations as explicit model selections, with their
   algorithms owned by their model records. Resolve and validate the
   actual grid, pricing currency and factor, including when an existing
   product operation derives them from explicit market/contract inputs.
   Select execution mode explicitly and report the mode used, effective
   chunk size, pairing, counts, grid and replay inputs/provenance. An
   unsupported parallel request errors on current serial WASM. Same-target
   native serial/Rayon identity requires the same admitted profile, grid,
   inputs and chunk size under the accepted execution conditions.
   Cross-target comparisons use qualified tolerances. Restore existing
   serialized pricing inputs/results through checked state, including
   count relations, currency, finite/ordered uncertainty and required
   execution metadata. Preserve exact seed values at supported host
   boundaries. Reconcile existing Rust, Python and WASM pricing surfaces,
   metrics, docs, schemas and parity. Generic process, scheme, RNG and
   payoff callbacks stay Rust-only. [Reproducibility guarantees](../../standards/0021-reproducibility-guarantees.md).

10. **Keep diagnostics and specialized inference with their owners.**
    Require capture to be explicitly requested through a capture-capable
    route and reject capture together with antithetics. Diagnostic
    recording must leave pricing draws, event order, terminal payoff
    evaluation and estimator membership identical. Emitted diagnostic
    cashflows must have the same lifetime regardless of capture and
    cannot feed back into payoff evaluation through the capture sink.
    Snapshot APIs, diagnostic limits, subset statistics and public raw-path
    simulation belong to record 28. Apply the independent estimator and
    count rules above to this pricing engine. Randomized QMC, LSMC,
    Greeks and control variates own their estimator, dependency and
    uncertainty semantics. Reconcile shared internals with each actual
    caller before changing shared results or interfaces. [Record 03](03-randomized-qmc.md),
    [record 23](23-lsmc-exercise.md), [record 25](25-finite-difference-greeks.md),
    [record 26](26-likelihood-ratio-greeks.md), [record 27](27-control-variate.md),
    [record 28](28-capture-and-simulation-contracts.md).

## Observed choice and domain

The generic engine composes a caller-selected process, scheme and payoff, evaluates undiscounted payoff units, and applies one scalar discount factor. The caller supplies the pricing time in years, model rates and volatilities in decimal units, and a currency tag. The European GBM convenience selects exact GBM, and the Heston convenience selects QE.

## Observed algorithm and controls

1. Validate 2–10,000,000 independent estimators, a nonempty grid, state dimension, nonnegative finite discount factor, positive chunk size, payoff maximum event index, model-specific grid, and dedicated scheme identity. Generic pricing rejects injected-noise processes and quasi-random streams.
2. Prepare the scheme and Cholesky-factor process correlation unless the scheme declares internal correlation. Reset payoff, state and workspace per path; populate state and call the payoff at time zero, then after every step. Uniform draws are supplied only when requested by the payoff.
3. Use `rng.split(path_id)` for splittable streams. Serial nonsplittable streams advance one cloned stream. Parallel execution requires splitting. Serial/parallel reductions use the same ordered chunks; default chunk size is `(num_paths/64).clamp(100,10000)`.
4. Antithetic mode runs `(z,-z)` and `(u,1-u)`, records the pair mean as one independent estimator, and reports twice as many simulated paths. Price paths are discounted before online moment updates. Nonfinite final discounted payoffs error.
5. `Estimate::from_stats` delegates sample mean, standard deviation, standard error and 95% Student-t interval to core `OnlineStats`. Optional serial stopping checks CI half-width after 5,000 samples and stops at the first strict improvement below target. It is an optional-stopping estimator, not fixed-sample coverage.

The [registry asset](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json) fixes Rust engine parallel=true/antithetic=false; European and path-dependent counts=100,000, seed=42; convenience parallel=false, 252 steps; LSMC antithetic=true. Currency defaults to USD only in convenience resolution. Registry schema validation checks positive counts; the engine imposes stronger minimum/maximum limits later.

## Observed failure behavior and alternatives

The generic engine checks final payoff finiteness. `simulate_paths` also checks raw state at each step. Process `drift`/`diffusion` methods and public parameter structs vary in admission strength. Default state mapping labels coordinate zero as spot and coordinate one as variance. The event loop reconstructs the receiving time as `t+dt`. Payoff amounts can be extracted before validating leg currency. Capture and antithetics are rejected together, as are parallel execution and auto-stop. Captured payoff snapshots call `value()` before maturity, and capture drains diagnostic cashflows as it runs. Current WASM uses the serial loop while captured-run metadata can report the requested parallel flag.

## Independent examples and qualification

| Example | Expected behavior under the accepted contract |
| --- | --- |
| USD leg pairs `(0,2),(2,4),(4,6)` and `D=0.5` | Observations `0.5,1.5,2.5`, three estimators and six trajectories. Mean `1.5`, sample SD `1`, SE `1/sqrt(3)`, df `2`, and 95% half-width `sqrt(722/117) ≈ 2.48413771175`. |
| All USD payoffs `-10` and `D=1.25` | Mean and both endpoints are USD `-12.5`. All requested trajectories execute. Reject a EUR zero leg in a USD run, or an unrepresentable Decimal factor such as binary64 `1e-29`. |
| Exact knots `[0, 2^-53, 1+2^-52]` | Final event gets the stored `1+2^-52`. Adding the second rounded step to its left knot instead gives `1`. A prepared grid with the same length and a different interior knot fails binding. |
| Correlation `rho=0.6`, raw innovations `(2,-1)` | The original-order factor gives `(2,0.4)` to rounding precision. Applying it a second time would give `(2,1.52)`. A custom payoff uniform so small that `1-u` rounds to `1` fails antithetic draw admission. |
| Existing width stop, IID Bernoulli payoff with `p=1e-5`, cap 5,001, tolerance `1e-5` | All-zero first 5,000 samples stop with probability `(1-p)^5000 ≈ 0.95122919` and yield `[0,0]`. The expected stopped mean is approximately `1.90208e-9` below `p`; the fixed-count sample mean has expectation `p`. The rare-payoff case also illustrates the fixed-count interval's coverage limitation. |

Independently qualify discount/currency and Decimal limits, pair arithmetic
and counts, exact binding and node coverage, correlation ownership and
truncation, draw order, lowest-index error selection, capture transparency,
fixed-budget uncertainty and width admission. Compare native serial and
parallel at fixed partitions and validate actual WASM mode and error/result
parity. Use model-owner benchmarks and convergence budgets for prices and
risk. The arithmetic examples exercise contract distinctions. They provide
no runtime, stream-quality or production price certification.

## Evidence

The source shows the pricing loop, callback boundaries, result conversion and
the live valuation forwarding of the width stop. Engine tests include
`test_serial_vs_parallel_bit_identical_for_varying_payoff`, thread-pool
determinism, antithetic counts, incompatible configurations and the
5,000-sample stop. These tests document observed implementation expectations.
[Engine](../../../../finstack-quant/models/src/monte_carlo/engine/pricing.rs),
[path event loop](../../../../finstack-quant/models/src/monte_carlo/engine/simulation.rs),
[traits](../../../../finstack-quant/models/src/monte_carlo/traits.rs),
[estimates](../../../../finstack-quant/models/src/monte_carlo/estimate.rs),
[results](../../../../finstack-quant/models/src/monte_carlo/results.rs),
[European](../../../../finstack-quant/models/src/monte_carlo/pricer/european.rs),
[Heston](../../../../finstack-quant/models/src/monte_carlo/pricer/heston.rs),
[piecewise GBM](../../../../finstack-quant/valuations/equity/src/piecewise_gbm.rs),
[engine tests](../../../../finstack-quant/models/src/monte_carlo/engine/tests.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
