# Portfolio benchmarks

Criterion benchmarks for portfolio valuation, metrics, cashflows, attribution, and
parallel thresholds.

## Run

```bash
cargo bench -p finstack-portfolio
cargo bench -p finstack-portfolio --bench portfolio_valuation
cargo bench -p finstack-portfolio -- --quick
cargo bench -p finstack-portfolio -- --save-baseline my_baseline
cargo bench -p finstack-portfolio -- --baseline my_baseline
```

## Benches

| Bench | Focus |
|-------|--------|
| `portfolio_valuation` | Full valuation, entity/multicurrency aggregation, filtering, scaling, `revalue_affected` |
| `portfolio_metrics` | `aggregate_metrics` alone and with valuation |
| `portfolio_cashflows` | Cashflow ladder aggregation |
| `portfolio_attribution` | P&L attribution paths |
| `parallel_thresholds` | Rayon parallel thresholds |

`portfolio_valuation` builds multi-entity portfolios (deposits, bonds, swaps, options,
CDS, convertibles, structured credit, and related types) from 10 to 1000 positions.

## Results

HTML reports: `target/criterion/<bench>/report/index.html`

```bash
open target/criterion/portfolio_valuation/report/index.html
```

Release builds only; timings vary by hardware. Use `--quick` during iteration.
