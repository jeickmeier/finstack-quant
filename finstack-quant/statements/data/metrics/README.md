# Built-in metric registry (`fin.*`)

JSON metric definitions for the `finstack-quant-statements` registry. These four
files are `include_str!`-embedded at compile time (by the crate-private
`registry::builtins` module), so `Registry::with_builtins()` /
`Registry::load_builtins()` / `ModelBuilder::with_builtin_metrics()` work in
packaged binaries and WASM builds with no runtime `data/metrics` directory.

## Files

| File | Metrics |
|------|---------|
| `fin_basic.json` | `gross_profit`, `operating_income`, `ebitda`, `ebit`, `ebt`, `net_income` |
| `fin_margins.json` | `gross_margin`, `operating_margin`, `ebitda_margin`, `net_margin`, `cogs_as_pct_revenue`, `opex_as_pct_revenue` |
| `fin_returns.json` | `roe`, `roa`, `roic`, `roce` |
| `fin_leverage.json` | `debt_to_equity`, `debt_to_assets`, `equity_multiplier`, `debt_to_ebitda`, `interest_coverage`, `debt_service_coverage` |

All four declare `"namespace": "fin"` and `"schema_version": 1`, so every metric
lands under a qualified id such as `fin.ebitda`.

## File format

Each file deserializes into `registry::MetricRegistry` with
`deny_unknown_fields`:

```json
{
  "namespace": "fin",
  "schema_version": 1,
  "metrics": [
    {
      "id": "ebitda_margin",
      "name": "EBITDA Margin",
      "formula": "ratio(ebitda, revenue)",
      "description": "EBITDA over same-period revenue",
      "category": "margins",
      "unit_type": "percentage",
      "rollup": "formula",
      "requires": ["ebitda", "revenue"],
      "tags": ["margins", "profitability"]
    }
  ]
}
```

`unit_type` is one of `percentage`, `currency`, `ratio`, `count`, `time_period`.
`description`, `category`, and `unit_type` are optional; `requires`, `tags`, and
`meta` default to empty. `formula` is statements-DSL text. The registry object
itself also accepts an optional top-level `meta` map. `unit_type` is metadata
only — nothing in the evaluator converts or validates units from it.

`rollup` is optional and is one of `sum`, `period_end`, `formula`. It is copied
onto the model node and decides how the metric combines when results are rolled
up to coarser periods. Nothing is summed by default: a metric that leaves it
out needs a method in the rollup request. Every bundled flow declares `sum`,
and every bundled ratio, margin and multiple declares `formula`, so its annual
value is recomputed from the annual inputs. The inputs follow their own declarations: declare `period_end` on
balances such as `total_debt` so a rolled-up leverage ratio reads the closing
balance.

Identifiers inside a formula are written unqualified, or as an exact qualified
id to reach another namespace. Bindings are pinned when the document is
registered: a reference to a metric **in the same namespace** is stored in its
qualified form (so `ratio(ebitda, revenue)` becomes
`ratio(fin.ebitda, revenue)` when `ebitda` is a sibling metric). Anything that
matches no metric stays as written and must resolve to a model node. A metric
registered later does not change an earlier metric's bindings.

## Loading your own registries

```rust
use finstack_quant_statements::registry::Registry;

fn load() -> finstack_quant_statements::Result<Registry> {
    let mut registry = Registry::with_builtins()?;
    registry.load_from_json("path/to/custom_metrics.json")?;
    Ok(registry)
}
```

`Registry::load_from_json_str` and `Registry::load_registry` take in-memory JSON
and a deserialized `MetricRegistry` respectively. Loads are atomic: a registry
whose definitions fail validation leaves the existing catalog untouched. Add
individual metrics to a model with
`ModelBuilder::add_metric_from_registry("fin.ebitda", &registry)`.

## Conventions the bundled metrics assume

### Reported costs include D&A

`cogs` and `opex` are the reported costs, with their depreciation and
amortization included. `depreciation` and `amortization` are the D&A included
in those two lines, and EBITDA adds them back once:

```text
fin.gross_profit     = revenue - cogs
fin.operating_income = fin.gross_profit - opex
fin.ebit             = fin.operating_income
fin.ebitda           = fin.ebit + depreciation + amortization
fin.ebt              = fin.ebit - interest_expense
fin.net_income       = fin.ebt - tax_expense
```

Expense charges are positive; a credit or reversal keeps its negative sign.
Supply an explicit zero for a known zero: a missing input leaves the metrics
that read it unavailable. If your chart of accounts reports OPEX before D&A,
restate the inputs or define your own metrics in a separate namespace.

### Margins need positive revenue

The four margins and two cost ratios are `ratio(numerator, revenue)`: a decimal
ratio that is unavailable (`undefined_base`) when revenue is zero or negative
and unavailable (`missing_input`) when it is missing. A loss keeps its sign on
positive revenue. For a coarser period the ratio is recomputed from the
aggregated numerator and revenue (`"rollup": "formula"`).

### Leverage and coverage

`fin_leverage.json` divides with `ratio`, so every metric is unavailable when
an input is missing or its denominator is zero or negative, and never fails
the run. Zero debt is a valid zero and a negative EBITDA gives a negative
coverage over a positive denominator.

The balance ratios read period-end values. `debt_to_ebitda`,
`interest_coverage` and `debt_service_coverage` read trailing-twelve-month
aggregates (`ttm(ebitda)`, `ttm(interest_expense)`,
`ttm(scheduled_cash_interest_due + scheduled_principal_due)`). `ttm()` needs a
complete year of history, 4 quarters or 12 months with no gap, and is
unavailable before that, so early periods legitimately show no value.

### Interest expense

`interest_expense` is the interest recognized in the income statement,
including recognized PIK. The capital-structure `cs.interest_expense`
component is cash plus PIK; `cs.interest_expense_cash` and
`cs.interest_expense_pik` are available separately. Capitalized interest
during a construction phase typically does not reach `interest_expense` at
all, which inflates coverage in development-stage models.

### Scheduled debt service

`debt_service_coverage` divides by two explicit inputs:
`scheduled_cash_interest_due` and `scheduled_principal_due`, the scheduled
cash interest and scheduled principal (amortization and maturity) on
borrowings, each recognized once on its contractual due date whether or not it
was paid. They exclude arrears settlements, advance payments, prepayments and
sweeps, PIK, fees, hedge flows and distress cash. With a capital structure,
define them from its components:

```text
scheduled_cash_interest_due = cs.scheduled_cash_interest_due.total
scheduled_principal_due     = cs.scheduled_principal_due.total
```

Principal repayment is not tax-deductible, so debt-service coverage measured on
pre-tax EBITDA overstates capacity. Consider EBIAT (`EBIT × (1 - tax_rate)`) as a
conservative numerator in a custom metric.

### Returns on capital

`fin_returns.json` reports same-period, unannualized decimal returns over the
average of the opening balance (the prior period's close) and the closing
balance: `ratio(earnings, (lag(balance, 1) + balance) / 2)`. The first period
of a model has no opening balance and is unavailable, as is a period whose
average balance is zero or negative; a loss stays negative. ROE and ROA read
`fin.net_income` (after statement `tax_expense`) and ROCE and ROIC read
`fin.ebit`, so no return carries its own earnings expression.

ROIC needs two inputs of its own. `operating_tax` is the operating tax expense
before financing effects, separate from statement `tax_expense`; a credit is
negative. `operating_invested_capital` is the period-end operating assets less
non-interest-bearing operating liabilities, including required operating cash
and excluding excess cash and nonoperating assets. Rolling up recomputes each
return from the coarser period's earnings and its own opening and closing
balances, so declare `period_end` on the balances.

More generally, `with_builtin_metrics()` inserts every `fin.*` metric as a
`Calculated` node without creating placeholder nodes for the line items they
reference. `build()` resolves every reference, so missing inputs (`opex`,
`depreciation`, `total_debt`, …) fail it with an unknown-identifier error.
Load selectively with
`add_metric_from_registry` if you only model part of the chart.

### Thresholds

The bundled ratios carry no threshold. Covenant levels and the definitions of
debt, EBITDA and debt service are agreement-specific: state them as explicit
covenant terms or custom metrics.

## Defining custom metrics

1. Use your own namespace (`"namespace": "custom"`). A model node whose id equals
   a qualified metric id shadows that metric, so keeping namespaces distinct
   avoids silent overrides.
2. Reference registry metrics by qualified id in model formulas: `fin.ebitda`,
   not `ebitda`.
3. Document how your line items are classified, especially where D&A sits.
4. State whether a ratio is TTM or single-period.

## See also

- [`../../src/registry/mod.rs`](../../src/registry/mod.rs) — namespace resolution
  and shadowing rules
- [`../../src/dsl/mod.rs`](../../src/dsl/mod.rs) — full DSL function reference
- [`../../README.md`](../../README.md) — crate overview
