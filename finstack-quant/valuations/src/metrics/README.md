# Metrics

Risk and analytics measures computed on demand, separate from core NPV pricing.
Calculators run through `Instrument::price_with_metrics` or directly against a
`MetricRegistry`, and their outputs land in `ValuationResult::measures`.

## Layout

```
metrics/
├── core/           # MetricId + ids/, MetricCalculator, MetricContext,
│                   # MetricRegistry, standard_registry, finite differences
├── sensitivities/  # DV01, CS01 (curve and Z-spread), vega, theta, FD Greeks,
│                   # FX01, carry decomposition, breakeven, cross-factor
├── risk/           # Historical VaR / expected shortfall, market history,
│                   # risk-factor extraction
└── shared/         # Cross-instrument calculators driven by trait metadata
```

`core` and `shared` are private modules; `sensitivities` is `pub(crate)`.
Everything supported is re-exported from `crate::metrics`. `risk` is public.

## Public surface

| Item | What it is |
|------|------------|
| `MetricId` | Strongly typed metric name; also the key type of `ValuationResult::measures` |
| `MetricGroup` | Ten logical groups covering every standard metric |
| `MetricCalculator` | `fn calculate(&self, &mut MetricContext) -> Result<f64>` plus `dependencies()` / `dynamic_dependencies()` |
| `MetricContext` | Instrument, `MarketContext`, `as_of`, base PV, and the `computed` / series / matrix caches |
| `MetricRegistry` | Calculator store with dependency resolution and strict, fail-fast errors |
| `standard_registry()` | Process-wide registry with all built-in calculators registered |
| `Structured2D` | Row/column labelled matrix for 2-D metrics |
| `bump_surface_vol_absolute` | Absolute vol-point surface bump helper |
| `STANDARD_BUCKETS_YEARS`, `STANDARD_BUCKET_LABELS`, `format_bucket_label` | Canonical key-rate bucket grid and labels |
| `CrossFactorCalculator`, `CrossFactorPair` | Cross-gamma style two-factor sensitivities |
| `collect_cashflows_in_period` | Theta helper for period cashflow collection |
| `risk::{calculate_var, calculate_var_with_pricing, VarConfig, VarMethod, VarResult}` | Historical VaR entry points |
| `risk::{GenericHVar, GenericExpectedShortfall, MarketHistory, MarketScenario, RiskFactorShift}` | Scenario inputs and estimators |
| `risk::{extract_risk_factors, RiskFactorType}` | Risk-factor extraction from a market context |

## MetricId

Standard IDs live in [`core/ids/`](core/ids/) and are enumerated by
`MetricId::ALL_STANDARD` — **209 metrics** partitioned across the ten
`MetricGroup` values (`Pricing`, `Carry`, `Sensitivity`, `Greeks`, `Credit`,
`Rates`, `Fx`, `Equity`, `StructuredCredit`, `Alternatives`). Every standard
metric belongs to exactly one group, and the union of the group slices equals
`ALL_STANDARD` (enforced by a test in `core/ids/tests.rs`).

Units, sign conventions, and bump definitions are documented per-ID in
`core/ids/` and in the instrument-specific metric modules. Measures are raw
`f64` but are not unitless: some are currency amounts, some currency-per-bump,
some decimal rates, some ratios or counts. Always read a value through its
`MetricId` contract.

Custom IDs (`MetricId::custom("dv01::USD-OIS")`) and composite IDs
(`MetricId::composite`) are for caller-owned bucket keys; they are not part of
the grouped cross-language contract.

Discovery:

- Rust: `MetricGroup::ALL`, `MetricGroup::metrics()`
- Registry: `MetricRegistry::available_metrics_grouped()` — registered standard
  metrics only, deterministic and sorted within each group
- Python: `finstack_quant.valuations.instruments.list_standard_metrics_grouped()`

At API boundaries, parse user-supplied names with `MetricId::parse_strict`,
which rejects anything not in `ALL_STANDARD`.

## Dependencies

The registry topologically sorts calculators so dependencies run first (for
example YTM before Macaulay duration). Results land in `context.computed` for
downstream calculators to read.

A calculator whose dependency set varies at runtime — because it reads a
pricing override, say — must implement `dynamic_dependencies`, not just
`dependencies`. The registry only calls `dynamic_dependencies` when building the
computation order, so a config-dependent input declared only in `dependencies`
will be missing whenever the caller did not request it earlier in the list.

## Bucketed metrics

- 1-D: `MetricContext::store_bucketed_series` — key-rate DV01, bucketed CS01.
  Read back with `get_series`.
- 2-D: `MetricContext::store_matrix2d` — vega by expiry x strike. Read back with
  `get_matrix2d`; shape mismatches return a `Validation` error.

Both also flatten every cell into `computed` under a stable composite key built
by `MetricId::composite` — `base::bucket` for a series, `base::row::col` for a
matrix. Labels are written literally (`bucketed_dv01::USD-OIS::10y`, `pv01::USD-OIS`); only the `::` delimiter, a literal `_x` and the reserved `_empty` spelling are escaped as `_xHH`. The flattened cells
are custom `MetricId`s, so they survive into `ValuationResult::measures`
alongside the scalar metrics and can be read back with
`ValuationResult::metric_series`.

Bucketed CS01 and DV01 require strictly increasing key-rate grids; duplicate or
unsorted tenors return `Validation` errors. Bucket totals use Neumaier
compensated summation so parallel totals reconcile to the scalar metric where
the implementation defines that invariant.

## Finite differences

`core/finite_difference.rs` is crate-private and holds the curve and scalar
bump helpers plus `CORRELATION_BUMP`. Spot, volatility, rate, credit-spread and
yield bump sizes are never hard-coded: calculators resolve them with
`sensitivities::config::resolve(context)`, which layers
`metric_pricing_overrides.bump_config` over the `valuations.sensitivities.v1`
config extension. Option-greek providers receive them as `GreekBumps`. The only
public item from `finite_difference.rs` is `bump_surface_vol_absolute`.

## Adding a metric

1. Add the `MetricId` constant in `core/ids/`, assign it to a `MetricGroup`, and
   add it to `ALL_STANDARD` when it belongs to the cross-language contract.
   Group ranges in `core/ids/group.rs` are index ranges into `ALL_STANDARD` and
   must be updated together with it.
2. Implement `MetricCalculator`, usually under
   `../instruments/<asset_class>/<instrument>/metrics/`.
3. Register it in that instrument's `register_<name>_metrics` function, which is
   called from `core/standard_registry.rs`.
4. Add tests next to the calculator or under `../../tests/metrics/`.

## Verification

```bash
cargo nextest run -p finstack-quant-valuations --test metrics
cargo bench -p finstack-quant-valuations --bench metrics
cargo bench -p finstack-quant-valuations --bench bucketed_risk
```

## Related

- [`../results/README.md`](../results/README.md) — `ValuationResult::measures`
- [`../instruments/README.md`](../instruments/README.md) — pricing entry points
- [`../calibration/README.md`](../calibration/README.md) — recalibration bumps
  used by CS01, key-rate DV01, and vega
