# Financial metrics registry

JSON metric definitions loaded by `finstack-quant-statements` registry APIs. Built-in files are embedded at compile time via `Registry::with_builtins()` / `ModelBuilder::with_builtin_metrics()`.

## Files

| File | Contents |
|------|----------|
| `fin_basic.json` | Core income statement metrics |
| `fin_leverage.json` | Leverage and coverage ratios |
| `fin_margins.json` | Margin metrics |
| `fin_returns.json` | Return metrics |

Additional registries can be loaded with `ModelBuilder::with_metrics()`.

## Conventions

### EBITDA

Coverage metrics in `fin_leverage.json` use EBITDA = `revenue - cogs - opex + depreciation + amortization`. D&A must be separate line items, not embedded in COGS or opex.

### Interest expense

Include cash interest, PIK accruals, and debt-cost amortization per your accounting policy. `cs.interest_expense` from capital-structure integration includes PIK automatically.

### Principal and taxes

Principal is not tax-deductible; debt-service coverage on pre-tax EBITDA can overstate capacity. Consider EBIAT (`EBIT × (1 - tax_rate)`) for conservative analysis.

### Capitalized interest

Capitalized interest during construction may be absent from `interest_expense`, distorting coverage during development phases.

### Reference thresholds

Typical industry guidelines (not enforced by the registry):

- Interest coverage: > 1.5x (IG), > 2.5x (strong)
- Debt service coverage: > 1.25x (covenant), > 1.5x (comfortable)
- Debt/EBITDA: < 3.0x (conservative), < 4.0x (acceptable for many industries)

### Custom metrics

1. Reference registry metrics with qualified ids: `fin.ebitda`, not `ebitda`.
2. Document line-item classification assumptions.
3. State whether ratios use TTM or period values.

## See also

- `src/registry/mod.rs` — namespace resolution and shadowing rules
- `src/dsl/mod.rs` — formula function reference
