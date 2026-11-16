# SCENARIOS-010: Base-correlation and structured-credit correlation shocks

- Decision status: **Accepted 2026-11-15 — complete exact base-correlation, validated structured-credit dependence and effect publication package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent detachment, sector and matrix admissibility calculations verified.** Production execution, pricing, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked scenario correlation shocks

1. **Bind the exact base-correlation source and match native detachments.**
   Require the qualified pool, maturity and loss-model basis. Divide integer
   detachment bp by 100 to obtain percent of original pool notional.
   Thus `700 bp` selects `7.0`. A native correlation delta of `.01` is one
   correlation percentage point. `None` selects all knots and an explicit
   empty list selects none. Validate the source, finite delta and every
   selector even at zero. Reject duplicate and off-grid detachments.
   [Accepted base-correlation sources](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks).

2. **Admit the complete base-correlation replacement.** Add the delta
   at the selected knots and reject any result outside `[0,1]`.
   Preserve exact endpoints, untouched knots and source identity.
   Rebind affected credit-index dependencies atomically. Keep shape
   findings advisory. The selected tranche pricer owns loss consistency
   and endpoint support.
   [Accepted typed market lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

3. **Require exact structured-credit inventory targets.** Supply a
   nonempty ordered list of unique top-level IDs. Resolve each target
   and the parameter actually consumed by its selected pricing route
   at the action's execution point. Missing, wrong-kind, composite and
   unsupported targets fail, including for zero. Retire the inventory-wide
   scan and silent skips. Record 011 owns composite price and spread
   overrides.
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Instrument override record 011](11-instrument-overrides.md).

4. **Apply and validate the selected asset-correlation structure.**
   Flat moves its single coefficient within `[0,1]`. Parallel sectored
   actions add the same delta to intra- and inter-sector coefficients.
   Distinct deltas require explicit targets and a complete candidate
   satisfying `0 ≤ inter ≤ intra ≤ 1`. Matrix actions add the delta
   once to each mirrored off-diagonal pair, preserving ordered labels,
   symmetry and unit diagonal. Check the entry bounds and apply the
   accepted numerical PSD gate with an explicit tolerance. Retire
   half-deltas and clipping. Matrix repair is a separate election.
   [Accepted pool correlation](../models-and-valuations-credit/08-prepayment-and-pool-correlation.md#accepted-decision-for-checked-prepayment-and-pool-correlation) ·
   [Accepted matrix admission](../core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission) ·
   [Higham on separate correlation repair](https://eprints.maths.manchester.ac.uk/232/).

5. **Shock the active paired-factor prepayment/default coefficient.**
   Require an explicitly configured and consumed paired-factor
   correlation in `[-1,1]`. Shared-factor and inactive configurations
   fail. An asset matrix supplies no coupling coefficient by itself.
   [Accepted waterfall factor inputs](../valuations-structured-credit/15-stochastic-waterfall.md).

6. **Publish and report represented changes.** Stage the complete
   selected update through the scenario transaction. Preserve
   requested delta, exact target and parameter identity, and typed
   before-and-after values. Count only represented changes as applied.
   Distinguish a valid unchanged result from a failed action.
   Record 017 owns the report representation.
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- Detachments `3,7,10` with correlations `.25,.45,.60` receive
  a `+.05` shock at `700 bp` as `.25,.50,.60`. The same
  arithmetic rejects `.98+.03` under the closed domain.
- Starting at intra `.30` and inter `.20`, a parallel delta
  of `.10` gives `.40,.30` and preserves their difference `.10`.
  The earlier half-delta rule gives `.40,.25` and a difference `.15`.
- A 3-by-3 identity matrix with a `−.60` parallel off-diagonal
  change has row sum and eigenvalue `−.20`. Every coefficient
  lies in range and the matrix fails exact PSD admission.

## Observed choice and rationale at the audit revision

Observed choice: apply additive decimal correlation changes, with bounds enforced by clipping rather than rejection. Surface base correlations and portfolio-model correlations are separate targets with different clipping intervals and semantics. A change of `0.01` is one correlation percentage point; the scenario spec allows finite changes in [-2,2].

## Base-correlation algorithm and confirmed unit discrepancy

`BaseCorrParallelPts` passes no detachment filter; bucket operations convert each integer `detachment_bp` to `f64/10,000`, then emit a typed base-correlation bucket bump. Core selects exact detachment knots within absolute `1e-10`, adds points and clamps correlations to [0,1]. The adapter previews matching knots to warn on clipping, and warns when a **nonempty** requested filter matches nothing. Empty filters silently select nothing. A missing surface is ultimately an application error, not a price-scalar fallback.

**Confirmed static unit mismatch:** core `BaseCorrelationCurve` documents detachment coordinates as **percent of pool notional**, supports [0,100], and tranche pricing queries `attach_pct`/`detach_pct`. A 7% tranche therefore queries 7.0. The scenario adapter maps 700 basis points to 0.07 rather than 7.0. Scenario bucket tests construct knots at 0.03/0.07/0.10 and label them 3%/7%/10%, so they demonstrate internally consistent fractional fixtures while missing the production convention. A bucket request can therefore warn/no-op against conventionally scaled curves. This is a reconciliation defect, not a new optional model convention; runtime impact has not been executed here.

## Instrument correlation algorithm

Scan the supplied top-level inventory and downcast to `StructuredCredit`; ignore other instruments and structures absent from `credit_model.correlation_structure`. Unlike generic price/spread shocks, this path does not descend into composites. Delegate to the model's structure bump:

- Flat asset correlation adds delta and clamps [0,0.99].
- Sectored asset correlation adds delta to intra-sector and **half delta** to inter-sector, each clamped [0,0.99].
- Matrix asset correlation adds delta to every off-diagonal entry and clamps [-0.99,0.99], retaining diagonal/labels. The adapter performs no post-bump PSD test or projection.
- Prepay/default correlation adds delta and clamps [-0.99,0.99] for flat/sectored structures. A matrix has no such field: the model returns it unchanged with a no-op message.

Every matching instrument is assigned the returned structure and counted as changed, including zero shocks and matrix prepay/default no-ops. All returned messages are wrapped as `CorrelationClamped`, even the matrix no-op explanation. If no matching instruments exist, emit `CorrelationShockNoMatch`. Applied-shock reports retain requested points, not effective clipped changes.

## Alternatives and evidence

Owning source: [adapters/basecorr.rs](../../../../finstack-quant/scenarios/src/adapters/basecorr.rs), [engine/instrument_shocks.rs](../../../../finstack-quant/scenarios/src/engine/instrument_shocks.rs). Inspected dependencies: [core base correlation](../../../../finstack-quant/core/src/market_data/term_structures/base_correlation.rs), [model correlation structure](../../../../finstack-quant/models/src/credit/pool/correlation.rs), [tranche query](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/expected_loss.rs). [Core methodology](../core-market/base-correlation.md) states the percentage coordinate. Inspected [bucket fixture](../../../../finstack-quant/scenarios/tests/shocks/bucket_filtering_test.rs) shows the conflicting fractional convention; inspected [engine tests](../../../../finstack-quant/scenarios/src/engine/tests.rs) assert flat clamping and report units. No independent tranche-price or matrix-validity benchmark was executed.

Loss-space/no-arbitrage recalibration and nearest-correlation projection are alternatives, not behavior implemented by these adapters.

## Relationship to the accepted base-correlation boundary

[CORE-MARKET-18](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
owns the exact detachment coordinate, source domain and shape diagnostics.
[CREDIT-08](../models-and-valuations-credit/08-prepayment-and-pool-correlation.md#accepted-decision-for-checked-prepayment-and-pool-correlation)
owns portfolio-model dependence and paired-factor admission.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
