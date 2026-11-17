# STATEMENTS-CONTRACTS-004: Formula grammar, bounded parsing and function dispatch

**Decision status:** Accepted for the full formula grammar, bounded parsing and function dispatch contract (2026-11-15).

**Implementation status:** Partly reconciled (2026-11-16). One signature table drives the compiler, the AST preflight and the evaluator's arity checks; the seven cumulative and full-history functions take exactly one argument; rendering writes negation as `!` and parenthesizes nested comparisons; identifiers need nonempty dot-separated segments; a negative literal offset is rejected as a future read; and caller-built trees are validated iteratively before lowering (256 terms, 320 levels). The typed `cs` selectors and scopes of clause 2 remain with capital reporting. The audited sections below describe revision `7e82fd03ad`.

**Validation status:** Grammar, grouping round-trip, arity, untaken-branch, future-read and boundary qualification are in the [DSL tests](../../../../finstack-quant/statements/tests/dsl/dsl_tests.rs). Typed `cs` selector qualification remains pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for formula grammar, function dispatch and AST validation

1. Parse the complete input with a case-sensitive grammar and finite numeric literals. Arithmetic chains associate left. Symbolic `!` binds at unary precedence. Keyword `not` binds outside a comparison. Require explicit parentheses around nested comparisons. Canonical rendering must preserve meaning and grouping when parsed again. For example, `!a>b` groups as `(!a) > b`. The expression `not a > b` groups as `not (a > b)`.
2. Use shared identifier and namespace rules with valid nonempty dot-separated segments. Apply the reserved-name rules in authoring and formula references. The `cs` namespace uses the typed capital reporting components and selectors, including instrument, native-currency total and consolidated scopes. [Authoring and identifier rules](02-authoring-and-availability.md#accepted-decision-for-authoring-imports-and-observation-availability) · [Capital reporting and selectors](../statements-capital/09-reporting-and-fx.md#accepted-decision-for-capital-reporting-and-fx).
3. Use canonical function signatures across parsing, compilation, evaluation and the documented host interfaces. Each of `cumsum`, `cumprod`, `cummin`, `cummax`, `std`, `var` and `median` requires exactly one argument. Reject invalid expressions in any branch, including branches that evaluation would skip. Reject future-period reads.
4. Limit text parsing to 64 levels of nesting and 256 charged terms. Safely preflight caller-supplied ASTs before recursive processing, including validation of their shape, finite literals and resource use. The runnable-model boundary adds closed references and supported units. [Runnable model validation](01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness).

## Audited input domain

Formula text is parsed by nom into StmtExpr, then recursively lowered to the core Expr representation. This layer chooses syntax, supported functions and argument counts; the statements evaluator owns the numerical temporal/statistical implementations.

Identifiers begin with an alphabetic Unicode character or underscore and continue with alphanumeric characters, underscores or dots. Capital-structure identifiers have exactly cs.component.instrument_or_total form, with alphanumeric/underscore component and alphanumeric/underscore/hyphen target. Numeric literals must be finite.

## Audited algorithm and controls

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

The audited `parse_and_compile` checks syntax and lowering. Model admission owns reference closure and unit resolution. Caller-built ASTs can bypass the finite-literal and resource checks in the audited parser.

## Evidence

[Parser](../../../../finstack-quant/statements/src/dsl/parser.rs), [compiler](../../../../finstack-quant/statements/src/dsl/compiler.rs), [AST formatting](../../../../finstack-quant/statements/src/dsl/ast.rs), [DSL reference](../../../../finstack-quant/statements/src/dsl/mod.rs), [DSL tests](../../../../finstack-quant/statements/tests/dsl/dsl_tests.rs), and [DSL property tests](../../../../finstack-quant/statements/tests/dsl/proptest_dsl.rs) are the evidence. Parser and module inline regressions exercise the 256-term boundary and compilation on a 512 KiB thread stack. AST round-trip tests support grouping preservation; they are representation tests, not financial-method validation.

Successful compilation is not numerical validation of the selected evaluator operation.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
