# CORE-NUMERICS-22: Scalar expression DAG evaluation and missing-value semantics

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`expr/{ast,context,dag,eval,eval_functions}.rs` and scalar helpers in `math/mod.rs`: elementwise expressions over named f64 columns, including literals, arithmetic, comparison/logic, conditionals and twelve scalar functions. Period/window/cumulative/aggregate and capital-structure functions share AST vocabulary but require the statements evaluator and are rejected here.

## Choice and rationale

Compile an iterative postorder DAG, deduplicate structurally identical nodes, and evaluate each once per call. This avoids repeated subtree work and recursive compile/drop on rejected input. The DAG key preserves operand order and literal f64 bit patterns; it performs no algebraic simplification.

## Algorithm

1. Visit the AST left-to-right using explicit Visit/Finish stacks. Reject depth≥512 and unsupported nodes before descending. A rejected owned tree is drained iteratively. Deduplicate by operation plus dependency node IDs; literal keys use bits, distinguishing signed zeros/NaN payloads.
2. Context construction rejects duplicate names and records exact case-sensitive insertion positions. On evaluation set row count to first supplied column length (zero if none), allocate one row-length arena segment per DAG node, and evaluate all dependencies in topological order.
3. Named-column lookup errors if absent from context or data. Short columns are NaN-padded; long columns truncate. Literals broadcast. Arithmetic uses f64, except division by either signed zero returns NaN. Comparisons use exact IEEE comparisons and return 0/1. Logic regards every nonzero as true, so NaN is true for `and`/`or` and false for `not`.
4. Both conditional branches are evaluated before selection. A NaN condition produces NaN; otherwise nonzero chooses then. Thus missing columns/errors in an unselected branch still fail the full evaluation.
5. Scalar abs/sign/pow/floor/ceil/log/exp/sqrt use elementwise kernels. `is_missing` is 1 for any nonfinite value. Clamp propagates any NaN and inverted bounds as NaN. Missing function arguments generate NaN output; no general arity check is performed at compilation.
6. Round defaults digits to zero. A supplied digits series must be bit-identical throughout or errors; a nonfinite, nonintegral or out-of-i32-range constant generates NaN. `round_half_away` scales by 10^digits, rounds ties away from zero, unscales, and returns original x if scaling overflows. Nonfinite x passes through.

## Selection, controls and conventions

The scalar compiler is explicitly selected; it does not route unsupported nodes automatically. No monetary scale, percentage conversion, date alignment or unit checking occurs. Inputs are already-aligned columns. Function labels/serde names are centralized in the AST macro.

## Limitations, failure and fallback

Comparison/logical missing-data semantics differ from conditional and arithmetic propagation. Empty columns produce empty result even for literals. Extreme negative digits can underflow the decimal scale to zero and produce NaN after division. Arena element multiplication is checked, but there is no configurable memory budget. Eager branch evaluation is material for error behavior.

## Alternatives and tradeoffs

The statements evaluator supplies separate period-aware behavior. Lazy conditional evaluation, three-valued logic, strict column lengths and dimensional analysis are not supported options in this compiler. DAG reuse reduces computation at the cost of retaining one full result column for each unique node.

## Evidence

- [DAG](../../../../finstack-quant/core/src/expr/dag.rs), [evaluator](../../../../finstack-quant/core/src/expr/eval.rs), [function kernels](../../../../finstack-quant/core/src/expr/eval_functions.rs), [scalar rounding](../../../../finstack-quant/core/src/math/mod.rs), [expression tests](../../../../finstack-quant/core/tests/expr/eval.rs), [function tests](../../../../finstack-quant/core/tests/expr/functions.rs).
- `dag_builder_deduplicates_structurally_identical_nodes` and `public_compilation_rejects_deep_trees_on_a_small_stack` exercise planning and rejection. `round_default_digits_ties_away_from_zero`, `binary_op_missing_tail_yields_nan` and `binop_division_by_zero_returns_nan` establish explicit scalar conventions. These are language-contract tests, not financial formula validation. No external mathematical standard is claimed for these language semantics.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
