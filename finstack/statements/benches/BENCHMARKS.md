# `finstack-statements` production-scale benchmarks

`statements_scale` targets evaluator, Monte Carlo, aggregate, and capital-structure hot paths at production sizes.

## Run

```bash
cargo bench -p finstack-statements --bench statements_scale
```

## When to re-run

After changes to:

- Evaluator hot path (`evaluator/{engine,formula,formula_dispatch,formula_aggregates,formula_helpers}`)
- Historical cache (`evaluator/context.rs`)
- Monte Carlo loop (`evaluator/monte_carlo.rs`)
- Capital-structure waterfall

Use `statements_operations` for smaller models (4–24 periods, ≤50 nodes).

Keep machine-specific timing baselines in Criterion output or CI artifacts.
