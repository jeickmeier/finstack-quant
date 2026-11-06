# COVENANTS-08: Deterministic forecasts, effective-term grouping and breach selection

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Single-spec forecast

Validate nonempty supplied periods, each start no later than its inclusive end, strictly increasing end dates, and no date before an explicit reference. Validate configuration and the spec; reject nonnumeric covenant types. At each test date, resolve the scheduled/static threshold (engine amendments additionally apply only through the batch internal path), then active/full-waiver/springing gates, then the required metric and denominator. Inactive dates use an internal NaN placeholder but output no projected value/headroom and zero breach probability. Invalid-ratio dates have absent values/headroom and probability one.

For deterministic mode, breach probability is the exact shared indicator. Headroom always uses unshocked base projections, even in stochastic mode. `first_breach_date` is the first active base-case breach, not the first date crossing the probability-reporting cutoff or a simulated first passage. `min_headroom_date/value` select the smallest finite base headroom using total floating-point ordering; no finite values gives `None`. Tied minima retain iterator minimum behavior rather than an explicit economic tie-break convention. Deterministic/analytic standard-error arrays contain zeros.

## Batch method

Validate configuration and the entire engine, then validate all periods before testing. Resolve applicable windows by date, filter requested scope, omit nonnumeric specs, and group dates by specification object identity. A label reused in a replacement window is a different group; one top-level spec can own disjoint date spans around a replacement. Resolve one common reference as the day before the first input period starts when not explicit, including deterministic batch calls. Each group is forecast separately with that reference and effective waivers.

Include every active base-case breach regardless of stochastic probability cutoff. In stochastic mode additionally include every active date with marginal probability `>= breach_probability_threshold`, default 0.05. At cutoff zero this includes even active zero-probability dates. Sort output by breach date then exact label. There is no early exit after the first breach, no aggregation of repeated dates into episode probabilities, no cure-period/default forecast, no execution of incurrence actions and no partial list on error. `FutureBreach` omits estimator standard error even when produced by Monte Carlo.

## Rationale and alternatives

Source comments make effective engine terms, complete input coverage and preservation of deterministic breaches explicit. Single-spec forecasting intentionally lacks engine waiver/window context. A first-passage/default probability, a maximum-over-covenants risk statistic, selective period imputation or a confidence-bound inclusion rule would require a separate method and joint model. The current date lists do not constitute those statistics.

## Evidence

Owning [forecast composition](../../../../finstack-quant/covenants/src/forward.rs), [effective term selection](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), and [dated wrappers](../../../../finstack-quant/covenants/src/series.rs). Inspected [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) cover scope/windows/amendments/inactivity, `stochastic_cutoff_never_hides_base_case_breaches`, and date/config rejection. Inline forward tests cover missing metrics, nonnumeric omission and deterministic first-breach selection.

Tests were inspected only.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
