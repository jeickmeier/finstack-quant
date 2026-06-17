# finstack-quant-features

Vectorized panel feature transforms for Finstack Quant. The crate turns a flat
value column plus grouping keys into derived feature columns, either
backward-looking per entity (time-series) or partitioned per timestamp
(cross-sectional). Values are `Option<f64>`; `None` and non-finite inputs are
skipped and produce `None` outputs, so callers can carry missing data through a
pipeline without sentinel values.

## Public API

| Function | Role |
|----------|------|
| `transform_timeseries` | Backward-looking transform per entity, ordered by a sortable key |
| `transform_timeseries_with_op` | Rust typed-op variant of `transform_timeseries` |
| `transform_cross_sectional` | Transform a value column across entities within each time partition |
| `transform_cross_sectional_with_op` | Rust typed-op variant of `transform_cross_sectional` |
| `transform_panel` | Apply a JSON-specified pipeline of named time-series and cross-sectional operations |
| `transform_panel_spec` | Rust typed-spec variant of `transform_panel` with ordered result columns |

All three return `finstack_quant_core::Result`. Outputs preserve input order and
length; element `i` of the output corresponds to element `i` of `values`.
The string/JSON entrypoints are retained for Python and WASM bindings. Rust
callers can use `TimeSeriesOp`, `CrossSectionalOp`, `PairwiseOp`,
`PanelTransformSpec`, `PanelOperation`, and `PanelTransformResult` to avoid
string dispatch.

## Time-series operations

`transform_timeseries(values, entity, order, op, params)` groups rows by
`entity`, sorts each group by `order` (then by input index as a stable
tie-break), and applies `op` within the group.

| `op` | Params (defaults) | Behavior |
|------|-------------------|----------|
| `returns` | `periods` (1) | Simple return `v_t / v_{t-periods} - 1` |
| `log_returns` | `periods` (1) | `ln(v_t / v_{t-periods})`; `None` when the ratio is not positive |
| `diff` | `periods` (1) | Difference `v_t - v_{t-periods}` |
| `lag` | `periods` (1) | Value shifted forward by `periods` |
| `rolling_mean` | `window` (1), `min_periods` (`window`) | Mean over the trailing window |
| `rolling_sum` | `window` (1), `min_periods` (`window`) | Sum over the trailing window |
| `rolling_std` | `window` (1), `min_periods` (`window`) | Sample (Bessel-corrected) std; requires at least 2 finite points |
| `rolling_min` | `window` (1), `min_periods` (`window`) | Minimum over the trailing window |
| `rolling_max` | `window` (1), `min_periods` (`window`) | Maximum over the trailing window |
| `rolling_zscore` | `window` (1), `min_periods` (`window`) | Current value z-score against the trailing window |
| `rolling_rank` | `window` (1), `min_periods` (`window`) | Current value percentile rank against the trailing window |
| `rolling_quantile` | `window` (1), `min_periods` (`window`), `quantile` (0.5) | Quantile over the trailing window |
| `rolling_skew` | `window` (1), `min_periods` (`window`) | Skewness over the trailing window |
| `rolling_kurtosis` | `window` (1), `min_periods` (`window`) | Excess kurtosis over the trailing window |
| `rolling_slope` | `window` (1), `min_periods` (`window`) | Linear trend slope over the trailing window |
| `rolling_sharpe` | `window` (1), `min_periods` (`window`) | Mean divided by sample std over the trailing window |
| `rolling_winsorize` | `window` (1), `min_periods` (`window`), `lower` (0.01), `upper` (0.99) | Clamp current value to trailing quantile bounds |
| `drawdown` | — | Current drawdown from the running peak |
| `hampel_filter` | `window` (1), `min_periods` (`window`), `threshold` (3.0) | Replace outliers with trailing median |
| `exponential_decay_weights` | `window` (1), `half_life` (required) | Current row's normalized exponential-decay weight |
| `ewma_mean` | `span` (required) | Exponentially weighted mean with `alpha = 2 / (span + 1)` |
| `ewma_vol` | `span` (required) | Exponentially weighted volatility of the input series |
| `ewma_zscore` | `span` (required) | Current value z-score against EWMA mean/variance state |

Notes:

- `returns` and `log_returns` yield `None` when the prior value's magnitude is
  at or below `1e-12`, avoiding division by (near-)zero.
- Rolling windows count only finite points; a row is `None` until at least
  `min_periods` finite values are present. Some operations raise the effective
  minimum: `rolling_std`, `rolling_zscore`, `rolling_slope`, and
  `rolling_sharpe` require at least 2 finite points; `rolling_skew` and
  `rolling_kurtosis` require at least 3.
- `drawdown` expects a positive level series (e.g. cumulative value); it reports
  `value / running_peak - 1` and yields `None` for non-positive inputs.
- EWMA operations require a finite, positive `span` and carry state across only
  the finite observations within an entity.

## Cross-sectional operations

`transform_cross_sectional(values, time_key, op, params)` partitions rows by
`time_key` and applies `op` independently within each partition (partitions are
processed in sorted-key order).

| `op` | Params (defaults) | Behavior |
|------|-------------------|----------|
| `zscore` | — | `(v - mean) / std` using the population std; `0.0` when std is at or below `1e-12` |
| `demean` | — | `v - mean` |
| `rank` | — | Percentile rank in `[0, 1]`; ties share the lowest rank; a single element maps to `0.0` |
| `percentile_rank` | — | Open-interval percentile rank using average tied positions |
| `quantile_bucket` | `buckets` (10) | Integer bucket label from `0` to `buckets - 1` |
| `robust_zscore` | — | Median/MAD z-score with normal-consistency scaling |
| `minmax_scale` | — | Scale finite values to `[0, 1]` within the partition |
| `clip` | `lower` (`-inf`), `upper` (`inf`) | Clamp to explicit value bounds |
| `clip_by_sigma` | `sigma` (3.0) | Clamp to `mean ± sigma * population_std` |
| `clip_by_quantile` | `lower` (0.01), `upper` (0.99) | Alias for `winsorize` |
| `normal_score_transform` | — | Map open-interval percentile ranks to standard-normal scores |
| `long_short_weights` | — | Demean signal values and normalize by gross absolute exposure |
| `dollar_neutral_weights` | — | Alias for dollar-neutral long/short weights |
| `cap_weights` | `max_abs` (1.0) | Cap centered weights before gross normalization |
| `fill_missing` | `value` (0.0) | Replace missing or non-finite values with a constant |
| `is_finite` | — | Emit `1.0` for finite inputs and `0.0` otherwise |
| `nan_mask` | — | Emit `1.0` for missing/non-finite inputs and `0.0` otherwise |
| `winsorize` | `lower` (0.01), `upper` (0.99) | Clamp to the linearly interpolated `lower`/`upper` sample quantiles |

`winsorize` requires `0 <= lower <= upper <= 1` and returns a validation error
otherwise.

## Multi-input and pipeline helpers

| Function | Role |
|----------|------|
| `transform_cross_sectional_grouped` | Apply a cross-sectional op within `(time_key, group)` sub-partitions |
| `neutralize` | Cross-sectional OLS residualization against exposure columns |
| `transform_timeseries_pairwise` | Rolling covariance, correlation, and beta between two columns |
| `rolling_regression_residual` | Per-entity rolling OLS residuals against exposure columns |
| `risk_scaled_weights` | Convert signal values into inverse-volatility-scaled weights |
| `clean_signal` | Default cross-sectional signal cleaning via quantile clipping |
| `normalize_signal` | Normalize with a selected cross-sectional op (`method`, default `zscore`) |
| `rank_to_weights` | Convert ranks into gross-normalized long/short weights |
| `neutralize_and_zscore` | Residualize against exposures, then cross-sectional z-score |

Python additionally exposes `finstack_quant.features.dataframe`, a pure-Python
pandas convenience layer. These helpers accept a DataFrame plus key selectors and
return a `pd.Series` aligned to the input index (or a `pd.DataFrame` for
`panel`). A key selector can be a DataFrame column name, an index level name, or
an integer index level position. Cross-sectional `time_key` and time-series
`order` may be omitted when `df.index` is a `DatetimeIndex`; for `MultiIndex`
inputs, pass the relevant level name or position explicitly. If a selector is
both a column and an index level, the helper raises rather than guessing.

## Quick examples

### Time-series returns and rolling std

```rust,no_run
use finstack_quant_features::{transform_timeseries_with_op, TimeSeriesOp};
use serde_json::json;

# fn main() -> finstack_quant_core::Result<()> {
let values = vec![Some(12.0), Some(10.0), Some(21.0), Some(20.0)];
let entity = vec!["A".into(), "A".into(), "B".into(), "B".into()];
let order = vec![
    "2026-01-02".into(),
    "2026-01-01".into(),
    "2026-01-02".into(),
    "2026-01-01".into(),
];

let returns = transform_timeseries_with_op(
    &values,
    &entity,
    &order,
    TimeSeriesOp::Returns,
    Some(&json!({"periods": 1})),
)?;
let rolling_std = transform_timeseries_with_op(
    &values,
    &entity,
    &order,
    TimeSeriesOp::RollingStd,
    Some(&json!({"window": 2, "min_periods": 2})),
)?;
assert_eq!(returns.len(), values.len());
assert_eq!(rolling_std.len(), values.len());
# Ok(())
# }
```

### Cross-sectional rank and winsorize

```rust,no_run
use finstack_quant_features::{transform_cross_sectional_with_op, CrossSectionalOp};
use serde_json::json;

# fn main() -> finstack_quant_core::Result<()> {
let values = vec![Some(1.0), Some(2.0), Some(100.0), Some(5.0)];
let time_key = vec![
    "2026-01-01".into(),
    "2026-01-01".into(),
    "2026-01-01".into(),
    "2026-01-02".into(),
];

let _ranks = transform_cross_sectional_with_op(&values, &time_key, CrossSectionalOp::Rank, None)?;
let _winsorized = transform_cross_sectional_with_op(
    &values,
    &time_key,
    CrossSectionalOp::Winsorize,
    Some(&json!({"lower": 0.0, "upper": 0.5})),
)?;
# Ok(())
# }
```

### JSON pipeline

`transform_panel` runs a list of named operations against one shared `values`
column and returns a JSON object mapping each operation `name` to its output
column. `transform_panel_spec` accepts the same model as Rust structs and
returns ordered `PanelTransformColumn` values. `entity`/`order` are required for
`timeseries` operations; `time_key` is required for `cross_sectional`
operations. Operation names must be unique and non-empty.

```rust,no_run
use finstack_quant_features::transform_panel;
use serde_json::json;

# fn main() -> finstack_quant_core::Result<()> {
let spec = json!({
    "values": [10.0, 12.0, 20.0, 21.0],
    "entity": ["A", "A", "B", "B"],
    "order": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
    "time_key": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
    "operations": [
        {"name": "ret1", "family": "timeseries", "op": "returns", "params": {"periods": 1}},
        {"name": "rank", "family": "cross_sectional", "op": "rank"}
    ]
});

let result_json = transform_panel(&spec.to_string())?;
// result_json => {"columns": {"ret1": [...], "rank": [...]}}
let _ = result_json;
# Ok(())
# }
```

The spec uses `serde(deny_unknown_fields)`; unrecognized keys are rejected.

## Conventions

- Inputs are `Option<f64>`; `None` and non-finite values are treated as missing
  and pass through as `None`.
- Output length and ordering always match the input `values` column.
- Standard deviation is sample (Bessel-corrected) for `rolling_std` and
  population for cross-sectional `zscore`.
- Integer params (`periods`, `window`, `min_periods`) must be positive; `0` is a
  validation error.
- The zero-denominator and zero-variance tolerance is `1e-12`.

## Related

- `finstack-quant-core` — provides `Error`/`Result` used for validation failures.
- `finstack-quant` — re-exports this crate as `finstack_quant::features`.
- `finstack-quant-py` — exposes these functions under the `features` Python
  submodule (`transform_timeseries`, `transform_cross_sectional`,
  `transform_panel`).
- `finstack-quant-wasm` — exposes the same functions through the `features`
  namespace using camelCase names (`transformTimeseries`,
  `transformCrossSectional`, `transformPanel`). JavaScript callers pass
  `number | null` arrays for values and plain objects for params.

## Verification

```bash
cargo test -p finstack-quant-features
cargo test -p finstack-quant-wasm --test dts_contract features_dts_matches_transform_surface
```
