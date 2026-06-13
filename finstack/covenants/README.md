# Covenants

Evaluate financial and non-financial covenants, track breaches and cure periods, apply consequences, and project compliance with headroom analytics.

## Layout

```
covenants/
├── engine.rs    # CovenantEngine, specs, consequences, breach tracking
├── forward.rs   # Forward projection (deterministic or analytic stochastic)
├── schedule.rs  # Piecewise threshold schedules
├── metric.rs    # CovenantMetricId, metric-source traits
├── templates.rs # Preset covenant packages (lbo_standard, cov_lite, ...)
├── json.rs      # Serde-first JSON binding surface
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

`evaluate_and_track` treats one continuous uncured breach of a covenant instance as one breach episode. The cure deadline is anchored to the original breach date, and metric recovery before that deadline marks the episode cured.

## Forward projection

`forecast_covenant_generic` projects metric values through a `ModelTimeSeries` adapter (no direct dependency on the statements crate). `CovenantForecastConfig` selects deterministic output or analytic lognormal per-date breach probabilities. The stochastic output is an independent marginal probability for each test date, not a first-passage distribution; `num_paths`, `random_seed`, and `antithetic` are retained for source compatibility and future path-consistent simulation.

Forecast IDs use `Covenant::instance_key()` so outputs can join to engine reports and breach history. Human-readable text is carried separately in `covenant_description`. Nullable forecast fields serialize as JSON `null` when a value is inactive or not meaningful, such as springing covenants outside activation periods or negative-EBITDA leverage ratios.

## Threshold schedules

`ThresholdSchedule` plus `threshold_for_date` support step-down limits; custom evaluators can combine schedules with metric lookups. `ThresholdSchedule::try_new` validates finite values and rejects duplicate dates.

## Windows

`CovenantWindow` restricts which specs apply between `start` and `end`; active windows override base specs. If windows exist but the test date falls outside every window, evaluation falls back to the base `specs`. Overlapping windows are rejected by engine validation.

## Templates

`templates` provides preset covenant packages that return `Vec<CovenantSpec>`
ready for `CovenantEngine`: `lbo_standard`, `cov_lite`, `real_estate`, and
`project_finance`.

## JSON

`json` is a serde-first binding surface: `evaluate_engine_json`, the
`validate_*_json` validators, and JSON template builders (`lbo_standard_json`,
`cov_lite_json`, `project_finance_json`, `real_estate_json`).

Inbound JSON denies unknown fields and runs domain validation, including non-negative cure periods, finite thresholds, valid waiver dates, non-overlapping windows, and valid threshold schedules.

Amount-style covenants such as `MaxCapex`, `MinLiquidity`, and `Basket` use bare `f64` thresholds in the deal currency agreed by the caller; currency-typed thresholds are outside this crate's current JSON surface.

## Related

- `finstack-statements` — statement node IDs commonly provide covenant metric inputs
- `finstack-valuations` — `ValuationResult` can attach `CovenantReport` outputs
