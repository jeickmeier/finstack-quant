// Generated from the finstack-quant-features JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

/**
 * Supported cross-sectional transform operation.
 */
export type CrossSectionalOp =
  | "zscore"
  | "rank"
  | "percentile_rank"
  | "quantile_bucket"
  | "demean"
  | "robust_zscore"
  | "minmax_scale"
  | "clip"
  | "clip_by_sigma"
  | "normal_score_transform"
  | "long_short_weights"
  | "cap_weights"
  | "fill_missing"
  | "is_finite"
  | "nan_mask"
  | "winsorize";
/**
 * Supported pairwise rolling time-series transform operation.
 */
export type PairwiseOp = "rolling_cov" | "rolling_corr" | "rolling_beta";
/**
 * A named panel transform operation.
 */
export type PanelOperation =
  | {
      family: "timeseries";
      /**
       * Source column. `None` (default) uses the previous operation output,
       * or the raw `values` column for the first operation. May name
       * `values` or an already evaluated column; forward references fail.
       */
      input?: string | null;
      /**
       * Output column name. Must not be the reserved name `values`.
       */
      name: string;
      /**
       * Operation to evaluate.
       */
      op: TimeSeriesOp;
      /**
       * Optional operation parameters.
       */
      params?: {
        [k: string]: unknown;
      };
    }
  | {
      family: "cross_sectional";
      /**
       * Source column. `None` (default) uses the previous operation output,
       * or the raw `values` column for the first operation. May name
       * `values` or an already evaluated column; forward references fail.
       */
      input?: string | null;
      /**
       * Output column name. Must not be the reserved name `values`.
       */
      name: string;
      /**
       * Operation to evaluate.
       */
      op: CrossSectionalOp;
      /**
       * Optional operation parameters.
       */
      params?: {
        [k: string]: unknown;
      };
    };
/**
 * Supported backward-looking time-series transform operation.
 */
export type TimeSeriesOp =
  | "returns"
  | "log_returns"
  | "diff"
  | "lag"
  | "rolling_mean"
  | "rolling_sum"
  | "rolling_std"
  | "rolling_min"
  | "rolling_max"
  | "rolling_zscore"
  | "rolling_rank"
  | "rolling_quantile"
  | "rolling_skew"
  | "rolling_kurtosis"
  | "rolling_slope"
  | "rolling_sharpe"
  | "rolling_winsorize"
  | "drawdown"
  | "hampel_filter"
  | "exponential_decay_weights"
  | "ewma_mean"
  | "ewma_vol"
  | "ewma_zscore";

/**
 * A named output column from a panel transform pipeline.
 */
export interface PanelTransformColumn {
  /**
   * Output column name.
   */
  name: string;
  /**
   * Output values aligned to the input `values` column.
   */
  values: (number | null)[];
}
/**
 * Ordered result columns from a panel transform pipeline.
 */
export interface PanelTransformResult {
  /**
   * Output columns in the same order as requested operations.
   */
  columns: PanelTransformColumn[];
}
/**
 * Specification for a panel transform pipeline.
 */
export interface PanelTransformSpec {
  /**
   * Entity key for time-series operations.
   */
  entity?: string[] | null;
  /**
   * Ordered operations evaluated sequentially; each reads the previous
   * column unless `input` selects `values` or an earlier named column.
   */
  operations: PanelOperation[];
  /**
   * Lexicographic order key for time-series operations.
   */
  order?: string[] | null;
  /**
   * Partition key for cross-sectional operations.
   */
  time_key?: string[] | null;
  /**
   * Input numeric value column. `None` represents missing data.
   */
  values: (number | null)[];
}
