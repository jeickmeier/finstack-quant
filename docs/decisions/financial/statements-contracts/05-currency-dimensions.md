# STATEMENTS-CONTRACTS-005: Formula units, unknown dimensions and capital-structure currencies

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Source baseline and audit date: [slice README](README.md).

## Choice and input domain

Model-specific inference uses three states: Unknown, Scalar and Monetary(currency). Unknown is unresolved information, not an implicit dimensionless value. Monetary arithmetic performs no FX conversion. Explicit observations seed inference, and formula nodes are resolved iteratively.

Capital-structure totals use the reporting currency; a named instrument reference uses that instrument's native notional currency. Reporting currency selection is explicit capital_structure.reporting_currency, then valid model meta.currency, then the unique currency across declared monetary nodes and instrument notionals. Zero or multiple candidates yield None.

## Algorithm and controls

- Addition, subtraction, comparison and if branches require matching known dimensions. A monetary/scalar mix, including comparison with a bare numeric zero, errors. Comparison returns Scalar after compatibility validation.
- Multiplication allows Scalar×Scalar or Scalar×Monetary in either order; Monetary×Monetary errors. Division permits Monetary/Scalar, Scalar/Scalar and same-currency Monetary/Monetary (Scalar); cross-currency and Scalar/Monetary division error.
- Negation preserves units. Logical operations and modulo require scalar arguments. Unknown propagates, but a known monetary operand still causes failure in a scalar-only operation.
- sum/mean/min/max/coalesce/clamp require compatible value dimensions. quantile preserves the first argument and requires scalar level; rank returns Scalar and requires scalar direction.
- abs, lag/shift, cumulative sum/min/max, diff, rolling level/std functions, EWM level/std, median/std, ttm/ytd/qtd/fiscal_ytd, annualize and rounding preserve the first argument's units; auxiliary controls must be scalar.
- sign, percentage change, growth_rate, rolling_count and is_missing produce Scalar. cumprod, annualize_rate, pow and logarithmic/exponential/square-root functions require scalar inputs.
- Variance of a Scalar remains Scalar. Variance of Monetary becomes Unknown because squared monetary units are not represented. The compiler does not pretend it is either money or dimensionless.
- A where clause must infer Scalar; an ordinary formula can remain Unknown. A declared formula type is checked when the formula's type can be resolved.

## Limitations and alternatives

Same-currency typing does not prove compatible scale, reporting period, accounting classification or percentage-versus-count interpretation. Registry UnitType metadata does not become a semantic type; numerical ratios and percentages are both Scalar. Unknown variance units can propagate through further operations without a squared-currency model.

A richer dimensional algebra could represent currency powers and distinguish rates, percentages, amounts and quantities, at the cost of more API complexity. Requiring all published outputs to resolve would be a narrower alternative. Neither is implemented.

## Evidence

[Dimension inference](../../../../finstack-quant/statements/src/dsl/compiler.rs), [model currency resolution](../../../../finstack-quant/statements/src/types/model.rs), [dimension regressions](../../../../finstack-quant/statements/tests/finance_dimension_regressions.rs) and [accounting unit regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) provide the evidence. Inspected tests reject currency laundering through nonlinear functions and monetary window controls, preserve currency in series transforms, and explicitly expect None for squared-money variance. These are type-contract tests, not proof that an economically correct formula was selected.

Native instrument currencies and reporting totals remain distinct; formula typing performs no implicit conversion.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
