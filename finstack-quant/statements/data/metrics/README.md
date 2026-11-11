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
      "formula": "ebitda / revenue",
      "description": "EBITDA as a percentage of revenue",
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
up to coarser periods. A metric that leaves it out follows the rollup default
and is summed, which is only right for a flow. Every bundled ratio, margin and
multiple declares `formula`, so its annual value is recomputed from the annual
inputs. The inputs follow their own declarations: declare `period_end` on
balances such as `total_debt` so a rolled-up leverage ratio reads the closing
balance.

Identifiers inside a formula are written unqualified. When a metric is inserted
into a model, references to other metrics **in the same namespace** are rewritten
to their qualified form (so `ebitda / revenue` becomes `fin.ebitda / revenue`
when `revenue` is a model node and `ebitda` is a sibling metric). Anything not
matching a sibling metric id stays as written and must resolve to a model node.

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

### Operating expenses exclude D&A

`opex` is treated as **excluding** depreciation and amortization; supply those as
separate `depreciation` and `amortization` nodes. Consequently:

```text
fin.ebitda           = revenue - cogs - opex
fin.ebit             = fin.ebitda - depreciation - amortization
fin.operating_income = revenue - cogs - opex - depreciation - amortization
```

so `fin.ebit == fin.operating_income` by construction. If your chart of accounts
embeds D&A inside `opex`, EBITDA will be understated — either restate the inputs
or define your own metrics in a separate namespace.

### Trailing-twelve-month coverage

`fin_leverage.json` computes coverage and leverage on TTM aggregates
(`ttm(ebitda)`, `ttm(interest_expense)`,
`ttm(interest_expense + principal_payment)`). `ttm()` needs a full year of
history — 4 quarters or 12 months — and returns `NaN` before that, so early
periods in a model legitimately show no coverage value.

### Interest expense

`interest_expense` must carry whatever your accounting policy includes: cash
interest, PIK accruals, and debt-cost amortization. The capital-structure
`cs.interest_expense` component is cash plus PIK; `cs.interest_expense_cash` and
`cs.interest_expense_pik` are available separately. Capitalized interest during a
construction phase typically does not reach `interest_expense` at all, which
inflates coverage in development-stage models.

### Principal and taxes

Principal repayment is not tax-deductible, so debt-service coverage measured on
pre-tax EBITDA overstates capacity. Consider EBIAT (`EBIT × (1 - tax_rate)`) as a
conservative numerator.

### Tax node naming

`fin_basic.json` deducts a node named `tax_expense`; `fin_returns.json` deducts a
node named `taxes`. A model that loads both files needs both nodes, or one
aliased to the other.

More generally, `with_builtin_metrics()` inserts every `fin.*` metric as a
`Calculated` node without creating placeholder nodes for the line items they
reference. Missing inputs (`opex`, `depreciation`, `total_debt`, …) are not
diagnosed by `build()`; they surface as unknown-identifier errors when the
dependency graph is built for evaluation. Load selectively with
`add_metric_from_registry` if you only model part of the chart.

### Reference thresholds

Industry rules of thumb, for orientation only — nothing in the registry enforces
them:

- Interest coverage: > 1.5x (investment grade), > 2.5x (strong)
- Debt service coverage: > 1.25x (typical covenant), > 1.5x (comfortable)
- Debt/EBITDA: < 3.0x (conservative), < 4.0x (acceptable in many industries)

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
