# FEATURES-018: Sign-preserving capped neutral weights by proportional redistribution

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Proportional capped redistribution is implemented. Final allocation checks and numerical boundary handling require reconciliation.

**Validation status:** Exact arithmetic checked a capped allocation and infeasible capacity. Rust fixtures were inspected. Exact-capacity and near-zero centering still require qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Center eligible signals within each partition and preserve the centered signs. Allocate 0.5 gross on each side. Rows at the mean receive zero, constant partitions receive zero weights, and missing rows stay missing.
2. Require finite 0 < max_abs ≤ 1, defaulting to 1. Apply caps independently on each side and redistribute remaining exposure in proportion to uncapped centered magnitudes. Fail the whole call if either side lacks capacity.
3. Enforce the individual cap. Check net zero and unit gross for varying partitions within a rounding tolerance before exposing the allocation. For [−10, −1, 1, 10], a cap of 0.30 gives [−0.30, −0.20, 0.20, 0.30]. A cap of 0.20 is infeasible because each side can carry only 0.40.

## Observed scope and selection

`CapWeights` accepts finite `max_abs` in (0,1], default 1. It is a separate allocation from simply clipping existing weights: center the original signal, retain centered signs, allocate positive gross 0.5 and negative gross 0.5, and enforce the final absolute cap. Missing values stay None. The grouped entry point applies the same method independently to each tuple partition.

## Ordered algorithm

1. Gather finite values and use scaled centering. Initialize all finite output rows to zero. Partition strictly positive deviations into the long side and strictly negative deviations into positive magnitudes on the short side; zero deviations remain zero.
2. If both sides are empty, return zero allocations. Otherwise require `side_count*max_abs >= .5` independently for both sides. An empty side or insufficient capacity fails the whole call; there is no sign reversal or lower-gross fallback.
3. Sort each side by descending magnitude. Build suffix sums from the smallest magnitude toward the largest so the remaining total is recomputed from still-active positions, avoiding subtraction of a dominant capped term.
4. Start remaining exposure R=.5. For each ordered magnitude m with suffix total S, set `w=min(R*(m/S),max_abs)`, emit the side's sign times w, and update `R=max(R-w,0)`.
5. Validate final outputs for finiteness. There is no final explicit residual/net/gross/cap tolerance check or corrective redistribution after the final iteration.

The cap is in unit-gross portfolio fractions; centered signs, not original raw signs, are preserved. Positions at the mean receive zero. Constant signals are a documented exception to unit gross and return all zeros. For feasible nonconstant signals, net zero, gross one and final caps are mathematical target identities, subject to floating arithmetic.

## Rationale, alternatives and limits

The API documents proportional redistribution; code comments specifically motivate suffix sums as reducing cancellation after dominant caps. Proposed rationale is preserving relative signal strengths among uncapped positions while meeting side budgets. Renormalizing a naively clipped vector can breach the cap; joint optimization, sign changes or a reduced gross target are different methods and are absent. Infeasibility is assessed by the computed centered signs, so near-zero centering roundoff can affect side membership/capacity.

## Evidence

- Owning [cap implementation](../../../../finstack-quant/features/src/cross_sectional.rs): `ResolvedCrossSectionalOp::resolve` and `cap_weights`; [grouped dispatch](../../../../finstack-quant/features/src/multi.rs).
- Inspected `final_weight_caps_preserve_neutrality_and_reject_infeasible_allocations` in [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): [-10,-1,1,10], cap=.3, expected [-.3,-.2,.2,.3], direct net/gross/cap assertions, two-row infeasibility and constant-signal zero output. Its numerical tolerance is 1e-11*max(1,abs(expected)).
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs) verify an already feasible .5 cap leaves the sample long-short allocation unchanged. These small cases are internal hand oracles, not broad optimization equivalence evidence.

No failure of these identities was numerically reproduced here.

## Follow-up tracking

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
