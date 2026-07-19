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

The default matrix keeps large workflow runs bounded while retaining the
63/64 threshold, 3,000-position PV/selective cases, 40/120 attribution cases,
10/100 scenario batches, and 20/250-snapshot replay cases. Enable the extended
large-book replay/scenario matrix and the 25,000-position valuation control
explicitly:

```bash
FINSTACK_PORTFOLIO_BENCH_FULL=1 \
FINSTACK_PORTFOLIO_BENCH_XL=1 \
cargo bench -p finstack-quant-portfolio
```

## Benches

| Bench | Focus |
|-------|--------|
| `portfolio_valuation` | Full valuation, entity/multicurrency aggregation, filtering, scaling, `revalue_affected` |
| `portfolio_cashflows` | Cashflow ladder aggregation |
| `portfolio_metrics` | `aggregate_metrics` alone and with valuation |
| `portfolio_attribution` | P&L attribution paths |
| `portfolio_workflows` | Batched scenario P&L and historical replay reuse |
| `sensitivity_simulation` | Full-repricing grids, factor stress, and simulation decomposition |
| `parallel_thresholds` | Rayon parallel thresholds |

`portfolio_valuation` builds multi-entity portfolios (deposits, bonds, swaps,
options, CDS, convertibles, structured credit, and related types). The
25,000-position control is opt-in because it materially increases Criterion
setup and measurement time.

## Results

HTML reports: `target/criterion/<bench>/report/index.html`

```bash
open target/criterion/portfolio_valuation/report/index.html
```

Release builds only; timings vary by hardware. Use `--quick` during iteration.
