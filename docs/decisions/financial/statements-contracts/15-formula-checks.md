# STATEMENTS-CONTRACTS-015: Formula checks, residual tolerances and insufficient-data classification

**Decision status:** Accepted for the full Statements formula-check contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-17). A formula check states its `mode` (`predicate` or `residual`) and its `scope` (all, actual or forecast periods); nothing is inferred from the tolerance. A predicate must be a Boolean formula and takes no tolerance; a residual must be a scalar or monetary formula and is compared with zero under an absolute tolerance in its own units, zero when omitted. The formula and the optional Boolean `eligibility` condition are validated over the whole AST (every branch, identifier, unit and capital scope) before any period runs. Each period evaluates lazily over the typed cells, history and capital state of the verified run: `coalesce` supplies the fallback for a cell without a value and an untaken branch does not consume its inputs. A false eligibility condition leaves the period out of scope; an eligibility condition or formula that needed an observation without a value is a coverage finding and the check is incomplete; division by zero, a domain error or an overflow is an execution error at any severity. `evaluated` counts the in-scope eligible periods. The residual and its tolerance are recorded as floats in the residual's units; typed comparison values belong to the comparison record's remainder. The sections below the accepted decision describe the audited implementation.

**Validation status:** Qualified by an explicit fallback over a missing cell, an untaken and a taken branch with a missing input, a negative square root as an execution error, false and unavailable eligibility and a scalar eligibility condition, each period scope, a scope with no eligible period, a numeric predicate, a Boolean residual, a tolerance on a predicate, negative and non-finite tolerances, an unknown node, an exact residual, a `lag` before the first period and a division by zero.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for formula checks and coverage

1. Select the check mode and the scope of actual periods, forecast periods or both explicitly. In Predicate mode, require a Boolean formula. True passes and false records a breach at the configured severity. In Residual mode, compare a signed scalar or monetary result against typed zero. Use an absolute tolerance with matching units and, for monetary residuals, matching currency. The tolerance defaults to typed zero. A residual passes when its absolute value is at most the tolerance. Retain actual, expected, signed difference and tolerance as typed comparison details. [Numerical comparison and suite outcomes](14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).
2. Validate the full AST, including every branch, all identifiers and units, and capital reporting scopes. Evaluate using current values, history and capital state from the verified evaluation run. Consume observations lazily on the paths evaluation takes. Use explicit `coalesce` for missing-data fallback. [Formula validation](04-dsl-syntax-and-lowering.md#accepted-decision-for-formula-grammar-function-dispatch-and-ast-validation) · [Capital reporting scopes](../statements-capital/09-reporting-and-fx.md#accepted-decision-for-capital-reporting-and-fx).
3. An optional Boolean eligibility condition can mark a period out of scope when false. An unavailable eligibility condition or a needed observation that is missing or unavailable makes the comparison incomplete. Invalid consumed inputs, division by zero, domain errors and arithmetic overflow produce execution errors even for an advisory check. [Data-quality validity and coverage](13-data-quality-checks.md#accepted-decision-for-data-quality-coverage-and-validity).

## Audited input domain

FormulaCheckSpec reuses the canonical statements parser, unit inference and evaluator. It runs over every model period and supports either a numeric residual against zero (tolerance supplied) or finite numeric truthiness (no tolerance). Check severity is caller-selected; failed Warning checks remain advisory.

## Audited algorithm and controls

1. Validate config and optional finite nonnegative tolerance. Parse the formula, reconcile all available model/result unit metadata and capital-structure currencies, infer dimensions and compile. Dimension errors are fatal; the final expression is not required to have Scalar type.
2. Build complete model-aligned node history and period capital-structure snapshots from supplied results.
3. Extract syntactic direct dependencies, excluding the first argument of lag/shift with a nonzero Literal offset. For known ordinary node dependencies, require a finite current cell before evaluation. If any are missing/non-finite, emit an insufficient-data Warning and skip the period. Unknown identifiers are left to evaluation and error.
4. Populate current values and cashflow snapshot, evaluate the canonical expression, and inspect warnings/operand state.
5. When there is no DivisionByZero warning and either the result is non-finite or a non-finite operand was consumed, emit an insufficient-data Warning and skip. Division by zero over finite inputs is instead evaluated as a failed check.
6. A finite residual passes when abs(value) <= the explicit tolerance. Without tolerance, any finite nonzero value passes. No default numeric tolerance is substituted for the boolean mode.
7. Finite residual checks record actual=value, expected=0 and tolerance. Boolean checks and non-finite results produce no numeric comparison. A failed check uses its configured severity and replaces only {period} in the message template. passed is false only when findings contain Error severity.

## Limitations and alternatives

Preflight examines all syntactic direct dependencies, including coalesce alternatives and branches not taken by if. Thus a formula intended to recover a missing current observation can be skipped before its fallback executes. Missing history is advisory, but the same post-evaluation rule also classifies non-finite domain/overflow results without DivisionByZero as insufficient data; it does not prove that the cause was missing input.

No formula-specific period scope, materiality calculation, result model-hash validation or proof of complete history is added here. Tolerance-bearing monetary residuals are allowed, but a monetary nonzero expression without tolerance is also accepted as truthy. Configure the intended residual/predicate deliberately.

The accepted check contract uses lazy observation consumption and distinguishes a breached check, incomplete comparison and execution error. Implementation awaits reconciliation.

## Evidence

[Formula check implementation](../../../../finstack-quant/statements/src/checks/formula.rs), [dependency helper](../../../../finstack-quant/statements/src/utils/formula.rs), [formula check tests](../../../../finstack-quant/statements/tests/checks/formula_check_tests.rs) and [unit regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) are the evidence. Inspected tests cover missing current cells, non-finite inputs, lag before history, capital-structure references and division by zero over finite inputs. They demonstrate canonical-engine integration, not an independent financial model comparison.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
