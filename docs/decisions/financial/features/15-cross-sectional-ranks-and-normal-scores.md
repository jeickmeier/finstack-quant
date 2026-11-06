# FEATURES-015: Cross-sectional tie ranks, quantile buckets and normal scores

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

`Rank`, `PercentileRank`, `QuantileBucket` and `NormalScoreTransform` operate on the same finite partition sort. Only quantile bucket accepts a parameter: positive integer `buckets`, default 10. No percentile estimation method, random tie breaking, equal-population constraint or tail clamp is selectable.

## Ordered algorithm

1. Gather finite `(original_index,value)` pairs, normalize signed zero, and sort by total numeric order then original index. Missing rows stay None.
2. For a tie run from zero-based position a up to exclusive position b in n finite values, closed rank is `a/(n-1)`. Singleton closed rank is 0. An all-tied partition also has closed ranks 0.
3. Open percentile uses average tied position `r=(a+b-1)/2`, then `(r+1)/(n+1)`. Singleton gives 0.5; all-equal partitions give 0.5. These plotting positions lie strictly between zero and one at ordinary representable counts.
4. Quantile bucket uses closed rank: `min(floor(rank*buckets),buckets-1)` returned as f64. The top distinct rank receives the final label. Tied values stay together and empty buckets are possible; the output is not a guarantee of equal group sizes.
5. Normal score applies core `standard_normal_inv_cdf` to the *open* percentile. Core delegates interior probabilities to a cached statrs standard normal distribution; endpoint saturation is not deliberately reached by the plotting-position formula.

Ranks/buckets/scores are dimensionless. Buckets are semantically integer labels in a floating column; very large bucket counts can exceed exact integer representation in f64. Numeric order within a partition is independent of entity identifiers and does not remove repeated entities.

## Rationale, alternatives and evidence

The chosen tie rules and open-rank-to-normal mapping are explicitly named in source, but no historical rationale selecting these plotting positions was found. Proposed rationale is deterministic tied signals and finite ordinary normal scores. Dense ranks, midpoint closed ranks, alternative plotting positions and equal-size bucket splitting would materially change signals and portfolio weights.

- Owning [rank implementation](../../../../finstack-quant/features/src/cross_sectional.rs): `partition_ranks`, `quantile_bucket`, `normal_score_transform`; delegated [normal quantile](../../../../finstack-quant/core/src/math/special_functions.rs) `standard_normal_inv_cdf`. Core accuracy commentary is not freshly validated here.
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs): tied open ranks [.2,.5,.5,.8], four-bucket labels [0,1,1,3], and three-point normal scores ±0.6744897501960818 and 0, generally at 1e-12 tolerance.
- Inspected `signed_zero_never_creates_a_rank_signal` in [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs) checks equality across all four transforms; it is a representational invariant.

No external quantile package was executed.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
