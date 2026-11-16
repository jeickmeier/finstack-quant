# SCENARIOS-005: Direct discount, forward and inflation rate shocks

- Decision status: **Accepted 2026-11-15 — complete direct discount, projection and CPI parameter shock package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent factor, contractual forward, off-pillar composition and anchored CPI calculations verified.** Production execution, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for typed direct rate and CPI parameter shocks

1. **Bind an exact typed source and validate the bump.** Select the
   discount, index-projection or CPI source by exact role and ID.
   Preserve the applicable index association. Convert signed basis
   points once to the decimal increment `b=bp/10,000`. Reject
   nonfinite values and nonzero bumps lost in conversion. Validate the
   source and geometry even for zero. Remove `discount_curve_id` from
   these direct operations.
   [Accepted typed market lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

2. **Apply continuous-zero shifts to discount and projection factors.**
   For the complete current factor evaluator use
   `C'(T)=C(T) exp(−b x w(x))`. A parallel shock uses `w=1`. A bucket
   uses the accepted exact triangular weights and flat outer wings.
   Preserve the original interpolation, tails and earlier shock shapes.
   Retire direct additions to displayed simple forwards and rebuilding
   from visible forward knots. Derive term forwards using checked
   shocked endpoint factors and actual contractual accrual.
   [Accepted factor shocks](../core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning) ·
   [Bank of England term-structure formulas](https://www.bankofengland.co.uk/-/media/boe/files/working-paper/1994/estimating-the-term-structure-of-interest-rates.pdf).

3. **Keep scenario date, source base and conditioning reference distinct.**
   For rate factors use `x=τ_B(T)−τ_B(v)`, where `B` owns the original
   source clock and `v` is the selected curve view's conditioning
   reference. Earlier shocks retain their own anchors. An authored tenor
   resolves to an exact date using its explicit origin and applicable
   roll rules, then maps to the source clock. A difference between dates
   is admissible when the source supports it. Record 006 owns node matching
   and any inverse delivery to evaluated off-pillar targets.
   [Node delivery record 006](06-node-delivery-solve.md).

4. **Shock CPI annual growth at original source pillars.** Keep source
   anchor `I_0` fixed. At each affected positive source pillar `t_i`,
   add the weighted increment to current annual gross growth
   `g_i=(I_i/I_0)^(1/t_i)` and reconstruct the CPI level through the
   checked source owner. Reinterpolate once on the retained grid, style
   and tails. Exact node targets identify unique positive original pillars.
   Preserve published fixings and retire the zero-time inflation-rate
   sentinel. Direct delivery governs the elected annual parameter
   moves at those pillars.
   [Accepted CPI parameter shocks](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks).

5. **Publish checked effects in scenario order.** Require finite positive
   factors or CPI levels wherever the owner requires admission. Admit
   negative rates within those domains. Record the exact target, requested
   bp, native parameter basis, source dates, geometry and actual effect.
   Preserve applicable original calibration records as history. A changed
   evaluator requires fresh qualification before quote replay. Publish
   through the accepted scenario transaction.
   [Accepted calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification) ·
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- A `+100 bp` continuous-zero shift takes a `4%` simple projected rate
  to about `5.2138198%` for a source-clock interval of `.30` years
  and contractual accrual of `.25`. The growth factor becomes
  `(1+.04×.25)exp(.01×.30)`.
- Take a `3%` flat continuous source with visible pillars at years
  1 and 3. A `+25 bp` triangular bucket centered at year 2 with
  endpoints 1 and 3 followed by a `+10 bp` rightmost bucket centered
  at year 3 with left endpoint 1 gives the year-two factor
  `exp(−.066)≈.93613086429`. Discarding the first shape at the
  visible-knot rebuild gives `exp(−.061)≈.94082323978`.
- A source CPI anchor of `100` and annual growth of `2%` give
  a two-year pillar of `104.04`. A `+100 bp` annual parameter
  move gives `106.09` with the same source anchor.

## Observed choice and rationale at the audit revision

Observed choice: rate scenarios operate on model curve quantities. Discount shifts change continuously compounded zero rates; forward shifts change projected rates; inflation shifts change the curve's implied inflation representation. These are distinct from shifting market calibration quotes and re-solving a full rates calibration.

## Ordered calculation and selection

For finite `bp`, `CurveParallelBp` first verifies the requested typed curve exists, then emits `BumpSpec::parallel_bp(bp)` to core. The discount result is `D'(t)=D(t)*exp(-bp*1e-4*t)`, anchored to the curve's current origin through its transform implementation. Forward shifts add `bp*1e-4`. Inflation uses the core inflation bump kernel; its projection rebuild and day-count semantics are recorded in [inflation projection](../core-market/inflation-projection.md). An optional `discount_curve_id` is validated as text by the spec but has no role in these direct rate operations.

For node requests, resolve requested tenors and use [native node delivery](06-node-delivery-solve.md). Discount and inflation produce triangular key-rate effects with left/right neighboring pillars. The first/last triangular buckets extend flat outside their outer side, as defined by core. The discount target quantity is continuous zero rate; the inflation target is `inflation_rate(0,t)`, with an explicit zero return for `t<=0`.

Forward nodes take visible knots and projected rates, add per-knot decimal shifts, then `rebuild_with_knots`. Interpolation style and metadata are retained, but rebuilding replaces the prior source/continuous-adjustment representation with the visible-knot interpolant. Thus a forward-node operation can change off-pillar behavior from previous continuous shocks even when selected pillar values are preserved. This follows the rebuild boundary; the numerical size for compounded real scenarios has not been reproduced here. Discount nodes instead apply composed transforms through `Bumpable`.

## Domain, failure and alternatives

Missing/wrong-kind data errors. Negative rates are not rejected merely for being negative; curve-specific finite/DF/domain validation controls acceptance. Construction constraints after stress can differ from original curve calibration constraints; see [core rate shocks](../core-market/rate-shocks.md). Node conversion uses scenario as-of and curve day count, so a mismatched scenario date and curve base date changes tenor-to-time mapping. No calendar is used for curve node conversion.

Quote bump and full recalibration would measure a different risk. A uniform continuous-transform implementation for forward nodes would preserve prior shock shape; preserving visible rates alone is the observed method. Agree which composition invariants are required and whether mismatch between execution date and curve base must error.

## Evidence

Owning source: [adapters/curves.rs](../../../../finstack-quant/scenarios/src/adapters/curves.rs); shared kernels [rate shocks](../core-market/rate-shocks.md). Inline tests inspect exact/interpolated delivery and preservation of forward tenor/reset-lag/grid/day-count/interpolation/extrapolation/FX-policy metadata. [Curve-variant tests](../../../../finstack-quant/scenarios/tests/shocks/curve_variants_test.rs) locate integration evidence. Selected positive-rate monotonicity fixtures do not prove all stressed DFs decrease or that negative forwards are prohibited.

## Relationship to the source and delivery owners

[CORE-MARKET-03](../core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning)
and [CORE-MARKET-10](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks)
own the factor and CPI calculations. [Record 006](06-node-delivery-solve.md)
owns node matching and inverse delivery. [Calibration targets](../calibration-targets/README.md)
own quoted-inflation recalibration.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
