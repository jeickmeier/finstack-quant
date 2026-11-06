# STATEMENTS-CONTRACTS-004: Formula grammar, bounded parsing and function dispatch

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

Formula text is parsed by nom into StmtExpr, then recursively lowered to the core Expr representation. This layer chooses syntax, supported functions and argument counts; the statements evaluator owns the numerical temporal/statistical implementations.

Identifiers begin with an alphabetic Unicode character or underscore and continue with alphanumeric characters, underscores or dots. Capital-structure identifiers have exactly cs.component.instrument_or_total form, with alphanumeric/underscore component and alphanumeric/underscore/hyphen target. Numeric literals must be finite.

## Algorithm and controls

The complete input must parse. Precedence is logical or, logical and, keyword not, one comparison, addition/subtraction, multiplication/division/modulo, then symbolic unary ! and minus and primary expressions. Arithmetic chains fold left. Parentheses and if(condition,then,else) recurse; a second unparenthesized comparison is not accepted. There is no exponentiation operator; pow is a function.

Each parse resets thread-local limits: maximum recursive parse depth 64 and maximum charged primary terms 256. Errors report line/column and distinguish nesting and term-budget violations. The primary-term counter is a parser-work bound, not a general persisted AST size specification.

Lowering preserves each operator and branch in core Expr. Capital-structure components are limited to interest_expense, interest_expense_cash, interest_expense_pik, interest_income, principal_payment, debt_balance, fees and accrued_interest. Unknown components/functions error. lead is deliberately unavailable.

| Function group | Accepted argument count at compilation |
|---|---|
| sum, mean, min, max | At least 1 |
| abs, sign, floor, ceil, ln, exp, log10, sqrt, is_missing, ttm, ytd, qtd | Exactly 1 |
| pow, lag, shift, ewm_mean, quantile, fiscal_ytd | Exactly 2 |
| clamp, annualize_rate | Exactly 3 |
| round, annualize, growth_rate, diff, pct_change, rank | 1 or 2 |
| rolling_mean/sum/std/var/median/min/max/count, ewm_std, ewm_var | 2 or 3 |
| cumsum, cumprod, cummin, cummax, std, var, median | At least 1, with no upper bound in this compiler |

StmtExpr display inserts parentheses around a weaker-precedence left child and same-or-weaker right child, retaining arithmetic grouping when printed and reparsed. Public AST constructors and compile accept caller-built trees, bypassing text-parser finite-literal and resource limits.

## Limitations and alternatives

The last table row disagrees with the one-argument DSL reference. Extra arguments are compiled and may affect dimension/dependency analysis although the numerical operation is a one-series calculation; the evaluator's precise consumption is separate. This is an observed contract discrepancy requiring reconciliation, not a newly supported multi-series variant.

parse_and_compile does not establish that references exist or units agree. Manual ASTs can contain non-finite literals and unbounded depth. A single shared function-signature table could align compilation, documentation and evaluation, but adopting it would be implementation work.

## Evidence

[Parser](../../../../finstack-quant/statements/src/dsl/parser.rs), [compiler](../../../../finstack-quant/statements/src/dsl/compiler.rs), [AST formatting](../../../../finstack-quant/statements/src/dsl/ast.rs), [DSL reference](../../../../finstack-quant/statements/src/dsl/mod.rs), [DSL tests](../../../../finstack-quant/statements/tests/dsl/dsl_tests.rs), and [DSL property tests](../../../../finstack-quant/statements/tests/dsl/proptest_dsl.rs) are the evidence. Parser and module inline regressions exercise the 256-term boundary and compilation on a 512 KiB thread stack. AST round-trip tests support grouping preservation; they are representation tests, not financial-method validation.

Successful compilation is not numerical validation of the selected evaluator operation.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
