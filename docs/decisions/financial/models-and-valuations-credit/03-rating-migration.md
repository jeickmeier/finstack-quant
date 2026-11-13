# MODELS-AND-VALUATIONS-CREDIT-03: Rating transition, generator recovery and path simulation

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Current behavior observed in the source listed below. The accepted contract requires Rust and caller reconciliation.
- Validation status: Independent formula and boundary examples evaluated. Production tests, host checks and empirical qualification were not run.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked rating migration, generator fitting and paths

1. **Require the complete state space and default designation.** A scale has
   at least two unique, nonblank, case-sensitive labels in one explicit order.
   Choose a designated default label or explicitly declare no designated
   default. The default label can occupy any position. Custom, labelled and
   DataFrame construction require the same validated scale, with both matrix
   axes in that order. Retire last-label inference and the competing custom
   constructor. Retain explicitly selected standard, standard-with-NR and
   notched presets as label layouts with declared default roles. Axis order
   identifies matrix states. Rating quality, notch distance, factor lookup and
   master-scale mapping use their own contracts. `NR` can be an explicitly
   modelled state. Its origin row, exits and treatment of withdrawal require
   caller-supplied semantics and data. Do not add, discard, redistribute or
   reweight a state to make a published table square.

2. **Admit exact structural invariants on checked binary64 values.** Both
   matrix types require exact square shape on the scale. Canonicalize signed
   zero. A supplied transition row has finite entries in `[0,1]` and its checked
   ordered compensated sum equals `1.0`. Keep the admitted probabilities.
   The designated default row is the literal unit row. In a directly supplied
   generator, every off-diagonal intensity is finite and nonnegative. Derive
   the canonical exit rate from their checked ordered compensated sum and
   require the supplied diagonal to equal its negative. Reuse that exit rate
   for inspection, projection and simulation. The default row is all zero.
   Any other zero exit row also absorbs, with default classification determined
   solely by the scale. Invalid input fails without tolerance, flooring or
   normalization. Exact structural comparisons use represented values.
   [Core accumulation contract](../core-numerics/sample-statistics.md).

3. **Use elapsed model years and ordered composition.** Express `Q` in inverse
   years and durations in finite nonnegative elapsed model years. A transition
   at zero duration must be the exact identity. Projection at zero returns it
   after input validation. Recovery requires a strictly positive source
   duration. A zero-duration path starts and ends in its initial state with
   zero draws. Compose successive kernels in chronological row-vector order
   `P1 P2` for equal ordered scales and default designations, without requiring
   equal durations. Add durations with checked arithmetic and reject a positive
   increment absorbed by rounding. A zero-duration operand acts as identity.
   The caller owns contiguity, calendar-to-year conversion, population and
   measure compatibility. Composition alone establishes no common generator.

4. **Keep one bounded, restricted principal logarithm.** For source `P(T)` use
   a checked real Schur decomposition with positive finite diagonal eigenvalues
   and no complex blocks. Bound Schur to `64*n*n` iterations at binary64
   `EPSILON` with checked size arithmetic. Reduce the triangular form with at
   most 64 checked principal square roots until `||X||∞ <= 0.25`, where `X` is
   the reduced form minus identity and `||·||∞` is the maximum absolute row
   sum. Use at most 64 whole-matrix Taylor terms for `log(I+X)`. Stop only when
   the computed remainder bound
   `||X^(k+1)||∞/((k+1)(1−||X||∞))` is at most `EPSILON` times the norm of the
   current series sum. Rescale, use the direct scalar logs on the triangular
   diagonal and transform back to obtain dimensionless `L`. Check intermediate
   values and errors. Divide entries by `T` when materializing annual rates,
   without first forming `1/T`. An exact identity yields a zero generator for
   any admitted positive duration. Unsupported spectra and exhausted or
   unresolved computations fail. Valid continuous-time generators can
   project to matrices outside this recovery domain. This method searches
   only the stated branch. [Generator existence and approximation](https://www.probability.ca/jeff/ftpdir/wei.pdf).

5. **Require fit mode and two independent acceptance budgets.** Strict
   principal mode rejects any negative computed off-diagonal entry and any
   nonzero off-diagonal entry in the designated default row. Opt-in diagonal
   adjustment clips negative off-diagonals to zero and restores the designated
   default row to zero. Both derive the diagonal from the canonical exit rate
   and refuse a positive annual rate lost during conversion. Both require a
   finite nonnegative dimensionless maximum for `||TQ−L||∞` over the complete
   matrix, including diagonal and default changes. Separately require a
   finite source-horizon fit maximum in `[0,2]` for
   `||P_checked(Q,T)−P(T)||∞`, using the same public checked projection and the
   unchanged admitted source. Compare both diagnostics inclusively. Remove
   the default `0.01` and automatic adjustment. Return the fit mode, source
   duration, declared budgets, measured full correction and fit residual,
   removed negative off-diagonal mass on `L`, and the projection correction
   alongside the generator. Direct `Q` construction has no fit diagnostics.
   A passing fit supports only the declared in-sample discrepancy. Exact
   embeddability, uniqueness, extrapolation quality and financial suitability
   require separate evidence. [Generator existence and approximation](https://www.probability.ca/jeff/ftpdir/wei.pdf).

6. **Retain checked order-13 Padé projection.** For positive time form `tQ`
   with checked range, rejecting a nonzero intensity lost to zero when
   forming or scaling this argument. Return exact identity for a zero
   generator. Select
   the least nonnegative binary scaling that makes its induced 1-norm
   (maximum absolute column sum) at most `5.371920351148152`. Cap the number
   of halvings and corresponding squarings at 64. Use one checked pivoted LU
   solve of the Padé matrix equation `M R=N`. Require finite intermediates and
   the scaled solve residual `||M R−N||₁/(||M||₁||R||₁+||N||₁) <= 1e−12`,
   computed without unnecessary overflow. Propagate failure without switching
   methods. Check every squaring and finalize the result using the following
   row rule. The threshold comes from Higham's Padé profile. The specified
   finite-precision gates constrain work and residuals, with forward accuracy
   and extreme-range qualification pending. [Higham, Algorithm 2.3 and Table 2.3](https://www.cis.upenn.edu/~cis6100/higham_matrix_exponential_siam_2004.pdf).

7. **Measure computed probability-row cleanup.** For projection and computed
   transition products or terminal frequencies, validate the raw finite
   matrix. Preserve any row already admitted unchanged. Otherwise replace
   negative entries by zero and divide by the checked positive row mass.
   Where the operation implies a literal unit row, including a projected zero
   exit row and every designated default row, construct that exact row.
   Require the maximum full row L1 change from the raw matrix to the delivered
   matrix to be at most `1e−12`, inclusive, and report it with the computed
   result. Count every change in that budget. Recheck the delivered matrix
   against direct admission and fail if any row still fails or the correction
   is too large. This fixed numerical budget is separate from the caller's
   generator-fit budgets. It measures the actual adjustment and does not
   certify matrix exponential error or rare-event relative accuracy.

8. **Use one fallible Gillespie jump law and explicit replay controls.** Use
   one CORE-NUMERICS-17 PCG stream per call with explicit `u64` seed and
   stream. Process paths in supplied
   order, and all-state simulations in canonical source-state order followed
   by path order. At a nonabsorbing state with time remaining, consume one
   uniform `U` in `[0,1)`, set `k=floor(U*2^52)` and use the exact open
   midpoint `V=(2*k+1)*2^−53`. The holding time is `−log1p(−V)/λ` where `λ`
   is the canonical exit intensity. Handle out-of-horizon holds with scaled
   comparisons when division would overflow. For a represented in-horizon
   event require strictly advancing finite time in `(t,T]`. Include an event
   exactly at `T` and stop there. Only for an in-horizon jump consume one more
   uniform and select the first strictly greater cumulative probability over
   positive off-diagonal rates in canonical order. Build checked increasing
   normalized cutpoints with final endpoint exactly one. Reject collapsed
   positive intervals. Remove the rate floor and destination fallback. An
   absorbing row consumes no draws. Full-path and terminal-only operations
   share this kernel and consume the same draws for matching jobs. Finite RNG
   resolution limits sampled probabilities. Replay depends on the pinned
   RNG, job order, arithmetic and target math implementation.
   [Core PCG contract](../core-numerics/pseudorandom-and-bridge.md),
   [Gillespie's direct method](https://www.ma.imperial.ac.uk/~nsjones/gillespie_1977.pdf).

9. **Bound execution and make sampled results conditional and explicit.**
   Require positive path counts and one explicit positive work budget per
   call, charging one unit per path start and one per attempted holding draw,
   including a censored hold. Check shape, count products, allocation and
   work before use. Exhaustion or any failed path rejects the entire result.
   A `RatingPath` has its scale, initial state, finite duration and strictly
   ordered state-changing events in `(0,T]`, with no event after entering
   default. Lookups are right-continuous on `[0,T]` and reject other times.
   Default time is first entry, including zero if already in designated
   default, or unavailable when there is no observed hit. The explicit scale
   designation distinguishes a no-default model. Replace `default_rate`
   with `default_frequency`. Require a common starting state, scale and
   duration, count all paths in the denominator and return unavailable for
   empty input or no designated default. Replace `empirical_matrix` with
   `simulated_transition_matrix`.
   Count terminal states for each starting state using the positive supplied
   count per row, retain counts and run controls, and derive probabilities
   without smoothing, censor-based exclusions or selective retention. A
   frequency of zero in a defined sample is literal. Structural agreement of
   externally restored paths does not establish common generator provenance
   or independent sampling.

10. **Validate canonical state and reconcile existing consumers.** Serialize
    ordered labels and the required nullable default label, canonical
    row-major matrix rows and transition duration, and only the path state
    needed to validate its history. Reject malformed shapes before constructing
    a matrix, unknown fields, invalid states, time violations and derived
    claims that cannot be verified. Rebuild the scale lookup and path default
    time. Fit diagnostics belong to the result of the fit, outside the bare
    generator wire. Rust owns all validation and calculations. Reconcile
    Python, WASM, the parity ledger, generated schemas and declarations and
    the UI worker and its fixtures. Hosts pass through required controls,
    nullable results and errors. Python integers and WASM safe integers or
    bigint preserve exact `u64` seed and stream. Distinguish invalid inputs,
    unknown states, unsupported recovery and failed or exhausted computations.
    Reconcile ECL's incidental migration-scale dependency under the core
    rating contract. The ECL owner sets downgrade and staging policy.
    [Core rating contract](../core-conventions/019-credit-ratings.md).

11. **Keep source interpretation and downstream policy with the caller.** A
    designated default column is cumulative conditional model default
    probability for the stated initial state and duration. External matrix
    sources require version, observation window, population/cohort, weighting,
    rating crosswalk, default definition, treatment of censoring/withdrawal
    and probability measure. A rated-origin table that reports NR only as a
    destination cannot identify transitions out of NR. The supplied model is
    one finite-state homogeneous single-name chain. Time-varying generators,
    dependence across obligors, rating-factor tables, empirical estimation,
    valuation and regulatory uses keep their own decisions.
    [S&P cohort methodology](https://www.maalot.co.il/Publications/FTS20250331162126.pdf#page=67).

## Observed behavior

- Transition construction accepts edge excursions of `1e−12` and row sums
  within `1e−8`, clips entries, and tolerates small positive default leakage.
  Custom and scale-omitted labelled construction can infer the last state as
  default. Both matrix wire readers flatten the incoming storage before
  rebuilding the square shape. The scale wire also stores its derived map.
- Direct generator construction accepts approximate conservation and adjusts
  small negative intensities. Its diagonal accessor and the simulator's sum
  of positive rates can disagree. Recovery uses unbounded real Schur,
  triangular square roots and the bounded series. It automatically adjusts
  off-diagonals and the default row, and defaults the maximum round-trip
  residual to `0.01`. It reports negative mass and maximum absolute row-sum
  reconstruction error. Direct and restored generators also carry those
  diagnostics as fields.
- Projection uses order-13 Padé, partial-pivot LU and then full-pivot LU on
  failure. It clips negative probabilities and normalizes without measuring
  the full change. Current norm, scaling and residual arithmetic lack the
  proposed finite and work gates.
- Simulation uses one seed-only PCG stream, floors tiny exit rates, and uses
  the final positive destination when the cutoff loop finds no match. Full
  and terminal paths share that jump loop. Lookup clamps some
  out-of-range values, restoration permits same-state events, and the
  default-frequency helper accepts mixed samples and substitutes zero for an
  empty sample. The UI worker infers default from label order and exposes an
  optional recovery tolerance. ECL staging consumes the scale for downgrade
  ordering. [UI worker](../../../../finstack-quant-ui/registry/shared/file/services/models.ts),
  [ECL staging](../../../../finstack-quant/statements-analytics/src/analysis/ecl/staging.rs).

## Evidence

Owners: [matrix.rs](../../../../finstack-quant/models/src/credit/migration/matrix.rs), [generator.rs](../../../../finstack-quant/models/src/credit/migration/generator.rs), [projection.rs](../../../../finstack-quant/models/src/credit/migration/projection.rs), [simulation.rs](../../../../finstack-quant/models/src/credit/migration/simulation.rs), [scale.rs](../../../../finstack-quant/models/src/credit/migration/scale.rs). [Migration tests](../../../../finstack-quant/models/src/credit/migration/tests.rs) includes matrix invariants, reconstruction and simulation evidence; simulation/analytic agreement is internal model consistency. References: [Generator recovery](../../../REFERENCES.md#israel-rosenthal-wei-2001), [CreditMetrics](../../../REFERENCES.md#creditmetrics-1997), [Gillespie](../../../REFERENCES.md#gillespie-1977).

### Independent review evidence

- A two-state chain with annual intensity `1e−16` and `T=1e16` has cumulative
  default probability `−expm1(−1) = 0.6321205588285577`. The current simulator
  stops at its rate floor. An inconsistent input row `[0,1e−9]` passes the
  current conservation tolerance. At `T=1e9`, exponentiation and normalization
  give default probability `0.5`, while the holding-rate model gives
  `0.6321205588285577`. An input default row `[5e−9,0.999999995]` has unit mass
  but leaks out of default. The proposed admission rejects both malformed rows.
- For `P=[[.9,.1,0],[0,.8,.2],[0,0,1]]` at `T=1`, positive eigenvalues coexist
  with a negative principal `L[A,D]` of about `−0.0124225200`. Non-embeddability
  follows independently from positive A-to-B and B-to-D probabilities with
  zero A-to-D probability. Diagonal adjustment gives a full correction of
  about `0.0248450400` and a reconstructed row-L1 residual of `0.0234835751`.
  The two gates therefore test different properties. A separate valid
  directed three-state cycle at `0.5/year` with a fourth absorbing default
  state projects at `T=1` to first row approximately
  `[0.6191798809,0.3048457771,0.0759743419,0]`, from Poisson counts modulo
  three. Its complex spectrum places it outside the selected recovery branch.
- In state order `[A,D,X]` with `A→D=0.1/year` and `A→X=0.2/year` and both
  destinations absorbing, the row at `T=2` is approximately
  `[0.5488116361,0.1503961213,0.3007922426]`. Only the middle entry measures
  default. The waiting transform has exact input endpoints `2^−53` and
  `1−2^−53`, with strictly positive finite hazards. Binary64 examples also
  exhibit a zero rounded holding interval and a positive interval that does
  not advance an already represented time.
- Existing tests were inspected or located. Independent checks used scalar
  formulas and source inspection. Qualification still requires matrix-log
  conditioning and repeated spectra, independent exponential and semigroup
  comparisons, fixed-stream path and boundary cases, statistical uncertainty,
  schema/host/worker tests and source-specific cohort validation.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
