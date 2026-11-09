# CORE-NUMERICS-20: Checked simulation grids and exact event knots

- Decision status: **Accepted — complete time-grid package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded binary64/rational references and read-only constructor/state/caller reviews were completed on 2026-11-08; these do not certify production execution, a uniform-only scheme or pricing convergence.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/time_grid.rs`: zero-origin continuous time grids, optional required times, exercise-time snapping, and date-based step mapping. The accepted core boundary below removes the generic financial snapping helpers. It does not choose a product timestep, calendar, or numerical scheme.

## Accepted decision: checked simulation grids and exact event knots

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host reconciliation and independent numerical/financial qualification
remain pending; observed behavior is retained below.

1. **Keep numeric model time and financial chronology separate.** `TimeGrid`
   stores year-fraction coordinates from model time zero, not dates, observation
   frequencies or a calendar. Existing financial callers must declare their
   actual model clock/context and reconcile other clocks explicitly. No generic
   timestep, day count, calendar, exercise, monitoring or expiry policy is inferred.
   Product/model numerical schemes and accuracy budgets remain separate owners.

2. **Use one immutable validated knot invariant.** Retain private `times` and
   derived step/horizon state, `Clone`/`Debug`, and the existing checked
   `from_times(Vec<f64>)` constructor. Require nonempty finite nonnegative knots,
   first value numerically zero and all later knots strictly increasing.
   Normalize signed zero to `+0`; derive every stored `dt` and horizon from knots.
   No sort, duplicate removal, spacing floor or coordinate repair in custom
   construction. No new core serde/default/builder, unchecked path or mutable
   field. All constructors publish state only after common validation succeeds.

3. **Make zero horizon an explicit identity.** `[0]` is a valid grid with zero
   intervals. Select `uniform(0,0) -> [0]`; reject positive horizon with zero
   steps and zero horizon with positive steps. Negative/nonfinite horizons error;
   signed zero normalizes to `+0`. Required-time construction has the same rule
   and still validates every supplied event before returning `[0]`. Model/product
   callers may retain narrower positive-horizon/minimum-step requirements; this
   identity does not silently authorize expired or empty simulations.

4. **Pin one overflow-safe uniform arithmetic profile.** Keep
   `uniform(t_max,num_steps)` with an explicit integer count. For positive `T,N`,
   pin `t_0=+0` and `t_N=T`; for `0<i<N`, compute the binary64 division
   `r=(i as f64)/(N as f64)`, then the binary64 multiplication `T*r`.
   Do not first round `T/N`, multiply `T*i`, recursively add a step, or substitute
   another arithmetic profile. Validate the generated knots through the same
   invariant. Rounded collapse/nonfinite geometry is a computation/resolution
   error, not permission to reduce N, nudge knots or manufacture positive steps.
   This fixed two-operation profile is not universally correct rounding of the
   exact real `i*T/N`, nor a guarantee that every valid real mesh is representable.

5. **Make actual adjacent knots authoritative for every interval.** Always
   compute `dt[i]=times[i+1]-times[i]`, finite and strictly positive, including
   every uniform interior interval. Remove the repeated nominal `T/N` cache
   recipe and custom `1e-10` floor. Permit representable positive subnormal
   intervals. No independent nominal step may replace actual geometry in engine,
   bridge, kernel, payoff or path-result calculations. Their numerical suitability
   remains separately checked; no exact telescoping/constant-dt claim is implied.

6. **Simplify required-time construction to explicit counts and exact union.**
   Keep one `uniform_with_required_times(t_max,num_steps,required_times)`;
   remove `steps_per_year`/`min_steps` and their density-rounding policy from core.
   Validate the complete event list: every value finite and in closed `[0,T]`,
   with signed zero normalized. Reject even `nextUp(T)`, not a tolerance snap.
   Validate the canonical base grid before union: merging events must not repair
   collapsed base knots or silently reduce its requested count. Form the union
   of validated base knots and events; sort and deduplicate only
   exact numerical equality. Preserve every distinct represented valid coordinate,
   including adjacent floats. Keep exact 0/T without overwriting a distinct knot.
   No hidden drop, `1e-10` merge, terminal adjustment or fallback base grid.

7. **Preserve event identity and multiplicity outside the coordinate set.**
   Deduplicating grid coordinates does not deduplicate fixing/payment/observation/
   exercise records. Owners retain original dates, order, labels and multiplicity;
   multiple events can deliberately reference one knot. Do not collapse Asian
   fixings or imply a common payoff/priority rule when different dates share a
   clock coordinate. Close distinct coordinates remain separate; owner schemes
   must explicitly admit their steps or fail, rather than silently coarsening them.

8. **Provide one checked exact knot lookup.** Add
   `get_index(time) -> Result<usize>` over the actual stored grid. Validate a finite
   in-horizon coordinate, normalize zero and return its exact numerical-equality
   knot index using binary search. A nonmember is an input/missing-knot error;
   there is no nearest, epsilon match, clamp or fabricated terminal index. Origin
   and terminal are valid nodes. Use this after inserting required events and
   retain repeated returned indices when event multiplicity requires them.

9. **Use canonical getters and checked indexing.** Rename accessors to
   `get_num_steps`, `get_t_max`, `get_times`, `get_dts`, `get_time` and `get_dt`,
   deleting old names rather than aliases. Full slices/count/horizon remain
   immutable infallible views; indexed queries return `Result<f64>`. Nodes use
   `0..=N`, intervals `0..N`; invalid indices are contextual input errors, never
   panic or zero/last-value fallback. `[0]` has node 0 and no interval 0.

10. **Define uniform recognition precisely, without an epsilon.** Retain
    `is_uniform()` as exact recognition of the canonical stored-knot profile
    defined in decision 4, recomputed from actual horizon/count/coordinates.
    `[0]` returns true. Construction provenance is irrelevant: custom input with
    those same knots has the same answer. Remove absolute `1e-10` comparisons
    and do not introduce another tolerance or cached origin flag. This means
    canonical uniform layout, **not equal stored adjacent steps or eligibility
    for a uniform-only model**. With `eta=2^-1074`, `uniform(3*eta,2)` can have
    steps `[2*eta,eta]` and still match that layout. It cannot authorize replacing
    those steps with `T/N` or claim a small relative perturbation.

11. **Check integer/resource admission before allocation and conversion.**
    Limit actual interval counts and uniform index conversion to `N<=2^53-1`,
    further constrained by platform `usize`, checked `N+1`, candidate merge sizes,
    vector byte/layout bounds and fallible reservations. Check merged counts too.
    No saturating float casts, unchecked capacity arithmetic or allocation panic
    path. Invalid requested shape/count is admission failure; allocation and
    generated resolution failures are computation failures. Errors preserve
    inputs and discard private partial state. Candidate-space/time admission does
    not promise every huge request is cheap. Do not impose the bridge's separate
    100,000-step cap on all grids or create new public resource/options objects.

12. **Remove generic financial date/exercise snapping helpers.** Delete
    `map_exercise_dates_to_steps` and `map_date_to_step`, their imports/re-exports
    and obsolete tests/docs. No replacement core nearest-step, rounding/tie,
    half-step extension, filtering or clamping API. Tree/product owners must
    explicitly adopt any allowed timing displacement, collision handling and
    exercise/maturity eligibility, and validate the full input before mapping.
    Do not relocate the old silent drop/clamp behavior unchanged as a private
    compatibility helper. Exact required-event insertion and deliberate numerical
    approximation remain different contracts.

13. **Keep date conversion faithful to accepted clocks and identities.**
    Actual owners convert from one fixed declared model origin with the required
    day count/context, composing valid reference slices when a convention needs
    them. Preserve calendar chronology and event records even when distinct dates
    map to one coordinate, including zero. A future date mapping to zero is not
    automatically expired; a nonpositive/unusable clock span is not permission
    to return an arbitrary node. Propagate missing context/clock failures and
    validate owner date bounds independently of numerical ratios. Accepted core
    dates 001/002 and shared ADR 0045 remain authoritative, not reopened here.

14. **Reconcile actual simulation and valuation consumers together.** Build one
    validated grid and use its same knots/dts for simulation, Rough-Heston kernel
    precomputation, bridge/QMC, payoff/event indexing and retained path times.
    Remove the independently rebuilt `expiry*i/n` kernel grid. Update all getter/
    Result consumers and replace epsilon event searches with exact lookup.
    Move McConfig/product density-to-count calculation to its actual owner,
    preserving separate review of that formula/default rather than re-inferring
    an integer count via `N/T`. Reconcile Asian/autocallable/cliquet/swaption,
    convertible and binomial callers, benchmarks and tests without new aliases.
    Volterra and LRM consumers must validate their own rounded-mesh suitability;
    `is_uniform()` alone is no equal-step/scheme/Greek certificate.

15. **Retain host and model-state boundaries.** Generic core TimeGrid and lookup
    remain intentionally Rust-only. Preserve existing tagged model
    `TimeGridSpec::Uniform{expiry,num_steps}` / `Times{times}` wire shapes and
    build them through canonical validation. Reconcile model-specific positive
    horizon/minimum counts, generated schema/docs/declarations and faithful
    result times; core `[0]` does not redefine those host/model contracts. No new
    TimeGrid host class, core serde/cache wire state, migration or fallback grid.
    Accepted 17 bridge and 18/19 randomness/fractional contracts remain separate.

16. **Qualify geometry, timing and price claims separately.** Cover actual dt
    identities, pinned endpoint/signed zero, tiny/custom/collapsed grids, exact
    required events and repeated indices, canonical recognition versus unequal
    dt, count/capacity/index failures and complete caller/kernel-grid identity.
    Independent binary64/rational examples, located runtime tests and actual
    financial convergence studies are distinct evidence. Owners must qualify
    deliberate snapping/discretization, monitoring/exercise and scheme accuracy;
    generic grid validation supplies none of those budgets. This review changes
    no runtime, tests, fixtures, dependencies, generated schemas or host API.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Custom `[]` / `[0]` / `[-0.0]` | Error / zero-step grid / normalized `+0` zero-step grid. |
| `uniform(0,0)` / `(0,1)` / `(1,0)` | `[0]` / error / error. |
| Custom `[0,eta,2*eta]`, eta=`2^-1074` | Valid tiny positive intervals; no absolute spacing floor. |
| `uniform(eta,2)` | Generated knots collapse; computation/resolution error. |
| `uniform(3*eta,2)` | Knots `[0,2*eta,3*eta]`, steps `[2*eta,eta]`; canonical layout true, equal-step suitability unproved. |
| `T=1,N=10` | Interior knot 3 is the selected-profile `0.3`; every stored dt comes from actual adjacent knots. |
| Required `.5` and `nextUp(.5)` within `[0,1]` | Preserve both distinct knots despite their tiny separation. |
| Required `nextUp(1)`, NaN or a negative time, with T=1 | Full-input error; no terminal snap or ignored event. |
| Two contractual events with the same valid time | One coordinate, two event records/indices when required by the product. |
| `get_index(.3)` on knots `[0,.5,1]` | Missing-knot error, not nearest step. |
| `[0]`: `get_time(0)` / `get_dt(0)` | Zero / invalid-interval error. |
| Uniform-only model receives canonical layout with materially unequal rounded dt | Model checks/fails explicitly; canonical recognition is insufficient. |
| Exercise/date outside an owner's valid domain | Owner validation/explicit timing policy; no generic core clamp/drop. |

## Observed choice and rationale

Uniform grids store the exact terminal time; required times are merged into the grid so product events can be represented. Separate nearest-step helpers snap events onto an existing uniform grid. These approaches have materially different event timing and must not be treated as equivalent.

## Observed algorithm

1. `uniform(T,n)` requires finite T>0, n>0 and allocatable capacity. First round `dt=T/n`, then emit 0, `i*dt` for interior i, and exact T; store n−1 copies of that nominal dt and final dt=T−last interior time. It does not pass through the custom-grid minimum-dt validator.
2. `uniform_with_required_times(T,density,min_steps,events)` uses `n=max(round(T*density),min_steps)`, creates uniform knots, accepts only finite event times>1e-10 up to T, and snaps events less than 1e-10 beyond T onto T. Others are ignored.
3. Sort knots, deduplicate differences<1e-10, force last knot=T, then call `from_times`. That constructor requires nonempty grid beginning numerically at zero, finite strictly increasing times, finite differences and every dt≥1e-10. A singleton [0] is valid.
4. `map_exercise_dates_to_steps` accepts ratios in `[0,1+0.5/steps]`, rounds `ratio*steps`, clamps to final step, then sorts/deduplicates. Invalid ratios are ignored. Nonpositive total time or zero steps returns empty.
5. `map_date_to_step` computes maturity/event year fractions with caller day count/context, clamps event time into [0,T], rounds to nearest uniform step; T≤0 or zero steps returns 0. Day-count errors propagate.

## Observed selection, controls and conventions

T, density, minimum count, event times, and day count are explicit. Times must share units, normally years; tolerances are absolute time units. Indexing accessors assume a valid index. `is_uniform` compares all stored dt values with the first within 1e-10.

## Observed limitations, failure and fallback

Required-time construction silently drops invalid/out-of-range inputs and merges close events. Nearest-step mapping changes timing by up to half a step and can include slightly beyond-maturity events. Uniform and custom construction differ on tiny dt acceptance. No grid-convergence evidence or event-preservation guarantee follows from allocation success.

## Observed alternatives and tradeoffs

Merge required events for exact represented timing, or snap to a fixed uniform grid for schemes requiring uniform spacing. Both are implemented explicitly. Adaptive refinement and rejection of every invalid event are not automatic options.

## Evidence

- [Core implementation and inline tests](../../../../finstack-quant/core/src/math/time_grid.rs)
  show private immutable state but inconsistent nominal versus adjacent dt,
  custom spacing floor, tolerance event merge/snap, unchecked indexed access and
  generic financial snapping. Located endpoint/capacity/event/date/context tests
  were inspected/located, **not executed**. Their old drop/clamp/floor assertions
  describe observed behavior, not this accepted contract or pricing convergence.
- Independent **2026-11-08** binary64/exact-rational examples checked the selected
  two-stage uniform profile at ordinary, MAX and subnormal scales; pinned
  endpoints; actual dt differences; rounded duplicate refusal; signed zero and
  singleton/invalid grids; exact adjacent-float event union; canonical recognition
  versus unequal dt and retained repeated event indices. A represented `.15`
  nearest-step boundary also demonstrates why a rounding operation profile is
  distinct from exact real nearest-step classification; no core approximation
  is selected here. No production kernel or runtime suite was called.
- [Official Rust binary64 documentation](https://doc.rust-lang.org/std/primitive.f64.html#associatedconstant.MANTISSA_DIGITS)
  identifies the 53-bit significand. The selected `2^53-1` index cap,
  two-stage knot profile, zero-grid admission, exact event policy and removals
  are project decisions, not portable financial-error or convergence guarantees.
- Read-only constructor/state/caller reviews identified actual boundaries:
  [model config/grid builder](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs),
  [simulation and independent Rough-Heston precomputation](../../../../finstack-quant/models/src/monte_carlo/simulate.rs),
  [fractional uniform-only selection](../../../../finstack-quant/models/src/monte_carlo/simulate/fractional.rs),
  [Asian fixing multiplicity/epsilon lookup](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs),
  [binomial exercise mapping](../../../../finstack-quant/models/src/trees/binomial_tree.rs)
  and [convertible clock/window mapping](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/valuator.rs).
  Convertible coupon/window adjustments are owner financial behavior; they are
  neither blanket generic clamp approval nor newly adopted product policy.
- [Accepted day-count rules](../core-conventions/001-actual-day-counts.md),
  [accepted reference-period rules](../core-conventions/002-coupon-context-day-counts.md)
  and [shared clock boundaries](../conventions/0045-financial-clocks-and-date-boundaries.md)
  distinguish date identity/context from numeric time. [Host parity scope](../../../../finstack-quant-py/parity_contract.toml)
  and existing model TimeGridSpec, rather than a generic core binding, own host
  reconciliation. No new market/reference requirement is invented for numeric
  grid storage; product PDE/tree/MC records must qualify their own discretization.

Exact grid/event representation does not establish validated product timing,
rounded uniform-only eligibility, numerical convergence or financial fitness.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
