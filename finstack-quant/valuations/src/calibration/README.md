# Calibration

Plan-driven calibration of discount, forward, hazard, inflation, volatility, and base-correlation structures from market quotes.

## Functionality

This module supports:

- **Interest Rate Curves**: Discount and forward curves using OIS, swaps, futures, and fra.
- **Credit Curves**: Survival and hazard rate curves from CDS and credit indices.
- **Inflation Curves**: Inflation-indexed curves.
- **Volatility Surfaces**: SABR and other volatility models.
- **Base Correlation**: For credit tranches.

## Structure

The module is organized into several key areas:

- `api/`: Defines the structured calibration schema and execution engine.
- `solver/`: Contains core numerical solvers (Sequential Bootstrap, Levenberg-Marquardt).
- `targets/`: Core logic for instrument-specific calibration targets (Bootstrappers).
- `prepared.rs`: Internal calibration quote envelopes (wrapping market-level quotes).
- `validation/`: Runtime validation of calibrated structures.
- `bumps/`: Support for re-calibration and risk sensitivities.

## Usage Examples

### Executing a Calibration Plan

```rust
use finstack_quant_valuations::calibration::api::engine;
use finstack_quant_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CALIBRATION_SCHEMA,
};

fn run_calibration(plan: CalibrationPlan) -> finstack_quant_core::Result<()> {
    let envelope = CalibrationEnvelope {
        schema: CALIBRATION_SCHEMA.to_string(),
        plan,
        initial_market: None,
    };

    let result = engine::execute(&envelope)?;
    println!("Calibrated {} structures", result.calibrated_structures.len());
    Ok(())
}
```

## Configuration Guide

### Tolerance Semantics

Calibration involves two distinct tolerance concepts that control different aspects:

1. **Solver Tolerance** (`config.solver.tolerance()`):
   - Controls when the numerical solver (Brent/Newton) terminates
   - This is an algorithmic convergence criterion in parameter space
   - The solver stops when successive parameter estimates differ by less than this tolerance
   - Default: `1e-12`

2. **Validation Tolerance** (`config.discount_curve.validation_tolerance`, etc.):
   - Controls whether calibration is considered *successful*
   - After the solver converges, final residuals are compared against this tolerance
   - If any residual exceeds `validation_tolerance`, calibration is marked as failed
   - Default: `1e-8` (suitable for per-unit-notional residuals)

**Why two tolerances?**

- Solver tolerance ensures numerical convergence but doesn't guarantee economic fit
- Validation tolerance ensures the calibrated curve actually prices instruments correctly
- For well-behaved problems, solver tolerance of `1e-12` with validation tolerance of `1e-8` works well: the solver finds a precise root, and we verify it prices accurately

### Configuration Hierarchy

Settings can be specified at multiple levels with the following precedence:

1. **Step-level** (`CalibrationStep.params.method`): Per-instrument-type overrides (highest priority)
2. **Plan-level** (`CalibrationPlan.settings`): Plan-wide defaults
3. **Global defaults** (`CalibrationConfig::default()`): Fallback values

Step-level settings always take precedence over plan-level settings. For example:

```rust
// Plan-level default: Bootstrap
let plan = CalibrationPlan {
    settings: CalibrationConfig::default(), // Uses Bootstrap by default
    steps: vec![
        CalibrationStep {
            // Step-level override: GlobalSolve for this specific curve
            params: StepParams::Discount(DiscountCurveParams {
                method: CalibrationMethod::GlobalSolve { use_analytical_jacobian: true },
                ..
            }),
            ..
        }
    ],
    ..
};
```

### Recommended Settings

| Use Case | Solver Tolerance | Validation Tolerance | Method |
|----------|------------------|---------------------|--------|
| Production risk systems | `1e-12` | `1e-8` | Bootstrap |
| Real-time pricing | `1e-6` | `1e-4` | Bootstrap |
| Interactive exploration | `1e-4` | `1e-2` | Bootstrap |
| Smooth curve fitting | `1e-10` | `1e-8` | GlobalSolve |
| Distressed credit | `1e-10` | `1e-6` | Bootstrap |

### `compute_diagnostics`

`CalibrationConfig::compute_diagnostics` defaults to `false` to keep
solver runs lean. Enable it for production calibrations that need the
extra post-solve diagnostics:

- **Per-quote sensitivity**: maximum |d residual / d param| for each quote.
- **Condition number**: `cond(J^T J)` for the finite-difference Jacobian.
- **RMS / max residual**: consistent residual reporting across solvers.

The extra cost is one finite-difference Jacobian evaluation per parameter
after convergence. The global-solve path still includes the top-3
worst-fit quotes in `convergence_reason` on failure, even when
`compute_diagnostics` is disabled.

## Adding New Features

### Adding a New Calibration Target

1. Implement the `BootstrapTarget` trait in `solver/` (if using bootstrapping).
2. Create a new target/bootstrapper in `targets/`.
3. Register the new target in the `api` engine and `targets/handlers.rs`.

### Adding a New Instrument Type

1. Define the instrument's quote type in `market/quotes/`.
2. Update the `targets/` logic to support building and pricing the new instrument.

## Reliability

- Solver loops reuse buffers where possible; residual keys use stable `BTreeMap` ordering.
- Runs are deterministic for fixed inputs (Halton multi-start, no system RNG).
- Global-solve failures include the top three worst-fit quotes in `convergence_reason`; set `compute_diagnostics = true` for full Jacobian diagnostics.
