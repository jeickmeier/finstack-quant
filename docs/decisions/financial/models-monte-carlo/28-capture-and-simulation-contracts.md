# MODELS-MONTE-CARLO-28: Captured-path diagnostics and public process/scheme dispatch

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Existing capture, estimates and raw Rust/Python/WASM simulation observed. Exact capture, diagnostic isolation, model-owned dispatch and checked restoration are pending.
- Validation status: Source, selected tests, primary references and independent selection, storage, grid, quantile and cashflow examples reviewed. Runtime, host and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked path capture and raw simulation

1. **Keep capture explicit and separate from inference.** Select
   off, all, or a fixed count with an explicit capture seed on
   a capture-capable Rust pricing route. The population is the
   entire declared fixed run, with zero-based physical trajectory
   IDs. Unpaired Philox uses its pricing group IDs. Qualified
   randomized Sobol uses replicate-major IDs `r*M+j` for
   `R*M` physical paths. Pricing capture with antithetics fails.
   Specialized estimators retain their accepted capture boundaries.
   Price and uncertainty use the owning full-run observations.
   Remove generic width stopping and any subset-based stopping
   or convergence inference.
   [Accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [accepted Sobol pricing](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).

2. **Retain exactly the selected count.** For population `N`,
   fixed-size capture admits `1 <= k <= min(N,100000)`.
   All requires `N <= 100000`. Rank IDs before simulation by the
   existing integer score and then by ID. Use unsigned 64-bit
   wrapping arithmetic and logical shifts. With
   `m=0x9e3779b97f4a7c15`, start with
   `h=((id*m) xor capture_seed)*m`. In order, XOR with
   `h>>16`, multiply by `0x85ebca6b`, XOR with `h>>13`,
   multiply by `0xc2b2ae35`, and XOR with `h>>16`.
   Retain the lowest `k` keys with storage bounded by `k`.
   Return paths in ascending ID order. Keep capture and pricing
   seed roles independent. Do not consume simulation draws for
   selection. Completeness follows validated membership and
   count, including a fixed-size request with `k=N`. The
   pinned ranking selects a deterministic diagnostic subset.
   It gives no probability-sampling or risk-coverage guarantee.

3. **Bound actual retained diagnostic storage.** Preflight
   fixed shapes, selection state and layout before draws.
   Limit capture-owned retained storage to 512,000,000 bytes,
   the byte equivalent of the existing 64-million binary64
   storage budget. Charge the actual path and node records,
   indices, coordinate buffers and diagnostic capacities,
   including snapshot and cashflow payloads. Count shared
   storage once and check temporary layouts separately.
   Use checked size arithmetic and fallible reservations.
   Account for variable emissions before retaining them.
   An invalid capture request fails admission. A diagnostic
   failure during a valid run is reported separately from
   the price, with no incomplete dataset published.

4. **Use one event loop and complete-only settlement.** Use
   the same draws, actual knots, path and payoff resets,
   callbacks, cash-emission lifetime, errors and reduction
   with capture on or off. Read terminal settlement once per
   complete trajectory and save the same checked discounted
   Money PV used for pricing. Optional payoff or state
   snapshots are read-only diagnostic projections with an
   explicit meaning and units. Availability can begin after
   an observation or at completion. Distinguish absent
   diagnostics, literal zero and diagnostic computation
   failure. Do not call final payoff evaluation at origin
   or at an incomplete node to produce a snapshot. Keep
   capture state and errors outside payoff-owned state.
   [Accepted terminal and Asian payoff](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs),
   [accepted lookback diagnostics](19-lookback-monitoring.md#accepted-decision-for-checked-monte-carlo-lookback-extrema).

5. **Preserve timed, typed diagnostic cashflows.** Record
   the actual emitting knot, the separate owner-supplied
   payment time or date, and checked signed cash and currency
   with its component identity. Preserve emission order,
   zero cash and tied payments. Payment can be later than
   the emitting knot or the end of simulation. Treat event
   emissions as diagnostic-only, with the same post-event
   drain or discard on every path. Validate an emission
   independently of capture. Keep any economic cash
   state with the payoff. Generic capture applies no
   implicit discount, time conversion, sorting by payment,
   netting or inferred investment cash. Keep mark-to-market
   in noncash diagnostics. Remove automatic periodic IRR
   and amount-only return extraction. An owner can compute
   XIRR only from its own complete qualified dated stream,
   including actual initial and terminal investment amounts.
   [Accepted return conventions](../core-conventions/017-irr.md#accepted-decision-irr-and-xirr-timing-eligibility-and-numerical-acceptance).

6. **Report descriptive retained-path statistics.** Use the
   retained complete discounted PVs for minimum, R7 lower
   quartile, median, upper quartile and maximum. Validate all
   values and use the checked core interpolation. A singleton
   returns that value for all five measures. No capture gives
   unavailable subset statistics. Label the subset population,
   actual count, currency and sampling policy. Keep the values
   in the diagnostics, without substituting them for full-run
   estimates, population risk or QMC replicate spread.
   [Accepted statistics](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty),
   [Hyndman and Fan](../../../REFERENCES.md#hyndman-fan-1996-quantiles).

7. **Use the accepted price-result and uncertainty boundary.**
   Keep Money price, any admitted confidence bounds, estimator
   count, physical count and retained count distinct. Relative
   standard error is checked `SE/abs(mean)` at nonzero mean,
   unavailable at exact zero or when sampling uncertainty is
   unavailable. Remove the small-mean floor and infinity
   sentinel. Keep QMC spread descriptive. Same-seed runs
   across budgets can be coupled, and realized standard error
   need not decrease. Model and grid convergence remain
   separate from sampling and from capture.
   [Accepted estimation](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

8. **Select the raw model and admitted scheme explicitly.**
   Use the existing typed process request and one accepted
   scheme selection. Reuse exact GBM, dividend GBM, additive
   Brownian and multi-OU, Hull–White and Schwartz–Smith
   transitions. Use typed left-node log-Euler for local
   volatility, QE for CIR/CIR++, QE-M for Heston and the
   accepted dedicated LMM and rough-model schemes. Remove
   the generic Default switch, obsolete model Euler overrides,
   generic log-Euler/Milstein and generic fBM overrides.
   Keep checked diagonal Euler only for its own qualified
   Rust callers and standalone fBM with its own route.
   Reject incompatible combinations. Piecewise GBM, credit
   lines and other product-private paths keep their owners.
   [Accepted stepper contracts](05-generic-steppers.md#accepted-decision-for-checked-coordinatewise-euler-and-exact-gaussian-steps),
   [accepted fractional drivers](13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers).

9. **Return the owner-declared coordinates on exact knots.**
   Raw `simulate_paths` returns trajectory rows in
   `[path][knot][coordinate]` order, including origin,
   with no payoff, price or discount. Use exact stored
   receiving knots, never `t+dt` reconstruction. Admit
   the canonical origin-only grid when the model allows
   it, validate the full request and consume no draws.
   Keep each model's initial state in one canonical source
   and distinguish reported coordinates from innovation
   layout and named observables. CIR++ takes the unshifted
   initial factor and publishes checked signed shifted rate.
   Heston and rough Heston take spot with the model's
   initial variance. LMM takes its initial forwards once.
   Hull–White reports short rate. Schwartz–Smith and
   Cheyette expose their respective `x,y` states.
   [Accepted time grid](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
   [accepted CIR++ projection](08-cir-qe.md#accepted-decision-for-checked-cir-and-cir-quadratic-exponential-paths).

10. **Bound and order raw execution.** Require explicit
    seed, pairing and serial or native parallel execution.
    Current WASM rejects parallel. Use 1 through 100,000
    base groups, with one trajectory or the adjacent
    primary/reflected pair per group. Check
    `(physical_paths*dim+3)*stored_knots <= 64000000`
    for flat output and shared grid storage. Check actual
    preparation, scratch, draw and byte bounds separately,
    including the actual host transport where applicable.
    Apply the declared correlation law once in its
    innovation order, with strict admission and checked
    state at each knot. Return errors in canonical group,
    leg and operation/node order. Publish no partial
    result. Same-target replay follows the accepted
    profile and execution conditions.
    [Accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

11. **Bind and validate replay provenance.** Built-in runs
    retain canonical effective model inputs and grid,
    scheme, state and draw layouts, correlation owner,
    seed/profile, execution, pairing and actual counts.
    Captured results also record the price/settlement basis,
    capture policy and physical IDs. Generic Rust inputs
    whose full identity cannot be recorded have explicitly
    unavailable replay provenance. A seed and descriptive
    parameter map alone cannot certify replay. Restore
    through checked Rust state. Validate exact shapes,
    finite domain-valid coordinates, canonical labels,
    order, count relations, capture membership and pricing
    currency and uncertainty consistency before returning
    results. Preserve exact host seeds and reject malformed
    or obsolete wire state.

12. **Reconcile actual callers and qualify the contracts.**
    Keep generic capture and callbacks Rust-only. Reconcile
    the existing Python/WASM raw simulation, shared result
    accessors, schemas, declarations, facade, parity and
    examples. Revolving-credit path cashflows and other
    separately generated product diagnostics keep their
    own contracts. Verify exact selection, resource
    boundaries, diagnostic transparency and failure
    behavior, timed export, seed and shape restoration,
    exact knots, model admission, origin-only execution
    and native/host parity. Qualify model transition moments
    and product convergence on independent references.

## Observed behavior and limitations

The [capture selector](../../../../finstack-quant/models/src/monte_carlo/engine/path_capture.rs)
uses a probability threshold, so actual count can exceed
the requested count or nominal limit. The
[capture loop](../../../../finstack-quant/models/src/monte_carlo/engine/simulation.rs)
drains cashflows differently from the plain path, calls
final payoff for optional intermediate snapshots and infers
periodic IRR from amount-only components. The
[subset statistics](../../../../finstack-quant/models/src/monte_carlo/captured_path_stats.rs)
attach min, max and quantiles to the estimate.
[Money result](../../../../finstack-quant/models/src/monte_carlo/results.rs)
relative standard error uses a small-mean threshold and
an infinity sentinel.

The [raw dispatcher](../../../../finstack-quant/models/src/monte_carlo/simulate.rs)
has model defaults and generic scheme overrides, reconstructs
output times from steps and inherits execution choices.
[Path metadata](../../../../finstack-quant/models/src/monte_carlo/paths.rs)
validates square correlation shape and name counts.
Current result restoration does not establish full layout
or replay identity.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| Ten paths, capture count three, capture seed 17 | Pinned ranking retains sorted IDs `[2,3,5]`. The old threshold retains only two. |
| One million paths, requested capture count 100,000, capture seed 42 | The old threshold retains 100,093. The pinned ranking retains exactly 100,000, subject to retained bytes. |
| Full PVs `[0,10,20,30,40]`, captured PVs `[10,40]` | Price mean remains `20`. Subset min, Q25, median, Q75 and max are `10,17.5,25,32.5,40`. |
| Adjacent knots `0.2,0.9` | Publishing the stored receiving knot gives `0.9`. Reconstructing it as `t+(next-t)` gives `0.8999999999999999`. |
| 100,000 antithetic base groups, two coordinates, 159 stored knots | Raw output has 200,000 trajectories and uses `63,600,477` values under the storage check. With 160 knots it uses `64,000,480` and fails. |
| Investment example `-100` on 2025-01-01, then `+75` interest and `+46` principal on 2027-01-01 | Preserve both timed receipts. A qualified owner would find a 10% ACT/365F XIRR. Flattened periodic amounts yield 15% per synthetic period, so generic capture supplies no IRR. |

Existing source tests cover path selection, subset quantiles,
model steps and serialization. Independent examples check the
chosen diagnostic meanings and resource boundaries. Runtime,
host and financial qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
