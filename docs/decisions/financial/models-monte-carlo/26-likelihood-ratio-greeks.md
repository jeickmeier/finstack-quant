# MODELS-MONTE-CARLO-26: GBM likelihood-ratio delta and vega density scores

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Existing Rust score helpers and Rust/Python/WASM Asian convenience observed. Streamed scores, checked inference and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent score, pairing and pricing examples reviewed. Runtime, host and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked GBM likelihood-ratio Greeks

1. **Differentiate the checked single-asset GBM law.** Require
   finite positive initial spot `S0`, finite strictly positive
   constant relative volatility `sigma`, positive actual model
   intervals and the accepted exact GBM transition. Use explicit
   flat or admitted bounded deterministic cumulative carry, held
   fixed in both partials. Admit a nonuniform exact grid. Derive
   all score parameters from the same checked model and grid
   used by the path. Require explicit currency and a checked
   positive deterministic scalar discount for the equivalent
   payoff settlement. Zero volatility and zero horizon do not
   admit this density score. Retain deterministic pricing with
   its owning route.
   [Accepted GBM](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
   [accepted grids](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots).

2. **Admit only payoff dependence through the sampled density.**
   At fixed positive-time spot observations and any admitted
   auxiliary variates whose law is independent of spot and
   volatility, the payoff and settlement representation must
   stay fixed under both differentiated inputs. Freeze
   contractual strike, quantity, references, fixing multipliers,
   certified history and payment factor. Qualify this property
   at the Rust payoff owner. Reject a live origin spot fixing
   or monitoring observation, a recalculated strike or reference,
   a volatility-dependent bridge, continuous extrema corrections,
   direct use of the model innovations and unsupported exercise
   or calibration dependencies.
   An origin callback that does not consume a differentiated
   input is allowed. Frozen historical observations remain
   fixed even when their numerical value equals the current spot.
   Admit qualified positive-time terminal and discrete Asian,
   barrier and lookback payoffs with their accepted event rules.
   Discontinuities on positive-time spots, including strict
   digital exercise, are eligible when the density identity and
   the required moments apply.
   [Accepted payoff contract](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

3. **Score the actual consumed innovations.** Let `dt_i` be
   the actual stored interval and `z_i` the standard normal
   consumed by that leg's exact transition. Accumulate on
   the same path and in interval order. Use
   `Q_delta = z_1/(S0*sigma*sqrt(dt_1))` and
   `Q_vega = sum_i[(z_i^2-1)/sigma - sqrt(dt_i)*z_i]`.
   The first transition is the only transition whose
   conditional density depends on `S0` with later states
   held fixed. Vega includes both the log-drift and
   variance dependence on `sigma`. Keep the signed linear
   term. Use the actual `dt_i` even for a canonical
   uniform grid. Remove reconstruction from rounded spots
   and the public helpers that accept unbound payoff and
   score arrays.
   [Density score references](../../../REFERENCES.md#gbm-likelihood-ratio-delta-and-vega).

4. **Use a one-step marginal only for a sole positive-time
   observation.** A terminal-only Rust caller can use
   `[0,t_obs]` with the same exact transition and score law
   when its payoff depends on the spot at that observation
   and frozen inputs. `t_obs` is the observation horizon.
   Its settlement discount may refer to a later payment.
   A path-dependent payoff uses the joint scores on its
   actual grid. Never replace them with the last transition
   or a normalized terminal Brownian sum. The Asian host
   method uses the joint path law.

5. **Form all three contributions from one checked leg.**
   Settle the payoff once on complete event coverage.
   Check its currency and apply the scalar discount once
   under the accepted Money rules. For discounted amount
   `P`, contribute `(P, P*Q_delta, P*Q_vega)`. Preserve
   the stored PV and use checked scaled arithmetic for
   `z_i^2-1`, signed terms, ordered accumulation and complete
   score-weighted outputs. An isolated score, reciprocal,
   denominator product or intermediate sum need not fit
   binary64 if the required output is resolved. Require a
   finite representable completed contribution, preserve
   literal zero and fail on unresolved nonzero range.
   Greek observations and summaries are numeric quantities
   in their stated units. The price retains Money.
   [Accepted Money](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
   [accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

6. **Use one streamed, fixed-budget Philox run.** Require
   explicit seed, `N` base groups from 2 through 10 million,
   antithetic selection and execution mode. Validate exact
   grid and events, actual path/node work, draw slots and
   live storage before simulation. Retain current state for
   each active leg, its required payoff state and scores, and
   bounded reduction scratch.
   Remove the four-million captured-point ceiling and
   inherited capture-only limits from this route. Reject
   capture, Sobol, Brownian-bridge QMC and adaptive stopping
   requests. Use the accepted Philox addresses, fixed layout
   and ordered native reduction, rejecting unsupported
   WASM parallel requests.
   [Accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

7. **Form an antithetic observation after completing each
   leg.** Use one triple per base group. When paired,
   compute complete triples from the primary and reflected
   paths, including each leg's own signed scores and PV.
   Average the corresponding components with checked
   arithmetic. For vega, reflection preserves the squared
   term and reverses the linear term. Multiplying an
   averaged PV by an averaged score generally changes the
   result. If a leg or component fails, report group,
   leg and node or operation as applicable and fail the
   whole run. No dropping observations or partial triple.

8. **Publish marginal fixed-sample uncertainty.** Reduce
   price, delta and vega over the same complete groups.
   Use the checked sample mean, standard error and two-sided
   95% Student-t interval of record 01 for each component.
   The intervals are marginal. Shared paths can correlate
   the results. Report the same `N` estimators and `N` or
   `2*N` physical trajectories for the whole run, with grid,
   currency, seed, pairing and execution provenance.
   Validate counts, units and finite ordered uncertainty
   on existing result restoration.
   [Accepted inference](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

9. **Use native Greek units.** Report spot delta in PV
   currency per spot unit and vega in PV currency per
   `1.0` of decimal annualized volatility. Remove the
   current implicit `0.01` multiplier. A per-volatility-point
   report uses an explicit `0.01` scale in the reporting
   owner. Apply the same selected scale to the uncertainty
   values. Signed results remain valid.
   [Accepted native derivatives](../models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results).

10. **State the ideal-law target and grid sensitivity.**
    The density identity gives local derivatives of the
    fixed payoff and grid under the ideal Gaussian law,
    when differentiation under the expectation is justified.
    Require finite second moments of the completed observations
    for the sampling interpretation.
    Finite sampling, finite random draws, rounded paths
    and cash, and payoff or model discretization retain
    separate error. For independent ideal Gaussian shocks,
    the unweighted delta score variance is
    `1/(S0^2*sigma^2*dt_1)` and the vega score variance
    is `2*m/sigma^2+sum_i dt_i` for `m` intervals.
    Refinement can increase score noise. These are score
    variances, with payoff-weighted uncertainty estimated
    from the actual groups. A constant realized sample
    can yield a point interval. The method gives no
    unconditional variance advantage.
    [Density score references](../../../REFERENCES.md#gbm-likelihood-ratio-delta-and-vega).

11. **Reconcile the existing Asian host operation.** Keep
    `price_with_lrm_greeks` on its existing Rust, Python and
    WASM path-dependent pricer. Require explicit GBM market
    and run controls, currency, exact grid, averaging kind,
    ordered positive-time fixing rows and frozen history.
    Retain tie multiplicity and fixed row multipliers under
    record 17. Require at least one live fixing on this
    host route. Remove every-step inference, the 32-step
    registry default, inherited pricing defaults and
    implicit currency. Reconcile the `LrmGreeks` result,
    wrappers, wire/schema, declarations, exact seeds, stubs,
    examples and parity. Keep generic payoff qualification
    and score execution Rust-owned. Dated products retain
    history eligibility, settlement, model and market risk.
    [Accepted Asian payoff](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs),
    [dated asset Asian](../valuations-fx-and-exotics/asset-asian.md).

12. **Qualify the score and price together.** Test a terminal
    call and a strict cash digital against independent
    analytic Greeks, an irregular two-fixing linear claim
    against exact moments, and fixed-input bumps on smooth
    examples. Test consumed-shock identity even when
    spots repeat after rounding, zero and tiny volatility
    boundaries, origin or bridge rejection, signed and extreme
    PV scaling, complete pairing, resource and error order,
    replay and host units. Qualify sample size and grid
    sensitivity separately. No automatic estimator switch.
    [Accepted finite differences](25-finite-difference-greeks.md#accepted-decision-for-checked-paired-monte-carlo-spot-greeks).

## Observed behavior and limitations

The [path-dependent pricer](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs)
requires a uniform grid, flat carry, strictly positive volatility
and no pairing or Sobol. It forces full spot capture, limited to
four million captured points and generic capture admission.
It reconstructs normals from adjacent captured spots with
`T/num_steps`, divides the stored discounted payoff by the
factor and reapplies discount through the score helpers.
The [score helpers](../../../../finstack-quant/models/src/monte_carlo/greeks/lrm.rs)
index delta shocks without checking length and zip vega
inputs, silently truncating a mismatch. Vega is currently
scaled to one volatility point. Payoff eligibility uses a
Rust opt-in predicate.

The [Python](../../../../finstack-quant-py/src/bindings/models/monte_carlo/pricers.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs)
methods compose a unit arithmetic Asian with every post-step
node as a fixing, defaulting to 32 steps. The dated product
uses its own pricing and risk routes.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| One-year call, `S0=K=100, r=.05, q=.02, sigma=.20` | Independent integration and closed form give PV about `9.22700550815`, delta `0.586851146135` and native vega `37.901157510017`. |
| Strict unit cash digital call at the same inputs | PV `0.494581091053`, delta `0.018950578755` and signed native vega `-0.473764468875`. |
| Undiscounted linear average `(S_.25+S_1)/2`, `S0=100, sigma=.2`, zero carry | Exact joint-score delta is `1` and vega `0`. Misapplying the terminal marginal scores gives `0.625` and `-1.875`. |
| `S0=100, sigma=1e-16, dt=.25, z=1`, zero carry | The next stored spot rounds to `100`. Recovering the shock from that spot yields about `2.5e-17`. The path consumed `1`. |
| `P=1e-28, z=.5, S0=1e-300, sigma=1e-30, dt=1` | The complete delta contribution is about `5e301`. Forming the denominator product first would underflow. |
| One antithetic group, `S0=100, sigma=.2, dt=1`, `z=(+1,-1)`, discounted PVs `(3,1)` | The group contributes `(2,.05,-1)` in price, delta and native vega. Averaging scores and PVs first incorrectly gives zero Greeks. |

Selected tests compare with finite differences under shared GBM
inputs and exercise payoff admission. Independent examples check
the proposed ideal score and arithmetic. Runtime, host and broader
sensitivity qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
