# MODELS-MONTE-CARLO-25: Paired common-random-number delta and gamma

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Existing paired Rust estimator and GBM European Rust/Python/WASM entry points observed. Checked stencils, coupling, uncertainty and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent stencil, pairing and GBM price examples reviewed. Runtime, host and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked paired Monte Carlo spot Greeks

1. **Define a frozen-input one-dimensional spot sensitivity.** The
   generic Rust route bumps only the admitted scalar initial spot of
   a compatible one-dimensional process. Hold the model coefficients,
   scheme, exact grid, payoff terms, observed history, currency and
   deterministic discount fixed. The initial spot is finite and
   strictly positive. Delta is PV currency per unit of spot and
   gamma is PV currency per squared unit of spot. These are signed
   scalar sensitivities to a specified finite stencil.

2. **Require an explicit relative bump and actual spot nodes.** For
   a finite decimal relative fraction `0 < b < 1`, use the binary64
   product `h = S0*b` and the represented nodes `Sd = S0-h` and
   `Su = S0+h`. Require finite ordered `0 < Sd < S0 < Su`.
   Reject collapsed, crossed and out-of-range stencils before
   simulation. Remove the absolute bump and minimum-spot floors.
   Keep the actual nodes and widths. No clamp, bump widening or
   one-sided fallback.

3. **Use two valuations for delta and three for gamma.** With
   `a = Su-S0`, `c = S0-Sd`, and `w = Su-Sd`, use
   `delta = (Vu-Vd)/w` and
   `gamma = 2*((Vu-V0)/a - (V0-Vd)/c)/w`.
   Delta is the endpoint secant, whose interval midpoint can differ
   from `S0` when the represented steps are unequal. Gamma is the
   curvature of the quadratic interpolant through the three actual
   nodes. Equal steps recover the usual symmetric formulas.
   [Stencil references](../../../REFERENCES.md#paired-monte-carlo-spot-sensitivities).

4. **Reuse full fixed-budget engine admission.** Validate the
   positive-horizon grid, the exact prepared scheme identity, payoff
   events, process/observable mappings and every spot scenario
   before draws. Check positive finite scalar discount and the
   actual work and storage products, including stencil points,
   legs, grid nodes and draw slots. Require an explicit seed,
   count from 2 through 10,000,000 base groups, antithetic selection
   and execution mode. Capture, adaptive stopping and QMC are
   outside the paired estimator. No partial result.
   [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [accepted Sobol boundary](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).

5. **Share the same logical Philox draws at each spot.** Assign one
   absolute base-group ID per independent observation. Replay each
   scenario using the same effective key, group ID and normal
   slots on the same stored nodes, and the same payoff-uniform
   slots including the origin if required. Fix this layout before
   draws and reject a scheme or payoff that changes slot consumption
   with simulated state. Reset path, payoff and workspace for each
   scenario and leg. In paired mode use matching primary and
   reflected shocks, including the corresponding `u,1-u` uniforms,
   across all scenarios. Matching seed alone cannot establish
   common draw assignment.
   [Accepted Philox profile](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation),
   [CRN references](../../../REFERENCES.md#paired-monte-carlo-spot-sensitivities).

6. **Discount and difference complete legs before statistics.** Form
   each scenario's same-currency checked Money PV with one scalar
   discount. Keep its exact stored decimal value through the payoff
   differences, including differences lost by first rounding both
   PVs to binary64. Use checked widened differences and scale-aware
   stencil arithmetic. A difference need not be stored as Money.
   Carry scale through the complete quotient and, when paired, the
   equal-weight combination of the two leg stencils. Avoid requiring
   a raw squared bump, a doubled base PV or a representable intermediate
   slope. Require a finite resolved binary64 group observation and
   preserve literal zero. The stencil inherits rounding already
   incurred in the modeled or discounted cash. Errors identify the
   actual group, scenario, leg and node as applicable.
   [Accepted Money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

7. **Accumulate one observation per complete group.** Use the
   stencil of one leg when unpaired and the equal-weight mean
   of both leg stencils when paired. Preserve covariance between
   all spot scenarios and within an antithetic pair. Reuse the
   accepted ordered chunk reduction in serial and native parallel
   execution. Reduce each group once without retaining all scenario
   payoff arrays. Report the lowest failing group in canonical
   scenario, leg and node order and discard the whole run on
   failure. Reject unsupported WASM parallel requests.

8. **Report the paired fixed-sample Student-t interval.** For `N`
   complete groups use their mean, sample SD with divisor `N-1`,
   standard error `s/sqrt(N)` and two-sided 95% Student-t interval
   with `N-1` degrees of freedom. Apply the accepted checked
   uncertainty rules, including point intervals for constant
   realized samples and failures for unresolved positive widths.
   The interval describes sampling conditional on the finite
   stencil and fixed pricing inputs. It excludes finite-bump,
   model, numerical and grid errors. Exact 95% coverage requires
   the accepted IID normal conditions. Rare strike crossings can
   yield a zero-gamma sample despite positive population gamma.
   CRN and antithetics provide no unconditional variance reduction.
   [Accepted inference](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

9. **Report work and replay inputs with the Greek.** Report
   the actual spot nodes, widths, units, currency, grid, seed,
   pairing and execution provenance. Distinguish `N` independent
   observations, `L*N` physical trajectories per scenario, and
   `2*L*N` delta or `3*L*N` gamma trajectory valuations in total,
   where `L` is one or two. Check these relations on result
   restoration. Preserve the generic single-price work meanings.

10. **Use one exact step for the standalone GBM European route.**
    Price a unit cash-settled call or put at positive expiry
    `T` on the exact grid `[0,T]`. Require positive finite spot and
    strike, finite signed continuous rate and yield, finite
    nonnegative volatility and explicit currency. Use net carry
    `r-q` and one checked discount `exp(-r*T)`. The terminal-only
    law requires no time subdivision. Admit literal zero volatility
    through the same fixed group and draw layout. Its deterministic
    finite stencil uses the actual cash rule, including a strike kink,
    and yields a point interval. Keep configurable exact grids with
    qualified generic Rust callers.
    [Accepted GBM](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
    [accepted terminal payoff](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

11. **Reconcile the existing host and product boundaries.** Keep the
    existing Rust/Python/WASM `finite_diff_delta` and `finite_diff_gamma`
    entry points with explicit bump, market, payoff, currency and run
    controls. Remove their registry defaults and optional terminal
    step count. Transport exact seeds, canonical checked results and
    error kinds through the existing wrappers, schema, generated
    types, facade, stubs and parity. Keep generic callbacks Rust-only.
    Product risk owners define any linked input rebuild, smile or
    calibration response, lifecycle and market shock. An independently
    repriced product using the same seed alone has no paired standard
    error under this contract. Likelihood-ratio Greeks have their own
    scope in record 26.
    [Product risk](../valuations-framework/option-sensitivities.md),
    [selected equity risk](../valuations-equity-and-composition/08-selected-risk-and-implied-vol.md),
    [likelihood-ratio Greeks](26-likelihood-ratio-greeks.md).

12. **Qualify the finite target and derivative accuracy separately.**
    Check collapsed and unequal spots, signed and nearly cancelling
    PVs, degenerate variance, complete groups, actual work, invalid
    layouts, native order and host transport. Compare independent
    closed-form prices at the actual GBM stencil nodes and use
    independent integration for paired sampling variance. Qualify
    sensitivity to bump and sample size at smooth points and near
    kinks before using the result as a local derivative. No adaptive
    bump selection, Richardson extrapolation or implicit switch to
    another estimator.
    [Sensitivity references](../../../REFERENCES.md#paired-monte-carlo-spot-sensitivities).

## Observed behavior and limitations

The [paired estimator](../../../../finstack-quant/models/src/monte_carlo/greeks/finite_diff.rs)
uses `max(S0*b,1e-8)`, a `1e-12` minimum lower spot and the
nominal denominators. It clones the substream for each scenario,
checks scheme pairing but omits the prepared-scheme step-count
check, retains all scenario payoff arrays and executes serially
even if parallel is requested. The [engine loop](../../../../finstack-quant/models/src/monte_carlo/engine/simulation.rs)
can query payoff uniform demand during the path.

The [GBM convenience](../../../../finstack-quant/models/src/monte_carlo/greeks/gbm_european.rs)
uses a normal multiplier of 1.96 and reports path legs per
scenario. Its omitted controls draw on the registry, including
the relative bump of 1%, 10,000 base groups, 50 steps, seed 42
and antithetic pairing. It requires positive volatility. The
[Python](../../../../finstack-quant-py/src/bindings/models/monte_carlo/greeks.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs)
routes call this Rust convenience.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| `S0=2^52`, relative bump `5*2^-55`, linear PV `V(S)=S` | Requested width is `0.625`; actual up/down widths are `1,0.5`. Actual delta is `1` and gamma is `0`. Nominal denominators give `1.2,1.28`. |
| PVs `1000000000000000000.01` and `1000000000000000000` at delta endpoints two spot units apart | Exact stored PV difference is `0.01`, giving delta `0.005`. Both independent binary64 PV views are `1e18`. |
| Gamma nodes near `(1,2,3)*1e-163` and PVs `(0,0,1e-28)` | Gamma is approximately `1e298` even though the standalone squared bump underflows in binary64. |
| Zero-volatility call, `S0=K=100`, zero carry and discount rate, spot widths `1` and `0.5` in separate runs | Each symmetric run gives delta `0.5`; gamma is `1` at width `1` and `2` at width `0.5`. Every group within a run is constant. |
| Three antithetic groups with leg stencil values `(0,2),(2,2),(4,2)` | Observations are `1,2,3`. Mean is `2`, SE is about `0.57735027`, and the Student-t half-width is about `2.48413771`. Work is six paths per scenario, twelve for delta or eighteen for gamma. |
| GBM call `S0=K=100, r=0.05, q=0.02, sigma=0.2, T=1, h=1` | Independent closed-form pricing and conditional integration give finite delta `0.586780115041` and gamma `0.018947785514`. Analytic local derivatives are `0.586851146135` and `0.018950578755`. Antithetic group SDs are about `0.08846568` and `0.07516269`. |
| Same GBM call and width with 20 unpaired groups | In the ideal law, an all-zero gamma sample has probability about `44.36%` even though population stencil gamma is positive. |

Selected tests cover deterministic payoffs, invalid stencils,
antithetics and analytic comparisons. The independent examples
check geometry, finite-target price and uncertainty semantics.
Runtime, host and financial qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
