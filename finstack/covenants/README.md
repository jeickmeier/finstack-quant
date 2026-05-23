# Covenants

Evaluate financial and non-financial covenants, track breaches and cure periods, apply consequences, and project compliance with headroom analytics.

## Layout

```
covenants/
├── engine.rs    # CovenantEngine, specs, consequences, breach tracking
├── forward.rs   # Forward projection (deterministic or MC)
├── schedule.rs  # Piecewise threshold schedules
└── report.rs    # CovenantReport
```

## Evaluation

```rust
use finstack_covenants::{
    Covenant, CovenantEngine, CovenantMetricId, CovenantSpec, HashMapMetricSource, CovenantType,
};
use finstack_core::dates::Tenor;

let covenant = Covenant::new(
    CovenantType::MaxTotalLeverage { threshold: 5.0 },
    Tenor::quarterly(),
)
.with_cure_period(Some(30));

let mut engine = CovenantEngine::new();
engine.add_spec(CovenantSpec::with_metric(
    covenant,
    CovenantMetricId::from("total_leverage"),
));

let mut metrics = HashMapMetricSource::from_pairs([("total_leverage", 4.2)]);
let reports = engine.evaluate(&mut metrics, test_date)?;
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

- `finstack-statements` — statement node IDs commonly provide covenant metric inputs
- `finstack-valuations` — `ValuationResult` can attach `CovenantReport` outputs
