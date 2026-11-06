# STATEMENTS-CONTRACTS-014: Numeric check tolerances, filtering and suite verdicts

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Source baseline and audit date: [slice README](README.md).

## Choice and input domain

CheckSuite runs an ordered list of Check implementations over a model and supplied StatementResult. It uses explicit findings to determine verdicts and preserves numeric comparisons independently of report filtering.

Default configuration is absolute tolerance 0.01, relative tolerance 1e-9, materiality threshold 0 and minimum severity Info. Numeric configuration and per-identity overrides must be finite and nonnegative. Tolerances are in the checked line's base units; relative tolerance is a fraction, while displayed Materiality.relative_pct is percent.

## Algorithm and controls

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

A result-provenance contract, unique check IDs and explicit coverage status would address different risks from numeric tolerances. These are possible additions, not implemented fallback behavior.

## Evidence

[Suite execution](../../../../finstack-quant/statements/src/checks/suite.rs), [comparison/config types](../../../../finstack-quant/statements/src/checks/types.rs), [input helpers](../../../../finstack-quant/statements/src/checks/helpers.rs), [suite tests](../../../../finstack-quant/statements/tests/checks/suite_tests.rs), [type tests](../../../../finstack-quant/statements/tests/checks/types_tests.rs) and [accounting regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) provide evidence. Inspected tests confirm supplied-result use, stale-unit rejection, retained-finding counts and that materiality cannot turn an Error identity into a pass.

Filtered passed counts do not independently certify accounting correctness.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
