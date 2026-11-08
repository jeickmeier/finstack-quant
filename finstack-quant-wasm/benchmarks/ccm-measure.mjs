import assert from 'node:assert/strict';

// Fixed before the first CCM-B2 host run. Units below are milliseconds per call.
export const CCM_NUMERICS = Object.freeze({
  scalar_absolute: 1e-12,
  scalar_relative: 1e-12,
  matrix_absolute: 1e-11,
  statistics_absolute: 1e-10,
  black_scholes_absolute: 2e-10,
  normal_cdf_042: 0.6627572731517506,
  gaussian_conditional: 0.26268639318685094,
  black_scholes_call: 10.450583572185558,
  black_scholes_put: 5.573526022256971,
});

// Batch sizes preserve each old row's measured call count at ten samples.
export const CCM_NODE_CASES = Object.freeze(
  [
    ['core/currency_constructor', 'core', 'Currency constructor', 500],
    ['core/money_add_sub', 'core', 'Money add / sub', 800],
    ['core/daycount_year_fraction', 'core', 'DayCount.yearFraction', 1000],
    ['core/discount_curve_df', 'core', 'DiscountCurve.df', 1500],
    ['core/cholesky_decomposition', 'core', 'choleskyDecomposition', 300],
    ['core/mean_variance_512', 'core', 'mean + variance (512)', 500],
    ['core/rate_bps', 'core', 'Rate / Bps conversions', 1200],
    ['core/tenor_parse', 'core', 'Tenor.parse + toYears', 800],
    ['core/forward_curve', 'core', 'ForwardCurve construction + rate()', 400],
    ['core/fx_matrix', 'core', 'FxMatrix setQuotes + rate', 400],
    ['core/cholesky_solve', 'core', 'choleskySolve', 500],
    ['core/norm_cdf', 'core', 'normCdf', 2000],
    ['core/quantile_512', 'core', 'quantile (512)', 600],
    ['core/kahan_sum_512', 'core', 'kahanSum (512)', 600],
    ['core/available_calendars', 'core', 'availableCalendars', 80],
    ['core/adjust', 'core', 'adjust', 400],
    ['correlation/bounds', 'correlation', 'correlationBounds', 2000],
    ['correlation/bernoulli', 'correlation', 'CorrelatedBernoulli.jointProbabilities', 2000],
    ['correlation/gaussian', 'correlation', 'Gaussian conditionalDefaultProb', 1500],
    ['correlation/recovery', 'correlation', 'Constant conditionalRecovery', 1500],
    ['models/bs_call', 'models', 'bsPrice (call)', 2000],
    ['models/bs_put', 'models', 'bsPrice (put)', 2000],
  ].map(([id, domain, name, batch_size]) => Object.freeze({ id, domain, name, batch_size }))
);

/** Parse only the benchmark's supported selectors; no WASM module is loaded. */
export function parseBenchmarkMode(arguments_) {
  const modes = ['--materialization-only', '--valuation-only', '--core-models-only'];
  const allowed = new Set([...modes, '--help', '-h']);
  for (const argument of arguments_) {
    if (!allowed.has(argument)) throw new Error(`Unknown benchmark argument: ${argument}`);
  }
  const selected = arguments_.filter((argument) => modes.includes(argument));
  if (selected.length > 1) throw new Error('Benchmark mode selectors are mutually exclusive');
  return {
    help: arguments_.includes('--help') || arguments_.includes('-h'),
    mode: selected[0] ?? 'all',
  };
}

function integerSetting(environment, name, fallback, minimum) {
  const raw = environment[name] ?? String(fallback);
  const value = Number(raw);
  if (!/^(0|[1-9]\d*)$/.test(raw) || !Number.isSafeInteger(value) || value < minimum) {
    throw new Error(`${name} must be an integer at least ${minimum}, got ${raw}`);
  }
  return value;
}

/** Read the explicit host protocol; Criterion variables have no effect here. */
export function ccmSettings(environment) {
  return {
    samples: integerSetting(environment, 'FQ_CCM_JS_SAMPLES', 10, 1),
    warmup_samples: integerSetting(environment, 'FQ_CCM_JS_WARMUP_SAMPLES', 1, 0),
  };
}

/** Dispose every handle even if one destructor fails; retain the first error. */
export function disposeAll(handles) {
  let firstError;
  for (const handle of handles) {
    try {
      handle.free();
    } catch (error) {
      firstError ??= error;
    }
  }
  if (firstError) throw firstError;
}

/** Transfer a group of handles on success, or dispose a partial construction. */
export function captureOwned(build) {
  const handles = [];
  const own = (handle) => {
    handles.push(handle);
    return handle;
  };
  try {
    return { handles, output: build(own) };
  } catch (error) {
    try {
      disposeAll(handles.reverse());
    } catch {
      // Preserve the operation error after attempting every destructor.
    }
    throw error;
  }
}

/**
 * Measure a fixed batch with full result observation and explicit disposal.
 * Preflight, every output check/snapshot, buffer allocation and disposal are
 * outside timing. Each timed batch retains at most batchSize results. A partial
 * failed batch is disposed and rejected rather than returned as observations.
 */
export function measureCheckedBatches({
  operation,
  validate,
  snapshot = (value) => value,
  cleanup = () => {},
  batchSize,
  samples,
  warmupSamples,
  clock = () => performance.now(),
}) {
  assert.ok(Number.isSafeInteger(batchSize) && batchSize > 0, 'positive batch size');
  assert.ok(Number.isSafeInteger(samples) && samples > 0, 'positive sample count');
  assert.ok(Number.isSafeInteger(warmupSamples) && warmupSamples >= 0, 'nonnegative warmup count');

  function runBatch(count, timed) {
    const results = new Array(count);
    let produced = 0;
    let firstError;
    let duration;
    let observed;
    try {
      const started = timed ? clock() : undefined;
      for (; produced < count; produced++) results[produced] = operation();
      if (timed) {
        duration = clock() - started;
        assert.ok(Number.isFinite(duration) && duration > 0, 'positive finite batch duration');
      }
      for (const result of results) validate(result);
      observed = snapshot(results[count - 1]);
    } catch (error) {
      firstError = error;
    }
    for (let index = 0; index < produced; index++) {
      try {
        cleanup(results[index]);
      } catch (error) {
        firstError ??= error;
      }
    }
    if (firstError) throw firstError;
    return { duration, observed };
  }

  const preflightOutput = runBatch(1, false).observed;
  for (let index = 0; index < warmupSamples; index++) runBatch(batchSize, false);
  const batchMilliseconds = [];
  const observedOutputs = [];
  for (let index = 0; index < samples; index++) {
    const batch = runBatch(batchSize, true);
    batchMilliseconds.push(batch.duration);
    observedOutputs.push(batch.observed);
  }
  const perCall = batchMilliseconds.map((duration) => duration / batchSize);
  const sorted = [...perCall].sort((a, b) => a - b);
  const midpoint = Math.floor(samples / 2);
  const median = samples % 2 ? sorted[midpoint] : (sorted[midpoint - 1] + sorted[midpoint]) / 2;
  const avg = perCall.reduce((sum, duration) => sum + duration, 0) / samples;
  return {
    batch_size: batchSize,
    measured_batches: samples,
    warmup_batches: warmupSamples,
    iterations: batchSize * samples,
    preflight_calls: 1,
    warmup_calls: batchSize * warmupSamples,
    batch_milliseconds: batchMilliseconds,
    samples: perCall,
    preflight_output: preflightOutput,
    observed_outputs: observedOutputs,
    best: sorted[0],
    avg,
    median,
    p95: sorted[Math.ceil(samples * 0.95) - 1],
    opsPerSec: 1000 / avg,
  };
}
