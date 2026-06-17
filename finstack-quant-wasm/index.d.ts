// Type declarations for the finstack-quant-wasm namespaced facade.
// Shapes follow `wasm-bindgen` JS names in `src/api/**` (see Rust `js_name`).
// The raw `pkg/finstack_quant_wasm.d.ts` emitted by wasm-bindgen is intentionally
// not the package root contract: it exposes a flat module, while `index.js`
// publishes a namespaced facade. Keep this file as the facade declaration and
// use generated `types/generated/*` files only for JSON envelope shapes.
//
// Building a MarketContext from quotes (canonical path):
//
//   import { valuations } from 'finstack-quant-wasm/exports/valuations.js';
//   import type { CalibrationEnvelope } from 'finstack-quant-wasm';
//   const envelope: CalibrationEnvelope = {
//     schema: 'finstack_quant.calibration',
//     plan: { id: 'usd_curves', quote_sets: {...}, steps: [...], settings: {} },
//     market_data: [],   // flat id-addressable quotes/snapshots
//     prior_market: [],  // optional pre-built curves/surfaces
//   };
//   const result = valuations.calibrate(envelope);  // CalibrationResultEnvelope
//   const marketJson = JSON.stringify(result.result.final_market);
//
// `result.result.final_market` is the materialized MarketContextState ready
// for any downstream pricing / scenario / attribution call that takes a
// market_json argument. Always check the per-step report
// (`result.result.step_reports`) and the plan summary
// (`result.result.report`) to confirm the curves actually fit before using
// the market downstream.
//
// `validateCalibrationJson` is a fast pre-flight check that canonicalizes
// the envelope without solving — use it to surface schema errors early.
//
// Phase 4 diagnostics: errors thrown by `calibrate`,
// `validateCalibrationJson`, `dryRun`, and `dependencyGraphJson` have:
//   - name: 'CalibrationEnvelopeError'
//   - cause: structured EnvelopeError payload (object with `kind` etc.)
// Standard try/catch exposes both via `e.name` and `e.cause`.
//
// WASM ownership: classes with a `free(): void` method own wasm heap memory.
// Call `free()` when a long-lived handle is no longer needed, especially for
// `Performance`, credit factor hierarchy handles, and `Portfolio`. Plain JSON
// result objects, arrays, and namespace functions do not need manual disposal.

export { default } from './pkg/finstack_quant_wasm';

// --- Calibration envelope types (generated from Rust via ts-rs) ---
import type { CalibrationEnvelope } from './types/generated/CalibrationEnvelope';
import type { CalibrationResultEnvelope } from './types/generated/CalibrationResultEnvelope';

export type { CalibrationEnvelope, CalibrationResultEnvelope };
export type { CalibrationPlan } from './types/generated/CalibrationPlan';
export type { CalibrationStep } from './types/generated/CalibrationStep';
export type { StepParams } from './types/generated/StepParams';
export type { MarketDatum } from './types/generated/MarketDatum';
export type { PriorMarketObject } from './types/generated/PriorMarketObject';
export type { CalibrationResult } from './types/generated/CalibrationResult';
export type { CalibrationReport } from './types/generated/CalibrationReport';

// --- core -----------------------------------------------------------------

export interface Currency {
  readonly code: string;
  readonly numeric: number;
  readonly decimals: number;
  toString(): string;
  toJson(): string;
}

export interface CurrencyConstructor {
  new (code: string): Currency;
  fromJson(json: string): Currency;
}

export interface Money {
  readonly amount: number;
  readonly currency: Currency;
  /** Lossless amount as a decimal string (exact Rust Decimal rendering). */
  amountDecimal(): string;
  add(other: Money): Money;
  sub(other: Money): Money;
  mulScalar(factor: number): Money;
  divScalar(divisor: number): Money;
  negate(): Money;
  toString(): string;
}

export interface MoneyConstructor {
  new (amount: number, currency: Currency): Money;
}

export interface Rate {
  readonly asDecimal: number;
  readonly asPercent: number;
  readonly asBps: number;
}

export interface RateConstructor {
  new (decimal: number): Rate;
  fromPercent(pct: number): Rate;
  fromBps(bps: number): Rate;
}

export interface Bps {
  asDecimal(): number;
  asBps(): number;
}

export interface BpsConstructor {
  new (value: number): Bps;
}

export interface Percentage {
  asDecimal(): number;
  asPercent(): number;
}

export interface PercentageConstructor {
  new (value: number): Percentage;
}

export interface DayCount {
  yearFraction(startEpochDays: number, endEpochDays: number): number;
  yearFractionWithContext(
    startEpochDays: number,
    endEpochDays: number,
    ctx: DayCountContext
  ): number;
  calendarDays(startEpochDays: number, endEpochDays: number): number;
  toString(): string;
}

export interface DayCountConstructor {
  new (name: string): DayCount;
  act360(): DayCount;
  act365f(): DayCount;
  thirty360(): DayCount;
  thirtyE360(): DayCount;
  actAct(): DayCount;
  actActIsma(): DayCount;
  bus252(): DayCount;
}

export interface DayCountContext {
  withCalendar(calendarCode: string): DayCountContext;
  withFrequency(frequency: Tenor): DayCountContext;
  withBusBasis(busBasis: number): DayCountContext;
  /** Reference coupon period (epoch days) for Act/Act ICMA. */
  withCouponPeriod(startEpochDays: number, endEpochDays: number): DayCountContext;
}

export interface DayCountContextConstructor {
  new (): DayCountContext;
}

export interface Tenor {
  readonly count: number;
  toYearsSimple(): number;
  toString(): string;
}

export interface TenorConstructor {
  new (s: string): Tenor;
  daily(): Tenor;
  weekly(): Tenor;
  monthly(): Tenor;
  quarterly(): Tenor;
  semiAnnual(): Tenor;
  annual(): Tenor;
}

export interface DiscountCurve {
  readonly id: string;
  readonly baseDate: string;
  df(t: number): number;
  zero(t: number): number;
  forwardRate(t1: number, t2: number): number;
}

export interface DiscountCurveConstructor {
  new (
    id: string,
    baseDate: string,
    knots: number[],
    interp?: string,
    extrapolation?: string,
    dayCount?: string
  ): DiscountCurve;
}

export interface ForwardCurve {
  readonly id: string;
  readonly baseDate: string;
  rate(t: number): number;
}

export interface ForwardCurveConstructor {
  new (
    id: string,
    tenor: number,
    baseDate: string,
    knots: number[],
    dayCount?: string,
    interp?: string,
    extrapolation?: string
  ): ForwardCurve;
}

export interface VolCube {
  readonly id: string;
  vol(expiry: number, tenor: number, strike: number): number;
  volClamped(expiry: number, tenor: number, strike: number): number;
  /**
   * Normal (Bachelier) implied vol in absolute rate units
   * (e.g. 0.008 = 80 bp/yr). Throws if expiry/tenor falls outside the
   * grid or for cross-zero quotes ((F+s)(K+s) <= 0) with beta > 0,
   * which require an explicit shift.
   */
  volNormal(expiry: number, tenor: number, strike: number): number;
  /**
   * Normal (Bachelier) implied vol with clamped extrapolation; never
   * throws and never returns a non-finite or non-positive value.
   */
  volNormalClamped(expiry: number, tenor: number, strike: number): number;
}

export interface VolCubeConstructor {
  new (
    id: string,
    expiries: number[],
    tenors: number[],
    paramsFlat: number[],
    forwards: number[]
  ): VolCube;
}

export interface FxConversionPolicy {
  toString(): string;
}

export interface FxConversionPolicyConstructor {
  cashflowDate(): FxConversionPolicy;
  periodEnd(): FxConversionPolicy;
  periodAverage(): FxConversionPolicy;
  custom(): FxConversionPolicy;
  fromName(name: string): FxConversionPolicy;
}

export interface FxRateResult {
  readonly rate: number;
  readonly triangulated: boolean;
}

/** `FxRateResult` has no public constructor; instances come from `FxMatrix.rate`. */
export interface FxRateResultConstructor {
  readonly prototype: FxRateResult;
}

export interface FxMatrix {
  setQuote(base: string, quote: string, rate: number): void;
  rate(base: string, quote: string, date: string, policy?: FxConversionPolicy): FxRateResult;
}

export interface FxMatrixConstructor {
  new (): FxMatrix;
}

/** FX vol surface quoted in delta space (ATM, 25-delta RR/BF, optional 10-delta wings). */
export interface FxDeltaVolSurface {
  readonly id: string;
  readonly expiries: number[];
  readonly numExpiries: number;
  /** Pillar vols at an expiry index as `[atm, put25dVol, call25dVol]`. */
  pillarVols(expiryIdx: number): number[];
  impliedVol(expiry: number, strike: number, forward: number, rD: number, rF: number): number;
}

export interface FxDeltaVolSurfaceConstructor {
  new (
    id: string,
    expiries: number[],
    atmVols: number[],
    rr25d: number[],
    bf25d: number[],
    rr10d?: number[],
    bf10d?: number[]
  ): FxDeltaVolSurface;
  /** Convert a forward delta to a strike (Garman-Kohlhagen, premium-unadjusted). */
  deltaToStrike(delta: number, forward: number, vol: number, expiry: number, rF: number): number;
  /** Convert a strike to forward (call) delta. */
  strikeToDelta(strike: number, forward: number, vol: number, expiry: number, rF: number): number;
}

/** Monte Carlo pricer result (JSON object from Rust). */
export interface MonteCarloEstimateJson {
  mean: number;
  currency: string;
  stderr: number;
  /** Sample standard deviation (absent when not computed). */
  std_dev?: number;
  ci_lower: number;
  ci_upper: number;
  /** Number of independent path estimators; equals `num_simulated_paths` without variance reduction, half of it with antithetic pairing. */
  num_paths: number;
  /** Total number of simulated sample paths; `2 * num_paths` with antithetic variates, otherwise equals `num_paths`. */
  num_simulated_paths: number;
  /** Legacy skipped-path count; current engines reject non-finite payoffs. */
  num_skipped: number;
  /** Median of captured discounted path values (absent when paths are not captured). */
  median?: number;
  /** 25th percentile of captured discounted path values (absent when paths are not captured). */
  percentile_25?: number;
  /** 75th percentile of captured discounted path values (absent when paths are not captured). */
  percentile_75?: number;
  /** Minimum of captured discounted path values (absent when paths are not captured). */
  min?: number;
  /** Maximum of captured discounted path values (absent when paths are not captured). */
  max?: number;
  /** Relative standard error (`stderr / |mean|`); `Infinity` near zero mean. */
  relative_stderr: number;
}

/** Variation margin calculator result (JSON object from Rust). */
export interface VariationMarginJson {
  gross_exposure: number;
  net_exposure: number;
  delivery_amount: number;
  return_amount: number;
  net_margin: number;
  requires_call: boolean;
}

/** Forecast backtest metrics (JSON object from Rust). */
export interface BacktestForecastMetricsJson {
  mae: number;
  mape: number;
  rmse: number;
  n: number;
}

export interface CoreNamespace {
  Currency: CurrencyConstructor;
  Money: MoneyConstructor;
  Rate: RateConstructor;
  Bps: BpsConstructor;
  Percentage: PercentageConstructor;
  DayCount: DayCountConstructor;
  DayCountContext: DayCountContextConstructor;
  Tenor: TenorConstructor;
  createDate(year: number, month: number, day: number): number;
  dateFromEpochDays(days: number): number[];
  adjust(epochDays: number, convention: string, calendarCode: string): number;
  availableCalendars(): string[];
  DiscountCurve: DiscountCurveConstructor;
  ForwardCurve: ForwardCurveConstructor;
  VolCube: VolCubeConstructor;
  FxDeltaVolSurface: FxDeltaVolSurfaceConstructor;
  FxConversionPolicy: FxConversionPolicyConstructor;
  FxRateResult: FxRateResultConstructor;
  FxMatrix: FxMatrixConstructor;
  choleskyDecomposition(matrix: number[][]): number[][];
  choleskySolve(chol: number[][], b: number[]): number[];
  /** Fast flat row-major Cholesky decomposition for typed numeric arrays. */
  choleskyDecompositionFlat(matrix: NumericArray, n: number): Float64Array;
  /** Fast flat row-major Cholesky solve for typed numeric arrays. */
  choleskySolveFlat(chol: NumericArray, b: NumericArray, n: number): Float64Array;
  /** Validates a square correlation matrix passed as flat row-major values. */
  validateCorrelationMatrixFlat(matrix: NumericArray, n: number): void;
  mean(data: number[]): number;
  meanArray(data: NumericArray): number;
  variance(data: number[]): number;
  varianceArray(data: NumericArray): number;
  populationVariance(data: number[]): number;
  populationVarianceArray(data: NumericArray): number;
  correlation(x: number[], y: number[]): number;
  correlationArray(x: NumericArray, y: NumericArray): number;
  covariance(x: number[], y: number[]): number;
  covarianceArray(x: NumericArray, y: NumericArray): number;
  quantile(data: number[], q: number): number;
  quantileArray(data: NumericArray, q: number): number;
  normCdf(x: number): number;
  normPdf(x: number): number;
  standardNormalInvCdf(p: number): number;
  erf(x: number): number;
  lnGamma(x: number): number;
  kahanSum(values: number[]): number;
  kahanSumArray(values: NumericArray): number;
  neumaierSum(values: number[]): number;
  neumaierSumArray(values: NumericArray): number;
  countConsecutive(values: number[]): number;
  countConsecutiveArray(values: NumericArray): number;
}

export declare const core: CoreNamespace;

// --- analytics ------------------------------------------------------------

export type NumericArray = number[] | Float64Array;
export type NumericMatrix = NumericArray[];

/** Descriptive statistics returned by `peerStats`. */
export interface PeerStatsJson {
  count: number;
  mean: number;
  median: number;
  std_dev: number;
  min: number;
  max: number;
  q1: number;
  q3: number;
  /** Interquartile range (`q3 - q1`). */
  iqr: number;
}

/** Single-factor OLS regression result returned by `regressionFairValue`. */
export interface RegressionResultJson {
  intercept: number;
  slope: number;
  r_squared: number;
  fitted_value: number;
  residual: number;
  n: number;
}

/** Per-dimension decomposition in a relative value score. */
export interface DimensionScoreJson {
  label: string;
  percentile: number;
  z_score: number;
  regression_residual: number | null;
  r_squared: number | null;
  weight: number;
}

/** Composite relative value result returned by `scoreRelativeValue`. */
export interface RelativeValueResultJson {
  company_id: string;
  composite_score: number;
  dimensions: DimensionScoreJson[];
  confidence: number;
  peer_count: number;
}

/** Structured formula explanation returned by `explainFormula`. */
export interface FormulaExplanationJson {
  node_id: string;
  period_id: string;
  final_value: number;
  node_type: string;
  formula_text?: string | null;
  breakdown: FormulaExplanationStepJson[];
}

/** One component in a structured formula explanation. */
export interface FormulaExplanationStepJson {
  component: string;
  value: number;
  operation?: string | null;
}

/** A single drawdown episode returned by `drawdownDetails`. */
export interface DrawdownEpisode {
  start: string;
  valley: string;
  end: string | null;
  duration_days: number;
  max_drawdown: number;
  near_recovery_threshold: number;
  truncated_at_start: boolean;
}

/** Aggregate statistics for grouped periodic returns. */
export interface PeriodStats {
  best: number;
  worst: number;
  consecutive_wins: number;
  consecutive_losses: number;
  win_rate: number;
  avg_return: number;
  avg_win: number;
  avg_loss: number;
  payoff_ratio: number;
  profit_factor: number;
  cpc_ratio: number;
  kelly_criterion: number;
}

/**
 * Dated rolling result returned by per-ticker rolling analytics.
 *
 * Exactly one metric-named key (`sharpe`, `sortino`, `volatility`, or
 * `return`) is present, matching the method that produced the series.
 */
export interface DatedSeries {
  dates: string[];
  sharpe?: Float64Array;
  sortino?: Float64Array;
  volatility?: Float64Array;
  return?: Float64Array;
}

/** Per-asset skewness/kurtosis pair returned by `skewKurt`. */
export interface SkewKurtResult {
  skewness: Float64Array;
  kurtosis: Float64Array;
}

/** Per-asset VaR/ES pair returned by `valueAtRiskAndEs`. */
export interface VarEsResult {
  value_at_risk: Float64Array;
  expected_shortfall: Float64Array;
}

/** OLS beta result with standard error and 95% confidence interval.
 *
 * The interval uses Student-t critical values for finite samples and an
 * asymptotic normal approximation once n - 2 >= 240.
 */
export interface BetaResult {
  beta: number;
  std_err: number;
  ci_lower: number;
  ci_upper: number;
}

/** Single-factor greeks (annualized Jensen alpha, beta, R², adjusted R²). */
export interface GreeksResult {
  alpha: number;
  beta: number;
  r_squared: number;
  adjusted_r_squared: number;
}

/** Rolling greeks output aligned with rolling-window end dates. */
export interface RollingGreeksResult {
  dates: string[];
  alphas: Float64Array;
  betas: Float64Array;
}

/** Multi-factor regression result. Alpha is the raw regression intercept, annualized. */
export interface MultiFactorResult {
  alpha: number;
  betas: number[];
  r_squared: number;
  adjusted_r_squared: number;
  residual_vol: number;
}

/** Period-to-date lookback returns (per ticker) returned by `lookbackReturns`. */
export interface LookbackReturns {
  mtd: number[];
  qtd: number[];
  ytd: number[];
  fytd: number[] | null;
}

/**
 * Stateful performance analytics engine over a panel of ticker series.
 *
 * `Performance` is the single entry point exposed to JS. Construct from
 * a price matrix (`new Performance(...)`) or a return matrix
 * (`Performance.fromReturns(...)`); every metric is then reachable as
 * an instance method.
 *
 * All multi-ticker scalar outputs come back as `number[]` indexed by the
 * panel's ticker order; vector / per-ticker / structured outputs are
 * serialized to plain JS objects (e.g. `DatedSeries`, `BetaResult[]`).
 */
export declare class Performance {
  constructor(
    dates: string[],
    prices: NumericMatrix,
    tickerNames: string[],
    benchmarkTicker?: string | null,
    freq?: string
  );
  /** Construct from a return matrix (one row per `dates` entry per ticker). */
  static fromReturns(
    dates: string[],
    returns: NumericMatrix,
    tickerNames: string[],
    benchmarkTicker?: string | null,
    freq?: string
  ): Performance;
  resetDateRange(start: string, end: string): void;
  resetBenchTicker(ticker: string): void;
  tickerNames(): string[];
  benchmarkIdx(): number;
  freq(): string;
  /** Full return-aligned date grid as ISO date strings (independent of any active window). */
  dates(): string[];
  /** Dates of the currently active analysis window as ISO date strings. */
  activeDates(): string[];
  /** Dates for one ticker's active return series as ISO date strings. */
  activeDatesForTicker(tickerIdx: number): string[];
  cagr(): Float64Array;
  meanReturn(annualize?: boolean): Float64Array;
  volatility(annualize?: boolean): Float64Array;
  sharpe(riskFreeRate?: number): Float64Array;
  /** Sortino ratio; mar is a per-period threshold. */
  sortino(mar?: number): Float64Array;
  calmar(): Float64Array;
  maxDrawdown(): Float64Array;
  meanDrawdown(): Float64Array;
  valueAtRisk(confidence?: number): Float64Array;
  expectedShortfall(confidence?: number): Float64Array;
  trackingError(): Float64Array;
  informationRatio(): Float64Array;
  skewness(): Float64Array;
  kurtosis(): Float64Array;
  geometricMean(): Float64Array;
  /** Skewness and kurtosis from one moments pass per asset. */
  skewKurt(): SkewKurtResult;
  /** Historical VaR and expected shortfall from one tail pass per asset. */
  valueAtRiskAndEs(confidence?: number): VarEsResult;
  /** Downside deviation; mar is a per-period threshold. */
  downsideDeviation(mar?: number): Float64Array;
  maxDrawdownDuration(): number[];
  /** Empyrical-style annualized geometric up-capture. */
  upCapture(): Float64Array;
  /** Empyrical-style annualized geometric down-capture. */
  downCapture(): Float64Array;
  /** Empyrical-style annualized geometric up/down capture ratio. */
  captureRatio(): Float64Array;
  omegaRatio(threshold?: number): Float64Array;
  treynor(riskFreeRate?: number): Float64Array;
  gainToPain(): Float64Array;
  ulcerIndex(): Float64Array;
  martinRatio(): Float64Array;
  recoveryFactor(): Float64Array;
  painIndex(): Float64Array;
  painRatio(riskFreeRate?: number): Float64Array;
  tailRatio(confidence?: number): Float64Array;
  rSquared(): Float64Array;
  battingAverage(): Float64Array;
  parametricVar(confidence?: number): Float64Array;
  cornishFisherVar(confidence?: number): Float64Array;
  cdar(confidence?: number): Float64Array;
  mSquared(riskFreeRate?: number): Float64Array;
  modifiedSharpe(riskFreeRate?: number, confidence?: number): Float64Array;
  sterlingRatio(riskFreeRate?: number, n?: number): Float64Array;
  burkeRatio(riskFreeRate?: number, n?: number): Float64Array;
  cumulativeReturns(): Float64Array[];
  drawdownSeries(): Float64Array[];
  correlationMatrix(): Float64Array[];
  cumulativeReturnsOutperformance(): Float64Array[];
  drawdownDifference(): Float64Array[];
  excessReturns(rf: NumericArray, nperiods?: number): Float64Array[];
  beta(): BetaResult[];
  greeks(riskFreeRate?: number): GreeksResult[];
  rollingGreeks(tickerIdx: number, window?: number, riskFreeRate?: number): RollingGreeksResult;
  rollingVolatility(tickerIdx: number, window?: number): DatedSeries;
  rollingSortino(tickerIdx: number, window?: number, mar?: number): DatedSeries;
  rollingSharpe(tickerIdx: number, window?: number, riskFreeRate?: number): DatedSeries;
  rollingReturns(tickerIdx: number, window: number): DatedSeries;
  drawdownDetails(tickerIdx: number, n?: number): DrawdownEpisode[];
  multiFactorGreeks(tickerIdx: number, factorReturns: NumericMatrix): MultiFactorResult;
  /**
   * Period-to-date lookback returns. The FYTD window starts at the fiscal-year
   * start adjusted to the next business day on `calendar` (default `"nyse"`);
   * pass the calendar id matching your market for non-US panels.
   */
  lookbackReturns(
    refDate: string,
    fiscalYearStartMonth?: number,
    fiscalYearStartDay?: number,
    calendar?: string
  ): LookbackReturns;
  periodStats(
    tickerIdx: number,
    aggFreq?: string,
    fiscalYearStartMonth?: number,
    fiscalYearStartDay?: number
  ): PeriodStats;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

export interface AnalyticsNamespace {
  /**
   * `Performance` is the single entry point for analytics on a panel of
   * ticker series. Construct from prices (`new Performance(...)`) or from
   * returns (`Performance.fromReturns(...)`); every metric — return/risk
   * scalars, drawdown statistics, rolling windows, periodic returns
   * (MTD/QTD/YTD/FYTD), benchmark alpha/beta, basic factor models — is a
   * method on the resulting instance.
   */
  Performance: typeof Performance;
}

export declare const analytics: AnalyticsNamespace;

// --- factor_model.credit ------------------------------------------------------

/**
 * Calibrated credit factor hierarchy artifact.
 *
 * Produced by `CreditCalibrator` or deserialized from JSON via `fromJson`.
 * Immutable once constructed.
 */
export declare class CreditFactorModel {
  private constructor();
  /** Deserialize and validate a `CreditFactorModel` from JSON. */
  static fromJson(s: string): CreditFactorModel;
  /** Serialize to pretty-printed JSON. */
  toJson(): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Deterministic calibrator that produces a `CreditFactorModel`.
 *
 * Configuration and inputs are passed as JSON strings.
 */
export declare class CreditCalibrator {
  /** Construct a calibrator from a JSON-serialized `CreditCalibrationConfig`. */
  constructor(configJson: string);
  /** Run the calibration pipeline and return a `CreditFactorModel`. */
  calibrate(inputsJson: string): CreditFactorModel;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Snapshot of all hierarchy-level factor values at a single date.
 *
 * Produced by `decomposeLevels`. Pass to `decomposePeriod` to compute
 * period-over-period changes.
 */
export declare class LevelsAtDate {
  private constructor();
  /** Serialize the snapshot to pretty-printed JSON. */
  toJson(): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Component-wise difference between two `LevelsAtDate` snapshots.
 *
 * Produced by `decomposePeriod`.
 */
export declare class PeriodDecomposition {
  private constructor();
  /** Serialize the decomposition to pretty-printed JSON. */
  toJson(): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Vol-forecast view over a calibrated `CreditFactorModel`.
 *
 * `VolHorizon::Custom` is intentionally **not** exposed.
 *
 * Horizon strings accepted by `covarianceAt`, `idiosyncraticVol`, and
 * `factorModelAt`:
 * - `"one_step"` — calibrated annualized variance unchanged.
 * - `"unconditional"` — long-run.
 * - `'{"n_steps": N}'` — variance scaled by `N`.
 */
export declare class FactorCovarianceForecast {
  constructor(model: CreditFactorModel);
  /**
   * Build the factor covariance matrix at the requested horizon.
   * Returns pretty-printed JSON of a `FactorCovarianceMatrix`.
   */
  covarianceAt(horizonJson: string): string;
  /** Idiosyncratic vol (std dev) for a specific issuer at the requested horizon. */
  idiosyncraticVol(issuerId: string, horizonJson: string): number;
  /**
   * Build a portfolio-level `FactorModelConfig` JSON at the given horizon and
   * risk measure.
   */
  factorModelAt(horizonJson: string, riskMeasureJson: string): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Decompose observed issuer spreads at a point in time into per-level factor
 * values and per-issuer residual adders.
 *
 * @param model                Calibrated `CreditFactorModel`.
 * @param observedSpreadsJson  JSON `{issuer_id: spread}` map.
 * @param observedGeneric      Generic (PC) factor value at `asOf`.
 * @param asOf                 ISO 8601 date string.
 * @param runtimeTagsJson      Optional JSON `{issuer_id: {dim_key: tag}}` for
 *                             issuers not present in the model artifact.
 */
export declare function decomposeLevels(
  model: CreditFactorModel,
  observedSpreadsJson: string,
  observedGeneric: number,
  asOf: string,
  runtimeTagsJson?: string
): LevelsAtDate;

/**
 * Difference two `LevelsAtDate` snapshots component-wise.
 *
 * Output is restricted to buckets and issuers present in **both** snapshots.
 */
export declare function decomposePeriod(
  fromLevels: LevelsAtDate,
  toLevels: LevelsAtDate
): PeriodDecomposition;

export interface FactorModelCreditNamespace {
  CreditFactorModel: typeof CreditFactorModel;
  CreditCalibrator: typeof CreditCalibrator;
  LevelsAtDate: typeof LevelsAtDate;
  PeriodDecomposition: typeof PeriodDecomposition;
  FactorCovarianceForecast: typeof FactorCovarianceForecast;
  decomposeLevels(
    model: CreditFactorModel,
    observedSpreadsJson: string,
    observedGeneric: number,
    asOf: string,
    runtimeTagsJson?: string
  ): LevelsAtDate;
  decomposePeriod(fromLevels: LevelsAtDate, toLevels: LevelsAtDate): PeriodDecomposition;
}

export interface FactorModelNamespace {
  /** Credit factor hierarchy artifacts, calibration, and decomposition. */
  credit: FactorModelCreditNamespace;
}

export declare const factor_model: FactorModelNamespace;

// --- features ---------------------------------------------------------------

export type FeatureValue = number | null;
export type FeatureParams = Record<string, unknown>;

/**
 * Vectorized panel feature transforms.
 *
 * `values` accepts finite numbers or `null`; non-finite values are treated as
 * missing by the Rust crate. Time-series transforms are grouped by `entity` and
 * sorted by `order`; cross-sectional transforms partition by `timeKey`.
 */
export interface FeaturesNamespace {
  /** Transform a time-series panel column per entity. */
  transformTimeseries(
    values: FeatureValue[],
    entity: string[],
    order: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /** Transform a cross-section per timestamp. */
  transformCrossSectional(
    values: FeatureValue[],
    timeKey: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /** Apply a JSON panel transform pipeline and return JSON result columns. */
  transformPanel(specJson: string): string;
}

export declare const features: FeaturesNamespace;

// --- valuations.correlation -------------------------------------------------

export interface Copula {
  readonly numFactors: number;
  readonly modelName: string;
  conditionalDefaultProb(
    defaultThreshold: number,
    factorRealization: number[],
    correlation: number
  ): number;
  /**
   * Strict lower-tail dependence coefficient `λ_L` at the given correlation.
   *
   * Returns `NaN` when the model has no closed-form `λ_L` (Random Factor
   * Loading); check `Number.isNaN()` before using the result. For the RFL
   * heuristic stress gauge use `stressCorrelationProxy` instead.
   */
  tailDependence(correlation: number): number;
  /**
   * Heuristic stress-correlation proxy for the Random Factor Loading copula.
   *
   * NOT the strict `λ_L` (which has no closed form for RFL — `tailDependence`
   * returns `NaN`). Gauges the extra correlation mass in the high-loading
   * tail; vanishes in the Gaussian (`loadingVol = 0`) limit. Throws for
   * non-RFL copulas.
   */
  stressCorrelationProxy(correlation: number): number;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

export interface CopulaSpec {
  readonly isGaussian: boolean;
  readonly isStudentT: boolean;
  readonly isRfl: boolean;
  readonly isMultiFactor: boolean;
  build(): Copula;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

export interface CopulaSpecConstructor {
  gaussian(): CopulaSpec;
  studentT(df: number): CopulaSpec;
  randomFactorLoading(loadingVol: number): CopulaSpec;
  multiFactor(numFactors: number): CopulaSpec;
}

export interface RecoveryModel {
  /** Expected (unconditional, Jensen-corrected) recovery rate. */
  readonly expectedRecovery: number;
  readonly lgd: number;
  /** Recovery-rate volatility scale (0 for constant models). */
  readonly recoveryVolatility: number;
  readonly isStochastic: boolean;
  readonly modelName: string;
  conditionalRecovery(marketFactor: number): number;
  /** Conditional LGD (1 − conditional recovery) given the market factor. */
  conditionalLgd(marketFactor: number): number;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

export interface RecoverySpec {
  /**
   * Location-parameter recovery rate of this spec: the constant rate for a
   * constant spec, or the `mean` input (target recovery at factor `Z = 0`)
   * for a market-correlated spec. The latter differs from the
   * Jensen-corrected unconditional mean — use `build().expectedRecovery`
   * for the true expected recovery.
   */
  readonly expectedRecovery: number;
  build(): RecoveryModel;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

export interface RecoverySpecConstructor {
  constant(rate: number): RecoverySpec;
  marketCorrelated(mean: number, vol: number, correlation: number): RecoverySpec;
  /**
   * Market-standard stochastic recovery (40% mean, 25% vol, +40% corr —
   * recovery falls in stress under the canonical low-factor-stress
   * convention).
   */
  marketStandardStochastic(): RecoverySpec;
}

/** Exported class; construct instances via `CopulaSpec.build()` (no public `new`). */
export interface CopulaClass {
  readonly prototype: Copula;
}

/** Exported class; construct instances via `RecoverySpec.build()` (no public `new`). */
export interface RecoveryModelClass {
  readonly prototype: RecoveryModel;
}

export interface CorrelationNamespace {
  CopulaSpec: CopulaSpecConstructor;
  Copula: CopulaClass;
  RecoverySpec: RecoverySpecConstructor;
  RecoveryModel: RecoveryModelClass;
  /** Fréchet-Hoeffding bounds `[rhoMin, rhoMax]` as a `Float64Array` (wasm-bindgen `Vec<f64>` return). */
  correlationBounds(p1: number, p2: number): Float64Array;
  /** Joint probabilities `[p11, p10, p01, p00]` as a `Float64Array` (wasm-bindgen `Vec<f64>` return). */
  jointProbabilities(p1: number, p2: number, correlation: number): Float64Array;
  /**
   * Validate a flat row-major correlation matrix with explicit dimension `n`
   * (finstack_quant_valuations::correlation). Checks unit diagonal, off-diagonal in
   * [-1, 1], symmetry, and positive semi-definiteness; throws a descriptive
   * error otherwise.
   */
  validateCorrelationMatrix(matrix: NumericArray, n: number): void;
  /**
   * Nearest correlation matrix (Higham 2002) for a near-PSD input.
   *
   * Projects a symmetric, near-unit-diagonal, near-PSD matrix onto the set of
   * valid correlation matrices in Frobenius norm. Gross input violations
   * (asymmetry > 1e-6 or diagonal far from 1) throw rather than being silently
   * reshaped. Returns the flat row-major result as a `Float64Array`.
   */
  /**
   * `maxIter` / `tol` default to the Rust `NearestCorrelationOpts::default()`
   * values (currently 200 and 1e-10).
   */
  nearestCorrelation(matrix: NumericArray, n: number, maxIter?: number, tol?: number): Float64Array;
}

// --- monte_carlo ----------------------------------------------------------
// Convenience subset of finstack-quant-monte-carlo. Advanced Rust process,
// discretization, RNG, payoff, and Greeks types are not standalone WASM types.

export interface MonteCarloNamespace {
  priceEuropeanCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  priceEuropeanPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  priceHestonCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    kappa: number,
    theta: number,
    volOfVol: number,
    rho: number,
    v0: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  priceHestonPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    kappa: number,
    theta: number,
    volOfVol: number,
    rho: number,
    v0: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  /**
   * Price an arithmetic Asian call using post-initial fixings at steps
   * `1..=numSteps`; the initial spot at step 0 is excluded.
   */
  priceAsianCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  /**
   * Price an arithmetic Asian put using post-initial fixings at steps
   * `1..=numSteps`; the initial spot at step 0 is excluded.
   */
  priceAsianPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  priceAmericanPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string,
    useParallel?: boolean,
    basis?: string,
    basisDegree?: number
  ): MonteCarloEstimateJson;
  priceAmericanCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string,
    useParallel?: boolean,
    basis?: string,
    basisDegree?: number
  ): MonteCarloEstimateJson;
  priceAmericanPutUnbiased(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    pricingSeed: bigint,
    numSteps?: number,
    currency?: string,
    useParallel?: boolean,
    basis?: string,
    basisDegree?: number
  ): MonteCarloEstimateJson;
  priceAmericanCallUnbiased(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    pricingSeed: bigint,
    numSteps?: number,
    currency?: string,
    useParallel?: boolean,
    basis?: string,
    basisDegree?: number
  ): MonteCarloEstimateJson;
  blackScholesCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number
  ): number;
  blackScholesPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number
  ): number;
}

export declare const monte_carlo: MonteCarloNamespace;

// --- margin ----------------------------------------------------------------

export interface MarginNamespace {
  csaUsdRegulatory(): string;
  csaEurRegulatory(): string;
  validateCsaJson(json: string): string;
  calculateVm(
    csaJson: string,
    exposure: number,
    postedCollateral: number,
    currency: string,
    year: number,
    month: number,
    day: number
  ): VariationMarginJson;
}

export declare const margin: MarginNamespace;

// --- cashflows -------------------------------------------------------------

/**
 * JSON bridge to the Rust `finstack-quant-cashflows` crate.
 *
 * All methods accept and return JSON strings that mirror the canonical Rust
 * serde model. Refer to `src/api/cashflows/mod.rs` for parameter and return-shape
 * details; the docstrings there are kept in sync with the underlying Rust
 * implementation.
 */
export interface CashflowsNamespace {
  /**
   * Build a cashflow schedule from a `CashflowScheduleBuildSpec` JSON string.
   *
   * @param specJson    JSON-encoded `CashflowScheduleBuildSpec`.
   * @param marketJson  Optional JSON-encoded market context for floating-rate lookups.
   * @returns           JSON-encoded `CashFlowSchedule`.
   * @throws            If the spec or market JSON is malformed, or schedule construction fails.
   */
  buildCashflowScheduleJson(specJson: string, marketJson?: string | null): string;

  /**
   * Build a stamped cashflow schedule envelope from a `CashflowScheduleBuildSpec` JSON string.
   *
   * @param specJson    JSON-encoded `CashflowScheduleBuildSpec`.
   * @param marketJson  Optional JSON-encoded market context for floating-rate lookups.
   * @returns           JSON-encoded `CashflowScheduleEnvelope`.
   * @throws            If the spec or market JSON is malformed, or schedule construction fails.
   */
  buildCashflowScheduleEnvelopeJson(specJson: string, marketJson?: string | null): string;

  /**
   * Validate a cashflow schedule JSON string and return it canonicalized.
   *
   * @param scheduleJson JSON-encoded `CashFlowSchedule`.
   * @returns            Canonicalized JSON-encoded `CashFlowSchedule`.
   * @throws             If the schedule JSON is malformed or fails validation.
   */
  validateCashflowScheduleJson(scheduleJson: string): string;

  /**
   * Validate a cashflow schedule envelope JSON string and return it canonicalized.
   *
   * @param envelopeJson JSON-encoded `CashflowScheduleEnvelope`.
   * @returns            Canonicalized JSON-encoded `CashflowScheduleEnvelope`.
   * @throws             If the envelope JSON is malformed or fails validation.
   */
  validateCashflowScheduleEnvelopeJson(envelopeJson: string): string;

  /**
   * Extract dated flows from a cashflow schedule.
   *
   * @param scheduleJson JSON-encoded `CashFlowSchedule`.
   * @returns            JSON array of `{date, amount}` entries, where `amount`
   *                     is itself `{amount, currency}`. `CFKind` and accrual
   *                     metadata are intentionally omitted.
   * @throws             If the schedule JSON is malformed.
   */
  datedFlowsJson(scheduleJson: string): string;

  /**
   * Compute accrued interest for a schedule as of a given date.
   *
   * @param scheduleJson JSON-encoded `CashFlowSchedule`.
   * @param asOf         ISO-8601 date (YYYY-MM-DD) for the accrual snapshot.
   * @param configJson   Optional JSON-encoded `AccrualConfig` overriding defaults.
   * @returns            Accrued interest in the schedule's settlement currency
   *                     as a JS number. For large notionals, compare with an
   *                     absolute tolerance scaled to notional rather than
   *                     expecting decimal-string equality.
   * @throws             If any JSON input is malformed or the accrual computation fails.
   */
  accruedInterestJson(scheduleJson: string, asOf: string, configJson?: string | null): number;

  /**
   * Construct a tagged Bond instrument JSON from a cashflow schedule.
   *
   * Convenience wrapper that crosses crates: it materializes a
   * `finstack_quant_valuations::instruments::fixed_income::bond::Bond` from the
   * supplied schedule and wraps it in the tagged `InstrumentJson` envelope.
   *
   * @param instrumentId    Identifier for the Bond instrument.
   * @param scheduleJson    JSON-encoded `CashFlowSchedule`.
   * @param discountCurveId Identifier of the discount curve used for pricing.
   * @param quotedClean     Optional clean quoted price used to calibrate yield on construction.
   * @returns               JSON-encoded tagged `InstrumentJson::Bond`.
   * @throws                If the schedule JSON is malformed or bond construction fails.
   */
  bondFromCashflowsJson(
    instrumentId: string,
    scheduleJson: string,
    discountCurveId: string,
    quotedClean?: number | null
  ): string;
}

export declare const cashflows: CashflowsNamespace;

// --- covenants -------------------------------------------------------------

/**
 * JSON bridge to the Rust `finstack-quant-covenants` crate.
 */
export interface CovenantsNamespace {
  validateCovenantSpec(specJson: string): string;
  validateCovenantReport(reportJson: string): string;
  validateCovenantEngine(engineJson: string): string;
  evaluateEngine(engineJson: string, metricsJson: string, asOf: string): string;
  lboStandard(
    initialLeverage: number,
    interestCoverage: number,
    fixedChargeCoverage: number,
    maxCapex: number
  ): string;
  covLite(maxLeverage: number, maxSeniorLeverage: number): string;
  realEstate(minDscr: number, minDebtYield: number, maxLtv: number): string;
  projectFinance(
    minDscr: number,
    distributionLockupDscr: number,
    minLiquidity: number,
    maxNetLeverage: number
  ): string;
}

export declare const covenants: CovenantsNamespace;

// --- valuations ------------------------------------------------------------

export declare class Market {
  constructor(json: string);
  toJson(): string;
}

export interface ValuationInstrumentsNamespace {
  validateInstrumentJson(json: string): string;
  priceInstrument(instrumentJson: string, marketJson: string, asOf: string, model: string): string;
  priceInstrumentWithMetrics(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    model: string,
    metrics: string[],
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): string;
  priceInstrumentWithMarket(
    instrumentJson: string,
    market: Market,
    asOf: string,
    model: string
  ): string;
  priceInstrumentWithMetricsAndMarket(
    instrumentJson: string,
    market: Market,
    asOf: string,
    model: string,
    metrics: string[],
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): string;
  instrumentCashflowsWithMarket(
    instrumentJson: string,
    market: Market,
    asOf: string,
    model: string
  ): string;
  listStandardMetrics(): string[];
  listStandardMetricsGrouped(): Record<string, string[]>;
}

export type FxInstrumentSpec = Record<string, unknown> | string;

export interface FxInstrument {
  toJson(): string;
  price(marketJson: string, asOf: string, model?: string | null): string;
  /** WASM order keeps optional arguments trailing: metrics precedes model. */
  priceWithMetrics(
    marketJson: string,
    asOf: string,
    metrics: string[],
    model?: string | null,
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): string;
}

export interface FxOptionInstrument extends FxInstrument {
  delta(marketJson: string, asOf: string, model?: string | null): number;
  gamma(marketJson: string, asOf: string, model?: string | null): number;
  vega(marketJson: string, asOf: string, model?: string | null): number;
  theta(marketJson: string, asOf: string, model?: string | null): number;
  rho(marketJson: string, asOf: string, model?: string | null): number;
  foreignRho(marketJson: string, asOf: string, model?: string | null): number;
  vanna(marketJson: string, asOf: string, model?: string | null): number;
  volga(marketJson: string, asOf: string, model?: string | null): number;
  greeks(marketJson: string, asOf: string, model?: string | null): Record<string, number>;
}

export interface FxInstrumentConstructor<T extends FxInstrument> {
  new (spec: FxInstrumentSpec): T;
  fromJson(json: string): T;
}

export interface FxNamespace {
  FxSpot: FxInstrumentConstructor<FxInstrument>;
  FxForward: FxInstrumentConstructor<FxInstrument>;
  FxSwap: FxInstrumentConstructor<FxInstrument>;
  Ndf: FxInstrumentConstructor<FxInstrument>;
  FxOption: FxInstrumentConstructor<FxOptionInstrument>;
  FxDigitalOption: FxInstrumentConstructor<FxOptionInstrument>;
  FxTouchOption: FxInstrumentConstructor<FxOptionInstrument>;
  FxBarrierOption: FxInstrumentConstructor<FxOptionInstrument>;
  FxVarianceSwap: FxInstrumentConstructor<FxInstrument>;
  QuantoOption: FxInstrumentConstructor<FxOptionInstrument>;
}

// --- SABR (Stochastic Alpha Beta Rho) volatility -------------------------

export interface SabrParameters {
  readonly alpha: number;
  readonly beta: number;
  readonly nu: number;
  readonly rho: number;
  readonly shift: number | undefined;
  isShifted(): boolean;
}

export interface SabrParametersConstructor {
  new (alpha: number, beta: number, nu: number, rho: number, shift?: number): SabrParameters;
  /** Equity-standard defaults `(alpha=0.20, beta=1.0, nu=0.30, rho=-0.20)`. */
  equityDefault(): SabrParameters;
  /** Rates-standard defaults `(alpha=0.02, beta=0.5, nu=0.30, rho=0.0)`. */
  ratesDefault(): SabrParameters;
}

export interface SabrModel {
  impliedVol(forward: number, strike: number, t: number): number;
  /** Parameters used by this model. */
  readonly params: SabrParameters;
  supportsNegativeRates(): boolean;
}

export interface SabrModelConstructor {
  new (params: SabrParameters): SabrModel;
}

export interface SabrSmileArbitrageResult {
  arbitrageFree: boolean;
  butterflyViolations: Array<{
    strike: number;
    butterflyValue: number;
    severityPct: number;
  }>;
  monotonicityViolations: Array<{
    strikeLow: number;
    strikeHigh: number;
    priceLow: number;
    priceHigh: number;
  }>;
}

export interface SabrSmile {
  atmVol(): number;
  impliedVol(strike: number): number;
  generateSmile(strikes: number[]): number[];
  arbitrageDiagnostics(strikes: number[], r?: number, q?: number): SabrSmileArbitrageResult;
}

export interface SabrSmileConstructor {
  new (params: SabrParameters, forward: number, t: number): SabrSmile;
}

export interface SabrCalibrator {
  /** Copy of this calibrator with an overridden convergence tolerance. */
  withTolerance(tolerance: number): SabrCalibrator;
  calibrate(
    forward: number,
    strikes: number[],
    marketVols: number[],
    t: number,
    beta: number
  ): SabrParameters;
  /** Calibrate with automatic shift selection for negative-rate smiles. */
  calibrateAutoShift(
    forward: number,
    strikes: number[],
    marketVols: number[],
    t: number,
    beta: number
  ): SabrParameters;
}

export interface SabrCalibratorConstructor {
  new (): SabrCalibrator;
  /** Tighter tolerance for production fits. */
  highPrecision(): SabrCalibrator;
}

export interface ValuationCreditNamespace {
  mertonModelJson(
    assetValue: number,
    assetVol: number,
    debtBarrier: number,
    riskFreeRate: number
  ): string;
  creditGradesModelJson(
    equityValue: number,
    equityVol: number,
    totalDebt: number,
    riskFreeRate: number,
    barrierUncertainty: number,
    meanRecovery: number
  ): string;
  mertonDefaultProbability(modelJson: string, horizon: number): number;
  mertonDistanceToDefault(modelJson: string, horizon: number): number;
  mertonImpliedSpread(modelJson: string, horizon: number, recovery: number): number;
  dynamicRecoveryAtNotional(specJson: string, notional: number): number;
  endogenousHazardAtLeverage(specJson: string, leverage: number): number;
  endogenousHazardAfterPikAccrual(
    specJson: string,
    accretedNotional: number,
    assetValue: number
  ): number;
  dynamicRecoveryConstantJson(recovery: number): string;
  endogenousHazardPowerLawJson(baseHazard: number, baseLeverage: number, exponent: number): string;
  creditStateJson(
    hazardRate: number,
    distanceToDefault: number | null | undefined,
    leverage: number,
    accretedNotional: number,
    couponDue: number,
    assetValue?: number | null
  ): string;
  toggleExerciseThresholdJson(
    variable: 'hazard_rate' | 'distance_to_default' | 'leverage',
    threshold: number,
    direction: 'above' | 'below'
  ): string;
  toggleExerciseOptimalJson(
    nestedPaths: number,
    equityDiscountRate: number,
    assetVol: number,
    riskFreeRate: number,
    horizon: number
  ): string;
}

export interface CreditDerivativesNamespace {
  creditDefaultSwapExampleJson(): string;
  cdsIndexExampleJson(): string;
  cdsTrancheExampleJson(): string;
  cdsOptionExampleJson(): string;
}

export interface ValuationsNamespace {
  /**
 * Credit-correlation infrastructure (copulas, recovery models, and matrix
 * utilities). Latent factor models are Python-only and not on this facade.
 */
  correlation: CorrelationNamespace;
  /** Structural credit models and toggle-exercise helpers. */
  credit: ValuationCreditNamespace;
  /** CDS-family JSON wrappers and pricing helpers. */
  creditDerivatives: CreditDerivativesNamespace;
  /** Direct FX instrument wrappers. */
  fx: FxNamespace;
  /** Instrument JSON validation and pricing helpers. */
  instruments: ValuationInstrumentsNamespace;
  validateValuationResultJson(json: string): string;
  /**
   * Validate a `CalibrationEnvelope` and return the canonical pretty-printed JSON string.
   * Accepts either a typed object or a pre-serialized JSON string.
   * Use as a pre-flight check before passing an envelope to `calibrate`.
   */
  validateCalibrationJson(envelope: CalibrationEnvelope | string): string;
  /**
   * Execute a `CalibrationEnvelope` and return the full `CalibrationResultEnvelope`.
   * Accepts either a typed object or a pre-serialized JSON string.
   * The canonical path for building a `MarketContext` from quotes — the resulting
   * `result.final_market` is a materialized state ready for `MarketContext::try_from`
   * (Rust) or `result.market` (Python).
   *
   * @throws Error with `name = "CalibrationEnvelopeError"` and structured `cause`
   *   (e.g. `e.cause.kind === "solver_not_converged"`) on calibration failure.
   *
   * ⚠️ BLOCKING: calibration may be CPU-heavy. Wrap calls in an application
   * timeout until `timeout_ms` is carried by the calibration envelope schema.
   */
  calibrate(envelope: CalibrationEnvelope | string): CalibrationResultEnvelope;
  /**
   * Pre-flight envelope validation without invoking the solver.
   * Returns a JSON-serialized `ValidationReport` listing every error found
   * plus the dependency graph. Microseconds.
   *
   * @throws Error with `name = "CalibrationEnvelopeError"` if the envelope JSON is malformed.
   */
  dryRun(envelope: CalibrationEnvelope | string): string;
  /**
   * Returns the static dependency graph of a calibration plan as JSON.
   *
   * @throws Error with `name = "CalibrationEnvelopeError"` if the envelope JSON is malformed.
   */
  dependencyGraphJson(envelope: CalibrationEnvelope | string): string;
  Market: typeof Market;
  /**
   * Per-flow cashflow envelope (DF / survival / PV) for a discountable
   * instrument. `model` must be `"discounting"` or `"hazard_rate"`; the
   * envelope's `total_pv` reconciles with `base_value` for supported pairs.
   */
  instrumentCashflowsJson(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    model: string
  ): string;
  /** Per-unit Black-Scholes / Garman-Kohlhagen price of a European option.
   *  All rates are continuously compounded decimals; `sigma` is annualized vol;
   *  `t` is years to expiry. */
  bsPrice(
    spot: number,
    strike: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    isCall: boolean
  ): number;
  /** Black-Scholes / Garman-Kohlhagen Greeks as a dict
   *  `{delta, gamma, vega, theta, rho, rhoQ}`. `vega` and both rho values are
   *  per 1% move; `theta` is per-day under the `thetaDays` day-count. */
  bsGreeks(
    spot: number,
    strike: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    isCall: boolean,
    thetaDays?: number
  ): {
    delta: number;
    gamma: number;
    vega: number;
    theta: number;
    rho: number;
    rhoQ: number;
  };
  /** Solve for Black-Scholes implied volatility given a target price. */
  bsImpliedVol(
    spot: number,
    strike: number,
    r: number,
    q: number,
    t: number,
    price: number,
    isCall: boolean
  ): number;
  /** Solve for Black-76 (forward-based) implied volatility given a target price. */
  black76ImpliedVol(
    forward: number,
    strike: number,
    df: number,
    t: number,
    price: number,
    isCall: boolean
  ): number;
  /** Reiner-Rubinstein continuous-monitoring barrier call.
   *  `direction` is `"up"`/`"down"`, `knock` is `"in"`/`"out"`. */
  barrierCall(
    spot: number,
    strike: number,
    barrier: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    direction: 'up' | 'down',
    knock: 'in' | 'out'
  ): number;
  /** Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option. */
  asianOptionPrice(
    spot: number,
    strike: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    numFixings: number,
    averaging?: 'arithmetic' | 'geometric',
    isCall?: boolean
  ): number;
  /** Conze-Viswanathan lookback option. */
  lookbackOptionPrice(
    spot: number,
    strike: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    extremum: number,
    strikeType?: 'fixed' | 'floating',
    isCall?: boolean
  ): number;
  /** Quanto (FX-adjusted cross-currency) option price in domestic currency. */
  quantoOptionPrice(
    spot: number,
    strike: number,
    t: number,
    rateDomestic: number,
    rateForeign: number,
    divYield: number,
    volAsset: number,
    volFx: number,
    correlation: number,
    isCall?: boolean
  ): number;
  /** SABR parameters `(alpha, beta, nu, rho)` with optional `shift`. */
  SabrParameters: SabrParametersConstructor;
  /** Hagan-2002 SABR volatility model. */
  SabrModel: SabrModelConstructor;
  /** SABR smile generator for a fixed `(forward, t)` pair. */
  SabrSmile: SabrSmileConstructor;
  /** Levenberg-Marquardt SABR calibrator (beta fixed). */
  SabrCalibrator: SabrCalibratorConstructor;
  /** Black-Scholes European option price via the Fang-Oosterlee COS method. */
  bsCosPrice(
    spot: number,
    strike: number,
    rate: number,
    dividend: number,
    vol: number,
    maturity: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /** Variance Gamma European option price via the COS method. */
  vgCosPrice(
    spot: number,
    strike: number,
    rate: number,
    dividend: number,
    sigma: number,
    theta: number,
    nu: number,
    maturity: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /** Merton (1976) jump-diffusion European option price via the COS method. */
  mertonJumpCosPrice(
    spot: number,
    strike: number,
    rate: number,
    dividend: number,
    sigma: number,
    muJump: number,
    sigmaJump: number,
    lambda: number,
    maturity: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /** Simulated TARN coupon profile. Returns `{coupons_paid, cumulative, redemption_index, redeemed_early}`. */
  tarnCouponProfile(
    fixedRate: number,
    couponFloor: number,
    floatingFixings: number[],
    targetCoupon: number,
    dayCountFraction: number
  ): {
    coupons_paid: number[];
    cumulative: number[];
    redemption_index: number | null;
    redeemed_early: boolean;
  };
  /** Snowball / inverse-floater coupon schedule. */
  snowballCouponProfile(
    initialCoupon: number,
    fixedRate: number,
    floatingFixings: number[],
    floor: number,
    cap: number,
    isInverseFloater: boolean,
    leverage?: number
  ): number[];
  /** Intrinsic (undiscounted) payoff of a CMS spread option. */
  cmsSpreadOptionIntrinsic(
    longCms: number,
    shortCms: number,
    strike: number,
    isCall: boolean,
    notional: number
  ): number;
  /** Accrued coupon on a range-accrual leg given observed rates. */
  callableRangeAccrualAccrued(
    lower: number,
    upper: number,
    observations: number[],
    couponRate: number,
    dayCountFraction: number
  ): number;
}

export declare const valuations: ValuationsNamespace;

// --- attribution -----------------------------------------------------------

export interface AttributionNamespace {
  /** Parameters constructor emitted by wasm-bindgen for attribution calls.
   *
   * `configJson` may include `{ "execution_policy": "serial" }` when the host
   * already parallelizes attribution at the portfolio or batch level.
   */
  AttributionParams: new (
    instrumentJson: string,
    marketT0Json: string,
    marketT1Json: string,
    asOfT0: string,
    asOfT1: string,
    methodJson: string,
    configJson?: string,
    fullCrossAttribution?: boolean
  ) => unknown;
  /** Run P&L attribution for a single instrument. */
  attributePnl(params: unknown): string;
  /** Run attribution from a full JSON AttributionEnvelope. */
  attributePnlFromSpec(specJson: string): string;
  /** Validate an attribution specification JSON. */
  validateAttributionJson(json: string): string;
  /** Return the default waterfall factor ordering. */
  defaultWaterfallOrder(): string[];
  /** Return the default metric IDs used by metrics-based attribution. */
  defaultAttributionMetrics(): string[];
}

export declare const attribution: AttributionNamespace;

// --- statements ------------------------------------------------------------

export interface StatementsNamespace {
  validateFinancialModelJson(json: string): string;
  modelNodeIds(json: string): string[];
  validateCheckSuiteSpec(json: string): string;
  validateCapitalStructureSpec(json: string): string;
  validateWaterfallSpec(json: string): string;
  validateEcfSweepSpec(json: string): string;
  validatePikToggleSpec(json: string): string;
  evaluateModel(modelJson: string): string;
  evaluateModelWithMarket(modelJson: string, marketJson: string, asOf: string): string;
  parseFormula(formula: string): string;
  validateFormula(formula: string): boolean;
}

export declare const statements: StatementsNamespace;

// --- statements_analytics -------------------------------------------------

export interface GoalSeekResult {
  solved_value: number;
  updated_model_json?: string;
}

export interface StatementsAnalyticsNamespace {
  runSensitivity(modelJson: string, configJson: string): string;
  runVariance(baseJson: string, comparisonJson: string, configJson: string): string;
  evaluateScenarioSet(modelJson: string, scenarioSetJson: string): string;
  backtestForecast(actual: number[], forecast: number[]): BacktestForecastMetricsJson;
  generateTornadoEntries(resultJson: string, metricNode: string, period?: string): string;
  runMonteCarlo(modelJson: string, configJson: string): string;
  goalSeek(
    modelJson: string,
    targetNode: string,
    targetPeriod: string,
    targetValue: number,
    driverNode: string,
    driverPeriod: string,
    updateModel: boolean,
    boundsLo?: number | null,
    boundsHi?: number | null
  ): GoalSeekResult;
  traceDependencies(modelJson: string, nodeId: string): string;
  explainFormula(modelJson: string, resultsJson: string, nodeId: string, period: string): FormulaExplanationJson;
  explainFormulaText(modelJson: string, resultsJson: string, nodeId: string, period: string): string;
  plSummaryReport(resultsJson: string, lineItems: string[], periods: string[]): string;
  creditAssessmentReport(resultsJson: string, asOf: string): string;
  runChecks(modelJson: string, suiteSpecJson: string, resultsJson?: string | null): string;
  runThreeStatementChecks(modelJson: string, mappingJson: string, resultsJson?: string | null): string;
  runCreditUnderwritingChecks(modelJson: string, mappingJson: string, resultsJson?: string | null): string;
  renderCheckReportText(reportJson: string): string;
  renderCheckReportHtml(reportJson: string): string;
  // Comps — comparable company analysis
  percentileRank(value: number, data: number[]): number | null;
  zScore(value: number, data: number[]): number | null;
  peerStats(data: number[]): PeerStatsJson | null;
  regressionFairValue(
    xValues: number[],
    yValues: number[],
    subjectX: number,
    subjectY: number
  ): RegressionResultJson | null;
  computeMultiple(companyMetrics: unknown, multiple: string): number | null;
  scoreRelativeValue(peerSet: unknown, dimensions: unknown[]): RelativeValueResultJson;
}

export declare const statements_analytics: StatementsAnalyticsNamespace;

// --- portfolio -------------------------------------------------------------

export interface ScenarioRevalueResult {
  valuation: Record<string, unknown>;
  report: Record<string, unknown>;
}

/**
 * Typed handle to a built portfolio. Construct once via
 * `Portfolio.fromSpec` and reuse it across cashflow / valuation calls to
 * skip the per-call `PortfolioSpec` parse + rebuild cost.
 */
export declare class Portfolio {
  private constructor();
  static fromSpec(specJson: string): Portfolio;
  readonly id: string;
  readonly asOf: string;
  readonly baseCcy: string;
  numPositions(): number;
  toSpecJson(): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

export interface PortfolioNamespace {
  /** Typed handle for cached portfolio builds. */
  Portfolio: typeof Portfolio;
  parsePortfolioSpec(jsonStr: string): string;
  /** Compute single-period Brinson-Fachler attribution from sector JSON. */
  brinsonFachler(sectorsJson: string): string;
  /** Compute Carino-linked multi-period Brinson attribution from period JSON. */
  carinoLink(periodsJson: string): string;
  /** Compute a Modified-Dietz TWRR sub-period return from period JSON. */
  twrrModifiedDietz(periodJson: string): number | undefined;
  /** Geometrically link TWRR sub-period returns from returns JSON. */
  twrrLinked(returnsJson: string, horizonYears: number): string | undefined;
  /** Compute money-weighted return via XIRR from dated cashflow JSON. */
  mwrXirr(cashflowsJson: string): number;
  buildPortfolioFromSpec(specJson: string): string;
  portfolioResultTotalValue(resultJson: string): number;
  portfolioResultGetMetric(resultJson: string, metricId: string): number | undefined;
  aggregateMetrics(
    valuationJson: string,
    baseCcy: string,
    marketJson: string,
    asOf: string
  ): string;
  valuePortfolio(specJson: string, marketJson: string, strictRisk: boolean): string;
  /**
   * Fast-path valuation that reuses a built `Portfolio` handle.
   * Skips the `PortfolioSpec` parse + `Portfolio::from_spec` rebuild cost.
   */
  valuePortfolioBuilt(portfolio: Portfolio, marketJson: string, strictRisk: boolean): string;
  aggregateFullCashflows(specJson: string, marketJson: string): string;
  /**
   * Fast-path cashflow aggregation that reuses a built `Portfolio` handle.
   * Skips the `PortfolioSpec` parse + `Portfolio::from_spec` rebuild cost.
   */
  aggregateFullCashflowsBuilt(portfolio: Portfolio, marketJson: string): string;
  applyScenarioAndRevalue(
    specJson: string,
    scenarioJson: string,
    marketJson: string
  ): ScenarioRevalueResult;
  /**
   * Fast-path scenario application that reuses a built `Portfolio` handle.
   * Returns structured JS objects for `valuation` and `report`.
   */
  applyScenarioAndRevalueBuilt(
    portfolio: Portfolio,
    scenarioJson: string,
    marketJson: string
  ): ScenarioRevalueResult;
  /** Optimize portfolio weights using the LP-based optimizer. */
  optimizePortfolio(specJson: string, marketJson: string): string;
  replayPortfolio(specJson: string, snapshotsJson: string, configJson: string): string;
  parametricVarDecomposition(
    positionIdsJson: string,
    weightsJson: string,
    covarianceJson: string,
    confidence: number
  ): string;
  parametricEsDecomposition(
    positionIdsJson: string,
    weightsJson: string,
    covarianceJson: string,
    confidence: number
  ): string;
  historicalVarDecomposition(
    positionIdsJson: string,
    positionPnlsJson: string,
    confidence: number
  ): string;
  evaluateRiskBudget(
    positionIdsJson: string,
    actualVarJson: string,
    targetVarPctJson: string,
    portfolioVar: number,
    utilizationThreshold: number
  ): string;
  rollEffectiveSpread(returnsJson: string): number | undefined;
  amihudIlliquidity(returnsJson: string, volumesJson: string): number | undefined;
  daysToLiquidate(positionValue: number, avgDailyVolume: number, participationRate: number): number;
  liquidityTier(daysToLiquidate: number): string;
  lvarBangia(
    varValue: number,
    spreadMean: number,
    spreadVol: number,
    confidence: number,
    positionValue: number
  ): string;
  almgrenChrissImpact(
    positionSize: number,
    avgDailyVolume: number,
    volatility: number,
    executionHorizonDays: number,
    permanentImpactCoef: number,
    temporaryImpactCoef: number,
    referencePrice?: number | null
  ): string;
  kyleLambda(volumesJson: string, returnsJson: string): number | undefined;
  /** Compute first-order factor sensitivities.
   *
   * ⚠️ BLOCKING: prefer `computeFactorSensitivitiesWithMarket` for repeated
   * calls so market JSON is parsed once into `Market`.
   */
  computeFactorSensitivities(
    positionsJson: string,
    factorsJson: string,
    marketJson: string,
    asOf: string,
    bumpConfigJson?: string
  ): string;
  /** Compute first-order factor sensitivities using a pre-parsed market. */
  computeFactorSensitivitiesWithMarket(
    positionsJson: string,
    factorsJson: string,
    market: Market,
    asOf: string,
    bumpConfigJson?: string
  ): string;
  /** Compute scenario P&L profiles via full repricing. */
  computePnlProfiles(
    positionsJson: string,
    factorsJson: string,
    marketJson: string,
    asOf: string,
    bumpConfigJson?: string,
    nScenarioPoints?: number
  ): string;
  /** Compute scenario P&L profiles via full repricing using a pre-parsed market. */
  computePnlProfilesWithMarket(
    positionsJson: string,
    factorsJson: string,
    market: Market,
    asOf: string,
    bumpConfigJson?: string,
    nScenarioPoints?: number
  ): string;
  /** Decompose portfolio risk into factor and position contributions.
   *
   * ⚠️ BLOCKING: sensitivity and covariance dimensions must match exactly;
   * malformed matrices throw instead of producing partial decompositions.
   */
  decomposeFactorRisk(
    sensitivitiesJson: string,
    covarianceJson: string,
    riskMeasureJson?: string
  ): string;
}

export declare const portfolio: PortfolioNamespace;

// --- scenarios -------------------------------------------------------------

export interface ScenarioWarning {
  kind: string;
  [key: string]: unknown;
}

export interface ScenarioApplyResult {
  market_json: string;
  model_json: string;
  operations_applied: number;
  user_operations: number;
  expanded_operations: number;
  warnings: ScenarioWarning[];
}

export interface ScenarioApplyMarketResult {
  market_json: string;
  operations_applied: number;
  user_operations: number;
  expanded_operations: number;
  warnings: ScenarioWarning[];
}

export interface ScenariosNamespace {
  parseScenarioSpec(jsonStr: string): string;
  composeScenarios(specsJson: string): string;
  validateScenarioSpec(jsonStr: string): boolean;
  listBuiltinTemplates(): string[];
  listBuiltinTemplateMetadata(): string;
  buildFromTemplate(templateId: string): string;
  listTemplateComponents(templateId: string): string[];
  buildTemplateComponent(templateId: string, componentId: string): string;
  buildScenarioSpec(
    id: string,
    operationsJson: string,
    name?: string,
    description?: string,
    priority?: number
  ): string;
  applyScenario(
    scenarioJson: string,
    marketJson: string,
    modelJson: string,
    asOf: string
  ): ScenarioApplyResult;
  applyScenarioToMarket(
    scenarioJson: string,
    marketJson: string,
    asOf: string
  ): ScenarioApplyMarketResult;
  computeHorizonReturn(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    scenarioJson: string,
    method?: string,
    configJson?: string
  ): string;
}

export declare const scenarios: ScenariosNamespace;
