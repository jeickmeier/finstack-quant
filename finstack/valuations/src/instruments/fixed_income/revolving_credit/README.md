## Revolving Credit Facility (RCF) — Pricing and Cashflow Engine

This module implements a complete, production‑grade revolving credit facility (RCF) with deterministic and stochastic utilization modeling, fixed or floating base rates, tiered fees, and optional credit‑risk survival weighting. A single unified implementation drives both pricing modes and cashflow generation to ensure parity, determinism, and maintainability.

### Key capabilities

- Deterministic draws/repays and full cashflow schedule generation
- Stochastic utilization via a 3‑factor Monte Carlo (utilization, interest rate, credit spread) with correlation
- Fixed or floating base rates (forward curve projection, margin, optional floors)
- Fees: upfront, commitment (tiered by utilization), usage (tiered), and facility fees
- Credit risk: hazard curve survival weighting or dynamic survival from simulated credit spreads
- Unified PV calculation and metric integration (DV01/CS01/Theta/Bucketed DV01 + facility metrics)

---

## Instrument surface

The instrument is defined by `RevolvingCredit` with a builder for ergonomic construction:

```rust
use finstack_valuations::instruments::fixed_income::revolving_credit::{
    RevolvingCredit, RevolvingCreditFees, BaseRateSpec, DrawRepaySpec, DrawRepayEvent,
};
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::money::Money;
use finstack_core::currency::Currency;

let facility = RevolvingCredit::builder()
    .id("RC-001".into())
    .commitment_amount(Money::new(10_000_000.0, Currency::USD))
    .drawn_amount(Money::new(5_000_000.0, Currency::USD))
    .commitment_date(Date::from_ymd(2025, 1, 1).unwrap())
    .maturity_date(Date::from_ymd(2026, 1, 1).unwrap())
    .base_rate_spec(BaseRateSpec::Floating {
        index_id: "USD-SOFR-3M".into(),
        margin_bp: 200.0,
        reset_freq: Tenor::quarterly(),
        index_floor_bp: Some(0.0),
    })
    .day_count(DayCount::Act360)
    .payment_frequency(Tenor::quarterly())
    .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
    .draw_repay_spec(DrawRepaySpec::Deterministic(vec![
        DrawRepayEvent { date: Date::from_ymd(2025, 3, 1).unwrap(), amount: Money::new(1_000_000.0, Currency::USD), is_draw: true },
        DrawRepayEvent { date: Date::from_ymd(2025, 6, 1).unwrap(), amount: Money::new(500_000.0, Currency::USD), is_draw: false },
    ]))
    .discount_curve_id("USD-OIS".into())
    // Optional credit risk inputs:
    // .hazard_curve_id("BORROWER-HZ".into())
    // .recovery_rate(0.4)
    .build()?;
```

Inputs of note:

- Base rate: `Fixed { rate }` or `Floating { index_id, margin_bp, reset_freq, index_floor_bp }`
- Fees: `upfront_fee`, `commitment_fee_tiers`, `usage_fee_tiers`, `facility_fee_bp`
- Draw/repay regime: `DrawRepaySpec::Deterministic(Vec<DrawRepayEvent>)` or `DrawRepaySpec::Stochastic(...)`
- Optional `hazard_curve_id` and `recovery_rate` to enable survival weighting
- Calendar metadata via `attributes` (e.g., `calendar_id`) to control schedule adjustments

---

## Architecture

- Cashflow engine (`cashflow_engine.rs`)
  - Deterministic: period slicing around intra‑period draw/repay events; accrues interest and fees exactly on sub‑period balances; emits period cashflows at period end and all principal flows on event dates
  - Stochastic: consumes 3‑factor paths (utilization, short rate, credit spread) and produces per‑period cashflows; principal changes occur when utilization moves between period start and end
- Path generator (`pricer/path_generator.rs`)
  - 3‑factor process with correlation:
    - Utilization: mean‑reverting OU‑style process bounded to [0,1] at output
    - Short rate: deterministic forward curve, or Hull‑White 1F
    - Credit spread: CIR or market‑anchored to hazard curve; maps to hazard for survival
  - RNG: Philox (default) or Sobol QMC
- Unified pricer (`pricer/unified.rs`)
  - Deterministic: single cashflow schedule → discount and survival weight
  - Stochastic: generate many paths → per‑path deterministic pricing → aggregate MC statistics; optional full path capture (`price_with_paths`)
- Components (`pricer/components.rs`)
  - Upfront fee PV, discount factor utilities, survival weights, and rate projection helpers
- Utilities (`utils.rs`)
  - Calendar‑aware schedule and reset‑date builders, floating‑rate projection, and balance evolution helpers

All paths produce a `CashFlowSchedule` which downstream metrics and exporters consume uniformly.

---

## Cashflows and sign conventions

From the lender’s perspective:

- Principal draws: negative cashflows (capital deployment)
- Principal repayments: positive cashflows
- Interest and all fees: positive cashflows at period end

Deterministic engine uses intra‑period event slicing to accrue interest/fees on exact drawn balances between events. Stochastic engine uses the average utilization in the period for accruals and posts principal deltas at period end.

Flow ordering at the same date is deterministic: interest/reset → fees → amortization/PIK → notional.

---

## Mathematics

### Interest and fees

For a sub‑period \([t_i, t_{i+1}]\) with accrual factor \(dt\):

- Interest (fixed): \(I = B_\text{drawn} \cdot r \cdot dt\)
- Interest (floating): \(I = B_\text{drawn} \cdot \max(\text{index}, \text{floor}) + \text{margin}\) applied over \(dt\)
- Commitment fee: \(F_c = (C - B_\text{drawn}) \cdot \text{commitment\_bps} \cdot 10^{-4} \cdot dt\)
- Usage fee: \(F_u = B_\text{drawn} \cdot \text{usage\_bps} \cdot 10^{-4} \cdot dt\)
- Facility fee: \(F_f = C \cdot \text{facility\_bp} \cdot 10^{-4} \cdot dt\)

Tiered fees choose the highest tier where \( \text{utilization} \ge \text{threshold} \).

### Credit survival weighting

PV uses discount factors and survival probabilities:
\[ \mathrm{PV} = \sum_i \left( \mathrm{CF}_i \cdot \mathrm{DF}(t_i) \cdot \mathrm{SP}(t_i) \right) + \mathrm{PV}_\text{upfront} \]

- Static hazard curve: \(\mathrm{SP}(t)\) taken from the hazard curve at each cashflow date.
- Dynamic survival from credit spread path: hazard is mapped via \( \lambda_t \approx \frac{s_t}{1 - R} \) and integrated cumulatively to get \(\mathrm{SP}(t) = e^{-\int_0^t \lambda_u du}\) with linear interpolation between simulated grid points.

### Monte Carlo processes (stochastic mode)

- Utilization: mean‑reverting OU on a real line, output clamped to \([0,1]\)
- Short rate:
  - Deterministic: read from forward curve by period (rate locking per step)
  - Stochastic: Hull‑White 1F \(dr_t = \kappa(\theta - r_t)dt + \sigma dW_t\)
- Credit spread:
  - CIR \(ds_t = \kappa(\theta - s_t)dt + \sigma \sqrt{s_t}\, dW_t\) with Feller‑safeguards
  - Market‑anchored: mean anchored to hazard curve average; initial to first segment; volatility scaled from CDS index vol, then mapped to hazard via \(s \approx (1-R)\lambda\)

Correlation across the 3 factors is supported via a 3×3 matrix. RNG supports Philox or Sobol (QMC). Zero volatility reduces stoch to deterministic (“parity mode”).

---

## APIs and workflows

### Pricing

```rust
use finstack_valuations::instruments::fixed_income::revolving_credit::pricer::unified::RevolvingCreditPricer;

// Deterministic (or fallback fast path)
let pv = RevolvingCreditPricer::price(&facility, &market, as_of)?; // auto‑dispatch

// Explicit deterministic
let pv_det = RevolvingCreditPricer::price_deterministic(&facility, &market, as_of)?;

// Stochastic with full path capture
let enhanced = RevolvingCreditPricer::price_with_paths(&facility_stoch, &market, as_of)?;
let mean_pv = enhanced.mc_result.estimate.mean;
let per_path = &enhanced.path_results; // PVs, cashflows, and 3‑factor trajectories
```

### Stochastic utilization spec

```rust
use finstack_valuations::instruments::fixed_income::revolving_credit::types::{
    StochasticUtilizationSpec, UtilizationProcess
};

let stoch = StochasticUtilizationSpec {
    utilization_process: UtilizationProcess::MeanReverting {
        target_rate: 0.5,
        speed: 1.0,
        volatility: 0.15,
    },
    num_paths: 10_000,
    seed: Some(42),
    antithetic: false,
    use_sobol_qmc: false,
    mc_config: None, // or Some(McConfig { ... }) for stochastic dynamics
};
```

Optional `McConfig` enables rate and credit dynamics, correlation, and market anchoring to a hazard curve.

### Cashflow schedules

Deterministic and stochastic modes both produce `CashFlowSchedule`. In deterministic mode, the engine slices periods around events and posts interest/fees at period end. In stochastic mode, per‑period average utilization drives accruals, with principal deltas posted at period end and a terminal repayment at maturity for any outstanding balance.

### Calendars and schedules

Payment schedules (and floating reset schedules) are built from `commitment_date → maturity_date` using the configured `Tenor`, and optionally adjusted using a calendar from `attributes` (e.g., `calendar_id="NYC"`). A sentinel date can be appended for exclusive‑end period aggregation where needed.

---

## Metrics

Standard risk metrics are available via the valuations registry:

- DV01 and CS01: unified spread sensitivity using symmetric bumping
- Theta and Bucketed DV01: via common metric helpers

Facility‑specific metrics:

- `utilization_rate`
- `available_capacity`
- `weighted_average_cost` (approximate)

IRR utilities are provided for explicit analysis of generated cashflows rather than as implicit metrics:

```rust
use finstack_valuations::instruments::fixed_income::revolving_credit::metrics::irr::calculate_path_irr;

let irr_opt = calculate_path_irr(&cashflows_as_(t, amt), base_date, day_count);
```

---

## Determinism, parity, and testing

- Deterministic vs stochastic parity: with zero vol and aligned configurations, the stochastic engine matches deterministic PV to tight tolerances (floating rate tolerances are slightly relaxed due to interpolation and period vs point‑in‑time differences).
- Property tests assert invariants: utilization bounds, undrawn arithmetic, event/balance consistency, cashflow ordering, non‑negative fees.
- Unit tests cover fee math, discounting utilities, and survival computations.

---

## Implementation details and design choices

- Unified cashflow engine: one code path for both deterministic and stochastic modes ensures consistent math and reduces drift.
- Survival weighting: either static from a hazard curve at cashflow dates or dynamic from simulated credit spreads mapped to hazard and integrated to cumulative survival.
- Principal flow handling:
  - Deterministic: principal flows occur exactly on event dates; sub‑period accruals computed with event slicing
  - Stochastic: principal deltas from utilization changes are posted at period end; accruals use average utilization in the period
- Floating rates: deterministic forward projection with margin and optional floors; HW1F available via `McConfig` when stochastic rates are required.
- Rounding and zero guards: consistent `RoundingContext` checks avoid emitting noise cashflows; safeguards applied to CIR params (e.g., Feller condition) to maintain numerical stability.
- Stochastic paths and advanced credit/rate dynamics use the workspace Monte Carlo infrastructure; deterministic pricing and scheduling remain the simpler path for standard facilities.

---

## Limitations / Known Issues

- CSA/funding adjustments are external; discounting is curve-driven without embedded FVA/CVA/DVA.
- Stochastic mode depends on Monte Carlo market and model inputs; without those inputs, use deterministic utilization paths.
- Covenant coverage is limited to the modeled triggers; bespoke covenants or restructuring events need explicit extensions.
- Multi-currency facilities are not modeled; all amounts assume a single currency throughout the lifecycle.

---

## Python and examples

Python bindings expose the same shapes and behaviors. Notebook examples that are
closest to revolving-credit workflows live in:

- `finstack-py/examples/02_pricing/instruments/loans_and_credit_facilities.ipynb`
- `finstack-py/examples/README.md`

---

## Extensibility

The design allows for:

- Additional utilization processes (e.g., jump‑diffusion, regime‑switching)
- Alternative credit or rate models
- More fee types and covenant modeling
- Enhanced cashflow tagging and reporting

All extensions should preserve the unified engine paradigm to maintain parity and keep PV/metrics consistent across modes.

## Pricing Methodology

- Deterministic engine: generates draws/repays, fees, and interest using schedules and rate specs; discounts cashflows via curve.
- Stochastic engine (requires `mc`): simulates utilization, rates, and credit spread factors with correlation; maps to cashflows via unified engine.
- Hazard/survival weighting optionally applied for credit risk; supports fee tiers and PIK/cash splits where configured.

## Metrics

- PV, facility-level DV01/CS01/Theta/Bucketed DV01 via generic calculators using cashflow outputs.
- Utilization metrics (peak/average), fee attribution, and carry/roll analyses.
- Scenario metrics from stochastic paths: distribution of utilization, loss-adjusted PV, and covenant breach statistics when modeled.
