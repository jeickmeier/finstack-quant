# Covenants

Evaluate financial and non-financial covenants, track breaches and cure periods, apply consequences, and project compliance with headroom analytics.

## Layout

```
covenants/
├── engine.rs    # CovenantEngine, specs, consequences, breach tracking
├── forward.rs   # Forward projection (deterministic or MC)
├── schedule.rs  # Piecewise threshold schedules
└── mod_types.rs # CovenantReport
```

## Evaluation

```rust
use finstack_valuations::covenants::{Covenant, CovenantEngine, CovenantSpec, CovenantType};
use finstack_core::dates::Tenor;
use finstack_valuations::metrics::{MetricContext, MetricId};

let covenant = Covenant::new(
    CovenantType::MaxTotalLeverage { threshold: 5.0 },
    Tenor::quarterly(),
)
.with_cure_period(Some(30));

let mut engine = CovenantEngine::new();
engine.add_spec(CovenantSpec::with_metric(
    covenant,
    MetricId::custom("total_leverage"),
));

let reports = engine.evaluate(&mut context, test_date)?;
```

Built-in financial types include leverage, coverage, and asset-coverage tests. `CovenantType::Custom` and non-financial affirmative/negative covenants use registered metrics or `CovenantSpec::with_evaluator`.

## Consequences

After cure expiry, `apply_consequences` can apply `RateIncrease`, `CashSweep`, `BlockDistributions`, `AccelerateMaturity`, `Default`, and related variants on instruments implementing `InstrumentMutator`.

## Forward projection

`forecast_covenant_generic` projects metric values through a `ModelTimeSeries` adapter (no direct dependency on the statements crate). `CovenantForecastConfig` selects deterministic paths or lognormal MC with optional antithetic variates.

## Threshold schedules

`ThresholdSchedule` plus `threshold_for_date` support step-down limits; custom evaluators can combine schedules with metric lookups.

## Windows

`CovenantWindow` restricts which specs apply between `start` and `end`; active windows override base specs.

## Related

- [`../metrics/README.md`](../metrics/README.md) — `MetricContext` inputs
- [`../results/README.md`](../results/README.md) — attach `CovenantReport` to valuations
