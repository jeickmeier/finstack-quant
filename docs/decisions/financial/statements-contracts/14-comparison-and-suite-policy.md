# STATEMENTS-CONTRACTS-014: Numeric check tolerances, filtering and suite verdicts

**Decision status:** Accepted for the full numerical comparison and check-suite contract (2026-11-15).

**Implementation status:** Reconciliation pending. The audited suite derives verdicts after filtering and accepts supplied results without checking their evaluation provenance.

**Validation status:** Existing source and regression evidence mapped. Independent qualification of tolerances, coverage, filtering and result provenance remains pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for numerical comparisons and check suites

1. Default numerical identity tolerance to zero. An explicit tolerance uses `max(absolute tolerance, relative tolerance × the identity owner's reference)`. An absolute override replaces both terms. Equality with the tolerance is within tolerance. Record typed actual, expected, signed difference and tolerance at the compared precision. Require resolved, compatible units. [Balance-sheet comparison](10-balance-sheet-articulation.md#accepted-decision-for-balance-sheet-articulation) · [Formula units and currency](05-currency-dimensions.md#accepted-decision-for-formula-units-and-currency).
2. Determine pass, fail, incomplete or unevaluated from the unfiltered outcomes. An empty suite or one with no applicable checks is unevaluated. A pass requires evaluated work and complete required coverage. Show all diagnostics by default. Severity and compatible-unit materiality filters apply only to advisory diagnostics. Errors, coverage and numerical comparisons remain visible.
3. Run checks in declared order with unique check IDs. Merge suites only when their policies are compatible and their combined IDs are distinct. Append the second suite's checks in their declared order. Configuration or execution errors abort the run. Report completed failures and incomplete outcomes.
4. Accept supplied results only with verified identity for the validated model content, period plan and requested evaluation run, including source snapshot and cutoff where applicable. Reject missing or mismatched provenance. When no results are supplied, evaluate the model in the requested context. [Model validation](01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness) · [Snapshot and cutoff](02-authoring-and-availability.md#accepted-decision-for-authoring-imports-and-observation-availability).

## Audited input domain

CheckSuite runs an ordered list of Check implementations over a model and supplied StatementResult. It uses explicit findings to determine verdicts and preserves numeric comparisons independently of report filtering.

Default configuration is absolute tolerance 0.01, relative tolerance 1e-9, materiality threshold 0 and minimum severity Info. Numeric configuration and per-identity overrides must be finite and nonnegative. Tolerances are in the checked line's base units; relative tolerance is a fraction, while displayed Materiality.relative_pct is percent.

## Audited algorithm and controls

The accounting identity helper computes tolerance = max(default_absolute, default_relative × abs(reference)) unless the check supplies an absolute override. An override replaces the relative component completely. Reference selection belongs to the specific identity. CheckComparison.difference is actual−expected and within_tolerance uses abs(difference) <= tolerance.

Input-unit helpers compare result metadata to model declarations/inferred explicit values, error on contradictions, and otherwise prefer result type then model type. If both are absent, they assume Scalar. Capital-structure helpers additionally reconcile reporting and per-instrument currencies and validate cashflow-breakdown currency invariants.

Suite run validates config, then executes each check in order. Any execution Err aborts the whole run with no completed report. For each successful CheckResult:

1. Remove findings below min_severity.
2. If materiality_threshold>0, remove only non-Error findings whose supplied absolute materiality is below that threshold. Findings without materiality remain. Error findings are never suppressed by materiality.
3. Recompute passed from retained findings: no Error means pass, even if a custom check supplied another passed value. Preserve all numeric comparisons.
4. Aggregate passed/failed check counts and retained Info/Warning/Error counts.

run_model uses supplied results directly, or invokes a new default Evaluator when results are absent. It does not recompute supplied financial values or prove their model/content hash. merge appends checks and keeps the left suite's name, description and configuration; duplicate check IDs are not rejected. CheckSuiteSpec resolves built-ins first and formulas second, validating config at resolution but deferring individual execution errors.

## Limitations and alternatives

A passing empty suite or a suite containing only skipped identities is possible. There is no separate checked-period count or complete/incomplete verdict. Filtering can hide advisory skip findings, although recorded comparisons survive. Direct CheckComparison construction is not independently validated; the accounting producers enforce finite values before publishing them.

Using a strict tolerance override of 0 changes exact-equality expectations; choosing a huge materiality threshold cannot excuse an Error. Scale conventions and business materiality remain caller choices.

Result provenance, unique check IDs and explicit coverage status require reconciliation under the accepted contract.

## Evidence

[Suite execution](../../../../finstack-quant/statements/src/checks/suite.rs), [comparison/config types](../../../../finstack-quant/statements/src/checks/types.rs), [input helpers](../../../../finstack-quant/statements/src/checks/helpers.rs), [suite tests](../../../../finstack-quant/statements/tests/checks/suite_tests.rs), [type tests](../../../../finstack-quant/statements/tests/checks/types_tests.rs) and [accounting regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) provide evidence. Inspected tests confirm supplied-result use, stale-unit rejection, retained-finding counts and that materiality cannot turn an Error identity into a pass.

Filtered passed counts do not independently certify accounting correctness.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
