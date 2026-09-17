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
| `CoverageTestSpec`, `CoverageTestAction`, `CoverageTestType`, `CoverageTrigger` (tranche-level), `TriggerConsequence` | OC/IC tests as waterfall positions — see [Coverage tests](#coverage-tests). |
| `DealFees`, `IncentiveFeeSpec` | Fee schedule the template turns into senior, junior and incentive fee tiers. |
| `CoverageRules`, `DefaultedValuation`, `CccBucketRule`, `DiscountObligationRule` | Collateral valuation rules for the OC tests (rating haircuts, defaulted-asset value, excess-CCC bucket, discount obligations); `CoverageRules::clo_standard()` from the registry. |
| `PrepaymentModelSpec`, `DefaultModelSpec`, `RecoveryModelSpec`, `PrepaymentCurve`, `DefaultCurve` | Deterministic behavioral models. |
| `PricingMode` | Valuation-owned stochastic pricing mode — see [`pricing/stochastic/README.md`](pricing/stochastic/README.md). |
| `StochasticPrepaySpec`, `StochasticDefaultSpec`, `CorrelationStructure`, `PoolGranularity` | Models-owned stochastic inputs; import from `finstack_quant_models::credit::pool`. |
| `StochasticPricingResult`, `TranchePricingResult` | Stochastic output. |
| `ReinvestmentPeriod`, `ReinvestmentCriteria`, `ReinvestmentAssumptions` | Deal-level reinvestment contract: window, eligibility, notes that amortize inside the window, and the replacement-collateral terms — see [Collection accounts and current-state inputs](#collection-accounts-and-current-state-inputs). |
| `LossAllocationPolicy` | `Tranche` loss treatment per deal type: `WriteDown` (RMBS/CMBS default) allocates realized losses at default, `ParPreserving` (CLO/CBO/ABS default) carries par and realizes the shortfall at legal final; override with `StructuredCredit.loss_allocation`. |
| `EarlyAmortizationSpec`, `ControlledAccumulationSpec`, `ExcessSpreadSpec` | ABS/credit-card structural features. |
| `CardPortfolioSpec`, `DelinquencyModel`, `AdvancingPolicy`, `ModificationSpec` | Card master-trust portfolio model and roll-rate delinquency with servicer advancing — see [Behavioral models](#behavioral-models). |
| `BalloonSpec`, `PrepaymentPenalty`, `SpecialServicingSpec` | Commercial-mortgage loan terms on `PoolAsset` — see [CMBS collateral terms](#cmbs-collateral-terms). |
| `LiquidationSpec` | NPL/RPL resolution timeline on `PoolAsset.liquidation` — see [NPL / RPL resolution](#npl--rpl-resolution). |
| `BorrowingBaseRules`, `AdvanceRate`, `EligibilityRule`, `ConcentrationLimit`, `ConcentrationScope`, `BorrowingBaseReport` | Advance rates, eligibility and concentration limits behind the borrowing-base coverage test and the `asset_backed_facility` instrument. |
| `HedgeSwap`, `SwapNotional`, `SwapPriority` | Interest-rate hedges paid through the waterfall as senior or junior fee recipients. |
| `CallAssumption`, `CallScope` | Deal or tranche call for price-to-call analytics (`TrancheMetrics.wal_to_call`, `z_spread_to_call_bp`, `dm_to_call_bp`) — see [Calls and clean-up calls](#calls-and-clean-up-calls). |
| `run_simulation_with_diagnostics`, `SimulationRun`, `SimulationDiagnostics`, `PeriodDiagnostics`, `CoverageTestDiagnostic`, `calculate_equity_metrics`, `EquityMetrics` | Period-by-period deal record and equity analytics — see [Deal diagnostics and equity analytics](#deal-diagnostics-and-equity-analytics). |
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
| `DealType::Abs` | Auto loans, credit cards (`credit_model.card`) | `abs_auto_standard` | `AbsChargeOff`, `AbsCreditEnhancement`, `AbsExcessSpread`, `AbsPaymentRate`, `AbsDelinquency` |
| `DealType::Clo` | Leveraged loans | `clo_standard` | `CloWarf`, `CloWas` |
| `DealType::Cmbs` | Commercial mortgages | `cmbs_standard` | `CmbsDscr` |
| `DealType::Rmbs` | Residential mortgages, NPL/RPL pools (`PoolAsset.liquidation`) | `rmbs_standard` | — (use `WAL`) |

`new_*` constructors pull their frequency, prepayment, default and recovery defaults from the
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

`Waterfall::standard_sequential` gives every note its own interest tier, in
seniority order, so a coverage-test position can sit between any two coupons;
principal and the residual follow. Coverage tests are inserted after the
interest tier of the class they are placed on (`insert_coverage_test`).

Interest and principal proceeds are separate accounts. Each tier draws on the
account matching its `PaymentType` unless it sets a `FundingSource`:
`InterestThenPrincipal` lets a fee or interest tier top up from principal
proceeds, and the cash taken is reported as
`WaterfallDistribution::principal_used_for_interest`. The template applies
this to senior fees and senior note coupons when
`StructuredCredit::principal_covers_senior_interest` is in force (the default
for CLO/CBO deals, off for every other deal type; explicit `Some(_)`
overrides), via `Waterfall::fund_senior_interest_from_principal`. Custom
waterfalls set `WaterfallTier::funding` per tier.

Hedge swaps (`StructuredCredit::hedge_swaps`, `HedgeSwap { swap, notional,
priority }`) settle through the waterfall rather than as an NPV overlay: each
period the swap's net flow from the deal's side is bucketed, a net receipt
joins interest proceeds and a net payment becomes a `SwapCounterparty` fee
recipient in the senior fee tier (`SwapPriority::SeniorFee`, so it nets out of
the IC numerator) or in a fee tier ahead of the residual (`JuniorFee`).
`SwapNotional::TranchePar`/`PoolPar` rescale the flows each period to the note
or performing pool balance (balance-guaranteed swaps); `Contractual` uses the
swap's own notional.

Fees (`DealFees`, `TemplateFees`): the trustee, senior management and
servicing fees form the leading `fees` tier; `subordinated_mgmt_fee_bp` forms a
`junior_fees` tier after every note coupon and ahead of principal; and
`incentive_fee` (`IncentiveFeeSpec { hurdle_irr, share_pct }`, 12% / 20% in the
standard CLO registry) forms an `incentive_fee` tier ahead of the residual
whose `PaymentCalculation::IncentiveFee` pays `share_pct` of the residual only
once the equity IRR to date (`WaterfallContext::equity_history`, capital at
closing against every equity distribution plus the residual on the payment
date) reaches the hurdle. Junior and incentive fees always draw on interest
proceeds.

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

### Attachment points

`Tranche.attachment_point` / `detachment_point` are `Option<f64>` percents.
`Tranche::from_balance(...)` (and a `TrancheBuilder` without points) leaves
them `None`; `TrancheStructure::new` derives them from the original-balance
shares in payment-priority order (first-loss class at 0, most senior class
detaching at exactly 100) and validates any declared points against those
same shares within 0.5%. `TrancheStructure::from_balances(tranches)` discards
declared points and derives all of them. The stochastic pricer's
`TranchePricingResult.attachment` / `detachment` read the resolved points, so
a derived structure prices exactly like its declared twin
(`unit/attachment_derivation_tests.rs`).

### Coverage tests

An OC/IC test is a *position* in the waterfall: a `PaymentType::CoverageTest`
tier carrying one or more `CoverageTestSpec`s. While any test at that position
fails, the interest still undistributed there — never the coupons already paid
above it — is diverted up to the binding cure (the largest failing cure, not
the sum) to the earliest principal tier's recipients in order
(`CoverageTestAction::PayDownSenior`), or retained as principal proceeds
(`CoverageTestAction::Reinvest`). A Class D test placed after D's own coupon
therefore traps only the residual; a Class A test placed after A's coupon traps
the mezzanine coupons and the residual. Every failing test also suspends
reinvestment for the period.

```rust
// Deal-level: placed after the tested class's interest tier by `create_waterfall`.
use finstack_quant_valuations::instruments::fixed_income::structured_credit::CoverageTestSpec;

let deal = deal.with_coverage_triggers(vec![
    CoverageTestSpec::oc("CLASS_B", 1.20),
    CoverageTestSpec::ic("CLASS_B", 1.15).after_tranche("CLASS_A"),
])?;
```

```rust
// Custom waterfall: an explicit test position between two interest tiers.
let waterfall = WaterfallBuilder::new(Currency::USD)
    .add_tier(WaterfallTier::new("a_interest", 1, PaymentType::Interest)
        .add_recipient(Recipient::tranche_interest("a_int", "CLASS_A")))
    .add_tier(WaterfallTier::coverage_tests("a_coverage", 2, vec![
        CoverageTestSpec::oc("CLASS_A", 1.25),
        CoverageTestSpec::ic("CLASS_A", 1.20),
    ]))
    // ... junior interest, principal, residual ...
    .build()?;
```

Tranche-level `CoverageTrigger`s (`Tranche::oc_trigger` / `ic_trigger`) are the
stateful variant: a threshold, a cure level and a `TriggerConsequence`. A
breached `DivertCashFlow` trigger inserts the same test position after that
tranche's interest tier for the period; the other consequences act on
reinvestment and amortization.

```rust
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    CoverageTrigger, TriggerConsequence,
};

let trigger = CoverageTrigger::new(1.20, TriggerConsequence::DivertCashFlow)
    .with_cure_level(1.25);
```

#### Borrowing-base tests

`CoverageTestSpec::borrowing_base(tranche_id, required_ratio)` is a third
test kind (`CoverageTestType::BorrowingBase`): its numerator is the
`BorrowingBaseRules` on `CoverageRules.borrowing_base` evaluated on the live
asset balances (advance rates by `AssetType` wire name or `"*"`, eligibility,
concentration limits that scale each over-cap obligor/industry/class down to
its cap) plus the collection and funding-account cash, its denominator the
tested class's balance plus everything senior. It diverts and cures like an
OC test and is what the `asset_backed_facility` instrument synthesizes; any
deal can carry one. `BorrowingBaseRules::evaluate` returns the
`BorrowingBaseReport` (eligible collateral, concentration excess, base).

#### Coverage rules

`CoverageRules` (`StructuredCredit::coverage_rules`, or `Waterfall::coverage_rules`
on a custom waterfall) decide what the OC numerator counts. Without rules
collateral is carried at par and defaulted assets at their modeled recovery.
`CoverageRules::clo_standard()` is the indenture convention from the registry;
each rule can also be set individually:

| Rule | Effect on the OC numerator |
|------|----------------------------|
| `rating_haircuts` (`CreditRating` → decimal fraction) | Each asset is carried at `par × (1 − haircut)` for its `credit_quality` (the `NR` entry covers unrated assets, including materialized instrument collateral). |
| `defaulted_valuation` | `Recovery` (default) carries pending defaulted par at its modeled recovery; `MarketValue { pct }` carries it at `pct`% of par. |
| `ccc_bucket { threshold_pct, carry_at_market_value }` | CCC-and-below par above `threshold_pct`% of performing par is carried at the CCC assets' balance-weighted `PoolAsset::market_price_pct` (or excluded). |
| `discount_obligation { price_threshold_pct }` | An asset whose `purchase_price` is below the threshold (percent of closing par) is carried at that purchase price. |

An asset is always carried at the lowest of the applicable values, never above
par. Percent fields are percents (`7.5`, `80.0`, `60.0`); haircuts are decimal
fractions. `unit/coverage_rules_tests.rs` hand-computes each rule.

## Behavioral models

Deterministic (single path, `PrepaymentModelSpec` / `DefaultModelSpec` /
`RecoveryModelSpec`, all re-exported at the module root):

| Model | Kind | Use |
|-------|------|-----|
| PSA | Prepayment | RMBS standard ramp |
| Constant CPR | Prepayment | Flat annual rate |
| CMBS lockout | Prepayment | Zero for `lockout_months`, then constant CPR |
| ABS speed (`PrepaymentModelSpec::abs`) | Prepayment | Auto/consumer ABS: `SMM_t = ABS / (1 − ABS·(t − 1))` (Fabozzi); `new_abs` uses the registry's 1.5% ABS |
| Vector (`PrepaymentModelSpec::vector`) | Prepayment | Explicit annual CPR per seasoning month, last value held |
| SDA | Default | RMBS standard ramp |
| Constant CDR | Default | Flat annual rate |
| Vector (`DefaultModelSpec::vector`) | Default | Explicit annual CDR per seasoning month, last value held |
| Cumulative loss (`DefaultModelSpec::cumulative_loss`) | Default | Rating-agency net-loss curve (percent of the pool balance at simulation start) plus severity; the engine sizes each month's defaults against the surviving balance (`mdr_with_survival`) so the curve is reproduced on amortizing pools |
| Timing (`DefaultModelSpec::timing`) | Default | Lifetime cumulative default rate spread by annual timing percentages (e.g. 15/30/30/15/10), linear within each year |
| Constant recovery | Recovery | Fixed rate with resolution lag |
| Severity vector (`RecoveryModelSpec::with_severity_vector`) | Recovery | Loss severity by month of default, last value held |

Cumulative-loss and timing curves and severity vectors are deterministic-only:
stochastic pricing rejects them at validation because the path engines size
defaults from a per-month rate on the surviving balance.

### Delinquency, advancing and modification

`credit_model.delinquency: Option<DelinquencyModel>` (asset and rep-line pools
only) turns the default model into the *entry* into 30 days delinquent.
Balances then roll bucket to bucket (`roll_rates`, monthly), cure back to
current (`cure_rates`) or stay; the roll out of the last bucket is the
charge-off, which enters the recovery queue like any default. Delinquent
balances remain in the pool's par (OC tests, step-down metrics and the
cleanup-call factor all count them) but pay no interest or scheduled
principal until they cure; balances still delinquent at the loan's maturity
charge off.

- `AdvancingPolicy::PrincipalAndInterest { recoverability_cap_pct }` makes the
  servicer advance the interest and scheduled principal the delinquent
  balance misses, while advances outstanding stay within the cap (percent of
  the delinquent balance). Advances are reimbursed from recovery proceeds
  before that cash reaches the waterfall.
- `ModificationSpec { rate_reduction_bp, term_extension_months,
  share_of_delinquent }` modifies a share of every bucket back to current each
  month; on a rep line the concession blends into the line's coupon (spread
  for floaters) and, for level-pay collateral, the recast payment over the
  extended term. The line's maturity is unchanged.
- `PoolAsset::delinquency_buckets` seeds the buckets of a seasoned pool (one
  amount per bucket) and drives the `abs_delinquency` metric.
- `StepDownTrigger::MaxDelinquency(max)` passes while the delinquent share of
  the pool is at or below `max`.

`unit/delinquency_tests.rs` checks the bucket recursion, cures, advancing and
the trigger against hand computations.

### Card master trusts

`credit_model.card: Option<CardPortfolioSpec>` (asset and rep-line pools
only) prices a credit-card master trust. The pool is the investor interest in
the receivables and the spec replaces the collateral's behavioral inputs:

| Field | Replaces | Effect |
|-------|----------|--------|
| `monthly_payment_rate` | prepayment model | Principal collections are the payment rate on the receivables; the revolving period (`pool.reinvestment_period`) recycles them into new receivables, controlled accumulation or amortization then pays the notes. |
| `portfolio_yield` | asset coupons | Interest collections are the annual yield (finance charges and fees) on the performing receivables. |
| `charge_off_rate` | default model | Annual charge-offs (an asset `mdr_override` still wins); recoveries follow `recovery_spec`. |

`EarlyAmortizationSpec::min_excess_spread_3m` adds the excess-spread test:
every period the engine records the annualized excess spread realized on the
opening pool balance (interest collections less debt coupons due, fees paid
and net charge-offs), and once the three-period trailing average falls below
the floor the revolving period ends for good. `abs_excess_spread` reports
the static excess spread (yield − balance-weighted debt coupon − servicing fee
− charge-off rate, percent per annum) and `abs_payment_rate` the monthly
payment rate in percent. The seller's interest and cardholder purchase rate
are outside the model: purchases are whatever the revolving period recycles.
`unit/card_master_trust_tests.rs` hand-computes the metrics, the
early-amortization timing and the accumulation bullet.

### CMBS collateral terms

Commercial mortgages carry their terms on the `PoolAsset`:

- `balloon: Option<BalloonSpec { default_prob, extension_months,
  extension_rate }>` — at the loan's maturity the share `default_prob` of the
  balance fails to refinance and is extended `extension_months` at
  `extension_rate` (the loan's coupon when `None`); the rest pays as the
  balloon. One extension per loan; the extended balance pays at the extended
  maturity.
- `prepayment_penalty: Option<PrepaymentPenalty>` — `Fixed { pct, through }`
  charges `pct` percent of prepaid principal, `YieldMaintenance {
  reinvestment_rate, through }` charges `prepaid × max(0, coupon −
  reinvestment_rate) × years to maturity` (undiscounted). The premium is
  collected as interest; nothing is charged after `through`.
- `special_servicing: Option<SpecialServicingSpec { appraisal_reduction_pct }>`
  — the appraisal reduction (ASER) cuts the interest advanced on the loan to
  `1 − pct/100` of the balance; the shortfall falls on the most junior
  classes through the sequential interest waterfall. The template's
  `special_servicer_fee_bp` accrues only on specially serviced balances via
  `PaymentCalculation::PercentageOfSpecialServiced` (custom waterfalls can use
  the same calculation).
- `noi: Option<Money>` — annual net operating income; when any asset carries
  it, `MetricId::CmbsDscr` is pool NOI over pool debt service (twelve
  contractual payments for level-pay loans, the coupon on the balance for
  interest-only loans), otherwise the deal-level `credit_factors` are used.

`unit/cmbs_tests.rs` covers the balloon extension, the ASER shortfall
ordering, both penalties, the special fee base and the loan-level DSCR.

### NPL / RPL resolution

A non-performing loan carries `liquidation: Option<LiquidationSpec {
months_to_resolution, proceeds_pct, carry_cost_pct, reperformance_prob,
modified_rate }>` on its `PoolAsset` and stays `is_defaulted: false` (the
timeline replaces the default flag; `recovery_amount` / `default_date` must be
unset). It pays nothing until the first payment date at or after
`months_to_resolution` months from closing. There the share
`1 − reperformance_prob` liquidates — booked as a default whose recovery is
`(proceeds_pct − carry_cost_pct)%` of the liquidated balance, released after
the deal's `recovery_lag` like any other recovery — and the share
`reperformance_prob` re-performs at `modified_rate` (the loan's coupon when
`None`) on its original amortization terms from the next period on, with the
level payment recast on the re-performing balance. Coverage tests carry the
loan at par until resolution unless a coverage rule (`discount_obligation`,
`rating_haircuts`) says otherwise, so NPL deals bought at a discount size
their notes on price with `LossAllocationPolicy::ParPreserving` or add a
`discount_obligation` rule. Asset rows only; stochastic pricing rejects
liquidation terms. `unit/npl_tests.rs` covers the timeline, the net proceeds
and the modified coupon.

### Calls and clean-up calls

- `StructuredCredit::call_assumption: Option<CallAssumption { date, price_pct,
  scope }>` prices the deal to an assumed optional redemption. `CallScope::Deal`
  liquidates the collateral on the first payment date at or after `date` and
  redeems every note at `price_pct` of its balance plus stub accrued and
  deferred interest (a premium over par is interest, a discount is a principal
  write-down); the projection ends there and equity takes the residual.
  `CallScope::Tranche(id)` leaves the deal's cashflows unchanged and prices
  only that class to its refinancing.
- `TrancheMetrics` always reports `wal`, `z_spread_bp` and the other figures
  to maturity (projected without the call) and adds `wal_to_call`,
  `z_spread_to_call_bp` and, for floaters, `dm_to_call_bp` when a call covers
  the tranche.
- `liquidation_price_pct` (percent of par, `None` = par) is the price the
  collateral realizes in a deal call or clean-up call. Proceeds are the
  liquidated pool plus pending recoveries and every cash account (funding,
  reserve, spread, undistributed). The clean-up call (`cleanup_call_pct`) is
  exercised only when those proceeds cover the debt notes' claims; a call
  assumption redeems regardless, bounded by the proceeds.

`unit/call_tests.rs` covers the redemption, the twins and the proceeds test.

### Deal diagnostics and equity analytics

`run_simulation_with_diagnostics` returns `SimulationRun { tranches,
diagnostics }`. Besides the reserve fields, `SimulationDiagnostics::periods`
holds one `PeriodDiagnostics` per payment date: end-of-period pool balance and
factor, balance-weighted coupon / spread / WARF of the performing collateral,
the period's interest and principal collections, defaults, released
recoveries, reinvested par and fees paid, the reserve, spread and funding
account balances, the delinquent balance, the realized excess spread, and
every coverage test as the executor evaluated it (`CoverageTestDiagnostic {
test_id, ratio, trigger_level, cushion, passing }`; a single source, not a
recomputation).

`calculate_equity_metrics(deal, market, as_of, purchase_price_pct)` returns
`EquityMetrics { invested, irr, moic, nav_pct, cash_on_cash }` for the residual
class: `−invested` on the valuation date against every projected distribution
(reserve interest routed to equity included), solved with `xirr`.
`unit/diagnostics_tests.rs` checks the coverage record, the IRR and the JSON
round-trip.

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

**Price basis**: note prices, quotes and spread solves are per CURRENT face
(`Tranche::current_balance`); `TrancheMetrics::factor` and
`TrancheValuation::factor` report `current / original`, and the deal-level
notional is the sum of current balances. A note paid down to factor 0.5 that is
worth par prints `price_pct ≈ 100`, not 50, and a desk quote of 99.0 solves a
sensible spread on a seasoned trade.

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
  `StructuredCredit`, `StructuredCreditBuilder` (one setter per settable Rust
  field: credit model specs, `delinquency`, `card`, `coverage_triggers`,
  `coverage_rules`, `call_assumption`, `cleanup_call_pct`,
  `liquidation_price_pct`, `loss_allocation`,
  `principal_covers_senior_interest`, `waterfall`, `hedge_swaps`, `fees`,
  `waterfall_rules`, `behavior_overrides`, `attributes` ...), `AssetPool`
  (`with_assets`, `with_rep_lines`, `with_instruments`, `with_reserve`,
  `with_reinvestment_period`, `with_accounts`), `PoolAsset` (one constructor
  keyword per Rust field, `fixed_rate_bond` / `floating_rate_loan`),
  `RepLine`, `Tranche`, `TrancheBuilder` (`current_balance`,
  `deferred_interest`, `rating`, `pik_enabled`, `oc_trigger`, `ic_trigger`,
  `attributes`), `TrancheStructure`, `CallAssumption`, `CoverageRules`,
  `HedgeSwap`, `Waterfall` (`StructuredCredit.create_waterfall()`
  introspection); deal methods `with_standard_fees`,
  `enable_stochastic_defaults`, `tranche_cashflows` (`TrancheCashflows`),
  `equity_metrics` (`EquityMetrics`), `run_simulation_with_diagnostics`
  (`SimulationDiagnostics` with the per-period record and coverage-test
  frame), `price_stochastic`; tranche analytics
  `structured_credit_tranche_metrics` (with the `*_to_call` twins),
  `structured_credit_tranche_oas`, `structured_credit_tranche_discount_margin`,
  `structured_credit_tranche_breakeven_cdr`,
  `structured_credit_tranche_scenario_table` and the result types
  `TrancheMetrics`, `OasResult`, `ScenarioTable`. Generic pricing via
  `price_instrument(...)`. `tests/parity/test_structured_credit_fields.py`
  checks every schema field of the five core types against the getters and
  setters; `tests/test_typed_structured_credit_surface.py` rebuilds the CLO
  regression golden from typed calls alone.
- **WASM** (`valuations.instruments`): `structuredCreditTrancheMetrics`
  (its `TrancheMetrics` shape carries the optional `wal_to_call`,
  `z_spread_to_call_bp`, `dm_to_call_bp`), `structuredCreditTrancheOas`,
  `structuredCreditTrancheDiscountMargin`,
  `structuredCreditTrancheBreakevenCdr`,
  `structuredCreditTrancheScenarioTable`, plus `priceInstrument` and
  `instrumentCashflowsJson` on the `InstrumentJson::StructuredCredit` envelope.

Deep sub-configs (`WaterfallRules`, the stochastic specs, `DealFees`,
`DelinquencyModel`, `CardPortfolioSpec`, `CoverageTestSpec`,
`CoverageTrigger`, the CMBS `PoolAsset` sub-specs, floating `TrancheCoupon`)
stay dict / JSON sub-fields in Python and everything stays JSON in WASM.

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

`pool.reinvestment_period` is the sole reinvestment configuration; it is not
available for instrument-collateral pools. Both `is_active` and its inclusive end
date control purchases. Every note is held flat during the period except those named
in `amortizing_tranches`, which are paid down first; only the principal left after
their paydown is recycled. Principal that cannot be invested stays in the principal
account until eligible placement or the end of the revolving period. Without
`assumptions`, replacements preserve the surviving collateral profile pro rata,
including credit-quality weights, maturities, and level payments. With
`ReinvestmentAssumptions { spread_bp, price_pct, maturity_months, index_id,
coupon_floor }`, each period's purchases are booked as a synthetic `REINVEST-{n}`
first-lien bullet row (ACT/360, maturity capped at legal final) at the stated
terms, so the replacement spread, price and tenor drive WAS, par build and
excess spread. `max_price` is percent of par; `min_yield` is annual decimal current
yield (coupon divided by price fraction). Floating replacement coupons use the
current projection. No unspecified eligibility filter is assumed.
`StructuredCredit::behavior_overrides.reinvestment_price` (percent of par)
overrides the purchase price from `price_pct` for scenario work.

Tranche coverage triggers retain their first breach date until the cure ratio is
reached. `divert_cash_flow` places a coverage-test position after the tranche's
interest tier and pays the cure from the interest below it;
`trap_excess_spread` holds residual interest until cure; `stop_reinvestment` suspends
purchases; `accelerate_amortization` also uses residual interest to repay debt below
revolving targets. A subsequent breach starts a new breach date. Configure an OC or IC
test either as a tranche trigger or as a deal/waterfall coverage test, avoiding
duplicate definitions of the same test. Removed fields include duplicate reinvestment end dates, expected maturity,
tranche credit-enhancement balances, and consequences without executable parameters.
