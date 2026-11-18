# FEATURES-015: Cross-sectional tie ranks, quantile buckets and normal scores

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Tie rules and normal-score mapping are implemented. Exact bucket boundary arithmetic and a portable count range require reconciliation.

**Validation status:** Exact arithmetic checked tied ranks and labels. An independent floating-point calculation reproduced a bucket boundary error. Rust fixtures were inspected. Host and boundary qualification remain open.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Rank finite partition members and preserve missing rows. Closed rank uses the lowest tied zero-based position divided by n−1, with zero for singletons. Open percentile uses the average tied one-based position divided by n+1. Normal score applies the standard-normal inverse CDF to the open percentile. Singleton and all-equal partitions receive open percentile 0.5 and normal score zero.
2. Default to 10 buckets and use a shared portable count range with exactly representable labels. Compute floor(closed rank × buckets) with exact integer arithmetic and cap at buckets−1. Keep ties together and permit empty buckets.

For [1, 2, 2, 4], open percentiles are [0.2, 0.5, 0.5, 0.8] and four-bucket labels are [0, 1, 1, 3]. Floating evaluation of (15/22) × 22 can floor to 14. Exact arithmetic gives bucket 15.

## Observed scope and selection

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

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
