# STATEMENTS-CONTRACTS-015: Formula checks, residual tolerances and insufficient-data classification

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

FormulaCheckSpec reuses the canonical statements parser, unit inference and evaluator. It runs over every model period and supports either a numeric residual against zero (tolerance supplied) or finite numeric truthiness (no tolerance). Check severity is caller-selected; failed Warning checks remain advisory.

## Algorithm and controls

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

Alternative policies could defer missing handling to a lazy expression evaluator or distinguish missing-data, invalid-domain and arithmetic-overflow failures. Such choices require tests that preserve the difference between a financial failure and an unjudgeable period.

## Evidence

[Formula check implementation](../../../../finstack-quant/statements/src/checks/formula.rs), [dependency helper](../../../../finstack-quant/statements/src/utils/formula.rs), [formula check tests](../../../../finstack-quant/statements/tests/checks/formula_check_tests.rs) and [unit regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) are the evidence. Inspected tests cover missing current cells, non-finite inputs, lag before history, capital-structure references and division by zero over finite inputs. They demonstrate canonical-engine integration, not an independent financial model comparison.

The described advisory behavior is observed implementation, not approval of its failure classification.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
