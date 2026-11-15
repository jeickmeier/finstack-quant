# STATEMENTS-CONTRACTS-005: Formula units, unknown dimensions and capital-structure currencies

**Decision status:** Accepted for the full Statements formula units and currency contract (2026-11-15).

**Implementation status:** Reconciliation pending. The audited path can retain unresolved units and infer a capital reporting currency from model metadata or candidate currencies.

**Validation status:** Existing source and regression evidence mapped. Independent qualification of units, numerical boundaries and reporting scopes remains pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for formula units and currency

1. Resolve every input, intermediate expression, branch and control before execution. The supported dimensions are `Scalar` and `Monetary(currency)`. A declared type can resolve an empty input series. Declared formula units must agree with inference. Reject unresolved and unsupported dimensions, including monetary variance. [Runnable model validation](01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness).
2. Addition, subtraction, comparison and selection require matching dimensions and currencies. Comparisons return Scalar. Branch conditions require Scalar, and scalar controls use their documented units. Multiplication allows two scalars or scalar scaling of money. Division allows scalar by scalar, money by scalar and a same-currency money ratio that returns Scalar. Reject other monetary products and quotients. Compatible aggregates and level transforms preserve their input unit, including monetary standard deviation. Counts, ranks, signs, missingness indicators, percentage changes and growth rates return Scalar. Logical, modulo, cumulative product, rate annualization and nonlinear mathematical operations require Scalar. [Canonical function signatures](04-dsl-syntax-and-lowering.md#accepted-decision-for-formula-grammar-function-dispatch-and-ast-validation).
3. Express monetary values in currency-tagged major units and rates and ratios as decimals. A zero used as money must carry a currency. Use exact monetary arithmetic for contractual amounts and ledgers under the shared monetary rules. When a forecast requires floating-point calculation, make conversion at the forecast boundary explicit and check representability and rounding when returning to money. [Monetary arithmetic and conversion](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
4. Resolve capital references using the accepted instrument-native, native-currency total and consolidated reporting scopes. Consolidation uses the run's selected reporting currency and its explicit period-close FX policy. A required missing quote makes consolidated reporting unavailable. Formula typing and arithmetic perform no implicit FX. [Capital reporting and FX](../statements-capital/09-reporting-and-fx.md#accepted-decision-for-capital-reporting-and-fx).

## Audited input domain

Model-specific inference uses three states: Unknown, Scalar and Monetary(currency). Unknown is unresolved information, not an implicit dimensionless value. Monetary arithmetic performs no FX conversion. Explicit observations seed inference, and formula nodes are resolved iteratively.

Capital-structure totals use the reporting currency; a named instrument reference uses that instrument's native notional currency. Reporting currency selection is explicit capital_structure.reporting_currency, then valid model meta.currency, then the unique currency across declared monetary nodes and instrument notionals. Zero or multiple candidates yield None.

## Audited algorithm and controls

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

The accepted contract requires resolution of supported units before execution. The audited implementation can allow unknown units to propagate.

## Evidence

[Dimension inference](../../../../finstack-quant/statements/src/dsl/compiler.rs), [model currency resolution](../../../../finstack-quant/statements/src/types/model.rs), [dimension regressions](../../../../finstack-quant/statements/tests/finance_dimension_regressions.rs) and [accounting unit regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) provide the evidence. Inspected tests reject currency laundering through nonlinear functions and monetary window controls, preserve currency in series transforms, and explicitly expect None for squared-money variance. These are type-contract tests, not proof that an economically correct formula was selected.

Native instrument currencies and reporting totals remain distinct; formula typing performs no implicit conversion.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
