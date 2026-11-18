# FEATURES-021: Inclusive rolling OLS current-row residuals

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Inclusive rolling fits and missing-window behavior are implemented. Empty-design admission and checked raw residual evaluation require reconciliation.

**Validation status:** Exact arithmetic checked rolling residuals with a missing current response. Rust fixtures were inspected. Empty-design and solver boundaries still require qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Fit complete rows in the inclusive trailing window using the checked full-rank SVD contract. Require at least max(min_periods, p) complete rows, where p counts exposures and the intercept. The intercept defaults to on. Allow intercept-only fitting and reject a request with neither exposures nor an intercept. [Shared OLS](20-scaled-svd-neutralization.md).
2. Return the current row's observed minus fitted residual in response units, with the current row included in its own fit. Return missing for an incomplete current row, insufficient complete rows, or rank deficiency. Preserve small representable residuals. Solver or arithmetic failures fail the whole call. [Numerical failure policy](02-parameters-missing-and-failure.md).

For exposures [1, 2, 3, 4] and responses [1, 1, 2, missing], a four-row window with minimum count 3 gives [missing, missing, 1/6, missing].

## Observed scope and selection

`rolling_regression_residual` uses the [scaled SVD solver](20-scaled-svd-neutralization.md) per entity and inclusive trailing row window. Parameters are positive `window` (default 1), `min_periods` (default window, <=window), and boolean `fit_intercept` (default true). Exposure columns and entity/order keys must match the response row count. The result is the residual at the current row, not fitted coefficients or forecasts.

## Ordered algorithm and failures

1. Validate parameters and shapes before scanning data; sort entities and order keys using the shared deterministic ordering.
2. For each trailing window, collect rows with finite y and every finite exposure. Require at least max(number_of_columns_including_intercept,min_periods) complete rows. Windows include missing rows in membership even though missing rows cannot count toward readiness.
3. Fit the same normalized full-rank SVD and exact-fit roundoff threshold described in FEATURES-020. No-fit from insufficient count, zero design width/spread or rank deficiency leaves the current output None and proceeds to the next row.
4. If a fit exists, evaluate only the current row. A missing current response/exposure gives None even if earlier complete rows support a fit. A finite complete current row is included in the estimation and its own fitted residual; this is an in-sample end-of-window residual, not a next-period forecast error.
5. SVD nonconvergence, solve errors or nonfinite coefficients are errors, not silently missing windows. Final nonfinite emitted residuals fail the entire public call. No partial result or fit-diagnostic series is returned.

Outputs retain dependent-variable units; time is entity-row position and lexical order, with no date interval or frequency conversion. A one-column intercept-only fit is possible when exposures is empty; no-intercept plus empty exposures has width zero and never fits. Exactly determined complete windows are allowed, often yielding roundoff-clamped zero residuals.

## Rationale, alternatives and evidence

Documentation explicitly distinguishes rolling rank-deficient None from cross-sectional whole-call failure. Proposed rationale is preserving a time series through unestimable windows, but whether that policy should expose diagnostic reasons remains a pending choice. Prior-only training windows, leave-one-out residuals, regularization and skipping to the latest finite observation would change the calculation and are not selected.

- Owning [rolling regression](../../../../finstack-quant/features/src/multi.rs): `rolling_regression_residual`, `fit_ols`, `residual_for_idx`; [row windows](../../../../finstack-quant/features/src/index.rs).
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs) expect final residual 1/3 for one hand three-row fit.
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): `rolling_regression_requires_the_configured_complete_row_count` requires four complete rows in a five-row window, gives final residual .5, and contrasts all-None singular rolling output with cross-sectional error. Exposure-unit sweeps also compare the final rolling residual to 1/6.

## Follow-up tracking

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
