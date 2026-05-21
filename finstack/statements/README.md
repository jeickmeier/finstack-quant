# finstack-statements

Period-based financial statement modeling: declarative formulas, forecasting, metric registries, and capital-structure integration.

Higher-level analysis (DCF, scenarios, scorecards, covenants) lives in `finstack-statements-analytics`.

## Usage notes

- Built-in metrics (`fin.*`) are embedded at compile time; no runtime `data/metrics` directory is required.
- Capital-structure formulas (`cs.*`) require `Evaluator::evaluate_with_market(&model, &market_ctx, as_of)`.
- Monte Carlo path evaluation is deterministic when using the same seed; non-finite path values fail during aggregation.

## Parallelism

Rayon-based path parallelism is always enabled on native targets (no Cargo features). Results match a serial run bit-for-bit given the same seed. WebAssembly builds omit Rayon.

## Module docs

- `src/lib.rs` — crate overview and quick start
- `src/dsl/mod.rs` — formula DSL operators and function reference
- `src/evaluator/mod.rs` — evaluation entry points and result conventions
- `src/capital_structure/mod.rs` — `cs.*` namespace and market-context evaluation
- `data/metrics/README.md` — built-in metric conventions

## Verification

```bash
cargo test -p finstack-statements
cargo bench -p finstack-statements --bench statements_operations --no-run
```

See `benches/README.md` for benchmark groups and `benches/BENCHMARKS.md` for production-scale workloads.
