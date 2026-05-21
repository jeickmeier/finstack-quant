# Metrics

Trait-based risk and analytics metrics, separate from core NPV pricing. Calculators run on demand through `price_with_metrics` or the metrics registry.

## Layout

```
metrics/
├── core/           # MetricId, MetricCalculator, MetricRegistry, finite differences
├── sensitivities/  # DV01, CS01, vega, theta, FD Greeks
├── risk/           # HVaR, expected shortfall, market history
└── shared/         # Cross-instrument helpers (e.g. discount factors)
```

## Core types

**`MetricCalculator`** — `calculate(&mut MetricContext) -> Result<f64>` plus optional `dependencies()`.

**`MetricContext`** — instrument, `MarketContext`, `as_of`, base PV, and caches (`computed`, bucketed series/matrices, cashflows).

**`MetricId`** — strongly typed metric names. Standard IDs live in [`core/ids.rs`](core/ids.rs) as `MetricId::ALL_STANDARD` (200 metrics across 10 `MetricGroup` values). Custom IDs (`MetricId::custom("dv01::USD-OIS")`) are for caller-owned bucket keys and are not part of the grouped contract.

Discovery:

- Rust: `MetricGroup::all_with_metrics()`
- Python: `list_standard_metrics_grouped()` (parity contract)
- Registry: `MetricRegistry::available_metrics_grouped()` — registered standard metrics only, sorted within each group

At API boundaries, parse user-supplied standard names with `MetricId::parse_strict`. Units, sign conventions, and bump definitions are documented on each `MetricId` in `core/ids.rs` and in instrument-specific metric modules.

## Dependencies

The registry topologically sorts calculators so dependencies (for example YTM before Macaulay duration) run first. Results land in `context.computed` for downstream calculators.

## Bucketed metrics

- **1D**: `store_bucketed_series` — key-rate DV01, bucketed CS01, etc.
- **2D**: `store_matrix` — vega by expiry × strike

Parallel bucket totals should reconcile to the scalar metric where the implementation defines that invariant.

## Adding a metric

1. Add `MetricId` constant in `core/ids.rs` and include it in `ALL_STANDARD` when it is part of the cross-language contract.
2. Implement `MetricCalculator` (often under `instruments/<type>/metrics/`).
3. Register in the instrument’s `register_*_metrics` function.
4. Add tests next to the calculator or under `tests/metrics/`.

## Finite differences

`core/finite_difference.rs` defines standard bump sizes (`bump_sizes`) and helpers for curves and scalars. Use the same bumps as documented on the metric to keep PV and risk consistent.

## Related

- [`../results/README.md`](../results/README.md) — `ValuationResult::measures`
- [`../instruments/README.md`](../instruments/README.md) — pricing entry points
