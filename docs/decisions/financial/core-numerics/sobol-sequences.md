# CORE-NUMERICS-18: Finite-period Sobol points and prefix hash scrambling

- Decision status: **Accepted — complete Sobol package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent table/prefix/midpoint references and read-only sequence/state/caller reviews were completed on 2026-11-08; these do not certify production execution or a randomized-QMC estimator.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/random/sobol.rs`: dimensions 1..40, 32-bit direction numbers, deterministic point filling, optional seeded scrambling and inverse-normal conversion. This API does not estimate uncertainty or enforce a randomized-QMC replicate design.

## Accepted decision: finite-period Sobol points and prefix hash scrambling

Accepted on **2026-11-08** after whole-record review. Runtime,
caller reconciliation and independent numerical/statistical/financial qualification
remain pending; source-observed behavior is retained below.

1. **Keep one fixed finite Sobol profile and identify its actual table.**
   Retain dimensions 1..40 and 32-bit directions, including the conventional
   first coordinate. Freeze the recorded dimensions 2..40 subset of
   `new-joe-kuo-6.21201` (author D6 table, updated 2010-09-16), with its recorded
   retrieval/checksum and golden copy. No higher-dimensional fetch, alternate
   table, precision selector, repeated coordinates or automatic PCG fallback.
   Table/expansion/profile changes require explicit evidence and fixture
   reconciliation; recorded provenance is distinct from fresh authentication.

2. **Use one checked constructor with an explicit optional seed.** Select
   `SobolRng::new(dimension, scramble_seed: Option<u64>) -> Result<Self>`.
   Both arguments are required: `None` is unscrambled; `Some(seed)`, including
   `Some(0)`, selects the hash permutation below. Remove `try_new`, the zero-seed
   sentinel, default/bool/magic-seed controls and unchecked constructors. Check
   dimension, allocation/count arithmetic and fallible reservation before
   publishing private configuration/cache. Keep `Clone`/`Debug`; no new serde,
   public state restoration, options hierarchy or report type.

3. **Retain natural binary index order and its exact cache definition.** Raw
   point `i` XORs the 32 direction columns selected by set bits of `i`, starting
   at index 0. Incremental advancement must equal that direct definition,
   flipping all columns changed by binary carry. The initial `2^m` point sets
   agree with Gray order; corresponding later same-start blocks need not agree.
   Cross-library parity requires matching table, order, precision, scramble and
   midpoint map, rather than the shared Sobol label alone.

4. **Retain the bounded hash permutation under an honest name.** Freeze the
   inspected wrapping `hash_combine`/`should_flip_bit` formulas below, with
   zero-based dimension/bit indices and 32 derived bit seeds per dimension.
   Process MSB to LSB: `y_j = x_j XOR f_j(previous output prefix, bit seed)`.
   `Some(0)` runs this same algorithm. Call it a **seeded prefix hash digit
   permutation**; remove misleading private names/docs and decorrelation claims.
   No canonical Owen, linear-matrix, digital-shift or algorithm-selector promise.

   For `H(seed, value)`, in u64 modular arithmetic: set
   `h = seed XOR (value + 0x9e3779b97f4a7c15)`, multiply by
   `0xbf58476d1ce4e5b9`, XOR with `h >> 30`, multiply by
   `0x94d049bb133111eb`, XOR with `h >> 31`, then retain the low 32 bits.
   Bit seed `j` for dimension `d` is `H(H(seed, d), j)`. For the u32
   `q = previous_output_prefix XOR bit_seed`, set
   `w = q * 0x9e3779b9 + 0x6a09e667`, XOR with `w >> 16`, multiply by
   `0x85ebca6b`, XOR with `w >> 13`; its low bit determines the flip.
   All u32 operations also wrap. The empty higher prefix is zero.

5. **Separate deterministic net preservation from randomization theory.** Each
   output prefix uniquely determines the corresponding input prefix, giving a
   bijection at every depth through 32 bits. Coordinatewise application permutes
   dyadic boxes of the same shape and their occupancy counts, preserving any
   applicable finite net property within that resolution. It need not preserve
   the count in each fixed box for an arbitrary sample. Hash collisions do not
   invalidate the bijection. This establishes neither independent uniform node
   permutations nor uniform point laws across seeds, continuous-integral
   unbiasedness, canonical Owen variance rates or valid replicate error bars.
   Correct both the old denial of net preservation and unsupported statistics.

6. **Keep the exact interior midpoint map and point 0.** Map each integer
   `v` to `(v + 0.5) / 2^32`, exactly representable in binary64, with range
   `[2^-33, 1 - 2^-33]`. It preserves the first 32 prefix bits. No clamp,
   first-point removal, burn-in, thinning or silent index shift. This finite
   midpoint grid is not a continuous uniform law. Generic fill allows any
   complete-point count; callers own net/block design, with no automatic
   power-of-two rounding.

7. **Use accepted record 12's checked inverse-normal conversion.**
   `fill_std_normals` applies canonical `standard_normal_inv_cdf` to those same
   successive coordinates and propagates the first error. Require finite
   output; no floor, endpoint nudge, hidden draw, alternate approximation or
   pair cache. The ideal quantile grid is bounded by
   `+/-Phi^-1(1 - 2^-33)`, about **+/-6.337957754553789**. This is neither a
   continuous truncated-normal law nor exact continuous normal sampling or a
   financial-tail coverage guarantee. Accepted 12's accuracy limits still apply.

8. **Make the finite period a hard consumption boundary.** Publish existing
   `SOBOL_PERIOD = 2^32` as the canonical public u64 limit alongside
   `MAX_SOBOL_DIMENSION`. The logical index is the next point to emit, in
   `0..=P`; `P` is exhausted. Emit indices `0..P-1` at most once per explicit
   run, ending at a canonical exhausted cache without evaluating column 32.
   Remove modulo advancement, implicit reset and wrap. Over-consumption errors;
   deterministic repetition is not new information.

9. **Specify checked relative skip, explicit reset and clone.**
   `skip(n) -> Result<()>` advances by whole points, validating `n <= P-index`
   and arithmetic before mutation. Landing at `P` is allowed without evaluating
   a point cache; `skip(0)` is an identity even there. Infallible `reset` returns
   to index 0/raw zero cache, preserving dimension/seed configuration. `Clone`
   copies configuration/cursor/cache; it does not create an independent replicate.
   No wrapping skip, random start, reseeding convenience or snapshot layer.

10. **Require complete buffers and preflight the whole request.**
    `fill_u01(out)` and `fill_std_normals(out)` return `Result<()>`; length must
    be an exact multiple of dimension in every build. Output is row-major whole
    points in coordinate order `0..d-1`. Validate shape, point count, remaining
    period and required size/resource arithmetic before state/output writes.
    Admission failures leave both unchanged. Empty fills succeed without inverse
    calls or advancement, including at exhaustion. No partial-point/scalar API.

11. **Commit sequence state atomically without a full-buffer copy.** Evaluate
    using a bounded local cursor/raw cache of at most 40 words; commit generator
    state only after the whole fill succeeds. A later inverse/computation failure
    leaves generator state unchanged, while output is unspecified and must be
    discarded. No full-output transaction copy or hidden retry/reset. Uniform
    hash/midpoint arithmetic has no ordinary late numerical failure after valid
    preflight; successful output is finite. A bounded per-point cost does not
    promise an enormous valid request will be cheap.

12. **Keep errors and replay claims truthful.** Invalid dimension, shape, skip
    and over-consumption are input/state-admission errors with cursor/count
    context. Preserve computation failures for allocation, impossible internal
    state and inverse evaluation. No panic, successful NaN/infinity, fabrication
    or flattening all errors into validation. Hash overflow is defined modular
    arithmetic; cursor/budget overflow errors. Uniform replay requires the fixed
    integer/table/seed/map profile; normal replay also depends on numerical
    implementation/build. No old sentinel/wrap compatibility or universal
    platform identity promise.

13. **Remove the incompatible generic stream adapter and reconcile real users.**
    Core Sobol stays a vector-point API outside `RandomNumberGenerator`. Remove
    the obsolete core-Sobol `RandomStream` implementation, its private model
    module and stale references. The generic engine already rejects quasi-random
    streams; do not add scalar buffering or compatibility. Preserve the dedicated
    path-dependent whole-point driver, propagating `Result` with up-front
    per-replicate dimension/period/resource checks. Retain its distinct private
    precomputed `SobolPathStream`, but check normal-budget/cursor bounds and
    propagate errors instead of slice panic or invented normals. Accepted 17's
    fallible model fill chain remains authoritative.

14. **Keep block design, seed law and uncertainty with estimator owners.** Use
    the complete aligned base-2 blocks required by the selected net analysis.
    Arbitrary whole-buffer counts/skips or distinct fixed seed labels do not
    establish rates or error bars. Independent seed random variables can yield
    independent replicate estimates as deterministic functions of those seeds;
    this does not establish the hash family's continuous-integral unbiasedness
    or Owen law. Within-net points are not IID. Accepted 09's inference
    requirements still apply to replicate-mean uncertainty. Core supplies no
    replicate count, weights, confidence level, SE estimator or stopping rule.

15. **Preserve model and host ownership.** Generic core Sobol, including its
    period constant, remains Rust-only. Callers use explicit `None`/`Some` and
    remove the zero-sentinel `.max(1)` workaround where it only selected scramble
    mode. Actual seed/salt/replicate law, eight-replicate/default budgets,
    coordinate-to-risk-factor order, bridge flags, Philox auxiliary streams and
    uncertainty/control policy stay with separately reviewed owners. No new
    Python/WASM class, alias, schema, random-start or fallback method. Record 20's
    grids and accepted 17's bridge/RNG choices are not silently changed here.

16. **Qualify data, algebra, state and estimators separately.** Verify frozen
    rows/all 32 directions and direct-binary prefixes using independent expansion,
    including exact midpoints. Cover `Some(0)` versus `None`, hash vectors/prefix
    inversion, batching/skip/reset/clone/exhaustion, release full/partial/empty
    buffers, inverse-failure rollback/output discard and dedicated caller budgets.
    Structural proofs, recorded provenance, fresh authentication, located versus
    executed tests and statistical/financial qualification have distinct roles.
    This review changes no runtime, fixtures, dependencies, generators or hosts.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Dimension 0 or 41 | Input error; no extension or pseudorandom fallback. |
| `None` / `Some(0)` | Explicit unscrambled / seeded prefix hash profile. |
| Raw integer 0 / `2^32-1` | Exact midpoint `2^-33` / `1-2^-33`. |
| Unscrambled index 0 in 2D | Both coordinates `2^-33`; point retained and finite inverse. |
| First four unscrambled 2D points | Raw `(0,0),(.5,.5),(.25,.75),(.75,.25)`, then add `(2^-33,2^-33)`. |
| Nonmultiple output length | Error with cursor and output unchanged, including release builds. |
| Exactly `P` points consumed/skipped | Exhausted: nonempty fill/positive skip errors; empty fill/skip zero succeeds. |
| Reset after exhaustion | Explicit replay from point 0, not independent information. |
| Inverse fails during a valid fill | Generator unchanged; discard output; propagate error. |
| Fixed seeded prefix permutation | Dyadic boxes/counts are permuted; applicable net properties preserved, without Owen variance guarantees. |
| Independent seed RVs / distinct fixed labels | Independence can follow from the former design; fixed distinct labels alone do not establish it. |
| Generic model scalar Sobol adapter | Remove it; preserve the dedicated checked whole-point driver. |

## Observed choice and rationale

Stored Joe–Kuo-labelled direction data avoid repeated parameter generation. A prefix-dependent bit-flip transform scrambles coordinates deterministically. Midpoint conversion keeps all coordinates strictly inside (0,1), avoiding infinite normal quantiles. The reference label alone does not certify the chosen hash scrambler's theoretical randomization properties.

## Observed algorithm

1. First coordinate direction bit i is `1<<(31−i)`. Remaining coordinates use stored polynomial degree/coefficient/initial odd direction values and the Sobol XOR recurrence to fill 32 columns.
2. Point integer value is XOR of direction columns selected by binary digits of the index. Incremental advancement flips every direction column changed by binary addition (all lower bits through the first new one); it is checked against the same direct definition, not a Gray-code reorder.
3. Seed 0 disables scrambling. Otherwise derive 32 per-dimension seeds with `hash_combine`; process coordinate bits MSB to LSB, derive the hash input from already-scrambled higher bits XOR that bit's seed, and flip the current bit when the final mixed hash low bit is one.
4. Convert uint32 value v to `(v+0.5)/2^32`. Standard normals apply the shared inverse normal CDF to these coordinates.
5. Index starts at 0 and wraps to 0 after 2^32 points. `skip(n)` advances modulo that period and recomputes coordinate state directly. `reset` restores index/state but preserves seed configuration.

## Observed selection, controls and conventions

Dimension and scramble seed are explicit. Buffer fills consume successive complete points; buffer length multiple-of-dimension is guarded only by `debug_assert`. No scalar RNG-trait adapter, random start, independent-replicate standard error, or automatic power-of-two count is supplied by core. Models separately contain an incompatible scalar RandomStream adapter and a dedicated whole-point driver; their boundaries are recorded above.

## Observed limitations, failure and fallback

Unsupported dimension errors. The finite midpoint grid bounds attainable inverse-normal coordinates at its two extreme quantiles; it is not a continuous truncated-normal law. A partial final buffer chunk is accepted in release builds and still advances one point. Repeated period points are deterministic repeats, not additional information. Dimension/seed equality implies matching implementation output; no cross-library sequence equivalence or scramble distribution guarantee is established.

## Observed alternatives and tradeoffs

Unscrambled and seeded-scrambled modes are implemented. [PCG](pseudorandom-and-bridge.md) is a separate pseudorandom alternative. Higher-dimensional direction sets or other scrambling algorithms would change the sequence and require evidence; they are not fallbacks above 40 dimensions.

## Evidence

- [Implementation, direction data and inline tests](../../../../finstack-quant/core/src/math/random/sobol.rs).
  The located `incremental_state_matches_from_scratch_definition` test checks
  cache/direct-definition consistency at selected indices. Seed/reset/skip/wrap
  and finite-normal assertions were inspected or located, **not executed**.
  Existing wrap/sentinel expectations describe current runtime, not this accepted contract.
- [Golden-data provenance](../../../../finstack-quant/core/tests/data/README_sobol.md)
  identifies author file `new-joe-kuo-6.21201`, retrieval 2026-06-11, full-file
  SHA-256 `68eedd2a4e3b659b9695e7aff0f8ac68718bcf620730fc3d3a8c65df2a067441`
  and the [39-row dimensions 2..40 subset](../../../../finstack-quant/core/tests/data/sobol_joe_kuo_d2_40.txt).
  [Located golden tests](../../../../finstack-quant/core/tests/sobol_golden.rs)
  compare all 40 x 32 expanded directions and 16-point prefixes in dimensions
  1, 2, 5, 21, 30 and 40. They were not run. Current zero-seed/Owen descriptions
  in those runtime fixtures/docs require reconciliation when the API changes.
- Independent documentation references on **2026-11-08** matched all 39 embedded
  parameter rows to that golden copy, expanded all 40 direction vectors, checked
  the first eight natural-order 2D points, exact midpoint endpoints/prefixes,
  nine bounded 8-bit prefix permutations and selected full-depth inverse examples.
  A 100-digit Decimal Mills continued-fraction calculation brackets the positive
  extreme normal quantile between 6.337957754553 and 6.337957754554. These bounded
  examples and the structural bijection argument do not execute production code.
- A separate read-only comparison matched the recorded subset against
  [Boost.Random's Joe–Kuo-derived table](https://raw.githubusercontent.com/boostorg/random/develop/include/boost/random/detail/sobol_table.hpp).
  The [author page](https://web.maths.unsw.edu.au/~fkuo/sobol/) identifies D6 and
  its 2010-09-16 update; author notes distinguish direct binary and Gray order.
  The recorded full-file checksum was **not freshly authenticated** in this
  review. Subset agreement and recorded provenance do not certify every upstream
  byte or production execution.
- Read-only mathematical/state/caller reviews checked the complete proposal.
  [Model adapter](../../../../finstack-quant/models/src/monte_carlo/rng/sobol.rs),
  [scalar trait](../../../../finstack-quant/models/src/monte_carlo/traits.rs),
  [generic-engine rejection](../../../../finstack-quant/models/src/monte_carlo/engine/pricing.rs)
  and [dedicated driver/private path stream](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs)
  identify actual reconciliation work. These are not runtime defect reproductions
  or approval of existing replicate defaults/uncertainty.
- [Joe–Kuo data and construction](../../../REFERENCES.md#joe-kuo-2008-sobol),
  [Sobol](../../../REFERENCES.md#sobol-1967),
  [Owen randomization](../../../REFERENCES.md#owen-1995-scrambling) and
  [first-point/block cautions](../../../REFERENCES.md#owen-2021-sobol-first-point)
  have distinct roles. No cited Owen theorem authenticates this project's hash
  seed law, finite-grid unbiasedness, variance rate or financial estimator.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
