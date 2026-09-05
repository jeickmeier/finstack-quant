# finstack-quant-statements

Period-based financial statement modeling. A model is a directed graph of named
nodes evaluated over a discrete period grid (monthly, quarterly, annual); each
node resolves per period from an explicit value, a forecast, or a formula
written in this crate's DSL.

Higher-level analysis built on top of this engine — DCF, scenario sets,
sensitivity, ECL, backtesting, corkscrew validation, credit scorecards — lives
in [`finstack-quant-statements-analytics`](../statements-analytics/README.md).

## Where it sits

Depends on [`finstack-quant-core`](../core/README.md) (dates, periods, money,
expression engine, `table` envelope), [`finstack-quant-cashflows`](../cashflows/README.md)
(cashflow kinds for capital-structure aggregation), and
[`finstack-quant-valuations`](../valuations/README.md) (typed debt instruments
priced against a `MarketContext`).

Consumed by `finstack-quant-statements-analytics`. Re-exported from the umbrella
crate as `finstack_quant::statements`.

`rayon` is a dependency only on non-`wasm32` targets; there is no cargo feature
that toggles it.

## Precedence: Value > Forecast > Formula

Every node resolution for a `(node, period)` pair walks the same ladder
(`evaluator/precedence.rs`):

1. An explicit value declared for that period wins.
2. Otherwise, if the node carries a `ForecastSpec` **and** the period is not an
   actuals period, the forecast produces the value.
3. Otherwise, the node's `formula_text` is evaluated.
4. Otherwise the node cannot be resolved and evaluation errors.

`NodeType` records which of these a node is allowed to use: `Value`,
`Calculated` (formula only), or `Mixed` (all three). Attaching a forecast with
`ModelBuilder::forecast` upgrades a `Value` or `Calculated` node to `Mixed`, so a
forecast can never silently shadow a formula it was not declared alongside.

The actuals boundary comes from the second argument to `ModelBuilder::periods`
(`actuals_until`). `Evaluator::evaluate_with_market` additionally applies the
`as_of` visibility cutoff. Explicit actuals become visible on their node-level
availability date; when omitted, availability defaults to the period's
exclusive end date.

## Public surface

| Module | What you reach for |
|--------|--------------------|
| `builder` | `ModelBuilder` (type-state: `NeedPeriods` → `Ready`), `MixedNodeBuilder`, `validate_node_id` |
| `types` | `FinancialModelSpec`, `NodeSpec`, `NodeId`, `NodeType`, `NodeValueType`, `AmountOrScalar`, `ForecastSpec`, `ForecastMethod`, `CapitalStructureSpec` |
| `evaluator` | `Evaluator`, `StatementResult`, `PreparedEvaluation`, `EvaluationContext`, `DependencyGraph`, `evaluate_order`, `MonteCarloConfig`/`MonteCarloResults`, `node_to_dated_schedule` |
| `dsl` | `parse_formula`, `compile`, `parse_and_compile`, `StmtExpr`, `BinOp`, `UnaryOp` |
| `forecast` | Deterministic and statistical projection methods driven by `ForecastSpec` |
| `registry` | `Registry`, `MetricRegistry`, `MetricDefinition`, `UnitType` — the `fin.*` built-in catalog and user namespaces |
| `capital_structure` | `WaterfallSpec`, `EcfSweepSpec`, `PikToggleSpec`, `CapitalStructureCashflows`, `CashflowBreakdown`, `execute_waterfall`, `calculate_period_flows` |
| `checks` | `CheckSuite`, `CheckSuiteSpec`, `Check`, `CheckReport`, `FormulaCheckSpec`, and the `builtins` implementations |
| `adjustments` | `engine::NormalizationEngine`; `types::NormalizationConfig`/`Adjustment`/`AdjustmentValue`/`AdjustmentCap`. This module has no root re-exports — name the submodule |
| `formula` | `extract_all_identifiers` — the curated helper boundary shared with the analytics crate |
| `schema` | `financial_model_spec_schema`, `statement_result_schema`, `normalization_config_schema` |
| `error` | `Error`, `Result`; both are also re-exported at the crate root |
| `prelude` | The `builder`, `checks`, `error`, and `types` surfaces, the headline `evaluator` items (`Evaluator`, `NumericMode`, `PreparedEvaluation`, `StatementResult`), `registry::Registry`, and core `Money`, `Currency`, `Date`, `PeriodId`, `Tenor` |

The prelude deliberately stops short of `dsl`, `capital_structure`, `adjustments`,
`formula`, and `schema`; import those from their own modules.

Item-level detail is in the rustdoc: `cargo doc -p finstack-quant-statements --open`.

## Quick start

```rust
use finstack_quant_statements::prelude::*;
use indexmap::indexmap;

fn build_and_evaluate() -> Result<StatementResult> {
    let model = ModelBuilder::new("acme")
        .periods("2025Q1..Q4", Some("2025Q2"))?
        .value("revenue", &[
            (PeriodId::quarter(2025, 1)?, AmountOrScalar::scalar(10_000_000.0)),
            (PeriodId::quarter(2025, 2)?, AmountOrScalar::scalar(11_000_000.0)),
        ])
        .forecast("revenue", ForecastSpec {
            method: ForecastMethod::GrowthPct,
            params: indexmap! { "rate".into() => serde_json::json!(0.05) },
        })
        .compute("cogs", "revenue * 0.6")?
        .compute("gross_profit", "revenue - cogs")?
        .build()?;

    // Q1/Q2 come from explicit values; Q3/Q4 grow 5% off the last actual
    // (Q3 = 11_550_000.0).
    let mut evaluator = Evaluator::new();
    evaluator.evaluate(&model)
}
```

`2025Q1..Q4` is the period range; `Some("2025Q2")` marks periods through Q2 as
actuals, so the forecast applies from Q3 onward. Without the forecast, `revenue`
would be a `Value` node with no value for Q3/Q4 and evaluation would error.

## The formula DSL

Formulas are parsed by `nom` into a `StmtExpr` AST and compiled to
`finstack_quant_core::expr::Expr`. Beyond arithmetic, comparison, and
`and`/`or`/`not`, the DSL provides:

- Conditionals and math: `if`, `abs`, `sign`, `pow`, `round`, `floor`, `ceil`,
  `ln`, `exp`, `log10`, `sqrt`, `clamp`, `is_missing`, `coalesce`.
- N-ary reducers over their arguments (not over history): `sum`, `mean`, `min`,
  `max`.
- Temporal lookups: `lag`, `shift`, `diff`, `pct_change`, `growth_rate`.
- Cumulative and rolling: `cumsum`/`cumprod`/`cummin`/`cummax`, and the
  `rolling_*` family (`mean`, `sum`, `std`, `var`, `median`, `min`, `max`,
  `count`) with pandas-parity `min_periods` semantics.
- Historical statistics over a node's own series: `std`, `var`, `median`,
  `rank`, `quantile`, `ewm_mean`, `ewm_std`, `ewm_var`.
- Calendar aggregates: `ttm`/`ltm`, `ytd`, `qtd`, `fiscal_ytd`, `annualize`,
  `annualize_rate`.

There is no `lead()`: forward-looking references would leak future values into
historical periods.

The full per-function table, including NaN and `min_periods` behavior, is in the
`dsl` module rustdoc.

### Corkscrews and temporal self-reference

`lag()` and `shift()` references are deliberately excluded from the dependency
graph's direct edges, so a node may refer to its own prior period without
creating a same-period cycle. That is what makes roll-forward ("corkscrew")
schedules expressible:

```text
closing_debt = lag(closing_debt, 1) + drawdowns - repayments
```

Build-time helpers for generating these node sets (`add_roll_forward`) and
runtime articulation validation (`CorkscrewExtension`) live in the analytics
crate.

## Metric registry (`fin.*`)

`Registry::with_builtins()` loads the bundled `fin.*` catalog, embedded at
compile time from [`data/metrics`](data/metrics/README.md) — no runtime data
directory is required, including in WASM builds. `ModelBuilder::with_builtin_metrics()`
loads and inserts them all; `ModelBuilder::add_metric_from_registry(id, &registry)`
adds one plus its in-registry dependencies, in dependency order, skipping any id
the model already defines. Additional namespaces load via
`Registry::load_from_json`, `load_from_json_str`, or `load_registry`; a registry
whose definitions fail validation is rejected whole, leaving the prior catalog
untouched.

Metrics are stored under fully qualified ids (`fin.ebitda`) and referenced that
way in formulas. Intra-namespace references inside a metric's own formula are
qualified automatically when the metric is inserted. A model node whose id
matches a qualified metric id shadows that metric — prefer a distinct namespace
(`custom.*`) for your own definitions.

## Capital structure (`cs.*`)

A model may carry typed debt instruments (`add_bond`, `add_bond_with_convention`,
`add_swap`, `add_swap_with_conventions`, `add_debt`) plus a reporting currency,
FX policy, and an optional `WaterfallSpec`. Formulas then reference aggregated
instrument flows through the `cs.*` namespace.

When `fx_policy` is omitted, `cs.*` cash items and balances convert on the
inclusive period-end date (`PeriodEnd`): the already-aggregated period bucket
is converted once, not each contractual cashflow date.

```text
cs.<component>.<instrument_id>
cs.<component>.total
```

Valid components: `interest_expense` (cash + PIK), `interest_expense_cash`,
`interest_expense_pik`, `interest_income`, `principal_payment`, `debt_balance`,
`fees`, `accrued_interest`. Unknown components are rejected at compile time.

`cs.*` requires market data, so these models must be evaluated with
`Evaluator::evaluate_with_market(&model, &market_ctx, as_of)`; plain `evaluate`
cannot resolve them.

Known limits (also stated in the module rustdoc): waterfall allocation is
pro-rata inside each payment class, walking class rank (empty
`payment_classes` is one implicit class); loan residual schedules rebuild
interest after outstanding changes, and Bond / ConvertibleBond plus a sweep
is rejected; `available_cash_node` is the pre-waterfall cash pool and must
not deduct `cs` debt-service tokens; omitted `fx_policy` converts
period-aggregated `cs.*` items on the inclusive period-end date
(`PeriodEnd`); prepayment penalties, call premiums, and OID accretion are
not modeled.

## Checks

`CheckSuite` runs validation against a model plus its `StatementResult` and
attaches a `CheckReport` to the result. Attach one with
`Evaluator::new().with_checks(suite)`, or build a suite declaratively from a
serializable `CheckSuiteSpec` via `CheckSuiteSpec::resolve()`.

Built-ins: `BalanceSheetArticulation`, `RetainedEarningsReconciliation`,
`CashReconciliation`, `MissingValueCheck`, `NonFiniteCheck`,
`SignConventionCheck`. `FormulaCheckSpec` evaluates an arbitrary DSL predicate per
period. Findings carry `Severity`, `CheckCategory`, `PeriodScope`, and a
`Materiality` tolerance.

Identity checks treat a missing operand and a non-finite operand the same way —
both skip with a warning rather than passing, because `NaN > tolerance` is
`false` and would otherwise fail open.

Domain-level checks (three-statement reconciliation, credit underwriting, LBO)
are in the analytics crate.

## Adjustments

`NormalizationEngine::normalize(&result, &config)` computes an adjusted metric
(typically adjusted EBITDA) from a reported node plus an explicit catalog of
add-backs. Each `Adjustment` carries an `AdjustmentValue` (fixed per-period
amounts or a percentage of another node) and an optional `AdjustmentCap`. The
returned `Vec<NormalizationResult>` keeps both the adjusted total (`final_value`)
and the per-adjustment audit trail (`adjustments: Vec<AppliedAdjustment>`), so
the reported-to-adjusted bridge stays explainable.
`NormalizationEngine::merge_into_results(&mut result, &normalized, "adj_ebitda")`
folds the adjusted series back into a `StatementResult` as a new node.

## Results and export

`StatementResult` holds `nodes: node_id → period_id → f64`, plus
`monetary_nodes` for `Money`-typed nodes, `node_value_types`, optional
`cs_cashflows`, an optional `check_report`, and `meta: EvalStats` (timings,
graph size, numeric mode, parallel flag, warnings). Accessors: `get`,
`get_money`, `get_scalar`, `get_node`, `get_or`, `all_periods`.

Tabular export returns `finstack_quant_core::table::TableEnvelope`:

| Method | Schema |
|--------|--------|
| `to_table_long()` | `(node_id, period_id, value, value_money, currency, value_type)` |
| `to_table_long_filtered(&["a", "b"])` | Same, restricted to named nodes; unknown ids are ignored |
| `to_table_wide()` | `(period_id, <node1>, <node2>, …)`, one row per period |

Wide export encodes missing node-period observations as `NaN`, not zero, so
absence stays distinguishable from an evaluated zero. Long export preserves
declaration order; wide export sorts periods ascending.

`node_to_dated_schedule(&model, &result, node_id, PeriodDateConvention::…)`
converts a node's series into `(Date, f64)` pairs for cashflow-shaped consumers.

## Repeated evaluation and Monte Carlo

`Evaluator::prepare(&model)` compiles formulas, builds the DAG, and caches the
evaluation order once; `evaluate_prepared(&model, &plan)` then re-evaluates with
changed input values only. Rebuild the plan after any structural or formula
change — it deliberately reuses the compiled expressions captured at prepare
time. This is what sensitivity sweeps and goal seek run on.

`Evaluator::evaluate_monte_carlo(&model, &MonteCarloConfig::new(n_paths, seed))`
runs stochastic forecast nodes across paths and returns percentile series, plus
an optional long-format path table when `with_path_data(true)` is set. Capital
structure is rejected in Monte Carlo mode; run instrument-level Monte Carlo in
`finstack-quant-valuations` instead.

## Conventions that bite

- **Numerics.** Node results are `f64`. `Money` appears in `monetary_nodes` and
  in capital-structure cashflows; `NodeValueType` records whether a node is
  `Monetary { currency }` or `Scalar`. See [INVARIANTS.md](../../INVARIANTS.md) §1.
- **Rates and ratios.** Decimal form throughout: a `GrowthPct` `rate` of `0.05`
  is 5%; a leverage metric of `4.5` is 4.5x.
- **Determinism and seeding.** Statistical forecast methods require a `seed`.
  Both single-run and Monte Carlo evaluation mix a stable hash of the node id
  into that seed so independent stochastic nodes do not draw identical shocks;
  Monte Carlo layers a per-path offset on top.
- **Parallel equals serial.** Monte Carlo paths run on rayon natively and
  serially on `wasm32`. Everything that reaches the wire is canonically ordered
  before it is emitted — percentiles are computed from a sorted vector, warnings
  are sorted, and path-table rows are sorted by `(path_id, metric, period)` —
  so a parallel run and a serial run at the same seed agree.
- **Non-finite values.** Ordinary evaluation stores non-finite results (for
  example, division by zero) and surfaces them as `EvalWarning`s rather than
  aborting; `NonFiniteCheck` is how you make them fail. Monte Carlo aggregation
  is stricter and errors on the first non-finite path value.
- **Serde strictness.** `FinancialModelSpec`, `StatementResult`,
  `NormalizationConfig`, `MetricRegistry`, and the check/waterfall specs all
  deny unknown fields. `StatementResult.schema_version` accepts only numeric `1`.
- **Formula size.** Formulas are capped at 256 terms. The budget exists because
  formulas are compiled on rayon workers with a 2 MiB default stack during Monte
  Carlo, and a stack overflow aborts the process rather than unwinding.

## Schemas

Checked-in JSON Schema artifacts live under `schemas/statements/1/`:
`financial_model_spec.schema.json`, `statement_result.schema.json`,
`normalization_config.schema.json`. Access them from Rust via the `schema`
module. Regenerate and verify with `mise run rust-gen-schemas` and
`mise run rust-check-schemas` (the `gen_statement_schemas` binary backs both).

See [docs/SERDE_STABILITY.md](../../docs/SERDE_STABILITY.md) for the wire-format
policy.

## Bindings

- **Python** — `finstack_quant.statements`: `FinancialModelSpec`, `ModelBuilder`,
  `MixedNodeBuilder`, `Evaluator`, `StatementResult`, `MetricRegistry`,
  `MonteCarloConfig`/`MonteCarloResults`/`run_monte_carlo`, `NodeType`,
  `NodeId`, `ForecastSpec`/`ForecastMethod`, `NumericMode`, `WaterfallSpec`/
  `EcfSweepSpec`/`PikToggleSpec`, `parse_formula`, `validate_formula`,
  `NormalizationConfig`/`normalize`, `CheckSuiteSpec`/`CheckReport`, `schema`.
- **WASM** — `statements` namespace in `finstack-quant-wasm/exports/statements.js`:
  `evaluateModel`, `evaluateModelWithMarket`, `runMonteCarlo`,
  `parseFormulaText`, `validateFormula`, `modelNodeIds`, and the
  `validate*Json` validators.

## Verification

```bash
cargo nextest run -p finstack-quant-statements --lib --test '*'
cargo test -p finstack-quant-statements --doc
cargo clippy -p finstack-quant-statements --lib --bins --tests --examples -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p finstack-quant-statements --no-deps
```

Do not run bare `cargo test -p …` for this crate; the project convention is
nextest for unit/integration tests and an explicit `--doc` pass for doctests.

Fuzzing (`cargo-fuzz`, nightly) targets the formula parser. `fuzz/` is a
separate, unpublished workspace with a seeded corpus under
`fuzz/corpus/parse_formula`:

```bash
cd finstack-quant/statements && cargo +nightly fuzz run parse_formula
```

Benchmarks: see [`benches/README.md`](benches/README.md).
