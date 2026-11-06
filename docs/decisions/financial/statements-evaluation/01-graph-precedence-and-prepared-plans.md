# STATEMENTS-EVALUATION-001 — Dependency graph, cell precedence and prepared evaluation

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice, scope and rationale

Statement evaluation is an ordered period-by-period execution of a current-period dependency DAG, with separate historical storage and explicit cell-source precedence. Preparation parses formulas, runs semantic analysis, resolves dependencies and infers units before execution. This separates reusable expression structure from caller-supplied numeric values; it does not establish adoption of every precedence or dependency rule. Input validation and expression-language contracts are also covered by the adjacent [statements-contracts inventory](../statements-contracts/README.md).

## Algorithm and selection

1. Clone/prepare the model, cache compiled formula and `where` ASTs, and build a graph from current-period dependencies. Formula and mask dependencies are retained even for branches or cells that explicit inputs will override. Unresolved ordinary identifiers fail with nearest-name diagnostics; `cs.*` references use dedicated capital-structure lookup. An iterative depth-first traversal detects cycles and produces topological evaluation order.
2. Lag/shift dependencies depend on the offset: a literal nonzero offset removes the first expression's current-period edge; zero or dynamic offsets retain it. Dependencies of the offset expression itself remain. Historical recurrence is therefore distinct from a simultaneous cycle. Forecast correlation peers are **not** graph edges; [record 08](08-correlated-shocks.md) describes the resulting ordering requirement.
3. For each model period, create a context with earlier rows, evaluate nodes in graph order, record numeric values and sources, then advance capital state. Evaluate `where` first: zero or nonfinite masks produce zero and `WhereMasked`, including when an explicit value exists. Otherwise use visible explicit value, then forecast for an effectively nonactual cell, then formula. An actual cell unavailable at the evaluation cutoff becomes forecast eligible.
4. Availability is visible on or after its date; omitted availability uses the exclusive period end. `evaluate_with_market` supplies the as-of cutoff. Ordinary/prepared evaluation has no availability cutoff and supplies an empty market; capital structures use the first period start as the fallback as-of. Missing required market data remains an error.
5. Numeric results, including nonfinite formula results, are stored; nonfinite values add diagnostics rather than aborting the entire evaluation. Ordinary finalization rebuilds units/Money views and runs an optional check suite. Monte Carlo follows a separate aggregation path and does not run that suite.

## Prepared-plan controls and history conventions

The fingerprint includes period IDs/bounds/actual flags, node order/identity/type, formulas/masks/availability, forecast parameters and declared value types. Explicit numeric values are excluded so they can change between prepared evaluations; each evaluation revalidates values and exact node identity/fingerprint. The fingerprint does not hash every model field: capital-structure and metadata objects can be read live. It is a structural execution-plan check, not a complete model/provenance digest.

History stores `Option<f64>` columns, distinguishing absent from present NaN. Current writes overlay a stored same-period value; collectors exclude future periods and invalidate caches on writes or capital snapshot replacement. Duplicate period pushes replace the previous row, and row width is normalized to the column layout. Historical-only names can obtain columns in the named context constructor. No automatic imputation or historical restatement occurs.

## Alternatives and limitations

A lazy dependency graph could avoid irrelevant branches but would change cycle/missing-name acceptance. A simultaneous solve could support algebraic cycles but is not implemented. Explicit-mask precedence, availability equality and prepared-plan mutation scope remain observed API behavior pending adoption. Do not describe a fingerprint match as proof that all financial inputs are identical. The accepted per-operation undefined-data policy in [ADR 0009](../conventions/0009-missing-and-invalid-data.md) remains applicable; storing undefined values is not a license to suppress invalid configuration errors.

## Evidence

Owning sources: [engine](../../../../finstack-quant/statements/src/evaluator/engine.rs), [DAG](../../../../finstack-quant/statements/src/evaluator/dag.rs), [precedence](../../../../finstack-quant/statements/src/evaluator/precedence.rs), [context](../../../../finstack-quant/statements/src/evaluator/context.rs), [history](../../../../finstack-quant/statements/src/evaluator/period_history.rs). Delegated dependency classification is in [formula utilities](../../../../finstack-quant/statements/src/utils/formula.rs). Located [engine regressions](../../../../finstack-quant/statements/tests/finance_engine_regressions.rs) cover revalidation and prepared identity/timeline mutations; [evaluator tests](../../../../finstack-quant/statements/tests/evaluator_tests.rs) cover ordinary execution. These are contract/regression evidence, not accounting-standard certification. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
