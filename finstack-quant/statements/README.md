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
(`actuals_until`). `Evaluator::evaluate_with_market` takes an explicit
`ObservationCutoff`: `FullSnapshot` reads every observation, and
`Through(date)` reads an observation only when its availability date is on or
before that date. A cutoff date needs a model that names its
`source_snapshot` and dates every observation in an actual period; an undated
explicit value in a forecast period is a forecast assumption and stays
visible. Keep an original filing and its revision as two models with different
snapshot identities to replay a past information set.

Node ids are unique. The defining setters (`value`, `compute`, `insert_node`,
`mixed().build()`, `import_observations`) reject a repeated id or a repeated
period; change an existing node with `replace_node`, `remove_node`, or the
atomic `FinancialModelSpec::apply_edits`. `where_clause` names its target
node.

## Public surface

| Module | What you reach for |
|--------|--------------------|
| `builder` | `ModelBuilder`, `MixedNodeBuilder`, `validate_node_id` |
| `types` | `FinancialModelSpec`, `NodeSpec`, `NodeId`, `NodeType`, `NodeValueType`, `AmountOrScalar`, `ForecastSpec`, `ForecastMethod`, `CapitalStructureSpec` |
| `evaluator` | `Evaluator`, `StatementResult`, `PreparedEvaluation`, `EvaluationContext`, `DependencyGraph`, `MonteCarloConfig`/`MonteCarloResults`, `node_to_dated_schedule` |
| `dsl` | `parse_formula`, `compile`, `parse_and_compile`, `extract_all_identifiers`, `StmtExpr` |
| `registry` | `Registry`, `MetricRegistry`, `MetricDefinition`, `UnitType` — the `fin.*` built-in catalog and user namespaces |
| `capital_structure` | `WaterfallSpec`, `EcfSweepSpec`, `PikToggleSpec`, `CapitalStructureCashflows`, `CashflowBreakdown`, `calculate_period_flows` |
| `checks` | `CheckSuite`, `CheckSuiteSpec`, `Check`, `CheckReport`, `FormulaCheckSpec`, and the `builtins` implementations |
| `adjustments` | `normalize`, `merge_into_results`, `NormalizationConfig`, `Adjustment`, `AdjustmentValue`, `AdjustmentCap` |
| `schema` | `ARTIFACTS` schema registry |
| Crate root | `Error`, `Result` |
| `prelude` | The `builder`, `checks`, `error`, and `types` surfaces, the headline `evaluator` items (`Evaluator`, `NumericMode`, `PreparedEvaluation`, `StatementResult`), `registry::Registry`, and core `Money`, `Currency`, `Date`, `PeriodId`, `Tenor` |

The prelude deliberately stops short of `dsl`, `capital_structure`, `adjustments`,
`formula`, and `schema`; import those from their own modules.

Item-level detail is in the rustdoc: `cargo doc -p finstack-quant-statements --open`.

## Quick start

```rust
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::Result;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec};

fn build_and_evaluate() -> Result<StatementResult> {
    let mut builder = ModelBuilder::new("acme");
    builder.periods("2025Q1..Q4", Some("2025Q2"))?;
    builder.value("revenue", &[
        (PeriodId::quarter(2025, 1)?, AmountOrScalar::scalar(10_000_000.0)),
        (PeriodId::quarter(2025, 2)?, AmountOrScalar::scalar(11_000_000.0)),
    ]);
    builder.forecast("revenue", ForecastSpec::growth(0.05));
    builder.compute("cogs", "revenue * 0.6")?;
    builder.compute("gross_profit", "revenue - cogs")?;
    let model = builder.build()?;

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
- Calendar aggregates: `ttm`, `ytd`, `qtd`, `fiscal_ytd`, `annualize`,
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
loads and inserts them all, so the model must define every input the catalog
reads before `build()`; `ModelBuilder::add_metric_from_registry(id, &registry)`
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

A model may carry typed debt instruments, built through their canonical
Valuations APIs and attached with `add_debt`, plus a reporting currency,
FX policy, and an optional `WaterfallSpec`. Formulas then reference aggregated
instrument flows through the `cs.*` namespace.

A capital reference names a component and one of three scopes:

```text
cs.<component>.<instrument_id>   one instrument, in its native currency
cs.<component>.total.<CCY>       every instrument of one native currency
cs.<component>.total             the whole structure, in the reporting currency
```

Each evaluates to exact money in the scope's currency. An unknown component or
malformed scope fails when the formula is parsed; an unknown instrument, a
native currency no instrument uses, or a consolidated total that cannot be
formed fails when the model is built. `total` cannot be an instrument id.

Instruments in more than one currency consolidate only into an explicit
`reporting_currency` with `fx_policy` set to `period_end`, the only policy:
each period's native totals convert at the rate of the inclusive period-close
date, and `CapitalStructureCashflows.fx_conversions` keeps the pair, rate, date
and source. There is no default policy. When a quote is missing the period's
consolidated total is unavailable (`consolidation_unavailable` gives the
reason, and a formula reading it gets an unavailable cell); the per-instrument
and native-currency values are unaffected.

Components: `interest_expense` (cash + PIK), `interest_expense_cash`,
`interest_expense_pik`, `interest_income`, `net_interest_expense_cash`,
`principal_payment` and its parts `scheduled_principal`, `mandatory_prepayment`,
`voluntary_prepayment` and `sweep_prepayment`, `scheduled_cash_interest_due`
and `scheduled_principal_due` (scheduled service recognized once on its
contractual due date, paid or not), `fees`, `opening_balance`,
`debt_balance`, `accrued_interest`, `interest_arrears`, `principal_pending`,
`principal_arrears` and `principal_advance_reserve`. An amount a period does
not record is an error, never zero.

Cash interest follows the declared instrument kind: positive bond and loan
coupons are issuer expenses, while negative debt coupons are income. Swaps use
the holder's signed cashflows, with receipts reported as `interest_income` and
payments as `interest_expense_cash`; both pay-fixed and receive-fixed positions
are supported, including negative rates and zero fixed coupons. Hedge trade
notionals size cashflows and do not count as borrowing principal. `accrued_interest`
is contractual cash interest earned through the period close and due after it,
calculated from the instrument's own earning terms (accrual method and coupon
frequency); an instrument that states none cannot be accrued. Interest already
due and unpaid is `interest_arrears`, never part of the accrual. Derivative
accrual valuation is outside this contract. The debt ledger must open each
period on the principal its dated schedule carries in: a disagreement is an
error naming the instrument, period, currency and both amounts, and no amount
is rescaled.

`debt_balance` is outstanding economic principal: a scheduled reduction takes
effect on its contractual date whether or not its cash was paid. The cash is a
separate dated claim. `principal_pending` is cash owed for a reduction already
effective and not yet due, `principal_arrears` is cash due and unpaid (paid
oldest due first, and settling it never reduces the balance again), and
`principal_advance_reserve` is face already paid for whose reduction is still
to come, which cannot be prepaid a second time. A waterfall needs dated claims
for every instrument; none is synthesized from an aggregate.
Placeholder option schedules are rejected because their contingent payouts are
not modeled.

Plain `Evaluator::evaluate` evaluates the capital structure from an empty
market as of the first period's start, which serves fixed-rate debt: contractual
coupons, amortization and balances need no curves. Instruments whose cashflows
need market data (floating coupons, swaps, caps, swaptions) fail there with a
missing-data error naming the instrument and must be evaluated with
`Evaluator::evaluate_with_market(&model, &market_ctx, as_of, cutoff)`; `as_of`
dates pricing and `cutoff` selects the visible observations.

The evaluator owns dated principal claims and waterfall state. It retains each
claim's cash payment date, economic principal-reduction date, and native-currency
amount so unpaid claims and paid advances affect outstanding balances exactly
once. Market-aware evaluation derives those inputs from canonical instrument
schedules and the actual model periods, including fiscal dates. Runtime state
and allocation machinery are private; Rust, Python, and WASM callers use the
canonical evaluator.

The default waterfall pays fees, interest, scheduled principal, and equity in
that order. Each prepayment requires an explicit payment priority and its
matching sizing rule. A waterfall with any prepayment priority supports
capital structures composed solely of term loans and revolvers.

Each prepayment elects its facilities: `mandatory_prepayment` and
`voluntary_prepayment` are `{amount, facilities}`, and `facilities` is
`"all_eligible"` or `{"named": [...]}`. A named facility must exist, and a term
loan with a call schedule cannot be elected, because prepayments are at par.
The ECF sweep is authored term by term: `ECF = cash_measure - deductions +
credits`, where each term is a formula in the waterfall currency or the cash
interest, fees or scheduled principal that earlier priorities actually paid.
Nothing is deducted unless it is listed. `sweep_percentage` applies to positive
ECF. A rung pays the least of its request, the remaining cash and the elected
facilities' principal, and `CapitalStructureCashflows.prepayments` records the
requested, paid and unpaid amounts. A negative request is an error. Sizing,
payment and the principal reduction all fall in the period being evaluated.

Waterfall allocation is exact. Senior payment classes are paid first (empty
`payment_classes` is one implicit class). Within a class, claims are paid in
full when the cash covers them; otherwise each pro-rata quota is floored to the
currency's minor unit and the cents left over go to the largest fractional
remainders, ties by instrument id. Every unpaid amount is carried, however
small, a negative fee or interest claim is rejected, and the period reconciles
exactly: fees, cash interest, principal, the equity distribution and retained
cash sum to the available cash. The residual is an equity distribution when
`Equity` is the terminal priority and `retained_cash` when it is absent. A
negative pool is a funding shortfall and allocates nothing, and the waterfall
runs in a debt-free period too.

`available_cash_node` is the cash pool before debt service. It may not depend
on any same-period `cs` component, directly or through the formulas and masks
of the nodes it names: every such component is an output of the waterfall the
pool funds. Read a prior period with `lag(...)`.

After a prepayment, the term loan or revolver recompiles its schedule from its
own terms and the dated principal changes: coupons, the contractual cash/PIK
split, balance-based fees and later principal follow the new balance from the
day after the period close, and interest already earned is preserved. The
ledger and the rebuilt schedules commit together.

A PIK toggle is the modeled exercise of a borrower's election. `pik_toggle` is
`{liquidity_metric, threshold, elections}` and each election authorizes one
term loan: `{instrument_id, pik_fraction, min_coupon_periods}`. In a forecast
period a metric below the threshold requests PIK. The election starts with the
loan's next coupon period, never on a coupon already running, lasts at least
its minimum number of coupon periods, and is applied by the loan's own coupon
terms: the elected share is zero cash and capitalizes once on the coupon date.
Overdue cash interest and fees stay cash claims and are never capitalized.
Actual periods are not exercised.

Known limits (also stated in the module rustdoc): prepayment penalties, call
premiums, and OID accretion are not modeled.

## Checks

`CheckSuite` runs validation against a model plus its `StatementResult` and
attaches a `CheckReport` to the result. Attach one with
`Evaluator::new().with_checks(suite)`, or build a suite declaratively from a
serializable `CheckSuiteSpec` via `CheckSuiteSpec::resolve()`.

Built-ins: `BalanceSheetArticulation`, `RetainedEarningsReconciliation`,
`CashReconciliation`, `MissingValueCheck`, `NonFiniteCheck`,
`SignConventionCheck`. `FormulaCheckSpec` evaluates a DSL formula per period in
an explicit `mode`: a `predicate` is a Boolean formula that must be true, a
`residual` is an amount compared with zero under an optional absolute
`tolerance`. Truthiness is never inferred, so a numeric predicate, a Boolean
residual and a tolerance on a predicate are rejected. `scope` (`PeriodScope`)
selects all, actual or forecast periods, and the optional Boolean `eligibility`
formula selects periods within it. Evaluation is lazy over typed cells:
`coalesce(a, b)` uses `b` when `a` has no value and an untaken `if` branch does
not read its inputs. A period whose formula or eligibility condition needed an
observation without a value (a missing or unavailable cell, a `lag` before the
first period) carries a coverage finding and makes the check incomplete; a
division by zero or other invalid arithmetic is an execution error.

Identity checks treat a missing operand and a non-finite operand the same way —
both skip with a warning rather than passing, because `NaN > tolerance` is
`false` and would otherwise fail open.

Domain-level checks (three-statement reconciliation, credit underwriting, LBO)
are in the analytics crate.

## Adjustments

`adjustments::normalize(&result, &config)` builds the ordered bridge from a
reported node to a separately named adjusted metric (typically reported EBITDA
to adjusted EBITDA). A `NormalizationConfig` names the reported `target_node`,
the new `output_node` and the adjustments in order. Each `Adjustment` carries
an `AdjustmentValue` (signed fixed per-period amounts in the target's units, or
a signed decimal percentage of another node's reported value) and an optional
`AdjustmentCap`: `Absolute { amount }`, or `Reference { base_node, fraction,
base_mode }`, a fraction of the positive part of a base node. A cap limits its
own adjustment's magnitude and keeps its sign; a cap on the target uses the
reported value unless `base_mode` is `Progressive`, which adds the earlier
applied adjustments.

The whole configuration is validated before any period is calculated:
identifiers, references, units and currencies, the period keys of fixed
amounts and every number. Money is added and scaled exactly and scalars with a
compensated sum, and every row is checked, so an arithmetic failure returns an
error and no bridge. Each `NormalizationResult` holds the reported
`base_value`, the rows (`AppliedAdjustment`: raw amount, cap base and limit,
applied amount and `adjusted_value`, the target through that row) and the
`final_value`, all typed. A period whose target, percentage reference or cap
base has no value is `unavailable` with its reason.

`adjustments::merge_into_results(&mut result, &config)` calculates the bridge
and publishes `output_node` as a new node. It never overwrites a node, leaves
the reported target and the check report as they are, and records the
configuration in `StatementResult::normalizations` beside the result's source
run. A ratio or covenant on the adjusted basis references the output node
explicitly.

## Results and export

`StatementResult` holds `cells: node_id → period_id → ResultCell`, the typed
result: a cell has a `status` (`value`, `missing`, `masked`, `unavailable`), a
finite scalar or exact `Money` when it is a value, a `reason` when it is
unavailable, and the value `source` that was selected. It also holds `periods`
(the plan `cells` is keyed by), `node_value_types`, `provenance` (model id and
content hash, source snapshot, observation cutoff, valuation date, run id), optional
`cs_cashflows`, an optional `check_report`, and `meta: EvalStats` (timings,
graph size, numeric mode, parallel flag, warnings). Accessors: `get`,
`get_money`, `get_scalar`, `get_node`, `get_or`, `all_periods`.

`rollup(&model, &RollupSpec)` rolls results up to a coarser frequency
(monthly or quarterly into quarterly, semi-annual or annual, or semi-annual into
annual). Each line declares its method on the node (`NodeSpec::rollup`):
`sum` for flows (exact money, a compensated scalar sum; a source cell without
a value leaves the target unavailable), `period_end` for balances and trailing
metrics, which copies the final source cell with its status, and `formula` for
ratios, which re-evaluates the mask and formula on the rolled-up values at the
target cadence. A line resolves as its entry in `RollupSpec::methods`, then its
declaration; a line with neither fails the rollup, because nothing is summed
by default and tags never choose a method. The source result must be a
verified run of the model. Every target period must be complete, and the
result's `periods` are the targets, each actual only when every source is. Its
`rollup_provenance` records the source run, the method used for each node and
the source periods of each target period.
`RollupSpec::declared_methods(&model, frequency)` lists the declared methods so a
host can seed its method choices.

Tabular export returns `finstack_quant_core::table::TableEnvelope`:

| Method | Schema |
|--------|--------|
| `to_table_long()` | Exact: `(node_id, period_id, status, value, amount, currency, value_type, reason, source)`, one row per node and plan period |
| `to_table_long_filtered(&["a", "b"])` | Same, restricted to named nodes; unknown ids are ignored |
| `to_table_wide()` | Approximate numeric view: `(period_id, <node1>, <node2>, …)`, one row per plan period, null for a cell without a value, units in column metadata |

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

`Evaluator::evaluate_monte_carlo(&model, &MonteCarloConfig::new(n_paths, seed, metrics), context)`
replays the complete model on each path and returns R-7 quantiles of the
requested `metrics` per forecast period, with their units and sample count,
plus an optional long-format path table when `with_path_data(true)` is set.
`context` is the optional market, valuation date and observation cutoff of
`evaluate_with_market`; `None` evaluates as `evaluate` does. A capital
structure runs on every path. When a requested sample has no value on some
path the run is incomplete: no statistics are published and `incomplete` names
the first such sample. `breach_probability(metric, threshold)` takes a
threshold typed in the metric's units and counts paths with any forecast value
strictly above it.

## Conventions that bite

- **Numerics.** A scalar node's value cells are finite `f64`; a monetary
  node's are exact `Money`, as are capital-structure cashflows. A cell without
  a value is never a zero or a `NaN`. `NodeValueType` records whether a node is
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
  aborting; `NonFiniteCheck` is how you make them fail. A Monte Carlo sample
  that is not finite is an error, and a sample without a value makes the run
  incomplete.
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
module. Regenerate and verify with `mise run gen-write` and
`mise run gen-check` (the `gen_statement_schemas` binary backs both).

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
  the `Evaluator`, `ModelBuilder`, `MixedNodeBuilder` and `Registry` classes,
  `parseFormula`, `parseAndCompile`, `normalize`, `modelNodeIds`, the
  `validate*Json` validators, and free-function twins of the Python result and
  spec methods (`statementResult*`, `capitalStructureCashflows*`,
  `forecastSpec*`, `adjustment*`). Data types are plain objects typed from the
  JSON Schemas.

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
