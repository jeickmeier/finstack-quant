# COVENANTS-08: Deterministic forecasts, effective-term grouping and breach selection

**Decision status:** Accepted 2026-11-17 for explicit forecast modes, effective-term coverage and breach alert selection.

**Implementation status:** Effective-term batch selection, base-case summaries and sorted breach output are implemented. Explicit modes and reference dates, full coverage, distinct alerts and uncertainty reporting remain pending.

**Validation status:** All 115 Covenants Rust tests and 14 focused forecast tests passed on 2026-11-17 against current behavior. The accepted coverage and alert contract still needs qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Elect deterministic, analytic or Monte Carlo mode explicitly. Stochastic modes require a supplied reference date and volatility. Deterministic mode reports the base-case assessment without a stochastic reference date.
2. Resolve batch forecasts by selected date, applicable window, explicit scope, waiver and effective threshold. Standalone forecasts use the supplied spec and schedule. Report effective terms and outcomes for every selected test, including incomplete and unassessed tests. Forecasting is read-only. [Effective terms](02-effective-terms.md) · [Period eligibility](07-dated-series.md).
3. Calculate first projected breach and minimum base-case headroom from unshocked projections. Keep absolute and relative summaries separate and compare only comparable values, using the earliest date for ties. A stochastic probability is marginal to one test date. First-passage, default and portfolio risk are outside this model.
4. Include every base-case breach in the alert list. Add stochastic alerts for a positive marginal probability at or above the caller's explicit cutoff. A zero cutoff includes all positive probabilities. Sort by test date and exact covenant label, identify each reason for inclusion and carry Monte Carlo standard error where applicable.

## Observed single-spec forecast

Validate nonempty supplied periods, each start no later than its inclusive end, strictly increasing end dates, and no date before an explicit reference. Validate configuration and the spec; reject nonnumeric covenant types. At each test date, resolve the scheduled/static threshold (engine amendments additionally apply only through the batch internal path), then active/full-waiver/springing gates, then the required metric and denominator. Inactive dates use an internal NaN placeholder but output no projected value/headroom and zero breach probability. Invalid-ratio dates have absent values/headroom and probability one.

For deterministic mode, breach probability is the exact shared indicator. Headroom always uses unshocked base projections, even in stochastic mode. `first_breach_date` is the first active base-case breach, not the first date crossing the probability-reporting cutoff or a simulated first passage. `min_headroom_date/value` select the smallest finite base headroom using total floating-point ordering; no finite values gives `None`. Tied minima retain iterator minimum behavior rather than an explicit economic tie-break convention. Deterministic/analytic standard-error arrays contain zeros.

## Observed batch method

Validate configuration and the entire engine, then validate all periods before testing. Resolve applicable windows by date, filter requested scope, omit nonnumeric specs, and group dates by specification object identity. A label reused in a replacement window is a different group; one top-level spec can own disjoint date spans around a replacement. Resolve one common reference as the day before the first input period starts when not explicit, including deterministic batch calls. Each group is forecast separately with that reference and effective waivers.

Include every active base-case breach regardless of stochastic probability cutoff. In stochastic mode additionally include every active date with marginal probability `>= breach_probability_threshold`, default 0.05. At cutoff zero this includes even active zero-probability dates. Sort output by breach date then exact label. There is no early exit after the first breach, no aggregation of repeated dates into episode probabilities, no cure-period/default forecast, no execution of incurrence actions and no partial list on error. `FutureBreach` omits estimator standard error even when produced by Monte Carlo.

## Rationale and alternatives

Source comments make effective engine terms, complete input coverage and preservation of deterministic breaches explicit. Single-spec forecasting intentionally lacks engine waiver/window context. A first-passage/default probability, a maximum-over-covenants risk statistic, selective period imputation or a confidence-bound inclusion rule would require a separate method and joint model. The current date lists do not constitute those statistics.

## Evidence

Owning [forecast composition](../../../../finstack-quant/covenants/src/forward.rs), [effective term selection](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), and [dated wrappers](../../../../finstack-quant/covenants/src/series.rs). Inspected [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) cover scope/windows/amendments/inactivity, `stochastic_cutoff_never_hides_base_case_breaches`, and date/config rejection. Inline forward tests cover missing metrics, nonnumeric omission and deterministic first-breach selection.

The current forecast regressions passed. Qualify explicit modes, deterministic dates at the calendar minimum, comparable headroom ties, zero cutoff and complete assessment coverage under the accepted contract.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
