## `core::expr` — Deterministic Scalar Expression Engine

The `core::expr` module is a small, deterministic expression engine used throughout
Finstack Quant for **time–series style** computations (lags, diffs, rolling windows, EWMs,
etc.) over plain `f64` slices. It provides:

- **Deterministic**: stable results across platforms and runs.
- **Allocation‑aware evaluation**: scratch arenas and an arena-style executor reduce per-node `Vec` allocations.
- **DAG‑optimized**: shared sub‑expressions across many formulas are evaluated once per `eval()` call.
- **Embedding‑friendly**: no Polars dependency, `SimpleContext` handles column resolution and can be constructed from any ordered iterator of column names.

> **Note (cache removal)**: an earlier version persisted intermediate node
> results across `eval()` calls in an LRU cache keyed on `(dag_node_id, len)`.
> Because the key carried no input fingerprint, re-evaluating the same
> `CompiledExpr` on different same-length data returned stale results
> . The cache
> was removed; `CompiledExpr::with_cache` and `EvalOpts.cache_budget_mb`
> remain as no-ops for API compatibility.

Semantics note: when input columns have mismatched lengths, missing tail values
propagate as `NaN` instead of being silently zero-filled. Adjusted EWM mean also
uses the standard weighted numerator/denominator form (`adjust=true`) rather than
normalizing a recursive EMA after the fact.

Typical workflow:

- **Build an AST** with `Expr`, `ExprNode`, `BinOp`, `UnaryOp`, and `Function`.
- **Compile** it into a `CompiledExpr` (optionally with a DAG `ExecutionPlan`).
- **Evaluate** it against a `SimpleContext` and a slice of numeric columns.

Serde note: inbound expression types (`Expr`, `ExprNode`, `EvalOpts`,
`SimpleContext`) deserialize strictly (`deny_unknown_fields`). `EvalOpts.plan`
is **not** part of the wire format (`#[serde(skip)]`): execution plans can only
be attached in-process, so a deserialized `EvalOpts` cannot inject an arbitrary
plan for `eval()` to execute.

---

### Public Surface

The `mod.rs` re‑exports the small public API:

- **AST / operations**
  - `Expr`, `ExprNode`
  - `BinOp`, `UnaryOp`
  - `Function`
  - `EvaluationResult`
- **Context**
  - `SimpleContext`
- **Evaluator**
  - `CompiledExpr`
  - `EvalOpts`

The Polars `Series` API is not exposed here; callers work with simple slices (`&[f64]`).

---

## Module Structure

- **`ast.rs`**: expression data model and function registry
  - `Expr`: top‑level expression with optional `id: Option<u64>` for DAG/caching identification.
  - `ExprNode`: core variants:
    - **Columns**: `Column(String)`
    - **Literals**: `Literal(f64)`
    - **Function calls**: `Call(Function, Vec<Expr>)`
    - **Operators**: `BinOp`, `UnaryOp`
    - **Conditionals**: `IfThenElse { condition, then_expr, else_expr }`
  - `Function`: enum of all supported scalar functions (lags, diffs, rolling, EWMs, ranking, and a few financial utilities).
  - `EvaluationResult`: `{ values: Vec<f64>, metadata: config::ResultsMeta }`.
  - **Hash/eq semantics**: `Expr` implements `Hash` / `Eq` **ignoring** `id` so structurally identical trees deduplicate in the DAG.

- **`context.rs`**: column resolution
  - `SimpleContext`: name→index map for small, in‑memory frames.

- **`dag.rs`**: DAG planning and execution plans
  - `DagNode { id, expr, dependencies, ref_count, cost }`.
  - `ExecutionPlan { nodes, roots, meta, cache_strategy }`.
  - `CacheStrategy { cache_nodes, expected_hit_rate, memory_budget }` — vestigial
    statistics from the removed result cache; nothing consults them at eval time.
    Retained only because `ExecutionPlan` is part of the serialized `CompiledExpr`.
  - `DagBuilder`: walks one or more roots, deduplicates identical sub‑trees, assigns
    IDs, computes ref counts and topological order. Expressions nested deeper than
    512 levels are rejected with a validation error (guards plan construction,
    ref counting, and topological sort against stack overflow).

- **`eval.rs`**: compiled evaluator and scalar implementations
  - `EvalOpts { plan: Option<ExecutionPlan> /* serde-skipped */, cache_budget_mb: Option<usize> /* no-op */, max_arena_bytes: usize }`.
  - `CompiledExpr`:
    - `ast: Expr`
    - `plan: Option<ExecutionPlan>`
    - internal `ScratchArena { tmp: Vec<f64>, window: Vec<f64> }` for allocations.
  - Evaluation entrypoint:
    `fn eval(&self, ctx: &SimpleContext, cols: &[&[f64]], opts: EvalOpts) -> EvaluationResult`.
  - Core responsibilities:
    - Decide execution plan (external `EvalOpts.plan` → internal `self.plan` → auto‑build).
    - Execute DAG nodes in **topological order** into a single arena `Vec<f64>`;
      each deduplicated node is computed exactly once per call, and no node
      result is reused across calls.
    - Use `eval_node_into` and scalar helpers (`eval_lag`, `eval_rolling_mean`, etc.) to write each node’s values into a slice of the arena.

- **`mod.rs`**: module docs and public re‑exports
  - High‑level description, supported functions list, and a simple example (see below for expanded usage).

---

## Supported Functions

The `Function` enum in `ast.rs` is the authoritative list. Broadly, functions fall into:

- **Shifts / differences**
  - `Lag`, `Lead`
  - `Diff`, `PctChange`
  - `Shift` (signed shift, positive = down, negative = up)

- **Cumulative operations**
  - `CumSum`, `CumProd`
  - `CumMin`, `CumMax`

- **Rolling window operations** (row‑based windows)
  - `RollingMean`, `RollingSum`
  - `RollingStd`, `RollingVar`, `RollingMedian`
  - `RollingMin`, `RollingMax`, `RollingCount`

- **Exponentially weighted moving stats**
  - `EwmMean`
  - `EwmStd`, `EwmVar`

- **Reducers over the entire series** (broadcast scalar result)
  - `Std`, `Var`, `Median`
  - `Rank` (dense ranking)
  - `Quantile` (global percentile, **not** rolling)

- **Financial utilities** (statement‑layer only)
  - `Sum`, `Mean`
  - `Annualize`, `AnnualizeRate`
  - `Ttm`, `Ytd`, `Qtd`, `FiscalYtd`
  - `Coalesce`

> **Important**: financial utilities (`Sum`, `Mean`, `Annualize*`, `Ttm`, `Ytd`, `Qtd`, `FiscalYtd`, `Coalesce`, `GrowthRate`) are meant to be evaluated at the
> **statements** layer. The scalar evaluator in `eval.rs` returns a typed validation error if they are invoked from `core::expr`.

---

## Basic Usage

### Building and Evaluating a Simple Expression

Below is a minimal end‑to‑end example using `SimpleContext` and direct evaluation:

```rust
use finstack_quant_core::expr::{
    BinOp, CompiledExpr, EvalOpts, Expr, Function, SimpleContext, UnaryOp,
};

// Columns in input frame: ["x", "y"]
let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
let x = vec![1.0, 2.0, 3.0, 4.0];
let y = vec![0.5, 1.5, 2.5, 3.5];
let cols: Vec<&[f64]> = vec![x.as_slice(), y.as_slice()];

// if x > y { x } else { -y }
let condition = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::column("y"));
let then_branch = Expr::column("x");
let else_branch = Expr::unary_op(UnaryOp::Neg, Expr::column("y"));
let expr = Expr::if_then_else(condition, then_branch, else_branch);

let compiled = CompiledExpr::new(expr);
let out = compiled.eval(&ctx, &cols, EvalOpts::default())?;

assert_eq!(out.values.len(), 4);
// out.values ≈ [-0.5, -1.5, 3.0, 4.0]
```

### Rolling Example: `rolling_mean`

```rust
use finstack_quant_core::expr::{CompiledExpr, EvalOpts, Expr, Function, SimpleContext};

// Single column ["x"]
let ctx = SimpleContext::new(["x"]).expect("unique columns");
let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
let cols: Vec<&[f64]> = vec![x.as_slice()];

// rolling_mean(x, 3)
let expr = Expr::call(
    Function::RollingMean,
    vec![Expr::column("x"), Expr::literal(3.0)],
);

let compiled = CompiledExpr::new(expr);
let out = compiled.eval(&ctx, &cols, EvalOpts::default())?;

assert!(out.values[0].is_nan());
assert!(out.values[1].is_nan());
assert!((out.values[2] - 2.0).abs() < 1e-12);
assert!((out.values[3] - 3.0).abs() < 1e-12);
assert!((out.values[4] - 4.0).abs() < 1e-12);
```

### Using DAG Planning

For large model graphs, a pre-built plan deduplicates shared sub-expressions so
each is evaluated once per `eval()` call. The `meta` passed to `with_planning`
is stamped into `EvaluationResult.metadata`:

```rust
use finstack_quant_core::config::{results_meta, FinstackConfig};
use finstack_quant_core::expr::{CompiledExpr, EvalOpts, Expr, Function, SimpleContext};

let ctx = SimpleContext::new(["x"]).expect("unique columns");
let x = vec![0.2, 0.5, 3.0, 4.0];
let cols: Vec<&[f64]> = vec![x.as_slice()];

let expr = Expr::call(
    Function::RollingSum,
    vec![Expr::column("x"), Expr::literal(2.0)],
);

let meta = results_meta(&FinstackConfig::default());
let compiled = CompiledExpr::with_planning(expr, meta)?;

let result = compiled.eval(&ctx, &cols, EvalOpts::default())?;

// Access both values and the metadata carried by the plan.
let values = result.values;
let meta = result.metadata;
```

### Reusing an Execution Plan

Callers can build a plan once and reuse it. Plan reuse only skips the DAG
*planning* step — every `eval()` call recomputes all node values from the
provided columns, so reusing a plan (or a `CompiledExpr`) across different
input data is safe. No intermediate result is carried over between
evaluations.

```rust
use finstack_quant_core::config::{results_meta, FinstackConfig};
use finstack_quant_core::expr::{CompiledExpr, EvalOpts, Expr, Function, SimpleContext};

let ctx = SimpleContext::new(["x"]).expect("unique columns");
let x = vec![0.2, 0.5, 3.0, 4.0];
let cols: Vec<&[f64]> = vec![x.as_slice()];

let expr = Expr::call(Function::Diff, vec![Expr::column("x"), Expr::literal(1.0)]);
let meta = results_meta(&FinstackConfig::default());
let compiled = CompiledExpr::with_planning(expr, meta)?;
let external_plan = compiled.plan.clone();

let result = compiled.eval(
    &ctx,
    &cols,
    EvalOpts {
        plan: external_plan,
        ..EvalOpts::default()
    },
)?;
```

---

## Execution Model and Determinism

- **Scalar only**: all functions operate on `&[f64]` slices; there is no dynamic dispatch to external DataFrame libraries.
- **Arena‑style execution**: the evaluator allocates a single `Vec<f64>` arena sized to
  `len × number_of_nodes`, and each node writes directly into a slice of that arena.
- **Topological order**: DAG nodes are executed in dependency order, ensuring all inputs are
  available before a node is computed.
- **NaN conventions** (see `eval_functions.rs` module docs for the full policy):
  - Rolling windows that are not yet full return `NaN`.
  - Global reducers (`median`, `quantile`, `std`, `var`) **skip NaN** inputs:
    NaNs are excluded from both the sample and the count; all-NaN inputs (or
    fewer than two valid values for `std`/`var`) broadcast `NaN`.
  - `rolling_mean`/`rolling_sum`/`rolling_std`/`rolling_var` **propagate**: a
    NaN inside the window makes that window's output `NaN`. `rolling_median`,
    `rolling_min`, `rolling_max`, and `rolling_count` skip missing values
    within the window.
  - Division by zero with the `/` operator returns `NaN`. `pct_change`
    intentionally differs: a zero base with a non-zero current value returns
    `±inf`, and `0/0` returns `0.0` (no change).
  - Window/step parameters must be constant series (normally literals);
    passing a non-constant series is a validation error.
- **Metadata**: `EvaluationResult.metadata` is the `ResultsMeta` carried by the
  execution plan (the `meta` passed to `with_planning` / `EvalOpts.plan`);
  auto-built plans use the default-config snapshot. The evaluator itself does
  not track timings or parallelism.
- **EWM seeding**: `ewm_mean`, `ewm_std`, and `ewm_var` seed from the **first
  non-NaN** observation; leading NaNs emit NaN, the seed point emits the input
  value for `ewm_mean` and `0.0` for `ewm_std`/`ewm_var` (single-observation
  variance convention), and interior NaNs after the seed emit NaN without
  updating the recursion state.

---

## Extending

Add scalar functions to `Function` in `ast.rs`, implement evaluation in
`eval_functions.rs`, and update cost estimates in `dag.rs`. Statement-layer
functions (`Sum`, `Ttm`, `Annualize`, etc.) must not be dispatched from
`core::expr` — the evaluator returns a typed validation error if they are
invoked here.
