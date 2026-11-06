# FEATURES-007: Rolling ranks, Type-7 quantiles and quantile winsorization

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

`RollingRank`, `RollingQuantile` and `RollingWinsorize` share trailing row windows with positive `window` and `min_periods` defaults 1/window. Quantile defaults to 0.5. Winsor limits default to lower=0.01, upper=0.99; probabilities must be in [0,1], with lower<=upper. Invalid parameters fail before any ready window, including empty data.

## Ordered computation

1. Collect finite values in the inclusive trailing row window and require `min_periods` finite observations. Canonicalize signed zeros through the shared finite helper.
2. Sort with total ordering. For rolling rank, require a finite current value, find its first equal sorted position `j`, and emit `j/(n-1)`; singleton gives 0. Ties share the lowest position. All-equal windows therefore rank 0, not 0.5.
3. Rolling quantile emits the [Type-7 interpolant](04-scaled-statistics-and-quantiles.md), even when the current row is missing, provided the window has enough finite data.
4. Rolling winsorization requires finite current input and clamps it to the Type-7 lower/upper quantiles of the same inclusive window. Equal probability limits collapse the finite current value to a single quantile.

Rank is dimensionless in [0,1]; quantile/winsorized values retain input units. No full window is required beyond min_periods, and no prior-only or forward-looking window is selected. Reapplying winsorization recomputes bounds from the changed history, so these transformations are not specified as idempotent.

## Rationale, alternatives and evidence

The enum/source document the current-value rank and quantile clamp; no historical reason for lowest-tie ranking or Type-7 selection is recorded. Proposed rationale is deterministic signals and continuously interpolated cutoffs. Midranks, open plotting positions, discrete quantiles, and excluding current data are materially different alternatives; cross-sectional open ranks are separately available but are not selected here.

- Owning [rolling dispatcher](../../../../finstack-quant/features/src/timeseries.rs), [rank helper](../../../../finstack-quant/features/src/timeseries/advanced.rs) `rolling_rank_value`, and [quantile helper](../../../../finstack-quant/features/src/types.rs).
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs), `transform_timeseries_supports_advanced_rolling_signal_ops`, checks ranks, medians and a [0,.5] winsor range on small windows with 1e-12 absolute tolerance.
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs), `signed_zero_never_creates_a_rank_signal` and eager probability validation, checks identity and failure policy; opposite-sign extreme median checks one interpolation boundary.

These are research transforms, not evidence of unbiased predictive signals.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
