# Portfolio Benchmarks

Criterion benchmarks for the maintained portfolio hot paths: full valuation,
cashflows, metrics, attribution, and parallel threshold behavior. The suite is manifest-driven
(`autobenches = false`) so new files do not expand benchmark runtime unless they
are deliberately added to `Cargo.toml`.

## Run

```bash
cargo bench -p finstack-quant-portfolio
cargo bench -p finstack-quant-portfolio --bench portfolio_valuation
cargo bench -p finstack-quant-portfolio -- --quick
cargo bench -p finstack-quant-portfolio -- --save-baseline my_baseline
cargo bench -p finstack-quant-portfolio -- --baseline my_baseline
```

## Benches

| Bench | Focus |
|-------|--------|
| `portfolio_valuation` | Full valuation, entity/multicurrency aggregation, filtering, scaling, `revalue_affected` |
| `portfolio_cashflows` | Cashflow ladder aggregation |
| `portfolio_metrics` | `aggregate_metrics` alone and with valuation |
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
