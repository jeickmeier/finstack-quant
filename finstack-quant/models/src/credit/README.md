# models::credit

Structural default models and the PIK-toggle machinery built on top of them:
Merton / Black-Cox / CreditGrades default probabilities, notional-dependent
recovery, leverage-dependent hazard rates, cash-vs-PIK exercise rules, and the
market-anchored credit-volatility conversions the callable lattice and the
revolving-credit CIR process consume.

These models drive the Merton Monte Carlo bond pricer and are usable standalone
for credit analytics (distance-to-default, implied spread, hazard-curve
bootstrapping from a structural fit).

## Position in the stack

Depends on `finstack_quant_core` for `HazardCurve`, `math::random`
(`RandomNumberGenerator`, `Pcg64Rng`, `poisson_inverse_cdf`),
`math::solver::BrentSolver`, `math::special_functions`, and `InputError`.
Nothing here reads a `MarketContext`.

Consumed by the Merton MC engine at
`instruments/fixed_income/bond/pricing/engine/merton_mc/`; by
`market::credit_option_vol` and the revolving-credit path generator
(`instruments/fixed_income/revolving_credit/pricing/path_generator.rs`), which
both route through `market_anchored`; and by both host bindings — see
[Binding exposure](#binding-exposure).

## Layout

| File | Contents |
|------|----------|
| [`mod.rs`](mod.rs) | Re-exports |
| [`merton/mod.rs`](merton/mod.rs) | Re-exports `MertonModel`, `AssetDynamics`, `BarrierType`, `SimulatedPaths` |
| [`merton/dynamics.rs`](merton/dynamics.rs) | `AssetDynamics`, `BarrierType` |
| [`merton/model.rs`](merton/model.rs) | `MertonModel`, `RawMertonModel`, constructors, accessors |
| [`merton/default_probability.rs`](merton/default_probability.rs) | Distance-to-default, PD, KMV default point |
| [`merton/spreads.rs`](merton/spreads.rs) | `implied_spread`, `debt_spread`, `cds_par_spread` |
| [`merton/calibration.rs`](merton/calibration.rs) | `from_equity`, `from_cds_spread`, `from_target_pd`, `credit_grades` |
| [`merton/hazard_curve.rs`](merton/hazard_curve.rs) | `to_hazard_curve` |
| [`merton/simulation.rs`](merton/simulation.rs) | `SimulatedPaths`, `simulate_paths` |
| [`dynamic_recovery.rs`](dynamic_recovery.rs) | `DynamicRecoverySpec`, `RecoveryModel` |
| [`endogenous_hazard.rs`](endogenous_hazard.rs) | `EndogenousHazardSpec`, `LeverageHazardMap` |
| [`toggle_exercise.rs`](toggle_exercise.rs) | `ToggleExerciseModel`, `CreditState`, `ThresholdToggle`, `StochasticToggle`, `OptimalToggle` |
| [`market_anchored.rs`](market_anchored.rs) | `CreditVolatilityConversion` and the fractional-to-absolute credit-vol mappings |

Re-exported at the `credit` root: `AssetDynamics`, `BarrierType`,
`MertonModel`, `SimulatedPaths`, `DynamicRecoverySpec`, `EndogenousHazardSpec`,
`CreditVolatilityConversion`, `CreditState`, `CreditStateVariable`,
`OptimalToggle`, `ThresholdDirection`, `ToggleExerciseModel`. `ThresholdToggle`
and `StochasticToggle` are reachable through `credit::toggle_exercise::`.

## Merton structural model (`merton/`)

Equity is a call option on firm assets; default is triggered when the asset
value crosses the debt barrier.

| Method | Formula / behaviour |
|--------|---------------------|
| `distance_to_default(horizon)` | `DD = [ln(V/B) + (r − q − σ²/2)T] / (σ√T)`; returns `+∞` for `horizon <= 0` |
| `distance_to_default_with_drift(asset_drift, horizon)` | Same with the physical asset return replacing `r` — the Moody's KMV DD |
| `default_probability(horizon)` | `Terminal`: `N(−DD)` under GBM, the Merton-1976 Poisson mixture under `JumpDiffusion`. `FirstPassage`: Black-Cox closed form. `CreditGrades`: the approximate survival function |
| `default_probability_with_drift(asset_drift, horizon)` | Same dispatch under the physical measure — the theoretical EDF |
| `implied_spread(horizon, recovery)` | `s = −ln(1 − PD·(1−R)) / T`, a zero-coupon bond spread with exogenous recovery paid at maturity |
| `debt_spread(horizon)` | Merton (1974) endogenous spread `−ln(D / B·e^{−rT}) / T` with `D = V·e^{−qT} − E`; `Terminal` only |
| `cds_par_spread(maturity, recovery)` | ISDA-style par spread from the model's survival curve, with premium leg, accrual on default, and discounting |
| `try_implied_equity(horizon)` | `-> Result<(equity_value, equity_vol)>` via Black-Scholes with a continuous payout rate; diffusion-only |
| `to_hazard_curve(id, base_date, &tenors, recovery, day_count)` | Piecewise-constant `HazardCurve` from the structural survival curve; tenors need not be sorted |
| `simulate_paths(num_paths, num_steps, horizon, &mut rng, antithetic)` | `-> Result<SimulatedPaths>` |

Read-only accessors: `asset_value()`, `asset_vol()`, `debt_barrier()`,
`risk_free_rate()`, `payout_rate()`, `barrier_type()`, `dynamics()`.

### Measure

`distance_to_default`, `default_probability`, the spread methods, and
`to_hazard_curve` are all **risk-neutral**. They are the right inputs for
pricing and materially overstate real-world default rates. The `_with_drift`
variants substitute the firm's expected physical asset return and give the
KMV/EDF quantities; pair them with the associated function
`MertonModel::kmv_default_point(short_term_debt, long_term_debt)`, which
returns the KMV default point `STD + 0.5·LTD`. `CreditGrades` is driftless by
construction, so both `_with_drift` methods reject it rather than silently
ignoring the drift.

### Spread conventions

The three spreads answer different questions and are not interchangeable:
`implied_spread` assumes an exogenous recovery paid at maturity,
`debt_spread` lets the firm's own terminal asset value be the recovery, and
`cds_par_spread` prices both CDS legs on a quarterly ACT/360 premium schedule
with a half-period accrual-on-default term. `cds_par_spread` exceeds
`implied_spread` by roughly 7% at a 30% cumulative default probability, which
is why `from_cds_spread` calibrates against the former.

### Asset dynamics

| Variant | Fields |
|---------|--------|
| `GeometricBrownian` | — |
| `JumpDiffusion` | `jump_intensity`, `jump_mean`, `jump_vol` (Merton 1976, Poisson-compensated) |
| `CreditGrades` | `barrier_uncertainty`, `mean_recovery` |

`CreditGrades::barrier_uncertainty` is the lognormal dispersion λ of the global
recovery rate, not a generic uncertainty scalar: it enters the survival formula
as `a_t² = σ²t + λ²` and the barrier shift as `exp(λ²)`.

Every parameter is validated at construction: jump intensity and volatility
must be finite and non-negative, `barrier_uncertainty` finite and non-negative,
and `mean_recovery` within `[0, 1]`.

### Barrier

`BarrierType::Terminal` (classic Merton, assessed at maturity only) or
`BarrierType::FirstPassage { barrier_growth_rate }` (Black-Cox, continuous
monitoring with an exponentially growing barrier). This is **not** the
barrier-option `finstack_quant_core::types::BarrierType`; the schema name is
`MertonBarrierType` to keep them apart on the wire.

The barrier and the dynamics must agree, and mismatches are rejected at
construction:

- `JumpDiffusion` requires `Terminal` — first passage of a jump-diffusion has
  no elementary closed form.
- `CreditGrades` requires `FirstPassage { barrier_growth_rate: 0.0 }` — its
  survival function *is* a first-passage law with a stochastic flat barrier.

### Constructors and calibration

| Constructor | Arguments |
|-------------|-----------|
| `new` | `(asset_value, asset_vol, debt_barrier, risk_free_rate)` |
| `new_with_dynamics` | adds `(payout_rate, BarrierType, AssetDynamics)` |
| `from_equity` | `(equity_value, equity_vol, total_debt, risk_free_rate, payout_rate, maturity)` — KMV fixed-point |
| `from_cds_spread` | `(cds_spread_bp, recovery, total_debt, risk_free_rate, maturity, asset_value, payout_rate)` — scan-then-Brent solve on σ |
| `from_target_pd` | `(asset_value, asset_vol, risk_free_rate, payout_rate, target_pd, maturity)` — Brent solve on the barrier |
| `credit_grades` | `(equity_value, equity_vol, total_debt, risk_free_rate, barrier_uncertainty, mean_recovery)` |

All return `Result<Self>` and reject non-positive `asset_value`, `asset_vol`,
or `debt_barrier` with `InputError::NonPositiveValue`. Deserialization is
routed through `new_with_dynamics` via `RawMertonModel`, so a model loaded from
JSON satisfies exactly the same invariants.

`asset_value <= debt_barrier` is **intentionally accepted** — it represents a
firm at or through its default point. Pricing then degenerates consistently:
first-passage paths default immediately, terminal-barrier PD approaches 1, and
the CreditGrades survival formula returns PD = 1 in the zero-variance limit.
Callers wanting a strictly solvent firm must validate that themselves.

`from_equity` rejects an `equity_value` that is a negligible fraction of firm
value up front: the KMV inversion `σ_V = σ_E·E / (N(d₁)·e^{−qT}·V)` is
ill-conditioned there and would drive iterates to inf/NaN, silently defeating
the convergence test. The fixed point requires **both** `V` and `σ_V` to settle
before it returns.

`from_cds_spread` does not assume the par spread is monotonic in σ — for a firm
below its barrier, raising volatility first lowers the default probability. The
objective is scanned across `[0.01, 2.0]`; a unique sign change is refined with
Brent, while zero or several sign changes raise a descriptive error naming the
attainable spread range or the competing brackets.

### `SimulatedPaths`

Flat path storage with `values_per_path()`, `get(path_idx, time_idx)`,
`path(path_idx)`, `iter_paths()`, and `to_nested()`. Seeded via the
`RandomNumberGenerator` the caller passes to `simulate_paths`, so the same seed
reproduces the same paths. `num_steps == 0` or `horizon <= 0` is a validation
error, not a degenerate grid.

`simulate_paths` returns the raw asset grid with no barrier applied. Inferring
first-passage default by testing `V_t <= B` at grid points alone understates
default, because a path can dip below the barrier and recover between steps;
apply the Brownian-bridge crossing probability
`exp(−2·ln(V_i/B)·ln(V_{i+1}/B) / (σ²·dt))` per surviving step, as the
PIK-toggle Monte Carlo bond engine does, to match the continuous-monitoring
`default_probability`.

## Dynamic recovery (`dynamic_recovery.rs`)

Recovery declines as PIK accrual inflates the outstanding notional.

| `RecoveryModel` | `recovery_at_notional(N)` |
|-----------------|---------------------------|
| `Constant` | `R₀` |
| `InverseLinear` | `R₀ · (N₀ / N)` |
| `InversePower { exponent }` | `R₀ · (N₀ / N)^α` |
| `FlooredInverse { floor }` | `max(floor, R₀ · (N₀ / N))` |
| `LinearDecline { sensitivity, floor }` | `max(floor, R₀ · (1 − β·(N/N₀ − 1)))` |

Constructors: `constant`, `inverse_linear`, `inverse_power`, `floored_inverse`,
`linear_decline` — all `-> Result<Self>`. `N <= 0` returns `0.0`; every other
result is clamped to `[0, base_recovery]`. Accessors: `base_recovery()`,
`base_notional()`, `model()`.

The clamp introduces a kink in recovery as a function of accreted notional;
paths far inside the clamped region all contribute the same floored or capped
recovery. No smoothed (logistic) rule is applied.

## Endogenous hazard (`endogenous_hazard.rs`)

Closes the feedback loop: PIK accrual raises leverage, which raises the hazard
rate.

| `LeverageHazardMap` | `hazard_at_leverage(L)` |
|---------------------|-------------------------|
| `PowerLaw { exponent }` | `λ₀ · (L / L₀)^β` |
| `Exponential { sensitivity }` | `λ₀ · exp(β·(L − L₀))` |
| `Tabular { leverage_points, hazard_points }` | Linear interpolation with flat extrapolation |

Constructors `power_law`, `exponential`, `tabular` return `Result<Self>`.
`hazard_after_pik_accrual(accreted_notional, asset_value)` computes leverage as
the ratio and delegates. Accessors: `base_hazard_rate()`, `base_leverage()`,
`leverage_hazard_map()`.

Results are clamped to `[0, MAX_HAZARD_RATE]` with `MAX_HAZARD_RATE = 1e6`, and
a `NaN` raw rate (e.g. `0 · inf`) collapses to `0.0`. The order matters —
`clamp` alone would propagate `NaN`. A degenerate tabular map (empty or
mismatched vector lengths, reachable only through `Deserialize` since the
constructor validates) yields `0.0`.

## Toggle exercise (`toggle_exercise.rs`)

Decides cash versus PIK at each coupon date.

| Variant | Rule |
|---------|------|
| `Threshold(ThresholdToggle)` | PIK when the credit metric crosses `threshold` in `direction` |
| `Stochastic(StochasticToggle)` | `P(PIK) = 1 / (1 + exp(−(intercept + sensitivity·x)))` |
| `OptimalExercise(OptimalToggle)` | Nested Monte Carlo comparing equity value under cash and PIK |

Constructors `ToggleExerciseModel::threshold(variable, threshold, direction)`
and `::stochastic(variable, intercept, sensitivity)` are infallible;
`OptimalExercise` is built by naming the `OptimalToggle` fields
(`nested_paths`, `equity_discount_rate`, `asset_vol`, `risk_free_rate`,
`horizon`) directly.

Decision entry points: `should_pik(&CreditState, &mut dyn
RandomNumberGenerator) -> bool`, the deterministic
`should_pik_with_uniform(&CreditState, u)`, and `pik_fraction(...)`.

`CreditStateVariable` is `HazardRate`, `DistanceToDefault`, or `Leverage`;
`ThresholdDirection` is `Above` or `Below`. Both implement `FromStr` over the
snake_case serde names, which is how the Python binding accepts strings.

`CreditState` carries `hazard_rate`, `distance_to_default: Option<f64>`,
`leverage`, `accreted_notional`, `coupon_due`, and `asset_value:
Option<f64>`.

**Missing distance-to-default reads as `0.0`** — maximally stressed. Under a
`Below` rule that deterministically elects PIK. The pessimism is deliberate (an
issuer with no computable DD should not be treated as healthy), but a
`DistanceToDefault` rule should populate the field explicitly rather than rely
on it.

The optimal model simulates `nested_paths` GBM paths over `horizon` at
`NESTED_STEPS_PER_YEAR = 12` steps per year with first-passage barrier checks
under both scenarios, and elects PIK when the estimated equity value under PIK
exceeds that under cash. A liquidity early-exit guard forces PIK when paying
cash would itself breach the default barrier. The nested simulation drifts
under the risk-neutral measure but discounts at `equity_discount_rate`, so the
intermediate equity figures are decision inputs, not measure-consistent prices;
the rate cancels in the comparison and does not bias the elected branch.

## Market-anchored credit volatility (`market_anchored.rs`)

Index CDS-option markets quote a **fractional** (relative, lognormal)
forward-spread volatility — `0.35` meaning 35% of the spread level. The
callable lattice wants an additive hazard-rate volatility in decimal hazard
points per √year, and the revolving-credit CIR process wants a square-root
diffusion coefficient. Feeding `0.35` into either is roughly an order of
magnitude wrong. This module owns the conversion so both consumers derive their
parameters from one place.

The credit triangle `s ≈ (1 − R)·λ` (O'Kane 2008, §5.4), differentiated at
fixed recovery, gives:

```text
σ_s,abs = σ_fractional · s_ref
σ_λ     = σ_fractional · λ_ref = σ_s,abs / (1 − R)
```

Recovery cancels from the hazard volatility exactly as it cancels from the
level relation.

| Item | Notes |
|------|-------|
| `CreditVolatilityConversion::from_survival_window(σ_frac, sp_start, sp_end, horizon, recovery)` | Anchors on a survival ratio from the target curve |
| `CreditVolatilityConversion::from_reference_hazard(σ_frac, λ_ref, horizon, recovery)` | Anchors on a quoted flat hazard |
| `conditional_average_hazard`, `reference_spread`, `absolute_spread_volatility`, `additive_hazard_volatility`, `cir_diffusion_coefficient` | The individual mappings |
| `MIN_REFERENCE_LEVEL = 1e-8` | Below this the fractional vol of a vanishing spread carries no absolute information and conversion is an error |

The returned struct reports every quantity it used and produced
(`horizon_years`, `recovery`, `reference_hazard`, `reference_spread`,
`fractional_spread_volatility`, `absolute_spread_volatility`,
`hazard_volatility`, `cir_diffusion`), so a relative quote cannot be dropped
into an additive lattice unnoticed.

This is an explicit first-order **local** mapping evaluated at one reference
hazard over one horizon. It is not a calibration: feeding the resulting `σ_λ`
into the callable lattice will not exactly reprice the index option it came
from. Term structure of credit volatility, skew, and issuer/index beta are out
of scope — applying an index-derived fractional vol to a single name is a
caller decision, made explicit by passing the target curve's own reference
hazard.

## Integration with the Merton MC engine

`MertonMcConfig`
(`instruments/fixed_income/bond/pricing/engine/merton_mc`) assembles these
pieces:

```text
MertonMcConfig
├── merton: MertonModel                            ← dynamics, barrier, calibration
├── pik_schedule: PikSchedule                      ← Uniform(PikMode) | Stepped(Vec<(f64, PikMode)>)
├── endogenous_hazard: Option<EndogenousHazardSpec>
├── dynamic_recovery: Option<DynamicRecoverySpec>
├── toggle_model: Option<ToggleExerciseModel>      ← consulted only at PikMode::Toggle dates
├── steps_per_year
├── barrier_crossing: BarrierCrossing              ← Discrete | BrownianBridge
├── recovery_rate                                  ← used when dynamic_recovery is None
├── calibration: Option<MertonMcCalibrationSpec>
└── discount factors (optional term structure; otherwise a flat rate)
```

Per path, per time step: evolve the asset value; take the hazard from
`EndogenousHazardSpec` if present, otherwise from the Merton model; check
first-passage default against the barrier; at coupon dates evaluate the toggle
model when `PikMode::Toggle` is active; on default compute recovery through
`DynamicRecoverySpec` if present, otherwise `recovery_rate`.

`PikMode::Toggle` falls back to `Cash` when no toggle model is set.
`BarrierCrossing` defaults to `BrownianBridge` when the Merton model uses
`FirstPassage`, otherwise `Discrete`.

`Bond::price_merton_mc(&config, discount_rate, as_of)` overrides a default
`PikSchedule::Uniform(Cash)` from the bond's own `CouponType`; a non-default
schedule on the config takes precedence.

## Example

```rust
use finstack_quant_models::credit::{
    toggle_exercise::{CreditStateVariable, ThresholdDirection},
    AssetDynamics, BarrierType, CreditVolatilityConversion, DynamicRecoverySpec,
    EndogenousHazardSpec, MertonModel, ToggleExerciseModel,
};

// Direct construction.
let model = MertonModel::new(100.0, 0.20, 80.0, 0.05)?;
let dd = model.distance_to_default(1.0);
let pd = model.default_probability(1.0);
let spread = model.implied_spread(5.0, 0.40)?;
assert!(dd > 0.0 && (0.0..1.0).contains(&pd) && spread > 0.0);

// The quotable CDS level exceeds the zero-coupon approximation.
assert!(model.cds_par_spread(5.0, 0.40)? > spread);

// Real-world default rate: swap the risk-free rate for the physical asset
// return and put the barrier at the KMV default point.
let default_point = MertonModel::kmv_default_point(40.0, 80.0)?;
let kmv = MertonModel::new(100.0, 0.20, default_point, 0.05)?;
let edf = kmv.default_probability_with_drift(0.12, 1.0)?;
assert!(edf < kmv.default_probability(1.0));

// Calibrate the barrier to a 5-year cumulative PD implied by a 2% annual hazard.
let five_year_pd = 1.0 - (-0.02_f64 * 5.0).exp();
let calibrated = MertonModel::from_target_pd(200.0, 0.25, 0.045, 0.0, five_year_pd, 5.0)?;
assert!((calibrated.default_probability(5.0) - five_year_pd).abs() < 1e-6);

// Black-Cox first passage with a growing barrier, same parameters as `model`.
let black_cox = MertonModel::new_with_dynamics(
    100.0,
    0.20,
    80.0,
    0.05,
    0.0,
    BarrierType::FirstPassage { barrier_growth_rate: 0.02 },
    AssetDynamics::GeometricBrownian,
)?;
// Continuous monitoring can only default at least as often as terminal-only.
assert!(black_cox.default_probability(5.0) >= model.default_probability(5.0));

// Structural PD -> hazard curve for the reduced-form engines.
let base_date = time::Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
let hazard_curve = model.to_hazard_curve(
    "ISSUER_001",
    base_date,
    &[1.0, 3.0, 5.0, 10.0],
    0.40,
    finstack_quant_core::dates::DayCount::Act365F,
)?;

// PIK feedback components.
let recovery = DynamicRecoverySpec::floored_inverse(0.40, 100.0, 0.15)?;
assert!(recovery.recovery_at_notional(130.0) < 0.40);

let hazard = EndogenousHazardSpec::power_law(0.05, 0.60, 2.0)?;
assert!(hazard.hazard_at_leverage(0.75) > 0.05);

let toggle = ToggleExerciseModel::threshold(
    CreditStateVariable::HazardRate,
    0.15,
    ThresholdDirection::Above,
);

// 35% relative CDS-option vol on a 3% hazard is a 1.05% absolute hazard vol.
let survival_end = (-0.03_f64 * 5.0).exp();
let conv = CreditVolatilityConversion::from_survival_window(0.35, 1.0, survival_end, 5.0, 0.4)?;
assert!((conv.hazard_volatility - 0.0105).abs() < 1e-12);
# Ok::<(), finstack_quant_core::Error>(())
```

## Conventions

- Rates, hazards, recoveries, and volatilities are decimals; `from_cds_spread`
  is the one exception and takes basis points.
- Horizons and maturities are year fractions.
- `MertonModel`, `AssetDynamics`, `BarrierType`, `DynamicRecoverySpec`,
  `EndogenousHazardSpec`, `CreditState`, and `ToggleExerciseModel` all derive
  `Serialize`/`Deserialize`/`JsonSchema`, so a whole `MertonMcConfig`
  round-trips through the wire format. `CreditVolatilityConversion` is a
  plain-value diagnostic and is not serialized.
- Fallible constructors return `finstack_quant_core::Result<Self>` with
  `InputError` variants or `Error::Validation`.
- Recovery is clamped to `[0, base_recovery]`; hazard to
  `[0, MAX_HAZARD_RATE]`.
- Simulation is deterministic given the seed: the same `RandomNumberGenerator`
  seed reproduces the same paths and the same toggle decisions.
- These are `f64` analytics, not `Money` — see
  [INVARIANTS.md](../../../../../INVARIANTS.md) §1.

## Binding exposure

**Python** — `finstack_quant.models.credit` exposes `MertonModel`,
`AssetDynamics`, `BarrierType`, `SimulatedPaths`, `DynamicRecoverySpec`,
`EndogenousHazardSpec`, `CreditState`, and `ToggleExerciseModel`.
`MertonMcConfig` and `MertonMcResult` live one namespace over in
`finstack_quant.valuations.instruments`; `MertonMcConfig` is a fluent builder
(`MertonMcConfig(merton, 0.4).steps_per_year(12)`), not a keyword constructor.
Path count, antithetic sampling and the seed label come from the bond's
`instrument_pricing_overrides.model_config` (`mc_paths`, `mc_antithetic`,
`mc_seed_scenario`).

The wire and export surface is not uniform across the eight classes:

| Class | `to_json` / `from_json` / `__reduce__` | `to_dataframe()` |
|-------|----------------------------------------|------------------|
| `MertonModel` | yes | yes |
| `DynamicRecoverySpec` | yes | yes |
| `EndogenousHazardSpec` | yes | yes |
| `CreditState` | yes | yes |
| `AssetDynamics` | yes | no |
| `BarrierType` | yes | no |
| `ToggleExerciseModel` | yes | no |
| `SimulatedPaths` | no | no |

`SimulatedPaths` is a plain path container: `times()`, `asset_values()`,
`num_paths()`, `num_steps()`, `get()`, `path()`, `to_nested()`.

Not every Rust constructor is bound. `DynamicRecoverySpec` exposes only
`constant`, `EndogenousHazardSpec` only `power_law`, and `ToggleExerciseModel`
only `threshold` and `optimal`. The remaining variants are reachable from
Python through `from_json` on the canonical wire form.

**WASM** — bound as the `valuations.credit` namespace
([`finstack-quant-wasm/exports/valuations/credit.js`](../../../../../finstack-quant-wasm/exports/valuations/credit.js)
over [`src/api/valuations/credit.rs`](../../../../../finstack-quant-wasm/src/api/valuations/credit.rs)),
as JSON-string functions rather than classes: `mertonModelJson`,
`mertonModelWithDynamicsJson`, `creditGradesModelJson`,
`mertonFromEquityJson`, `mertonFromCdsSpreadJson`, `mertonFromTargetPdJson`,
`mertonDefaultProbability`, `mertonDefaultProbabilityWithDrift`,
`mertonDistanceToDefault`, `mertonDistanceToDefaultWithDrift`,
`mertonKmvDefaultPoint`, `mertonImpliedSpread`, `mertonDebtSpread`,
`mertonCdsParSpread`, `mertonTryImpliedEquity`, `mertonToHazardCurveJson`,
`mertonSimulatePathsJson`, `dynamicRecoveryConstantJson`,
`dynamicRecoveryAtNotional`, `endogenousHazardPowerLawJson`,
`endogenousHazardAtLeverage`, `endogenousHazardAfterPikAccrual`,
`creditStateJson`, `toggleExerciseThresholdJson`,
`toggleExerciseOptimalJson`. The same constructor gaps as Python apply.

`market_anchored` is Rust-only in both hosts.

## Verification

```bash
# Unit tests for this module (never `cargo test` — it would run doc tests).
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/models::credit/)'

# One area at a time.
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/credit::merton/)'
cargo nextest run -p finstack-quant-valuations --lib -E 'test(/credit::toggle_exercise/)'

mise run rust-test
mise run rust-lint

# Python binding behaviour.
mise run python-build
uv run pytest finstack-quant-py/tests/test_merton_model.py
```

Coverage highlights: textbook DD/PD values, monotonicity in vol and leverage,
first-passage versus terminal ordering, implied-equity/KMV/CDS-spread/target-PD
round-trips, the CreditGrades survival formula, hazard-curve survival matching,
MC mean convergence, jump-diffusion versus GBM divergence; threshold
above/below, stochastic probability monotonicity, optimal-toggle stressed
versus healthy behaviour, seeded reproducibility, zero-notional and zero-vol
guards; per-model recovery formulas with floor and cap enforcement; base
-leverage identity, leverage monotonicity, PIK accrual effect, tabular
interpolation and extrapolation.

## Extending

**New recovery model.** Add a `RecoveryModel` variant, implement it in
`DynamicRecoverySpec::recovery_at_notional`, add a fallible convenience
constructor, and test the formula, its edge cases, and the `[0, base_recovery]`
clamp.

**New hazard mapping.** Add a `LeverageHazardMap` variant, implement it in
`EndogenousHazardSpec::hazard_at_leverage`, add a constructor, and test that
`hazard_at_leverage(base_leverage) == base_hazard_rate`, that the map is
monotonic where it should be, and that the `NaN`/`inf` guards hold.

**New toggle model.** Add a `ToggleExerciseModel` variant and its config
struct, implement the branch in `should_pik` (and
`should_pik_with_uniform` / `pik_fraction`), and test determinism under a fixed
seed, boundary behaviour, and the economic intuition that stressed firms prefer
PIK.

**New asset dynamics.** Add an `AssetDynamics` variant, handle it in
`simulate_paths` (including drift compensation), and either add an analytical
branch to `default_probability` or document that Monte Carlo is required. Test
mean convergence, path dimensions, and divergence from the existing dynamics.

Across all four: derive `Serialize, Deserialize, schemars::JsonSchema`; return
`finstack_quant_core::Result<T>` from validating constructors; and mirror the
new variant into the Python binding at
`finstack-quant-py/src/bindings/valuations/credit.rs` with a matching stub in
`finstack-quant-py/finstack_quant/valuations/models/credit/__init__.pyi`.

## References

| Concept | Source |
|---------|--------|
| Structural default | Merton, R. C. (1974). "On the Pricing of Corporate Debt: The Risk Structure of Interest Rates." *Journal of Finance*, 29(2), 449-470. |
| First-passage barrier | Black, F. & Cox, J. C. (1976). "Valuing Corporate Securities: Some Effects of Bond Indenture Provisions." *Journal of Finance*, 31(2), 351-367. |
| Jump diffusion | Merton, R. C. (1976). "Option Pricing When Underlying Stock Returns Are Discontinuous." *Journal of Financial Economics*, 3(1-2), 125-144. |
| CreditGrades | Finger, C., Finkelstein, V., Pan, G., Lardy, J.-P., Ta, T. & Tierney, J. (2002). *CreditGrades Technical Document*. RiskMetrics Group. |
| KMV calibration | Hull, J. C. *Options, Futures, and Other Derivatives*, ch. 17. |
| Physical DD, EDF, default point | Crosbie, P. & Bohn, J. (2003). *Modeling Default Risk*. Moody's KMV. |
| Credit triangle, CDS conventions | O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*. Wiley, §5.4. |

Full bibliography with stable anchors: [docs/REFERENCES.md](../../../../../docs/REFERENCES.md).
