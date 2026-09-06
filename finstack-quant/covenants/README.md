# finstack-quant-covenants

Loan covenant definition, point-in-time evaluation, breach and cure tracking,
consequence application, and forward compliance projection with headroom
analytics.

The crate is metric-agnostic: it evaluates numbers a caller supplies through a
`CovenantMetricSource` and never computes EBITDA, leverage, or DSCR itself.

## Where it sits

[`finstack-quant-core`](../core/README.md) (dates, tenors, `Result`,
`ResultsMeta`) is the only workspace crate it depends on; the rest are
serde/schemars, indexmap, time, and tracing. It defines no error type of its own
— every fallible entry point returns `finstack_quant_core::Result`.

Consumed by [`finstack-quant-valuations`](../valuations/README.md) (a
`ValuationResult` can carry `Option<IndexMap<String, CovenantReport>>`) and by
[`finstack-quant-statements-analytics`](../statements-analytics/README.md),
which adapts a `StatementResult` into the `ModelTimeSeries` trait so statement
models can drive covenant forecasts without this crate depending on statements.
Re-exported from the umbrella crate as `finstack_quant::covenants`.

## Layout

```text
covenants/
├── engine/      CovenantEngine, Covenant, CovenantSpec, CovenantType,
│                consequences, breaches, waivers, windows, springing conditions
├── forward.rs   Forward projection (deterministic, analytic, or Monte Carlo)
├── schedule.rs  ThresholdSchedule — piecewise step-down thresholds
├── metric.rs    CovenantMetricId, CovenantMetricSource, HashMapMetricSource
├── templates.rs Preset covenant packages
├── json.rs      Serde-first binding surface
└── report.rs    CovenantReport
```

Only `json`, `metric`, and `templates` are public module paths; the rest are
`pub(crate)` and reach callers through crate-root re-exports. Import from
`finstack_quant_covenants::` directly.

## Evaluation

```rust
use finstack_quant_core::dates::{create_date, Month, Tenor};
use finstack_quant_covenants::{
    Covenant, CovenantEngine, CovenantSpec, CovenantType, HashMapMetricSource,
};

fn main() -> finstack_quant_core::Result<()> {
    let covenant = Covenant::new(
        CovenantType::MaxDebtToEbitda { threshold: 4.5 },
        Tenor::quarterly(),
        "max_total_leverage", // the instance label — also the report key
    );
    let mut engine = CovenantEngine::new();
    engine.add_spec(CovenantSpec::with_metric(covenant, "debt_to_ebitda"));

    let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 3.2)]);
    let test_date = create_date(2025, Month::March, 31)?;
    let reports = engine.evaluate(&metrics, test_date)?;

    let report = &reports["max_total_leverage"];
    assert!(report.passed);
    assert_eq!(report.threshold, Some(4.5));
    // headroom = (4.5 - 3.2) / 4.5 ≈ 0.2889
    Ok(())
}
```

`Covenant::new` takes the covenant type, a nominal test frequency, and a
**label**. The label is the covenant's identity: `Covenant::instance_key()`
returns it, and `evaluate` returns `IndexMap<String, CovenantReport>` keyed by
it, as does breach history, forecast output, and consequence lookup. Two
covenants of the same type must carry different labels or they overwrite each
other. Human-readable text lives in `CovenantReport::covenant_type` /
`details`, never in the key.

`CovenantType::covenant_id()` supplies the conventional label for a type
(`"max_debt_ebitda"`, `"min_interest_coverage"`, `"min_dscr"`, …); the bundled
templates use it and only override where two covenants share a type.

Defaults from `Covenant::new`: 30-day cure period, no consequences, active,
`CovenantScope::Maintenance`, no springing condition. Override with
`with_cure_period`, `with_consequence`, `with_scope`, and
`with_springing_condition`.

### Covenant types

Numeric maintenance and incurrence tests: `MaxDebtToEbitda`,
`MaxNetDebtToEbitda`, `MaxTotalLeverage`, `MaxSeniorLeverage`,
`MinInterestCoverage`, `MinFixedChargeCoverage`, `MinDscr`, `MinAssetCoverage`,
`MaxCapex`, `MinLiquidity`, `Basket { name, limit }`, and
`Custom { metric, test }` with a `ThresholdTest::Minimum`/`Maximum` bound.
Non-financial `Affirmative { requirement }` and `Negative { restriction }`
covenants carry text only and report as passing; they have no numeric test.

Attach a metric with `CovenantSpec::with_metric(covenant, "metric_id")`.

The bundled `project_finance` package shows why labels are mandatory: it carries
two `MinDscr` covenants (`min_dscr_default` and `min_dscr_lockup`) whose
consequences differ, and only the labels keep their reports, breaches, and
consequences apart. `evaluate` errors on duplicate applicable instance keys
rather than silently overwriting.

### Scope, windows, waivers, springing conditions

- `CovenantScope` distinguishes scheduled `Maintenance` tests from
  action-driven `Incurrence` tests on `Covenant`.
- `CovenantWindow` restricts which specs apply between `start` and `end`. The
  first window containing the test date wins outright and replaces the base
  specs; if windows exist but none contains the test date, evaluation falls back
  to the base `specs`. `CovenantEngine::validate` rejects inverted, duplicate,
  and overlapping windows.
- `CovenantWaiver` grants a full waiver or an amended threshold over a date
  range; `expiry_date: None` is a permanent amendment. Validation rejects an
  expiry before the effective date, a non-finite amended threshold, and overlapping
  waivers for one label. End an earlier amendment before adding its replacement.
- `SpringingCondition` keeps a covenant inactive until its trigger metric
  crosses a bound; an inactive covenant reports as passing with no threshold or
  headroom.

## Threshold schedules

`ThresholdSchedule::new(entries)` builds a piecewise-constant, ascending-sorted
map from effective date to threshold — the standard step-down structure. Attach
one with `CovenantSpec::with_threshold_schedule`.

In point-in-time evaluation the effective threshold resolves in this order: an
active waiver's `amended_threshold`, then the schedule entry with the largest
date <= the test date, then the covenant's static threshold. A test date before
the schedule's first entry therefore falls back to the static threshold.

Batch forward projection uses the same effective windows, waivers, and
threshold precedence as spot evaluation. Single-spec forecasting has no engine
waiver context; it honors the supplied spec and its active/springing flags.

Construction — and deserialization, which routes through `new` — rejects
non-finite values and duplicate dates.

## Breach tracking and consequences

`evaluate_and_track(context, date, scope)` evaluates the explicitly selected
maintenance or completed-action incurrence scope and maintains `breach_history`. One continuous
uncured breach of a covenant instance is one breach episode: the cure deadline
is anchored to the **original** breach date, and metric recovery on or before
that deadline marks the episode cured rather than opening a new one. Inactive
and waived tests are not recovery evidence. Cure terms and consequences are
captured from the effective specification, including window-only covenants.

`apply_consequences(&mut instrument, &breaches, as_of)` resolves snapshots to
current history and acts only on uncured breaches after their cure deadline.
Each successful action is recorded individually, so retries resume at the first
unfinished action. The target implements `InstrumentMutator + Clone`: each
action is committed from a clone only after success. Unsupported collateral
requirements remain outstanding instead of being reported as applied.
`CovenantConsequence` variants:

| Variant | Payload |
|---------|---------|
| `Default` | — (event of default) |
| `RateIncrease` | `bp_increase: f64` |
| `CashSweep` | `sweep_percentage: f64` |
| `BlockDistributions` | — |
| `RequireCollateral` | `description: String` |
| `AccelerateMaturity` | `new_maturity: Date` |

## Forward projection

`forecast_covenant_generic(covenant, model, periods, config)` projects one
covenant across reporting periods; `forecast_breaches_generic(engine, model,
periods, config)` sweeps a whole engine and returns `Vec<FutureBreach>`. The
`model` argument is any `ModelTimeSeries` — a two-method trait
(`get_scalar`, `period_end_date`) — so the crate has no dependency on the
statements crate.

`CovenantForecastConfig` selects three modes:

| Mode | Config | Behavior |
|------|--------|----------|
| Deterministic | `stochastic: false` | Breach probability is `0.0` (pass) or `1.0` (breach); `breach_probability_stderr` is zero |
| Analytic | `stochastic: true`, `num_paths: 0` | Closed-form lognormal probability per test date via `norm_cdf`; zero estimator error. Independent marginal probabilities, not a first-passage distribution |
| Monte Carlo | `stochastic: true`, `num_paths > 0` | Path-consistent lognormal simulation. Path `p` draws from an independent `Pcg64Rng` stream keyed by `(random_seed, p)`, so results are deterministic and independent of evaluation order. `breach_probability_stderr` carries the MC standard error |

`antithetic` pairs `(Z, -Z)` draws in Monte Carlo mode and rounds `num_paths` up
to a whole number of pairs; it is rejected during validation when
`num_paths == 0`, where it would be inert. `volatility` is annualized and
required in stochastic mode. `reference_date` anchors the `sqrt(T)` shock
scaling; when `None`, the end date of the period immediately preceding the first
forecast period is used so the first point still has a non-zero horizon. Finite non-positive metrics use deterministic decisions because multiplicative
lognormal shocks are not meaningful there. Missing or non-finite required inputs
fail the whole forecast. MC requires at least two independent samples (two
antithetic pairs); dates must be strictly increasing and cannot precede the
reference date. `breach_probability_threshold` (default `0.05`) adds stochastic-risk dates to
batch output; deterministic breaches are always included. Batch scope defaults
to maintenance; `config.with_scope(CovenantScope::Incurrence)` selects hypothetical
action capacity. It does not record an executed incurrence breach.

Forecast ids are `Covenant::instance_key()`; the display string travels
separately in `covenant_description`. Nullable forecast fields
(`projected_values`, `headroom`) serialize as JSON `null` when a value is
inactive or not meaningful — a springing covenant outside its activation window,
or a leverage ratio on negative EBITDA.

## Templates

`templates` validates its thresholds (finite, non-negative) and returns
`Result<Vec<CovenantSpec>>` ready to feed a `CovenantEngine`:

| Function | Signature | Package |
|----------|-----------|---------|
| `lbo_standard` | `(initial_leverage, interest_coverage, fixed_charge_coverage, max_capex)` | Maintenance leverage + coverage + capex |
| `cov_lite` | `(max_leverage, max_senior_leverage)` | Incurrence-only total and senior leverage |
| `real_estate` | `(min_dscr, min_debt_yield, max_ltv)` | DSCR with 30-day cure and full cash sweep, plus labeled `min_debt_yield` / `max_ltv` custom tests |
| `project_finance` | `(min_dscr, distribution_lockup_dscr, min_liquidity, max_net_leverage)` | Two labeled DSCR tests (default vs. distribution lock-up), liquidity reserve, net leverage |

## JSON surface

`json` is the serde-first boundary used by the language bindings:
`evaluate_engine_map`, the validators (`validate_covenant_spec_json`,
`validate_covenant_report_json`, `validate_covenant_engine_json`), and JSON
template builders (`lbo_standard_json`, `cov_lite_json`, `real_estate_json`,
`project_finance_json`).

Inbound JSON denies unknown fields and runs domain validation on top of the
schema: non-negative cure periods, finite thresholds, valid waiver dates,
non-overlapping windows, and valid threshold schedules.

## Conventions

- **Test dates are the caller's.** `Covenant::test_frequency` is metadata; the
  engine never schedules tests for you.
- **Metrics arrive calculation-ready.** The engine does no LTM or other window
  aggregation. Ratio metrics are in turns (`4.5` means 4.5x); rate-style inputs
  such as debt yield and LTV are decimals (`0.10` means 10%).
- **Amount metrics are bare `f64`.** `MaxCapex`, `MinLiquidity`, and `Basket`
  thresholds carry no currency; the caller is responsible for keeping metric and
  threshold in the same reporting currency. Currency-typed thresholds are not
  part of this crate's surface.
- **Net-debt denominators are explicit.** Net debt/EBITDA specs require
  `denominator_metric_id`, defaulting to `ebitda` in constructors and templates.
  `with_denominator_metric` selects adjusted earnings. Positive earnings permit
  a negative net-cash ratio; non-positive earnings produce an indeterminate
  ratio breach with no headroom. Cash eligibility and netting remain upstream.
- **Equity cures are not modeled.** Use a `CovenantWaiver` for a full waiver or
  an amended threshold.
- **Headroom is relative, not absolute.** It is signed distance to the threshold
  divided by `|threshold|` — `(threshold - value) / |threshold|` for an at-most
  bound, `(value - threshold) / |threshold|` for at-least. Positive is a passing
  cushion, negative a deficit. Undefined ratio headroom is `None`; non-finite
  caller observations are rejected. A zero
  threshold uses a denominator of `1.0`.
- **Result stamping.** Every `CovenantReport` carries
  `finstack_quant_core::config::ResultsMeta` (numeric mode, rounding context, FX
  policy). See [INVARIANTS.md](../../INVARIANTS.md).

## Bindings

- **Python** — `finstack_quant.covenants`: typed `Covenant`, `CovenantType`,
  `CovenantSpec`, `ThresholdSchedule`, `CovenantWaiver`, `SpringingCondition`,
  `CovenantConsequence`, `CovenantEngine` (`evaluate` / `evaluate_and_track` /
  `evaluate_series`), `CovenantBreach`, `CovenantReport`; typed templates
  `lbo_standard` / `cov_lite` / `real_estate` / `project_finance`;
  DataFrame-backed `forecast_covenant` / `forecast_breaches` with
  `CovenantForecastConfig`, `CovenantForecast`, `FutureBreach`; plus the JSON
  surface (`evaluate_engine`, the three `validate_covenant_*_json` functions,
  and the `*_json` template twins).
- **WASM** — `covenants` namespace in
  `finstack-quant-wasm/exports/covenants.js`: `evaluateEngine`, the
  `validate*Json` validators, and `lboStandardJson` / `covLiteJson` /
  `realEstateJson` / `projectFinanceJson`.

## Verification

```bash
cargo nextest run -p finstack-quant-covenants --lib --test '*'
cargo test -p finstack-quant-covenants --doc
cargo clippy -p finstack-quant-covenants --lib --bins --tests --examples -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p finstack-quant-covenants --no-deps
```

Integration tests: `tests/engine_conventions.rs` (evaluation semantics),
`tests/integration.rs` (end-to-end packages), `tests/serialization.rs` (wire
format and strictness).

## Related

- [`../statements/README.md`](../statements/README.md) — statement node ids
  commonly supply covenant metric inputs
- [`../statements-analytics/README.md`](../statements-analytics/README.md) —
  `StatementsAdapter` bridges statement results into `ModelTimeSeries`
- [`../valuations/README.md`](../valuations/README.md) — `ValuationResult` can
  carry `CovenantReport` outputs
