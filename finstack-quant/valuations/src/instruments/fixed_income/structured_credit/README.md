# Structured Credit

ABS, RMBS, CMBS and CLO deals modeled as one instrument (`StructuredCredit`):
a collateral pool, a tranche capital structure, a payment waterfall with
coverage tests, and deal-type behavioral assumptions.

Deterministic pricing runs a period-by-period simulation of the pool and the
waterfall; stochastic pricing runs the same engine over simulated prepayment
and default paths.

## Public surface

Import path:
`finstack_quant_valuations::instruments::fixed_income::structured_credit`.
There is **no `prelude` module** — import the names you need directly.

| Item | Purpose |
|------|---------|
| `StructuredCredit` | The instrument. `new_abs`/`new_clo`/`new_cmbs`/`new_rmbs` apply deal-type defaults; `builder()` for full control; `example()` for a canonical deal. |
| `DealType`, `AssetType`, `TrancheSeniority` | `DealType::{Clo, Cbo, Abs, Rmbs, Cmbs, Auto, Card}` and pool/tranche taxonomy. Only `Abs`, `Clo`, `Cmbs` and `Rmbs` have `new_*` constructors and registry profiles. |
| `AssetPool`, `PoolAsset`, `RepLine`, `PoolStats`, `calculate_pool_stats` | Collateral pool and its aggregates. |
| `Tranche`, `TrancheBuilder`, `TrancheStructure`, `TrancheCoupon`, `TrancheBehaviorType` | Capital structure. |
| `Waterfall`, `WaterfallBuilder`, `WaterfallTier`, `Recipient`, `RecipientType`, `PaymentType`, `PaymentCalculation`, `AllocationMode` | Waterfall construction. |
| `WaterfallRules`, `AfcSpec`, `StepDownSpec`, `StepDownTrigger`, `ShiftingInterestSpec` | Declarative rules layered onto the base waterfall by `resolve_waterfall`. |
| `CoverageTrigger` (tranche-level), `waterfall::CoverageTrigger` (waterfall-level), `CoverageTestConfig`, `CoverageTestType`, `TriggerConsequence` | OC/IC triggers — see [Coverage triggers](#coverage-triggers), the two types are different. |
| `DealConfig`, `DealDates`, `DealFees`, `DefaultAssumptions` | Deal setup and behavioral defaults. |
| `PrepaymentModelSpec`, `DefaultModelSpec`, `RecoveryModelSpec`, `PrepaymentCurve`, `DefaultCurve` | Deterministic behavioral models. |
| `PricingMode` | Valuation-owned stochastic pricing mode — see [`pricing/stochastic/README.md`](pricing/stochastic/README.md). |
| `StochasticPrepaySpec`, `StochasticDefaultSpec`, `CorrelationStructure`, `PoolGranularity` | Models-owned stochastic inputs; import from `finstack_quant_models::credit::pool`. |
| `StochasticPricingResult`, `TranchePricingResult` | Stochastic output. |
| `ReinvestmentPeriod`, `ReinvestmentCriteria` | CLO reinvestment contract used by the simulation engine. |
| `EarlyAmortizationSpec`, `ControlledAccumulationSpec`, `ExcessSpreadSpec` | ABS/credit-card structural features. |
| `run_simulation`, `generate_cashflows`, `generate_tranche_cashflows` | Deterministic projection entry points. |
| `execute_waterfall`, `execute_waterfall_with_explanation`, `WaterfallContext`, `WaterfallDistribution`, `resolve_waterfall` | Waterfall execution. |
| `CoverageTest`, `TestContext`, `TestResult` | Coverage-test evaluation. |
| `calculate_tranche_metrics`, `TrancheMetrics`, `scenario_table`, `ScenarioTable`/`ScenarioGrid`/`ScenarioCell` | Tranche summary and scenario grids. |
| `calculate_tranche_wal`, `_duration`, `_convexity`, `_z_spread`, `_discount_margin`, `_oas` (+ `OasConfig`, `OasResult`), `_cs01`, `_breakeven_cdr` | Individual tranche analytics, the same functions the Python/WASM `structured_credit_tranche_*` entry points wrap. |
| `clamped_cpr_to_smm`, `clamped_smm_to_cpr`, `clamped_cdr_to_mdr`, `clamped_mdr_to_cdr`, `psa_to_cpr` | Rate conversions. |
| `is_valid_waterfall_spec`, `get_validation_errors`, `ValidationError` | Waterfall validation. |
| Deal-type constants | Standard speeds, fees and concentration limits, re-exported at the module root: `clo_standard_cdr`, `rmbs_standard_psa`, `sda_peak_cdr`, … (the `types` submodule itself is `pub(crate)`). |

## Module layout

```
structured_credit/
├── mod.rs         # re-exports and module-level rustdoc
├── pricer.rs      # StructuredCreditDiscountingPricer
├── assumptions.rs # embedded assumption registry loader
├── types/
│   ├── instrument.rs / structured_credit_impl.rs  # the StructuredCredit struct and its impls
│   ├── constructors.rs   # new_abs / new_clo / new_cmbs / new_rmbs / example
│   ├── pricing_methods.rs# advanced-only value_tranche / price_stochastic; hosts use ModelKey
│   ├── stochastic.rs     # enable_stochastic_defaults, with_stochastic_* setters
│   ├── constants.rs enums.rs pool.rs pool_state.rs tranches.rs waterfall.rs
│   ├── results.rs setup.rs
├── pricing/
│   ├── simulation_engine/ # the deterministic period loop, pool flows, conservation checks
│   ├── waterfall.rs       # execute_waterfall(_with_explanation), WaterfallContext
│   ├── resolve.rs         # resolve_waterfall: layer WaterfallRules onto the base waterfall
│   ├── coverage_tests.rs  # OC/IC test evaluation
│   └── stochastic/        # calibration presets, tree and Monte Carlo orchestration
├── metrics/
│   ├── pricing/       # clean/dirty price, accrued, WAL
│   ├── risk/          # duration, convexity, YTM, z-spread, OAS, CS01, breakeven CDR, *01 sensitivities
│   ├── pool/          # WAM, CPR, CDR, WARF, WAS
│   ├── deal_specific/ # ABS charge-off & credit enhancement, CMBS DSCR
│   ├── scenario.rs    # scenario_table
│   └── summary.rs     # calculate_tranche_metrics
└── utils/             # rate conversions, floating-rate helpers, recovery queue, validation
```

## Deal types

| Deal type | Collateral | Registry defaults | Deal-specific metrics |
|-----------|-----------|-------------------|-----------------------|
| `DealType::Abs` | Auto loans, credit cards | `abs_auto_standard` | `AbsChargeOff`, `AbsCreditEnhancement` |
| `DealType::Clo` | Leveraged loans | `clo_standard` | `CloWarf`, `CloWas` |
| `DealType::Cmbs` | Commercial mortgages | `cmbs_standard` | `CmbsDscr` |
| `DealType::Rmbs` | Residential mortgages | `rmbs_standard` | — (use `WAL`) |

`new_*` constructors pull their `DealConfig` and `DefaultAssumptions` from the
embedded registry in
[`data/assumptions/structured_credit_assumptions.v1.json`](../../../../data/assumptions/structured_credit_assumptions.v1.json)
(fee defaults, PSA/SDA parameters, concentration limits, standard speeds).

## Constructing a deal

```rust
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;

// Deal-type constructors take (id, pool, tranches, closing_date, maturity, discount_curve_id)
// and apply the registry defaults for that deal type.
let clo = StructuredCredit::new_clo(
    "MY_CLO",
    pool,        // AssetPool
    tranches,    // TrancheStructure
    closing_date,
    legal_maturity,
    "USD-OIS",
);

// Or start from the canonical example.
let deal = StructuredCredit::example();
```

For full control use `StructuredCredit::builder()` and set `deal_type`, `pool`,
`tranches`, `waterfall` and the credit model explicitly.

### Instrument collateral

A pool can hold real instruments instead of asset rows or rep lines:
`AssetPool::instruments = Some(InstrumentCollateral { bonds, term_loans,
revolvers, .. })` accepts any `Bond` (fixed, floating, step-up, amortizing,
callable/puttable, custom cashflows), `TermLoan` (including delayed draws and
PIK) and `RevolvingCredit` in the pool base currency. Each instrument's own
`raw_cashflow_schedule` is bucketed by legal payment period; defaults come
from the instrument's `credit_curve_id` when present, else from the deal
`DefaultModelSpec`, and behavioural prepayment from the deal
`PrepaymentModelSpec`. Call and put exercise follow
`InstrumentCollateral::call_exercise` / `put_exercise`
(`CallExercisePolicy::{Contractual, FirstCall, Worst, RefinancingIncentive}`,
`PutExercisePolicy::{Never, FirstPut, ReinvestmentIncentive{threshold_bp}}`) with per-instrument `overrides`.

Collateral draws (revolver utilization increases, delayed draws and
loan-equivalent draws at default) are funded from `AssetPool::reserve_account`,
then from the period's principal collections; revolver repayments replenish
the reserve up to `reserve_target`. The reserve earns `reserve_account_rate`
(annual decimal, ACT/360 on the opening balance) routed per
`reserve_interest_destination`: the waterfall (default), a named tranche, or
retained in the reserve. A deterministic run refuses a draw calendar the
reserve cannot fund; stochastic runs cap the draw and report the affected
paths (`StochasticPricingResult::unfunded_draw_path_fraction`).
`run_simulation_with_diagnostics` returns the reserve path and draw funding
alongside the tranche cashflows.

Stochastic pricing (`price_stochastic*`, OAS) drives the same instrument
engine path by path: per-name defaults through the deal's copula (or the
period's realized pool rate), and for stochastic revolvers a spread process
and a utilization process whose target can follow the spread
(`UtilizationProcess::MeanReverting::spread_sensitivity`), so draws rise on
the stress paths where names default. `StochasticPricingResult::draw_option_cost`
(and each tranche's share) values those draws at the contractual margin
against the path's fair spread; see
[`pricing/stochastic/README.md`](pricing/stochastic/README.md).

## Valuation

```rust
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::metrics::MetricId;

// Deal-level NPV (sum over tranches).
let pv = deal.value(&context, as_of)?;

// Deal-level NPV plus metrics. Note the fourth argument.
let result = deal.price_with_metrics(
    &context,
    as_of,
    &[MetricId::WAL, MetricId::DurationMod, MetricId::Cs01],
    PricingOptions::default(),
)?;

// Per-tranche.
let tranche_pv = deal.value_tranche("CLASS_A", &context, as_of)?;
let tranche_valuation = deal.value_tranche_with_metrics(
    "CLASS_A",
    &context,
    as_of,
    &[MetricId::WAL, MetricId::ZSpread],
)?;
```

Deterministic cashflows without pricing:

```rust
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    generate_cashflows, generate_tranche_cashflows, run_simulation,
};

let per_tranche = run_simulation(&deal, &context, as_of)?;          // HashMap<String, TrancheCashflows>
let aggregate = generate_cashflows(&deal, &context, as_of)?;        // DatedFlows
let class_a = generate_tranche_cashflows(&deal, "CLASS_A", &context, as_of)?;
```

## Waterfall

```text
Pool collections → Fees → Senior interest → Subordinate interest → Principal → Equity
                          ↓ (OC/IC failure)
                          divert to senior principal (turbo)
```

```rust
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    execute_waterfall, AllocationMode, PaymentType, Recipient, WaterfallBuilder, WaterfallContext,
    WaterfallTier,
};

let waterfall = WaterfallBuilder::new(Currency::USD)
    .add_tier(
        WaterfallTier::new("fees", 1, PaymentType::Fee).add_recipient(Recipient::fixed_fee(
            "trustee",
            "Trustee",
            Money::new(25_000.0, Currency::USD)?,
        )),
    )
    .add_tier(
        WaterfallTier::new("interest", 2, PaymentType::Interest)
            .allocation_mode(AllocationMode::Sequential)
            .add_recipient(Recipient::tranche_interest("A_INT", "CLASS_A")),
    )
    .build()?;

// Execution takes the whole period state as a WaterfallContext.
let distribution = execute_waterfall(&waterfall, &tranches, &pool, context)?;
```

`WaterfallBuilder::build()` returns `Result<Waterfall>`. `WaterfallContext`
carries the full period state: `available_cash`, `interest_collections`,
`principal_collections`, `payment_date`, `period_start`, `valuation_date`,
`pool_balance`, the `MarketContext`, plus optional current tranche/asset
balances, deferred interest, reserve and restricted cash, recovery proceeds and
an OAS floating-rate shift. `execute_waterfall_with_explanation` returns the
same distribution with a trace.

`Waterfall::standard_sequential` is deal-type aware: CLO/CBO deals split
interest so an OC/IC fail can trap junior coupon; ABS/RMBS/CMBS keep a
single non-divertible interest tier and turbo only residual cash.

Non-PIK deferred interest is a carried claim paid from later interest
collections. It does not compound. Interest coverage uses the same
denominator as the waterfall claim (current coupon plus those arrears).

Shifting-interest blends scheduled (pro-rata) and unscheduled (lock-out
schedule) principal by the period's unscheduled fraction; that blend is
equivalent to a two-bucket split when juniors share the remainder
pro-rata.

Declarative rules (`WaterfallRules`: available-funds cap, step-down triggers,
shifting interest) are layered onto the base waterfall by `resolve_waterfall`,
which is the identity when no rules are configured.

### Coverage triggers

Two distinct types share the name `CoverageTrigger`. They are not
interchangeable:

```rust
// Tranche-level: a threshold and what happens while it is breached.
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    CoverageTrigger, TriggerConsequence,
};

let trigger = CoverageTrigger::new(1.20, TriggerConsequence::DivertCashFlow)
    .with_cure_level(1.25);
```

```rust
// Waterfall-level: which tranche's OC/IC levels gate diversion in the waterfall.
use finstack_quant_valuations::instruments::fixed_income::structured_credit::waterfall::CoverageTrigger;

let waterfall = WaterfallBuilder::new(Currency::USD)
    // ... tiers ...
    .add_coverage_trigger(CoverageTrigger {
        tranche_id: "CLASS_A".into(),
        oc_trigger: Some(1.25),
        ic_trigger: Some(1.20),
    })
    .build()?;
```

## Behavioral models

Deterministic (single path, `PrepaymentModelSpec` / `DefaultModelSpec` /
`RecoveryModelSpec`, all re-exported at the module root):

| Model | Kind | Use |
|-------|------|-----|
| PSA | Prepayment | RMBS standard ramp |
| Constant CPR | Prepayment | Flat annual rate |
| SDA | Default | RMBS standard ramp |
| Constant CDR | Default | Flat annual rate |
| Constant recovery | Recovery | Fixed rate with resolution lag |

Stochastic (multi-path, `pricing::stochastic`): copula-based and
intensity-process defaults, factor-correlated / Richard-Roll /
regime-switching prepayment. Details in
[`pricing/stochastic/README.md`](pricing/stochastic/README.md).

Use deterministic for day-to-day valuation and regulatory reporting; use
stochastic for VaR / expected shortfall, correlation risk, and stress work that
needs a loss distribution.

### Rate conversions

```rust
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    clamped_cdr_to_mdr, clamped_cpr_to_smm, psa_to_cpr,
};

let smm = clamped_cpr_to_smm(0.06);  // 6% annual CPR -> monthly SMM
let mdr = clamped_cdr_to_mdr(0.02);  // 2% annual CDR -> monthly MDR
let cpr = psa_to_cpr(1.5, 30);       // 150% PSA at month 30
```

The `clamped_*` forms clamp their input to a valid rate range before
converting; the unclamped `cpr_to_smm` / `cdr_to_mdr` primitives are internal.

## Metrics

Registered for `InstrumentType::StructuredCredit`:

| Group | `MetricId` |
|-------|-----------|
| Pricing | `Accrued`, `CleanPrice`, `DirtyPrice`, `WAL` |
| Risk | `DurationMac`, `DurationMod`, `Convexity`, `Ytm`, `ZSpread`, `SpreadDuration` |
| Rates | `Dv01`, `BucketedDv01` |
| Credit | `Cs01`, `BucketedCs01` |
| Pool | `WAM`, `CPR`, `CDR`, `CloWarf`, `CloWas` |
| Deal-specific | `CmbsDscr`, `AbsChargeOff`, `AbsCreditEnhancement` |
| Sensitivities | `Recovery01`, `Prepayment01`, `Default01`, `Severity01` |

`Theta` is registered universally by `metrics::standard_registry()`.

**Metric time basis**: every structured-credit risk metric measures time on
Act/365F (the crate-internal `structured_credit::metrics::METRIC_TIME_BASIS`),
so duration and convexity are quoted
against the same yield unit the bump metrics define their shocks in. Do not
introduce a metric on the discount curve's own day count.

`BucketedCs01` here is a time-bucketing of the parallel z-spread shock:
`StructuredCredit` has no credit curve, so the z-spread is a scalar and
"key rate" means attribution by cashflow year fraction.

## Market conventions

- **Tranche interest** uses each tranche's own `day_count` (typically ACT/360)
  and its own payment frequency — the engine does not assume quarterly.
- **Pool interest collections** use asset-level day count when available,
  defaulting to ACT/360 for loans.
- **Coverage tests** use the tranche payment frequency for the IC calculation.
- Typical frequencies: ABS monthly, CLO quarterly, CMBS monthly, RMBS monthly.

## Bindings

Instrument collateral is typed in Python: `AssetPool.with_instruments(bonds=,
term_loans=, revolvers=, call_exercise=, put_exercise=, overrides=)` accepts
the typed `Bond`, `TermLoan` and `RevolvingCredit` classes,
`AssetPool.with_reserve(...)` configures the reserve account and its interest
destination, `StructuredCredit.run_simulation_with_diagnostics(market, as_of)`
returns the reserve path and draw funding (`SimulationDiagnostics`) and
`StructuredCredit.price_stochastic(market, as_of, num_paths=, antithetic=)`
returns a `StochasticPricingResult` with the tranche shares of the draw
option cost and its per-path distribution (`draw_option_cost_dataframe()`).
WASM carries the same fields in the deal JSON.

Both bindings expose structured credit under their `instruments` namespace:

- **Python** (`finstack_quant.valuations.instruments`): typed
  `StructuredCredit`, `StructuredCreditBuilder`, `AssetPool`, `RepLine`,
  `Tranche`, `TrancheBuilder`, `TrancheStructure`, plus tranche analytics
  `structured_credit_tranche_metrics`, `structured_credit_tranche_oas`,
  `structured_credit_tranche_discount_margin`,
  `structured_credit_tranche_breakeven_cdr`,
  `structured_credit_tranche_scenario_table` and the result types
  `TrancheMetrics`, `OasResult`, `ScenarioTable`. Generic pricing via
  `price_instrument(...)`.
- **WASM** (`valuations.instruments`): `structuredCreditTrancheMetrics`,
  `structuredCreditTrancheOas`, `structuredCreditTrancheDiscountMargin`,
  `structuredCreditTrancheBreakevenCdr`,
  `structuredCreditTrancheScenarioTable`, plus `priceInstrument` and
  `instrumentCashflowsJson` on the `InstrumentJson::StructuredCredit` envelope.

Deep sub-configs (`WaterfallRules`, `CreditModelConfig`, `DealFees`,
loan-level `PoolAsset`, floating `TrancheCoupon`) stay JSON sub-fields in both
bindings.

## Verification

```bash
# Structured-credit unit, feature, waterfall-golden and simulation tests
cargo nextest run -p finstack-quant-valuations --test instruments structured_credit::

# Whole workspace (never `cargo test` — it runs doctests)
mise run rust-test

# Lints
mise run rust-lint
```

The registry JSON is embedded with `include_str!` and parsed at load, so a
malformed or renamed profile fails the tests above rather than silently
defaulting. `mise run assumptions-audit` is a *separate* scan: it reports
hard-coded assumptions elsewhere in the workspace that should move into a
registry — it does not validate this file's contents.

## See also

- [`pricing/stochastic/README.md`](pricing/stochastic/README.md) — stochastic models and pricing modes
- [`../../README.md`](../../README.md) — instrument module map and how to add one
- [`../../../metrics/README.md`](../../../metrics/README.md) — metric ids and calculators
- [`INVARIANTS.md`](../../../../../../INVARIANTS.md) — Decimal/f64, determinism and serde invariants
- [`docs/REFERENCES.md`](../../../../../../docs/REFERENCES.md) — bibliography


### Collection accounts and current-state inputs

The waterfall keeps interest and principal in separate accounts. Fees and coupons
consume interest; principal pays capital or buys replacement collateral. A coverage
cure or acceleration explicitly transfers interest into debt principal. Equity
interest distributions do not retire its loss-absorbing balance. Undistributed cash
carries forward by account and is distributed at termination; a cash trap cannot
extinguish the balance.

Supply current collateral and note balances. For an already-defaulted asset,
`balance` is its defaulted par, `default_date` is its economic default date, and
`recovery_amount` is the outstanding, unreceived claim. Performing collateral excludes
that par. The claim enters the recovery queue once, with payment at the default date
plus the canonical recovery lag. Current note balances must already reflect past
losses; those losses inform cumulative triggers without another write-down.

`pool.reinvestment_period` is the sole reinvestment configuration. Both `is_active`
and its inclusive end date control purchases. Revolving notes retain their configured
`target_balance`, or current balance when no target is supplied; other notes amortize.
Principal that cannot be invested stays in the principal account until eligible
placement or the end of the revolving period. Replacements preserve the surviving
collateral profile pro rata, including credit-quality weights, maturities, and level
payments. `max_price` is percent of par; `min_yield` is annual decimal current yield
(coupon divided by price fraction). Floating replacement coupons use the current
projection. No unspecified eligibility filter is assumed.

Tranche coverage triggers retain their first breach date until the cure ratio is
reached. `divert_cash_flow` pays the coverage cure from divertible interest;
`trap_excess_spread` holds residual interest until cure; `stop_reinvestment` suspends
purchases; `accelerate_amortization` also uses residual interest to repay debt below
revolving targets. A subsequent breach starts a new breach date. Configure an OC or IC
test in either the tranche or waterfall, avoiding duplicate definitions of the same
test. Removed fields include duplicate reinvestment end dates, expected maturity,
tranche credit-enhancement balances, and consequences without executable parameters.
