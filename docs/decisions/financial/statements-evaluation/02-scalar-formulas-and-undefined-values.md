# STATEMENTS-EVALUATION-002 — Scalar formulas, boolean control flow and undefined values

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice, input domain and conventions

The DSL evaluates floating-point expressions with operation-specific undefined handling. Arithmetic values preserve the declared node units established by semantic analysis; decimal rates remain decimal, and no implicit sign or currency conversion occurs in these kernels. The implementation rationale is direct compositional expression evaluation. Adoption is pending for individual numerical tolerances and missing-data operations, subject to accepted [ADR 0009](../conventions/0009-missing-and-invalid-data.md).

## Algorithm and controls

- Addition, subtraction, multiplication, negation, absolute value, floor/ceiling, power, logarithms, exponential and square root use ordinary `f64` behavior. Division/remainder by exact zero return NaN and a division diagnostic when a node identity is available. Other domain failures may propagate NaN/infinity until the result layer records them.
- Equality uses `abs(a-b) <= 1e-10`; inequality uses `> 1e-10`. This is an absolute tolerance, independent of units or value scale. Both comparisons are false for a NaN difference. Ordered comparisons use normal IEEE comparisons. `sign` preserves NaN and maps infinities to their sign.
- Boolean truth means finite and nonzero. AND/OR and `if` evaluate only the required branch; boolean consumers retain a nonfinite-operand flag for masking diagnostics. `not(nonfinite)` returns true under this truth rule. `is_missing` tests nonfiniteness, not merely absence of a map key.
- `coalesce` lazily returns the first finite evaluated argument, but does not catch an evaluation error. N-ary `sum`/`mean` evaluate all arguments, discard nonfinite values with a skip warning and use compensated summation; no finite arguments gives NaN. N-ary `min`/`max` fold left to right using the shared comparison semantics: NaN behavior can depend on argument order and right-hand ties win.
- `round(value, digits=0)` requires a finite integer precision fitting `i32`, using the integer tolerance before conversion, and delegates to half-away-from-zero rounding. `clamp` returns NaN for any NaN operand or reversed bounds; otherwise it applies `min(value,hi).max(lo)`, including ordinary clamping of infinities.
- `annualize(value, periods_per_year=current_frequency)` multiplies by the frequency; a nonpositive frequency errors, while the local check does not categorically reject positive infinity. `annualize_rate` requires rate, frequency and a numeric compounding flag: any NaN argument first produces NaN, a nonpositive frequency errors, flag zero gives `r*p`, and any other non-NaN flag gives `(1+r)^p-1`. A nonfinite compounded result becomes NaN with tracing diagnostics. The flag is not restricted to finite Boolean values.

## Selection, failures and alternatives

Function dispatch is an explicit match over the parsed function enum. Standard handlers enforce their arity; cumulative and full-history statistics have the separate surplus-argument discrepancy in [record 04](04-historical-statistics-and-ewm.md). Formula evaluation errors propagate with active-node context; `coalesce` is not an error-recovery operator. Finite-only sums and ordinary arithmetic intentionally differ, so a blanket claim that all missing inputs fail or all missing inputs are skipped would be inaccurate.

Alternatives include exact equality, scale-aware comparisons, explicit nullable values, strict finite-only arithmetic and separate Boolean types. None is adopted here. The absolute equality tolerance can be economically material for scaled amounts or tiny rates; its meaning depends on the input scale. The existing source documents the min/max choice; this audit does not classify that already documented behavior as a new defect.

## Evidence

Owning sources: [formula evaluator](../../../../finstack-quant/statements/src/evaluator/formula.rs), [dispatch](../../../../finstack-quant/statements/src/evaluator/formula_dispatch.rs), [helpers](../../../../finstack-quant/statements/src/evaluator/formula_helpers.rs). Located [NaN tests](../../../../finstack-quant/statements/tests/functions/nan_handling_tests.rs) and [min/max tests](../../../../finstack-quant/statements/tests/functions/min_max_tests.rs) characterize propagation and operand order. These establish intended program behavior, not universal financial semantics. Numerical extremes, infinite frequency and nonfinite compounding flags lie outside the established qualification evidence. No external accounting or market reference was found establishing this complete scalar policy; no tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
