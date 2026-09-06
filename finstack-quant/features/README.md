# finstack-quant-features

Vectorized panel feature transforms for Finstack Quant. The crate turns a flat
value column plus grouping keys into derived feature columns, either
backward-looking per entity (time-series) or partitioned per timestamp
(cross-sectional). Values are `Option<f64>`; `None` and non-finite inputs are
excluded from calculations. Rolling aggregates may still emit at a missing row
when its row window has enough finite observations; current-value transforms
(lag, returns, z-score, rank, filters, drawdown, EWMA, regression residuals)
require a finite current input. Fill and mask operations explicitly handle missing rows.

## Position in the stack

A bindings-facing leaf. Depends on `finstack-quant-core` and the workspace
version of `nalgebra` for scaled SVD regression (plus `serde` /
`serde_json` / `schemars` for the JSON entry points); no other domain crate
depends on it. Its only consumers are the umbrella crate, which re-exports it
as `finstack_quant::features`, and the two binding crates
`finstack-quant-py` and `finstack-quant-wasm`.

There is no financial-domain type here: no `Money`, no `Date`, no currency or
day-count handling. Keys are opaque `String`s sorted lexicographically, so
callers pass ISO-8601 dates (or any other lexicographically ordered key) as
strings. Errors are `finstack_quant_core::Error`; the crate defines no error
type of its own.

## Public API

| Function | Role |
|----------|------|
| `transform_timeseries` | Backward-looking transform per entity, ordered by a sortable key |
| `transform_timeseries_with_op` | Rust typed-op variant of `transform_timeseries` |
| `transform_cross_sectional` | Transform a value column across entities within each time partition |
| `transform_cross_sectional_with_op` | Rust typed-op variant of `transform_cross_sectional` |
| `transform_panel_json` | Apply a JSON-specified pipeline of named time-series and cross-sectional operations |
| `transform_panel` | Rust typed-spec variant of `transform_panel_json` with ordered result columns |

These entry points return `finstack_quant_core::Result`. Outputs preserve input
order and length; element `i` of the output corresponds to element `i` of
`values`. The string/JSON entry points are retained for Python and WASM
bindings. Rust callers can use `TimeSeriesOp`, `CrossSectionalOp`, `PairwiseOp`,
`PanelTransformSpec`, `PanelOperation`, `PanelTransformResult`, and
`PanelTransformColumn` to avoid string dispatch. Each op enum implements
`FromStr` for the canonical snake_case names accepted by the string entry
points.

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
| `rolling_skew` | `window` (1), `min_periods` (`window`) | Pandas / Fisher G1 skewness over the trailing window |
| `rolling_kurtosis` | `window` (1), `min_periods` (`window`) | Pandas / Fisher G2 excess kurtosis over the trailing window |
| `rolling_slope` | `window` (1), `min_periods` (`window`) | Linear trend slope per row position; missing rows retain their time gap |
| `rolling_sharpe` | `window` (1), `min_periods` (`window`), `risk_free` (0.0) | Period feature `(mean - risk_free) / sample_std`; not the annualized `analytics` Sharpe |
| `rolling_winsorize` | `window` (1), `min_periods` (`window`), `lower` (0.01), `upper` (0.99) | Clamp current value to trailing quantile bounds |
| `drawdown` | — | Current drawdown from the running peak |
| `hampel_filter` | `window` (1), `min_periods` (`window`), `threshold` (3.0) | Replace outliers with trailing median |
| `exponential_decay_weights` | `window` (1), `half_life` (required) | Current row's normalized exponential-decay weight |
| `ewma_mean` | `span` (required) | Pandas-span EWMA mean of a **return** series (`alpha = 2 / (span + 1)`) |
| `ewma_vol` | `span` (required) | Centered pandas-span EWMA volatility of a **return** series; first finite observation is `None` |
| `ewma_zscore` | `span` (required) | `(x - ewma_mean) / ewma_vol` on the same shared state; `0.0` when vol is missing |

Notes:

- `returns` and `log_returns` yield `None` for a zero prior value.
  Non-finite arithmetic results raise a validation error; they are not missing data.
- Rolling windows span rows and require `min_periods <= window`; a row is `None` until at least
  `min_periods` finite values are present. Some operations raise the effective
  minimum: `rolling_std`, `rolling_zscore`, `rolling_slope`, and
  `rolling_sharpe` require at least 2 finite points; `rolling_skew` requires
  at least 3 (Fisher G1); `rolling_kurtosis` requires at least 4 (Fisher G2).
  Zero-variance windows emit `0.0`.
- `drawdown` expects a positive level series (e.g. cumulative value); it reports
  `value / running_peak - 1` and yields `None` for non-positive inputs.
- `rolling_sharpe` is a period feature `(mean - risk_free) / sample_std` on a
  **return** series. It is not annualized and is not the `analytics`
  `rolling_sharpe` (annualized excess / vol). `risk_free` defaults to `0.0`
  in the same units as the return series.
- EWMA operations require a finite pandas `span >= 1` (`alpha = 2 /
  (span + 1)`), not a RiskMetrics `lambda`. Pass **returns**, not prices.
  `ewma_mean`, `ewma_vol`, and `ewma_zscore` share one `adjust=False`
  centered, biased variance recursion (`ignore_na=True`, `bias=True` for
  pandas variance comparisons); missing rows skip without decaying. The first finite observation has mean `x`, variance `0` (vol is
  `None`, z-score is `0.0`). After two finite observations, a constant series
  has volatility `0.0`; z-scores are zero when volatility is zero.

## Cross-sectional operations

`transform_cross_sectional(values, time_key, op, params)` partitions rows by
`time_key` and applies `op` independently within each partition (partitions are
processed in sorted-key order).

| `op` | Params (defaults) | Behavior |
|------|-------------------|----------|
| `zscore` | — | `(v - mean) / std` using the population std; `0.0` for constant values |
| `demean` | — | `v - mean` |
| `rank` | — | Percentile rank in `[0, 1]`; ties share the lowest rank; a single element maps to `0.0` |
| `percentile_rank` | — | Open-interval percentile rank using average tied positions |
| `quantile_bucket` | `buckets` (10) | Integer bucket label from `0` to `buckets - 1` |
| `robust_zscore` | — | Median/MAD z-score with normal-consistency scaling |
| `minmax_scale` | — | Scale finite values to `[0, 1]` within the partition |
| `clip` | `lower` (`-inf`), `upper` (`inf`) | Clamp to explicit value bounds |
| `clip_by_sigma` | `sigma` (3.0) | Clamp to `mean ± sigma * population_std` |
| `normal_score_transform` | — | Map open-interval percentile ranks to standard-normal scores |
| `long_short_weights` | — | Demean signal values and normalize by gross absolute exposure |
| `cap_weights` | `max_abs` (1.0) | Final absolute cap with zero net and unit gross exposure; infeasible caps fail |
| `fill_missing` | `value` (0.0) | Replace missing or non-finite values with a constant |
| `is_finite` | — | Emit `1.0` for finite inputs and `0.0` otherwise |
| `nan_mask` | — | Emit `1.0` for missing/non-finite inputs and `0.0` otherwise |
| `winsorize` | `lower` (0.01), `upper` (0.99) | Clamp to the linearly interpolated `lower`/`upper` sample quantiles |

`winsorize` requires `0 <= lower <= upper <= 1` and returns a validation error
otherwise. `cap_weights` requires `0 < max_abs <= 1`. It preserves the signs
of the demeaned signal, allocates gross exposure `0.5` to each side, and caps
positions before redistributing remaining exposure proportionally on that side.
Each side needs at least `ceil(0.5 / max_abs)` nonzero signals; otherwise the
call fails. A constant signal produces zero weights. Missing signals stay missing.
`hampel_filter` replaces a nonzero deviation from a zero-MAD median with that median.

## Multi-input and pipeline helpers

| Function | Role |
|----------|------|
| `transform_cross_sectional_grouped` | Apply a cross-sectional op within `(time_key, group)` sub-partitions |
| `transform_cross_sectional_grouped_with_op` | Typed-op variant of `transform_cross_sectional_grouped` |
| `neutralize` | Equal-weighted cross-sectional OLS residualization (`fit_intercept`, default `true`); fails if a date is singular or underdetermined |
| `transform_timeseries_pairwise` | Rolling covariance, correlation, and beta between two columns (`rolling_cov`, `rolling_corr`, `rolling_beta`) |
| `transform_timeseries_pairwise_with_op` | Typed-op variant of `transform_timeseries_pairwise` |
| `rolling_regression_residual` | Per-entity rolling OLS residuals; rank-deficient windows emit `None` (unlike `neutralize`) |
| `risk_scaled_weights` | Inverse-vol scale, demean, then gross-normalize so each cross-section is dollar-neutral |
| `rank_to_weights` | Convert ranks into gross-normalized long/short weights |
| `neutralize_and_zscore` | Residualize with an intercept, then cross-sectional z-score; `fit_intercept=false` fails |

Python additionally exposes `finstack_quant.features.dataframe`, a pure-Python
pandas convenience layer. These helpers accept a DataFrame plus key selectors and
return a `pd.Series` aligned to the input index (or a `pd.DataFrame` for
`panel`). A key selector can be a DataFrame column name, an index level name, or
an integer index level position. Cross-sectional `time_key` and time-series
`order` may be omitted when `df.index` is a `DatetimeIndex`; for `MultiIndex`
inputs, pass the relevant level name or position explicitly. If a selector is
both a column and an index level, the helper raises rather than guessing.

Date-like Python keys use one binding conversion for direct, typed-panel,
mapping-panel, and DataFrame calls: aware datetimes are normalized to UTC with
fixed nanosecond precision; naive datetimes retain wall time with the same
precision. A key column must not mix aware and naive datetimes. Strings remain
opaque; temporal strings must use a consistent timezone and precision. Python
panel values normalize NaN and infinities to missing, just like direct calls.

## Quick examples

### Time-series returns and rolling std

```rust
use finstack_quant_features::{transform_timeseries_with_op, TimeSeriesOp};
use serde_json::json;

fn example() -> finstack_quant_core::Result<()> {
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
Ok(())
}
```

### Cross-sectional rank and winsorize

```rust
use finstack_quant_features::{transform_cross_sectional_with_op, CrossSectionalOp};
use serde_json::json;

fn example() -> finstack_quant_core::Result<()> {
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
Ok(())
}
```

### JSON pipeline

`transform_panel_json` runs a list of named operations **sequentially**. Each
operation reads the previous column by default; set `input` to `"values"` to
branch from the raw column, or to an earlier operation name. The result is a
JSON object with a single `columns` array, one entry per operation in request
order, each carrying that operation's `name` and its output `values`.
`transform_panel` accepts the same model as Rust structs and returns the
equivalent `PanelTransformResult`, whose `get_column(name)` looks a column up
by name. `entity`/`order` are required for `timeseries` operations; `time_key`
is required for `cross_sectional` operations. Operation names must be unique,
non-empty, and must not be the reserved name `values`.

```rust
use finstack_quant_features::transform_panel_json;
use serde_json::json;

fn example() -> finstack_quant_core::Result<()> {
let spec = json!({
    "values": [10.0, 12.0, 20.0, 21.0],
    "entity": ["A", "A", "B", "B"],
    "order": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
    "time_key": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
    "operations": [
        {"name": "ret1", "family": "timeseries", "op": "returns", "params": {"periods": 1}},
        {"name": "rank", "family": "cross_sectional", "op": "rank", "input": "values"}
    ]
});

let result_json = transform_panel_json(&spec.to_string())?;
// result_json => {"columns": [{"name": "ret1", "values": [...]}, ...]}
let _ = result_json;
Ok(())
}
```

The spec uses `serde(deny_unknown_fields)`; unrecognized keys are rejected.

## Conventions

- Keys are opaque `String`s. Time order is lexicographic, so callers who want
  calendar order must pass ISO-8601 (or any other lexicographic clock).
- `periods` (`returns`, `log_returns`, `diff`, `lag`) counts **finite
  observations** (observation time): a missing row never advances the lag.
  `half_life` and EWMA `span` likewise only advance on finite rows. Rolling
  `window`s span the trailing `window` rows of the entity (not calendar days);
  only finite rows inside the window contribute and `min_periods` of them are
  required. Callers who need business-day half-lives must resample first.
  There is no calendar-aware window implementation.
- `params` are strict: any key an operation does not read is rejected with an
  error naming the accepted keys.
- Inputs are `Option<f64>`; `None` and non-finite values are treated as missing
  and are excluded. Rolling aggregates can emit at missing rows;
  current-value transforms require a finite current observation.
- Output length and ordering always match the input `values` column.
- Standard deviation is sample (Bessel-corrected) for `rolling_std` and
  population for cross-sectional `zscore`.
- Integer params (`periods`, `window`, `min_periods`) must be positive; `0` is a
  validation error.
- Dimensionless statistics use scaled calculations rather than an absolute
  variance cutoff. Exactly degenerate samples keep the documented zero result.
- OLS uses a scaled SVD design with a relative rank threshold; exact-fit residuals
  within solver roundoff are zeroed before standardization.
- `risk_scaled_weights` rejects negative volatility; zero or missing volatility
  gives missing weights. All volatility estimates must use the same horizon/units.
- Operation parameters are validated even for empty or unready inputs. Arithmetic
  failures raise validation errors in Rust, `ValueError` in Python, and
  `FinstackError` with `kind="validation"` in WASM, including JSON pipelines.
- `drawdown` takes a **level** series (`value / running_peak - 1`). The
  `analytics` drawdown takes **returns**.
- `rolling_sharpe` is a period feature `(mean - risk_free) / sample_std` on
  returns, not the annualized `analytics` / GIPS Sharpe. `risk_free` defaults
  to `0.0` in the same units as the return series.
- `transform_panel_json` is sequential. Use `input: "values"` to branch from the
  raw column.

## Bindings

- **Python** — string/JSON entry points under `finstack_quant.features`, plus
  the pure-Python `finstack_quant.features.dataframe` pandas layer described
  above. Both are declared in
  [`parity_contract.toml`](../../finstack-quant-py/parity_contract.toml).
- **WASM** — the same entry points in camelCase through the `features`
  namespace ([`exports/features.js`](../../finstack-quant-wasm/exports/features.js)):
  `transformTimeseries`, `transformCrossSectional`, `transformPanelJson`,
  `transformTimeseriesPairwise`, `transformCrossSectionalGrouped`,
  `neutralize`, `neutralizeAndZscore`,
  `rankToWeights`, `riskScaledWeights`, `rollingRegressionResidual`. JavaScript
  callers pass `number | null` arrays for values and plain objects for params.

The typed-op Rust variants (`*_with_op`) and `transform_panel` have no
host twin; both bindings go through the string/JSON entry points.

## Related

- `finstack-quant-core` — provides `Error`/`Result` used for validation failures.
- `finstack-quant` — re-exports this crate as `finstack_quant::features`.

## Tests

[`tests/transforms.rs`](tests/transforms.rs) is the single integration suite; it
covers every op in the tables above plus the validation errors. Criterion
targets live under [`benches/`](benches).

## Verification

```bash
cargo clippy -p finstack-quant-features --lib --bins --tests --examples --all-features -- -D warnings
cargo nextest run -p finstack-quant-features --lib --test '*'
cargo nextest run -p finstack-quant-wasm --lib --test dts_contract \
  -E 'test(features_dts_matches_transform_surface)'
```

Workspace gates (`mise run rust-lint`, `mise run rust-test`, `mise run rust-doc`
— the last one runs doctests) are what CI enforces. Use `cargo nextest`, not
`cargo test`, for crate-scoped runs; see
[`CONTRIBUTING.md`](../../CONTRIBUTING.md).
