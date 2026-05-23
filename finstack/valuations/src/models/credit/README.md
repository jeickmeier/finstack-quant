# Credit Models

Structural credit models for default probability estimation, PIK/cash toggle decisions,
dynamic recovery, and endogenous hazard rates. These models power the Monte Carlo pricing
engine for bonds with pay-in-kind (PIK) features and can be used standalone for credit
analytics.

## Module Structure

```
credit/
├── mod.rs                 # Public re-exports
├── merton.rs              # Merton / Black-Cox structural model
├── toggle_exercise.rs     # PIK vs cash toggle decision models
├── dynamic_recovery.rs    # Notional-dependent recovery rates
└── endogenous_hazard.rs   # Leverage-dependent hazard rates
```

## Features

### Merton Structural Model (`merton.rs`)

Models a firm's equity as a call option on its assets. Default occurs when
the asset value falls below the debt barrier.

**Core analytics**

| Method                  | Description                                                      |
|-------------------------|------------------------------------------------------------------|
| `distance_to_default()` | DD = (ln(V/B) + (r - q - σ²/2)·T) / (σ√T)                      |
| `default_probability()` | Terminal: PD = N(-DD). First-passage: Black-Cox closed-form      |
| `implied_spread()`      | s = -ln(1 - PD·(1-R)) / T                                       |
| `implied_equity()`      | Black-Scholes call formula with continuous payout rate            |
| `to_hazard_curve()`     | Converts structural PD to piecewise-constant hazard curve        |

**Asset dynamics**

- `GeometricBrownian` — standard lognormal diffusion (GBM).
- `JumpDiffusion` — Merton (1976) Poisson-compensated jumps on top of GBM.
- `CreditGrades` — simplified Finger et al. (2002) with deterministic barrier.

**Barrier types**

- `Terminal` — classic Merton; default assessed only at maturity.
- `FirstPassage` — Black-Cox continuous monitoring with exponential barrier growth.

**Calibration**

| Constructor        | Calibrates from                                                  |
|--------------------|------------------------------------------------------------------|
| `new()`            | Direct specification (V, σ, B, r)                                |
| `from_equity()`    | KMV fixed-point iteration from observed equity value and vol     |
| `from_cds_spread()`| Brent solver on σ to match a target CDS spread                   |
| `from_target_pd()` | Brent solver on B to match a target cumulative PD                |
| `credit_grades()`  | CreditGrades construction from equity observables                |

**Monte Carlo**

`simulate_paths()` generates forward asset-value paths under GBM or
jump-diffusion dynamics with optional antithetic variates.

### Toggle Exercise (`toggle_exercise.rs`)

Decides whether the borrower pays in kind (PIK) or pays cash at each coupon date.

| Model              | Decision rule                                                         |
|--------------------|-----------------------------------------------------------------------|
| `Threshold`        | PIK when a credit metric (hazard rate, DD, leverage) crosses a boundary |
| `Stochastic`       | PIK probability via logistic sigmoid: P = 1/(1 + exp(-(a + b·x)))    |
| `OptimalExercise`  | Nested Monte Carlo comparing equity value under cash vs PIK scenarios |

The optimal exercise model runs a small nested GBM simulation at each coupon date
with first-passage barrier checks, including a liquidity early-exit guard that
forces PIK when cash payment would breach the default barrier.

### Dynamic Recovery (`dynamic_recovery.rs`)

Recovery rates that decline as PIK accrual inflates the outstanding notional.

| Model            | Formula                                                    |
|------------------|------------------------------------------------------------|
| `Constant`       | R(t) = R₀                                                 |
| `InverseLinear`  | R(t) = R₀ · (N₀ / N(t))                                  |
| `InversePower`   | R(t) = R₀ · (N₀ / N(t))^α                                |
| `FlooredInverse` | R(t) = max(floor, R₀ · (N₀ / N(t)))                      |
| `LinearDecline`  | R(t) = clamp(R₀ · (1 - β · (N(t)/N₀ - 1)), floor, R₀)   |

All outputs are clamped to `[0, base_recovery]`.

### Endogenous Hazard (`endogenous_hazard.rs`)

Creates a feedback loop: PIK accrual increases leverage, which drives the
hazard rate higher.

| Model          | Formula                                        |
|----------------|------------------------------------------------|
| `PowerLaw`     | λ(L) = λ₀ · (L / L₀)^β                       |
| `Exponential`  | λ(L) = λ₀ · exp(β · (L - L₀))                |
| `Tabular`      | Linear interpolation with flat extrapolation   |

All outputs are floored at 0.

## Integration with Pricing Engines

The credit models feed into `MertonMcEngine` (the Monte Carlo bond pricer):

```
MertonMcConfig
├── merton: MertonModel           ← asset dynamics, barrier, calibration
├── pik_schedule: PikSchedule     ← per-coupon cash/PIK/toggle behavior
├── endogenous_hazard: Option<EndogenousHazardSpec>
├── dynamic_recovery: Option<DynamicRecoverySpec>
└── toggle_model: Option<ToggleExerciseModel>
```

**Simulation loop** (per path, per time step):

1. Evolve asset value via GBM or jump-diffusion.
2. Compute hazard rate from `EndogenousHazardSpec` (if present) or from the
   Merton model directly.
3. Check for first-passage default against the barrier.
4. At coupon dates, evaluate `ToggleExerciseModel` to decide PIK vs cash
   (when `PikMode::Toggle` is active).
5. On default, compute recovery via `DynamicRecoverySpec` (if present).

## Usage Examples

### Rust

```rust
use finstack_valuations::models::credit::{
    MertonModel, BarrierType, AssetDynamics,
    DynamicRecoverySpec, EndogenousHazardSpec, ToggleExerciseModel,
};
use finstack_valuations::models::credit::toggle_exercise::{
    CreditState, CreditStateVariable, ThresholdDirection,
};

// --- Merton model: direct construction ---
let model = MertonModel::new(100.0, 0.20, 80.0, 0.05)?;
let dd = model.distance_to_default(1.0);    // ~1.27
let pd = model.default_probability(1.0);     // ~10.3%
let spread = model.implied_spread(5.0, 0.40); // implied credit spread

// --- Calibrate from equity observables (KMV) ---
let model = MertonModel::from_equity(
    25.0,   // equity_value
    0.50,   // equity_vol
    80.0,   // total_debt
    0.05,   // risk_free_rate
    0.0,    // payout_rate
    1.0,    // maturity
)?;

// --- Calibrate barrier from target PD ---
let annual_pd = 0.02;
let five_year_pd = 1.0 - (-annual_pd * 5.0_f64).exp();
let model = MertonModel::from_target_pd(200.0, 0.25, 0.045, five_year_pd, 5.0)?;

// --- First-passage (Black-Cox) with growing barrier ---
let model = MertonModel::new_with_dynamics(
    100.0, 0.25, 80.0, 0.05, 0.0,
    BarrierType::FirstPassage { barrier_growth_rate: 0.02 },
    AssetDynamics::GeometricBrownian,
)?;

// --- Generate hazard curve for use with other engines ---
let hc = model.to_hazard_curve("ISSUER_001", base_date, &[1.0, 3.0, 5.0, 10.0], 0.40)?;

// --- Dynamic recovery ---
let dyn_rec = DynamicRecoverySpec::floored_inverse(0.40, 100.0, 0.15)?;
let recovery = dyn_rec.recovery_at_notional(130.0); // R declines as notional accretes

// --- Endogenous hazard ---
let endo = EndogenousHazardSpec::power_law(0.05, 0.60, 2.0)?;
let lambda = endo.hazard_at_leverage(0.75); // hazard rises with leverage

// --- Toggle exercise ---
let toggle = ToggleExerciseModel::threshold(
    CreditStateVariable::HazardRate, 0.15, ThresholdDirection::Above,
);
```

### Python

```python
from finstack.valuations import (
    MertonModel, MertonMcConfig, EndogenousHazardSpec,
    DynamicRecoverySpec, ToggleExerciseModel, Bond,
)
import math

# Calibrate Merton from target PD
annual_pd = 0.02
five_year_pd = 1.0 - math.exp(-annual_pd * 5.0)
merton = MertonModel.from_target_pd(
    asset_value=200.0,
    asset_vol=0.25,
    risk_free_rate=0.045,
    target_pd=five_year_pd,
    maturity=5.0,
)

# Build credit components
endo = EndogenousHazardSpec.power_law(
    base_hazard=0.02, base_leverage=0.60, exponent=2.0,
)
dyn_rec = DynamicRecoverySpec.floored_inverse(
    base_recovery=0.40, base_notional=100.0, floor=0.15,
)
toggle = ToggleExerciseModel.threshold(
    variable="hazard_rate", threshold=0.15, direction="above",
)

# Assemble MC config
config = MertonMcConfig(
    merton,
    endogenous_hazard=endo,
    dynamic_recovery=dyn_rec,
    toggle_model=toggle,
    num_paths=50_000,
    seed=42,
    antithetic=True,
)

# Price a PIK toggle bond
result = bond.price_merton_mc(config, discount_rate=0.05, as_of=as_of_date)
```

## Academic References

| Model / Concept        | Reference                                                                                                          |
|------------------------|--------------------------------------------------------------------------------------------------------------------|
| Structural default     | Merton, R. C. (1974). "On the Pricing of Corporate Debt: The Risk Structure of Interest Rates." *JF*, 29(2), 449-470. |
| First-passage barrier  | Black, F. & Cox, J. C. (1976). "Valuing Corporate Securities: Some Effects of Bond Indenture Provisions." *JF*, 31(2), 351-367. |
| Jump-diffusion         | Merton, R. C. (1976). "Option Pricing When Underlying Stock Returns Are Discontinuous." *JFE*, 3(1-2), 125-144.    |
| CreditGrades           | Finger, C., Finkelstein, V., Pan, G., Lardy, J.-P., Ta, T., & Tierney, J. (2002). *CreditGrades Technical Document*. RiskMetrics Group. |
| KMV calibration        | Hull, J. C. *Options, Futures, and Other Derivatives*, 9th ed., Chapter 17.                                         |

## Adding New Features

### Adding a new recovery model

1. Add a variant to `RecoveryModel` in `dynamic_recovery.rs`.
2. Implement the formula in `DynamicRecoverySpec::recovery_at_notional()`.
3. Add a convenience constructor (e.g., `DynamicRecoverySpec::new_model_name()`).
4. Add unit tests verifying the formula, edge cases, and clamping behavior.
5. Expose the new variant in the Python bindings
   (`finstack-py/src/valuations/instruments/credit/dynamic_recovery.rs`).

### Adding a new hazard mapping

1. Add a variant to `LeverageHazardMap` in `endogenous_hazard.rs`.
2. Implement the formula in `EndogenousHazardSpec::hazard_at_leverage()`.
3. Add a convenience constructor.
4. Add unit tests (base leverage returns base hazard, monotonicity, edge cases).
5. Update Python bindings in
   `finstack-py/src/valuations/instruments/credit/endogenous_hazard.rs`.

### Adding a new toggle model

1. Add a variant to `ToggleExerciseModel` in `toggle_exercise.rs`.
2. Implement the decision logic in `should_pik()`.
3. Ensure the model receives `CreditState` and `&mut dyn RandomNumberGenerator`.
4. Add tests for determinism (same seed = same result), boundary behavior, and
   economic intuition (stressed firms should prefer PIK).
5. Update Python bindings in
   `finstack-py/src/valuations/instruments/credit/toggle_exercise.rs`.

### Adding a new asset dynamics variant

1. Add a variant to `AssetDynamics` in `merton.rs`.
2. Update `simulate_paths()` to handle the new dynamics (drift compensation, etc.).
3. If the new dynamics affect `default_probability()`, add an analytical branch
   or note that MC is required.
4. Add unit tests: mean convergence, path dimension checks, comparison with
   existing dynamics.

### General checklist

- All new types must derive `Serialize, Deserialize` for configuration persistence.
- Input validation must return `finstack_core::Result<T>` with appropriate
  `InputError` variants.
- Public constructors should use the `Result<Self>` pattern for fallible creation.
- Recovery rates are clamped to `[0, base_recovery]`; hazard rates are floored at 0.
- Python bindings live under `finstack-py/src/valuations/instruments/credit/`.
- Python stub files live under `finstack-py/finstack/valuations/instruments/credit/`.

## Testing

Unit tests are co-located in each module. Run with:

```bash
cargo test -p finstack-valuations -- models::credit
```

Python binding tests:

```bash
uv run pytest finstack-py/tests/test_merton_bindings.py
uv run pytest finstack-py/tests/test_credit_specs_bindings.py
uv run pytest finstack-py/tests/test_merton_mc_bindings.py
```

### Test coverage highlights

- **Merton**: textbook DD/PD values, monotonicity in vol and leverage, first-passage
  vs terminal ordering, implied equity round-trip, KMV round-trip, CDS spread round-trip,
  `from_target_pd` round-trip for BB/B/CCC ratings, CreditGrades formula verification,
  hazard curve survival matching, MC mean convergence, jump-diffusion vs GBM divergence.
- **Toggle**: threshold above/below, stochastic probability monotonicity, optimal
  toggle stressed-vs-healthy behavior, deterministic reproducibility, zero-notional
  guard, zero-vol nested MC determinism.
- **Dynamic recovery**: per-model formula verification, floor enforcement, base-recovery
  capping, input validation.
- **Endogenous hazard**: base-leverage identity, leverage monotonicity, PIK accrual
  effect, tabular interpolation and extrapolation, input validation.
