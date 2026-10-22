# Calibration

Plan-driven construction of discount, forward, hazard, inflation, cross-currency
basis, parametric, volatility, and base-correlation structures from market
quotes. This is the canonical path from raw quotes to a
`finstack_quant_core::market_data::context::MarketContext`.

`MarketContext::try_from(MarketContextState)` is the *snapshot* deserializer — it
rehydrates a previously saved context. It does not build one from quotes. Use it
only to replay an already-calibrated context.

## Layout

| Path | Visibility | Contents |
|------|-----------|----------|
| `api/` | public | `CalibrationEnvelope` schema, `engine::calibrate`, envelope validation, market-datum and prior-market inputs |
| `recalibration/` | public | Cached quote-space replay implementing valuations' `RecalibrationProvider` port |
| `quotes/` | public | Raw market quote DTOs and quote identifiers |
| `build/` | crate-private | Quote-to-instrument construction and date resolution |
| `hull_white/` | public | Hull-White 1F calibration to swaptions and cap/floors |
| `solver/` | crate-private | Sequential bootstrap, global fit (Newton/LM), multi-start, shared helpers; re-exports `SolverConfig` |
| `targets/` | crate-private | Per-step targets: discount, forward, hazard, inflation, vol, swaption, SVI, base correlation, Student-t, XCCY basis, parametric |
| `validation/` | public | Curve/surface validators and rate bounds; preflight implementation is private |
| `config.rs` | re-exported | `CalibrationConfig` and the per-curve solve configs |
| `report.rs` | re-exported | `CalibrationReport`, `CalibrationDiagnostics`, `QuoteQuality` |
| `step_runtime.rs` | crate-private | Step execution plumbing |
| `lmm.rs` | private | Bermudan LMM implementation; its two calibration functions are exported at the crate root |
| `versions.rs` | crate-private | Methodology identifiers recorded in reports |
| `constants.rs` | crate-private | Shared numerical thresholds for the solvers and targets |

Public re-exports from `finstack_quant_calibration`: `CalibrationConfig`,
`CalibrationMethod`, `DiscountCurveSolveConfig`, `ForwardCurveSolveConfig`,
`HazardCurveSolveConfig`, `InflationCurveSolveConfig`, `VolSurfaceSolveConfig`,
`RatesStepConventions`, `ResidualWeightingScheme`, `MarketFreshnessPolicy`,
`MarketQuoteSide`, `SolverConfig`, `CalibrationReport`, `CalibrationDiagnostics`,
`QuoteQuality`, `ResidualUnits`, `SolverMethod`, `FittedSlice`, and
`FittedSliceParameters`. The root also exports
`calibrate_bermudan_lmm_base_vol` and `calibrate_bermudan_lmm_base_vol_from_json`.
`RateBounds`, `RateBoundsPolicy`, `ValidationConfig`, `ValidationMode`,
`CurveValidator`, and the surface validators live in
`finstack_quant_calibration::validation`. Quote types are exported directly
from `quotes`; their implementation submodules are private.

## Envelope structure

A `CalibrationEnvelope` (schema marker `finstack_quant.calibration/1`) carries
quotes in two complementary tracks:

- **Track A — bootstrapping.** `plan.quote_sets` names ID lists resolved against
  `market_data`; `plan.steps` consume them. Step kinds mirror the `StepParams`
  variants: `discount`, `forward`, `hazard`, `inflation`, `vol_surface`,
  `swaption_vol`, `base_correlation`, `student_t`, `hull_white`,
  `cap_floor_hull_white`, `svi_surface`, `xccy_basis`, `parametric`.
- **Track B — snapshot data.** FX matrices, bond prices, equity spots, and
  dividend schedules are not bootstrapped. Supply them as `market_data` entries
  (`fx_spot`, `price`, `dividend_schedule`), or pass already-calibrated objects
  in `prior_market`.

Both tracks may appear in the same envelope; the engine merges `market_data` and
`prior_market` into the working context before running steps.

`plan.settings.market_freshness` records the RFC3339 snapshot timestamp,
maximum permitted age, and selected quote side (`mid`, `bid`, or `ask`).
Execution rejects incomplete, malformed, future, or stale freshness assertions
and stamps the plan report as `verified` or `unverified`. Quotes remain
single-sided values rather than bid/ask pairs, so crossed-market validation is
not applicable inside this schema; upstream ingestion must reject a crossed
pair before selecting the side placed in the envelope.

### Strict loading

All bounded parser, schema-marker, schema-version, and envelope-structure
failures use the single `EnvelopeError::StrictLoad` variant and serialize with
`"kind": "strict_load"`. The underlying contract diagnostic remains in the
error message; callers should branch on the canonical kind rather than parsing
that message.

## Executing a plan

```rust
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan,
};
use finstack_quant_calibration::api::engine;

fn run(plan: CalibrationPlan) -> finstack_quant_core::Result<MarketContext> {
    let envelope = CalibrationEnvelope::new(plan, Vec::new(), Vec::new());

    let result = engine::calibrate(&envelope)?;
    println!("executed {} steps", result.result.step_reports.len());

    MarketContext::try_from(result.result.final_market)
}
```

`engine::calibrate` returns a `CalibrationResultEnvelope` whose `result` carries
`final_market` (a `MarketContextState`), a merged plan-level `report`,
`step_reports` keyed by step id, and `results_meta`. Failures are a structured
`ExecuteError` (including `worst_quote_id` on solver non-convergence). Static
validation is fail-fast; `dry_run` lists every static error without solving.
`From<ExecuteError>` maps to `finstack_quant_core::Error` so `?` still works
in `core::Result` functions.

Both hosts take the same envelope JSON but hand back different shapes.
`finstack_quant.calibration.calibrate(envelope_json)` returns a
`CalibrationResultEnvelope` object whose `.market` getter rebuilds a
`MarketContext`, alongside `.success`, `.report`, `.step_ids`, and `.iterations`.
Per-step reports are available through `.step_report(step_id)`; residual
statistics are fields of these reports. The WASM `calibration.calibrate(envelope)` has no
such wrapper: it returns a plain `CalibrationResultEnvelope` object mirroring
the Rust wire type, so a caller writes `calibrate(env).result.final_market` and
re-ingests that sub-document rather than reading a `.market` property. Runnable
envelope examples live in
[`../examples/market_bootstrap/`](../examples/market_bootstrap/README.md).

## Configuration

### Two tolerances

1. **Solver tolerance** (`config.solver.tolerance()`, default `1e-12`) — when
   the numerical solver stops. `SolverConfig` stores the convergence tolerance
   and iteration budget shared by the root-finding and global solvers; this
   stopping criterion is distinct from economic fit.
2. **Validation tolerance** — whether the calibration counts as successful.
   After the solver converges, final residuals are compared against the
   step's success tolerance; any residual above the threshold marks the
   report failed.

A precise root is not the same as an accurate reprice, which is why both exist.
Curve validation tolerances are per-unit-notional residuals. Vol-surface
residuals are decimal implied vols.

| Setting | Default | Residual unit | Used by |
|---------|---------|---------------|---------|
| `solver.tolerance()` | `1e-12` | parameter space | all numerical solvers |
| `discount_curve.validation_tolerance` | `1e-8` | PV / notional | discount, xccy |
| `forward_curve.validation_tolerance` | `1e-8` | PV / notional | forward |
| `hazard_curve.validation_tolerance` | `1e-8` | PV / notional | hazard |
| `inflation_curve.validation_tolerance` | `1e-8` | PV / notional | inflation |
| `vol_surface.validation_tolerance` | `1e-3` | decimal implied vol | SABR and SVI surfaces |

`CalibrationConfig::fail_on_bad_fit` defaults to `true`: a step whose
`report.success` is false is propagated as
`finstack_quant_core::Error::Calibration` and its output is **not** installed
into the context. Diagnostic workflows that want the report without aborting can
set it to `false`.

`CalibrationConfig::use_parallel` defaults to `false` for determinism.

### Precedence

1. **Step-level** — `CalibrationStep.params` (a `StepParams` variant carrying,
   for example, `DiscountCurveParams { method, .. }`). Highest priority.
2. **Plan-level** — `CalibrationPlan.settings` (`CalibrationConfig`).
3. **Global defaults** — `CalibrationConfig::default()`.

Curve calibration methods are configured per step through `params.method`.
Global settings contain numerical and validation policy.

`CalibrationMethod` is `Bootstrap` (default) or
`GlobalSolve { use_analytical_jacobian: bool }`.

### Forward-curve method policy

Forward-curve steps **require** `GlobalSolve`. `Bootstrap` is rejected with a
validation error rather than silently reinterpreted: projection discount factors
chain the actual contractual reset/end-date grid, so calendar-adjusted periods
couple adjacent reset rates and must be solved simultaneously.

Forward-curve interpolation knots remain simple fixed-tenor rate controls.
Calibrated curves separately store a validated contractual `projection_grid` of
reset/end-date boundaries, keeping `rate(reset)` and DF-implied
`rate_between(reset, end)` coherent for off-grid 3M periods such as 91- or
92-day Act/360 accruals. Sparse or older curves without that optional grid fall
back to fixed numeric-tenor stepping from zero.

The global forward target enforces `CalibrationConfig::effective_rate_bounds`
per fitted reset-rate parameter and reads `forward_curve.weighting_scheme` and
`forward_curve.validation_tolerance`.

### Interpolation

Step `interpolation` is caller-owned. The engine does not pick a production
default. `Linear` interpolates the stored ordinates — discount factors on a
discount or XCCY curve, forward rates on a forward curve — and is **not** the
QuantLib or Bloomberg production choice. Use `LogLinear` (log-DF) or
`MonotoneConvex` (Hagan–West) for production discount curves.

### Cross-currency `fx_spot`

`XccyBasisParams.fx_spot` is the T+0 cash FX (domestic per foreign). Screen
spot is T+2 for most G10 pairs and T+1 for USD/CAD; convert to T+0 with ON/TN
points before passing the rate. Mixing T+2 screen spot with T+0 discounting
biases long-tenor basis by roughly 1–2 bp. The engine does not convert
settlement lag.

### Rate bounds

`rate_bounds_policy` defaults to `RateBoundsPolicy::AutoCurrency`, deriving
bounds per currency. Set it to `Explicit` to use the `rate_bounds` field
verbatim. `effective_rate_bounds(currency)` resolves the pair.

### Recommended settings

| Use case | Solver tolerance | Validation tolerance | Method |
|----------|------------------|----------------------|--------|
| Forward curves | `1e-12` | `1e-8` | `GlobalSolve` (required) |
| Discount curves | `1e-12` | `1e-8` | `Bootstrap` or `GlobalSolve` |
| Smooth curve fitting | `1e-10` | `1e-8` | `GlobalSolve` |
| Distressed credit | `1e-10` | `1e-6` | `Bootstrap` |
| Real-time pricing | `1e-6` | `1e-4` | target-dependent |
| Interactive exploration | `1e-4` | `1e-2` | target-dependent |

### `compute_diagnostics`

Defaults to `false` to keep solver runs lean. Enabling it adds one
finite-difference Jacobian evaluation per parameter after convergence and
produces:

- per-quote sensitivity: max `|d residual / d param|` per quote
- condition number: `cond(J^T J)` for the finite-difference Jacobian
- consistent RMS / max residual reporting across solvers

Global-solve failures always include the three worst-fit quotes in
`convergence_reason`, diagnostics on or off.

## Reports

`CalibrationReport` carries `success`, `residuals` (a `BTreeMap` keyed by
instrument id, so ordering is stable), `iterations`, `objective_value`,
`max_residual`, `rmse`, `validation_passed`, `validation_error`,
`convergence_reason`, `metadata`, `solver_config`, and `results_meta`.
`solver_method` and `residual_units` are typed fields; missing units remain
unknown. The metadata map does not provide a fallback. Optional diagnostics
include `diagnostics`, `worst_quote_id`, `worst_quote_residual`, `explanation`,
and `model_version`; fitted surface parameters are stored in `fitted_slices`.

Per-quote rows contain `quote_label`, `quote_value`, `residual`, and
`sensitivity`. Quote values retain their native units, including decimal
fractions for CDS/tranche upfronts. Residuals use the report's declared units.
Rows without recorded diagnostics omit `quote_value` and carry `NaN` for
`sensitivity`; Python's DataFrame represents the missing quote value as `NaN`.

Surface validation requires one positive finite forward per expiry. The
standalone validators return errors for acceptance violations. Within a plan,
`validation_mode=error` marks the report unsuccessful; `warn` records a
`validation_warning` without changing existing fit success. Disabling
`check_arbitrage` skips arbitrage checks while retaining forward-input and
volatility-bound validation. `fail_on_bad_fit` controls whether an unsuccessful
report aborts execution.

## Recalibration

`CachedRecalibrationProvider` implements the valuations-owned
`RecalibrationProvider` port. Quote-space rate and CDS spread shocks use
`QuoteBump` (parallel basis points or ordered tenor basis points) and replay the
stored construction recipe. Direct discount, inflation, volatility, and model
hazard-intensity shocks use core `BumpSpec`/`Bumpable` operations instead.

For quote-recalibrated shocks, both base and bumped curves reprice every input quote to
within their calibration tolerances, so residual leakage into a sensitivity is
bounded by roughly (sum of the two repricing tolerances) / (bump size) — on the
order of `2e-10 / 1e-4 = 2e-6` of the PV unit at a 1bp bump, negligible versus
the bump.

## Determinism

- Fixed inputs give identical outputs: Halton multi-start, no system RNG.
- Residual keys use `BTreeMap` ordering.
- Solver loops reuse buffers; parallelism is opt-in via `use_parallel`.
- A parallel batch in which several steps fail reports the first failing step
  in plan order, the same error as the sequential run and as the wasm32 build.
- Outputs are reproducible per build target. `f64` results from native
  (Python) and wasm32 (WASM) builds can differ at the ulp level, and multi-start
  diagnostics such as iteration counts can differ with them, so cross-host
  comparisons must use a tolerance (see `INVARIANTS.md` §2.1).

## Extending

**New calibration target**: implement the target trait in `solver/` (bootstrap
targets implement `BootstrapTarget`; global targets implement
`GlobalSolveTarget`), add the target module under `targets/`, add a `StepParams`
variant plus its params struct in `api/schema.rs`, and wire it through
`targets/mod.rs` and the engine.

**New quote-driven instrument**: define the quote type under
[`quotes/`](quotes/), add a builder under `build/`, then teach the
relevant target to build and price it.

Regenerate schemas after any public wire change: `mise run rust-gen-schemas`,
verify with `mise run rust-check-schemas`.

## Verification

```bash
mise run rust-test-crate -- finstack-quant-calibration
mise run rust-bench-crate -- finstack-quant-calibration calibration
mise run rust-bench-crate -- finstack-quant-calibration global_calibration
```
